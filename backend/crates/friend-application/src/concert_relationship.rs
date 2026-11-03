//! Friend-owned bidirectional block observations. Absence of a tombstone is
//! not Auth account eligibility and these observations are not mutation fences.
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadError {
    InvalidInput,
    Cancelled,
    DeadlineExceeded,
    Unavailable,
}
impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "Invalid relationship input",
            Self::Cancelled => "Relationship read cancelled",
            Self::DeadlineExceeded => "Relationship read deadline elapsed",
            Self::Unavailable => "Relationship owner unavailable",
        })
    }
}
impl std::error::Error for ReadError {}
#[derive(Clone, Default)]
pub struct ReadCancellation(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl ReadCancellation {
    pub fn cancel(&self) {
        self.0.store(true, std::sync::atomic::Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Acquire)
    }
}
impl fmt::Debug for ReadCancellation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReadCancellation([REDACTED])")
    }
}
#[derive(Clone)]
pub struct ReadBudget {
    deadline: std::time::Instant,
    cancellation: ReadCancellation,
}
impl ReadBudget {
    pub const MAX_DURATION: std::time::Duration = std::time::Duration::from_secs(2);
    pub fn new(
        deadline: std::time::Instant,
        cancellation: ReadCancellation,
    ) -> Result<Self, ReadError> {
        if deadline.saturating_duration_since(std::time::Instant::now()) > Self::MAX_DURATION {
            return Err(ReadError::InvalidInput);
        }
        Ok(Self {
            deadline,
            cancellation,
        })
    }
    pub fn deadline(&self) -> std::time::Instant {
        self.deadline
    }
    pub fn check(&self) -> Result<(), ReadError> {
        if self.cancellation.is_cancelled() {
            Err(ReadError::Cancelled)
        } else if std::time::Instant::now() >= self.deadline {
            Err(ReadError::DeadlineExceeded)
        } else {
            Ok(())
        }
    }
    pub fn remaining(&self) -> Result<std::time::Duration, ReadError> {
        self.check()?;
        Ok(self
            .deadline
            .saturating_duration_since(std::time::Instant::now()))
    }
}
impl fmt::Debug for ReadBudget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReadBudget([REDACTED])")
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CurrentRelationship {
    pub query: RelationshipRead,
    pub observed_at: DateTime<Utc>,
    pub valid_until: DateTime<Utc>,
    pub owner_epoch: Uuid,
    pub revision: i64,
    pub actor_blocks_subject: bool,
    pub subject_blocks_actor: bool,
    pub actor_deleted: bool,
    pub subject_deleted: bool,
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
    pub fn blocked(self) -> bool {
        self.actor_blocks_subject || self.subject_blocks_actor
    }
    pub fn subjects_present(self) -> bool {
        !self.actor_deleted && !self.subject_deleted
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
