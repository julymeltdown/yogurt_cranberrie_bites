//! Controlled repository failures; library diagnostics stay in the adapter.
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationRepositoryError {
    ReportMissing,
    ReceiptMissing,
    ReceiptExpired,
    ReceiptConflict,
    TargetUnavailable,
    DecisionInProgress,
    DecisionConflict,
    ResolutionNoteRequired,
    ReplayConflict,
    Storage,
    Unexpected,
}

pub type ModerationRepositoryResult<T> = Result<T, ModerationRepositoryError>;

impl ModerationRepositoryError {
    pub fn is_receipt(self) -> bool {
        matches!(
            self,
            Self::ReceiptMissing | Self::ReceiptExpired | Self::ReceiptConflict
        )
    }
}

impl fmt::Display for ModerationRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ReportMissing => "Report not found",
            Self::ReceiptMissing => "moderation report mutation receipt is missing",
            Self::ReceiptExpired => "moderation report mutation receipt is expired",
            Self::ReceiptConflict => {
                "moderation report mutation receipt conflicts with the operation"
            }
            Self::TargetUnavailable => "moderation target is no longer available",
            Self::DecisionInProgress => "moderation decision is already being processed",
            Self::DecisionConflict => "moderation report has a conflicting staged decision",
            Self::ResolutionNoteRequired => {
                "a non-empty moderation note is required to dismiss an unavailable target"
            }
            Self::ReplayConflict => {
                "moderation content decision cannot be replayed in its current state"
            }
            Self::Storage => "Moderation repository operation failed",
            Self::Unexpected => "Internal error",
        })
    }
}

impl std::error::Error for ModerationRepositoryError {}
