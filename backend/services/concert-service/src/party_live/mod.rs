//! Bounded C04 party list/detail/create producer.
//!
//! This module owns transport input validation and orchestration only. The injected
//! owner fence must acquire current canonical Auth/Friend exclusion and current
//! public Profile content. Local event, admission, participant, pair, party and
//! capacity locks are held on the provided PostgreSQL connection through commit.
//! No permissive authority or fixture implementation is installed here.
mod cursor;
mod product;
mod scope_guard;
mod wire;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{
    commands::{CommandError, FingerprintKeys, ReceiptState},
    community::PartyCommand,
};
use concert_domain::community::party::{Party, PartyAction, PartyError, PartyFields};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use std::{fmt, future::Future, pin::Pin, sync::Arc, time::Duration};
use uuid::Uuid;

use crate::adapters::out::relationship_authority::guarded::OwnerGuardScope;
use crate::adapters::out::repository::receipt::{CommandProducer, PgCommandRepository};
use crate::community::{CommunityAccess, CommunityAuthority, CommunityError};
use crate::product_rpc::proto::ProductRequest;
use concert_application::commands::{Completion, Invalidation, SafeReference, Scope};
use prost::Message;

pub use cursor::{PartyCursorBinding, PartyCursorCodec, PartyPosition, PartyWindow};
pub use product::PartyProductProducer;
pub use scope_guard::ScopePartyLiveGuard;
#[doc(hidden)]
pub use wire::{operation_for_fixture, parse_verified_body_for_fixture};
pub use wire::{PartyLiveAction, PartyLiveReply, PartyLiveRequest, PartyQuery, PARTY_OPERATIONS};

pub const REQUEST_CEILING: Duration = Duration::from_secs(5);
const MAX_SCAN: usize = 1000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartyLiveError {
    InvalidInput,
    Denied,
    ResourceUnavailable,
    DependencyUnavailable,
    VersionConflict,
    InvalidTransition,
    EventReadOnly,
    Internal,
}

impl std::fmt::Display for PartyLiveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.reason())
    }
}

impl std::error::Error for PartyLiveError {}

impl PartyLiveError {
    pub fn status(self) -> u16 {
        match self {
            Self::InvalidInput => 400,
            Self::Denied => 403,
            Self::ResourceUnavailable => 404,
            Self::VersionConflict | Self::InvalidTransition | Self::EventReadOnly => 409,
            Self::DependencyUnavailable => 503,
            Self::Internal => 500,
        }
    }

    pub fn reason(self) -> &'static str {
        match self {
            Self::InvalidInput => "INVALID_INPUT",
            Self::Denied => "PERMISSION_DENIED",
            Self::ResourceUnavailable => "RESOURCE_UNAVAILABLE",
            Self::DependencyUnavailable => "DEPENDENCY_UNAVAILABLE",
            Self::VersionConflict => "VERSION_CONFLICT",
            Self::InvalidTransition => "INVALID_TRANSITION",
            Self::EventReadOnly => "EVENT_READ_ONLY",
            Self::Internal => "INTERNAL_ERROR",
        }
    }
}

impl From<CommandError> for PartyLiveError {
    fn from(value: CommandError) -> Self {
        match value {
            CommandError::InvalidInput => Self::InvalidInput,
            CommandError::Denied => Self::Denied,
            CommandError::Unavailable | CommandError::Capacity => Self::DependencyUnavailable,
            CommandError::Conflict => Self::VersionConflict,
            CommandError::Expired => Self::InvalidTransition,
            CommandError::InvalidState => Self::Internal,
        }
    }
}

impl From<CommunityError> for PartyLiveError {
    fn from(value: CommunityError) -> Self {
        match value {
            CommunityError::Command(value) => value.into(),
            CommunityError::Party(concert_domain::community::party::PartyError::Denied) => {
                Self::Denied
            }
            CommunityError::Party(
                concert_domain::community::party::PartyError::VersionConflict,
            ) => Self::VersionConflict,
            CommunityError::Party(
                concert_domain::community::party::PartyError::InvalidTransition,
            ) => Self::InvalidTransition,
            CommunityError::EventReadOnly => Self::EventReadOnly,
            CommunityError::Party(_) => Self::InvalidInput,
        }
    }
}

/// Exact request and authority binding retained by the owner holder. It is not a
/// grant and intentionally has no Debug or Serialize implementation.
pub struct PartyLiveBinding {
    actor: Uuid,
    event: Uuid,
    session_family: Uuid,
    request_id: Uuid,
    deadline: platform::actor_delegation::authority_deadline::AuthorityRpcDeadline,
    operation: &'static str,
    method: String,
    target: String,
    body_sha256: [u8; 32],
    idempotency_key: Option<Uuid>,
    parent_digest: Option<[u8; 32]>,
}

