pub mod replay_routes;

use crate::config::{AppState, RuntimeMode};
use anyhow::Context;
use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::http::HeaderValue;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use chrono::Utc;
use std::sync::Arc;
use tokio::net::TcpListener;

pub fn router(state: Arc<AppState>, runtime_mode: RuntimeMode) -> Router {
    let router = Router::new();
    if runtime_mode.allows_admin_http_routes() {
        router.merge(replay_routes::router(state))
    } else {
        router
    }
    .layer(platform::trace_context::TraceContextLayer)
}

pub fn metrics_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/metrics", get(metrics))
        .with_state(state)
}

pub async fn bind_listeners(
    application_addr: std::net::SocketAddr,
    metrics_addr: std::net::SocketAddr,
) -> anyhow::Result<(TcpListener, TcpListener)> {
    let application = TcpListener::bind(application_addr)
        .await
        .with_context(|| format!("failed to bind application listener at {application_addr}"))?;
    let metrics = TcpListener::bind(metrics_addr)
        .await
        .with_context(|| format!("failed to bind metrics listener at {metrics_addr}"))?;
    Ok((application, metrics))
}

async fn metrics(State(state): State<Arc<AppState>>) -> Result<Response, axum::http::StatusCode> {
    let snapshot = state
        .outbox
        .metrics_snapshot(Utc::now())
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
        )],
        state.metrics.render_prometheus(&snapshot),
    )
        .into_response())
}
