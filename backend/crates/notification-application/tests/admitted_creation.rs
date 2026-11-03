use async_trait::async_trait;
use chrono::{DateTime, Utc};
use notification_application::{
    dto::{notification_dto::NotificationCreateDto, repository_dto::NotificationRecordDto},
    errors::{NotificationError, NotificationResult},
    notification_contracts::NotificationCreateOutcome,
    notification_creation::{
        NotificationAdmittedCreatePort, NotificationCreatePort, NotificationWriteDeadlineDto,
    },
    notification_runtime::NotificationRuntimePort,
    use_cases::create_notification::CreateNotificationUseCase,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use uuid::Uuid;

struct Runtime {
    now: DateTime<Utc>,
    ids: AtomicUsize,
}
impl NotificationRuntimePort for Runtime {
    fn now(&self) -> DateTime<Utc> {
        self.now
    }
    fn new_id(&self) -> Uuid {
        Uuid::from_u128((self.ids.fetch_add(1, Ordering::SeqCst) + 1) as u128)
    }
}
#[derive(Clone)]
struct Recording {
    legacy: Arc<AtomicUsize>,
    selected: Arc<Mutex<Option<(NotificationRecordDto, NotificationWriteDeadlineDto)>>>,
    failure: Option<NotificationError>,
}
#[async_trait]
impl NotificationCreatePort for Recording {
    async fn create_unless_suppressed(
        &self,
        _: &NotificationRecordDto,
    ) -> NotificationResult<NotificationCreateOutcome> {
        self.legacy.fetch_add(1, Ordering::SeqCst);
        Err(NotificationError::Unexpected)
    }
}
#[async_trait]
impl NotificationAdmittedCreatePort for Recording {
    async fn create_admitted(
        &self,
        n: &NotificationRecordDto,
        d: NotificationWriteDeadlineDto,
    ) -> NotificationResult<NotificationCreateOutcome> {
        *self.selected.lock().unwrap() = Some((n.clone(), d));
        if let Some(e) = self.failure {
            return Err(e);
        }
        Ok(NotificationCreateOutcome::Created(n.clone()))
    }
}
fn complete<T>(f: impl std::future::Future<Output = T>) -> T {
    let mut f = std::pin::pin!(f);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match f.as_mut().poll(&mut cx) {
        std::task::Poll::Ready(r) => r,
        _ => panic!("unit recording capability must complete synchronously"),
    }
}
fn setup(failure: Option<NotificationError>) -> (Recording, Arc<Runtime>, NotificationCreateDto) {
    (
        Recording {
            legacy: Arc::new(AtomicUsize::new(0)),
            selected: Arc::new(Mutex::new(None)),
            failure,
        },
        Arc::new(Runtime {
            now: DateTime::from_timestamp_micros(1_000_000).unwrap(),
            ids: AtomicUsize::new(0),
        }),
        NotificationCreateDto {
            user_id: Uuid::new_v4().to_string(),
            r#type: "POST_LIKED".into(),
            payload_json: "{\"payloadVersion\":1}".into(),
            event_id: Uuid::new_v4().to_string(),
            actor_id: Uuid::new_v4().to_string(),
            occurred_at: "2026-10-14T00:00:00+00:00".into(),
        },
    )
}
#[test]
fn expired_admission_never_allocates_an_id_or_calls_either_repository_capability() {
    for expiry in [999_999, 1_000_000] {
        let (repo, runtime, request) = setup(None);
        let use_case = CreateNotificationUseCase::new(repo.clone(), runtime.clone());
        assert_eq!(
            complete(use_case.execute_admitted(
                request,
                NotificationWriteDeadlineDto {
                    expires_at: DateTime::from_timestamp_micros(expiry).unwrap()
                }
            ))
            .err(),
            Some(NotificationError::AdmissionExpired)
        );
        assert_eq!(runtime.ids.load(Ordering::SeqCst), 0);
        assert_eq!(repo.legacy.load(Ordering::SeqCst), 0);
        assert!(repo.selected.lock().unwrap().is_none());
    }
}
#[test]
fn live_admitted_creation_preserves_exact_deadline_and_identity_without_legacy_fallback() {
    let (repo, runtime, request) = setup(None);
    let use_case = CreateNotificationUseCase::new(repo.clone(), runtime);
    let expires_at = DateTime::from_timestamp_micros(1_000_001).unwrap();
    let original = request.clone();
    let result =
        complete(use_case.execute_admitted(request, NotificationWriteDeadlineDto { expires_at }))
            .unwrap();
    assert!(!result.suppressed);
    let selected = repo.selected.lock().unwrap();
    let (stored, deadline) = selected.as_ref().unwrap();
    assert_eq!(deadline.expires_at, expires_at);
    assert_eq!(stored.event_id.to_string(), original.event_id);
    assert_eq!(stored.payload_json, original.payload_json);
    assert_eq!(stored.actor_id.unwrap().to_string(), original.actor_id);
    assert_eq!(repo.legacy.load(Ordering::SeqCst), 0);
}
#[test]
fn expired_or_uncertain_persistence_never_becomes_successful_suppression() {
    for error in [
        NotificationError::AdmissionExpired,
        NotificationError::DependencyUnavailable,
    ] {
        let (repo, runtime, request) = setup(Some(error));
        let use_case = CreateNotificationUseCase::new(repo.clone(), runtime);
        assert_eq!(
            complete(use_case.execute_admitted(
                request,
                NotificationWriteDeadlineDto {
                    expires_at: DateTime::from_timestamp_micros(2_000_000).unwrap()
                }
            ))
            .err(),
            Some(error)
        );
        assert!(repo.selected.lock().unwrap().is_some());
        assert_eq!(repo.legacy.load(Ordering::SeqCst), 0);
    }
}