impl fmt::Debug for PartyLiveBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PartyLiveBinding([REDACTED])")
    }
}

impl PartyLiveBinding {
    pub fn actor(&self) -> Uuid {
        self.actor
    }
    pub fn event(&self) -> Uuid {
        self.event
    }
    pub fn session_family(&self) -> Uuid {
        self.session_family
    }
    pub fn request_id(&self) -> Uuid {
        self.request_id
    }
    pub fn deadline(&self) -> platform::actor_delegation::authority_deadline::AuthorityRpcDeadline {
        self.deadline
    }
    pub fn operation(&self) -> &'static str {
        self.operation
    }
    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn idempotency_key(&self) -> Option<Uuid> {
        self.idempotency_key
    }
    pub fn parent_digest(&self) -> Result<[u8; 32], PartyLiveError> {
        self.parent_digest
            .ok_or(PartyLiveError::DependencyUnavailable)
    }
    pub fn check(&self) -> Result<(), PartyLiveError> {
        let root = format!("/api/concert/events/{}/parties", self.event);
        let (path, query) = self
            .target
            .split_once('?')
            .map_or((self.target.as_str(), None), |(path, query)| {
                (path, Some(query))
            });
        let exact_target = match self.operation {
            "listConcertParties" => path == root && self.method == "GET",
            "getConcertParty" => {
                path.strip_prefix(&(root.clone() + "/"))
                    .is_some_and(|party| canonical_uuid(party).is_some())
                    && query.is_none_or(str::is_empty)
                    && self.method == "GET"
            }
            "createConcertParty" => path == root && query.is_none() && self.method == "POST",
            _ => false,
        };
        let exact_idempotency = match self.operation {
            "createConcertParty" => self.idempotency_key.is_some_and(|key| !key.is_nil()),
            "listConcertParties" | "getConcertParty" => self.idempotency_key.is_none(),
            _ => false,
        };
        if [self.actor, self.event, self.session_family, self.request_id]
            .iter()
            .any(Uuid::is_nil)
            || !exact_target
            || !exact_idempotency
            || self.target.len() > 4096
            || self.target.chars().any(char::is_control)
        {
            return Err(PartyLiveError::InvalidInput);
        }
        self.deadline
            .remaining(REQUEST_CEILING, self.deadline.expires_micros())
            .map_err(|_| PartyLiveError::DependencyUnavailable)?;
        Ok(())
    }

    /// Domain-separated exact binding. The owner holder and future canonical
    /// adapter compare this value before local effects and again before settlement.
    pub fn digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"MCR party-live binding v1\0");
        for id in [self.actor, self.event, self.session_family, self.request_id] {
            hash.update(id.as_bytes());
        }
        hash.update(self.deadline.expires_micros().to_be_bytes());
        for value in [self.method.as_str(), self.target.as_str()] {
            hash.update((value.len() as u64).to_be_bytes());
            hash.update(value.as_bytes());
        }
        hash.update(self.body_sha256);
        if let Some(digest) = self.parent_digest {
            hash.update([1]);
            hash.update(digest);
        } else {
            hash.update([0]);
        }
        match self.idempotency_key {
            Some(key) => {
                hash.update([1]);
                hash.update(key.as_bytes());
            }
            None => hash.update([0]),
        }
        hash.finalize().into()
    }

    /// A production issuer must resolve the sealed Gateway parent before any
    /// canonical holder can be acquired. This is a source seam, not a grant.
    pub fn require_parent_for_holder(&self) -> Result<[u8; 32], PartyLiveError> {
        self.check()?;
        self.parent_digest()
    }

    /// Root supplies the already sealed canonical GuardBinding; this strict
    /// conversion does not authenticate or infer a parent from bare identifiers.
    /// Root also supplies the create key after parsing its exact HTTP request.
    pub fn from_owner_guard(
        guard: &crate::adapters::out::relationship_authority::guarded::GuardBinding,
        operation: &'static str,
        body: &[u8],
        idempotency_key: Option<Uuid>,
    ) -> Result<Self, PartyLiveError> {
        guard
            .check()
            .map_err(|_| PartyLiveError::DependencyUnavailable)?;
        if !PARTY_OPERATIONS.contains(&operation)
            || guard.operation() != operation
            || guard.http_method()
                != if operation == "createConcertParty" {
                    "POST"
                } else {
                    "GET"
                }
            || !guard
                .http_target()
                .starts_with(&format!("/api/concert/events/{}/parties", guard.event()))
            || guard.http_target().len() > 4096
        {
            return Err(PartyLiveError::InvalidInput);
        }
        Ok(Self::construct(
            guard.actor(),
            guard.event(),
            guard.session_family(),
            guard.request_id(),
            guard.deadline(),
            operation,
            guard.http_method(),
            guard.http_target().to_owned(),
            body,
            if operation == "createConcertParty" {
                Some(
                    idempotency_key
                        .filter(|key| !key.is_nil())
                        .ok_or(PartyLiveError::InvalidInput)?,
                )
            } else if idempotency_key.is_some() {
                return Err(PartyLiveError::InvalidInput);
            } else {
                None
            },
            Some(guard.digest()),
        ))
    }
}

