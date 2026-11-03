use chrono::{DateTime, Utc};
use notification_service::application::dto::notification_dto::{
    DeleteNotificationDto, NotificationCreateDto, PurgeExpiredNotificationsDto,
};
use uuid::Uuid;

type DeletedNotificationRow = (
    Option<DateTime<Utc>>,
    Option<Uuid>,
    Option<String>,
    Option<DateTime<Utc>>,
);

#[tokio::test]
async fn postgres_backed_notification_flow_supports_idempotent_create_and_reads() {
    let app = crate::support::start_postgres_app().await.expect("app");

    let request = NotificationCreateDto {
        user_id: "11111111-1111-1111-1111-111111111111".into(),
        r#type: "COMMENT".into(),
        payload_json: "{\"commentId\":\"c1\"}".into(),
        event_id: "cccccccc-1111-1111-1111-111111111111".into(),
        actor_id: "22222222-2222-2222-2222-222222222222".into(),
        occurred_at: "2026-01-20T09:10:00Z".into(),
    };
    let (created, duplicate) = tokio::join!(
        app.state.create_notification.execute(request.clone()),
        app.state.create_notification.execute(request),
    );
    let created = created
        .expect("create")
        .notification
        .expect("created notification");
    let duplicate = duplicate
        .expect("duplicate")
        .notification
        .expect("duplicate notification");
    assert_eq!(created.id, duplicate.id);
    let row_count: i64 = sqlx::query_scalar(
        "select count(*) from notifications where user_id = $1 and event_id = $2 and type = 'COMMENT'",
    )
    .bind(Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("user id"))
    .bind(Uuid::parse_str("cccccccc-1111-1111-1111-111111111111").expect("event id"))
    .fetch_one(&app.pool)
    .await
    .expect("notification count");
    assert_eq!(row_count, 1);

    let page = app
        .state
        .list_notifications
        .execute("11111111-1111-1111-1111-111111111111", "", 10)
        .await
        .expect("list");
    assert_eq!(page.items.len(), 1);

    let marked = app
        .state
        .mark_read
        .execute("11111111-1111-1111-1111-111111111111", &created.id)
        .await
        .expect("mark");
    assert!(marked.updated);

    let updated = app
        .state
        .mark_all_read
        .execute("11111111-1111-1111-1111-111111111111")
        .await
        .expect("mark all");
    assert_eq!(updated, 0);
    app.shutdown().await.expect("shutdown test app");
}

#[tokio::test]
async fn deleting_notification_soft_deletes_row_records_audit_and_hides_reads() {
    let app = crate::support::start_postgres_app().await.expect("app");
    let user_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid");

    let created = app
        .state
        .create_notification
        .execute(NotificationCreateDto {
            user_id: user_id.to_string(),
            r#type: "SYSTEM".into(),
            payload_json: "{\"message\":\"delete me\"}".into(),
            event_id: "dddddddd-1111-1111-1111-111111111111".into(),
            actor_id: String::new(),
            occurred_at: "2026-01-20T11:10:00Z".into(),
        })
        .await
        .expect("create")
        .notification
        .expect("notification");
    let notification_id = Uuid::parse_str(&created.id).expect("notification id");

    let deleted = app
        .state
        .delete_notification
        .execute(DeleteNotificationDto {
            notification_id: created.id.clone(),
            user_id: user_id.to_string(),
            actor_id: user_id.to_string(),
            request_id: Some("req-notification-delete-1".into()),
            ip_address: Some("203.0.113.90".into()),
            user_agent: Some("notification-delete-test".into()),
        })
        .await
        .expect("delete notification");
    assert!(deleted.deleted);
    assert_eq!(deleted.notification_id, created.id);

    let row: DeletedNotificationRow = sqlx::query_as(
        r#"
        select deleted_at, deleted_by, delete_reason, retention_until
        from notifications
        where id = $1
        "#,
    )
    .bind(notification_id)
    .fetch_one(&app.pool)
    .await
    .expect("notification row");
    let deleted_at = row.0.expect("deleted_at");
    assert_eq!(row.1, Some(user_id));
    assert_eq!(row.2.as_deref(), Some("user_request"));
    assert!(row.3.expect("retention_until") > deleted_at);

    let list = app
        .state
        .list_notifications
        .execute(&user_id.to_string(), "", 10)
        .await
        .expect("list");
    assert!(list.items.is_empty());

    let mark_error = app
        .state
        .mark_read
        .execute(&user_id.to_string(), &created.id)
        .await
        .expect_err("deleted notification should be hidden");
    assert_eq!(mark_error.to_string(), "Notification not found");

    let audit_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from notification_audit_events
        where notification_id = $1
          and user_id = $2
          and actor_id = $3
          and action = 'NOTIFICATION_DELETED'
          and outcome = 'SUCCESS'
          and request_id = 'req-notification-delete-1'
          and ip_address = '203.0.113.90'
          and user_agent = 'notification-delete-test'
        "#,
    )
    .bind(notification_id)
    .bind(user_id)
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .expect("audit count");
    assert_eq!(audit_count, 1);
    app.shutdown().await.expect("shutdown test app");
}

