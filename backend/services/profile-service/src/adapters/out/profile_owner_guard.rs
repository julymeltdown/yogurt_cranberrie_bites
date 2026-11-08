//! Profile-scoped owner exclusion.
//!
//! The concert producer fences owner work through
//! `concert_service::adapters::out::relationship_authority::guarded`, whose
//! binding is keyed by `event_id` and whose target must start with
//! `/api/concert/events/{event}/`. A profile operation has no event, so this
//! module provides the profile analogue with one deliberate difference: the
//! binding is keyed by `profile_id`, and the target must never name an event.
//!
//! The shape is otherwise the same, on purpose:
//!
//! * a binding is a fingerprint, never authority;
//! * only a successful `acquire` against the registered profile scope may
//!   construct an `ProfileOwnerScope`;
//! * the scope holds a single-use delivery secret, a disclosure ceiling derived
//!   from the original deadline, and a supervised producer future that no
//!   detached task can outlive;
//! * returning from the producer NEVER completes the scope. Completion is a
//!   separate settlement operation after the actual HTTP body terminal.
use platform::actor_delegation::{
    authority_deadline::AuthorityRpcDeadline, exchange::VerifiedParentAuthority,
    owner_binding::OwnerBinding,
};
use ring::rand::{SecureRandom, SystemRandom};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fmt,
    future::Future,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, Weak,
    },
};
use tonic::Status;
use uuid::Uuid;

/// Longest single owner exclusion window, matching the concert guard.
pub const MAX_GUARD_DURATION: std::time::Duration = std::time::Duration::from_secs(5);

fn invalid() -> Status {
    Status::invalid_argument("Invalid profile owner binding")
}

/// A profile binding is not authority. Only acquisition against the registered
/// profile scope may construct an owner scope. Never log proofs or secrets.
#[derive(Clone)]
pub struct ProfileGuardBinding {
    pub(crate) actor: Uuid,
    pub(crate) profile: Uuid,
    pub(crate) family: Uuid,
    pub(crate) request: Uuid,
    pub(crate) operation: Arc<str>,
    #[allow(dead_code)]
    pub(crate) parent_method: Arc<str>,
    #[allow(dead_code)]
    pub(crate) parent_digest: [u8; 32],
    #[allow(dead_code)]
    pub(crate) parent_body_digest: [u8; 32],
    pub(crate) http_method: Arc<str>,
    pub(crate) http_target: Arc<str>,
    #[allow(dead_code)]
    pub(crate) body_digest: [u8; 32],
    pub(crate) deadline: AuthorityRpcDeadline,
    digest: [u8; 32],
}

impl fmt::Debug for ProfileGuardBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProfileGuardBinding([REDACTED])")
    }
}

