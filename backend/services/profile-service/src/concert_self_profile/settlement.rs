//! Actual configured-Profile settlement proof to the canonical Auth/Friend
//! owners. A transaction outcome never closes Concert's root response scope.
use super::{
    guard::{binding_digest, canonical, wire_identity},
    identity::NativeStatus,
    operation::Operation,
    read_reference::ReadReference,
    server::{no_store, status, SelfProfileService},
    wire::lease,
};
use platform::{
    actor_delegation::authority_deadline::AuthorityRpcDeadline,
    grpc_auth::{VerifiedCaller, Workload},
    grpc_method_policy::MethodPolicy,
};
use prost::Message;
use sha2::{Digest, Sha256};
use tonic::{Request, Response, Status};
use uuid::Uuid;
pub static SETTLEMENT_POLICY: MethodPolicy = MethodPolicy {
    service: "authority.lease.v1.ProducerSettlementService",
    workload: Some(Workload::Profile),
    methods: &[
        ("ResolveTransaction", &[Workload::Auth, Workload::Friend]),
        ("ResolveScope", &[Workload::Auth, Workload::Friend]),
    ],
    actor_fields: &[],
};
impl SelfProfileService {
    pub fn settlement_server(
        self,
    ) -> lease::producer_settlement_service_server::ProducerSettlementServiceServer<Self> {
        lease::producer_settlement_service_server::ProducerSettlementServiceServer::new(self)
            .max_decoding_message_size(65536)
            .max_encoding_message_size(32768)
    }
}
#[tonic::async_trait]
impl lease::producer_settlement_service_server::ProducerSettlementService for SelfProfileService {
    async fn resolve_transaction(
        &self,
        request: Request<lease::ResolveTransactionRequest>,
    ) -> Result<Response<lease::ResolveTransactionResponse>, Status> {
        let deadline = system_deadline(&request)?;
        let message = request.into_inner();
        canonical(&message.registration_id)?;
        let binding = message.binding.ok_or_else(denied)?;
        let digest = binding_digest(&binding)?;
        let transaction = message.transaction.ok_or_else(denied)?;
        let reference =
            Uuid::from_slice(&transaction.settlement_reference).map_err(|_| denied())?;
        if reference.is_nil() || transaction.producer != lease::Producer::Profile as i32 {
            return Err(denied());
        }
        let work = async {
            let state = if let Some(operation) = Operation::load(&self.pool, reference, deadline)
                .await
                .map_err(status)?
            {
                if operation.binding_proto != binding.encode_to_vec()
                    || operation.binding_digest != digest
                    || wire_identity(&operation.identity, reference) != transaction
                {
                    return Err(denied());
                }
                // Prepared effects may be recovered here, but only by the
                // configured actual Concert decision query, never by this caller.
                self.settle(&operation, deadline).await.map_err(status)?
            } else if let Some(read) = ReadReference::load(&self.pool, reference)
                .await
                .map_err(status)?
            {
                if read.binding_proto != binding.encode_to_vec()
                    || read.binding_digest != digest
                    || wire_identity(&read.identity, reference) != transaction
                {
                    return Err(denied());
                }
                read.status(&self.pool, deadline).await.map_err(status)?
            } else {
                NativeStatus::Unknown
            };
            let state = match state {
                NativeStatus::InProgress | NativeStatus::Prepared => {
                    lease::TransactionStatus::InProgress
                }
                NativeStatus::Committed => lease::TransactionStatus::Committed,
                NativeStatus::Aborted => lease::TransactionStatus::Aborted,
                NativeStatus::Unknown => lease::TransactionStatus::Unknown,
            };
            let evidence = if matches!(
                state,
                lease::TransactionStatus::Committed | lease::TransactionStatus::Aborted
            ) {
                let mut hash = Sha256::new();
                hash.update(b"mcr profile self native settlement v1\0");
                hash.update(digest);
                hash.update(transaction.encode_to_vec());
                hash.update((state as i32).to_be_bytes());
                hash.finalize().to_vec()
            } else {
                Vec::new()
            };
            Ok::<_, Status>(lease::ResolveTransactionResponse {
                transaction: Some(transaction),
                binding_digest: digest.to_vec(),
                status: state as i32,
                receipt_evidence: evidence,
            })
        };
        let reply = tokio::time::timeout_at(deadline.stop(), work)
            .await
            .map_err(|_| Status::deadline_exceeded("Profile settlement unresolved"))??;
        super::identity::remaining(deadline).map_err(status)?;
        Ok(no_store(reply))
    }
    async fn resolve_scope(
        &self,
        request: Request<lease::ResolveScopeRequest>,
    ) -> Result<Response<lease::ResolveScopeResponse>, Status> {
        let deadline = system_deadline(&request)?;
        let scope = request.into_inner().scope.ok_or_else(denied)?;
        canonical(&scope.instance_id)?;
        canonical(&scope.scope_id)?;
        if scope.binding_digest.len() != 32 {
            return Err(denied());
        }
        super::identity::remaining(deadline).map_err(status)?;
        // Profile owns no HTTP response delivery registry. Only the current
        // Concert root can prove its sealed, finished response scope.
        Ok(no_store(lease::ResolveScopeResponse {
            scope: Some(scope),
            sealed_and_finished: false,
        }))
    }
}
fn system_deadline<T>(request: &Request<T>) -> Result<AuthorityRpcDeadline, Status> {
    if !matches!(
        request.extensions().get::<VerifiedCaller>(),
        Some(VerifiedCaller(Workload::Auth | Workload::Friend))
    ) {
        return Err(denied());
    }
    let original = platform::call_deadline::current()
        .ok_or_else(|| Status::invalid_argument("Original settlement deadline required"))?;
    AuthorityRpcDeadline::within_original(
        chrono::Utc::now().timestamp_micros() + 5_000_000,
        original,
        super::server::BUDGET,
    )
}
fn denied() -> Status {
    Status::permission_denied("Profile settlement binding denied")
}
