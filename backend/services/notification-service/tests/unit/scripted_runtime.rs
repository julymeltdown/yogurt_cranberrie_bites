use chrono::{DateTime, Utc};
use notification_service::application::notification_runtime::NotificationRuntimePort;
use std::collections::VecDeque;
use std::sync::Mutex;
use uuid::Uuid;

pub enum RuntimeValue {
    Time(DateTime<Utc>),
    Id(Uuid),
}

pub struct ScriptedRuntime(Mutex<VecDeque<RuntimeValue>>);

impl ScriptedRuntime {
    pub fn new(values: impl IntoIterator<Item = RuntimeValue>) -> Self {
        Self(Mutex::new(values.into_iter().collect()))
    }

    pub fn assert_exhausted(&self) {
        assert!(self.0.lock().unwrap().is_empty(), "unused runtime values");
    }
}

impl NotificationRuntimePort for ScriptedRuntime {
    fn now(&self) -> DateTime<Utc> {
        match self.0.lock().unwrap().pop_front() {
            Some(RuntimeValue::Time(value)) => value,
            _ => panic!("unexpected clock call or runtime selection order"),
        }
    }

    fn new_id(&self) -> Uuid {
        match self.0.lock().unwrap().pop_front() {
            Some(RuntimeValue::Id(value)) => value,
            _ => panic!("unexpected identity call or runtime selection order"),
        }
    }
}