impl ProfileGuardBinding {
    #[allow(clippy::too_many_arguments)]
    pub fn from_parent(
        parent: &VerifiedParentAuthority,
        actor: Uuid,
        profile: Uuid,
        family: Uuid,
        request: Uuid,
        operation: &str,
        http_method: &str,
        http_target: &str,
        body_digest: [u8; 32],
        original: AuthorityRpcDeadline,
    ) -> Result<Self, Status> {
        if actor != parent.actor()
            || [actor, profile, family, request].iter().any(Uuid::is_nil)
            || operation.is_empty()
            || operation.len() > 100
            || operation.chars().any(char::is_control)
            || !matches!(http_method, "GET" | "POST" | "PATCH" | "PUT" | "DELETE")
            // A profile operation never carries an event path. This is the
            // second, binding-level guard behind the request-level rejection.
            || http_target.contains("/events/")
            || !http_target.starts_with("/api/")
            || http_target.len() > 4096
            || http_target.chars().any(char::is_control)
            || body_digest == [0; 32]
        {
            return Err(Status::permission_denied("Invalid profile owner binding"));
        }
        let expires = parent
            .expires_at()
            .checked_mul(1_000_000)
            .ok_or_else(|| Status::permission_denied("Invalid parent deadline"))?
            .min(original.expires_micros());
        let deadline = AuthorityRpcDeadline::within_original(
            expires,
            original.stop().into_std(),
            MAX_GUARD_DURATION,
        )?;
        let wire_binding = OwnerBinding {
            actor_id: actor,
            // The profile owner binding is keyed by the profile, never an event.
            event_id: profile,
            session_family_id: family,
            request_id: request,
            operation: operation.into(),
            expires_unix_micros: deadline.expires_micros(),
            parent_digest: parent.parent_digest(),
            parent_body_digest: parent.request_digest(),
            parent_method: parent.request().method.into(),
            http_method: http_method.into(),
            http_target: http_target.into(),
            body_digest,
        };
        let digest = profile_binding_digest(&wire_binding)?;
        Ok(Self {
            digest,
            actor,
            profile,
            family,
            request,
            operation: operation.into(),
            parent_method: parent.request().method.into(),
            parent_digest: parent.parent_digest(),
            parent_body_digest: parent.request_digest(),
            http_method: http_method.into(),
            http_target: http_target.into(),
            body_digest,
            deadline,
        })
    }
    pub fn actor(&self) -> Uuid {
        self.actor
    }
    pub fn profile(&self) -> Uuid {
        self.profile
    }
    pub fn session_family(&self) -> Uuid {
        self.family
    }
    pub fn request_id(&self) -> Uuid {
        self.request
    }
    pub fn operation(&self) -> &str {
        &self.operation
    }
    pub fn http_target(&self) -> &str {
        &self.http_target
    }
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
    pub fn deadline(&self) -> AuthorityRpcDeadline {
        self.deadline
    }
    pub fn check(&self) -> Result<(), Status> {
        self.deadline
            .remaining(MAX_GUARD_DURATION, self.deadline.expires_micros())
            .map(|_| ())
    }
}

/// Profile-domain-separated fingerprint. The concert digest is not reused, so a
/// profile binding can never collide with an event binding that happens to carry
/// the same UUID in the event slot.
pub fn profile_binding_digest(binding: &OwnerBinding) -> Result<[u8; 32], Status> {
    if binding.event_id.is_nil() || binding.http_target.contains("/events/") {
        return Err(invalid());
    }
    let mut hash = Sha256::new();
    hash.update(b"mcr profile owner guard binding v1\0");
    for id in [
        binding.actor_id,
        binding.event_id,
        binding.session_family_id,
        binding.request_id,
    ] {
        hash.update(id.as_bytes());
    }
    for value in [
        &*binding.operation,
        &*binding.parent_method,
        &*binding.http_method,
        &*binding.http_target,
    ] {
        hash.update((value.len() as u64).to_be_bytes());
        hash.update(value.as_bytes());
    }
    hash.update(binding.parent_digest);
    hash.update(binding.parent_body_digest);
    hash.update(binding.body_digest);
    hash.update(binding.expires_unix_micros.to_be_bytes());
    Ok(hash.finalize().into())
}

/// Producer scope identity. This is the durable handle the ledger records and
/// the delivery capability names. It is not a grant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileProducerScope {
    pub instance_id: Uuid,
    pub scope_id: Uuid,
    pub binding_digest: [u8; 32],
}

impl ProfileProducerScope {
    pub fn validate(&self) -> Result<(), Status> {
        if self.instance_id.is_nil()
            || self.scope_id.is_nil()
            || self.binding_digest == [0; 32]
        {
            return Err(invalid());
        }
        Ok(())
    }
}

pub enum ScopePhase {
    Active,
    Sealed,
    RootFinished,
}

impl ScopePhase {
    pub fn parse(value: &str) -> Result<Self, Status> {
        match value {
            "ACTIVE" => Ok(Self::Active),
            "SEALED" => Ok(Self::Sealed),
            "ROOT_FINISHED" => Ok(Self::RootFinished),
            _ => Err(Status::unavailable("Unknown profile scope lifecycle")),
        }
    }
}

