//! Controlled failures carried by Notification capabilities and decisions.
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationInvalidInput {
    UserIdRequired,
    NotificationIdRequired,
    ActorIdRequired,
    SagaIdRequired,
    RetentionDeadlineInvalid,
    InvalidEventId,
    InvalidActorId,
    InvalidOccurredAt,
    TypeRequired,
    UnsupportedType,
    InvalidCursor,
    CursorRecipientMismatch,
    LegacyCursorUnavailable,
    LegacyCursorMismatch,
    DeletionSagaInputMismatch,
}

impl NotificationInvalidInput {
    pub fn message(self) -> &'static str {
        match self {
            Self::UserIdRequired => "User ID is required",
            Self::NotificationIdRequired => "Notification ID is required",
            Self::ActorIdRequired => "Actor ID is required",
            Self::SagaIdRequired => "Saga ID is required",
            Self::RetentionDeadlineInvalid => "Retention deadline is invalid",
            Self::InvalidEventId => "Invalid event ID",
            Self::InvalidActorId => "Invalid actor ID",
            Self::InvalidOccurredAt => "Invalid occurredAt",
            Self::TypeRequired => "Type is required",
            Self::UnsupportedType => "Unsupported notification type",
            Self::InvalidCursor => "Invalid cursor format",
            Self::CursorRecipientMismatch => "Cursor recipient mismatch",
            Self::LegacyCursorUnavailable => "Legacy cursor anchor is unavailable",
            Self::LegacyCursorMismatch => "Legacy cursor anchor does not match its timestamp",
            Self::DeletionSagaInputMismatch => {
                "Notification deletion saga id was reused with different inputs"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationError {
    InvalidInput(NotificationInvalidInput),
    NotFound,
    Forbidden,
    AdmissionExpired,
    DependencyUnavailable,
    Unexpected,
}

pub type NotificationResult<T> = Result<T, NotificationError>;

impl From<NotificationInvalidInput> for NotificationError {
    fn from(value: NotificationInvalidInput) -> Self {
        Self::InvalidInput(value)
    }
}

impl fmt::Display for NotificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput(value) => value.message(),
            Self::NotFound => "Notification not found",
            Self::Forbidden => "Forbidden",
            Self::AdmissionExpired => "Notification admission expired",
            Self::DependencyUnavailable => "repository unavailable",
            Self::Unexpected => "Internal error",
        })
    }
}

// No library cause, caller data or diagnostic string can cross this boundary.
impl std::error::Error for NotificationError {}
