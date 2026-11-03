use async_trait::async_trait;
use chrono::{DateTime, Utc};
use notification_service::adapters::r#in::messaging::retention_purge_worker::NotificationRetentionPurgeWorker;
use notification_service::application::dto::notification_dto::PurgeExpiredNotificationsDto;
use notification_service::application::dto::repository_dto::{
    NotificationAuditRecordDto as RepositoryAudit, NotificationPageRecordDto as RepositoryPage,
    NotificationRecordDto as RepositoryNotification,
};
use notification_service::application::ports::NotificationRepositoryPort;
use notification_service::application::use_cases::purge_expired_notifications::PurgeExpiredNotificationsUseCase;
use notification_service::config::NotificationMetrics;
use notification_service::domain::notification::Notification;
use notification_service::domain::notification_page::NotificationPage;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[tokio::test]
async fn worker_tick_passes_system_metadata_and_records_success_metrics() {
    let actor_id = uuid("99999999-9999-9999-9999-999999999999");
    let repository = Arc::new(FakeNotificationRepository::with_purged_ids(vec![
        uuid("00000000-0000-0000-0000-000000000901"),
        uuid("00000000-0000-0000-0000-000000000902"),
    ]));
    let metrics = Arc::new(NotificationMetrics::default());
    let worker = worker(Arc::clone(&repository), actor_id, Arc::clone(&metrics));

    let purged_count = worker.tick().await.expect("worker tick");

    assert_eq!(purged_count, 2);
    assert_eq!(
        *repository.purge_calls.lock().expect("purge calls"),
        vec![PurgeCall {
            actor_id,
            request_id: Some("notification-retention-worker".to_owned()),
            ip_address: None,
            user_agent: Some("notification-retention-worker".to_owned()),
            dry_run: false,
        }]
    );
    let metrics = metrics.render_prometheus();
    assert!(metrics.contains("notification_retention_purge_runs_total{outcome=\"success\"} 1"));
    assert!(metrics
        .contains("notification_retention_purged_notifications_total{outcome=\"success\"} 2"));
}

#[tokio::test]
async fn worker_tick_records_failure_metrics_when_purge_use_case_fails() {
    let actor_id = uuid("99999999-9999-9999-9999-999999999999");
    let repository = Arc::new(FakeNotificationRepository::with_error(
        notification_service::application::errors::NotificationError::DependencyUnavailable,
    ));
    let metrics = Arc::new(NotificationMetrics::default());
    let worker = worker(Arc::clone(&repository), actor_id, Arc::clone(&metrics));

    let error = worker.tick().await.expect_err("worker tick should fail");

    assert_eq!(error.to_string(), "repository unavailable");
    assert_eq!(repository.purge_calls.lock().expect("purge calls").len(), 1);
    let metrics = metrics.render_prometheus();
    assert!(metrics.contains("notification_retention_purge_runs_total{outcome=\"failure\"} 1"));
    assert!(metrics
        .contains("notification_retention_purged_notifications_total{outcome=\"failure\"} 0"));
}

#[tokio::test]
async fn purge_use_case_rejects_blank_actor_without_repository_call() {
    let repository = Arc::new(FakeNotificationRepository::default());
    let use_case = PurgeExpiredNotificationsUseCase::new(
        Arc::clone(&repository),
        std::sync::Arc::new(
            notification_service::adapters::out::runtime::SystemNotificationRuntime,
        ),
    );

    let error = use_case
        .execute(PurgeExpiredNotificationsDto {
            actor_id: " ".to_owned(),
            request_id: Some("request".to_owned()),
            ip_address: None,
            user_agent: None,
            dry_run: false,
        })
        .await
        .expect_err("blank actor should fail");

    assert_eq!(error.to_string(), "Actor ID is required");
    assert!(repository
        .purge_calls
        .lock()
        .expect("purge calls")
        .is_empty());
}

