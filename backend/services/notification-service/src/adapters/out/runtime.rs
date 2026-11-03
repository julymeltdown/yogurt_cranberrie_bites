use crate::application::notification_runtime::NotificationRuntimePort;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Production clock and UUID generation, selected explicitly by composition.
pub struct SystemNotificationRuntime;

impl NotificationRuntimePort for SystemNotificationRuntime {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn new_id(&self) -> Uuid {
        Uuid::new_v4()
    }
}