/// The registered scope set. Membership is process-local; the durable ledger is
/// the cross-restart record and is consulted by both publication and settlement.
#[derive(Default)]
pub struct ProfileScopeRegistry {
    scopes: Mutex<BTreeMap<Uuid, Weak<ProfileOwnerScope>>>,
}

impl ProfileScopeRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
    fn reserve(&self, binding: &ProfileGuardBinding) -> Result<ProfileProducerScope, Status> {
        let mut scope_id = [0u8; 16];
        SystemRandom::new()
            .fill(&mut scope_id)
            .map_err(|_| Status::unavailable("Profile authority entropy unavailable"))?;
        let scope = ProfileProducerScope {
            instance_id: *self.instance(),
            scope_id: Uuid::from_bytes(scope_id),
            binding_digest: binding.digest(),
        };
        scope.validate()?;
        Ok(scope)
    }
    fn instance(&self) -> &Uuid {
        static INSTANCE: std::sync::OnceLock<Uuid> = std::sync::OnceLock::new();
        INSTANCE.get_or_init(Uuid::new_v4)
    }
    fn activate(&self, scope: Arc<ProfileOwnerScope>) -> Result<(), Status> {
        let mut scopes = self
            .scopes
            .lock()
            .map_err(|_| Status::unavailable("Profile scope registry unavailable"))?;
        if scopes.contains_key(&scope.producer.scope_id) {
            return Err(Status::permission_denied("Profile scope already registered"));
        }
        scopes.insert(scope.producer.scope_id, Arc::downgrade(&scope));
        Ok(())
    }
    /// Number of scopes this process still holds. Diagnostic only; it is not a
    /// grant and it never settles anything.
    pub fn live(&self) -> usize {
        self.scopes
            .lock()
            .map(|scopes| scopes.values().filter(|scope| scope.strong_count() > 0).count())
            .unwrap_or(0)
    }
    pub fn lookup_for_settlement(
        &self,
        scope: &ProfileProducerScope,
    ) -> Result<Arc<ProfileOwnerScope>, Status> {
        let scopes = self
            .scopes
            .lock()
            .map_err(|_| Status::unavailable("Profile scope registry unavailable"))?;
        scopes
            .get(&scope.scope_id)
            .and_then(Weak::upgrade)
            .filter(|held| held.producer == *scope)
            .ok_or_else(|| Status::permission_denied("Profile scope is not held"))
    }
}

/// Only a successful acquisition against the registered profile scope may
/// construct this value. It owns the delivery secret and the completion flag.
pub struct ProfileOwnerScope {
    binding: ProfileGuardBinding,
    producer: ProfileProducerScope,
    delivery_secret: Mutex<Option<[u8; 32]>>,
    serialized: AtomicBool,
    complete: AtomicBool,
    sealed: AtomicBool,
    owned_work_started: AtomicBool,
}

