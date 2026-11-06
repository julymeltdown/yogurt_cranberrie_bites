//! Bounded C04 party read and command producer.
//!
//! This module owns transport input validation and orchestration only. The injected
//! owner fence must acquire current canonical Auth/Friend exclusion and current
//! public Profile content. Local event, admission, participant, pair, party and
//! capacity locks are held on the provided PostgreSQL connection through commit.
//! A Party mutation locks event admission, then the Party root and its child
//! revision, and finally the current subject pair before recording a BE15
//! receipt. Approval capacity is checked by the same locked root transaction.
//! No permissive authority or fixture implementation is installed here.
pub mod account_deletion;
mod cursor;
mod product;
pub mod resources;
mod scope_guard;
pub mod settlement;
mod wire;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{
    commands::{CommandError, FingerprintKeys, ReceiptState},
    community::PartyCommand,
};
use concert_domain::community::party::{Party, PartyAction, PartyError, PartyFields, PartyState};
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

pub use cursor::{PartyCursorBinding, PartyCursorCodec, PartyPosition, PartyWindow};
pub use product::PartyProductProducer;
pub use product::ScopedPartyProductProducer;
pub use scope_guard::ScopePartyLiveGuard;
pub use settlement::PartyNativeResolver;
#[doc(hidden)]
pub use wire::{operation_for_fixture, parse_verified_body_for_fixture};
pub use wire::{PartyLiveAction, PartyLiveReply, PartyLiveRequest, PartyQuery, PARTY_OPERATIONS};

