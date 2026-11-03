//! Public C03 bootstrap ingress. Identity comes only from the strict access
//! extractor and fresh signed owner reads; no browser field selects the actor.
use super::{concert_access::HttpConcertAccess, GatewayError};
use crate::{adapters::out::concert_client::BootstrapError, config::AppState};
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, OriginalUri, Path, Request, State},
    http::{header, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use std::sync::Arc;
use uuid::Uuid;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/concert/events/:event_id/bootstrap", get(bootstrap))
        .layer(DefaultBodyLimit::max(0))
        .with_state(state)
}

async fn bootstrap(
    State(state): State<Arc<AppState>>,
    Path(event): Path<String>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    access: HttpConcertAccess,
    body: Bytes,
) -> Result<Json<serde_json::Value>, GatewayError> {
    let event_id = Uuid::parse_str(&event)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == event)
        .ok_or_else(|| GatewayError::bad_request("Invalid event"))?;
    if method != Method::GET || uri.query().is_some() || !body.is_empty() {
        return Err(GatewayError::bad_request("Invalid bootstrap request"));
    }
    let client = state
        .concert_client
        .as_ref()
        .ok_or(GatewayError::ServiceBusy)?;
    let reader = state
        .concert_identity
        .as_ref()
        .ok_or(GatewayError::ServiceBusy)?;
    let result = Box::pin(client.bootstrap(reader, access.borrowed()?, event_id, Uuid::new_v4()))
        .await
        .map_err(|error| match error {
            BootstrapError::Denied => GatewayError::forbidden("Access is not permitted"),
            BootstrapError::NotFound => GatewayError::not_found("Resource unavailable"),
            BootstrapError::Expired => GatewayError::gateway_timeout("Request timed out"),
            BootstrapError::Identity
            | BootstrapError::Unavailable
            | BootstrapError::InvalidResponse => GatewayError::ServiceBusy,
        })?;
    access.check()?;
    Ok(Json(result))
}

fn is_bootstrap(path: &str) -> bool {
    path.strip_prefix("/api/concert/events/")
        .and_then(|s| s.strip_suffix("/bootstrap"))
        .is_some_and(|id| !id.is_empty() && !id.contains('/'))
}

/// Runs outside request timeouts and transport guards too. Replace inherited
/// error bodies with the frozen C03 shape without reading or copying diagnostics.
pub async fn protect_response(request: Request, next: Next) -> Response {
    let protected = is_bootstrap(request.uri().path());
    let mut response = next.run(request).await;
    if !protected {
        return response;
    }
    if response.status().is_client_error() || response.status().is_server_error() {
        let status = response.status();
        let reason = match status.as_u16() {
            400 | 405 | 422 => "INVALID_INPUT",
            401 => "AUTHENTICATION_REQUIRED",
            403 => "PERMISSION_DENIED",
            404 => "RESOURCE_UNAVAILABLE",
            413 => "PAYLOAD_TOO_LARGE",
            415 => "INVALID_MEDIA_TYPE",
            429 => "RATE_LIMITED",
            502 | 503 => "DEPENDENCY_UNAVAILABLE",
            504 => "DEADLINE_EXCEEDED",
            _ => "INTERNAL_ERROR",
        };
        // No 405 is defined by C03. A non-GET request is invalid input.
        let status = if status == StatusCode::METHOD_NOT_ALLOWED {
            StatusCode::BAD_REQUEST
        } else {
            status
        };
        let mut details = serde_json::json!({"reason": reason});
        if status == StatusCode::TOO_MANY_REQUESTS {
            if let Some(ms) = response
                .headers()
                .get(header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .and_then(|s| s.checked_mul(1000))
                .filter(|ms| (1..=86_400_000).contains(ms))
            {
                details["retry_after_ms"] = ms.into();
            }
        }
        let safe = Json(serde_json::json!({
            "code": format!("GATEWAY_{}", status.as_u16()),
            "message": "The request could not be completed.",
            "details": details,
        }))
        .into_response();
        let (mut parts, _) = response.into_parts();
        parts.status = status;
        parts.headers.remove(header::CONTENT_LENGTH);
        parts.headers.remove(header::CONTENT_ENCODING);
        parts.headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        response = Response::from_parts(parts, safe.into_body());
    }
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    response
        .headers_mut()
        .append(header::VARY, HeaderValue::from_static("Authorization"));
    response
}
