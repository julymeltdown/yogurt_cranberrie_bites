//! Auth/Friend-only system settlement. This grants no read authority and uses no
//! expired browser actor. Actual producer adapters own committed receipt proof.
use super::{
    canonical_uuid,
    scope_ledger::{PgScopeLedger, ProducerTransaction, ScopePhase},
    scope_lifecycle,
};
use crate::adapters::out::relationship_authority::guarded::proto;
use platform::{
    actor_delegation::owner_binding::OwnerBinding,
    grpc_auth::{VerifiedCaller, Workload},
    grpc_method_policy::MethodPolicy,
};
use std::{sync::Arc, time::Duration};
use tonic::{Request, Response, Status};

pub const POLICY: MethodPolicy = MethodPolicy {
    service: "authority.lease.v1.ProducerSettlementService",
    workload: Some(Workload::Concert),
    methods: &[
        ("ResolveTransaction", &[Workload::Auth, Workload::Friend]),
        ("ResolveScope", &[Workload::Auth, Workload::Friend]),
    ],
    actor_fields: &[],
};
pub struct NativeTransactionResolution {
    pub status: proto::TransactionStatus,
    pub receipt_evidence: Vec<u8>,
}
#[async_trait::async_trait]
pub trait NativeTransactionResolver: Send + Sync {
    async fn resolve(
        &self,
        binding: &OwnerBinding,
        identity: &ProducerTransaction,
    ) -> Result<NativeTransactionResolution, Status>;
}
/// Actual configured Concert store proof for rollback/in-progress only.
/// COMMITTED needs a producer-specific same-transaction receipt/witness and is
/// deliberately UNKNOWN here. This adapter never invents a commit receipt.
pub struct PgNativeAbortResolver {
    pool: sqlx::PgPool,
    instance: uuid::Uuid,
}
impl PgNativeAbortResolver {
    pub fn new(pool: sqlx::PgPool, instance: uuid::Uuid) -> Result<Self, Status> {
        if instance.is_nil() {
            return Err(Status::invalid_argument("Invalid native producer instance"));
        }
        Ok(Self { pool, instance })
    }
}
#[async_trait::async_trait]
impl NativeTransactionResolver for PgNativeAbortResolver {
    async fn resolve(
        &self,
        binding: &OwnerBinding,
        identity: &ProducerTransaction,
    ) -> Result<NativeTransactionResolution, Status> {
        identity.digest()?;
        if binding.digest()? != identity.effect_binding_digest {
            return Err(Status::permission_denied(
                "Native settlement binding mismatch",
            ));
        }
        let mut status = proto::TransactionStatus::Unknown;
        if identity.producer == "CONCERT" && identity.producer_instance == self.instance {
            let mut tx = self
                .pool
                .begin()
                .await
                .map_err(|_| Status::unavailable("Native settlement unavailable"))?;
            let actual:(String,i64)=sqlx::query_as("SELECT s.system_identifier::text,d.oid::bigint FROM pg_control_system() s CROSS JOIN pg_database d WHERE d.datname=current_database()")
                .fetch_one(&mut *tx).await.map_err(|_|Status::unavailable("Native settlement unavailable"))?;
            if actual
                == (
                    identity.system_identifier.clone(),
                    identity.database_oid as i64,
                )
            {
                let native: Option<String> =
                    sqlx::query_scalar("SELECT pg_xact_status($1::text::xid8)")
                        .bind(&identity.full_xid)
                        .fetch_one(&mut *tx)
                        .await
                        .map_err(|_| Status::unavailable("Native settlement unavailable"))?;
                status = match native.as_deref() {
                    Some("aborted") => proto::TransactionStatus::Aborted,
                    Some("in progress") => proto::TransactionStatus::InProgress,
                    _ => proto::TransactionStatus::Unknown,
                };
            }
            tx.commit()
                .await
                .map_err(|_| Status::unavailable("Native settlement unavailable"))?;
        }
        Ok(NativeTransactionResolution {
            status,
            receipt_evidence: vec![],
        })
    }
}
#[derive(Clone)]
pub struct ProducerSettlementService {
    ledger: PgScopeLedger,
    resolver: Arc<dyn NativeTransactionResolver>,
    capacity: Arc<tokio::sync::Semaphore>,
}
impl ProducerSettlementService {
    pub fn new(ledger: PgScopeLedger, resolver: Arc<dyn NativeTransactionResolver>) -> Self {
        Self {
            ledger,
            resolver,
            capacity: Arc::new(tokio::sync::Semaphore::new(16)),
        }
    }
    pub fn server(
        self,
    ) -> proto::producer_settlement_service_server::ProducerSettlementServiceServer<Self> {
        proto::producer_settlement_service_server::ProducerSettlementServiceServer::new(self)
            .max_decoding_message_size(16 * 1024)
            .max_encoding_message_size(16 * 1024)
    }
}
fn owner<T>(request: &Request<T>) -> Result<Workload, Status> {
    match request.extensions().get::<VerifiedCaller>() {
        Some(VerifiedCaller(owner @ (Workload::Auth | Workload::Friend))) => Ok(*owner),
        _ => Err(Status::unauthenticated(
            "Verified settlement owner required",
        )),
    }
}
fn response<T>(value: T) -> Response<T> {
    let mut result = Response::new(value);
    result.metadata_mut().insert(
        "cache-control",
        "no-store".parse().expect("static metadata"),
    );
    result
}
#[tonic::async_trait]
impl proto::producer_settlement_service_server::ProducerSettlementService
    for ProducerSettlementService
{
    async fn resolve_scope(
        &self,
        request: Request<proto::ResolveScopeRequest>,
    ) -> Result<Response<proto::ResolveScopeResponse>, Status> {
        owner(&request)?;
        let _permit = self
            .capacity
            .clone()
            .try_acquire_owned()
            .map_err(|_| Status::unavailable("Settlement capacity unavailable"))?;
        let scope = request
            .into_inner()
            .scope
            .ok_or_else(|| Status::invalid_argument("Missing producer scope"))?;
        let key = scope_lifecycle::key(&scope)?;
        let phase = tokio::time::timeout(Duration::from_secs(2), self.ledger.resolve_scope(&key))
            .await
            .map_err(|_| Status::unavailable("Scope settlement unresolved"))??;
        Ok(response(proto::ResolveScopeResponse {
            scope: Some(scope),
            sealed_and_finished: phase == ScopePhase::RootFinished,
        }))
    }
    async fn resolve_transaction(
        &self,
        request: Request<proto::ResolveTransactionRequest>,
    ) -> Result<Response<proto::ResolveTransactionResponse>, Status> {
        let owner = owner(&request)?;
        let _permit = self
            .capacity
            .clone()
            .try_acquire_owned()
            .map_err(|_| Status::unavailable("Settlement capacity unavailable"))?;
        let request = request.into_inner();
        let wire = request
            .transaction
            .ok_or_else(|| Status::invalid_argument("Missing native transaction"))?;
        let identity = scope_lifecycle::transaction(&wire)?;
        let incoming = scope_lifecycle::binding(
            &request
                .binding
                .ok_or_else(|| Status::invalid_argument("Missing settlement binding"))?,
        )?;
        if incoming.digest()? != identity.effect_binding_digest {
            return Err(Status::permission_denied("Settlement binding mismatch"));
        }
        let registration = canonical_uuid(&request.registration_id)?;
        let work = async {
            let (binding, ack, phase) = self
                .ledger
                .settlement_registration(owner, registration, &identity)
                .await?;
            if binding.digest()? != incoming.digest()? {
                return Err(Status::permission_denied("Settlement binding mismatch"));
            }
            let mut resolved = if phase == ScopePhase::RootFinished {
                self.resolver.resolve(&binding, &identity).await?
            } else {
                NativeTransactionResolution {
                    status: proto::TransactionStatus::Unknown,
                    receipt_evidence: vec![],
                }
            };
            if !ack && resolved.status != proto::TransactionStatus::Aborted {
                resolved = NativeTransactionResolution {
                    status: proto::TransactionStatus::Unknown,
                    receipt_evidence: vec![],
                };
            }
            if resolved.receipt_evidence.len() > 2048
                || resolved.status == proto::TransactionStatus::Unspecified
                || (resolved.status == proto::TransactionStatus::Committed
                    && resolved.receipt_evidence.is_empty())
            {
                return Err(Status::unavailable("Producer settlement proof invalid"));
            }
            Ok(response(proto::ResolveTransactionResponse {
                transaction: Some(wire),
                binding_digest: binding.digest()?.to_vec(),
                status: resolved.status as i32,
                receipt_evidence: resolved.receipt_evidence,
            }))
        };
        tokio::time::timeout(Duration::from_secs(2), work)
            .await
            .map_err(|_| Status::unavailable("Transaction settlement unresolved"))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settlement_is_system_only_and_has_no_actor_or_browser_method() {
        for workload in Workload::ALL {
            for method in ["ResolveTransaction", "ResolveScope"] {
                let path = format!("/authority.lease.v1.ProducerSettlementService/{method}");
                assert_eq!(
                    POLICY.allows(&path, workload),
                    matches!(workload, Workload::Auth | Workload::Friend)
                );
                assert!(POLICY.actor_fields_for(&path).is_none());
            }
        }
        assert!(!POLICY.allows(
            "/authority.lease.v1.ProducerSettlementService/Dispatch",
            Workload::Auth
        ));
    }
}