/// The eight operations implemented by the Party producer. Other contract
/// Party operations are classified by the Product router and deliberately
/// remain unavailable until a producer exists.
pub fn handles_operation(operation: &str) -> bool {
    PARTY_OPERATIONS.contains(&operation)
}

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
    PartyFull,
    DuplicateApplication,
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
            Self::VersionConflict
            | Self::InvalidTransition
            | Self::EventReadOnly
            | Self::PartyFull
            | Self::DuplicateApplication => 409,
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
            Self::PartyFull => "PARTY_FULL",
            Self::DuplicateApplication => "DUPLICATE_APPLICATION",
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
        let application_root = format!("/api/concert/events/{}/party-applications", self.event);
        let (path, query) = self
            .target
            .split_once('?')
            .map_or((self.target.as_str(), None), |(path, query)| {
                (path, Some(query))
            });
        let party_tail = |value: &str, tail: &str| {
            value
                .strip_prefix(&(root.clone() + "/"))
                .and_then(|suffix| suffix.strip_suffix(&format!("/{tail}")))
                .and_then(canonical_uuid)
        };
        let application_id = |value: &str, tail: &str| {
            value
                .strip_prefix(&(application_root.clone() + "/"))
                .and_then(|suffix| {
                    if tail.is_empty() {
                        Some(suffix)
                    } else {
                        suffix.strip_suffix(&format!("/{tail}"))
                    }
                })
                .and_then(canonical_uuid)
        };
        let member_target = path
            .strip_prefix(&(root.clone() + "/"))
            .and_then(|suffix| {
                let (party, user) = suffix.split_once("/members/")?;
                Some((canonical_uuid(party)?, canonical_uuid(user)?))
            })
            .is_some();
        let exact_target = match self.operation {
            "listConcertParties" => path == root && self.method == "GET",
            "getConcertParty" => {
                path.strip_prefix(&(root.clone() + "/"))
                    .is_some_and(|party| canonical_uuid(party).is_some())
                    && query.is_none_or(str::is_empty)
                    && self.method == "GET"
            }
            "createConcertParty" => path == root && query.is_none() && self.method == "POST",
            "applyConcertParty" => {
                party_tail(path, "applications").is_some()
                    && query.is_none()
                    && self.method == "POST"
            }
            "decideConcertApplication" => {
                application_id(path, "decision").is_some()
                    && query.is_none()
                    && self.method == "POST"
            }
            "cancelConcertApplication" => {
                // The revision is admitted and fingerprinted by the exact HTTP
                // command request. A retry must carry the same query revision;
                // the aggregate independently validates the current child.
                application_id(path, "").is_some() && self.method == "DELETE"
            }
            "removeConcertMember" => member_target && self.method == "DELETE",
            "cancelConcertParty" => {
                party_tail(path, "cancel").is_some() && query.is_none() && self.method == "POST"
            }
            _ => false,
        };
        let exact_idempotency = match self.operation {
            "createConcertParty"
            | "applyConcertParty"
            | "decideConcertApplication"
            | "cancelConcertApplication"
            | "removeConcertMember"
            | "cancelConcertParty" => self.idempotency_key.is_some_and(|key| !key.is_nil()),
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
            || guard.http_method() != method_for(operation)
            || wire::parse_operation(guard.event(), guard.http_method(), guard.http_target())?
                != operation
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
            if mutation_operation(operation) {
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
    /// Current profile for a party child record. The binding still identifies the
    /// exact command, while the implementation scopes this read to `subject`.
    async fn application_profile_in(
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
    /// The command's separate application lookup precedes its receipt. The
    /// canonical implementation records LOOKUP and forbids command effects.
    async fn track_lookup_in(
        &self,
        connection: &mut PgConnection,
        binding: &PartyLiveBinding,
    ) -> Result<(), PartyLiveError> {
        self.track_in(connection, binding).await
    }
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
                Self::list_in(pool, authority, cursors, binding, query, guard).await
            }
            PartyLiveAction::Detail(party) => {
                Self::detail_in(pool, authority, binding, party, guard).await
            }
            PartyLiveAction::Create(fields) => {
                Self::create(pool, authority, keys, binding, fields, guard).await
            }
            PartyLiveAction::Apply {
                party,
                party_revision,
                note,
            } => {
                Self::mutate(
                    pool,
                    authority,
                    keys,
                    binding,
                    party,
                    PartyAction::Apply {
                        party_revision,
                        note,
                    },
                    guard,
                )
                .await
            }
            PartyLiveAction::Decide {
                application,
                expected_revision,
                party_revision,
                approve,
            } => {
                let party = application_party(&pool, &binding, application, &guard).await?;
                Self::mutate(
                    pool,
                    authority,
                    keys,
                    binding,
                    party,
                    PartyAction::Decide {
                        application,
                        expected: expected_revision,
                        party_revision,
                        approve,
                    },
                    guard,
                )
                .await
            }
            PartyLiveAction::CancelApplication {
                application,
                expected_revision,
            } => {
                Self::mutate(
                    pool,
                    authority,
                    keys,
                    binding,
                    Uuid::nil(),
                    PartyAction::CancelApplication {
                        application,
                        expected: expected_revision,
                    },
                    guard,
                )
                .await
            }
            PartyLiveAction::Remove {
                party,
                user,
                expected_revision,
                party_revision,
            } => {
                Self::mutate(
                    pool,
                    authority,
                    keys,
                    binding,
                    party,
                    PartyAction::Remove {
                        user,
                        expected: expected_revision,
                        party_revision,
                    },
                    guard,
                )
                .await
            }
            PartyLiveAction::CancelParty {
                party,
                expected_revision,
            } => {
                Self::mutate(
                    pool,
                    authority,
                    keys,
                    binding,
                    party,
                    PartyAction::Cancel {
                        expected: expected_revision,
                    },
                    guard,
                )
                .await
            }
        }
    }

    async fn list_in(
        pool: PgPool,
        authority: Arc<A>,
        cursors: PartyCursorCodec,
        binding: Arc<PartyLiveBinding>,
        query: PartyQuery,
        guard: Arc<dyn PartyLiveOwnerGuard>,
    ) -> Result<PartyLiveReply, PartyLiveError> {
        let cursor_binding = query.cursor_binding(&binding)?;
        let mut tx = pool.begin().await.map_err(db)?;
        guard.track_in(&mut tx, &binding).await?;
        local_actor_in(&mut tx, &binding).await?;
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
        // Lock the complete bounded candidate window before acquiring policy
        // pair locks. Taking another Party root after a pair lock can deadlock
        // with a mutation holding that root while waiting for the same pair.
        let rows = wire::scan(
            &mut tx,
            binding.event(),
            &cursor_binding,
            window.upper,
            window.after,
            i64::try_from(MAX_SCAN).map_err(|_| PartyLiveError::Internal)?,
        )
        .await?;
        authority
            .actor_in(
                &mut tx,
                binding.actor(),
                binding.event(),
                CommunityAccess::ReadParties,
            )
            .await
            .map_err(PartyLiveError::from)?;
        local_pair_in(&mut tx, binding.event(), binding.actor(), binding.actor()).await?;
        let count = rows.len();
        let mut after = window.after;
        let mut items = Vec::with_capacity(usize::from(cursor_binding.limit));
        let mut scanned = 0usize;
        for row in rows {
            scanned += 1;
            after = Some(row.position);
            match project_party(&mut tx, &binding, row.party, &guard, &authority).await {
                Ok(value) => items.push(value),
                Err(PartyLiveError::Denied | PartyLiveError::ResourceUnavailable) => continue,
                Err(error) => return Err(error),
            }
            if items.len() == usize::from(cursor_binding.limit) {
                break;
            }
        }
        let more = scanned < count || count == MAX_SCAN;
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
        authority: Arc<A>,
        binding: Arc<PartyLiveBinding>,
        party: Uuid,
        guard: Arc<dyn PartyLiveOwnerGuard>,
    ) -> Result<PartyLiveReply, PartyLiveError> {
        let mut tx = pool.begin().await.map_err(db)?;
        guard.track_in(&mut tx, &binding).await?;
        local_actor_in(&mut tx, &binding).await?;
        let value = wire::load(&mut tx, binding.event(), party)
            .await?
            .ok_or(PartyLiveError::ResourceUnavailable)?;
        authority
            .actor_in(
                &mut tx,
                binding.actor(),
                binding.event(),
                CommunityAccess::ReadParties,
            )
            .await
            .map_err(PartyLiveError::from)?;
        local_pair_in(&mut tx, binding.event(), binding.actor(), binding.actor()).await?;
        let value = project_party(&mut tx, &binding, value, &guard, &authority).await?;
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
        guard.track_in(&mut tx, &binding).await?;
        local_actor_in(&mut tx, &binding).await?;
        let party = wire::load(&mut tx, binding.event(), reference.resource)
            .await?
            .ok_or(PartyLiveError::DependencyUnavailable)?;
        local_pair_in(&mut tx, binding.event(), binding.actor(), binding.actor()).await?;
        let value = project_party(&mut tx, &binding, party, &guard, &authority).await?;
        let reply = wire::party(value)?;
        guard_ready(&guard, &binding)?;
        tx.commit().await.map_err(db)?;
        Ok(reply)
    }

    async fn mutate(
        pool: PgPool,
        authority: Arc<A>,
        keys: Arc<FingerprintKeys>,
        binding: Arc<PartyLiveBinding>,
        party: Uuid,
        action: PartyAction,
        guard: Arc<dyn PartyLiveOwnerGuard>,
    ) -> Result<PartyLiveReply, PartyLiveError> {
        let key = binding
            .idempotency_key()
            .ok_or(PartyLiveError::InvalidInput)?;
        let party = if party.is_nil() {
            let PartyAction::CancelApplication { application, .. } = &action else {
                return Err(PartyLiveError::InvalidInput);
            };
            application_party(&pool, &binding, *application, &guard).await?
        } else {
            party
        };
        let command = PartyCommand {
            actor: binding.actor(),
            event: binding.event(),
            key,
            party,
            action,
        };
        let (receipt, domain_error) = execute_mutation_command(
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
                ReceiptState::Failed => match receipt.failure {
                    Some(concert_application::commands::SafeFailure::PartyFull) => {
                        PartyLiveError::PartyFull
                    }
                    Some(concert_application::commands::SafeFailure::DuplicateApplication) => {
                        PartyLiveError::DuplicateApplication
                    }
                    Some(concert_application::commands::SafeFailure::VersionConflict) => {
                        PartyLiveError::VersionConflict
                    }
                    Some(concert_application::commands::SafeFailure::InvalidTransition) => {
                        PartyLiveError::InvalidTransition
                    }
                    _ => PartyLiveError::InvalidTransition,
                },
                _ => PartyLiveError::DependencyUnavailable,
            });
        }
        let reference = receipt.reference.ok_or(PartyLiveError::Internal)?;
        let mut tx = pool.begin().await.map_err(db)?;
        guard.track_in(&mut tx, &binding).await?;
        local_actor_in(&mut tx, &binding).await?;
        // The reply is a second transaction: retain root-before-pair order
        // here too, including when a competing command already holds UPDATE.
        let reply_party = match binding.operation() {
            "applyConcertParty" | "cancelConcertApplication" | "decideConcertApplication" => {
                wire::load_application(&mut tx, binding.event(), reference.resource).await?
            }
            "removeConcertMember" => {
                wire::load_member_party(&mut tx, binding.event(), party).await?
            }
            "cancelConcertParty" => wire::load(&mut tx, binding.event(), party)
                .await?
                .ok_or(PartyLiveError::DependencyUnavailable)?,
            _ => return Err(PartyLiveError::InvalidInput),
        };
        local_pair_in(&mut tx, binding.event(), binding.actor(), binding.actor()).await?;
        let reply = match binding.operation() {
            "applyConcertParty" | "cancelConcertApplication" | "decideConcertApplication" => {
                wire::application_from_receipt(&mut tx, &binding, reference.resource, &guard)
                    .await?
            }
            "removeConcertMember" => {
                let _ = project_party(&mut tx, &binding, reply_party.clone(), &guard, &authority)
                    .await?;
                wire::member(&reply_party, reference.resource)?
            }
            "cancelConcertParty" => {
                let value =
                    project_party(&mut tx, &binding, reply_party, &guard, &authority).await?;
                wire::party(value)?
            }
            _ => return Err(PartyLiveError::InvalidInput),
        };
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

