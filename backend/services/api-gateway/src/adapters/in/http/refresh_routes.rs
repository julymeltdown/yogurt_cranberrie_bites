use super::{
    append_set_cookie, build_refresh_cookie, map_service_error, resolve_cookie,
    strip_refresh_token, GatewayError, REFRESH_COOKIE_NAME,
};
use crate::config::AppState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use std::sync::Arc;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/auth/refresh", post(refresh))
        .with_state(state)
}

async fn refresh(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let refresh_token = resolve_cookie(&headers, REFRESH_COOKIE_NAME)
        .ok_or_else(|| GatewayError::unauthorized("Missing refresh token"))?;
    let response = state
        .auth_service
        .refresh(refresh_token)
        .await
        .map_err(map_service_error)?;
    let mut response_headers = HeaderMap::new();
    append_set_cookie(
        &mut response_headers,
        build_refresh_cookie(&state, &response.refresh_token),
    )?;
    Ok((
        StatusCode::OK,
        response_headers,
        Json(strip_refresh_token(response)),
    )
        .into_response())
}