#[tokio::test]
async fn purging_expired_notification_retention_hard_deletes_only_expired_rows_and_records_audit() {
    let app = crate::support::start_postgres_app().await.expect("app");
    let expired_user_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid");
    let retained_user_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("uuid");
    let operator_id = Uuid::parse_str("99999999-9999-9999-9999-999999999999").expect("uuid");

    let expired_id = create_deleted_notification(
        &app,
        expired_user_id,
        "eeeeeeee-1111-1111-1111-111111111111",
        "req-delete-expired-notification",
    )
    .await;
    let retained_id = create_deleted_notification(
        &app,
        retained_user_id,
        "eeeeeeee-2222-2222-2222-222222222222",
        "req-delete-retained-notification",
    )
    .await;

    sqlx::query(
        r#"
        update notifications
        set retention_until = now() - interval '1 day'
        where id = $1
        "#,
    )
    .bind(expired_id)
    .execute(&app.pool)
    .await
    .expect("expire notification");

    let purged = app
        .state
        .purge_expired_notifications
        .execute(PurgeExpiredNotificationsDto {
            actor_id: operator_id.to_string(),
            request_id: Some("req-notification-retention-purge-1".into()),
            ip_address: Some("203.0.113.91".into()),
            user_agent: Some("notification-retention-purge-test".into()),
            dry_run: false,
        })
        .await
        .expect("purge notifications");
    assert_eq!(purged.purged_count, 1);

    let expired_count: i64 = sqlx::query_scalar("select count(*) from notifications where id = $1")
        .bind(expired_id)
        .fetch_one(&app.pool)
        .await
        .expect("expired count");
    assert_eq!(expired_count, 0);
    let retained_count: i64 =
        sqlx::query_scalar("select count(*) from notifications where id = $1")
            .bind(retained_id)
            .fetch_one(&app.pool)
            .await
            .expect("retained count");
    assert_eq!(retained_count, 1);

    let audit_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from notification_audit_events
        where notification_id = $1
          and user_id = $2
          and actor_id = $3
          and action = 'NOTIFICATION_PURGED'
          and outcome = 'SUCCESS'
          and request_id = 'req-notification-retention-purge-1'
          and ip_address = '203.0.113.91'
          and user_agent = 'notification-retention-purge-test'
        "#,
    )
    .bind(expired_id)
    .bind(expired_user_id)
    .bind(operator_id)
    .fetch_one(&app.pool)
    .await
    .expect("purge audit count");
    assert_eq!(audit_count, 1);
    app.shutdown().await.expect("shutdown test app");
}

