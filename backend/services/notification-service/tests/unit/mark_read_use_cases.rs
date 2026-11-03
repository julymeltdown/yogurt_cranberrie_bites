use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use notification_service::application::dto::repository_dto::{
    NotificationAuditRecordDto as RepositoryAudit, NotificationPageRecordDto as RepositoryPage,
    NotificationRecordDto as RepositoryNotification,
};
use notification_service::application::ports::NotificationRepositoryPort;
use notification_service::application::use_cases::mark_all_read::MarkAllReadUseCase;
use notification_service::application::use_cases::mark_read::MarkReadUseCase;
use notification_service::domain::notification::Notification;
use notification_service::domain::notification_page::NotificationPage;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[tokio::test]
async fn unread_mark_read_uses_one_atomic_repository_mutation() {
    let user_id = uuid("00000000-0000-0000-0000-000000000001");
    let notification_id = uuid("00000000-0000-0000-0000-000000000101");
    let repository = Arc::new(FakeNotificationRepository::with_notification(notification(
        user_id,
        notification_id,
        None,
    )));
    let use_case = MarkReadUseCase::new(
        Arc::clone(&repository),
        std::sync::Arc::new(
            notification_service::adapters::out::runtime::SystemNotificationRuntime,
        ),
    );

    let response = use_case
        .execute(&user_id.to_string(), &notification_id.to_string())
        .await
        .expect("mark unread notification");

    assert!(response.updated);
    assert_eq!(response.notification.id, notification_id.to_string());
    let mark_read_calls = repository.mark_read_calls.lock().unwrap();
    assert_eq!(mark_read_calls.len(), 1);
    assert_eq!(mark_read_calls[0].user_id, user_id);
    assert_eq!(mark_read_calls[0].notification_id, notification_id);
    assert_eq!(
        response.notification.read_at,
        mark_read_calls[0].read_at.to_rfc3339()
    );
    assert!(repository.find_by_id_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn already_read_mark_read_returns_existing_notification_without_persisting() {
    let user_id = uuid("00000000-0000-0000-0000-000000000002");
    let notification_id = uuid("00000000-0000-0000-0000-000000000102");
    let original_read_at = instant(1_800);
    let repository = Arc::new(FakeNotificationRepository::with_notification(notification(
        user_id,
        notification_id,
        Some(original_read_at),
    )));
    let use_case = MarkReadUseCase::new(
        Arc::clone(&repository),
        std::sync::Arc::new(
            notification_service::adapters::out::runtime::SystemNotificationRuntime,
        ),
    );

    let response = use_case
        .execute(&user_id.to_string(), &notification_id.to_string())
        .await
        .expect("mark already-read notification");

    assert!(!response.updated);
    assert_eq!(response.notification.read_at, original_read_at.to_rfc3339());
    assert_eq!(repository.mark_read_calls.lock().unwrap().len(), 1);
    assert!(repository.find_by_id_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn missing_notification_errors_without_mark_read_mutation() {
    let user_id = uuid("00000000-0000-0000-0000-000000000003");
    let notification_id = uuid("00000000-0000-0000-0000-000000000103");
    let repository = Arc::new(FakeNotificationRepository::default());
    let use_case = MarkReadUseCase::new(
        Arc::clone(&repository),
        std::sync::Arc::new(
            notification_service::adapters::out::runtime::SystemNotificationRuntime,
        ),
    );

    let error = use_case
        .execute(&user_id.to_string(), &notification_id.to_string())
        .await
        .expect_err("missing notification should fail");

    assert_eq!(error.to_string(), "Notification not found");
    assert_eq!(repository.mark_read_calls.lock().unwrap().len(), 1);
    assert!(repository.find_by_id_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn mark_all_read_rejects_blank_and_malformed_user_ids() {
    let repository = Arc::new(FakeNotificationRepository::default());
    let use_case = MarkAllReadUseCase::new(
        Arc::clone(&repository),
        std::sync::Arc::new(
            notification_service::adapters::out::runtime::SystemNotificationRuntime,
        ),
    );

    for user_id in ["", "   ", "not-a-uuid"] {
        let error = use_case
            .execute(user_id)
            .await
            .expect_err("invalid user id should fail");

        assert_eq!(error.to_string(), "User ID is required");
    }
    assert!(repository.mark_all_read_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn mark_all_read_returns_repository_count_for_valid_user() {
    let user_id = uuid("00000000-0000-0000-0000-000000000004");
    let repository = Arc::new(FakeNotificationRepository::with_mark_all_read_count(3));
    let use_case = MarkAllReadUseCase::new(
        Arc::clone(&repository),
        std::sync::Arc::new(
            notification_service::adapters::out::runtime::SystemNotificationRuntime,
        ),
    );

    let count = use_case
        .execute(&user_id.to_string())
        .await
        .expect("mark all read");

    assert_eq!(count, 3);
    let calls = repository.mark_all_read_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].user_id, user_id);
}

#[derive(Default)]
struct FakeNotificationRepository {
    notification: Mutex<Option<Notification>>,
    find_by_id_calls: Mutex<Vec<FindByIdCall>>,
    mark_read_calls: Mutex<Vec<MarkReadCall>>,
    mark_all_read_calls: Mutex<Vec<MarkAllReadCall>>,
    mark_all_read_count: Mutex<i64>,
}

impl FakeNotificationRepository {
    fn with_notification(notification: Notification) -> Self {
        let repository = Self::default();
        *repository.notification.lock().unwrap() = Some(notification);
        repository
    }

    fn with_mark_all_read_count(count: i64) -> Self {
        let repository = Self::default();
        *repository.mark_all_read_count.lock().unwrap() = count;
        repository
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FindByIdCall {
    user_id: Uuid,
    notification_id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MarkReadCall {
    user_id: Uuid,
    notification_id: Uuid,
    read_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MarkAllReadCall {
    user_id: Uuid,
    read_at: DateTime<Utc>,
}

#[async_trait]
impl NotificationRepositoryPort for FakeNotificationRepository {
    async fn save(
        &self,
        notification: &RepositoryNotification,
    ) -> notification_service::application::errors::NotificationResult<RepositoryNotification> {
        let notification = &Notification::from(notification);
        let result: notification_service::application::errors::NotificationResult<Notification> =
            async { Ok(notification.clone()) }.await;
        result.map(|value| value.into())
    }

    async fn find_by_event_id_and_type(
        &self,
        _user_id: Uuid,
        _event_id: Uuid,
        _notification_type: &str,
    ) -> notification_service::application::errors::NotificationResult<Option<RepositoryNotification>>
    {
        let result: notification_service::application::errors::NotificationResult<
            Option<Notification>,
        > = async { unimplemented!("not needed by mark read use case tests") }.await;
        result.map(|value| value.map(Into::into))
    }

    async fn find_by_id(
        &self,
        user_id: Uuid,
        notification_id: Uuid,
    ) -> notification_service::application::errors::NotificationResult<Option<RepositoryNotification>>
    {
        let result: notification_service::application::errors::NotificationResult<
            Option<Notification>,
        > = async {
            self.find_by_id_calls.lock().unwrap().push(FindByIdCall {
                user_id,
                notification_id,
            });
            Ok(self.notification.lock().unwrap().clone())
        }
        .await;
        result.map(|value| value.map(Into::into))
    }

    async fn list(
        &self,
        _user_id: Uuid,
        _cursor: Option<(i64, Uuid)>,
        _limit: i32,
    ) -> notification_service::application::errors::NotificationResult<RepositoryPage> {
        let result: notification_service::application::errors::NotificationResult<
            NotificationPage,
        > = async { unimplemented!("not needed by mark read use case tests") }.await;
        result.map(|value| value.into())
    }

    async fn mark_read(
        &self,
        user_id: Uuid,
        notification_id: Uuid,
        read_at: DateTime<Utc>,
    ) -> notification_service::application::errors::NotificationResult<
        Option<(RepositoryNotification, bool)>,
    > {
        let result: notification_service::application::errors::NotificationResult<
            Option<(Notification, bool)>,
        > = async {
            self.mark_read_calls.lock().unwrap().push(MarkReadCall {
                user_id,
                notification_id,
                read_at,
            });
            let mut stored = self.notification.lock().unwrap();
            let Some(notification) = stored.as_ref() else {
                return Ok(None);
            };
            if notification.user_id != user_id || notification.id != notification_id {
                return Ok(None);
            }
            let (marked, updated) = notification.clone().mark_read(read_at);
            *stored = Some(marked.clone());
            Ok(Some((marked, updated)))
        }
        .await;
        result.map(|value| value.map(|(notification, updated)| (notification.into(), updated)))
    }

    async fn mark_all_read(
        &self,
        user_id: Uuid,
        read_at: DateTime<Utc>,
    ) -> notification_service::application::errors::NotificationResult<i64> {
        self.mark_all_read_calls
            .lock()
            .unwrap()
            .push(MarkAllReadCall { user_id, read_at });
        Ok(*self.mark_all_read_count.lock().unwrap())
    }

    async fn soft_delete_with_audit(
        &self,
        _user_id: Uuid,
        _notification_id: Uuid,
        _actor_id: Uuid,
        _reason: &str,
        _retention_until: DateTime<Utc>,
        _audit_event: &RepositoryAudit,
    ) -> notification_service::application::errors::NotificationResult<bool> {
        unimplemented!("not needed by mark read use case tests")
    }

    async fn record_audit_event(
        &self,
        _event: &RepositoryAudit,
    ) -> notification_service::application::errors::NotificationResult<()> {
        unimplemented!("not needed by mark read use case tests")
    }

    async fn purge_expired_notifications(
        &self,
        _cutoff: DateTime<Utc>,
        _actor_id: Uuid,
        _request_id: Option<String>,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
        _dry_run: bool,
    ) -> notification_service::application::errors::NotificationResult<Vec<Uuid>> {
        unimplemented!("not needed by mark read use case tests")
    }
    // Explicit recording capabilities; these do not establish PG atomicity.
    async fn is_recipient_suppressed(
        &self,
        _user_id: Uuid,
    ) -> notification_service::application::errors::NotificationResult<bool> {
        Err(notification_service::application::errors::NotificationError::Unexpected)
    }

    async fn is_actor_suppressed(
        &self,
        _user_id: Uuid,
    ) -> notification_service::application::errors::NotificationResult<bool> {
        Err(notification_service::application::errors::NotificationError::Unexpected)
    }

    async fn create_consistent(
        &self,
        _notification: &RepositoryNotification,
    ) -> notification_service::application::errors::NotificationResult<
        notification_service::application::notification_contracts::NotificationCreateOutcome,
    > {
        Err(notification_service::application::errors::NotificationError::Unexpected)
    }

    async fn list_consistent(
        &self,
        _user_id: Uuid,
        _cursor: Option<
            notification_service::application::notification_contracts::NotificationCursor,
        >,
        _limit: i32,
    ) -> notification_service::application::errors::NotificationResult<
        notification_service::application::notification_contracts::NotificationQueryPage,
    > {
        Err(notification_service::application::errors::NotificationError::Unexpected)
    }

    async fn purge_expired_batch_consistent(
        &self,
        _cutoff: DateTime<Utc>,
        _actor_id: Uuid,
        _request_id: Option<String>,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
        _dry_run: bool,
        _limit: i32,
    ) -> notification_service::application::errors::NotificationResult<Vec<Uuid>> {
        Err(notification_service::application::errors::NotificationError::Unexpected)
    }

    async fn apply_user_deletion(
        &self,
        _saga_id: Uuid,
        _user_id: Uuid,
        _actor_id: Uuid,
        _retention_until: DateTime<Utc>,
    ) -> notification_service::application::errors::NotificationResult<()> {
        Err(notification_service::application::errors::NotificationError::Unexpected)
    }
}

fn notification(
    user_id: Uuid,
    notification_id: Uuid,
    read_at: Option<DateTime<Utc>>,
) -> Notification {
    Notification {
        id: notification_id,
        user_id,
        notification_type: "POST_LIKED".into(),
        payload_json: "{}".into(),
        created_at: instant(1_000),
        read_at,
        event_id: uuid("00000000-0000-0000-0000-000000000201"),
        actor_id: Some(uuid("00000000-0000-0000-0000-000000000301")),
        occurred_at: instant(900),
    }
}

fn instant(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(seconds, 0).single().unwrap()
}

fn uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).unwrap()
}

#[tokio::test]
async fn mark_read_passes_exact_nanosecond_runtime_time() {
    use super::scripted_runtime::{RuntimeValue, ScriptedRuntime};
    let user = Uuid::from_u128(61);
    let id = Uuid::from_u128(62);
    let time = Utc.timestamp_opt(8_000, 123_987_456).unwrap();
    let runtime = Arc::new(ScriptedRuntime::new([RuntimeValue::Time(time)]));
    let repository = Arc::new(FakeNotificationRepository::with_notification(notification(
        user, id, None,
    )));
    let response = MarkReadUseCase::new(repository.clone(), runtime.clone())
        .execute(&user.to_string(), &id.to_string())
        .await
        .unwrap();
    assert!(response.updated);
    let calls = repository.mark_read_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].read_at, time);
    assert_eq!(response.notification.read_at, time.to_rfc3339());
    runtime.assert_exhausted();
}

#[tokio::test]
async fn mark_all_read_passes_exact_runtime_time() {
    use super::scripted_runtime::{RuntimeValue, ScriptedRuntime};
    let user = Uuid::from_u128(71);
    let time = Utc.timestamp_opt(9_000, 987_123_654).unwrap();
    let runtime = Arc::new(ScriptedRuntime::new([RuntimeValue::Time(time)]));
    let repository = Arc::new(FakeNotificationRepository::with_mark_all_read_count(3));
    assert_eq!(
        MarkAllReadUseCase::new(repository.clone(), runtime.clone())
            .execute(&user.to_string())
            .await
            .unwrap(),
        3
    );
    let calls = repository.mark_all_read_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].read_at, time);
    runtime.assert_exhausted();
}

#[tokio::test]
async fn invalid_mark_requests_do_not_spend_clock_or_mutate() {
    use super::scripted_runtime::ScriptedRuntime;
    let runtime = Arc::new(ScriptedRuntime::new([]));
    let repository = Arc::new(FakeNotificationRepository::default());
    let mark = MarkReadUseCase::new(repository.clone(), runtime.clone());
    let all = MarkAllReadUseCase::new(repository.clone(), runtime.clone());
    for invalid in ["", " ", "invalid"] {
        assert!(mark
            .execute(invalid, &Uuid::from_u128(81).to_string())
            .await
            .is_err());
        assert!(mark
            .execute(&Uuid::from_u128(82).to_string(), invalid)
            .await
            .is_err());
        assert!(all.execute(invalid).await.is_err());
    }
    assert!(repository.mark_read_calls.lock().unwrap().is_empty());
    assert!(repository.mark_all_read_calls.lock().unwrap().is_empty());
    runtime.assert_exhausted();
}
