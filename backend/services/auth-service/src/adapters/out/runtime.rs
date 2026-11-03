use crate::application::auth_runtime::AuthRuntimePort;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, Copy, Default)]
pub struct SystemAuthRuntime;

impl AuthRuntimePort for SystemAuthRuntime {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn new_id(&self) -> Uuid {
        Uuid::new_v4()
    }
}