#[derive(Default)]
struct FakeNotificationRepository {
    purged_ids: Mutex<Vec<Uuid>>,
    purge_error: Mutex<Option<notification_service::application::errors::NotificationError>>,
    purge_calls: Mutex<Vec<PurgeCall>>,
}

impl FakeNotificationRepository {
    fn with_purged_ids(purged_ids: Vec<Uuid>) -> Self {
        Self {
            purged_ids: Mutex::new(purged_ids),
            ..Self::default()
        }
    }

    fn with_error(message: notification_service::application::errors::NotificationError) -> Self {
        Self {
            purge_error: Mutex::new(Some(message)),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PurgeCall {
    actor_id: Uuid,
    request_id: Option<String>,
    ip_address: Option<String>,
    user_agent: Option<String>,
    dry_run: bool,
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
        > = async { unimplemented!("not needed by retention purge worker tests") }.await;
        result.map(|value| value.map(Into::into))
    }

    async fn find_by_id(
        &self,
        _user_id: Uuid,
        _notification_id: Uuid,
    ) -> notification_service::application::errors::NotificationResult<Option<RepositoryNotification>>
    {
        let result: notification_service::application::errors::NotificationResult<
            Option<Notification>,
        > = async { unimplemented!("not needed by retention purge worker tests") }.await;
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
        > = async { unimplemented!("not needed by retention purge worker tests") }.await;
        result.map(|value| value.into())
    }

    async fn mark_read(
        &self,
        _user_id: Uuid,
        _notification_id: Uuid,
        _read_at: DateTime<Utc>,
    ) -> notification_service::application::errors::NotificationResult<
        Option<(RepositoryNotification, bool)>,
    > {
        let result: notification_service::application::errors::NotificationResult<
            Option<(Notification, bool)>,
        > = async { unimplemented!("not needed by retention purge worker tests") }.await;
        result.map(|value| value.map(|(notification, updated)| (notification.into(), updated)))
    }

    async fn mark_all_read(
        &self,
        _user_id: Uuid,
        _read_at: DateTime<Utc>,
    ) -> notification_service::application::errors::NotificationResult<i64> {
        unimplemented!("not needed by retention purge worker tests")
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
        unimplemented!("not needed by retention purge worker tests")
    }

    async fn record_audit_event(
        &self,
        _event: &RepositoryAudit,
    ) -> notification_service::application::errors::NotificationResult<()> {
        unimplemented!("not needed by retention purge worker tests")
    }

    async fn purge_expired_notifications(
        &self,
        _cutoff: DateTime<Utc>,
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
    ) -> notification_service::application::errors::NotificationResult<Vec<Uuid>> {
        self.purge_calls
            .lock()
            .expect("purge calls")
            .push(PurgeCall {
                actor_id,
                request_id,
                ip_address,
                user_agent,
                dry_run,
            });
        if let Some(message) = *self.purge_error.lock().expect("purge error") {
            return Err(message);
        }
        Ok(self.purged_ids.lock().expect("purged ids").clone())
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
        cutoff: DateTime<Utc>,
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
        limit: i32,
    ) -> notification_service::application::errors::NotificationResult<Vec<Uuid>> {
        let mut values = self
            .purge_expired_notifications(
                cutoff, actor_id, request_id, ip_address, user_agent, dry_run,
            )
            .await?;
        values.truncate(limit.max(1) as usize);
        Ok(values)
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

fn worker(
    repository: Arc<FakeNotificationRepository>,
    actor_id: Uuid,
    metrics: Arc<NotificationMetrics>,
) -> NotificationRetentionPurgeWorker<Arc<FakeNotificationRepository>> {
    NotificationRetentionPurgeWorker::new(
        PurgeExpiredNotificationsUseCase::new(
            repository,
            std::sync::Arc::new(
                notification_service::adapters::out::runtime::SystemNotificationRuntime,
            ),
        ),
        actor_id.to_string(),
        metrics,
    )
}

fn uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).expect("valid uuid")
}
