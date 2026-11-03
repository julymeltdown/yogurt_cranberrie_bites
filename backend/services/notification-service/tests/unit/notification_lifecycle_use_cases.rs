use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use notification_service::application::dto::notification_dto::{
    DeleteNotificationDto, NotificationCreateDto, PurgeExpiredNotificationsDto,
};
use notification_service::application::dto::repository_dto::{
    NotificationAuditRecordDto as RepositoryAudit, NotificationPageRecordDto as RepositoryPage,
    NotificationRecordDto as RepositoryNotification,
};
use notification_service::application::notification_cursor::MAX_NOTIFICATION_CURSOR_CHARS;
use notification_service::application::ports::NotificationRepositoryPort;
use notification_service::application::use_cases::create_notification::CreateNotificationUseCase;
use notification_service::application::use_cases::delete_notification::DeleteNotificationUseCase;
use notification_service::application::use_cases::list_notifications::ListNotificationsUseCase;
use notification_service::application::use_cases::purge_expired_notifications::PurgeExpiredNotificationsUseCase;
use notification_service::domain::notification::{Notification, NotificationAuditEvent};
use notification_service::domain::notification_page::NotificationPage;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[tokio::test]
async fn suppressed_recipient_does_not_receive_new_notifications() {
    let user_id = uuid("00000000-0000-0000-0000-000000000200");
    let event_id = uuid("00000000-0000-0000-0000-000000000300");
    let repository = Arc::new(FakeNotificationRepository::default());
    repository.suppressed_users.lock().unwrap().insert(user_id);
    let use_case = CreateNotificationUseCase::new(
        Arc::clone(&repository),
        std::sync::Arc::new(
            notification_service::adapters::out::runtime::SystemNotificationRuntime,
        ),
    );

    let response = use_case
        .execute(create_request(user_id, event_id, "POST_LIKED"))
        .await
        .expect("suppressed notification is a successful no-op");

    assert!(response.suppressed);
    assert!(response.notification.is_none());
    assert_eq!(*repository.save_calls.lock().unwrap(), 0);
}

