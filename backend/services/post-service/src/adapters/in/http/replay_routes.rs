use crate::application::dto::post_dto::{ReplayRequestDto, ReplayResponseDto};
use crate::config::AppState;
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use std::sync::Arc;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/admin/events/replay", post(replay_events))
        .with_state(state)
}

async fn replay_events(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ReplayRequestDto>,
) -> Result<Json<ReplayResponseDto>, axum::http::StatusCode> {
    state
        .replay_events
        .execute(request)
        .await
        .map(Json)
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)
}
