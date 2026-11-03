//! C03 browser transport policy. Credentials and canonical identity remain Auth-owned.
use super::{auth_routes, refresh_routes, verification_routes, CORRELATION_ID};
use crate::config::AppState;
use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, Method, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use chrono::{SecondsFormat, Utc};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

pub fn router(state: Arc<AppState>) -> Router {
    // OAuth, availability and parity/shadow routes are deliberately not composed.
    auth_routes::browser_router(state.clone())
        .merge(refresh_routes::browser_router(state.clone()))
        .merge(verification_routes::browser_router(state))
        .route("/api/auth/capabilities", get(capabilities))
}

async fn capabilities() -> Json<serde_json::Value> {
    Json(json!({
        "contract_version": "0.1.0",
        "providers": [
            {"provider": "EMAIL", "enabled": true, "disabled_reason": null},
            {"provider": "GOOGLE", "enabled": false, "disabled_reason": "HANDOFF_NOT_QUALIFIED"},
            {"provider": "APPLE", "enabled": false, "disabled_reason": "HANDOFF_NOT_QUALIFIED"},
            {"provider": "X", "enabled": false, "disabled_reason": "HANDOFF_NOT_QUALIFIED"}
        ],
        "server_time": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
    }))
}

fn auth_surface(path: &str) -> bool {
    path == "/api/auth"
        || path.starts_with("/api/auth/")
        || path == "/auth"
        || path.starts_with("/auth/")
}

fn exactly_one(headers: &HeaderMap, name: &str) -> Option<String> {
    let mut values = headers.get_all(name).iter();
    let first = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        return None;
    }
    Some(first.to_owned())
}

fn denial(reason: &'static str, correlation: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({
            "code": "GATEWAY_403", "message": "Browser request denied",
            "traceId": correlation,
            "timestamp": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            "details": {"reason": reason}
        })),
    )
        .into_response()
}

/// Run outside admission, telemetry, JSON extraction, cookies and downstream calls.
/// Installed at both composed-router and executable-transport boundaries; idempotent.
pub async fn boundary(
    State(origins): State<Vec<String>>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let mutation = !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    );
    let capabilities =
        request.uri().path() == "/api/auth/capabilities" && request.method() == Method::GET;
    // All public Gateway mutations are browser commands. Internal service traffic
    // uses authenticated gRPC, never an unguarded HTTP exception here.
    if !auth_surface(request.uri().path()) && !mutation {
        return next.run(request).await;
    }
    let correlation = exactly_one(request.headers(), "x-correlation-id")
        .and_then(|value| platform::correlation::normalize(&value))
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    request.headers_mut().insert(
        "x-correlation-id",
        HeaderValue::from_str(&correlation).expect("normalized correlation"),
    );
    let rejection = if mutation {
        match exactly_one(request.headers(), "origin") {
            Some(origin) if origins.iter().any(|allowed| allowed == &origin) => {
                (exactly_one(request.headers(), "x-mcr-intent").as_deref() != Some("browser"))
                    .then_some("BROWSER_INTENT_REQUIRED")
            }
            _ => Some("ORIGIN_NOT_ALLOWED"),
        }
    } else {
        None
    };
    let mut response = CORRELATION_ID
        .scope(correlation.clone(), async {
            match rejection {
                Some(reason) => denial(reason, &correlation),
                None => next.run(request).await,
            }
        })
        .await;
    if capabilities && (response.status().is_client_error() || response.status().is_server_error())
    {
        let status = response.status();
        let reason = match status.as_u16() {
            400 | 422 => "INVALID_INPUT",
            401 => "AUTHENTICATION_REQUIRED",
            403 => "PERMISSION_DENIED",
            404 => "RESOURCE_UNAVAILABLE",
            409 => "VERSION_CONFLICT",
            413 => "PAYLOAD_TOO_LARGE",
            415 => "INVALID_MEDIA_TYPE",
            429 => "RATE_LIMITED",
            502 | 503 => "DEPENDENCY_UNAVAILABLE",
            504 => "DEADLINE_EXCEEDED",
            _ => "INTERNAL_ERROR",
        };
        let retry_after = response.headers().get(header::RETRY_AFTER).cloned();
        response = (status, Json(json!({
            "code": format!("GATEWAY_{}", status.as_str()), "message": "Request could not be completed",
            "traceId": correlation, "timestamp": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            "details": {"reason": reason}
        }))).into_response();
        if let Some(value) = retry_after {
            response.headers_mut().insert(header::RETRY_AFTER, value);
        }
    }
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    headers.insert(
        "x-correlation-id",
        HeaderValue::from_str(&correlation).expect("normalized correlation"),
    );
    response
}
