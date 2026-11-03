//! Public C05 room/ticket ingress. Identity is strict Access plus fresh owners.
use super::{concert_access::HttpConcertAccess, GatewayError};
use crate::{adapters::out::concert_world::WorldClientError, config::AppState};
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, OriginalUri, Path, Request, State},
    http::{header, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/concert/events/:event_id/rooms", get(rooms))
        .route("/api/concert/events/:event_id/world-tickets", post(ticket))
        .layer(DefaultBodyLimit::max(4096))
        .with_state(state)
}

async fn boundary_inner(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    let method = request.method();
    let protected = method == Method::POST
        && path.ends_with("/world-tickets")
        && path.starts_with("/api/concert/events/")
        || method == Method::GET
            && path.ends_with("/rooms")
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
    let intent_ok = headers
        .get("x-mcr-intent")
        .and_then(|value| value.to_str().ok())
        == Some("browser");
    if !origin_ok || !intent_ok {
        return GatewayError::forbidden("Browser intent required").into_response();
    }
    protect_response(request, next).await
}

pub fn router_with_boundary(state: Arc<AppState>) -> Router {
    router(state.clone()).layer(axum::middleware::from_fn_with_state(state, boundary_inner))
}

fn event(path: &str) -> Result<Uuid, GatewayError> {
    Uuid::parse_str(path)
        .ok()
        .filter(|value| !value.is_nil() && value.to_string() == path)
        .ok_or_else(|| GatewayError::bad_request("Invalid event"))
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
    }

    #[test]
    fn rooms_response_requires_exact_event_and_bounded_catalog_shape() {
        let event = Uuid::new_v4();
        let mut value = serde_json::json!({
            "event_id": event.to_string(),
            "default_room_id": Uuid::new_v4().to_string(),
            "items": [],
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
    body: Bytes,
    access: HttpConcertAccess,
) -> Result<Json<Value>, GatewayError> {
    let event = event(event_id.as_str())?;
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
        .map_err(|_| GatewayError::ServiceBusy)?;
    identity
        .check()
        .map_err(|_| GatewayError::gateway_timeout("Request timed out"))?;
    let value = client
        .rooms(&identity, access.borrowed()?, event)
        .await
        .map_err(world_error)?;
    access.check()?;
    validate_rooms(&value, event)?;
    Ok(Json(value))
}

async fn ticket(
    State(state): State<Arc<AppState>>,
    Path(event_id): Path<String>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    body: Bytes,
    access: HttpConcertAccess,
) -> Result<Json<Value>, GatewayError> {
    let event = event(event_id.as_str())?;
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
        .and_then(|value| Uuid::parse_str(value).ok())
        .filter(|value| !value.is_nil())
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
        .map_err(|_| GatewayError::ServiceBusy)?;
    identity
        .check()
        .map_err(|_| GatewayError::gateway_timeout("Request timed out"))?;
    let value = client
        .ticket(&identity, access.borrowed()?, event, body.to_vec())
        .await
        .map_err(world_error)?;
    access.check()?;
    validate_ticket(&value, event, room)?;
    Ok(Json(value))
}

fn validate_rooms(value: &Value, event: Uuid) -> Result<(), GatewayError> {
    if value["event_id"] != event.to_string()
        || value["default_room_id"].as_str().is_none()
        || !value["items"].is_array()
    {
        return Err(GatewayError::ServiceBusy);
    }
    Ok(())
}

fn validate_ticket(value: &Value, event: Uuid, room: Uuid) -> Result<(), GatewayError> {
    let ticket = value["ticket"].as_str().unwrap_or_default();
    if value["protocol_version"] != concert_application::world_ticket::PROTOCOL_VERSION
        || ticket.len() != concert_application::world_ticket::TICKET_ENCODED_LENGTH
        || !ticket
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        || value["event_id"] != event.to_string()
        || value["room_id"] != room.to_string()
        || value["ws_path"] != concert_application::world_ticket::WORLD_PATH
        || value["auth_timeout_ms"].as_u64() != Some(3000)
    {
        return Err(GatewayError::ServiceBusy);
    }
    Ok(())
}

fn world_error(error: WorldClientError) -> GatewayError {
    match error {
        WorldClientError::Expired => GatewayError::gateway_timeout("Request timed out"),
        WorldClientError::Denied => GatewayError::forbidden("Access is not permitted"),
        WorldClientError::NotFound => GatewayError::not_found("Resource unavailable"),
        WorldClientError::Full => GatewayError::conflict("Room full"),
        WorldClientError::VersionMismatch => GatewayError::conflict("Map version mismatch"),
        WorldClientError::Unavailable | WorldClientError::InvalidResponse => {
            GatewayError::ServiceBusy
        }
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
