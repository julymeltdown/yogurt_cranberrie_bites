use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use notification_service::application::dto::notification_dto::ApplyUserDeletionDto;
use notification_service::application::dto::repository_dto::{
    NotificationAuditRecordDto as RepositoryAudit, NotificationPageRecordDto as RepositoryPage,
    NotificationRecordDto as RepositoryNotification,
};
use notification_service::application::ports::NotificationRepositoryPort;
use notification_service::application::use_cases::apply_user_deletion::ApplyUserDeletionUseCase;
use notification_service::domain::notification::Notification;
use notification_service::domain::notification_page::NotificationPage;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

type UserDeletionCall = (Uuid, Uuid, Uuid, DateTime<Utc>);

#[derive(Clone, Default)]
struct RecordingRepository {
    calls: Arc<Mutex<Vec<UserDeletionCall>>>,
}

#[async_trait]
impl NotificationRepositoryPort for RecordingRepository {
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
        > = async { Ok(None) }.await;
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
        > = async { Ok(None) }.await;
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
        > = async {
            Ok(NotificationPage {
                items: Vec::new(),
                next_cursor: None,
            })
        }
        .await;
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
        > = async { Ok(None) }.await;
        result.map(|value| value.map(|(notification, updated)| (notification.into(), updated)))
    }

    async fn mark_all_read(
        &self,
        _user_id: Uuid,
        _read_at: DateTime<Utc>,
    ) -> notification_service::application::errors::NotificationResult<i64> {
        Ok(0)
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
        Ok(false)
    }

    async fn record_audit_event(
        &self,
        _event: &RepositoryAudit,
    ) -> notification_service::application::errors::NotificationResult<()> {
        Ok(())
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
        Ok(Vec::new())
    }

    async fn apply_user_deletion(
        &self,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> notification_service::application::errors::NotificationResult<()> {
        self.calls
            .lock()
            .unwrap()
            .push((saga_id, user_id, actor_id, retention_until));
        Ok(())
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
}

#[tokio::test]
async fn account_deletion_suppresses_recipient_and_hides_existing_notifications_idempotently() {
    let repository = RecordingRepository::default();
    let use_case = ApplyUserDeletionUseCase::new(repository.clone());
    let saga_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let retention = Utc.with_ymd_and_hms(2026, 9, 2, 12, 0, 0).unwrap();
    let request = ApplyUserDeletionDto {
        saga_id: saga_id.to_string(),
        user_id: user_id.to_string(),
        actor_id: user_id.to_string(),
        retention_until: retention.to_rfc3339(),
    };

    assert!(
        use_case
            .execute(request.clone())
            .await
            .expect("first")
            .applied
    );
    assert!(use_case.execute(request).await.expect("repeat").applied);
    assert_eq!(
        repository.calls.lock().unwrap().as_slice(),
        &[
            (saga_id, user_id, user_id, retention),
            (saga_id, user_id, user_id, retention),
        ]
    );
}
