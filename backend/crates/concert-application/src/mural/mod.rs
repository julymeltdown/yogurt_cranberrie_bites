//! Narrow internal ports; no transport, persistence, environment defaults or operator roles.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_domain::mural::{MuralDelta, MuralSnapshot};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MuralActor {
    pub event: Uuid,
    pub actor: Uuid,
}
impl MuralActor {
    pub fn valid(self) -> bool {
        !self.event.is_nil() && !self.actor.is_nil()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MuralError {
    #[error("Invalid mural command")]
    InvalidInput,
    #[error("Resource unavailable")]
    Denied,
    #[error("Current policy unavailable")]
    PolicyUnavailable,
    #[error("Storage unavailable; recover using the same command key")]
    StorageUnavailable,
    #[error("Mural revision conflict")]
    RevisionConflict,
    #[error("Event is read only")]
    EventReadOnly,
    #[error("Mural paused")]
    Paused,
    #[error("Mural frozen")]
    Frozen,
    #[error("Pixel region locked")]
    RegionLocked,
    #[error("Mural cooldown ({0:?})")]
    Cooldown(Option<DateTime<Utc>>),
    #[error("Invalid stored mural state")]
    InvalidState,
    #[error("Command correlation failed: {0}")]
    Command(#[from] crate::commands::CommandError),
}
#[async_trait]
pub trait MuralReader: Send + Sync {
    /// Fresh canonical Auth/event/relationship authorization for EVERY delivery, fail closed.
    async fn authorize(&self, actor: MuralActor) -> Result<(), MuralError>;
    async fn snapshot(&self, actor: MuralActor) -> Result<MuralSnapshot, MuralError>;
    async fn delta(&self, actor: MuralActor, after: i64) -> Result<MuralDelta, MuralError>;
}

pub mod tools_b;
