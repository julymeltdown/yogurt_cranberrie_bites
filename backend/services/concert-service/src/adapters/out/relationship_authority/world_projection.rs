//! Local, sealed projection scope derived from checked World Auth. These values
//! are bounded read contexts, never admission grants or distributed fences.
use super::{
    world_auth::{WorldAuthObservation, WorldOwnerRequest},
    BUDGET,
};
use crate::world::CurrentOwnedMembership;
use chrono::{DateTime, Utc};
use concert_application::world_ticket::{
    ConnectionId, OwnerIdentity, ResourcePin, RoomId, TicketClaim, WorldError, WorldSubject,
};
use concert_domain::relationship::{Field, PolicyQuery};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use std::{
    collections::BTreeMap,
    fmt,
    future::Future,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use uuid::Uuid;

// Request-local bookkeeping, bounded above the largest room (800 members).
const MAX_TARGETS: usize = 800;
#[derive(Clone)]
pub(super) enum ProjectionSource {
    Consumed(TicketClaim),
    Current(CurrentOwnedMembership),
}
#[derive(Default)]
struct PresenceAttempts {
    auth: AtomicBool,
    friend: AtomicBool,
}
#[derive(Default)]
pub(super) struct ProjectionState {
    cancelled: AtomicBool,
    profile: AtomicBool,
    targets: Mutex<BTreeMap<Uuid, Arc<PresenceAttempts>>>,
}
impl ProjectionState {
    pub(super) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub(super) fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    fn target(&self, subject: Uuid) -> Result<Arc<PresenceAttempts>, WorldError> {
        let mut targets = self.targets.lock().map_err(|_| WorldError::Unavailable)?;
        if let Some(gates) = targets.get(&subject) {
            return Ok(gates.clone());
        }
        if targets.len() >= MAX_TARGETS {
            return Err(WorldError::Unavailable);
        }
        let gates = Arc::new(PresenceAttempts::default());
        targets.insert(subject, gates.clone());
        Ok(gates)
    }
}
#[derive(Clone)]
pub(crate) struct WorldProjectionParent {
    subject: WorldSubject,
    family: Uuid,
    request: Uuid,
    credential_expires_at: DateTime<Utc>,
    source_expires_at: DateTime<Utc>,
    original: AuthorityRpcDeadline,
    deadline: AuthorityRpcDeadline,
    source: ProjectionSource,
    state: Arc<ProjectionState>,
}
impl WorldProjectionParent {
    // Only WorldOwnerRequest::projection_parent calls this after auth.check.
    pub(super) fn capture(
        request: &WorldOwnerRequest,
        auth: &WorldAuthObservation,
    ) -> Result<Self, WorldError> {
        auth.check(request)?;
        let deadline = clip(request.deadline, auth.valid_until().timestamp_micros())?;
        let result = Self {
            subject: request.subject.clone(),
            family: request.family,
            request: request.request_id,
            credential_expires_at: request.credential_expires_at,
            source_expires_at: request.source_expires_at,
            original: request.original_deadline,
            deadline,
            source: request.source.clone(),
            state: request.state.clone(),
        };
        result.check()?;
        Ok(result)
    }
    pub(crate) fn subject(&self) -> &WorldSubject {
        &self.subject
    }
    pub(crate) fn actor(&self) -> Uuid {
        self.subject.key.user_id.uuid()
    }
    pub(crate) fn event_id(&self) -> Uuid {
        self.subject.event_id.uuid()
    }
    pub(crate) fn session_family_id(&self) -> Uuid {
        self.family
    }
    pub(crate) fn request_id(&self) -> Uuid {
        self.request
    }
    pub(crate) fn credential_expires_at(&self) -> DateTime<Utc> {
        self.credential_expires_at
    }
    pub(crate) fn source_expires_at(&self) -> DateTime<Utc> {
        self.source_expires_at
    }
    pub(crate) fn deadline(&self) -> AuthorityRpcDeadline {
        self.deadline
    }
    pub(crate) fn original_deadline(&self) -> AuthorityRpcDeadline {
        self.original
    }
    pub(crate) fn current_membership(&self) -> Option<&CurrentOwnedMembership> {
        match &self.source {
            ProjectionSource::Current(m) => Some(m),
            _ => None,
        }
    }
    pub(crate) fn consumed_ticket(&self) -> Option<&TicketClaim> {
        match &self.source {
            ProjectionSource::Consumed(c) => Some(c),
            _ => None,
        }
    }
    pub(crate) fn pin(&self) -> &ResourcePin {
        match &self.source {
            ProjectionSource::Current(m) => &m.membership.pin,
            ProjectionSource::Consumed(c) => &c.record.pin,
        }
    }
    pub(crate) fn room_id(&self) -> RoomId {
        self.pin().room_id
    }
    pub(crate) fn connection_id(&self) -> ConnectionId {
        match &self.source {
            ProjectionSource::Current(m) => m.membership.connection_id,
            ProjectionSource::Consumed(c) => c.connection_id,
        }
    }
    pub(crate) fn check(&self) -> Result<(), WorldError> {
        if self.state.cancelled() {
            return Err(WorldError::Unavailable);
        }
        if Utc::now() >= self.credential_expires_at || Utc::now() >= self.source_expires_at {
            return Err(WorldError::CredentialExpired);
        }
        self.deadline
            .remaining(
                BUDGET,
                self.source_expires_at
                    .timestamp_micros()
                    .min(self.credential_expires_at.timestamp_micros()),
            )
            .map(|_| ())
            .map_err(|_| WorldError::Unavailable)
    }
    pub(crate) fn cancel(&self) {
        self.state.cancel();
    }
    pub(crate) fn profile_projection(&self) -> Result<WorldProfileProjection, WorldError> {
        self.check()?;
        Ok(WorldProfileProjection {
            parent: self.clone(),
        })
    }
    pub(crate) fn presence_projection(
        &self,
        candidate: &CurrentOwnedMembership,
        expected_owner: &OwnerIdentity,
        expected_subject: &WorldSubject,
        expected_connection: ConnectionId,
    ) -> Result<WorldPresenceProjection, WorldError> {
        self.check()?;
        let actor = self
            .current_membership()
            .ok_or(WorldError::MembershipMissing)?;
        WorldOwnerRequest::validate_current(
            candidate,
            expected_owner,
            expected_subject,
            expected_connection,
        )?;
        if candidate.owner != actor.owner
            || candidate.membership.key.event_id != actor.membership.key.event_id
            || candidate.membership.room_id != actor.membership.room_id
            || candidate.membership.pin != actor.membership.pin
        {
            return Err(WorldError::Conflict);
        }
        let deadline = clip(
            self.deadline,
            candidate
                .membership
                .lease_until
                .timestamp_micros()
                .min(candidate.membership.access_expires_at.timestamp_micros()),
        )?;
        let gates = self.state.target(candidate.membership.key.user_id.uuid())?;
        Ok(WorldPresenceProjection {
            parent: self.clone(),
            candidate: candidate.clone(),
            deadline,
            gates,
        })
    }
}
fn clip(deadline: AuthorityRpcDeadline, expiry: i64) -> Result<AuthorityRpcDeadline, WorldError> {
    AuthorityRpcDeadline::within_original(
        deadline.expires_micros().min(expiry),
        deadline.stop().into_std(),
        BUDGET,
    )
    .map_err(|_| WorldError::Unavailable)
}
fn burn(gate: &AtomicBool) -> Result<(), WorldError> {
    gate.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map(|_| ())
        .map_err(|_| WorldError::CredentialConsumed)
}
// Derivation and cloning preserve scope and shared gates; no public raw constructor.
#[derive(Clone)]
pub(crate) struct WorldProfileProjection {
    parent: WorldProjectionParent,
}
#[derive(Clone)]
pub(crate) struct WorldPresenceProjection {
    parent: WorldProjectionParent,
    candidate: CurrentOwnedMembership,
    deadline: AuthorityRpcDeadline,
    gates: Arc<PresenceAttempts>,
}
macro_rules! scope_getters {
    () => {
        pub(crate) fn actor(&self) -> Uuid {
            self.parent.actor()
        }
        pub(crate) fn event_id(&self) -> Uuid {
            self.parent.event_id()
        }
        pub(crate) fn session_family_id(&self) -> Uuid {
            self.parent.session_family_id()
        }
        pub(crate) fn request_id(&self) -> Uuid {
            self.parent.request_id()
        }
        pub(crate) fn credential_expires_at(&self) -> DateTime<Utc> {
            self.parent.credential_expires_at()
        }
        pub(crate) fn source_expires_at(&self) -> DateTime<Utc> {
            self.parent.source_expires_at()
        }
        pub(crate) fn original_deadline(&self) -> AuthorityRpcDeadline {
            self.parent.original_deadline()
        }
        pub(crate) fn cancel(&self) {
            self.parent.cancel();
        }
    };
}
impl WorldProfileProjection {
    scope_getters!();
    pub(crate) fn subject(&self) -> Uuid {
        self.actor()
    }
    pub(crate) fn purpose(&self) -> &'static str {
        "WORLD_SELF_PROFILE"
    }
    pub(crate) fn deadline(&self) -> AuthorityRpcDeadline {
        self.parent.deadline()
    }
    pub(crate) fn check(&self) -> Result<(), WorldError> {
        self.parent.check()
    }
    pub(crate) fn begin_read(&self) -> Result<WorldProjectionAttempt, WorldError> {
        // Burn before checking expiry/cancellation: an attempted owner never gets
        // another slot, including failed reservations and dropped tokens.
        burn(&self.parent.state.profile)?;
        WorldProjectionAttempt::reserve(self.parent.clone(), None, self.deadline(), self.subject())
    }
}
impl WorldPresenceProjection {
    scope_getters!();
    pub(crate) fn parent(&self) -> &WorldProjectionParent {
        &self.parent
    }
    pub(crate) fn candidate(&self) -> &CurrentOwnedMembership {
        &self.candidate
    }
    pub(crate) fn subject(&self) -> Uuid {
        self.candidate.membership.key.user_id.uuid()
    }
    pub(crate) fn purpose(&self) -> &'static str {
        "PRESENCE"
    }
    pub(crate) fn query(&self) -> PolicyQuery {
        PolicyQuery {
            event: self.parent.subject.event_id,
            actor: self.parent.subject.key.user_id,
            subject: self.candidate.membership.key.user_id,
            field: Field::Presence,
        }
    }
    pub(crate) fn candidate_credential_expires_at(&self) -> DateTime<Utc> {
        self.candidate.membership.access_expires_at
    }
    pub(crate) fn candidate_source_expires_at(&self) -> DateTime<Utc> {
        self.candidate.membership.lease_until
    }
    pub(crate) fn deadline(&self) -> AuthorityRpcDeadline {
        self.deadline
    }
    pub(crate) fn check(&self) -> Result<(), WorldError> {
        self.parent.check()?;
        check_candidate(&self.candidate)?;
        self.deadline
            .remaining(BUDGET, self.deadline.expires_micros())
            .map(|_| ())
            .map_err(|_| WorldError::Unavailable)
    }
    pub(crate) fn begin_auth_read(&self) -> Result<WorldProjectionAttempt, WorldError> {
        burn(&self.gates.auth)?;
        WorldProjectionAttempt::reserve(
            self.parent.clone(),
            Some(self.candidate.clone()),
            self.deadline,
            self.subject(),
        )
    }
    pub(crate) fn begin_friend_read(&self) -> Result<WorldProjectionAttempt, WorldError> {
        burn(&self.gates.friend)?;
        WorldProjectionAttempt::reserve(
            self.parent.clone(),
            Some(self.candidate.clone()),
            self.deadline,
            self.subject(),
        )
    }
}
fn check_candidate(candidate: &CurrentOwnedMembership) -> Result<(), WorldError> {
    if Utc::now() >= candidate.membership.access_expires_at
        || Utc::now() >= candidate.membership.lease_until
    {
        Err(WorldError::CredentialExpired)
    } else {
        Ok(())
    }
}
/// Not Clone. Reservation burns the corresponding owner slot even if dropped.
pub(crate) struct WorldProjectionAttempt {
    parent: WorldProjectionParent,
    candidate: Option<CurrentOwnedMembership>,
    deadline: AuthorityRpcDeadline,
    subject: Uuid,
}
impl WorldProjectionAttempt {
    scope_getters!();
    fn reserve(
        parent: WorldProjectionParent,
        candidate: Option<CurrentOwnedMembership>,
        deadline: AuthorityRpcDeadline,
        subject: Uuid,
    ) -> Result<Self, WorldError> {
        let stop = platform::call_deadline::current()
            .unwrap_or(deadline.stop().into_std())
            .min(deadline.stop().into_std());
        let deadline =
            AuthorityRpcDeadline::within_original(deadline.expires_micros(), stop, BUDGET)
                .map_err(|_| WorldError::Unavailable)?;
        let result = Self {
            parent,
            candidate,
            deadline,
            subject,
        };
        result.check()?;
        Ok(result)
    }
    pub(crate) fn subject(&self) -> Uuid {
        self.subject
    }
    pub(crate) fn deadline(&self) -> AuthorityRpcDeadline {
        self.deadline
    }
    pub(crate) fn check(&self) -> Result<(), WorldError> {
        self.parent.check()?;
        if let Some(candidate) = &self.candidate {
            check_candidate(candidate)?;
        }
        self.deadline
            .remaining(BUDGET, self.deadline.expires_micros())
            .map(|_| ())
            .map_err(|_| WorldError::Unavailable)
    }
    pub(crate) async fn run<T, F>(mut self, future: F) -> Result<T, WorldError>
    where
        F: Future<Output = Result<T, WorldError>>,
    {
        // An earlier ambient stop at reservation or execution can only shorten.
        if let Some(outer) = platform::call_deadline::current() {
            self.deadline = AuthorityRpcDeadline::within_original(
                self.deadline.expires_micros(),
                self.deadline.stop().into_std().min(outer),
                BUDGET,
            )
            .map_err(|_| WorldError::Unavailable)?;
        }
        self.check()?;
        let stop = self.deadline.stop();
        let result = platform::call_deadline::scope(stop.into_std(), async {
            tokio::select! {
                biased;
                _ = tokio::time::sleep_until(stop) => Err(WorldError::Unavailable),
                _ = async { loop { if self.parent.state.cancelled() { break; } tokio::time::sleep(Duration::from_millis(5)).await; } } => Err(WorldError::Unavailable),
                result = future => result,
            }
        }).await;
        // A synchronous/noncooperative poll must not publish a late success.
        self.check()?;
        result
    }
}
macro_rules! redacted_debug { ($($ty:ty),+) => { $(impl fmt::Debug for $ty { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(concat!(stringify!($ty), "([REDACTED])")) } })+ }; }
redacted_debug!(
    WorldProjectionParent,
    WorldProfileProjection,
    WorldPresenceProjection,
    WorldProjectionAttempt
);