/// Canonical owner/profile holder. The real adapter must map this to both owner
/// guards and to the guarded public Profile projection. It owns uncertain-outcome
/// quarantine and must not release on Drop, deadline, cancellation or process loss.
#[async_trait]
pub trait PartyLiveOwnerFence: Send + Sync {
    async fn run(
        &self,
        binding: Arc<PartyLiveBinding>,
        work: PartyLiveOwnerWork,
    ) -> Result<PartyLiveReply, PartyLiveError>;
}

pub type PartyLiveOwnerFuture =
    Pin<Box<dyn Future<Output = Result<PartyLiveReply, PartyLiveError>> + Send>>;
pub type PartyLiveOwnerWork =
    Box<dyn FnOnce(Arc<dyn PartyLiveOwnerGuard>) -> PartyLiveOwnerFuture + Send>;

/// Actual owner guard used only while the root holder remains alive. The guard's
/// methods may perform remote checks and must tie local locks to the provided
/// connection. They cannot construct an admission from the identifiers themselves.
#[async_trait]
pub trait PartyLiveOwnerGuard: Send + Sync {
    fn binding_digest(&self) -> [u8; 32];
    fn authority_revision(&self) -> i64;
    /// Current actor/session/Auth/Friend exclusion plus the local pair decision.
    async fn authorize_in(
        &self,
        connection: &mut PgConnection,
        binding: &PartyLiveBinding,
        subject: Uuid,
    ) -> Result<(), PartyLiveError>;
    /// Current C03 PublicProfile, including the avatar union. The real adapter
    /// retains its Profile child in this owner scope until response settlement.
    async fn public_profile_in(
        &self,
        connection: &mut PgConnection,
        binding: &PartyLiveBinding,
        subject: Uuid,
    ) -> Result<Value, PartyLiveError>;
    /// Register this actual producer transaction before protected SQL effects.
    async fn track_in(
        &self,
        connection: &mut PgConnection,
        binding: &PartyLiveBinding,
    ) -> Result<(), PartyLiveError>;
}

/// Explicit failure-only provider. No production startup may substitute this for a
/// qualified canonical owner holder and call it success.
pub struct UnavailablePartyLiveOwnerFence;

#[async_trait]
impl PartyLiveOwnerFence for UnavailablePartyLiveOwnerFence {
    async fn run(
        &self,
        _: Arc<PartyLiveBinding>,
        _: PartyLiveOwnerWork,
    ) -> Result<PartyLiveReply, PartyLiveError> {
        Err(PartyLiveError::DependencyUnavailable)
    }
}

pub struct PartyLiveService<A, F> {
    pool: PgPool,
    authority: Arc<A>,
    keys: Arc<FingerprintKeys>,
    cursors: PartyCursorCodec,
    owner: Arc<F>,
}