async fn execute_mutation_command<A>(
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
    let producer = PartyMutationProducer {
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

async fn application_party(
    pool: &PgPool,
    binding: &PartyLiveBinding,
    application: Uuid,
    guard: &Arc<dyn PartyLiveOwnerGuard>,
) -> Result<Uuid, PartyLiveError> {
    let mut tx = pool.begin().await.map_err(db)?;
    guard.track_lookup_in(&mut tx, binding).await?;
    local_actor_in(&mut tx, binding).await?;
    let party = wire::load_application(&mut tx, binding.event(), application).await?;
    tx.commit().await.map_err(db)?;
    Ok(party.id)
}

async fn project_party<A: CommunityAuthority>(
    connection: &mut PgConnection,
    binding: &PartyLiveBinding,
    party: Party,
    guard: &Arc<dyn PartyLiveOwnerGuard>,
    authority: &Arc<A>,
) -> Result<Value, PartyLiveError> {
    authority
        .authorize_in(
            connection,
            binding.actor(),
            binding.event(),
            party.owner,
            CommunityAccess::ReadParties,
        )
        .await
        .map_err(PartyLiveError::from)?;
    local_subject_in(connection, binding.event(), party.owner).await?;
    local_pair_in(connection, binding.event(), binding.actor(), party.owner).await?;
    guard.authorize_in(connection, binding, party.owner).await?;
    let owner = guard
        .public_profile_in(connection, binding, party.owner)
        .await?;
    wire::public_party(&party, owner)
}

pub(crate) async fn profile_for_subject(
    connection: &mut PgConnection,
    binding: &PartyLiveBinding,
    subject: Uuid,
    guard: &Arc<dyn PartyLiveOwnerGuard>,
) -> Result<Value, PartyLiveError> {
    local_subject_in(connection, binding.event(), subject).await?;
    guard
        .application_profile_in(connection, binding, subject)
        .await
}

struct PartyCreateProducer<'a, A, G: ?Sized> {
    command: &'a PartyCommand,
    authority: &'a Arc<A>,
    binding: &'a PartyLiveBinding,
    guard: &'a G,
    error: std::sync::Mutex<Option<PartyLiveError>>,
}

struct PartyMutationProducer<'a, A, G: ?Sized> {
    command: &'a PartyCommand,
    authority: &'a Arc<A>,
    binding: &'a PartyLiveBinding,
    guard: &'a G,
    error: std::sync::Mutex<Option<PartyLiveError>>,
}

