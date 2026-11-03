use super::{map_service_error, CurrentUser, GatewayError};
use crate::config::AppState;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use std::sync::Arc;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/aggregate/:route_key", get(aggregate))
        .with_state(state)
}

async fn aggregate(
    State(state): State<Arc<AppState>>,
    _current_user: CurrentUser,
    Path(route_key): Path<String>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    Ok(Json(serde_json::to_value(
        state
            .aggregation_service
            .aggregate(route_key)
            .await
            .map_err(map_service_error)?,
    )?))
}
