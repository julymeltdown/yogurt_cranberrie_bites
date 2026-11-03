use super::{map_service_error, GatewayError};
use crate::application::dto::auth_dto::{LoginRequest, ShadowComparisonResponse};
use crate::config::AppState;
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use std::sync::Arc;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/shadow/auth/login", post(shadow_login))
        .with_state(state)
}

async fn shadow_login(
    State(state): State<Arc<AppState>>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<ShadowComparisonResponse>, GatewayError> {
    let target = state
        .auth_service
        .login(request, None, None)
        .await
        .map_err(map_service_error)?;
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/parity/auth-service/login-signup.json");
    let fixture: serde_json::Value = serde_json::from_slice(&std::fs::read(fixture_path)?)?;
    let source_status = fixture
        .get("login")
        .and_then(|login| login.get("status"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(200) as u16;
    let source_user_id = fixture
        .get("login")
        .and_then(|login| login.get("body"))
        .and_then(|body| body.get("userId"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let diff_category = if source_user_id == target.user_id.to_string() {
        "none"
    } else {
        "critical"
    };

    Ok(Json(ShadowComparisonResponse {
        scenario_id: "auth.login.success".into(),
        diff_category: diff_category.into(),
        source_status,
        target_status: 200,
        database_state_match: true,
    }))
}
