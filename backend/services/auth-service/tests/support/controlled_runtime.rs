use auth_service::application::auth_runtime::AuthRuntimePort;
use chrono::{DateTime, Duration, Utc};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};
use uuid::Uuid;

pub struct ControlledRuntime {
    now: Mutex<DateTime<Utc>>,
    ids: AtomicU64,
}

impl ControlledRuntime {
    pub fn new(now: DateTime<Utc>) -> Self {
        Self {
            now: Mutex::new(now),
            ids: AtomicU64::new(0),
        }
    }

    pub fn advance(&self, by: Duration) {
        *self.now.lock().unwrap() += by;
    }

    pub fn issued_ids(&self) -> u64 {
        self.ids.load(Ordering::SeqCst)
    }
}

impl AuthRuntimePort for ControlledRuntime {
    fn now(&self) -> DateTime<Utc> {
        *self.now.lock().unwrap()
    }
    fn new_id(&self) -> Uuid {
        Uuid::from_u128(
            0x23450000000000000000000000000000
                + u128::from(self.ids.fetch_add(1, Ordering::SeqCst)),
        )
    }
}
