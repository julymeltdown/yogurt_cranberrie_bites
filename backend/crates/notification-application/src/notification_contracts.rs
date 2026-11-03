use crate::dto::repository_dto::NotificationRecordDto;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationSuppression {
    Recipient,
    Actor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationCreateOutcome {
    Created(NotificationRecordDto),
    Existing(NotificationRecordDto),
    Suppressed(NotificationSuppression),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationPosition {
    pub created_at: DateTime<Utc>,
    pub id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationCursor {
    V2 {
        recipient_id: Uuid,
        position: NotificationPosition,
    },
    Legacy {
        epoch_millis: i64,
        id: Uuid,
    },
}

#[derive(Clone, PartialEq, Eq)]
pub struct NotificationQueryPage {
    pub items: Vec<NotificationRecordDto>,
    pub next_position: Option<NotificationPosition>,
    /// Compatibility only for legacy in-memory facade implementations. The
    /// PostgreSQL adapter never sets this and always emits recipient-bound v2.
    pub legacy_next_cursor: Option<String>,
}

impl std::fmt::Debug for NotificationQueryPage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NotificationQueryPage")
            .field("items_len", &self.items.len())
            .field("next_position", &"[REDACTED]")
            .field("legacy_next_cursor", &"[REDACTED]")
            .finish()
    }
}
