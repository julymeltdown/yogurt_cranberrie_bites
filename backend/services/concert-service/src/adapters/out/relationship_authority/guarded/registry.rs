use super::{contract::*, proto, scope::OwnerGuardScope, wire};
use platform::actor_delegation::owner_binding::{secret_hash, secret_matches};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
};
use tonic::Status;
use uuid::Uuid;
/// The owning Concert implementation persists these transitions. No in-memory
/// success default exists and no absent process/record may attest completion.
#[async_trait::async_trait]
pub trait ScopeLifecycle: Send + Sync {
    async fn register(
        &self,
        scope: &proto::ProducerScope,
        binding: &GuardBinding,
    ) -> Result<(), Status>;
    async fn register_transaction(
        &self,
        scope: &proto::ProducerScope,
        transaction: &proto::TransactionIdentity,
    ) -> Result<(), Status>;
    async fn record_tracking(
        &self,
        scope: &proto::ProducerScope,
        transaction: &proto::TransactionIdentity,
        owner: platform::grpc_auth::Workload,
        registration: &str,
    ) -> Result<(), Status>;
    async fn confirm_tracking(
        &self,
        scope: &proto::ProducerScope,
        transaction: &proto::TransactionIdentity,
        auth_registration: &str,
        friend_registration: &str,
    ) -> Result<(), Status>;
    /// Persist a closed private intent BEFORE any remote child RPC or local
    /// PREPARE can be dispatched. Duplicate exact intent is idempotent; another
    /// reference under the same (kind, intent UUID) must be rejected.
    async fn register_child_intent(
        &self,
        scope: &proto::ProducerScope,
        reference: &super::ChildRecoveryReference,
    ) -> Result<(), Status>;
    async fn record_child_completed(
        &self,
        scope: &proto::ProducerScope,
        reference: &super::ChildRecoveryReference,
    ) -> Result<(), Status>;
    /// Resolve ALL preallocated child intents, including cancelled/lost-reply
    /// children without a live Arc handle. Actual owner/native proofs only.
    async fn settle_child_intents(&self, scope: &proto::ProducerScope) -> Result<(), Status>;
    async fn mark_serialized(&self, scope: &proto::ProducerScope) -> Result<(), Status>;
    async fn seal(&self, scope: &proto::ProducerScope) -> Result<(), Status>;
    /// Proves only that this root can issue no more work, after body exhaustion
    /// and all admitted callbacks finish. Owners resolve producer transactions
    /// independently AFTER this transition, before releasing their guards.
    async fn finish_root_work(&self, scope: &proto::ProducerScope) -> Result<(), Status>;
    /// Called only with the supervisor's private destroyed-producer proof and
    /// sealed/drained callback admission. Older lifecycle adapters fail closed.
    async fn finish_cancelled_root_work(
        &self,
        _scope: &proto::ProducerScope,
    ) -> Result<(), Status> {
        Err(Status::unavailable(
            "Durable cancellation proof unsupported",
        ))
    }
    async fn record_cancellation_settled(
        &self,
        _scope: &proto::ProducerScope,
    ) -> Result<(), Status> {
        Err(Status::unavailable(
            "Durable cancellation settlement unsupported",
        ))
    }
}
struct Entry {
    digest: [u8; 32],
    secret_hash: [u8; 32],
    sealed: bool,
    inflight: Arc<AtomicUsize>,
    scope: Option<Arc<OwnerGuardScope>>,
}
pub struct ScopeRegistry {
    pub(super) instance: Uuid,
    pub(super) lifecycle: Arc<dyn ScopeLifecycle>,
    entries: Mutex<HashMap<Uuid, Entry>>,
}
impl ScopeRegistry {
    pub fn new(instance: Uuid, lifecycle: Arc<dyn ScopeLifecycle>) -> Result<Arc<Self>, Status> {
        if instance.is_nil() {
            return Err(Status::invalid_argument("Invalid producer instance"));
        }
        Ok(Arc::new(Self {
            instance,
            lifecycle,
            entries: Mutex::new(HashMap::new()),
        }))
    }
    pub(super) fn reserve(
        self: &Arc<Self>,
        binding: &GuardBinding,
        secret: &[u8],
    ) -> Result<proto::ProducerScope, Status> {
        let mut entries = self.entries.lock().map_err(unavailable)?;
        if entries.len() >= 16 {
            return Err(Status::resource_exhausted("Canonical root scope capacity"));
        }
        let id = Uuid::new_v4();
        let scope = proto::ProducerScope {
            instance_id: self.instance.to_string(),
            scope_id: id.to_string(),
            binding_digest: binding.digest().to_vec(),
        };
        entries.insert(
            id,
            Entry {
                digest: binding.digest(),
                secret_hash: secret_hash(secret)?,
                sealed: false,
                inflight: Arc::new(AtomicUsize::new(0)),
                scope: None,
            },
        );
        Ok(scope)
    }
    pub(super) fn activate(&self, scope: Arc<OwnerGuardScope>) -> Result<(), Status> {
        let id = wire::canonical_uuid(&scope.producer_scope().scope_id)?;
        let mut entries = self.entries.lock().map_err(unavailable)?;
        let entry = entries
            .get_mut(&id)
            .ok_or_else(|| Status::unavailable("Missing registered root scope"))?;
        if entry.sealed || entry.scope.is_some() {
            return Err(Status::unavailable("Root scope not activatable"));
        }
        entry.scope = Some(scope);
        Ok(())
    }
    pub fn lookup_root_read(&self, view: &proto::RootReadView) -> Result<RootReadPermit, Status> {
        let scope = view
            .scope
            .as_ref()
            .ok_or_else(|| Status::permission_denied("Missing root scope"))?;
        if wire::canonical_uuid(&scope.instance_id)? != self.instance {
            return Err(Status::unavailable("Unknown producer instance"));
        }
        let id = wire::canonical_uuid(&scope.scope_id)?;
        let binding = view
            .binding
            .as_ref()
            .ok_or_else(|| Status::permission_denied("Missing root binding"))?;
        let digest = wire::wire_binding_digest(binding)?;
        let entries = self.entries.lock().map_err(unavailable)?;
        let entry = entries
            .get(&id)
            .ok_or_else(|| Status::unavailable("Unknown root scope"))?;
        if entry.sealed
            || entry.digest != digest
            || scope.binding_digest.as_slice() != digest
            || !secret_matches(&view.view_secret, &entry.secret_hash)
        {
            return Err(Status::permission_denied("Invalid root read capability"));
        }
        let actual = entry
            .scope
            .as_ref()
            .ok_or_else(|| Status::unavailable("Root scope acquiring"))?
            .clone();
        actual.binding().check()?;
        entry.inflight.fetch_add(1, Ordering::AcqRel);
        Ok(RootReadPermit {
            scope: actual,
            inflight: entry.inflight.clone(),
        })
    }
    /// Root's separately authenticated Gateway delivery handler may locate an
    /// existing scope for settlement after expiry. This method grants no read,
    /// enrollment or mutation and must never be routed as an unguarded RPC.
    pub fn lookup_for_settlement(
        &self,
        scope: &proto::ProducerScope,
    ) -> Result<Arc<OwnerGuardScope>, Status> {
        if wire::canonical_uuid(&scope.instance_id)? != self.instance {
            return Err(Status::unavailable("Unknown producer instance"));
        }
        let id = wire::canonical_uuid(&scope.scope_id)?;
        let entries = self.entries.lock().map_err(unavailable)?;
        let entry = entries
            .get(&id)
            .ok_or_else(|| Status::unavailable("Unknown root scope"))?;
        if scope.binding_digest.as_slice() != entry.digest {
            return Err(Status::permission_denied("Root scope mismatch"));
        }
        entry
            .scope
            .clone()
            .ok_or_else(|| Status::unavailable("Root scope not acquired"))
    }
    pub(super) fn seal(&self, scope: &proto::ProducerScope) -> Result<Arc<AtomicUsize>, Status> {
        let id = wire::canonical_uuid(&scope.scope_id)?;
        let mut entries = self.entries.lock().map_err(unavailable)?;
        let entry = entries
            .get_mut(&id)
            .ok_or_else(|| Status::unavailable("Unknown root scope"))?;
        entry.sealed = true;
        Ok(entry.inflight.clone())
    }
    pub(super) fn retire(&self, scope: &proto::ProducerScope) -> Result<(), Status> {
        let id = wire::canonical_uuid(&scope.scope_id)?;
        self.entries.lock().map_err(unavailable)?.remove(&id);
        Ok(())
    }
    pub(super) fn weak(self: &Arc<Self>) -> Weak<Self> {
        Arc::downgrade(self)
    }
}
/// Callback admission, held through the actual local subject guard acquisition.
/// It carries no completion secret. The registry prevents new permits on seal.
pub struct RootReadPermit {
    scope: Arc<OwnerGuardScope>,
    inflight: Arc<AtomicUsize>,
}
impl RootReadPermit {
    pub fn scope(&self) -> Arc<OwnerGuardScope> {
        self.scope.clone()
    }
}
impl Drop for RootReadPermit {
    fn drop(&mut self) {
        self.inflight.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Cleanup proof is restricted to an acquisition whose capability has never
/// escaped this private constructor. Ordinary live scopes have NO such Drop.
pub(super) struct UnexposedAcquisition {
    registry: Arc<ScopeRegistry>,
    producer: proto::ProducerScope,
    armed: bool,
}
impl UnexposedAcquisition {
    pub fn new(registry: Arc<ScopeRegistry>, producer: proto::ProducerScope) -> Self {
        Self {
            registry,
            producer,
            armed: true,
        }
    }
    pub fn disarm(&mut self) {
        self.armed = false;
    }
    pub async fn settle(&self) {
        settle_unexposed(self.registry.clone(), self.producer.clone()).await;
    }
}
impl Drop for UnexposedAcquisition {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        // Synchronously deny any late root callback before a cleanup task can
        // record no-root-work. No owner transaction is released here.
        if self.registry.seal(&self.producer).is_err() {
            return;
        }
        let registry = self.registry.clone();
        let producer = self.producer.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                settle_unexposed(registry, producer).await;
            });
        }
        // Missing runtime or unknown settlement retains counted quarantine.
    }
}
async fn settle_unexposed(registry: Arc<ScopeRegistry>, producer: proto::ProducerScope) {
    if registry.seal(&producer).is_err() {
        return;
    }
    let cleanup = async {
        registry.lifecycle.seal(&producer).await?;
        registry.lifecycle.finish_root_work(&producer).await
    };
    if tokio::time::timeout(std::time::Duration::from_secs(2), cleanup)
        .await
        .is_ok_and(|result| result.is_ok())
    {
        let _ = registry.retire(&producer);
    }
}