impl<A, G: PartyLiveOwnerGuard + ?Sized> PartyMutationProducer<'_, A, G> {
    fn domain(&self, error: PartyError) -> CommandError {
        if let Ok(mut slot) = self.error.lock() {
            *slot = Some(match error {
                PartyError::Denied => PartyLiveError::Denied,
                PartyError::VersionConflict => PartyLiveError::VersionConflict,
                PartyError::InvalidTransition => PartyLiveError::InvalidTransition,
                PartyError::Full => PartyLiveError::PartyFull,
                PartyError::DuplicateApplication => PartyLiveError::DuplicateApplication,
                PartyError::Unavailable => PartyLiveError::DependencyUnavailable,
                PartyError::InvalidInput => PartyLiveError::InvalidInput,
            });
        }
        CommandError::InvalidState
    }
}

#[async_trait]
impl<A, G> CommandProducer for PartyMutationProducer<'_, A, G>
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
        // Match the canonical audience reader: event -> Party root -> pair.
        // Take UPDATE before any policy locks or BE15 budget/receipt work.
        self.phase_write_in(connection).await?;
        let party =
            wire::load_for_update(connection, self.binding.event(), self.command.party_id())
                .await
                .map_err(command_error)?;
        self.guard
            .track_in(connection, self.binding)
            .await
            .map_err(command_error)?;
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
        self.authority
            .authorize_in(
                connection,
                self.binding.actor(),
                self.binding.event(),
                self.binding.actor(),
                CommunityAccess::party(true, self.command.safety()),
            )
            .await?;
        self.guard
            .authorize_in(connection, self.binding, self.binding.actor())
            .await
            .map_err(command_error)?;
        self.authorize_subject(connection, scope, &party).await?;
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
        let mut party = wire::load_member_party(connection, scope.event, self.command.party_id())
            .await
            .map_err(command_error)?;
        let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *connection)
            .await
            .map_err(|_| CommandError::Unavailable)?;
        let change = match party.change(
            scope.actor,
            &self.command.action,
            self.command.created_id(),
            now,
        ) {
            Ok(change) => change,
            Err(error) => {
                if let Some(failure) = party_safe_failure(error) {
                    return Ok((Completion::Failed(failure), None));
                }
                return Err(self.domain(error));
            }
        };
        if matches!(
            self.command.action,
            PartyAction::Apply { .. } | PartyAction::Decide { approve: true, .. }
        ) {
            crate::community::policy::pin_effect_until(connection, party.fields.meeting_ends_at)
                .await?;
        }
        save_party(connection, &party).await?;
        let reference = SafeReference {
            resource: change.resource,
            revision: change.revision,
        }
        .validate()?;
        Ok((
            Completion::Succeeded {
                reference,
                status: 200,
            },
            // BE15 binds result and invalidation to the exact command target.
            // Party audience revocation is synchronous in the locked aggregate;
            // it never waits for this application/member outbox notification.
            Some(Invalidation { reference }),
        ))
    }
}

