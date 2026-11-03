//! Operational listener only. Product RPC/HTTP mapping belongs to BE06 integration.
use axum::{extract::State, http::StatusCode, routing::get, Router};
use platform::readiness::{Probe, Readiness, DATABASE_PROBE_TIMEOUT};
use sqlx::PgPool;
pub fn readiness(pool: PgPool) -> Readiness {
    Readiness::new(Probe::new(
        "database",
        true,
        DATABASE_PROBE_TIMEOUT,
        move || {
            let pool = pool.clone();
            async move { sqlx::query("select 1").execute(&pool).await.is_ok() }
        },
    ))
}
pub fn router(readiness: Readiness) -> Router {
    Router::new()
        .route(
            "/livez",
            get(|| async { (StatusCode::OK, [("cache-control", "no-store")]) }),
        )
        .route("/readyz", get(ready))
        .with_state(readiness)
}
async fn ready(State(readiness): State<Readiness>) -> impl axum::response::IntoResponse {
    let report = readiness.report().await;
    (
        if report.ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        [("cache-control", "no-store")],
        axum::Json(report),
    )
}