impl<A, F> PartyLiveService<A, F>
where
    A: CommunityAuthority + 'static,
    F: PartyLiveOwnerFence,
{
    pub fn new(
        pool: PgPool,
        authority: Arc<A>,
        keys: Arc<FingerprintKeys>,
        cursors: PartyCursorCodec,
        owner: Arc<F>,
    ) -> Self {
        Self {
            pool,
            authority,
            keys,
            cursors,
            owner,
        }
    }

    /// Root-owned Product dispatch after the sealed parent has been verified and
    /// the canonical owner scope acquired. The root retains the scope through
    /// outer HTTP response completion; this producer neither acquires nor settles
    /// a second fence and returns JSON status/body only.
    ///
    /// Root also publishes the delivery capability before invoking this method,
    /// attaches its private delivery/token fields to the Product response, and
    /// completes that delivery only at outer HTTP body EOF. None of those values
    /// belong in the Party JSON reply or HTTP headers.
    pub async fn dispatch_for_scope(
        &self,
        scope: Arc<OwnerGuardScope>,
        guard: Arc<dyn PartyLiveOwnerGuard>,
        request: ProductRequest,
    ) -> Result<PartyLiveReply, PartyLiveError> {
        scope
            .binding()
            .check()
            .map_err(|_| PartyLiveError::DependencyUnavailable)?;
        if Sha256::digest(request.encode_to_vec()).as_slice()
            != scope.binding().owner_binding().parent_body_digest
        {
            return Err(PartyLiveError::InvalidInput);
        }
        let parsed = PartyLiveRequest::from_owner_scope(scope.binding(), &request)?;
        if guard.binding_digest() != parsed.binding.digest() {
            return Err(PartyLiveError::DependencyUnavailable);
        }
        let deadline = parsed.binding.deadline();
        let binding = Arc::new(parsed.binding);
        let result = tokio::time::timeout_at(
            deadline.stop(),
            Self::dispatch_guarded(
                self.pool.clone(),
                self.authority.clone(),
                self.keys.clone(),
                self.cursors.clone(),
                binding,
                parsed.action,
                guard,
            ),
        )
        .await
        .map_err(|_| PartyLiveError::DependencyUnavailable)?;
        // Keep the canonical owner-held scope alive through producer completion.
        // Only the root's outer HTTP EOF settlement may finish this scope.
        let _scope = scope;
        result
    }

    /// Run the exact guarded producer work without a Product transport context.
    /// This is only for the owned native harness: authority acquisition and the
    /// guard lifetime remain injected, and no production route can call it.
    #[doc(hidden)]
    pub async fn dispatch_guarded_for_test(
        &self,
        binding: Arc<PartyLiveBinding>,
        action: PartyLiveAction,
    ) -> Result<PartyLiveReply, PartyLiveError> {
        binding.check()?;
        let deadline = binding.deadline();
        let work_binding = binding.clone();
        let pool = self.pool.clone();
        let authority = self.authority.clone();
        let keys = self.keys.clone();
        let cursors = self.cursors.clone();
        let work: PartyLiveOwnerWork = Box::new(move |guard| {
            Box::pin(async move {
                Self::dispatch_guarded(pool, authority, keys, cursors, work_binding, action, guard)
                    .await
            })
        });
        tokio::time::timeout_at(deadline.stop(), self.owner.run(binding, work))
            .await
            .map_err(|_| PartyLiveError::DependencyUnavailable)?
    }

    async fn dispatch_guarded(
        pool: PgPool,
        authority: Arc<A>,
        keys: Arc<FingerprintKeys>,
        cursors: PartyCursorCodec,
        binding: Arc<PartyLiveBinding>,
        action: PartyLiveAction,
        guard: Arc<dyn PartyLiveOwnerGuard>,
    ) -> Result<PartyLiveReply, PartyLiveError> {
        binding.check()?;
        if guard.binding_digest() != binding.digest()
            || !(1..=9_007_199_254_740_991).contains(&guard.authority_revision())
        {
            return Err(PartyLiveError::DependencyUnavailable);
        }
        match action {
            PartyLiveAction::List(query) => {
                Self::list_in(pool, cursors, binding, query, guard).await
            }
            PartyLiveAction::Detail(party) => Self::detail_in(pool, binding, party, guard).await,
            PartyLiveAction::Create(fields) => {
                Self::create(pool, authority, keys, binding, fields, guard).await
            }
        }
    }

    async fn list_in(
        pool: PgPool,
        cursors: PartyCursorCodec,
        binding: Arc<PartyLiveBinding>,
        query: PartyQuery,
        guard: Arc<dyn PartyLiveOwnerGuard>,
    ) -> Result<PartyLiveReply, PartyLiveError> {
        let cursor_binding = query.cursor_binding(&binding)?;
        let mut tx = pool.begin().await.map_err(db)?;
        local_actor_in(&mut tx, &binding).await?;
        local_pair_in(&mut tx, binding.event(), binding.actor(), binding.actor()).await?;
        guard.track_in(&mut tx, &binding).await?;
        let upper = if query.cursor.is_some() {
            // Opened below after the immutable initial upper is available.
            None
        } else {
            Some(
                sqlx::query_scalar::<_, DateTime<Utc>>("select clock_timestamp()")
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(db)?,
            )
        };
        let window = match query.cursor.as_deref() {
            Some(token) => {
                let at = sqlx::query_scalar::<_, DateTime<Utc>>("select clock_timestamp()")
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(db)?;
                cursors.open(&cursor_binding, token, at)?
            }
            None => PartyWindow {
                upper: upper.expect("initial window"),
                after: None,
            },
        };
        let mut after = window.after;
        let mut items = Vec::with_capacity(usize::from(cursor_binding.limit));
        let mut scanned = 0usize;
        let mut more = false;
        while items.len() < usize::from(cursor_binding.limit) && scanned < MAX_SCAN {
            let remaining_visible = usize::from(cursor_binding.limit) - items.len();
            let batch = remaining_visible
                .saturating_mul(2)
                .clamp(16, 100)
                .min(MAX_SCAN - scanned);
            let rows = wire::scan(
                &mut tx,
                binding.event(),
                &cursor_binding,
                window.upper,
                after,
                i64::try_from(batch).map_err(|_| PartyLiveError::Internal)?,
            )
            .await?;
            if rows.is_empty() {
                more = false;
                break;
            }
            more = rows.len() == batch;
            for row in rows {
                scanned += 1;
                after = Some(row.position);
                match project_party(&mut tx, &binding, row.party, &guard).await {
                    Ok(value) => items.push(value),
                    Err(PartyLiveError::Denied | PartyLiveError::ResourceUnavailable) => continue,
                    Err(error) => return Err(error),
                }
                if items.len() == usize::from(cursor_binding.limit) {
                    break;
                }
            }
            if !more {
                break;
            }
        }
        // A full batch may still end exactly at the collection tail. Only expose a
        // continuation when this bounded scan actually observed additional rows or
        // reached the scan ceiling.
        if items.len() == usize::from(cursor_binding.limit) || scanned >= MAX_SCAN {
            more = more || scanned >= MAX_SCAN;
        }
        let next = if more {
            after
                .map(|position| {
                    cursors.seal(
                        &cursor_binding,
                        PartyWindow {
                            upper: window.upper,
                            after: Some(position),
                        },
                        Utc::now(),
                    )
                })
                .transpose()?
        } else {
            None
        };
        let reply = wire::list(items, next)?;
        guard_ready(&guard, &binding)?;
        tx.commit().await.map_err(db)?;
        Ok(reply)
    }

    async fn detail_in(
        pool: PgPool,
        binding: Arc<PartyLiveBinding>,
        party: Uuid,
        guard: Arc<dyn PartyLiveOwnerGuard>,
    ) -> Result<PartyLiveReply, PartyLiveError> {
        let mut tx = pool.begin().await.map_err(db)?;
        local_actor_in(&mut tx, &binding).await?;
        local_pair_in(&mut tx, binding.event(), binding.actor(), binding.actor()).await?;
        guard.track_in(&mut tx, &binding).await?;
        let value = wire::load(&mut tx, binding.event(), party)
            .await?
            .ok_or(PartyLiveError::ResourceUnavailable)?;
        let value = project_party(&mut tx, &binding, value, &guard).await?;
        let reply = wire::party(value)?;
        guard_ready(&guard, &binding)?;
        tx.commit().await.map_err(db)?;
        Ok(reply)
    }

    async fn create(
        pool: PgPool,
        authority: Arc<A>,
        keys: Arc<FingerprintKeys>,
        binding: Arc<PartyLiveBinding>,
        fields: PartyFields,
        guard: Arc<dyn PartyLiveOwnerGuard>,
    ) -> Result<PartyLiveReply, PartyLiveError> {
        let key = binding
            .idempotency_key()
            .ok_or(PartyLiveError::InvalidInput)?;
        let command = PartyCommand {
            actor: binding.actor(),
            event: binding.event(),
            key,
            party: Uuid::nil(),
            action: PartyAction::Create(fields),
        };
        let (receipt, domain_error) = execute_create_command(
            pool.clone(),
            authority.clone(),
            keys.clone(),
            binding.clone(),
            command,
            guard.clone(),
        )
        .await;
        if let Some(error) = domain_error {
            return Err(error);
        }
        let receipt = receipt.map_err(PartyLiveError::from)?;
        if receipt.state != ReceiptState::Succeeded {
            return Err(match receipt.state {
                ReceiptState::Failed => PartyLiveError::InvalidTransition,
                _ => PartyLiveError::DependencyUnavailable,
            });
        }
        let reference = receipt.reference.ok_or(PartyLiveError::Internal)?;
        let mut tx = pool.begin().await.map_err(db)?;
        local_actor_in(&mut tx, &binding).await?;
        local_pair_in(&mut tx, binding.event(), binding.actor(), binding.actor()).await?;
        guard.track_in(&mut tx, &binding).await?;
        let party = wire::load(&mut tx, binding.event(), reference.resource)
            .await?
            .ok_or(PartyLiveError::DependencyUnavailable)?;
        let value = project_party(&mut tx, &binding, party, &guard).await?;
        let reply = wire::party(value)?;
        guard_ready(&guard, &binding)?;
        tx.commit().await.map_err(db)?;
        Ok(reply)
    }
}

