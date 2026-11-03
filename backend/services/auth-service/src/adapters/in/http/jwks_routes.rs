use crate::config::AppState;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use std::sync::Arc;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/.well-known/jwks.json", get(jwks))
        .with_state(state)
}

async fn jwks(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(crate::adapters::r#in::jwks::public_jwks(
        &state.login.jwk_set(),
    ))
}
