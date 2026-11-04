//! Explicit owner ports; implementations authenticate callers and fence revocation.
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_domain::notification::*;
use uuid::Uuid;

/// Construct only at a verified server boundary, never from browser actor fields.
/// The authority port must bind this to the current Auth family and event policy.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Context {
    pub actor_id: Uuid,
    pub event_id: Uuid,
    pub session_family_id: Uuid,
    pub request_id: Uuid,
    pub authority_digest: [u8; 32],
    pub observed_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
impl Context {
    pub fn check(self, now: DateTime<Utc>) -> Result<()> {
        if [
            self.actor_id,
            self.event_id,
            self.session_family_id,
            self.request_id,
        ]
        .iter()
        .any(Uuid::is_nil)
            || self.authority_digest == [0; 32]
            || self.observed_at > now
            || self.expires_at <= now
            || self.expires_at <= self.observed_at
            || self.expires_at - self.observed_at > Duration::seconds(5)
        {
            return Err(NotificationError::PermissionDenied);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub notification_id: Uuid,
    pub source_actor_id: Option<Uuid>,
    pub target: Option<Target>,
}
#[derive(Clone, Debug)]
pub struct Query {
    pub limit: u16,
    pub unread_only: bool,
    pub cursor: Option<String>,
}
impl Default for Query {
    fn default() -> Self {
        Self {
            limit: 20,
            unread_only: false,
            cursor: None,
        }
    }
}
#[async_trait]
pub trait DurableNotificationOwner: Send + Sync {
    /// Success means a durable receipt plus all local effects are committed.
    /// Workload authentication and source-owner verification are mandatory.
    async fn deliver(&self, envelope: Envelope, recipients: Vec<Recipient>) -> Result<()>;
}
#[async_trait]
pub trait NotificationProjectionOwner: Send + Sync {
    /// Resolve metadata from the authoritative committed receipt/resource, not
    /// untrusted event payload. Empty means this operation creates no inbox item.
    /// Repeated delivery may resolve changed recipients; the owner receipt freezes
    /// the first committed fanout. Missing/unavailable authority must retry.
    async fn resolve(&self, envelope: Envelope) -> Result<Vec<Recipient>>;
}