async fn execute_create_command<A>(
    pool: PgPool,
    authority: Arc<A>,
    keys: Arc<FingerprintKeys>,
    binding: Arc<PartyLiveBinding>,
    command: PartyCommand,
    guard: Arc<dyn PartyLiveOwnerGuard>,
) -> (
    Result<concert_application::commands::SafeReceipt, CommandError>,
    Option<PartyLiveError>,
)
where
    A: CommunityAuthority,
{
    let producer: PartyCreateProducer<'_, A, dyn PartyLiveOwnerGuard> = PartyCreateProducer {
        command: &command,
        authority: &authority,
        binding: &binding,
        guard: guard.as_ref(),
        error: std::sync::Mutex::new(None),
    };
    let result = match command.request() {
        Ok(request) => {
            PgCommandRepository::new(pool)
                .execute(&request, command.target(), &keys, &producer)
                .await
        }
        Err(error) => Err(error),
    };
    let domain_error = producer.error.lock().ok().and_then(|mut slot| slot.take());
    (result, domain_error)
}

async fn project_party(
    connection: &mut PgConnection,
    binding: &PartyLiveBinding,
    party: Party,
    guard: &Arc<dyn PartyLiveOwnerGuard>,
) -> Result<Value, PartyLiveError> {
    local_subject_in(connection, binding.event(), party.owner).await?;
    local_pair_in(connection, binding.event(), binding.actor(), party.owner).await?;
    guard.authorize_in(connection, binding, party.owner).await?;
    let owner = guard
        .public_profile_in(connection, binding, party.owner)
        .await?;
    wire::public_party(&party, owner)
}

