//! Versioned private Wall B projections. Existing pixel/snapshot DTOs stay strict.
use super::{MuralActor, MuralError};
use chrono::{DateTime, Utc};
pub use concert_domain::mural::tools_b::{
    ActionKind, ErasePixel, FillRegion, OwnAction, ToolCapabilities, UndoAction, UndoEligibility,
    UnresolvedAction,
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolCommand {
    Erase(ErasePixel),
    Fill(FillRegion),
    Undo(UndoAction),
}
impl ToolCommand {
    pub fn operation(self) -> &'static str {
        match self {
            Self::Erase(_) => "eraseConcertMuralPixel",
            Self::Fill(_) => "fillConcertMuralRegion",
            Self::Undo(_) => "undoConcertMuralAction",
        }
    }
    pub fn kind(self) -> ActionKind {
        match self {
            Self::Erase(_) => ActionKind::Erase,
            Self::Fill(_) => ActionKind::Fill,
            Self::Undo(_) => ActionKind::Undo,
        }
    }
    pub fn expected_revision(self) -> i64 {
        match self {
            Self::Erase(i) => i.expected_revision,
            Self::Fill(i) => i.expected_revision,
            Self::Undo(i) => i.expected_revision,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ToolError {
    #[error(transparent)]
    Mural(#[from] MuralError),
    #[error("Wall tools are not provisioned")]
    Unavailable,
    #[error("The action changes no cells")]
    NoChange,
    #[error("Fill exceeds 256 changed cells")]
    FillTooLarge,
    #[error("The shared changed-cell budget is exhausted")]
    CellBudget(Option<DateTime<Utc>>),
    #[error("Own action is no longer eligible for undo")]
    UndoUnavailable,
    #[error("Own action undo expired")]
    UndoExpired,
    #[error("Own action is not the latest unchanged action")]
    UndoNotLatest,
    #[error("Recover the original unresolved action before another mutation")]
    Unresolved,
}

#[async_trait::async_trait]
pub trait MuralTools: Send + Sync {
    async fn capabilities(&self, actor: MuralActor) -> Result<ToolCapabilities, ToolError>;
    async fn action_result(&self, actor: MuralActor, key: Uuid) -> Result<OwnAction, ToolError>;
    async fn execute(
        &self,
        actor: MuralActor,
        key: Uuid,
        command: ToolCommand,
    ) -> Result<OwnAction, ToolError>;
}
