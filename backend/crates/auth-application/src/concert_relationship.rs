//! Auth-owned bounded relationship observations, never transaction fences.
pub use crate::concert_identity::{ReadBudget, ReadCancellation, ReadError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::fmt;
use uuid::Uuid;

pub const MAX_REVISION: i64 = 9_007_199_254_740_991;
pub const MAX_OBSERVATION: chrono::Duration = chrono::Duration::seconds(2);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    PublicProfile,
    Presence,
    Interaction,
    SafetySelf,
    SafetyInteraction,
    PrivateHandle,
}
impl Purpose {
    pub fn parse(value: &str) -> Result<Self, ReadError> {
        match value {
            "PUBLIC_PROFILE" => Ok(Self::PublicProfile),
            "PRESENCE" => Ok(Self::Presence),
            "INTERACTION" => Ok(Self::Interaction),
            "SAFETY_SELF" => Ok(Self::SafetySelf),
            "SAFETY_INTERACTION" => Ok(Self::SafetyInteraction),
            "PRIVATE_HANDLE" => Ok(Self::PrivateHandle),
            _ => Err(ReadError::InvalidInput),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PublicProfile => "PUBLIC_PROFILE",
            Self::Presence => "PRESENCE",
            Self::Interaction => "INTERACTION",
            Self::SafetySelf => "SAFETY_SELF",
            Self::SafetyInteraction => "SAFETY_INTERACTION",
            Self::PrivateHandle => "PRIVATE_HANDLE",
        }
    }
}
/// Shape validation only. Transport must authenticate the signed caller/actor,
/// original deadline and complete body before constructing this owner query.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RelationshipRead {
    event_id: Uuid,
    actor_id: Uuid,
    subject_id: Uuid,
    session_family_id: Uuid,
    request_id: Uuid,
    purpose: Purpose,
    expires_at: DateTime<Utc>,
}
impl fmt::Debug for RelationshipRead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RelationshipRead([REDACTED])")
    }
}
impl RelationshipRead {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        event_id: Uuid,
        actor_id: Uuid,
        subject_id: Uuid,
        session_family_id: Uuid,
        request_id: Uuid,
        purpose: Purpose,
        expires_at: DateTime<Utc>,
    ) -> Result<Self, ReadError> {
        if [
            event_id,
            actor_id,
            subject_id,
            session_family_id,
            request_id,
        ]
        .iter()
        .any(Uuid::is_nil)
        {
            return Err(ReadError::InvalidInput);
        }
        if purpose == Purpose::SafetySelf && actor_id != subject_id {
            return Err(ReadError::InvalidInput);
        }
        Ok(Self {
            event_id,
            actor_id,
            subject_id,
            session_family_id,
            request_id,
            purpose,
            expires_at,
        })
    }
    pub fn event_id(self) -> Uuid {
        self.event_id
    }
    pub fn actor_id(self) -> Uuid {
        self.actor_id
    }
    pub fn subject_id(self) -> Uuid {
        self.subject_id
    }
    pub fn session_family_id(self) -> Uuid {
        self.session_family_id
    }
    pub fn request_id(self) -> Uuid {
        self.request_id
    }
    pub fn purpose(self) -> Purpose {
        self.purpose
    }
    pub fn expires_at(self) -> DateTime<Utc> {
        self.expires_at
    }
    pub fn check_at(self, at: DateTime<Utc>) -> Result<(), ReadError> {
        if at >= self.expires_at {
            Err(ReadError::DeadlineExceeded)
        } else if self.expires_at - at > MAX_OBSERVATION {
            Err(ReadError::InvalidInput)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CurrentRelationship {
    pub query: RelationshipRead,
    pub observed_at: DateTime<Utc>,
    pub valid_until: DateTime<Utc>,
    pub owner_epoch: Uuid,
    pub revision: i64,
    pub actor_active: bool,
    pub session_live: bool,
    pub subject_active: bool,
}
impl fmt::Debug for CurrentRelationship {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CurrentRelationship([REDACTED])")
    }
}
impl CurrentRelationship {
    pub fn validate(self, query: RelationshipRead, now: DateTime<Utc>) -> Result<Self, ReadError> {
        if self.query != query
            || self.owner_epoch.is_nil()
            || !(1..=MAX_REVISION).contains(&self.revision)
            || self.observed_at > now
            || self.valid_until <= now
            || self.valid_until <= self.observed_at
            || self.valid_until > query.expires_at()
            || self.valid_until - self.observed_at > MAX_OBSERVATION
        {
            return Err(ReadError::Unavailable);
        }
        Ok(self)
    }
    pub fn eligible(self) -> bool {
        self.actor_active && self.session_live && self.subject_active
    }
}
#[async_trait]
pub trait CurrentRelationshipPort: Send + Sync {
    async fn read_current(
        &self,
        request: RelationshipRead,
        budget: &ReadBudget,
    ) -> Result<CurrentRelationship, ReadError>;
}