#[tokio::test]
async fn dry_run_notification_retention_purge_counts_expired_rows_without_deleting_and_records_audit(
) {
    let app = crate::support::start_postgres_app().await.expect("app");
    let user_id = Uuid::parse_str("33333333-3333-3333-3333-333333333333").expect("uuid");
    let operator_id = Uuid::parse_str("99999999-9999-9999-9999-999999999999").expect("uuid");

    let notification_id = create_deleted_notification(
        &app,
        user_id,
        "eeeeeeee-3333-3333-3333-333333333333",
        "req-delete-dry-run-notification",
    )
    .await;
    sqlx::query(
        r#"
        update notifications
        set retention_until = now() - interval '1 day'
        where id = $1
        "#,
    )
    .bind(notification_id)
    .execute(&app.pool)
    .await
    .expect("expire notification");

    let purged = app
        .state
        .purge_expired_notifications
        .execute(PurgeExpiredNotificationsDto {
            actor_id: operator_id.to_string(),
            request_id: Some("req-notification-retention-dry-run-1".into()),
            ip_address: Some("203.0.113.92".into()),
            user_agent: Some("notification-retention-dry-run-test".into()),
            dry_run: true,
        })
        .await
        .expect("dry-run purge notifications");
    assert_eq!(purged.purged_count, 1);

    let notification_count: i64 =
        sqlx::query_scalar("select count(*) from notifications where id = $1")
            .bind(notification_id)
            .fetch_one(&app.pool)
            .await
            .expect("notification count");
    assert_eq!(notification_count, 1);

    let audit_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from notification_audit_events
        where notification_id = $1
          and user_id = $2
          and actor_id = $3
          and action = 'NOTIFICATION_PURGED'
          and outcome = 'DRY_RUN'
          and request_id = 'req-notification-retention-dry-run-1'
          and ip_address = '203.0.113.92'
          and user_agent = 'notification-retention-dry-run-test'
        "#,
    )
    .bind(notification_id)
    .bind(user_id)
    .bind(operator_id)
    .fetch_one(&app.pool)
    .await
    .expect("dry-run audit count");
    assert_eq!(audit_count, 1);
    app.shutdown().await.expect("shutdown test app");
}

#[tokio::test]
async fn retention_purge_worker_hard_deletes_expired_notifications_and_records_metrics() {
    let app = crate::support::start_postgres_app().await.expect("app");
    let user_id = Uuid::parse_str("44444444-4444-4444-4444-444444444444").expect("uuid");
    let operator_id = Uuid::parse_str("99999999-9999-9999-9999-999999999999").expect("uuid");

    let notification_id = create_deleted_notification(
        &app,
        user_id,
        "eeeeeeee-4444-4444-4444-444444444444",
        "req-delete-worker-notification",
    )
    .await;
    sqlx::query(
        r#"
        update notifications
        set retention_until = now() - interval '1 day'
        where id = $1
        "#,
    )
    .bind(notification_id)
    .execute(&app.pool)
    .await
    .expect("expire notification");

    let purged = app
        .state
        .retention_purge_worker
        .tick()
        .await
        .expect("retention worker tick");
    assert_eq!(purged, 1);

    let notification_count: i64 =
        sqlx::query_scalar("select count(*) from notifications where id = $1")
            .bind(notification_id)
            .fetch_one(&app.pool)
            .await
            .expect("notification count");
    assert_eq!(notification_count, 0);

    let audit_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from notification_audit_events
        where notification_id = $1
          and user_id = $2
          and actor_id = $3
          and action = 'NOTIFICATION_PURGED'
          and outcome = 'SUCCESS'
          and request_id = 'notification-retention-worker'
          and user_agent = 'notification-retention-worker'
        "#,
    )
    .bind(notification_id)
    .bind(user_id)
    .bind(operator_id)
    .fetch_one(&app.pool)
    .await
    .expect("worker audit count");
    assert_eq!(audit_count, 1);

    let metrics = app.state.metrics.render_prometheus();
    assert!(metrics.contains("notification_retention_purge_runs_total{outcome=\"success\"} 1"));
    assert!(metrics
        .contains("notification_retention_purged_notifications_total{outcome=\"success\"} 1"));
    app.shutdown().await.expect("shutdown test app");
}

