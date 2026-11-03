use notification_service::adapters::r#in::grpc::server::{proto, NotificationGrpcServer};
use proto::notification_service_server::NotificationService;
use tonic::{Code, Request, Response, Status};
use uuid::Uuid;

#[tokio::test]
async fn create_notification_maps_validation_errors_to_invalid_argument_status() {
    let server = grpc_server().await;

    let status = expect_status(
        server
            .create_notification(Request::new(proto::CreateNotificationRequest {
                user_id: Uuid::new_v4().to_string(),
                r#type: "unknown".to_owned(),
                payload_json: "{}".to_owned(),
                event_id: String::new(),
                actor_id: String::new(),
                occurred_at: String::new(),
            }))
            .await,
    );

    assert_eq!(status.code(), Code::InvalidArgument);
    assert_eq!(status.message(), "Unsupported notification type");
}

#[tokio::test]
async fn list_notifications_maps_invalid_cursor_to_invalid_argument_status() {
    let server = grpc_server().await;

    let status = expect_status(
        server
            .list_notifications(Request::new(proto::ListNotificationsRequest {
                user_id: Uuid::new_v4().to_string(),
                cursor: "broken-cursor".to_owned(),
                limit: 20,
            }))
            .await,
    );

    assert_eq!(status.code(), Code::InvalidArgument);
    assert_eq!(status.message(), "Invalid cursor format");
}

#[tokio::test]
async fn mark_notification_read_maps_malformed_user_to_invalid_argument_status() {
    let server = grpc_server().await;

    let status = expect_status(
        server
            .mark_notification_read(Request::new(proto::MarkNotificationReadRequest {
                user_id: "not-a-uuid".to_owned(),
                notification_id: Uuid::new_v4().to_string(),
            }))
            .await,
    );

    assert_eq!(status.code(), Code::InvalidArgument);
    assert_eq!(status.message(), "User ID is required");
}

#[tokio::test]
async fn mark_all_notifications_read_maps_blank_user_to_invalid_argument_status() {
    let server = grpc_server().await;

    let status = expect_status(
        server
            .mark_all_notifications_read(Request::new(proto::MarkAllNotificationsReadRequest {
                user_id: " ".to_owned(),
            }))
            .await,
    );

    assert_eq!(status.code(), Code::InvalidArgument);
    assert_eq!(status.message(), "User ID is required");
}

#[tokio::test]
async fn delete_notification_maps_malformed_notification_id_to_invalid_argument_status() {
    let server = grpc_server().await;

    let status = expect_status(
        server
            .delete_notification(caller_request(proto::DeleteNotificationRequest {
                user_id: Uuid::new_v4().to_string(),
                notification_id: "not-a-uuid".to_owned(),
                actor_id: Uuid::new_v4().to_string(),
                request_id: " ".to_owned(),
                ip_address: " 10.0.0.10 ".to_owned(),
                user_agent: String::new(),
            }))
            .await,
    );

    assert_eq!(status.code(), Code::InvalidArgument);
    assert_eq!(status.message(), "Notification ID is required");
}

async fn grpc_server() -> NotificationGrpcServer {
    NotificationGrpcServer::new(crate::support::lazy_app_state().await)
}

#[tokio::test]
async fn selected_original_write_requires_guard_seal_even_with_post_identity_and_metadata() {
    let server =
        NotificationGrpcServer::requiring_claimed_admission(crate::support::lazy_app_state().await);
    let mut request = Request::new(proto::CreateNotificationRequest {
        user_id: Uuid::new_v4().to_string(),
        r#type: "POST_LIKED".into(),
        payload_json: "{}".into(),
        event_id: Uuid::new_v4().to_string(),
        actor_id: Uuid::new_v4().to_string(),
        occurred_at: "2026-10-14T00:00:00Z".into(),
    });
    request
        .extensions_mut()
        .insert(platform::grpc_auth::VerifiedCaller(
            platform::grpc_auth::Workload::Post,
        ));
    request.metadata_mut().insert(
        platform::actor_delegation::notification::HEADER,
        "unverified-metadata-is-not-admission".parse().unwrap(),
    );
    let status = expect_status(server.create_notification(request).await);
    assert_eq!(status.code(), Code::PermissionDenied);
    assert_eq!(status.message(), "Notification admission required");
}

/// Attach the verified identity the server interceptor would set, so a direct
/// trait-method call still exercises the authorization path.
fn caller_request<T>(message: T) -> Request<T> {
    let mut request = Request::new(message);
    request
        .extensions_mut()
        .insert(platform::grpc_auth::VerifiedCaller(
            platform::grpc_auth::Workload::Gateway,
        ));
    request
}

fn expect_status<T>(result: Result<Response<T>, Status>) -> Status {
    match result {
        Ok(_) => panic!("expected gRPC status"),
        Err(status) => status,
    }
}
