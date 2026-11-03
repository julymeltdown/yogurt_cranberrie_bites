//! Public C05 room/ticket ingress. Identity is strict Access plus fresh owners.
use super::{concert_access::HttpConcertAccess, GatewayError};
use crate::{adapters::out::concert_world::WorldClientError, config::AppState};
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, OriginalUri, Path, Request, State},
    http::{header, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

const C05_PROTOCOL_VERSION: &str = "1.0.0";
const C05_TICKET_LENGTH: usize = 43;
const C05_WORLD_PATH: &str = "/world-ws";

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route(
            "/api/concert/events/:event_id/rooms",
            get(rooms).layer(middleware::from_fn(protect_response)),
        )
        .route(
            "/api/concert/events/:event_id/world-tickets",
            post(ticket).layer(middleware::from_fn(protect_response)),
        )
        .layer(DefaultBodyLimit::max(4096))
        .layer(axum::middleware::from_fn(protect_error_response))
        .with_state(state)
}

async fn boundary_inner(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    let method = request.method();
    // C05 applies the browser mutation guard to ticket issuance. Room discovery
    // is a protected read and uses only the strict Access bearer context.
    let protected = method == Method::POST
        && path.ends_with("/world-tickets")
        && path.starts_with("/api/concert/events/");
    if !protected {
        return next.run(request).await;
    }
    let headers = request.headers();
    let origin_ok = headers.get_all(header::ORIGIN).iter().count() == 1
        && headers
            .get(header::ORIGIN)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|origin| state.browser_auth_origins.iter().any(|item| item == origin));
    let mut intents = headers.get_all("x-mcr-intent").iter();
    let intent_ok = intents.next().and_then(|value| value.to_str().ok()) == Some("browser")
        && intents.next().is_none();
    let reason = if !origin_ok {
        Some("ORIGIN_NOT_ALLOWED")
    } else if !intent_ok {
        Some("BROWSER_INTENT_REQUIRED")
    } else {
        None
    };
    if let Some(reason) = reason {
        let correlation = request
            .headers()
            .get("x-correlation-id")
            .and_then(|value| value.to_str().ok())
            .and_then(platform::correlation::normalize)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let mut response = Json(serde_json::json!({
            "code": "GATEWAY_403",
            "message": "Browser request denied",
            "traceId": correlation,
            "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "details": {"reason": reason},
        }))
        .into_response();
        *response.status_mut() = StatusCode::FORBIDDEN;
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response
            .headers_mut()
            .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
        return response;
    }
    protect_response(request, next).await
}

pub fn router_with_boundary(state: Arc<AppState>) -> Router {
    router(state.clone()).layer(axum::middleware::from_fn_with_state(state, boundary_inner))
}

