//! Explicit nondeterministic values used by Notification application decisions.
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Required clock and identity capability; implementations belong outside core.
pub trait NotificationRuntimePort: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
    fn new_id(&self) -> Uuid;
}
