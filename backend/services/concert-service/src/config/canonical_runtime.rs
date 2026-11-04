//! Shared canonical scope composition for live owners and system recovery.
//!
//! Construct once for the configured producer instance, then pass clones to
//! feature composition. The caller supplies the actual typed child settler and
//! native resolver; this core grants no authority and starts no background task.
use crate::{
    adapters::out::relationship_authority::guarded::{CanonicalOwnerFence, ScopeRegistry},
    product_rpc::{
        child_recovery::{ChildRecoverySettings, ChildRecoveryWorker},
        delivery::DeliveryPublisher,
        scope_ledger::{ChildIntentSettler, ChildRecoveryKeys, PgScopeLedger},
        settlement::{NativeTransactionResolver, ProducerSettlementService},
    },
};
use platform::{
    actor_delegation::relationship_read::RelationshipReadSigner, grpc_tls::TlsMaterial,
};
use sqlx::PgPool;
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;

#[derive(Clone)]
pub struct CanonicalRuntimeCore {
    ledger: PgScopeLedger,
    registry: Arc<ScopeRegistry>,
    delivery: DeliveryPublisher,
}
impl CanonicalRuntimeCore {
    /// Preserve the caller's configured instance, retained keys and typed
    /// settler. Validation is delegated to the existing production constructors.
    pub fn new(
        pool: PgPool,
        instance: Uuid,
        keys: Arc<ChildRecoveryKeys>,
        settler: Arc<dyn ChildIntentSettler>,
    ) -> Result<Self, Status> {
        let ledger = PgScopeLedger::new(pool, instance, keys, settler)?;
        let registry = ScopeRegistry::new(instance, Arc::new(ledger.clone()))?;
        let delivery = DeliveryPublisher::new(ledger.clone(), registry.clone());
        Ok(Self {
            ledger,
            registry,
            delivery,
        })
    }

    pub fn ledger(&self) -> PgScopeLedger {
        self.ledger.clone()
    }

    pub fn registry(&self) -> Arc<ScopeRegistry> {
        self.registry.clone()
    }

    pub fn delivery(&self) -> DeliveryPublisher {
        self.delivery.clone()
    }

    /// Build the actual Auth/Friend fence against this core's existing registry.
    /// Endpoint, credential, TLS and signer validation remain with that adapter.
    pub fn owner_fence(
        &self,
        auth_endpoint: &str,
        friend_endpoint: &str,
        credential: &str,
        tls: &TlsMaterial,
        signer: RelationshipReadSigner,
    ) -> anyhow::Result<CanonicalOwnerFence> {
        CanonicalOwnerFence::connect(
            auth_endpoint,
            friend_endpoint,
            credential,
            tls,
            signer,
            self.registry(),
        )
    }

    /// The caller supplies the actual producer-specific native proof adapter.
    pub fn settlement_service(
        &self,
        resolver: Arc<dyn NativeTransactionResolver>,
    ) -> ProducerSettlementService {
        ProducerSettlementService::new(self.ledger(), resolver)
    }

    /// Keep the same retained keys and typed settler as live handler scopes.
    /// The caller owns supervision and the worker's validated system settings.
    pub fn child_recovery_worker(
        &self,
        settings: ChildRecoverySettings,
    ) -> Result<ChildRecoveryWorker, Status> {
        ChildRecoveryWorker::new(self.ledger(), settings)
    }
}
