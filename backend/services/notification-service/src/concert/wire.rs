//! Exact C04 operation boundary. Gateway owns parsing/authentication/browser guards.
use super::{FencedInboxAuthority, PgConcertInbox};
use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use concert_application::notification::{Context, Query};
use concert_domain::notification::{NotificationError, Result};
use serde_json::{json, Value};
use uuid::Uuid;

pub const PREFIX: &str = "/api/concert/events/{eventId}/me/notifications";
pub const OPERATIONS: [(&str, &str, &str); 4] = [
    (
        "listConcertNotifications",
        "GET",
        "/api/concert/events/{eventId}/me/notifications",
    ),
    (
        "getConcertNotificationUnreadCount",
        "GET",
        "/api/concert/events/{eventId}/me/notifications/unread-count",
    ),
    (
        "markConcertNotification",
        "PATCH",
        "/api/concert/events/{eventId}/me/notifications/{notificationId}/read",
    ),
    (
        "markAllConcertNotifications",
        "PATCH",
        "/api/concert/events/{eventId}/me/notifications/read-all",
    ),
];
pub enum Operation {
    List(Query),
    UnreadCount,
    MarkRead {
        notification_id: Uuid,
        expected_revision: i64,
        idempotency_key: Uuid,
    },
    MarkAll {
        idempotency_key: Uuid,
    },
}
pub async fn execute<A: FencedInboxAuthority>(
    service: &PgConcertInbox<A>,
    context: Context,
    operation: Operation,
) -> Result<Value> {
    let value = match operation {
        Operation::List(query) => serde_json::to_value(service.list(context, query).await?),
        Operation::UnreadCount => serde_json::to_value(service.unread_count(context).await?),
        Operation::MarkRead {
            notification_id,
            expected_revision,
            idempotency_key,
        } => serde_json::to_value(
            service
                .mark_read(context, notification_id, expected_revision, idempotency_key)
                .await?,
        ),
        Operation::MarkAll { idempotency_key } => {
            serde_json::to_value(service.mark_all_read(context, idempotency_key).await?)
        }
    };
    value.map_err(|_| NotificationError::Unavailable)
}
pub async fn response<A: FencedInboxAuthority>(
    service: &PgConcertInbox<A>,
    context: Context,
    operation: Operation,
) -> Response {
    let mut response = match execute(service, context, operation).await {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err(e) => {
            let (status, reason) = failure(e);
            (status,Json(json!({"code":format!("GATEWAY_{}",status.as_u16()),"message":"The requested action is unavailable.",
                "traceId":Uuid::new_v4().to_string(),"timestamp":Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis,true),"details":{"reason":reason}}))).into_response()
        }
    };
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}
pub fn failure(e: NotificationError) -> (StatusCode, &'static str) {
    match e {
        NotificationError::InvalidInput => (StatusCode::BAD_REQUEST, "INVALID_INPUT"),
        NotificationError::ResourceUnavailable => (StatusCode::NOT_FOUND, "RESOURCE_UNAVAILABLE"),
        NotificationError::PermissionDenied => (StatusCode::FORBIDDEN, "PERMISSION_DENIED"),
        NotificationError::Unavailable => {
            (StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE")
        }
        NotificationError::VersionConflict => (StatusCode::CONFLICT, "VERSION_CONFLICT"),
        NotificationError::IdempotencyConflict => (StatusCode::CONFLICT, "IDEMPOTENCY_CONFLICT"),
        NotificationError::IdempotencyExpired => (StatusCode::CONFLICT, "IDEMPOTENCY_EXPIRED"),
    }
}
