use super::{map_service_error, CurrentUser, GatewayError, RequestDeadline};
use crate::config::AppState;
use axum::extract::{Path, Query, State};
use axum::routing::{get, patch};
use axum::Extension;
use axum::{Json, Router};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct NotificationQuery {
    cursor: Option<String>,
    #[serde(default = "default_limit")]
    limit: i32,
}

fn default_limit() -> i32 {
    10
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/notifications", get(list_notifications))
        .route("/api/notifications/read-all", patch(mark_all_read))
        .route("/api/notifications/:notification_id/read", patch(mark_read))
        .with_state(state)
}

async fn list_notifications(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Query(query): Query<NotificationQuery>,
    deadline: Option<Extension<RequestDeadline>>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    Ok(Json(serde_json::to_value(
        state
            .notification_service
            .list_notifications_with_budget(
                current_user.user_id,
                query.cursor,
                query.limit,
                deadline.map(|Extension(deadline)| deadline.projection_budget()),
            )
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn mark_read(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(notification_id): Path<String>,
    deadline: Option<Extension<RequestDeadline>>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    Ok(Json(serde_json::to_value(
        state
            .notification_service
            .mark_read_with_budget(
                current_user.user_id,
                notification_id,
                deadline.map(|Extension(deadline)| deadline.projection_budget()),
            )
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn mark_all_read(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
) -> Result<Json<serde_json::Value>, GatewayError> {
    Ok(Json(serde_json::to_value(
        state
            .notification_service
            .mark_all_read(current_user.user_id)
            .await
            .map_err(map_service_error)?,
    )?))
}
