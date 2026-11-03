//! Explicit nondeterministic capabilities for Profile application decisions.
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub trait ProfileRuntimePort: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
    fn new_id(&self) -> Uuid;
}
