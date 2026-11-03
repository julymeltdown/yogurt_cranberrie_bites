//! Request-bound uploader-only media runtime. No public/link authority is implied.
//! Production composition must supply a qualified canonical Auth owner fence.
pub mod context;
pub mod settlement;
mod wire;
use crate::adapters::out::media::{MediaAuthority, PgMedia, PrivateImage};
use async_trait::async_trait;
use concert_application::{
    commands::{CommandError, FingerprintKeys},
    media::*,
};
pub use context::{capture_owner_context_in, OwnerMediaContextMode};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};
use uuid::Uuid;
pub use wire::{OwnerMediaReply, OwnerMediaRequest};

pub const REQUEST_CEILING: Duration = Duration::from_secs(10);
/// Inputs copied ONLY from verified product delegation, never browser identity fields.
#[derive(Clone, Copy)]
pub struct OwnerMediaIdentity {
    pub actor: Uuid,
    pub event: Uuid,
    pub session_family: Uuid,
    pub request_id: Uuid,
    pub deadline: AuthorityRpcDeadline,
}
impl OwnerMediaIdentity {
    /// Shape and original deadline only. This does not authenticate or grant access.
    pub fn check_current(self) -> Result<(), CommandError> {
        if [self.actor, self.event, self.session_family, self.request_id]
            .iter()
            .any(Uuid::is_nil)
        {
            return Err(CommandError::Denied);
        }
        self.deadline
            .remaining(REQUEST_CEILING, self.deadline.expires_micros())
            .map_err(|_| CommandError::Unavailable)?;
        Ok(())
    }
}
/// Exact request scope retained by the owner guard. No credentials or image data.
/// No Debug: this binding is internal authority, not diagnostic/public data.
pub struct OwnerMediaBinding {
    pub identity: OwnerMediaIdentity,
    pub operation: &'static str,
    pub method: &'static str,
    pub target: String,
    pub content_type: String,
    pub key: Option<Uuid>,
    pub body_sha256: [u8; 32],
}
impl OwnerMediaBinding {
    pub fn check_current(&self) -> Result<(), CommandError> {
        self.identity.check_current()
    }
    /// Domain-separated, length-prefixed exact binding; not a transferable grant.
    pub fn digest(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b"MCR-owner-media-binding-v1");
        for id in [
            self.identity.actor,
            self.identity.event,
            self.identity.session_family,
            self.identity.request_id,
        ] {
            h.update(id.as_bytes());
        }
        h.update(self.identity.deadline.expires_micros().to_be_bytes());
        for s in [
            self.operation,
            self.method,
            self.target.as_str(),
            self.content_type.as_str(),
        ] {
            h.update((s.len() as u64).to_be_bytes());
            h.update(s.as_bytes());
        }
        h.update([u8::from(self.key.is_some())]);
        if let Some(key) = self.key {
            h.update(key.as_bytes());
        }
        h.update(self.body_sha256);
        h.finalize().into()
    }
}

/// A production implementation must order account deletion/deactivation, session
/// revocation/expiry and eligibility changes against the entire Concert transaction.
/// Mere signed observations, caches, invalidation hints or expiring unfenced leases
/// DO NOT implement this port. No success implementation/default is supplied here.
#[async_trait]
pub trait OwnerMediaFence: Send + Sync {
    /// The scope owns cancellation, durable quarantine and settlement. A dropped
    /// work future or uncertain SQL outcome must NEVER release its owner exclusion.
    async fn run(
        &self,
        binding: Arc<OwnerMediaBinding>,
        work: OwnerMediaWork,
    ) -> Result<OwnerMediaReply, CommandError>;
}
pub type OwnerMediaFuture =
    Pin<Box<dyn Future<Output = Result<OwnerMediaReply, CommandError>> + Send>>;
pub type OwnerMediaWork = Box<dyn FnOnce(Arc<dyn OwnerMediaGuard>) -> OwnerMediaFuture + Send>;

/// Held until producer/read/recovery SQL finishes (including commit/rollback).
/// The guard must stay effective on owner/network failure: uncertain remote guard
/// release must never permit a conflicting revoke to overtake a possible commit.
#[async_trait]
pub trait OwnerMediaGuard: Send + Sync {
    fn binding_digest(&self) -> [u8; 32];
    /// Stable owner security revision, not a newly minted observation counter.
    fn authority_revision(&self) -> i64;
    /// Before returning success, durably register the actual connection transaction
    /// identity with the canonical scope (see settlement::MediaTransactionIdentity).
    /// Registration/quarantine must survive cancellation; never release on absence.
    async fn authorize_in(
        &self,
        connection: &mut PgConnection,
        binding: &OwnerMediaBinding,
    ) -> Result<(), CommandError>;
}

