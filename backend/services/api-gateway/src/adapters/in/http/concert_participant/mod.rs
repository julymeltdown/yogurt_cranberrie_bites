//! C03 public routes. Root supplies the actual client and outer transport layers.
use super::{concert_access::HttpConcertAccess, GatewayError};
use crate::{
    adapters::out::concert_participant::{
        failure_status, GrpcConcertParticipantClient, Operation, ParticipantHttpRequest, MAX_BODY,
    },
    config::AppState,
};
use axum::{
    body::{to_bytes, Body},
    extract::{FromRef, OriginalUri, Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, put},
    Json, Router,
};
use std::sync::{Arc, OnceLock};
use uuid::Uuid;
#[derive(Clone)]
struct ParticipantState {
    app: Arc<AppState>,
    client: Arc<GrpcConcertParticipantClient>,
}
impl FromRef<ParticipantState> for Arc<AppState> {
    fn from_ref(s: &ParticipantState) -> Self {
        s.app.clone()
    }
}
pub fn router(state: Arc<AppState>, client: Arc<GrpcConcertParticipantClient>) -> Router {
    Router::new()
        .route(
            "/api/concert/events/:event_id/me",
            get(dispatch).patch(dispatch),
        )
        .route("/api/concert/events/:event_id/me/avatar", put(dispatch))
        .with_state(ParticipantState { app: state, client })
}
#[derive(Clone)]
struct ParticipantRepresentation;
fn protect(r: &mut Response) {
    r.extensions_mut().insert(ParticipantRepresentation);
    let h = r.headers_mut();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.remove(header::ETAG);
    h.remove(header::LAST_MODIFIED);
}
fn error(status: StatusCode, reason: &'static str) -> Response {
    let mut response=(status,Json(serde_json::json!({"code":format!("CONCERT_{}",status.as_u16()),"message":"Concert request unavailable","traceId":super::correlation_id_for_response(),"timestamp":chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis,true),"details":{"reason":reason}}))).into_response();
    protect(&mut response);
    response
}
fn rpc_error(status: tonic::Status) -> Response {
    let pair = match status.code() {
        tonic::Code::InvalidArgument => (StatusCode::BAD_REQUEST, "INVALID_INPUT"),
        tonic::Code::Unauthenticated => (StatusCode::UNAUTHORIZED, "AUTHENTICATION_REQUIRED"),
        tonic::Code::PermissionDenied => (StatusCode::FORBIDDEN, "PERMISSION_DENIED"),
        tonic::Code::DeadlineExceeded => (StatusCode::GATEWAY_TIMEOUT, "DEADLINE_EXCEEDED"),
        _ => (StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE"),
    };
    error(pair.0, pair.1)
}
fn single<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut all = headers.get_all(name).iter();
    let v = all.next()?.to_str().ok()?;
    if all.next().is_some() {
        None
    } else {
        Some(v)
    }
}
fn uuid(text: &str) -> Option<Uuid> {
    Uuid::parse_str(text)
        .ok()
        .filter(|v| !v.is_nil() && v.to_string() == text)
}
fn path(path: &str) -> Option<(Uuid, bool)> {
    let suffix = path.strip_prefix("/api/concert/events/")?;
    let (event, tail) = suffix.split_once('/')?;
    Some((
        uuid(event)?,
        match tail {
            "me" => false,
            "me/avatar" => true,
            _ => return None,
        },
    ))
}
/// Mount outside transport rejection layers so C03 errors cannot disclose diagnostics.
pub async fn protect_response(request: Request, next: Next) -> Response {
    let protected = request
        .uri()
        .path()
        .strip_prefix("/api/concert/events/")
        .and_then(|s| s.split_once('/'))
        .is_some_and(|(_, tail)| matches!(tail, "me" | "me/avatar"));
    let mut response = next.run(request).await;
    if !protected {
        return response;
    }
    if (response.status().is_client_error() || response.status().is_server_error())
        && response
            .extensions()
            .get::<ParticipantRepresentation>()
            .is_none()
    {
        let status = response.status();
        let pair = match status.as_u16() {
            400 | 405 | 422 => (StatusCode::BAD_REQUEST, "INVALID_INPUT"),
            401 => (StatusCode::UNAUTHORIZED, "AUTHENTICATION_REQUIRED"),
            403 => (StatusCode::FORBIDDEN, "PERMISSION_DENIED"),
            413 => (StatusCode::PAYLOAD_TOO_LARGE, "PAYLOAD_TOO_LARGE"),
            415 => (StatusCode::UNSUPPORTED_MEDIA_TYPE, "INVALID_MEDIA_TYPE"),
            429 => (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED"),
            504 => (StatusCode::GATEWAY_TIMEOUT, "DEADLINE_EXCEEDED"),
            _ => (StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE"),
        };
        response = error(pair.0, pair.1);
    }
    protect(&mut response);
    response
}
async fn dispatch(
    State(state): State<ParticipantState>,
    OriginalUri(uri): OriginalUri,
    access: Result<HttpConcertAccess, GatewayError>,
    request: Request,
) -> Response {
    let access = match access {
        Ok(v) => v,
        Err(_) => return error(StatusCode::UNAUTHORIZED, "AUTHENTICATION_REQUIRED"),
    };
    let Some((event, avatar)) = path(uri.path()) else {
        return error(StatusCode::BAD_REQUEST, "INVALID_INPUT");
    };
    if uri.query().is_some() || single(request.headers(), "authorization").is_none() {
        return error(StatusCode::BAD_REQUEST, "INVALID_INPUT");
    }
    let operation = match (request.method(), avatar) {
        (&Method::GET, false) => Operation::Read,
        (&Method::PATCH, false) => Operation::Update,
        (&Method::PUT, true) => Operation::Avatar,
        _ => return error(StatusCode::BAD_REQUEST, "INVALID_INPUT"),
    };
    static CAPACITY: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    let _permit = match CAPACITY
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(8)))
        .clone()
        .try_acquire_owned()
    {
        Ok(v) => v,
        Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE"),
    };
    if operation != Operation::Read {
        if !single(request.headers(), "origin")
            .is_some_and(|v| state.app.browser_auth_origins.iter().any(|o| o == v))
        {
            return error(StatusCode::FORBIDDEN, "ORIGIN_NOT_ALLOWED");
        }
        if single(request.headers(), "x-mcr-intent") != Some("browser") {
            return error(StatusCode::FORBIDDEN, "BROWSER_INTENT_REQUIRED");
        }
        if !single(request.headers(), "content-type").is_some_and(|v| {
            v.split(';')
                .next()
                .is_some_and(|t| t.trim().eq_ignore_ascii_case("application/json"))
        }) {
            return error(StatusCode::UNSUPPORTED_MEDIA_TYPE, "INVALID_MEDIA_TYPE");
        }
    }
    let remaining = match access.remaining(std::time::Duration::from_secs(2)) {
        Ok(v) => v,
        Err(_) => return error(StatusCode::GATEWAY_TIMEOUT, "DEADLINE_EXCEEDED"),
    };
    let limit = if operation == Operation::Read {
        0
    } else {
        MAX_BODY
    };
    let body = match tokio::time::timeout(remaining, to_bytes(request.into_body(), limit)).await {
        Ok(Ok(v)) => v,
        Ok(Err(_)) => {
            return error(
                if operation == Operation::Read {
                    StatusCode::BAD_REQUEST
                } else {
                    StatusCode::PAYLOAD_TOO_LARGE
                },
                if operation == Operation::Read {
                    "INVALID_INPUT"
                } else {
                    "PAYLOAD_TOO_LARGE"
                },
            )
        }
        Err(_) => return error(StatusCode::GATEWAY_TIMEOUT, "DEADLINE_EXCEEDED"),
    };
    let Some(reader) = state.app.concert_identity.as_ref() else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE");
    };
    let context = match access.borrowed() {
        Ok(v) => v,
        Err(_) => return error(StatusCode::UNAUTHORIZED, "AUTHENTICATION_REQUIRED"),
    };
    let original = match context.rpc_deadline() {
        Ok(v) => v,
        Err(_) => return error(StatusCode::GATEWAY_TIMEOUT, "DEADLINE_EXCEEDED"),
    };
    let value = match Box::pin(state.client.dispatch(
        reader,
        context,
        ParticipantHttpRequest {
            event,
            request_id: Uuid::new_v4(),
            operation,
            body: body.to_vec(),
        },
    ))
    .await
    {
        Ok(v) => v,
        Err(e) => return rpc_error(e),
    };
    // The outbound adapter has already validated this authenticated reply.
    // Retain its cleanup capability before any late access/disclosure refusal.
    let delivery = match value
        .delivery
        .and_then(|v| state.client.delivery_token(v).ok())
    {
        Some(v) => v,
        None => return error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE"),
    };
    let mut response = if access.check().is_err() {
        error(StatusCode::UNAUTHORIZED, "AUTHENTICATION_REQUIRED")
    } else if let Some(failure) = value.failure {
        match failure_status(&failure) {
            Ok((status, reason)) => error(status, reason),
            Err(e) => return rpc_error(e),
        }
    } else {
        // Only protected success content carries the original disclosure gate.
        // A locally authored safe error contains no protected owner payload and
        // must remain consumable after that ceiling without extending it.
        match super::response_disclosure::DisclosureBody::new(
            Body::from(value.response_json),
            value.content_expires_unix_micros,
            original,
        ) {
            Ok(body) => {
                let mut response = Response::new(Body::new(body));
                response.headers_mut().insert(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                );
                response
            }
            Err(e) => rpc_error(e),
        }
    };
    // This same token survives both success and closed error representations.
    // Only Root's actual outer HTTP EOF layer may complete it.
    response.extensions_mut().insert(delivery);
    protect(&mut response);
    response
}
