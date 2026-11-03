use api_gateway::application::ports::GatewayRuntimePort;
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use uuid::Uuid;

pub struct ScriptedRuntime {
    pub wall: Mutex<DateTime<Utc>>,
    pub id: Uuid,
    pub utc_reads: AtomicUsize,
    pub monotonic_reads: AtomicUsize,
    pub id_reads: AtomicUsize,
    monotonic: Mutex<VecDeque<Instant>>,
}

impl ScriptedRuntime {
    pub fn new() -> Arc<Self> {
        Self::with_monotonic(vec![Instant::now()])
    }

    pub fn with_monotonic(values: Vec<Instant>) -> Arc<Self> {
        assert!(!values.is_empty());
        Arc::new(Self {
            wall: Mutex::new(
                DateTime::parse_from_rfc3339("2030-02-03T04:05:06.123456789Z")
                    .unwrap()
                    .with_timezone(&Utc),
            ),
            id: Uuid::from_u128(123),
            utc_reads: AtomicUsize::new(0),
            monotonic_reads: AtomicUsize::new(0),
            id_reads: AtomicUsize::new(0),
            monotonic: Mutex::new(values.into()),
        })
    }
}

impl GatewayRuntimePort for ScriptedRuntime {
    fn utc_now(&self) -> DateTime<Utc> {
        self.utc_reads.fetch_add(1, Ordering::SeqCst);
        *self.wall.lock().unwrap()
    }

    fn monotonic_now(&self) -> Instant {
        self.monotonic_reads.fetch_add(1, Ordering::SeqCst);
        let mut values = self.monotonic.lock().unwrap();
        if values.len() > 1 {
            values.pop_front().unwrap()
        } else {
            *values.front().unwrap()
        }
    }

    fn new_id(&self) -> Uuid {
        self.id_reads.fetch_add(1, Ordering::SeqCst);
        self.id
    }
}

#[test]
fn search_cursor_public_entrypoints_use_injected_time_for_expiry_and_legacy_bounds() {
    use api_gateway::application::search_cursor::{SearchCursorCodec, SearchSort};
    use chrono::Duration;
    let runtime = ScriptedRuntime::new();
    let now = *runtime.wall.lock().unwrap();
    let key: Arc<[u8]> = Arc::from(b"runtime-cursor-key-with-at-least-thirty-two-bytes".as_slice());
    assert!(SearchCursorCodec::new(
        key.clone(),
        60,
        Some(now + Duration::days(8)),
        runtime.clone()
    )
    .is_err());
    let codec =
        SearchCursorCodec::new(key, 60, Some(now + Duration::days(7)), runtime.clone()).unwrap();
    let sort = SearchSort::ProfilesUsernameAscV1;
    let signed = codec
        .encode(Some("hana"), "hana", Some("viewer"), sort)
        .unwrap()
        .unwrap();
    assert_eq!(
        codec
            .decode(Some(&signed), "hana", Some("viewer"), sort)
            .unwrap(),
        Some("hana".into())
    );
    *runtime.wall.lock().unwrap() = now + Duration::seconds(61);
    assert!(codec
        .decode(Some(&signed), "hana", Some("viewer"), sort)
        .is_err());
    assert_eq!(
        codec
            .decode(Some("hana"), "hana", Some("viewer"), sort)
            .unwrap(),
        Some("hana".into())
    );
    *runtime.wall.lock().unwrap() = now + Duration::days(7) + Duration::seconds(1);
    assert!(codec
        .decode(Some("hana"), "hana", Some("viewer"), sort)
        .is_err());
    assert_eq!(runtime.utc_reads.load(Ordering::SeqCst), 7);
    assert_eq!(runtime.monotonic_reads.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.id_reads.load(Ordering::SeqCst), 0);
}
