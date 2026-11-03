//! Owner-serialized bridge from qualified canonical authority to BE12/BE14.
//! This module does not qualify the unfinished admission implementation or mint credentials.
use super::{
    interest::PublicIdentity,
    session::AccessDeadline,
    simulation::{AuthorityError, Binding, MembershipLease, MembershipPort, AUTH_LEASE_MAX_MS},
};
use chrono::{DateTime, Utc};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock},
};
use uuid::Uuid;

/// Fixed process sample: subsequent reads cannot move captured access expiry forward.
#[derive(Clone, Debug)]
pub struct ClockAnchor {
    pub monotonic_ms: u64,
    pub utc: DateTime<Utc>,
}
impl ClockAnchor {
    pub fn deadline(&self, utc: DateTime<Utc>) -> Result<u64, AuthorityError> {
        let duration = utc.signed_duration_since(self.utc);
        if duration <= chrono::TimeDelta::zero() {
            return Err(AuthorityError::Revoked);
        }
        let millis =
            u64::try_from(duration.num_milliseconds()).map_err(|_| AuthorityError::Unavailable)?;
        self.monotonic_ms
            .checked_add(millis)
            .ok_or(AuthorityError::Unavailable)
    }
}
/// Trusted observation from an adapter that has checked canonical Auth, Concert
/// participation/event/suspension, exact owner/session/connection and resource pins.
/// `checked_at_ms` describes this local owner-fenced read, not a fabricated cross-service stamp.
#[derive(Clone)]
pub struct CanonicalMembership {
    pub binding: Binding,
    pub checked_at_ms: u64,
    pub lease_expires_at: DateTime<Utc>,
    pub access_expires_at: DateTime<Utc>,
    pub identity: PublicIdentity,
}
/// Nonblocking view only: upstream I/O/refresh occurs outside the room mutation/publication fence.
/// The real adapter must invalidate this view on known revocation and fail closed on expiry/outage.
/// Ticket consumption and database ownership commits precede installation; they are NOT inferred here.
pub trait CanonicalAuthorityPort: Send + Sync + 'static {
    /// Stable shared fence also used by canonical-view and recipient-policy invalidation.
    /// Upstream I/O must finish before taking this fence; never await while holding it.
    fn publication_fence(&self) -> Arc<Mutex<()>>;
    /// Promote a previously checked pending observation inside the owner fence.
    /// Default supports ports whose observations are already synchronously current.
    fn install_pending(&self, _binding: &Binding, _now_ms: u64) -> Result<(), AuthorityError> {
        Ok(())
    }
    fn current(
        &self,
        expected: &Binding,
        now_ms: u64,
    ) -> Result<CanonicalMembership, AuthorityError>;
    /// Idempotent owner-fenced cleanup. Confirmed absence is success; never remove a newer binding.
    fn release(&self, expected: &Binding) -> Result<(), AuthorityError>;
}
/// Production default until an actual canonical adapter is independently qualified.
pub struct UnavailableCanonicalAuthority;
impl CanonicalAuthorityPort for UnavailableCanonicalAuthority {
    fn publication_fence(&self) -> Arc<Mutex<()>> {
        static FENCE: OnceLock<Arc<Mutex<()>>> = OnceLock::new();
        FENCE.get_or_init(|| Arc::new(Mutex::new(()))).clone()
    }
    fn current(&self, _: &Binding, _: u64) -> Result<CanonicalMembership, AuthorityError> {
        Err(AuthorityError::Unavailable)
    }
    fn release(&self, _: &Binding) -> Result<(), AuthorityError> {
        Err(AuthorityError::Unavailable)
    }
}
#[derive(Clone)]
struct Captured {
    binding: Binding,
    deadline: AccessDeadline,
}
pub struct InstalledMembership {
    pub deadline: AccessDeadline,
    pub identity: PublicIdentity,
    pub displaced: Option<Binding>,
}
pub struct MembershipAdapter<P: CanonicalAuthorityPort> {
    port: P,
    anchor: ClockAnchor,
    captured: BTreeMap<(Uuid, Uuid), Captured>,
    capacity: usize,
}
fn key(b: &Binding) -> (Uuid, Uuid) {
    (b.key.event_id.uuid(), b.key.user_id.uuid())
}
fn valid_binding(b: &Binding) -> bool {
    [
        b.key.event_id.uuid(),
        b.key.user_id.uuid(),
        b.session_id,
        b.connection_id,
        b.owner_id,
        b.room_id,
        b.room_epoch,
        b.stream_id,
    ]
    .iter()
    .all(|id| !id.is_nil())
}
impl<P: CanonicalAuthorityPort> MembershipAdapter<P> {
    pub fn new(port: P, anchor: ClockAnchor, capacity: usize) -> Result<Self, AuthorityError> {
        if capacity == 0 || capacity > 800 {
            return Err(AuthorityError::Unavailable);
        }
        Ok(Self {
            port,
            anchor,
            captured: BTreeMap::new(),
            capacity,
        })
    }
    pub fn publication_fence(&self) -> Arc<Mutex<()>> {
        self.port.publication_fence()
    }