#[tokio::test]
async fn create_notification_delegates_idempotency_to_atomic_save() {
    let user_id = uuid("00000000-0000-0000-0000-000000000201");
    let event_id = uuid("00000000-0000-0000-0000-000000000301");
    let existing = notification(
        user_id,
        uuid("00000000-0000-0000-0000-000000000401"),
        event_id,
    );
    let repository = Arc::new(FakeNotificationRepository::with_existing_event(existing));
    let use_case = CreateNotificationUseCase::new(
        Arc::clone(&repository),
        std::sync::Arc::new(
            notification_service::adapters::out::runtime::SystemNotificationRuntime,
        ),
    );

    let response = use_case
        .execute(create_request(user_id, event_id, " post_liked "))
        .await
        .expect("existing notification");

    let response = response.notification.expect("notification");
    assert_eq!(response.event_id, event_id.to_string());
    assert_eq!(response.r#type, "POST_LIKED");
    assert!(repository.saved_notifications.lock().unwrap().is_empty());
    assert_eq!(*repository.save_calls.lock().unwrap(), 1);
    assert!(repository.find_by_event_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn create_notification_saves_normalized_notification_with_parsed_actor_and_time() {
    let user_id = uuid("00000000-0000-0000-0000-000000000202");
    let actor_id = uuid("00000000-0000-0000-0000-000000000203");
    let event_id = uuid("00000000-0000-0000-0000-000000000302");
    let occurred_at = instant(2_000);
    let repository = Arc::new(FakeNotificationRepository::default());
    let use_case = CreateNotificationUseCase::new(
        Arc::clone(&repository),
        std::sync::Arc::new(
            notification_service::adapters::out::runtime::SystemNotificationRuntime,
        ),
    );

    let response = use_case
        .execute(NotificationCreateDto {
            user_id: user_id.to_string(),
            r#type: " friend_accepted ".to_owned(),
            payload_json: "{\"ok\":true}".to_owned(),
            event_id: event_id.to_string(),
            actor_id: actor_id.to_string(),
            occurred_at: occurred_at.to_rfc3339(),
        })
        .await
        .expect("create notification");

    let response = response.notification.expect("notification");
    assert_eq!(response.user_id, user_id.to_string());
    assert_eq!(response.r#type, "FRIEND_ACCEPTED");
    assert_eq!(response.event_id, event_id.to_string());
    assert_eq!(response.actor_id, actor_id.to_string());
    assert_eq!(response.occurred_at, occurred_at.to_rfc3339());
    let saved = repository.saved_notifications.lock().unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].notification_type, "FRIEND_ACCEPTED");
    assert_eq!(saved[0].payload_json, "{\"ok\":true}");
}

#[tokio::test]
async fn list_notifications_caps_limit_and_parses_cursor() {
    let user_id = uuid("00000000-0000-0000-0000-000000000204");
    let cursor_id = uuid("00000000-0000-0000-0000-000000000304");
    let repository = Arc::new(FakeNotificationRepository::with_page(NotificationPage {
        items: vec![notification(
            user_id,
            uuid("00000000-0000-0000-0000-000000000404"),
            uuid("00000000-0000-0000-0000-000000000504"),
        )],
        next_cursor: Some("next".to_owned()),
    }));
    let use_case = ListNotificationsUseCase::new(Arc::clone(&repository));

    let response = use_case
        .execute(&user_id.to_string(), &format!("12345:{cursor_id}"), 10_000)
        .await
        .expect("list notifications");

    assert_eq!(response.items.len(), 1);
    assert_eq!(response.next_cursor.as_deref(), Some("next"));
    assert_eq!(
        *repository.list_calls.lock().unwrap(),
        vec![ListCall {
            user_id,
            cursor: Some((12_345, cursor_id)),
            limit: 100,
        }]
    );
}

#[tokio::test]
async fn invalid_list_cursor_fails_before_repository_lookup() {
    let user_id = uuid("00000000-0000-0000-0000-000000000205");
    let repository = Arc::new(FakeNotificationRepository::default());
    let use_case = ListNotificationsUseCase::new(Arc::clone(&repository));

    let oversized_cursor = " ".repeat(MAX_NOTIFICATION_CURSOR_CHARS + 1);
    for cursor in ["bad-cursor", oversized_cursor.as_str()] {
        let error = use_case
            .execute(&user_id.to_string(), cursor, 20)
            .await
            .expect_err("invalid cursor");

        assert_eq!(error.to_string(), "Invalid cursor format");
    }
    assert!(repository.list_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn owner_delete_soft_deletes_with_success_audit() {
    let user_id = uuid("00000000-0000-0000-0000-000000000206");
    let notification_id = uuid("00000000-0000-0000-0000-000000000406");
    let repository = Arc::new(FakeNotificationRepository::with_notification(notification(
        user_id,
        notification_id,
        uuid("00000000-0000-0000-0000-000000000506"),
    )));
    let use_case = DeleteNotificationUseCase::new(
        Arc::clone(&repository),
        std::sync::Arc::new(
            notification_service::adapters::out::runtime::SystemNotificationRuntime,
        ),
    );

    let response = use_case
        .execute(delete_request(user_id, notification_id, user_id))
        .await
        .expect("delete notification");

    assert!(response.deleted);
    assert_eq!(response.notification_id, notification_id.to_string());
    let deletes = repository.soft_delete_calls.lock().unwrap();
    assert_eq!(deletes.len(), 1);
    assert_eq!(deletes[0].user_id, user_id);
    assert_eq!(deletes[0].notification_id, notification_id);
    assert_eq!(deletes[0].actor_id, user_id);
    assert_eq!(deletes[0].reason, "user_request");
    assert_eq!(deletes[0].audit_event.outcome, "SUCCESS");
    assert_eq!(deletes[0].audit_event.request_id.as_deref(), Some("req-1"));
}

#[tokio::test]
async fn non_owner_delete_records_forbidden_audit_without_soft_delete() {
    let user_id = uuid("00000000-0000-0000-0000-000000000207");
    let actor_id = uuid("00000000-0000-0000-0000-000000000208");
    let notification_id = uuid("00000000-0000-0000-0000-000000000407");
    let repository = Arc::new(FakeNotificationRepository::with_notification(notification(
        user_id,
        notification_id,
        uuid("00000000-0000-0000-0000-000000000507"),
    )));
    let use_case = DeleteNotificationUseCase::new(
        Arc::clone(&repository),
        std::sync::Arc::new(
            notification_service::adapters::out::runtime::SystemNotificationRuntime,
        ),
    );

    let error = use_case
        .execute(delete_request(user_id, notification_id, actor_id))
        .await
        .expect_err("forbidden delete");

    assert_eq!(error.to_string(), "Forbidden");
    assert!(repository.soft_delete_calls.lock().unwrap().is_empty());
    let audits = repository.audit_events.lock().unwrap();
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].actor_id, Some(actor_id));
    assert_eq!(audits[0].notification_id, Some(notification_id));
    assert_eq!(audits[0].user_id, Some(user_id));
    assert_eq!(audits[0].outcome, "FORBIDDEN");
}

#[tokio::test]
async fn purge_notifications_trims_optional_metadata_and_returns_count() {
    let actor_id = uuid("00000000-0000-0000-0000-000000000209");
    let purged_id = uuid("00000000-0000-0000-0000-000000000409");
    let repository = Arc::new(FakeNotificationRepository::default());
    *repository.purge_result.lock().unwrap() = vec![purged_id];
    let use_case = PurgeExpiredNotificationsUseCase::new(
        Arc::clone(&repository),
        std::sync::Arc::new(
            notification_service::adapters::out::runtime::SystemNotificationRuntime,
        ),
    );

    let response = use_case
        .execute(PurgeExpiredNotificationsDto {
            actor_id: format!(" {actor_id} "),
            request_id: Some(" req-2 ".to_owned()),
            ip_address: Some(" 10.0.0.2 ".to_owned()),
            user_agent: Some("  ".to_owned()),
            dry_run: true,
        })
        .await
        .expect("purge notifications");

    assert_eq!(response.purged_count, 1);
    let calls = repository.purge_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].actor_id, actor_id);
    assert_eq!(calls[0].request_id.as_deref(), Some("req-2"));
    assert_eq!(calls[0].ip_address.as_deref(), Some("10.0.0.2"));
    assert_eq!(calls[0].user_agent, None);
    assert!(calls[0].dry_run);
}