impl<A, G: PartyLiveOwnerGuard + ?Sized> PartyMutationProducer<'_, A, G>
where
    A: CommunityAuthority,
{
    async fn phase_write_in(&self, connection: &mut PgConnection) -> Result<(), CommandError> {
        let phase: Option<String> = sqlx::query_scalar(
            "select phase from public.concert_events where event_id=$1 for share",
        )
        .bind(self.binding.event())
        .fetch_optional(&mut *connection)
        .await
        .map_err(|_| CommandError::Unavailable)?;
        if phase.is_none() {
            return Err(CommandError::Denied);
        }
        if matches!(phase.as_deref(), Some("OPEN" | "LIVE"))
            || (self.command.safety() && phase.as_deref() == Some("READ_ONLY"))
        {
            Ok(())
        } else {
            Err(CommandError::Denied)
        }
    }

    async fn authorize_subject(
        &self,
        connection: &mut PgConnection,
        scope: Scope,
        party: &Party,
    ) -> Result<(), CommandError> {
        // Authorization runs for both fresh commands and durable receipt replay.
        // Revision/state checks remain in the domain so successful retries work.
        match &self.command.action {
            PartyAction::Decide { .. } | PartyAction::Cancel { .. }
                if scope.actor != party.owner =>
            {
                return Err(CommandError::Denied)
            }
            PartyAction::CancelApplication { application, .. }
                if !party
                    .applications
                    .iter()
                    .any(|a| a.id == *application && a.user == scope.actor) =>
            {
                return Err(CommandError::Denied)
            }
            PartyAction::Remove { user, .. }
                if scope.actor != party.owner && scope.actor != *user =>
            {
                return Err(CommandError::Denied)
            }
            _ => {}
        }
        let subject = match &self.command.action {
            PartyAction::Apply { .. } => scope.actor,
            PartyAction::Decide { application, .. }
            | PartyAction::CancelApplication { application, .. } => {
                party
                    .applications
                    .iter()
                    .find(|value| value.id == *application)
                    .ok_or(CommandError::Denied)?
                    .user
            }
            PartyAction::Remove { user, .. } => *user,
            PartyAction::Cancel { .. } => scope.actor,
            _ => return Err(CommandError::InvalidInput),
        };
        let mut subjects = vec![party.owner, subject];
        subjects.sort_unstable();
        subjects.dedup();
        for subject in subjects {
            local_subject_in(connection, scope.event, subject)
                .await
                .map_err(command_error)?;
            self.authority
                .authorize_in(
                    connection,
                    scope.actor,
                    scope.event,
                    subject,
                    CommunityAccess::party(true, self.command.safety()),
                )
                .await?;
            local_pair_in(connection, scope.event, scope.actor, subject)
                .await
                .map_err(command_error)?;
            self.guard
                .authorize_in(connection, self.binding, subject)
                .await
                .map_err(command_error)?;
        }
        // Application responses contain a profile; authorize that projection
        // before committing the command, including cancellation/rejection.
        if matches!(
            self.command.action,
            PartyAction::Apply { .. }
                | PartyAction::Decide { .. }
                | PartyAction::CancelApplication { .. }
        ) {
            self.guard
                .application_profile_in(connection, self.binding, subject)
                .await
                .map_err(command_error)?;
        }
        Ok(())
    }
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
        self.guard
            .track_in(connection, self.binding)
            .await
            .map_err(command_error)?;
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
        PartyLiveError::PartyFull | PartyLiveError::DuplicateApplication => {
            CommandError::InvalidState
        }
        PartyLiveError::Internal => CommandError::InvalidState,
    }
}