struct PartyCreateProducer<'a, A, G: ?Sized> {
    command: &'a PartyCommand,
    authority: &'a Arc<A>,
    binding: &'a PartyLiveBinding,
    guard: &'a G,
    error: std::sync::Mutex<Option<PartyLiveError>>,
}

impl<A, G: PartyLiveOwnerGuard + ?Sized> PartyCreateProducer<'_, A, G> {
    fn domain(&self, error: PartyError) -> CommandError {
        if let Ok(mut slot) = self.error.lock() {
            *slot = Some(match error {
                PartyError::Denied => PartyLiveError::Denied,
                PartyError::VersionConflict => PartyLiveError::VersionConflict,
                PartyError::InvalidTransition => PartyLiveError::InvalidTransition,
                PartyError::Full | PartyError::DuplicateApplication => {
                    PartyLiveError::InvalidTransition
                }
                PartyError::Unavailable => PartyLiveError::DependencyUnavailable,
                PartyError::InvalidInput => PartyLiveError::InvalidInput,
            });
        }
        CommandError::InvalidState
    }
}

#[async_trait]
impl<A, G> CommandProducer for PartyCreateProducer<'_, A, G>
where
    A: CommunityAuthority,
    G: PartyLiveOwnerGuard + ?Sized,
{
    async fn authorize(
        &self,
        connection: &mut PgConnection,
        scope: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        if scope != self.command.scope()? || target != self.command.target() {
            return Err(CommandError::Denied);
        }
        self.binding.check().map_err(|_| CommandError::Denied)?;
        local_actor_in(connection, self.binding)
            .await
            .map_err(command_error)?;
        local_pair_in(
            connection,
            self.binding.event(),
            self.binding.actor(),
            self.binding.actor(),
        )
        .await
        .map_err(command_error)?;
        self.guard
            .authorize_in(connection, self.binding, self.binding.actor())
            .await
            .map_err(command_error)?;
        self.authority
            .authorize_in(
                connection,
                self.binding.actor(),
                self.binding.event(),
                self.binding.actor(),
                CommunityAccess::WriteParties,
            )
            .await?;
        let PartyAction::Create(fields) = &self.command.action else {
            return Err(CommandError::InvalidInput);
        };
        self.authority
            .place_in(connection, self.binding.event(), fields.public_place_id)
            .await?;
        local_phase_write_in(connection, self.binding.event()).await?;
        self.guard
            .track_in(connection, self.binding)
            .await
            .map_err(command_error)
    }

    async fn apply(
        &self,
        connection: &mut PgConnection,
        scope: Scope,
        target: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        if scope != self.command.scope()? || target != self.command.target() {
            return Err(CommandError::Denied);
        }
        local_phase_write_in(connection, self.binding.event()).await?;
        local_pair_in(
            connection,
            self.binding.event(),
            self.binding.actor(),
            self.binding.actor(),
        )
        .await
        .map_err(command_error)?;
        self.authority
            .authorize_in(
                connection,
                self.binding.actor(),
                self.binding.event(),
                self.binding.actor(),
                CommunityAccess::WriteParties,
            )
            .await?;
        self.guard
            .authorize_in(connection, self.binding, self.binding.actor())
            .await
            .map_err(command_error)?;
        let PartyAction::Create(fields) = &self.command.action else {
            return Err(CommandError::InvalidInput);
        };
        sqlx::query("savepoint party_mutation")
            .execute(&mut *connection)
            .await
            .map_err(|_| CommandError::Unavailable)?;
        let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *connection)
            .await
            .map_err(|_| CommandError::Unavailable)?;
        let party = match Party::create(target, scope.event, scope.actor, fields.clone(), now) {
            Ok(party) => party,
            Err(error) => {
                sqlx::query("rollback to savepoint party_mutation")
                    .execute(&mut *connection)
                    .await
                    .map_err(|_| CommandError::Unavailable)?;
                return Err(self.domain(error));
            }
        };
        let aggregate = serde_json::to_value(&party).map_err(|_| CommandError::InvalidState)?;
        sqlx::query(
            "insert into public.concert_community_parties(event_id,party_id,owner_id,revision,state,member_count,capacity,meeting_revision,audience_revision,aggregate) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
        )
        .bind(party.event)
        .bind(party.id)
        .bind(party.owner)
        .bind(party.revision)
        .bind("OPEN")
        .bind(party.member_count())
        .bind(party.fields.capacity)
        .bind(party.meeting_revision)
        .bind(party.audience_revision)
        .bind(aggregate)
        .execute(&mut *connection)
        .await
        .map_err(|_| CommandError::Unavailable)?;
        for member in &party.members {
            sqlx::query(
                "insert into public.concert_community_members(event_id,party_id,user_id,is_owner,state,revision) values($1,$2,$3,$4,$5,$6)",
            )
            .bind(party.event)
            .bind(party.id)
            .bind(member.user)
            .bind(member.owner)
            .bind(match member.state {
                concert_domain::community::party::MemberState::Active => "ACTIVE",
                concert_domain::community::party::MemberState::Removed => "REMOVED",
                concert_domain::community::party::MemberState::Left => "LEFT",
            })
            .bind(member.revision)
            .execute(&mut *connection)
            .await
            .map_err(|_| CommandError::Unavailable)?;
        }
        let reference = SafeReference {
            resource: target,
            revision: 1,
        }
        .validate()?;
        Ok((
            Completion::Succeeded {
                reference,
                status: 200,
            },
            Some(Invalidation { reference }),
        ))
    }
}

