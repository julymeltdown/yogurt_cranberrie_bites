use notification_service::adapters::r#in::grpc::server::proto;
use platform::grpc_auth::{VerifiedCaller, Workload};
use tonic::Request;

#[tokio::test]
async fn backend_diagnostics_are_not_forwarded_as_client_validation_messages() -> anyhow::Result<()>
{
    use proto::notification_service_server::NotificationService;
    let app = crate::support::start_postgres_app().await?;
    let grpc = crate::support::grpc_server(app.state.clone());
    let user_id = uuid::Uuid::new_v4().to_string();
    grpc.create_notification(Request::new(proto::CreateNotificationRequest {
        user_id: user_id.clone(),
        r#type: "SYSTEM".into(),
        payload_json: "{}".into(),
        event_id: uuid::Uuid::new_v4().to_string(),
        actor_id: String::new(),
        occurred_at: String::new(),
    }))
    .await?;
    // Only this fresh, owned fixture is changed. Its read view raises a known
    // diagnostic, proving the owning PG error traverses the real repository.
    sqlx::raw_sql(
        r#"
        alter table notifications rename to notification_error_source;
        create function notification_private_failure() returns boolean
        language plpgsql as $$ begin
            raise exception 'PRIVATE_NOTIFICATION_DIAGNOSTIC_CANARY';
        end $$;
        create view notifications as
            select * from notification_error_source where notification_private_failure();
    "#,
    )
    .execute(&app.pool)
    .await?;
    let backend = sqlx::query("select * from notifications")
        .fetch_all(&app.pool)
        .await
        .unwrap_err();
    anyhow::ensure!(backend
        .to_string()
        .contains("PRIVATE_NOTIFICATION_DIAGNOSTIC_CANARY"));
    let status = grpc
        .list_notifications(Request::new(proto::ListNotificationsRequest {
            user_id,
            cursor: String::new(),
            limit: 10,
        }))
        .await
        .unwrap_err();
    let leaked = status
        .message()
        .contains("PRIVATE_NOTIFICATION_DIAGNOSTIC_CANARY");
    println!(
        "NOTIFICATION_ERROR_PROBE code={:?} leaked={leaked}",
        status.code()
    );
    let result = if status.code() == tonic::Code::Internal
        && status.message() == "Internal error"
        && !leaked
    {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "backend diagnostic was not safely classified: code={:?}, leaked={leaked}",
            status.code()
        ))
    };
    app.shutdown().await?;
    result
}

/// Build a request carrying the verified identity the interceptor attaches.
///
/// These tests call the generated trait method directly, which bypasses the
/// server interceptor. Attaching the extension keeps the authorization path
/// under test instead of skipping it.
fn as_caller<T>(message: T, caller: Workload) -> Request<T> {
    let mut request = Request::new(message);
    request.extensions_mut().insert(VerifiedCaller(caller));
    request
}

#[tokio::test]
async fn grpc_notification_contract_covers_create_list_mark_and_mark_all() {
    let app = crate::support::start_postgres_app().await.expect("app");
    let grpc = crate::support::grpc_server(app.state.clone());

    let created = proto::notification_service_server::NotificationService::create_notification(
        &grpc,
        Request::new(proto::CreateNotificationRequest {
            user_id: "11111111-1111-1111-1111-111111111111".into(),
            r#type: "POST_CREATED".into(),
            payload_json: "{\"postId\":\"p1\"}".into(),
            event_id: "aaaaaaaa-1111-1111-1111-111111111111".into(),
            actor_id: "22222222-2222-2222-2222-222222222222".into(),
            occurred_at: "2026-01-19T09:10:00Z".into(),
        }),
    )
    .await
    .expect("create")
    .into_inner();
    let created = created.notification.expect("notification");
    assert_eq!(created.r#type, "POST_CREATED");

    let list = proto::notification_service_server::NotificationService::list_notifications(
        &grpc,
        Request::new(proto::ListNotificationsRequest {
            user_id: "11111111-1111-1111-1111-111111111111".into(),
            cursor: String::new(),
            limit: 10,
        }),
    )
    .await
    .expect("list")
    .into_inner();
    assert_eq!(list.items.len(), 1);

    let mark = proto::notification_service_server::NotificationService::mark_notification_read(
        &grpc,
        Request::new(proto::MarkNotificationReadRequest {
            user_id: "11111111-1111-1111-1111-111111111111".into(),
            notification_id: created.id.clone(),
        }),
    )
    .await
    .expect("mark")
    .into_inner();
    assert!(mark.updated);
    assert!(!mark.notification.expect("notification").read_at.is_empty());

    let second = proto::notification_service_server::NotificationService::create_notification(
        &grpc,
        Request::new(proto::CreateNotificationRequest {
            user_id: "11111111-1111-1111-1111-111111111111".into(),
            r#type: "SYSTEM".into(),
            payload_json: "{\"message\":\"hello\"}".into(),
            event_id: "bbbbbbbb-1111-1111-1111-111111111111".into(),
            actor_id: String::new(),
            occurred_at: "2026-01-19T10:10:00Z".into(),
        }),
    )
    .await
    .expect("create2")
    .into_inner();
    assert_eq!(second.notification.expect("notification").r#type, "SYSTEM");

    let mark_all =
        proto::notification_service_server::NotificationService::mark_all_notifications_read(
            &grpc,
            Request::new(proto::MarkAllNotificationsReadRequest {
                user_id: "11111111-1111-1111-1111-111111111111".into(),
            }),
        )
        .await
        .expect("mark all")
        .into_inner();
    assert_eq!(mark_all.updated_count, 1);

    let deleted = proto::notification_service_server::NotificationService::delete_notification(
        &grpc,
        as_caller(
            proto::DeleteNotificationRequest {
                user_id: "11111111-1111-1111-1111-111111111111".into(),
                notification_id: created.id.clone(),
                actor_id: "11111111-1111-1111-1111-111111111111".into(),
                request_id: "grpc-delete-notification-1".into(),
                ip_address: "198.51.100.9".into(),
                user_agent: "notification-grpc-contract".into(),
            },
            Workload::Gateway,
        ),
    )
    .await
    .expect("delete notification")
    .into_inner();
    assert!(deleted.deleted);
    assert_eq!(deleted.notification_id, created.id);

    let list_after_delete =
        proto::notification_service_server::NotificationService::list_notifications(
            &grpc,
            Request::new(proto::ListNotificationsRequest {
                user_id: "11111111-1111-1111-1111-111111111111".into(),
                cursor: String::new(),
                limit: 10,
            }),
        )
        .await
        .expect("list after delete")
        .into_inner();
    assert!(list_after_delete
        .items
        .iter()
        .all(|item| item.id != created.id));

    drop(grpc);
    app.shutdown().await.expect("shutdown app");
}