fn party_safe_failure(error: PartyError) -> Option<concert_application::commands::SafeFailure> {
    use concert_application::commands::SafeFailure;
    match error {
        PartyError::Full => Some(SafeFailure::PartyFull),
        PartyError::DuplicateApplication => Some(SafeFailure::DuplicateApplication),
        PartyError::VersionConflict => Some(SafeFailure::VersionConflict),
        PartyError::InvalidTransition => Some(SafeFailure::InvalidTransition),
        _ => None,
    }
}

pub(crate) async fn save_party(connection: &mut PgConnection, party: &Party) -> Result<(), CommandError> {
    let aggregate = serde_json::to_value(party).map_err(|_| CommandError::InvalidState)?;
    sqlx::query("insert into public.concert_community_parties(event_id,party_id,owner_id,revision,state,member_count,capacity,meeting_revision,audience_revision,aggregate) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) on conflict(event_id,party_id) do update set owner_id=excluded.owner_id,revision=excluded.revision,state=excluded.state,member_count=excluded.member_count,capacity=excluded.capacity,meeting_revision=excluded.meeting_revision,audience_revision=excluded.audience_revision,aggregate=excluded.aggregate")
        .bind(party.event).bind(party.id).bind(party.owner).bind(party.revision)
        .bind(match party.state { PartyState::Open=>"OPEN",PartyState::Closed=>"CLOSED",PartyState::Cancelled=>"CANCELLED",PartyState::Completed=>"COMPLETED" })
        .bind(party.member_count()).bind(party.fields.capacity).bind(party.meeting_revision)
        .bind(party.audience_revision).bind(aggregate).execute(&mut *connection).await.map_err(|_| CommandError::Unavailable)?;
    sqlx::query("update public.concert_community_members set is_owner=false where event_id=$1 and party_id=$2 and is_owner and user_id<>$3")
        .bind(party.event).bind(party.id).bind(party.owner).execute(&mut *connection).await.map_err(|_| CommandError::Unavailable)?;
    for member in &party.members {
        sqlx::query("insert into public.concert_community_members(event_id,party_id,user_id,is_owner,state,revision) values($1,$2,$3,$4,$5,$6) on conflict(event_id,party_id,user_id) do update set is_owner=excluded.is_owner,state=excluded.state,revision=excluded.revision")
            .bind(party.event).bind(party.id).bind(member.user).bind(member.owner)
            .bind(match member.state { concert_domain::community::party::MemberState::Active=>"ACTIVE",concert_domain::community::party::MemberState::Removed=>"REMOVED",concert_domain::community::party::MemberState::Left=>"LEFT" })
            .bind(member.revision).execute(&mut *connection).await.map_err(|_| CommandError::Unavailable)?;
    }
    for application in &party.applications {
        sqlx::query("insert into public.concert_community_applications(event_id,party_id,application_id,user_id,state,revision) values($1,$2,$3,$4,$5,$6) on conflict(event_id,application_id) do update set state=excluded.state,revision=excluded.revision")
            .bind(party.event).bind(party.id).bind(application.id).bind(application.user)
            .bind(match application.state { concert_domain::community::party::ApplicationState::Pending=>"PENDING",concert_domain::community::party::ApplicationState::Approved=>"APPROVED",concert_domain::community::party::ApplicationState::Rejected=>"REJECTED",concert_domain::community::party::ApplicationState::Cancelled=>"CANCELLED" })
            .bind(application.revision).execute(&mut *connection).await.map_err(|_| CommandError::Unavailable)?;
    }
    Ok(())
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

fn mutation_operation(operation: &str) -> bool {
    matches!(
        operation,
        "createConcertParty"
            | "applyConcertParty"
            | "decideConcertApplication"
            | "cancelConcertApplication"
            | "removeConcertMember"
            | "cancelConcertParty"
    )
}

fn method_for(operation: &str) -> &'static str {
    match operation {
        "listConcertParties" | "getConcertParty" => "GET",
        "cancelConcertApplication" | "removeConcertMember" => "DELETE",
        _ => "POST",
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

#[cfg(test)]
pub(crate) async fn probe_visible_in(
    c: &mut PgConnection,
    event: Uuid,
    actor: Uuid,
    subject: Uuid,
) -> bool {
    let _ = sqlx::query("select set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '1 minute')::text,false)").execute(&mut *c).await;
    local_subject_in(c, event, subject).await.is_ok()
        && local_pair_in(c, event, actor, subject).await.is_ok()
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