#[derive(Default)]
struct FakeNotificationRepository {
    existing_event: Mutex<Option<Notification>>,
    notification: Mutex<Option<Notification>>,
    page: Mutex<Option<NotificationPage>>,
    purge_result: Mutex<Vec<Uuid>>,
    saved_notifications: Mutex<Vec<Notification>>,
    save_calls: Mutex<usize>,
    find_by_event_calls: Mutex<Vec<FindByEventCall>>,
    find_by_id_calls: Mutex<Vec<FindByIdCall>>,
    list_calls: Mutex<Vec<ListCall>>,
    soft_delete_calls: Mutex<Vec<SoftDeleteCall>>,
    audit_events: Mutex<Vec<NotificationAuditEvent>>,
    purge_calls: Mutex<Vec<PurgeCall>>,
    suppressed_users: Mutex<HashSet<Uuid>>,
}

impl FakeNotificationRepository {
    fn with_existing_event(notification: Notification) -> Self {
        Self {
            existing_event: Mutex::new(Some(notification)),
            ..Self::default()
        }
    }

    fn with_notification(notification: Notification) -> Self {
        Self {
            notification: Mutex::new(Some(notification)),
            ..Self::default()
        }
    }

    fn with_page(page: NotificationPage) -> Self {
        Self {
            page: Mutex::new(Some(page)),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FindByEventCall {
    user_id: Uuid,
    event_id: Uuid,
    notification_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FindByIdCall {
    user_id: Uuid,
    notification_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ListCall {
    user_id: Uuid,
    cursor: Option<(i64, Uuid)>,
    limit: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SoftDeleteCall {
    user_id: Uuid,
    notification_id: Uuid,
    actor_id: Uuid,
    reason: String,
    retention_until: DateTime<Utc>,
    audit_event: NotificationAuditEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PurgeCall {
    cutoff: DateTime<Utc>,
    actor_id: Uuid,
    request_id: Option<String>,
    ip_address: Option<String>,
    user_agent: Option<String>,
    dry_run: bool,
}

#[async_trait]
impl NotificationRepositoryPort for FakeNotificationRepository {
    async fn is_recipient_suppressed(
        &self,
        user_id: Uuid,
    ) -> notification_service::application::errors::NotificationResult<bool> {
        Ok(self.suppressed_users.lock().unwrap().contains(&user_id))
    }

    async fn save(
        &self,
        notification: &RepositoryNotification,
    ) -> notification_service::application::errors::NotificationResult<RepositoryNotification> {
        let notification = &Notification::from(notification);
        let result: notification_service::application::errors::NotificationResult<Notification> =
            async {
                *self.save_calls.lock().unwrap() += 1;
                if let Some(existing) = self.existing_event.lock().unwrap().clone() {
                    if existing.user_id == notification.user_id
                        && existing.event_id == notification.event_id
                        && existing.notification_type == notification.notification_type
                    {
                        return Ok(existing);
                    }
                }
                self.saved_notifications
                    .lock()
                    .unwrap()
                    .push(notification.clone());
                Ok(notification.clone())
            }
            .await;
        result.map(|value| value.into())
    }

    async fn find_by_event_id_and_type(
        &self,
        user_id: Uuid,
        event_id: Uuid,
        notification_type: &str,
    ) -> notification_service::application::errors::NotificationResult<Option<RepositoryNotification>>
    {
        let result: notification_service::application::errors::NotificationResult<
            Option<Notification>,
        > = async {
            self.find_by_event_calls
                .lock()
                .unwrap()
                .push(FindByEventCall {
                    user_id,
                    event_id,
                    notification_type: notification_type.to_owned(),
                });
            Ok(self.existing_event.lock().unwrap().clone())
        }
        .await;
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
        user_id: Uuid,
        cursor: Option<(i64, Uuid)>,
        limit: i32,
    ) -> notification_service::application::errors::NotificationResult<RepositoryPage> {
        let result: notification_service::application::errors::NotificationResult<
            NotificationPage,
        > = async {
            self.list_calls.lock().unwrap().push(ListCall {
                user_id,
                cursor,
                limit,
            });
            Ok(self
                .page
                .lock()
                .unwrap()
                .clone()
                .unwrap_or(NotificationPage {
                    items: Vec::new(),
                    next_cursor: None,
                }))
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
        user_id: Uuid,
        notification_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
        audit_event: &RepositoryAudit,
    ) -> notification_service::application::errors::NotificationResult<bool> {
        let audit_event = &NotificationAuditEvent::from(audit_event);

        self.soft_delete_calls.lock().unwrap().push(SoftDeleteCall {
            user_id,
            notification_id,
            actor_id,
            reason: reason.to_owned(),
            retention_until,
            audit_event: audit_event.clone(),
        });
        Ok(self
            .notification
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|value| value.id == notification_id && value.user_id == user_id))
    }

    async fn record_audit_event(
        &self,
        event: &RepositoryAudit,
    ) -> notification_service::application::errors::NotificationResult<()> {
        let event = &NotificationAuditEvent::from(event);

        self.audit_events.lock().unwrap().push(event.clone());
        Ok(())
    }

    async fn purge_expired_notifications(
        &self,
        cutoff: DateTime<Utc>,
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
    ) -> notification_service::application::errors::NotificationResult<Vec<Uuid>> {
        self.purge_calls.lock().unwrap().push(PurgeCall {
            cutoff,
            actor_id,
            request_id,
            ip_address,
            user_agent,
            dry_run,
        });
        Ok(self.purge_result.lock().unwrap().clone())
    }
    // Explicit recording capabilities; these do not establish PG atomicity.
    async fn is_actor_suppressed(
        &self,
        user_id: Uuid,
    ) -> notification_service::application::errors::NotificationResult<bool> {
        self.is_recipient_suppressed(user_id).await
    }

    async fn create_consistent(
        &self,
        notification: &RepositoryNotification,
    ) -> notification_service::application::errors::NotificationResult<
        notification_service::application::notification_contracts::NotificationCreateOutcome,
    > {
        if self.is_recipient_suppressed(notification.user_id).await? {
            return Ok(notification_service::application::notification_contracts::NotificationCreateOutcome::Suppressed(
                notification_service::application::notification_contracts::NotificationSuppression::Recipient,
            ));
        }
        if let Some(actor_id) = notification.actor_id {
            if self.is_actor_suppressed(actor_id).await? {
                return Ok(notification_service::application::notification_contracts::NotificationCreateOutcome::Suppressed(
                    notification_service::application::notification_contracts::NotificationSuppression::Actor,
                ));
            }
        }
        let stored = self.save(notification).await?;
        if stored.id == notification.id {
            Ok(notification_service::application::notification_contracts::NotificationCreateOutcome::Created(stored))
        } else {
            Ok(notification_service::application::notification_contracts::NotificationCreateOutcome::Existing(stored))
        }
    }

    async fn list_consistent(
        &self,
        user_id: Uuid,
        cursor: Option<
            notification_service::application::notification_contracts::NotificationCursor,
        >,
        limit: i32,
    ) -> notification_service::application::errors::NotificationResult<
        notification_service::application::notification_contracts::NotificationQueryPage,
    > {
        let legacy = cursor.map(|cursor| {
            match cursor {
        notification_service::application::notification_contracts::NotificationCursor::Legacy {
            epoch_millis,
            id,
        } => (epoch_millis, id),
        notification_service::application::notification_contracts::NotificationCursor::V2 {
            position,
            ..
        } => (position.created_at.timestamp_millis(), position.id),
    }
        });
        let page = self.list(user_id, legacy, limit).await?;
        let next_position = if page.next_cursor.is_some() {
            page.items.last().map(|item| {
                notification_service::application::notification_contracts::NotificationPosition {
                    created_at: item.created_at,
                    id: item.id,
                }
            })
        } else {
            None
        };
        Ok(
            notification_service::application::notification_contracts::NotificationQueryPage {
                items: page.items,
                next_position,
                legacy_next_cursor: page.next_cursor,
            },
        )
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

fn create_request(user_id: Uuid, event_id: Uuid, notification_type: &str) -> NotificationCreateDto {
    NotificationCreateDto {
        user_id: user_id.to_string(),
        r#type: notification_type.to_owned(),
        payload_json: "{}".to_owned(),
        event_id: event_id.to_string(),
        actor_id: String::new(),
        occurred_at: instant(1_000).to_rfc3339(),
    }
}

fn delete_request(user_id: Uuid, notification_id: Uuid, actor_id: Uuid) -> DeleteNotificationDto {
    DeleteNotificationDto {
        notification_id: notification_id.to_string(),
        user_id: user_id.to_string(),
        actor_id: actor_id.to_string(),
        request_id: Some(" req-1 ".to_owned()),
        ip_address: Some(" 127.0.0.1 ".to_owned()),
        user_agent: Some(" unit-test ".to_owned()),
    }
}

fn notification(user_id: Uuid, notification_id: Uuid, event_id: Uuid) -> Notification {
    Notification {
        id: notification_id,
        user_id,
        notification_type: "POST_LIKED".to_owned(),
        payload_json: "{}".to_owned(),
        created_at: instant(1_000),
        read_at: None,
        event_id,
        actor_id: None,
        occurred_at: instant(1_000),
    }
}

fn instant(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(seconds, 0)
        .single()
        .expect("valid instant")
}

fn uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).expect("valid uuid")
}

#[tokio::test]
async fn runtime_preserves_fallback_event_and_notification_selection_order() {
    use super::scripted_runtime::{RuntimeValue, ScriptedRuntime};
    let user_id = Uuid::from_u128(1);
    let event_id = Uuid::from_u128(2);
    let notification_id = Uuid::from_u128(3);
    let occurred_at = Utc.timestamp_opt(2_000, 123_456_789).unwrap();
    let created_at = Utc.timestamp_opt(3_000, 987_654_321).unwrap();
    let runtime = Arc::new(ScriptedRuntime::new([
        RuntimeValue::Id(event_id),
        RuntimeValue::Time(occurred_at),
        RuntimeValue::Id(notification_id),
        RuntimeValue::Time(created_at),
    ]));
    let repository = Arc::new(FakeNotificationRepository::default());
    let mut request = create_request(user_id, event_id, "POST_LIKED");
    request.event_id.clear();
    request.occurred_at.clear();
    let result = CreateNotificationUseCase::new(repository.clone(), runtime.clone())
        .execute(request)
        .await
        .unwrap();
    assert!(!result.suppressed);
    let saved = repository.saved_notifications.lock().unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].event_id, event_id);
    assert_eq!(saved[0].id, notification_id);
    assert_eq!(saved[0].occurred_at, occurred_at);
    assert_eq!(saved[0].created_at, created_at);
    runtime.assert_exhausted();
}

#[tokio::test]
async fn explicit_event_and_time_do_not_spend_runtime_fallback_values() {
    use super::scripted_runtime::{RuntimeValue, ScriptedRuntime};
    let user_id = Uuid::from_u128(11);
    let event_id = Uuid::from_u128(12);
    let notification_id = Uuid::from_u128(13);
    let created_at = Utc.timestamp_opt(4_000, 555_666_777).unwrap();
    let runtime = Arc::new(ScriptedRuntime::new([
        RuntimeValue::Id(notification_id),
        RuntimeValue::Time(created_at),
    ]));
    let repository = Arc::new(FakeNotificationRepository::default());
    let request = create_request(user_id, event_id, "POST_LIKED");
    let occurred_at = DateTime::parse_from_rfc3339(&request.occurred_at)
        .unwrap()
        .with_timezone(&Utc);
    CreateNotificationUseCase::new(repository.clone(), runtime.clone())
        .execute(request)
        .await
        .unwrap();
    let saved = repository.saved_notifications.lock().unwrap();
    assert_eq!(saved[0].event_id, event_id);
    assert_eq!(saved[0].id, notification_id);
    assert_eq!(saved[0].occurred_at, occurred_at);
    assert_eq!(saved[0].created_at, created_at);
    runtime.assert_exhausted();
}

#[tokio::test]
async fn invalid_creation_inputs_with_explicit_event_do_not_spend_runtime_values() {
    use super::scripted_runtime::ScriptedRuntime;
    let runtime = Arc::new(ScriptedRuntime::new([]));
    let repository = Arc::new(FakeNotificationRepository::default());
    let use_case = CreateNotificationUseCase::new(repository.clone(), runtime.clone());
    for field in ["user", "type", "actor", "event", "time"] {
        let mut request = create_request(Uuid::from_u128(21), Uuid::from_u128(22), "POST_LIKED");
        match field {
            "user" => request.user_id = "invalid".into(),
            "type" => request.r#type = "INVALID".into(),
            "actor" => request.actor_id = "invalid".into(),
            "event" => request.event_id = "invalid".into(),
            _ => request.occurred_at = "invalid".into(),
        }
        assert!(use_case.execute(request).await.is_err());
    }
    assert_eq!(*repository.save_calls.lock().unwrap(), 0);
    runtime.assert_exhausted();
}

#[tokio::test]
async fn delete_preserves_distinct_retention_and_audit_times_and_identity() {
    use super::scripted_runtime::{RuntimeValue, ScriptedRuntime};
    let user = Uuid::from_u128(31);
    let id = Uuid::from_u128(32);
    let audit_id = Uuid::from_u128(33);
    let accepted_at = Utc.timestamp_opt(5_000, 100_200_300).unwrap();
    let audited_at = Utc.timestamp_opt(5_001, 700_800_900).unwrap();
    let runtime = Arc::new(ScriptedRuntime::new([
        RuntimeValue::Time(accepted_at),
        RuntimeValue::Id(audit_id),
        RuntimeValue::Time(audited_at),
    ]));
    let repository = Arc::new(FakeNotificationRepository::with_notification(notification(
        user,
        id,
        Uuid::from_u128(34),
    )));
    let response = DeleteNotificationUseCase::new(repository.clone(), runtime.clone())
        .execute(delete_request(user, id, user))
        .await
        .unwrap();
    assert!(response.deleted);
    let calls = repository.soft_delete_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    let call = &calls[0];
    assert_eq!(
        call.retention_until,
        accepted_at + chrono::Duration::days(30)
    );
    assert_eq!(call.audit_event.id, audit_id);
    assert_eq!(call.audit_event.occurred_at, audited_at);
    assert_eq!(call.audit_event.outcome, "SUCCESS");
    assert_eq!(call.audit_event.actor_id, Some(user));
    let metadata: serde_json::Value =
        serde_json::from_str(&call.audit_event.metadata_json).unwrap();
    assert_eq!(
        metadata["retentionUntil"],
        serde_json::json!(call.retention_until)
    );
    runtime.assert_exhausted();
}

#[tokio::test]
async fn forbidden_delete_spends_only_audit_identity_and_time() {
    use super::scripted_runtime::{RuntimeValue, ScriptedRuntime};
    let user = Uuid::from_u128(41);
    let actor = Uuid::from_u128(42);
    let id = Uuid::from_u128(43);
    let audit_id = Uuid::from_u128(44);
    let audited_at = Utc.timestamp_opt(6_000, 111_222_333).unwrap();
    let runtime = Arc::new(ScriptedRuntime::new([
        RuntimeValue::Id(audit_id),
        RuntimeValue::Time(audited_at),
    ]));
    let repository = Arc::new(FakeNotificationRepository::with_notification(notification(
        user,
        id,
        Uuid::from_u128(45),
    )));
    let error = DeleteNotificationUseCase::new(repository.clone(), runtime.clone())
        .execute(delete_request(user, id, actor))
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "Forbidden");
    assert!(repository.soft_delete_calls.lock().unwrap().is_empty());
    let audits = repository.audit_events.lock().unwrap();
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].id, audit_id);
    assert_eq!(audits[0].occurred_at, audited_at);
    assert_eq!(audits[0].outcome, "FORBIDDEN");
    assert_eq!(audits[0].actor_id, Some(actor));
    runtime.assert_exhausted();
}

#[tokio::test]
async fn purge_passes_exact_runtime_cutoff_after_actor_validation() {
    use super::scripted_runtime::{RuntimeValue, ScriptedRuntime};
    let actor = Uuid::from_u128(51);
    let cutoff = Utc.timestamp_opt(7_000, 444_555_666).unwrap();
    let runtime = Arc::new(ScriptedRuntime::new([RuntimeValue::Time(cutoff)]));
    let repository = Arc::new(FakeNotificationRepository::default());
    let use_case = PurgeExpiredNotificationsUseCase::new(repository.clone(), runtime.clone());
    let request = |actor_id| PurgeExpiredNotificationsDto {
        actor_id,
        request_id: None,
        ip_address: None,
        user_agent: None,
        dry_run: true,
    };
    assert!(use_case.execute(request("invalid".into())).await.is_err());
    assert!(repository.purge_calls.lock().unwrap().is_empty());
    assert_eq!(
        use_case
            .execute(request(actor.to_string()))
            .await
            .unwrap()
            .purged_count,
        0
    );
    let calls = repository.purge_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].cutoff, cutoff);
    assert!(calls[0].dry_run);
    runtime.assert_exhausted();
}

#[tokio::test]
async fn missing_delete_lookup_does_not_spend_runtime_or_record_audit() {
    use super::scripted_runtime::ScriptedRuntime;
    let user = Uuid::from_u128(91);
    let runtime = Arc::new(ScriptedRuntime::new([]));
    let repository = Arc::new(FakeNotificationRepository::default());
    let use_case = DeleteNotificationUseCase::new(repository.clone(), runtime.clone());
    let error = use_case
        .execute(delete_request(user, Uuid::from_u128(92), user))
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "Notification not found");
    assert_eq!(repository.find_by_id_calls.lock().unwrap().len(), 1);
    assert!(repository.soft_delete_calls.lock().unwrap().is_empty());
    assert!(repository.audit_events.lock().unwrap().is_empty());
    runtime.assert_exhausted();
}
