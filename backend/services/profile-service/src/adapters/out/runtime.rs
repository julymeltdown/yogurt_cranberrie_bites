use crate::application::profile_runtime::ProfileRuntimePort;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, Copy, Default)]
pub struct SystemProfileRuntime;

impl ProfileRuntimePort for SystemProfileRuntime {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
    fn new_id(&self) -> Uuid {
        Uuid::new_v4()
    }
}
