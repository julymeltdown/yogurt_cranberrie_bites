//! Notification-owned durable Concert inbox over inherited notification lifecycle.
//! No default production authority or actor-forwarding shortcut is installed.
pub mod authority;
pub mod delivery;
pub mod deletion_owner;
mod repository;
pub mod target_client;
pub mod transport;
pub mod wire;
pub use repository::PgConcertInbox;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::notification::{Candidate, Context};
use concert_domain::notification::*;
use sqlx::PgConnection;

/// Retained until the local transaction commits and the response is projected.
/// Production implementations must fence Auth/target revocation (or a qualified
/// owning-service snapshot protocol). A timestamp alone is not a mutation fence.
pub trait AuthorityFence: Send + Sync {
    fn check(&self, now: DateTime<Utc>) -> Result<()>;
}
pub struct Authorization {
    pub context: Context,
    pub checked_at: DateTime<Utc>,
    pub valid_until: DateTime<Utc>,
    /// Exact ordered candidate binding; omission/addition/reordering fails closed.
    pub decisions: Vec<(Candidate, Visibility)>,
    pub fence: Box<dyn AuthorityFence>,
}
impl Authorization {
    pub(crate) fn validate(
        &self,
        context: Context,
        candidates: &[Candidate],
        now: DateTime<Utc>,
    ) -> Result<()> {
        context.check(now)?;
        if self.context != context
            || self.checked_at > now
            || self.checked_at < context.observed_at
            || self.valid_until > context.expires_at
            || self.valid_until <= now
            || self.decisions.len() != candidates.len()
            || self
                .decisions
                .iter()
                .zip(candidates)
                .any(|((a, _), b)| a != b)
        {
            return Err(NotificationError::Unavailable);
        }
        self.fence.check(now)
    }
}
#[async_trait]
pub trait FencedInboxAuthority: Send + Sync {
    /// Actor/event/family authentication plus current membership, moderation,
    /// blocks, deletion and target authorization for the WHOLE supplied snapshot.
    /// Empty candidates still require canonical own-inbox authorization.
    async fn authorize_in(
        &self,
        connection: &mut PgConnection,
        context: Context,
        candidates: &[Candidate],
        now: DateTime<Utc>,
    ) -> Result<Authorization>;
}
pub struct UnavailableAuthority;
#[async_trait]
impl FencedInboxAuthority for UnavailableAuthority {
    async fn authorize_in(
        &self,
        _: &mut PgConnection,
        _: Context,
        _: &[Candidate],
        _: DateTime<Utc>,
    ) -> Result<Authorization> {
        Err(NotificationError::Unavailable)
    }
}
