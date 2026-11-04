//! Removal-only metadata derived from the sealed authenticated replay client.
use super::{ApplyError, Victim};
use crate::adapters::out::relationship_authority::invalidation::{
    InvalidationOwner, OwnerCursor, SignalReason, SignalScope, VerifiedSignal,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use uuid::Uuid;

pub(super) const FRESHNESS: Duration = Duration::from_millis(250);
pub(super) const POLL: Duration = Duration::from_millis(125);
pub(super) const MAX_BRIDGES: usize = 16;
pub(super) const MAX_VICTIMS: usize = MAX_BRIDGES * 800;

/// A checkpoint is never a grant. This gate only suppresses the existing typed
/// authorization path; opening it cannot manufacture a view or a permit.
pub(crate) struct RemoteReadiness(Mutex<(Uuid, Option<Instant>)>);
impl RemoteReadiness {
    pub(super) fn new() -> Self {
        Self(Mutex::new((Uuid::new_v4(), None)))
    }
    pub(crate) fn snapshot(&self) -> Option<Uuid> {
        let state = self.0.lock().ok()?;
        state
            .1
            .filter(|until| Instant::now() < *until)
            .map(|_| state.0)
    }
    pub(super) fn close(&self) -> Result<(), ApplyError> {
        let mut state = self.0.lock().map_err(|_| ApplyError::Unavailable)?;
        state.0 = Uuid::new_v4();
        state.1 = None;
        Ok(())
    }
    pub(super) fn open(&self, checked: Instant) -> Result<(), ApplyError> {
        let until = checked
            .checked_add(FRESHNESS)
            .ok_or(ApplyError::Unavailable)?;
        if until <= Instant::now() {
            return Err(ApplyError::Unavailable);
        }
        self.0.lock().map_err(|_| ApplyError::Unavailable)?.1 = Some(until);
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RemoteScope {
    User(Uuid),
    Family { user: Uuid, family: Uuid },
    Pair { a: Uuid, b: Uuid },
}
impl RemoteScope {
    pub(crate) fn matches(&self, user: Uuid, family: Uuid) -> bool {
        match self {
            Self::User(u) => *u == user,
            Self::Family { user: u, family: f } => *u == user && *f == family,
            Self::Pair { a, b } => *a == user || *b == user,
        }
    }
    pub(crate) fn public_users(&self) -> Vec<Uuid> {
        match self {
            Self::User(u) => vec![*u],
            Self::Pair { a, b } => vec![*a, *b],
            // A different family of the same user is not a victim.
            Self::Family { .. } => vec![],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RemoteVictim {
    pub event: Uuid,
    pub victim: Victim,
}

pub(super) fn owner_name(owner: InvalidationOwner) -> &'static str {
    match owner {
        InvalidationOwner::Auth => "AUTH",
        InvalidationOwner::Friend => "FRIEND",
    }
}
pub(super) struct Signal {
    pub cursor: OwnerCursor,
    pub id: Uuid,
    pub reason: &'static str,
    pub kind: &'static str,
    pub a: Uuid,
    pub b: Uuid,
    pub scope: RemoteScope,
}
impl Signal {
    pub fn from_verified(value: &VerifiedSignal) -> Result<Self, ApplyError> {
        use InvalidationOwner::*;
        use SignalReason::*;
        let reason = match (value.owner(), value.reason(), value.scope()) {
            (Auth, AuthSessionChanged, SignalScope::Family { .. }) => "AUTH_SESSION_CHANGED",
            (Auth, AuthUserAuthorityChanged, SignalScope::User { .. }) => {
                "AUTH_USER_AUTHORITY_CHANGED"
            }
            (Auth, AuthCredentialsChanged, SignalScope::User { .. }) => "AUTH_CREDENTIALS_CHANGED",
            (Auth, AuthUserRemoved, SignalScope::User { .. }) => "AUTH_USER_REMOVED",
            (Friend, FriendRelationshipChanged, SignalScope::Pair { .. }) => {
                "FRIEND_RELATIONSHIP_CHANGED"
            }
            (Friend, FriendUserRemoved, SignalScope::User { .. }) => "FRIEND_USER_REMOVED",
            _ => return Err(ApplyError::UnqualifiedSource),
        };
        let (kind, a, b, scope) = match value.scope() {
            SignalScope::User { user } => ("USER", user, user, RemoteScope::User(user)),
            SignalScope::Family { user, family } => {
                ("FAMILY", user, family, RemoteScope::Family { user, family })
            }
            SignalScope::Pair { actor, subject } => (
                "PAIR",
                actor,
                subject,
                RemoteScope::Pair {
                    a: actor,
                    b: subject,
                },
            ),
        };
        if a.is_nil() || b.is_nil() || value.signal_id().is_nil() || value.cursor().sequence() < 1 {
            return Err(ApplyError::UnqualifiedSource);
        }
        Ok(Self {
            cursor: value.cursor(),
            id: value.signal_id(),
            reason,
            kind,
            a,
            b,
            scope,
        })
    }
    /// Independent journals may legally use the same UUID. Native marker keys
    /// include their source and epoch, never an unqualified signal ID alone.
    pub fn fence_id(&self, consumer: Uuid) -> Uuid {
        let mut hash = Sha256::new();
        hash.update(b"mcr-world-remote-removal-v1");
        hash.update(consumer.as_bytes());
        hash.update(owner_name(self.cursor.owner()).as_bytes());
        hash.update(self.cursor.epoch().as_bytes());
        hash.update(self.id.as_bytes());
        let digest = hash.finalize();
        let mut bytes = [0; 16];
        bytes.copy_from_slice(&digest[..16]);
        bytes[6] = (bytes[6] & 0x0f) | 0x80;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Uuid::from_bytes(bytes)
    }
}
