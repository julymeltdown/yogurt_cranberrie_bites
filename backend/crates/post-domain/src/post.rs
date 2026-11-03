use crate::comment::Comment;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Post {
    pub id: Uuid,
    pub author_id: Uuid,
    pub content: String,
    pub visibility: String,
    pub created_at: DateTime<Utc>,
    pub comment_count: i64,
    pub like_count: i64,
}

/// Current authorization-relevant state for cached post content. Only active,
/// non-moderated posts receive a snapshot; use its author and visibility values
/// instead of the cached values before checking viewer access.
#[derive(Clone, PartialEq, Eq)]
pub struct PostAccessSnapshot {
    pub author_id: Uuid,
    pub visibility: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostDetail {
    pub post: Post,
    pub comments: Vec<Comment>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostLikeStatus {
    pub post_id: Uuid,
    pub like_count: i64,
    pub liked: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostAuditEvent {
    pub id: Uuid,
    pub actor_id: Option<Uuid>,
    pub post_id: Option<Uuid>,
    pub comment_id: Option<Uuid>,
    pub action: String,
    pub outcome: String,
    pub request_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub metadata_json: String,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationRequest {
    pub user_id: Uuid,
    pub notification_type: String,
    pub payload_json: String,
    pub event_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub occurred_at: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSnapshot {
    pub user_id: Uuid,
    pub nickname: String,
    pub avatar_url: String,
    pub bio: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredEvent {
    pub event_id: Uuid,
    pub event_type: String,
    pub aggregate_id: Uuid,
    /// Database-assigned sequence for events created after aggregate ordering
    /// was introduced. Legacy rows deliberately remain unsequenced.
    pub aggregate_version: Option<i64>,
    /// Internal queue metadata used to link asynchronous delivery to its source.
    /// It is carried in Kafka headers, never in the event body.
    #[serde(skip)]
    pub origin_traceparent: Option<String>,
    pub actor_id: Option<Uuid>,
    pub attributes: Vec<String>,
    pub payload: String,
    pub occurred_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub published: bool,
    pub published_at: Option<DateTime<Utc>>,
    /// The operator resolution currently allowing this event to retry after it
    /// reached the dead-letter threshold. Delivery attempts retain this ID.
    pub replay_resolution_id: Option<Uuid>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct DeadLetterReplayResolution {
    pub resolution_id: Uuid,
    pub event_id: Uuid,
    pub actor_id: Uuid,
    pub reason: String,
    pub evidence_reference: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadLetterReplayOutcome {
    Created,
    Existing,
}

#[derive(Debug, thiserror::Error)]
pub enum DeadLetterReplayError {
    #[error("dead-letter event was not found")]
    EventNotFound,
    #[error("event is not an unpublished dead letter")]
    NotDeadLettered,
    #[error("resolution ID or event state conflicts with an existing resolution")]
    Conflict,
    #[error("reason must contain 20 to 1024 non-control characters and at most 1024 UTF-8 bytes")]
    InvalidReason,
    #[error("evidence reference must be a 3 to 256 character protected-record identifier")]
    InvalidEvidenceReference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OutboxMetricsSnapshot {
    pub unpublished_total: i64,
    pub oldest_unpublished_age_seconds: i64,
    pub dead_lettered_total: i64,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub struct RetentionPurgeResult {
    pub post_ids: Vec<Uuid>,
    pub comment_ids: Vec<Uuid>,
}

impl RetentionPurgeResult {
    pub fn purged_count(&self) -> i64 {
        (self.post_ids.len() + self.comment_ids.len()) as i64
    }

    pub fn purged_post_count(&self) -> i64 {
        self.post_ids.len() as i64
    }

    pub fn purged_comment_count(&self) -> i64 {
        self.comment_ids.len() as i64
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct EventReplayFilter {
    pub actor_id: Option<Uuid>,
    pub event_type: Option<String>,
    pub attribute: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub published: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReplayTarget {
    Internal,
    External,
    Both,
}

/// The delivery destinations an outbox event must reach.
///
/// Progress is tracked per target rather than per event, which is what lets a
/// failure in one destination leave the others free to advance. It is defined in
/// the domain because the name is persisted: it is the key of a delivery
/// checkpoint row, so it is part of the on-disk contract.
///
/// Renaming a variant would make already-recorded progress unreadable and cause
/// every past event to be redelivered, so the names are load-bearing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryTarget {
    /// The materialized feed and its ranking features.
    Timeline,
    /// The feed and post caches.
    Cache,
    /// In-process notification fan-out.
    Internal,
    /// The external event stream.
    External,
}

impl DeliveryTarget {
    /// Every target, in the order they are attempted.
    pub const ALL: [DeliveryTarget; 4] = [
        DeliveryTarget::Timeline,
        DeliveryTarget::Cache,
        DeliveryTarget::Internal,
        DeliveryTarget::External,
    ];

    /// The stable name stored in the delivery checkpoint table.
    pub fn as_str(self) -> &'static str {
        match self {
            DeliveryTarget::Timeline => "TIMELINE",
            DeliveryTarget::Cache => "CACHE",
            DeliveryTarget::Internal => "INTERNAL",
            DeliveryTarget::External => "EXTERNAL",
        }
    }
}

/// Where a target delivery attempt originated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryAttemptSource {
    OutboxWorker,
    AdminReplay,
}

impl DeliveryAttemptSource {
    pub fn as_str(self) -> &'static str {
        match self {
            DeliveryAttemptSource::OutboxWorker => "OUTBOX_WORKER",
            DeliveryAttemptSource::AdminReplay => "ADMIN_REPLAY",
        }
    }
}

/// Immutable lifecycle marker for one target attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryAttemptPhase {
    Started,
    Succeeded,
    Failed,
    /// Positive prefix progress, not completion of the requested target.
    Progressed,
}

impl DeliveryAttemptPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            DeliveryAttemptPhase::Started => "STARTED",
            DeliveryAttemptPhase::Succeeded => "SUCCEEDED",
            DeliveryAttemptPhase::Failed => "FAILED",
            DeliveryAttemptPhase::Progressed => "PROGRESSED",
        }
    }
}

/// Append-only metadata for one attempt to deliver an event to one target.
/// The record intentionally excludes payloads, actor data, and error text.
#[derive(Clone, PartialEq, Eq)]
pub struct DeliveryAttempt {
    pub attempt_id: Uuid,
    pub event_id: Uuid,
    pub target: DeliveryTarget,
    pub source: DeliveryAttemptSource,
    /// Present for worker attempts released by an audited dead-letter replay.
    pub resolution_id: Option<Uuid>,
    pub phase: DeliveryAttemptPhase,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RelationshipPolicySnapshot {
    pub blocked_by_viewer: bool,
    pub blocked_viewer: bool,
    pub muted_by_viewer: bool,
    pub following: bool,
}

impl RelationshipPolicySnapshot {
    pub fn is_blocked(self) -> bool {
        self.blocked_by_viewer || self.blocked_viewer
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentModerationTarget {
    Post,
    Comment,
}

impl ContentModerationTarget {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "POST" => Ok(Self::Post),
            "COMMENT" => Ok(Self::Comment),
            _ => anyhow::bail!("Content moderation target must be POST or COMMENT"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Post => "POST",
            Self::Comment => "COMMENT",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentModerationAction {
    Hide,
    Restore,
}

impl ContentModerationAction {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "HIDE" => Ok(Self::Hide),
            "RESTORE" => Ok(Self::Restore),
            _ => anyhow::bail!("Content moderation action must be HIDE or RESTORE"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hide => "HIDE",
            Self::Restore => "RESTORE",
        }
    }
}

macro_rules! redact_debug {
    ($($type:ty),+ $(,)?) => {
        $(
            impl std::fmt::Debug for $type {
                fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    formatter
                        .debug_struct(stringify!($type))
                        .field("details", &"[REDACTED]")
                        .finish()
                }
            }
        )+
    };
}

redact_debug!(
    Post,
    PostAccessSnapshot,
    PostDetail,
    PostLikeStatus,
    PostAuditEvent,
    NotificationRequest,
    ProfileSnapshot,
    StoredEvent,
    DeadLetterReplayResolution,
    RetentionPurgeResult,
    EventReplayFilter,
    DeliveryAttempt,
);

#[cfg(test)]
mod debug_redaction_tests {
    use super::*;

    #[test]
    fn content_and_event_debug_output_hides_user_data() {
        let sentinel = "private-post-content-7f91";
        let post = Post {
            id: Uuid::new_v4(),
            author_id: Uuid::new_v4(),
            content: sentinel.into(),
            visibility: "PRIVATE".into(),
            created_at: Utc::now(),
            comment_count: 3,
            like_count: 4,
        };
        let event = StoredEvent {
            event_id: Uuid::new_v4(),
            event_type: "POST_CREATED".into(),
            aggregate_id: post.id,
            aggregate_version: Some(1),
            origin_traceparent: None,
            actor_id: Some(post.author_id),
            attributes: vec!["private-attribute-94ab".into()],
            payload: sentinel.into(),
            occurred_at: Utc::now(),
            created_at: Utc::now(),
            published: false,
            published_at: None,
            replay_resolution_id: None,
        };

        for rendered in [format!("{post:?}"), format!("{event:?}")] {
            assert!(!rendered.contains(sentinel));
            assert!(!rendered.contains("private-attribute-94ab"));
            assert!(rendered.contains("[REDACTED]"));
        }
    }

    #[test]
    fn audit_debug_output_hides_request_metadata() {
        let event = PostAuditEvent {
            id: Uuid::new_v4(),
            actor_id: Some(Uuid::new_v4()),
            post_id: Some(Uuid::new_v4()),
            comment_id: None,
            action: "DELETE_POST".into(),
            outcome: "SUCCESS".into(),
            request_id: Some("private-request-82c1".into()),
            ip_address: Some("198.51.100.24".into()),
            user_agent: Some("private-agent-18f0".into()),
            metadata_json: r#"{"secret":"private-audit-meta-91e2"}"#.into(),
            occurred_at: Utc::now(),
        };

        let rendered = format!("{event:?}");
        for secret in [
            "private-request-82c1",
            "198.51.100.24",
            "private-agent-18f0",
            "private-audit-meta-91e2",
        ] {
            assert!(!rendered.contains(secret));
        }
    }

    #[test]
    fn debug_redaction_does_not_change_post_serialization() {
        let post = Post {
            id: Uuid::nil(),
            author_id: Uuid::nil(),
            content: "serializable-content".into(),
            visibility: "PUBLIC".into(),
            created_at: Utc::now(),
            comment_count: 0,
            like_count: 0,
        };
        let value = serde_json::to_value(post).unwrap();
        assert_eq!(value["content"], "serializable-content");
        assert_eq!(value["visibility"], "PUBLIC");
        assert!(value.get("author_id").is_some());
    }
}