fn command_error(error: PartyLiveError) -> CommandError {
    match error {
        PartyLiveError::InvalidInput => CommandError::InvalidInput,
        PartyLiveError::Denied | PartyLiveError::ResourceUnavailable => CommandError::Denied,
        PartyLiveError::DependencyUnavailable => CommandError::Unavailable,
        PartyLiveError::VersionConflict => CommandError::Conflict,
        PartyLiveError::InvalidTransition | PartyLiveError::EventReadOnly => {
            CommandError::InvalidState
        }
        PartyLiveError::Internal => CommandError::InvalidState,
    }
}

async fn local_phase_write_in(
    connection: &mut PgConnection,
    event: Uuid,
) -> Result<(), CommandError> {
    let phase: Option<String> =
        sqlx::query_scalar("select phase from public.concert_events where event_id=$1 for share")
            .bind(event)
            .fetch_optional(&mut *connection)
            .await
            .map_err(|_| CommandError::Unavailable)?;
    if matches!(phase.as_deref(), Some("OPEN" | "LIVE")) {
        Ok(())
    } else {
        Err(CommandError::Denied)
    }
}

async fn local_actor_in(
    connection: &mut PgConnection,
    binding: &PartyLiveBinding,
) -> Result<(), PartyLiveError> {
    let event: Option<(String, i64)> = sqlx::query_as(
        "select phase,revision from public.concert_events where event_id=$1 for share",
    )
    .bind(binding.event())
    .fetch_optional(&mut *connection)
    .await
    .map_err(db)?;
    let Some((phase, _)) = event else {
        return Err(PartyLiveError::ResourceUnavailable);
    };
    if !matches!(phase.as_str(), "OPEN" | "LIVE" | "READ_ONLY") {
        return Err(PartyLiveError::Denied);
    }
    local_subject_in(connection, binding.event(), binding.actor()).await
}

