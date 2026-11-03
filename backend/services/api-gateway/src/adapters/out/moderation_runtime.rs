use crate::application::ports::ModerationRuntimePort;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// System values for moderation application decisions, composed by the adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemModerationRuntime;

impl ModerationRuntimePort for SystemModerationRuntime {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn new_id(&self) -> Uuid {
        Uuid::new_v4()
    }
}
