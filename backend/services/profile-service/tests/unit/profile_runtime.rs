use chrono::{DateTime, Duration, Utc};
use profile_service::application::profile_runtime::ProfileRuntimePort;
use std::sync::atomic::{AtomicUsize, Ordering};
use uuid::Uuid;

#[derive(Default)]
pub(crate) struct AdvancingProfileRuntime {
    pub(crate) time_calls: AtomicUsize,
    pub(crate) id_calls: AtomicUsize,
}

pub(crate) fn instant() -> DateTime<Utc> {
    DateTime::from_timestamp(1_700_000_000, 0).expect("fixture instant")
}

impl ProfileRuntimePort for AdvancingProfileRuntime {
    fn now(&self) -> DateTime<Utc> {
        instant() + Duration::seconds(self.time_calls.fetch_add(1, Ordering::SeqCst) as i64)
    }
    fn new_id(&self) -> Uuid {
        Uuid::from_u128(0xabc0 + self.id_calls.fetch_add(1, Ordering::SeqCst) as u128)
    }
}
