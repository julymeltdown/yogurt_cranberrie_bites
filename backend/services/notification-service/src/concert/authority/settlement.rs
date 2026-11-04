//! Independently authenticated owner recovery. Never claims Root scope closure.
use super::{canonical, AuthorityStore, NativeOutcome};
use crate::concert::transport::lease;
use platform::{
    actor_delegation::authority_deadline::AuthorityRpcDeadline,
    grpc_auth::{VerifiedCaller, Workload},
    grpc_method_policy::MethodPolicy,
};
use prost::Message;
use sha2::{Digest, Sha256};
use tonic::{Request, Response, Status};

pub static SETTLEMENT_POLICY: MethodPolicy = MethodPolicy {
    service: "authority.lease.v1.ProducerSettlementService",
    workload: Some(Workload::Notification),
    methods: &[
        ("ResolveTransaction", &[Workload::Auth, Workload::Friend]),
        ("ResolveScope", &[Workload::Auth, Workload::Friend]),
    ],
    actor_fields: &[],
};
#[derive(Clone)]
pub struct NotificationSettlement {
    pub(crate) store: AuthorityStore,
}
impl NotificationSettlement {
    pub fn server(
        self,
    ) -> lease::producer_settlement_service_server::ProducerSettlementServiceServer<Self> {
        lease::producer_settlement_service_server::ProducerSettlementServiceServer::new(self)
            .max_decoding_message_size(65536)
            .max_encoding_message_size(32768)
    }
}
#[tonic::async_trait]
impl lease::producer_settlement_service_server::ProducerSettlementService
    for NotificationSettlement
{
    async fn resolve_transaction(
        &self,
        request: Request<lease::ResolveTransactionRequest>,
    ) -> Result<Response<lease::ResolveTransactionResponse>, Status> {
        let (deadline, auth) = deadline(&request)?;
        let message = request.into_inner();
        let binding = message.binding.ok_or_else(denied)?;
        let transaction = message.transaction.ok_or_else(denied)?;
        let (intent, state) = self
            .store
            .resolve(
                &binding,
                &transaction,
                &message.registration_id,
                auth,
                deadline,
            )
            .await?;
        let state = match state {
            NativeOutcome::InProgress => lease::TransactionStatus::InProgress,
            NativeOutcome::Committed => lease::TransactionStatus::Committed,
            NativeOutcome::Aborted => lease::TransactionStatus::Aborted,
            NativeOutcome::NoEffect | NativeOutcome::Unknown => lease::TransactionStatus::Unknown,
        };
        let evidence = if matches!(
            state,
            lease::TransactionStatus::Committed | lease::TransactionStatus::Aborted
        ) {
            let mut hash = Sha256::new();
            hash.update(b"mcr notification native settlement v1\0");
            hash.update(intent.binding_digest);
            hash.update(transaction.encode_to_vec());
            hash.update((state as i32).to_be_bytes());
            hash.finalize().to_vec()
        } else {
            Vec::new()
        };
        Ok(no_store(lease::ResolveTransactionResponse {
            transaction: Some(transaction),
            binding_digest: intent.binding_digest.to_vec(),
            status: state as i32,
            receipt_evidence: evidence,
        }))
    }
    async fn resolve_scope(
        &self,
        request: Request<lease::ResolveScopeRequest>,
    ) -> Result<Response<lease::ResolveScopeResponse>, Status> {
        let (deadline, _) = deadline(&request)?;
        let scope = request.into_inner().scope.ok_or_else(denied)?;
        canonical(&scope.instance_id)?;
        canonical(&scope.scope_id)?;
        if scope.binding_digest.len() != 32 || scope.binding_digest.iter().all(|b| *b == 0) {
            return Err(denied());
        }
        deadline.remaining(std::time::Duration::from_secs(5), deadline.expires_micros())?;
        Ok(no_store(lease::ResolveScopeResponse {
            scope: Some(scope),
            sealed_and_finished: false,
        }))
    }
}
fn deadline<T>(request: &Request<T>) -> Result<(AuthorityRpcDeadline, bool), Status> {
    let auth = match request.extensions().get::<VerifiedCaller>() {
        Some(VerifiedCaller(Workload::Auth)) => true,
        Some(VerifiedCaller(Workload::Friend)) => false,
        _ => return Err(denied()),
    };
    let original = platform::call_deadline::current().ok_or_else(|| {
        Status::invalid_argument("Original Notification settlement deadline required")
    })?;
    Ok((
        AuthorityRpcDeadline::within_original(
            chrono::Utc::now().timestamp_micros() + 5_000_000,
            original,
            std::time::Duration::from_secs(5),
        )?,
        auth,
    ))
}
fn denied() -> Status {
    Status::permission_denied("Notification settlement binding denied")
}
pub(super) fn no_store<T>(value: T) -> Response<T> {
    let mut reply = Response::new(value);
    reply.metadata_mut().insert(
        "cache-control",
        "no-store".parse().expect("static metadata"),
    );
    reply
}