pub struct OwnerMediaRuntime {
    pool: PgPool,
    sanitizer: Arc<dyn ImageSanitizer>,
    keys: Arc<FingerprintKeys>,
    fence: Arc<dyn OwnerMediaFence>,
}
impl OwnerMediaRuntime {
    /// Strict constructor: production cannot obtain an enabled runtime without
    /// explicitly selecting its qualified owner fence. Runtime itself is not readiness.
    pub fn new(
        pool: PgPool,
        sanitizer: Arc<dyn ImageSanitizer>,
        keys: Arc<FingerprintKeys>,
        fence: Arc<dyn OwnerMediaFence>,
    ) -> Self {
        Self {
            pool,
            sanitizer,
            keys,
            fence,
        }
    }
    pub async fn dispatch(
        &self,
        request: OwnerMediaRequest,
    ) -> Result<OwnerMediaReply, CommandError> {
        request.binding.check_current()?;
        let deadline = request.binding.identity.deadline;
        let binding = Arc::new(request.binding);
        let pool = self.pool.clone();
        let sanitizer = self.sanitizer.clone();
        let keys = self.keys.clone();
        let work_binding = binding.clone();
        let work: OwnerMediaWork = Box::new(move |guard| {
            Box::pin(Self::dispatch_guarded(
                pool,
                sanitizer,
                keys,
                work_binding,
                request.action,
                guard,
            ))
        });
        let result = tokio::time::timeout_at(deadline.stop(), self.fence.run(binding, work))
            .await
            .map_err(|_| CommandError::Unavailable)?;
        // A timeout is uncertain execution, never proof of rollback. Recovery uses
        // the original actor/event/operation/key, never automatic new-key retries.
        deadline
            .remaining(REQUEST_CEILING, deadline.expires_micros())
            .map_err(|_| CommandError::Unavailable)?;
        result
    }
    async fn dispatch_guarded(
        pool: PgPool,
        sanitizer: Arc<dyn ImageSanitizer>,
        keys: Arc<FingerprintKeys>,
        binding: Arc<OwnerMediaBinding>,
        action: wire::Action,
        guard: Arc<dyn OwnerMediaGuard>,
    ) -> Result<OwnerMediaReply, CommandError> {
        binding.check_current()?;
        if guard.binding_digest() != binding.digest()
            || !(1..=9007199254740991).contains(&guard.authority_revision())
        {
            return Err(CommandError::Unavailable);
        }
        let authority = Arc::new(BoundOwnerAuthority {
            binding,
            guard,
            purpose: action.purpose(),
            access: action.access(),
            recovery: matches!(action, wire::Action::Recover { .. }),
            read_after_write: matches!(action, wire::Action::Finalize { .. }),
        });
        let mut tx = pool.begin().await.map_err(|_| CommandError::Unavailable)?;
        let context = authority.context_in(&mut tx).await?;
        authority
            .guard
            .authorize_in(&mut tx, &authority.binding)
            .await?;
        authority.binding.check_current()?;
        tx.commit().await.map_err(|_| CommandError::Unavailable)?;
        let producer = PgMedia::new(pool, authority.clone(), sanitizer, keys);
        let key = authority.binding.key;
        use wire::Action;
        let reply = match action {
            Action::Initialize(input) => {
                let key = key.ok_or(CommandError::InvalidInput)?;
                let receipt = producer.initialize(context, key, input).await?;
                wire::successful(&receipt)?;
                let id = PgMedia::initialized_id(context, key);
                let (revision, expires) = producer.intent(context, id).await?;
                wire::intent(context.event, id, revision, expires)
            }
            Action::Put { id, mime, bytes } => {
                let receipt = producer
                    .put(
                        context,
                        key.ok_or(CommandError::InvalidInput)?,
                        id,
                        mime,
                        bytes,
                    )
                    .await?;
                let reference = wire::successful(&receipt)?;
                wire::uploaded(id, reference.revision)
            }
            Action::Finalize { id, expected } => {
                let receipt = producer
                    .finalize(
                        context,
                        key.ok_or(CommandError::InvalidInput)?,
                        id,
                        expected,
                    )
                    .await?;
                wire::successful(&receipt)?;
                wire::metadata(producer.metadata(context, id).await?)
            }
            Action::Metadata(id) => wire::metadata(producer.metadata(context, id).await?),
            Action::Content(id) => OwnerMediaReply::Image(producer.read(context, id).await?),
            Action::Recover { operation, key } => {
                let receipt = producer.recover_owned(context, key, operation).await?;
                wire::receipt(receipt)
            }
        };
        authority.binding.check_current()?;
        // authority (and its owner guard) remains alive across every awaited commit.
        Ok(reply)
    }
}
struct BoundOwnerAuthority {
    binding: Arc<OwnerMediaBinding>,
    guard: Arc<dyn OwnerMediaGuard>,
    purpose: Option<Purpose>,
    access: wire::AccessKind,
    recovery: bool,
    read_after_write: bool,
}
impl BoundOwnerAuthority {
    async fn context_in(&self, c: &mut PgConnection) -> Result<Context, CommandError> {
        let mode = if self.access == wire::AccessKind::Read || self.recovery {
            OwnerMediaContextMode::OwnerReadOrRecovery
        } else {
            OwnerMediaContextMode::Mutation
        };
        capture_owner_context_in(
            c,
            self.binding.identity,
            self.guard.authority_revision(),
            mode,
        )
        .await
    }
}
#[async_trait]
impl MediaAuthority for BoundOwnerAuthority {
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        context: Context,
        access: Access,
        owner: Uuid,
        target: Option<Target>,
    ) -> Result<(), CommandError> {
        if owner != self.binding.identity.actor || target.is_some() {
            return Err(CommandError::Denied);
        }
        let (kind, purpose) = match access {
            Access::Initialize(p) => (wire::AccessKind::Initialize, p),
            Access::Write(p) => (wire::AccessKind::Write, p),
            Access::Read(p) => (wire::AccessKind::Read, p),
            Access::Link(_) | Access::Delete(_) => return Err(CommandError::Denied),
        };
        if (kind != self.access && !(self.read_after_write && kind == wire::AccessKind::Read))
            || self.purpose.is_some_and(|p| p != purpose)
        {
            return Err(CommandError::Denied);
        }
        if self.context_in(c).await? != context {
            return Err(CommandError::Denied);
        }
        self.guard.authorize_in(c, &self.binding).await?;
        self.binding.check_current()
    }
}
