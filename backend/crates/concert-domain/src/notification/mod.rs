//! Safe notification values. No arbitrary summary, location, handle or payload field.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NotificationError {
    #[error("Invalid input")]
    InvalidInput,
    #[error("Resource unavailable")]
    ResourceUnavailable,
    #[error("Permission denied")]
    PermissionDenied,
    #[error("Required dependency unavailable")]
    Unavailable,
    #[error("Version conflict")]
    VersionConflict,
    #[error("Idempotency conflict")]
    IdempotencyConflict,
    #[error("Idempotency expired")]
    IdempotencyExpired,
}
pub type Result<T> = std::result::Result<T, NotificationError>;

macro_rules! tags {
    ($name:ident { $($tag:ident => $text:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        pub enum $name { $($tag),+ }
        impl $name {
            pub fn as_str(self) -> &'static str { match self { $(Self::$tag => $text),+ } }
            pub fn parse(value: &str) -> Result<Self> { match value { $($text => Ok(Self::$tag)),+, _ => Err(NotificationError::InvalidInput) } }
        }
    }
}
tags!(Kind { Party => "PARTY", Giveaway => "GIVEAWAY", Social => "SOCIAL", Moderation => "MODERATION", Notice => "NOTICE", Feed => "FEED" });
tags!(TargetType { Party => "PARTY", Application => "APPLICATION", Giveaway => "GIVEAWAY", Reservation => "RESERVATION", SocialRequest => "SOCIAL_REQUEST", Post => "POST", Notice => "NOTICE", Report => "REPORT" });

impl Kind {
    /// Fixed neutral labels only, never interpolated source text. C04 DTO uses kind.
    pub fn safe_summary(self) -> &'static str {
        match self {
            Self::Party => "Party activity updated",
            Self::Giveaway => "Giveaway activity updated",
            Self::Social => "Sharing request updated",
            Self::Moderation => "Moderation update",
            Self::Notice => "Event notice updated",
            Self::Feed => "Feed activity updated",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub target_type: TargetType,
    pub target_id: Uuid,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notification {
    pub notification_id: Uuid,
    pub event_id: Uuid,
    pub kind: Kind,
    pub target: Option<Target>,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
    pub revision: i64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationList {
    pub items: Vec<Notification>,
    pub next_cursor: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnreadCount {
    pub actor_id: Uuid,
    pub event_id: Uuid,
    pub unread_total: i64,
    pub as_of: DateTime<Utc>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadAllResult {
    pub read_count: i32,
    pub read_at: DateTime<Utc>,
}
/// Trusted BE15 envelope. attempt/lease are intentionally outside dedupe identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub outbox_id: Uuid,
    pub receipt_id: Uuid,
    pub event_id: Uuid,
    pub resource_id: Uuid,
    pub resource_revision: i64,
}
impl Envelope {
    pub fn validate(self) -> Result<()> {
        if [
            self.outbox_id,
            self.receipt_id,
            self.event_id,
            self.resource_id,
        ]
        .iter()
        .any(Uuid::is_nil)
            || !(1..=crate::MAX_REVISION).contains(&self.resource_revision)
        {
            return Err(NotificationError::InvalidInput);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipient {
    pub user_id: Uuid,
    pub source_actor_id: Option<Uuid>,
    pub kind: Kind,
    pub target: Option<Target>,
}
impl Recipient {
    pub fn validate(self) -> Result<()> {
        if self.user_id.is_nil()
            || self.source_actor_id.is_some_and(|v| v.is_nil())
            || self.target.is_some_and(|v| v.target_id.is_nil())
        {
            return Err(NotificationError::InvalidInput);
        }
        Ok(())
    }
}
/// Hidden records disclose neither notification existence nor target. Stale is
/// allowed only when owner policy explicitly permits the neutral inbox record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Visible,
    Stale,
    Hidden,
}
