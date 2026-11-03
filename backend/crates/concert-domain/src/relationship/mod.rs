//! Internal relationship decisions. No wire DTO, credentials, handles or copied identities.
use crate::{AuthUserId, EventId, EventPhase, Revision};
use chrono::{DateTime, Utc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    PublicProfile,
    Presence,
    Interaction,
    /// Own safety operations only; target ownership/lifecycle stays with the producer.
    SafetySelf,
    /// Privacy-restricting cross-user operations; producer proves target ownership.
    SafetyInteraction,
    /// A generic relationship decision can never grant handle access. Use the
    /// reciprocal, platform/version-bound reveal use case in its own transaction.
    PrivateHandle,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolicyQuery {
    pub event: EventId,
    pub actor: AuthUserId,
    pub subject: AuthUserId,
    pub field: Field,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolicyStamp {
    pub event_revision: Revision,
    pub relationship_revision: Revision,
    pub authority_revision: Revision,
}
/// Owner assertions must be current and request-bound, never inferred from a profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreshAuthority {
    pub query: PolicyQuery,
    pub checked_at: DateTime<Utc>,
    pub valid_until: DateTime<Utc>,
    pub revision: Revision,
    pub actor_allowed: bool,
    pub subject_allowed: bool,
    pub inherited_block: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolicySnapshot {
    pub query: PolicyQuery,
    pub phase: EventPhase,
    pub event_revision: Revision,
    pub relationship_revision: Revision,
    pub actor_member: bool,
    pub subject_member: bool,
    pub blocked: bool,
    pub now: DateTime<Utc>,
    pub authority: FreshAuthority,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RelationshipError {
    #[error("Resource is unavailable")]
    Denied,
    #[error("Required policy is unavailable")]
    Unavailable,
    #[error("Authorization changed")]
    Stale,
}
/// A point-in-time projection decision, NOT permission to perform a later mutation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolicyDecision {
    query: PolicyQuery,
    stamp: PolicyStamp,
    checked_at: DateTime<Utc>,
    valid_until: DateTime<Utc>,
}
impl PolicyDecision {
    pub fn query(self) -> PolicyQuery {
        self.query
    }
    pub fn stamp(self) -> PolicyStamp {
        self.stamp
    }
    pub fn checked_at(self) -> DateTime<Utc> {
        self.checked_at
    }
    pub fn valid_until(self) -> DateTime<Utc> {
        self.valid_until
    }
    /// Consumers must fetch current revisions/authority before retaining a projection.
    pub fn ensure_current(
        self,
        query: PolicyQuery,
        stamp: PolicyStamp,
        now: DateTime<Utc>,
    ) -> Result<(), RelationshipError> {
        if self.query != query
            || self.stamp != stamp
            || now < self.checked_at
            || now >= self.valid_until
        {
            return Err(RelationshipError::Stale);
        }
        Ok(())
    }
}
pub fn decide(s: PolicySnapshot) -> Result<PolicyDecision, RelationshipError> {
    let a = s.authority;
    if a.query != s.query || a.checked_at != s.now || a.valid_until <= s.now {
        return Err(RelationshipError::Unavailable);
    }
    if !s.actor_member
        || !s.subject_member
        || !a.actor_allowed
        || !a.subject_allowed
        || s.blocked
        || a.inherited_block
        || matches!(s.phase, EventPhase::Draft | EventPhase::Archived)
        || (s.query.field == Field::SafetySelf && s.query.actor != s.query.subject)
        || s.query.field == Field::PrivateHandle
        || (s.query.field == Field::Interaction && !s.phase.permits_participant_writes())
    {
        return Err(RelationshipError::Denied);
    }
    Ok(PolicyDecision {
        query: s.query,
        stamp: PolicyStamp {
            event_revision: s.event_revision,
            relationship_revision: s.relationship_revision,
            authority_revision: a.revision,
        },
        checked_at: s.now,
        valid_until: a.valid_until,
    })
}

pub mod social;

/// Current actor eligibility only; deliberately carries no relationship revision or target grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActorDecision {
    key: crate::ParticipantKey,
    event_revision: Revision,
    authority_revision: Revision,
    checked_at: DateTime<Utc>,
    valid_until: DateTime<Utc>,
}
impl ActorDecision {
    pub fn key(self) -> crate::ParticipantKey {
        self.key
    }
    pub fn event_revision(self) -> Revision {
        self.event_revision
    }
    pub fn authority_revision(self) -> Revision {
        self.authority_revision
    }
    pub fn checked_at(self) -> DateTime<Utc> {
        self.checked_at
    }
    pub fn valid_until(self) -> DateTime<Utc> {
        self.valid_until
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActorSnapshot {
    pub key: crate::ParticipantKey,
    pub phase: EventPhase,
    pub event_revision: Revision,
    pub member: bool,
    pub now: DateTime<Utc>,
    pub authority: FreshAuthority,
}
pub fn decide_actor(s: ActorSnapshot) -> Result<ActorDecision, RelationshipError> {
    let a = s.authority;
    let expected = PolicyQuery {
        event: s.key.event_id,
        actor: s.key.user_id,
        subject: s.key.user_id,
        field: Field::SafetySelf,
    };
    if a.query != expected || a.checked_at != s.now || a.valid_until <= s.now {
        return Err(RelationshipError::Unavailable);
    }
    if !s.member || !a.actor_allowed || matches!(s.phase, EventPhase::Draft | EventPhase::Archived)
    {
        return Err(RelationshipError::Denied);
    }
    Ok(ActorDecision {
        key: s.key,
        event_revision: s.event_revision,
        authority_revision: a.revision,
        checked_at: s.now,
        valid_until: a.valid_until,
    })
}
