use super::{CurrentUser, GatewayError};
use crate::application::dto::operation_view_dto::CallerOperationStatusResponseDto;
use crate::config::AppState;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use std::sync::Arc;
use uuid::Uuid;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/operations/:operation_id", get(operation_status))
        .with_state(state)
}

async fn operation_status(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(operation_id): Path<Uuid>,
) -> Result<Response, GatewayError> {
    let status = state
        .operation_journal_service
        .caller_operation_status(operation_id, &current_user.user_id)
        .await
        .map_err(|error| GatewayError::Internal(error.into()))?
        .ok_or_else(|| GatewayError::not_found("Unknown operation id"))?;

    let mut response = axum::Json(CallerOperationStatusResponseDto::from(status)).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