async fn local_subject_in(
    connection: &mut PgConnection,
    event: Uuid,
    user: Uuid,
) -> Result<(), PartyLiveError> {
    if user.is_nil() {
        return Err(PartyLiveError::InvalidInput);
    }
    let admission: Option<String> = sqlx::query_scalar(
        "select state from public.concert_event_admissions where event_id=$1 and user_id=$2 for share",
    )
    .bind(event)
    .bind(user)
    .fetch_optional(&mut *connection)
    .await
    .map_err(db)?;
    if admission.as_deref() != Some("ACTIVE") {
        return Err(PartyLiveError::Denied);
    }
    let participant: Option<Uuid> = sqlx::query_scalar(
        "select user_id from public.concert_participants where event_id=$1 and user_id=$2 for share",
    )
    .bind(event)
    .bind(user)
    .fetch_optional(&mut *connection)
    .await
    .map_err(db)?;
    if participant != Some(user) {
        return Err(PartyLiveError::Denied);
    }
    Ok(())
}

async fn local_pair_in(
    connection: &mut PgConnection,
    event: Uuid,
    actor: Uuid,
    subject: Uuid,
) -> Result<(), PartyLiveError> {
    if actor.is_nil() || subject.is_nil() {
        return Err(PartyLiveError::InvalidInput);
    }
    let (low, high) = if actor <= subject {
        (actor, subject)
    } else {
        (subject, actor)
    };
    // The owning policy reserves a missing pair before taking SHARE. This closes
    // concurrent first-block insertion without treating absence as an allow.
    sqlx::query("insert into public.concert_relationship_pairs(event_id,low_user,high_user) values($1,$2,$3) on conflict do nothing")
        .bind(event)
        .bind(low)
        .bind(high)
        .execute(&mut *connection)
        .await
        .map_err(db)?;
    let blocked: Option<(bool, bool)> = sqlx::query_as(
        "select low_blocks_high,high_blocks_low from public.concert_relationship_pairs where event_id=$1 and low_user=$2 and high_user=$3 for share",
    )
    .bind(event)
    .bind(low)
    .bind(high)
    .fetch_optional(&mut *connection)
    .await
    .map_err(db)?;
    match blocked {
        Some((false, false)) => Ok(()),
        Some(_) => Err(PartyLiveError::Denied),
        None => Err(PartyLiveError::DependencyUnavailable),
    }
}

fn guard_ready(
    guard: &Arc<dyn PartyLiveOwnerGuard>,
    binding: &PartyLiveBinding,
) -> Result<(), PartyLiveError> {
    binding.check()?;
    if guard.binding_digest() != binding.digest() {
        return Err(PartyLiveError::Denied);
    }
    Ok(())
}

fn db(_: sqlx::Error) -> PartyLiveError {
    PartyLiveError::DependencyUnavailable
}

fn canonical_uuid(value: &str) -> Option<Uuid> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
}

impl PartyLiveBinding {
    #[allow(clippy::too_many_arguments)]
    #[doc(hidden)]
    pub fn test_binding(
        actor: Uuid,
        event: Uuid,
        session_family: Uuid,
        request_id: Uuid,
        deadline: platform::actor_delegation::authority_deadline::AuthorityRpcDeadline,
        operation: &'static str,
        method: &str,
        target: String,
        body: &[u8],
        idempotency_key: Option<Uuid>,
        parent_digest: Option<[u8; 32]>,
    ) -> Self {
        Self {
            actor,
            event,
            session_family,
            request_id,
            deadline,
            operation,
            method: method.into(),
            target,
            body_sha256: Sha256::digest(body).into(),
            idempotency_key,
            parent_digest,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_checked_fields(
        actor: Uuid,
        event: Uuid,
        session_family: Uuid,
        request_id: Uuid,
        deadline: platform::actor_delegation::authority_deadline::AuthorityRpcDeadline,
        operation: &'static str,
        method: &str,
        target: String,
        body: &[u8],
        idempotency_key: Option<Uuid>,
        parent_digest: Option<[u8; 32]>,
    ) -> Self {
        Self::construct(
            actor,
            event,
            session_family,
            request_id,
            deadline,
            operation,
            method,
            target,
            body,
            idempotency_key,
            parent_digest,
        )
    }

    #[doc(hidden)]
    pub fn test_parent(mut self, parent: [u8; 32]) -> Self {
        self.parent_digest = Some(parent);
        self
    }

    #[allow(clippy::too_many_arguments)]
    fn construct(
        actor: Uuid,
        event: Uuid,
        session_family: Uuid,
        request_id: Uuid,
        deadline: platform::actor_delegation::authority_deadline::AuthorityRpcDeadline,
        operation: &'static str,
        method: &str,
        target: String,
        body: &[u8],
        idempotency_key: Option<Uuid>,
        parent_digest: Option<[u8; 32]>,
    ) -> Self {
        Self {
            actor,
            event,
            session_family,
            request_id,
            deadline,
            operation,
            method: method.into(),
            target,
            body_sha256: Sha256::digest(body).into(),
            idempotency_key,
            parent_digest,
        }
    }
}