    fn observe(
        &self,
        b: &Binding,
        now: u64,
    ) -> Result<(CanonicalMembership, MembershipLease), AuthorityError> {
        if !valid_binding(b) || now < self.anchor.monotonic_ms {
            return Err(AuthorityError::Unavailable);
        }
        let observation = self.port.current(b, now)?;
        if observation.binding != *b || observation.checked_at_ms != now {
            return Err(AuthorityError::Revoked);
        }
        let lease_until_ms = self.anchor.deadline(observation.lease_expires_at)?;
        let access_expires_at_ms = self.anchor.deadline(observation.access_expires_at)?;
        if lease_until_ms <= now
            || access_expires_at_ms <= now
            || lease_until_ms - now > AUTH_LEASE_MAX_MS
        {
            return Err(AuthorityError::Revoked);
        }
        Ok((
            observation,
            MembershipLease {
                binding: b.clone(),
                lease_until_ms,
                access_expires_at_ms,
            },
        ))
    }
    /// Called solely by the serialized owner after committed ticket/admission or recovery authority.
    pub fn install(
        &mut self,
        b: &Binding,
        now: u64,
    ) -> Result<InstalledMembership, AuthorityError> {
        self.port.install_pending(b, now)?;
        let (observation, lease) = self.observe(b, now)?;
        let previous = self.captured.get(&key(b));
        if previous.is_none() && self.captured.len() >= self.capacity {
            return Err(AuthorityError::Unavailable);
        }
        if let Some(old) = previous {
            if old.binding.connection_id == b.connection_id
                && (old.binding.session_id != b.session_id
                    || old.binding.resources != b.resources
                    || old.deadline.monotonic_ms != lease.access_expires_at_ms
                    || old.deadline.utc != observation.access_expires_at)
            {
                return Err(AuthorityError::Revoked);
            }
        }
        // A connection cannot acquire a second canonical identity/event slot.
        if self
            .captured
            .values()
            .any(|old| old.binding.connection_id == b.connection_id && key(&old.binding) != key(b))
        {
            return Err(AuthorityError::Revoked);
        }
        let displaced = previous.map(|p| p.binding.clone());
        let deadline = AccessDeadline {
            monotonic_ms: lease.access_expires_at_ms,
            utc: observation.access_expires_at,
        };
        self.captured.insert(
            key(b),
            Captured {
                binding: b.clone(),
                deadline: deadline.clone(),
            },
        );
        Ok(InstalledMembership {
            deadline,
            identity: observation.identity,
            displaced,
        })
    }
    /// Public identity is fetched from current authority on every frame, never client metadata.
    pub fn identity(&self, b: &Binding, now: u64) -> Result<PublicIdentity, AuthorityError> {
        let captured = self
            .captured
            .get(&key(b))
            .filter(|p| p.binding == *b)
            .ok_or(AuthorityError::Revoked)?;
        let (observation, lease) = self.observe(b, now)?;
        if captured.deadline.monotonic_ms != lease.access_expires_at_ms
            || captured.deadline.utc != observation.access_expires_at
        {
            return Err(AuthorityError::Revoked);
        }
        Ok(observation.identity)
    }
    pub fn release(&mut self, b: &Binding) -> Result<(), AuthorityError> {
        self.port.release(b)?;
        if self.captured.get(&key(b)).is_some_and(|p| p.binding == *b) {
            self.captured.remove(&key(b));
        }
        Ok(())
    }
    pub fn bindings(&self) -> Vec<Binding> {
        self.captured.values().map(|p| p.binding.clone()).collect()
    }
    pub fn captured_count(&self) -> usize {
        self.captured.len()
    }
}
impl<P: CanonicalAuthorityPort> MembershipPort for MembershipAdapter<P> {
    fn current(&self, b: &Binding, now: u64) -> Result<MembershipLease, AuthorityError> {
        let captured = self
            .captured
            .get(&key(b))
            .filter(|p| p.binding == *b)
            .ok_or(AuthorityError::Revoked)?;
        let (observation, lease) = self.observe(b, now)?;
        if captured.deadline.monotonic_ms != lease.access_expires_at_ms
            || captured.deadline.utc != observation.access_expires_at
        {
            return Err(AuthorityError::Revoked);
        }
        Ok(lease)
    }
}
