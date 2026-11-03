pub mod jwks_routes;
pub mod oauth_routes;
pub mod routes;
pub mod server;
pub mod verification_routes;

use crate::config::{AppState, AuthMetrics};
use axum::extract::State;
use axum::http::header::{HeaderValue, CONTENT_TYPE};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    /// Password hashing is saturated; the caller should retry shortly.
    #[error("service busy")]
    Busy,
    #[error("dependency unavailable")]
    DependencyUnavailable,
    #[error("internal error")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::BadRequest(message) => (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "code": "BAD_REQUEST",
                    "message": message
                })),
            )
                .into_response(),
            AppError::Busy => {
                let mut response = (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({
                        "code": "AUTH_BUSY",
                        "message": "Service busy; retry later"
                    })),
                )
                    .into_response();
                response
                    .headers_mut()
                    .insert(axum::http::header::RETRY_AFTER, HeaderValue::from(1_u64));
                response
            }
            AppError::DependencyUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"code": "AUTH_DEPENDENCY_UNAVAILABLE", "message": "Dependency unavailable"})),
            ).into_response(),
            AppError::Internal(_error) => {
                tracing::error!(code = "AUTH_INTERNAL", "unhandled auth http error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "code": "INTERNAL_ERROR",
                        "message": "Internal error"
                    })),
                )
                    .into_response()
            }
        }
    }
}

/// Development HTTP adaptation: retain known client messages, never expose
/// arbitrary repository/provider/library details as a caller rejection.
pub(super) fn map_application_error(error: crate::application::errors::AuthError) -> AppError {
    use crate::application::presentation::{present_auth_error, AuthFailureKind};
    let dto = present_auth_error(Some(&error));
    match dto.kind {
        AuthFailureKind::Busy => AppError::Busy,
        AuthFailureKind::DependencyUnavailable => AppError::DependencyUnavailable,
        AuthFailureKind::Internal => AppError::Internal(error.into()),
        _ => AppError::BadRequest(dto.legacy_message.to_owned()),
    }
}

/// Operational surface that is always served: JWKS only.
///
/// Publishing keys is not a user-facing auth operation and the Gateway validates
/// tokens locally, so this listener must stay reachable for key rotation.
pub fn operational_router(state: Arc<AppState>) -> Router {
    jwks_routes::router(state)
}

/// Development-only application surface duplicating the Gateway auth routes.
///
/// Serving login, signup, reset, and OAuth here bypasses Gateway rate limiting,
/// client identity normalization, the durable request journal, and OAuth
/// state-cookie binding. Callers must opt in explicitly, and production rejects
/// the setting.
pub fn application_router(state: Arc<AppState>) -> Router {
    Router::new()
        .merge(routes::router(state.clone()))
        .merge(verification_routes::router(state.clone()))
        .merge(oauth_routes::router(state.clone()))
        .merge(jwks_routes::router(state))
}

/// The surface the production http listener serves.
pub fn router(state: Arc<AppState>) -> Router {
    if state.public_application_routes_enabled() {
        application_router(state)
    } else {
        operational_router(state)
    }
}

pub fn metrics_router(metrics_state: Arc<AuthMetrics>) -> Router {
    Router::new()
        .route("/metrics", get(metrics))
        .with_state(metrics_state)
}

async fn metrics(State(metrics): State<Arc<AuthMetrics>>) -> Response {
    (
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
        )],
        metrics.render_prometheus(),
    )
        .into_response()
}

#[cfg(test)]
mod presentation_tests {
    use super::*;
    use crate::application::errors::AuthError;
    use http_body_util::BodyExt;

    #[tokio::test]
    async fn development_http_failures_preserve_known_shapes_and_sanitize_dependencies() {
        for (error, code, message, status, retry) in [
            (
                AuthError::InvalidCredentials,
                "BAD_REQUEST",
                "Invalid credentials",
                StatusCode::BAD_REQUEST,
                false,
            ),
            (
                AuthError::ProviderFailure(
                    auth_domain::oauth_failure::OAuthProviderFailure::code_invalid(),
                ),
                "BAD_REQUEST",
                "OAuth code is invalid",
                StatusCode::BAD_REQUEST,
                false,
            ),
            (
                AuthError::Conflict("Email already registered"),
                "BAD_REQUEST",
                "Email already registered",
                StatusCode::BAD_REQUEST,
                false,
            ),
            (
                AuthError::Busy,
                "AUTH_BUSY",
                "Service busy; retry later",
                StatusCode::SERVICE_UNAVAILABLE,
                true,
            ),
            (
                AuthError::DependencyUnavailable,
                "AUTH_DEPENDENCY_UNAVAILABLE",
                "Dependency unavailable",
                StatusCode::SERVICE_UNAVAILABLE,
                false,
            ),
            (
                AuthError::Invariant("fixed internal policy diagnostic"),
                "INTERNAL_ERROR",
                "Internal error",
                StatusCode::INTERNAL_SERVER_ERROR,
                false,
            ),
            (
                AuthError::Unexpected,
                "INTERNAL_ERROR",
                "Internal error",
                StatusCode::INTERNAL_SERVER_ERROR,
                false,
            ),
        ] {
            let response = map_application_error(error).into_response();
            assert_eq!(response.status(), status);
            assert_eq!(
                response
                    .headers()
                    .get(axum::http::header::RETRY_AFTER)
                    .is_some(),
                retry
            );
            if retry {
                assert_eq!(response.headers()[axum::http::header::RETRY_AFTER], "1");
            }
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(value, serde_json::json!({"code":code,"message":message}));
            assert!(!String::from_utf8_lossy(&bytes).contains("private"));
        }
    }
}