impl ProfileOwnerScope {
    pub fn binding(&self) -> &ProfileGuardBinding {
        &self.binding
    }
    pub fn producer_scope(&self) -> ProfileProducerScope {
        self.producer.clone()
    }
    /// Bind the single-use delivery secret. Publication happens exactly once,
    /// before any protected producer work.
    pub(crate) fn bind_delivery(&self, secret: [u8; 32]) -> Result<(), Status> {
        let mut slot = self
            .delivery_secret
            .lock()
            .map_err(|_| Status::unavailable("Profile scope unavailable"))?;
        if slot.is_some() {
            return Err(Status::permission_denied("Profile delivery already published"));
        }
        *slot = Some(secret);
        Ok(())
    }
    /// The durable handle the settlement path needs. This is not authority: the
    /// settlement path still has to find the same in-memory scope and present
    /// the exact delivery secret.
    pub fn producer(&self) -> &ProfileProducerScope {
        &self.producer
    }
    pub fn delivery_secret(&self) -> Result<[u8; 32], Status> {
        self.delivery_secret
            .lock()
            .map_err(|_| Status::unavailable("Profile scope unavailable"))?
            .ok_or_else(|| Status::permission_denied("Profile delivery not published"))
    }
    /// Response-wide disclosure ceiling. This is not a fresh authorization and
    /// not a settlement grant; it only shortens the original deadline.
    pub fn disclosure_ceiling(&self) -> Result<AuthorityRpcDeadline, Status> {
        self.binding.check()?;
        if self.sealed.load(Ordering::SeqCst) {
            return Err(Status::permission_denied("Profile root scope sealed"));
        }
        let original = self.binding.deadline();
        AuthorityRpcDeadline::within_original(
            original.expires_micros(),
            original.stop().into_std(),
            MAX_GUARD_DURATION,
        )
    }
    /// The Concert→Gateway gRPC EOF marker. Actual owner completion happens only
    /// in CompleteResponse after the Gateway's outer HTTP body EOF.
    pub async fn mark_serialized(&self) -> Result<(), Status> {
        if self.serialized.swap(true, Ordering::SeqCst) {
            return Err(Status::permission_denied("Profile response already serialized"));
        }
        Ok(())
    }
    /// Own the entire post-acquisition producer future, including delivery
    /// publication. No detached task may retain producer work outside it.
    pub async fn run_owned<T>(
        &self,
        future: impl Future<Output = Result<T, Status>>,
    ) -> Result<T, Status> {
        if self.owned_work_started.swap(true, Ordering::SeqCst) {
            return Err(Status::permission_denied("Profile producer already started"));
        }
        self.binding.check()?;
        let result = tokio::time::timeout_at(self.binding.deadline().stop(), future)
            .await
            .map_err(|_| Status::deadline_exceeded("Profile producer deadline"))?;
        // The producer future has been destroyed by this point. Any outstanding
        // owner exclusion stays until the separate settlement operation.
        result
    }
    /// Settlement is the only path that completes the scope. It is idempotent
    /// for the exact same delivery secret and refuses a second, different one.
    pub fn complete(&self, secret: &[u8]) -> Result<bool, Status> {
        let expected = self.delivery_secret()?;
        if !constant_time_equal(&expected, secret) {
            return Err(Status::permission_denied(
                "Invalid profile delivery capability",
            ));
        }
        if self.complete.swap(true, Ordering::SeqCst) {
            return Ok(false);
        }
        self.sealed.store(true, Ordering::SeqCst);
        Ok(true)
    }
    pub fn is_complete(&self) -> bool {
        self.complete.load(Ordering::SeqCst)
    }
}


fn constant_time_equal(expected: &[u8; 32], actual: &[u8]) -> bool {
    if actual.len() != 32 {
        return false;
    }
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, b"mcr profile delivery compare v1\0");
    let tag = ring::hmac::sign(&key, expected);
    ring::hmac::verify(&key, actual, tag.as_ref()).is_ok()
}

/// Acquire profile owner exclusion. This is the only constructor of
/// `ProfileOwnerScope`.
#[derive(Clone)]
pub struct ProfileOwnerFence {
    registry: Arc<ProfileScopeRegistry>,
    ledger: crate::profile_product::ledger::ProfileScopeLedger,
}

impl ProfileOwnerFence {
    pub fn new(
        registry: Arc<ProfileScopeRegistry>,
        ledger: crate::profile_product::ledger::ProfileScopeLedger,
    ) -> Self {
        Self { registry, ledger }
    }
    pub async fn acquire(
        &self,
        binding: ProfileGuardBinding,
    ) -> Result<Arc<ProfileOwnerScope>, Status> {
        binding.check()?;
        let producer = self.registry.reserve(&binding)?;
        // Durable registration precedes any owner work so a lost reply still
        // leaves a record the settlement path can reconcile.
        self.ledger.register(&producer, &binding).await?;
        binding.check()?;
        let scope = Arc::new(ProfileOwnerScope {
            binding,
            producer,
            delivery_secret: Mutex::new(None),
            serialized: AtomicBool::new(false),
            complete: AtomicBool::new(false),
            sealed: AtomicBool::new(false),
            owned_work_started: AtomicBool::new(false),
        });
        self.registry.activate(scope.clone())?;
        Ok(scope)
    }
}