fn canonical_uuid(path: &str, message: &'static str) -> Result<Uuid, GatewayError> {
    Uuid::parse_str(path)
        .ok()
        .filter(|value| !value.is_nil() && value.to_string() == path)
        .ok_or_else(|| GatewayError::bad_request(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_response_rejects_wrong_binding_and_noncanonical_secret() {
        let event = Uuid::new_v4();
        let room = Uuid::new_v4();
        let mut value = serde_json::json!({
            "protocol_version": "1.0.0",
            "ticket": "A".repeat(43),
            "event_id": event.to_string(),
            "room_id": room.to_string(),
            "map_id": "map_fixture",
            "map_revision": 1,
            "avatar_manifest_id": "avatar_fixture",
            "avatar_manifest_version": 1,
            "server_time": "2026-11-07T10:00:00Z",
            "expires_at": "2026-11-07T10:00:30Z",
            "ws_path": "/world-ws",
            "auth_timeout_ms": 3000
        });
        assert!(validate_ticket(&value, event, room).is_ok());
        value["room_id"] = Uuid::new_v4().to_string().into();
        assert!(validate_ticket(&value, event, room).is_err());
        value["room_id"] = room.to_string().into();
        value["ticket"] = "!".repeat(43).into();
        assert!(validate_ticket(&value, event, room).is_err());
        value["ticket"] = "A".repeat(43).into();
        value["expires_at"] = "2026-11-07T10:00:31Z".into();
        assert!(validate_ticket(&value, event, room).is_err());
        value["expires_at"] = "2026-11-07T10:00:30Z".into();
    }

    #[test]
    fn rooms_response_requires_exact_event_and_bounded_catalog_shape() {
        let event = Uuid::new_v4();
        let default = Uuid::new_v4();
        let mut value = serde_json::json!({
            "event_id": event.to_string(),
            "default_room_id": default.to_string(),
            "items": [{
                "room_id": default.to_string(),
                "label": "Main",
                "capacity": 50,
                "availability": "AVAILABLE"
            }],
            "server_time": "2026-11-07T10:00:00Z"
        });
        assert!(validate_rooms(&value, event).is_ok());
        value["event_id"] = Uuid::new_v4().to_string().into();
        assert!(validate_rooms(&value, event).is_err());
    }
}

async fn rooms(
    State(state): State<Arc<AppState>>,
    Path(event_id): Path<String>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    access: HttpConcertAccess,
    body: Bytes,
) -> Result<Response, GatewayError> {
    let event = canonical_uuid(event_id.as_str(), "Invalid event")?;
    if method != Method::GET || uri.query().is_some() || !body.is_empty() {
        return Err(GatewayError::bad_request("Invalid rooms request"));
    }
    let client = state
        .concert_world_client
        .as_ref()
        .ok_or(GatewayError::ServiceBusy)?
        .clone();
    let reader = state
        .concert_identity
        .as_ref()
        .ok_or(GatewayError::ServiceBusy)?;
    let identity = reader
        .read(access.borrowed()?, None)
        .await
        .map_err(concert_identity_error)?;
    identity
        .check()
        .map_err(|_| GatewayError::gateway_timeout("Request timed out"))?;
    let value = match client.rooms(&identity, access.borrowed()?, event).await {
        Ok(value) => value,
        Err(error) => return Ok(c05_reason(error)),
    };
    access.check()?;
    if validate_rooms(&value, event).is_err() {
        return Ok(c05_invalid_response());
    }
    Ok(Json(value).into_response())
}

async fn ticket(
    State(state): State<Arc<AppState>>,
    Path(event_id): Path<String>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    access: HttpConcertAccess,
    body: Bytes,
) -> Result<Response, GatewayError> {
    let event = canonical_uuid(event_id.as_str(), "Invalid event")?;
    if method != Method::POST || uri.query().is_some() || body.is_empty() || body.len() > 4096 {
        return Err(GatewayError::bad_request("Invalid world ticket request"));
    }
    let input: Value = serde_json::from_slice(&body)
        .map_err(|_| GatewayError::bad_request("Invalid world ticket request"))?;
    let input = input
        .as_object()
        .filter(|value| value.len() == 5)
        .ok_or_else(|| GatewayError::bad_request("Invalid world ticket request"))?;
    let room = input
        .get("room_id")
        .and_then(Value::as_str)
        .and_then(|value| canonical_uuid(value, "Invalid world ticket request").ok())
        .ok_or_else(|| GatewayError::bad_request("Invalid world ticket request"))?;
    let client = state
        .concert_world_client
        .as_ref()
        .ok_or(GatewayError::ServiceBusy)?
        .clone();
    let reader = state
        .concert_identity
        .as_ref()
        .ok_or(GatewayError::ServiceBusy)?;
    let identity = reader
        .read(access.borrowed()?, None)
        .await
        .map_err(concert_identity_error)?;
    identity
        .check()
        .map_err(|_| GatewayError::gateway_timeout("Request timed out"))?;
    let value = match client
        .ticket(&identity, access.borrowed()?, event, body.to_vec())
        .await
    {
        Ok(value) => value,
        Err(error) => return Ok(c05_reason(error)),
    };
    access.check()?;
    if validate_ticket(&value, event, room).is_err() {
        return Ok(c05_invalid_response());
    }
    Ok(Json(value).into_response())
}

fn validate_rooms(value: &Value, event: Uuid) -> Result<(), GatewayError> {
    let event_text = event.to_string();
    let Some(value) = value.as_object().filter(|value| value.len() == 4) else {
        return Err(GatewayError::ServiceBusy);
    };
    let event_id = value.get("event_id").and_then(Value::as_str);
    let default = value
        .get("default_room_id")
        .and_then(Value::as_str)
        .ok_or(GatewayError::ServiceBusy)?;
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .ok_or(GatewayError::ServiceBusy)?;
    let server_time = value
        .get("server_time")
        .and_then(Value::as_str)
        .filter(|value| value.ends_with('Z'))
        .ok_or(GatewayError::ServiceBusy)?;
    if chrono::DateTime::parse_from_rfc3339(server_time).is_err()
        || event_id != Some(event_text.as_str())
        || canonical_uuid(default, "Invalid event").is_err()
    {
        return Err(GatewayError::ServiceBusy);
    }
    if items.is_empty() || items.len() > 16 {
        return Err(GatewayError::ServiceBusy);
    }
    let mut ids = std::collections::HashSet::new();
    for item in items {
        let Some(item) = item.as_object().filter(|item| item.len() == 4) else {
            return Err(GatewayError::ServiceBusy);
        };
        let room = item
            .get("room_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let label = item
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let capacity = item.get("capacity").and_then(Value::as_u64);
        let availability = item
            .get("availability")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if canonical_uuid(room, "Invalid event").is_err()
            || !ids.insert(room)
            || label.trim().is_empty()
            || label.chars().count() > 48
            || label.chars().any(char::is_control)
            || !matches!(capacity, Some(1..=50))
            || !matches!(availability, "AVAILABLE" | "FULL" | "UNAVAILABLE")
        {
            return Err(GatewayError::ServiceBusy);
        }
    }
    if !ids.contains(default) {
        return Err(GatewayError::ServiceBusy);
    }
    Ok(())
}

fn validate_ticket(value: &Value, event: Uuid, room: Uuid) -> Result<(), GatewayError> {
    let Some(value) = value.as_object().filter(|value| value.len() == 12) else {
        return Err(GatewayError::ServiceBusy);
    };
    let ticket = value["ticket"].as_str().unwrap_or_default();
    let server_time = value["server_time"]
        .as_str()
        .filter(|value| value.ends_with('Z'))
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .ok_or(GatewayError::ServiceBusy)?;
    let expires_at = value["expires_at"]
        .as_str()
        .filter(|value| value.ends_with('Z'))
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .ok_or(GatewayError::ServiceBusy)?;
    if value["protocol_version"] != C05_PROTOCOL_VERSION
        || ticket.len() != C05_TICKET_LENGTH
        || !ticket
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        || value["event_id"] != event.to_string()
        || value["room_id"] != room.to_string()
        || value["ws_path"] != C05_WORLD_PATH
        || value["auth_timeout_ms"].as_u64() != Some(3000)
        || !matches!(
            value["map_revision"].as_u64(),
            Some(1..=9_007_199_254_740_991)
        )
        || !matches!(
            value["avatar_manifest_version"].as_u64(),
            Some(1..=9_007_199_254_740_991)
        )
        || !value["map_id"].as_str().is_some_and(valid_asset)
        || !value["avatar_manifest_id"]
            .as_str()
            .is_some_and(valid_asset)
        || expires_at <= server_time
        || expires_at - server_time > chrono::Duration::seconds(30)
        || !value["event_id"].as_str().is_some_and(valid_uuid)
        || !value["room_id"].as_str().is_some_and(valid_uuid)
    {
        return Err(GatewayError::ServiceBusy);
    }
    Ok(())
}

fn valid_uuid(value: &str) -> bool {
    canonical_uuid(value, "Invalid identifier").is_ok()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct C05Reason(&'static str);

fn valid_asset(value: &str) -> bool {
    let bytes = value.as_bytes();
    (1..=64).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
}

fn c05_reason(error: WorldClientError) -> Response {
    let (status, reason, message) = match error {
        WorldClientError::Expired => (
            StatusCode::GATEWAY_TIMEOUT,
            "DEADLINE_EXCEEDED",
            "Request timed out",
        ),
        WorldClientError::Denied => (
            StatusCode::FORBIDDEN,
            "PERMISSION_DENIED",
            "Access is not permitted",
        ),
        WorldClientError::NotFound => (
            StatusCode::NOT_FOUND,
            "RESOURCE_UNAVAILABLE",
            "Resource unavailable",
        ),
        WorldClientError::Full => (
            StatusCode::CONFLICT,
            "RESOURCE_UNAVAILABLE",
            "Room unavailable",
        ),
        WorldClientError::VersionMismatch => (
            StatusCode::CONFLICT,
            "MAP_VERSION_MISMATCH",
            "Map version mismatch",
        ),
        WorldClientError::Unavailable | WorldClientError::InvalidResponse => (
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
            "Service busy; retry later",
        ),
    };
    let mut response = (status, Json(serde_json::json!({"message": message}))).into_response();
    response.extensions_mut().insert(C05Reason(reason));
    response
}

fn c05_invalid_response() -> Response {
    let mut response = (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({"message": "Service busy; retry later"})),
    )
        .into_response();
    response
        .extensions_mut()
        .insert(C05Reason("DEPENDENCY_UNAVAILABLE"));
    response
}

fn concert_identity_error(
    error: crate::adapters::out::concert_identity::ConcertIdentityReadError,
) -> GatewayError {
    use crate::adapters::out::concert_identity::ConcertIdentityReadError;
    match error {
        ConcertIdentityReadError::AccessExpired | ConcertIdentityReadError::DeadlineExceeded => {
            GatewayError::gateway_timeout("Request timed out")
        }
        ConcertIdentityReadError::NotEligible => GatewayError::forbidden("Access is not permitted"),
        _ => GatewayError::ServiceBusy,
    }
}

#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn full_and_stale_release_reasons_remain_distinct() {
        assert_eq!(
            c05_reason(WorldClientError::Full).status(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            c05_reason(WorldClientError::VersionMismatch).status(),
            StatusCode::CONFLICT
        );
    }
}

pub async fn protect_response(request: Request, next: Next) -> Response {
    let protected = request.uri().path().contains("/world-tickets")
        || (request.uri().path().contains("/events/") && request.uri().path().ends_with("/rooms"));
    let mut response = next.run(request).await;
    if protected {
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response
            .headers_mut()
            .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    }
    response
}

/// C05 errors use the shared CoreErrorResponse shape. This remains scoped to
/// the isolated world router and does not change unrelated Gateway responses.
pub async fn protect_error_response(request: Request, next: Next) -> Response {
    let protected = request.uri().path().contains("/world-tickets")
        || (request.uri().path().contains("/events/") && request.uri().path().ends_with("/rooms"));
    let response = next.run(request).await;
    if !protected || (!response.status().is_client_error() && !response.status().is_server_error())
    {
        return response;
    }
    let status = response.status();
    let reason = match response.extensions().get::<C05Reason>() {
        Some(C05Reason("MAP_VERSION_MISMATCH")) => "MAP_VERSION_MISMATCH",
        Some(C05Reason("RESOURCE_UNAVAILABLE")) => "RESOURCE_UNAVAILABLE",
        Some(C05Reason("DEADLINE_EXCEEDED")) => "DEADLINE_EXCEEDED",
        Some(C05Reason("DEPENDENCY_UNAVAILABLE")) => "DEPENDENCY_UNAVAILABLE",
        Some(C05Reason("PERMISSION_DENIED")) => "PERMISSION_DENIED",
        _ => match status.as_u16() {
            400 | 405 | 422 => "INVALID_INPUT",
            401 => "AUTHENTICATION_REQUIRED",
            403 => "PERMISSION_DENIED",
            404 => "RESOURCE_UNAVAILABLE",
            409 => "RESOURCE_UNAVAILABLE",
            410 => "EVENT_READ_ONLY",
            413 => "PAYLOAD_TOO_LARGE",
            415 => "INVALID_MEDIA_TYPE",
            429 => "RATE_LIMITED",
            502 | 503 => "DEPENDENCY_UNAVAILABLE",
            504 => "DEADLINE_EXCEEDED",
            _ => "INTERNAL_ERROR",
        },
    };
    let status = if status == StatusCode::METHOD_NOT_ALLOWED {
        StatusCode::BAD_REQUEST
    } else {
        status
    };
    let mut safe = Json(serde_json::json!({
        "code": format!("GATEWAY_{}", status.as_u16()),
        "message": "The request could not be completed.",
        "details": {"reason": reason}
    }))
    .into_response();
    *safe.status_mut() = status;
    safe.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    safe.headers_mut()
        .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    safe
}