async fn create_deleted_notification(
    app: &crate::support::TestApp,
    user_id: Uuid,
    event_id: &str,
    request_id: &str,
) -> Uuid {
    let created = app
        .state
        .create_notification
        .execute(NotificationCreateDto {
            user_id: user_id.to_string(),
            r#type: "SYSTEM".into(),
            payload_json: "{\"message\":\"retention\"}".into(),
            event_id: event_id.into(),
            actor_id: String::new(),
            occurred_at: "2026-01-21T11:10:00Z".into(),
        })
        .await
        .expect("create notification")
        .notification
        .expect("notification");
    app.state
        .delete_notification
        .execute(DeleteNotificationDto {
            notification_id: created.id.clone(),
            user_id: user_id.to_string(),
            actor_id: user_id.to_string(),
            request_id: Some(request_id.into()),
            ip_address: Some("203.0.113.93".into()),
            user_agent: Some("notification-retention-test".into()),
        })
        .await
        .expect("delete notification");
    Uuid::parse_str(&created.id).expect("notification id")
}

/// T01.1 ownership: every delegated Notification method is keyed by the
/// delegated user in SQL, so another user's notification id is
/// indistinguishable from a missing one and is never read, marked or deleted.
#[tokio::test]
async fn another_users_notification_is_not_listed_marked_or_deleted() {
    let app = crate::support::start_postgres_app().await.expect("app");
    let owner = Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("owner");
    let intruder = Uuid::parse_str("33333333-3333-3333-3333-333333333333").expect("intruder");
    let created = app
        .state
        .create_notification
        .execute(NotificationCreateDto {
            user_id: owner.to_string(),
            r#type: "SYSTEM".into(),
            payload_json: "{\"message\":\"private\"}".into(),
            event_id: "eeeeeeee-1111-1111-1111-111111111111".into(),
            actor_id: String::new(),
            occurred_at: "2026-01-20T12:00:00Z".into(),
        })
        .await
        .expect("create")
        .notification
        .expect("notification");
    let notification_id = Uuid::parse_str(&created.id).expect("notification id");

    let listed = app
        .state
        .list_notifications
        .execute(&intruder.to_string(), "", 100)
        .await
        .expect("intruder list");
    assert!(listed.items.is_empty());

    let mark = app
        .state
        .mark_read
        .execute(&intruder.to_string(), &created.id)
        .await
        .expect_err("intruder cannot mark another user's notification");
    assert_eq!(mark.to_string(), "Notification not found");

    let marked_all = app
        .state
        .mark_all_read
        .execute(&intruder.to_string())
        .await
        .expect("intruder mark all");
    assert_eq!(marked_all, 0);

    // The delegation binds both user fields to the intruder.
    let delete = app
        .state
        .delete_notification
        .execute(DeleteNotificationDto {
            notification_id: created.id.clone(),
            user_id: intruder.to_string(),
            actor_id: intruder.to_string(),
            request_id: None,
            ip_address: None,
            user_agent: None,
        })
        .await
        .expect_err("intruder cannot delete another user's notification");
    assert_eq!(delete.to_string(), "Notification not found");

    let (read_at, deleted_at): (Option<DateTime<Utc>>, Option<DateTime<Utc>>) =
        sqlx::query_as("select read_at, deleted_at from notifications where id = $1")
            .bind(notification_id)
            .fetch_one(&app.pool)
            .await
            .expect("notification row");
    assert!(
        read_at.is_none(),
        "intruder marked the owner's notification"
    );
    assert!(
        deleted_at.is_none(),
        "intruder deleted the owner's notification"
    );
    let owner_page = app
        .state
        .list_notifications
        .execute(&owner.to_string(), "", 100)
        .await
        .expect("owner list");
    assert_eq!(owner_page.items.len(), 1);
    app.shutdown().await.expect("shutdown test app");
}
