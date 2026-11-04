//! Frozen C04 product routes. Browser input never supplies canonical identity.
use super::{concert_access::HttpConcertAccess, GatewayError};
use crate::{
    adapters::out::concert_product::{ProductHttpRequest, MAX_BODY},
    config::AppState,
};
use axum::{
    body::{to_bytes, Body},
    extract::{OriginalUri, Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use std::sync::{Arc, OnceLock};
use uuid::Uuid;
mod disclosure;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/concert/events/:event_id/feed", get(dispatch))
        .route(
            "/api/concert/events/:event_id/posts/:post_id",
            get(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/posts/:post_id/comments",
            get(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/parties",
            get(dispatch).post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/parties/:party_id",
            get(dispatch),
        )
        .route("/api/concert/events/:event_id/commands", get(dispatch))
        .route(
            "/api/concert/events/:event_id/media/uploads",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/media/:media_id",
            get(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/media/:media_id/bytes",
            put(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/media/:media_id/content",
            get(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/media/:media_id/finalize",
            post(dispatch),
        )
        .with_state(state)
}

/// Concert owns only these exact command receipts; all other routes retain
/// their inherited Gateway journal and idempotency policy.
pub(super) fn owns_receipt(method: &Method, path: &str) -> bool {
    let Some(suffix) = path.strip_prefix("/api/concert/events/") else {
        return false;
    };
    let Some((event, tail)) = suffix.split_once('/') else {
        return false;
    };
    if uuid(event).is_none() {
        return false;
    }
    if method == Method::POST && matches!(tail, "media/uploads" | "parties") {
        return true;
    }
    let pieces: Vec<_> = tail.split('/').collect();
    if pieces.len() != 3 || pieces[0] != "media" || uuid(pieces[1]).is_none() {
        return false;
    }
    (method == Method::PUT && pieces[2] == "bytes")
        || (method == Method::POST && pieces[2] == "finalize")
}

#[cfg(test)]
mod receipt_scope_tests {
    use super::*;
    #[test]
    fn journal_bypass_is_exclusive_to_canonical_c04_media_mutations() {
        let event = Uuid::new_v4();
        let media = Uuid::new_v4();
        let root = format!("/api/concert/events/{event}");
        for (method, tail) in [
            (Method::POST, "media/uploads".to_owned()),
            (Method::PUT, format!("media/{media}/bytes")),
            (Method::POST, format!("media/{media}/finalize")),
        ] {
            assert!(owns_receipt(&method, &format!("{root}/{tail}")));
            assert!(!owns_receipt(&Method::GET, &format!("{root}/{tail}")));
            assert!(!owns_receipt(&method, &format!("{root}/{tail}/extra")));
        }
        for (method, path) in [
            (Method::POST, format!("{root}/feed")),
            (Method::PUT, format!("{root}/media/{media}/content")),
            (Method::PUT, "/api/profile/privacy".into()),
            (Method::POST, "/api/posts".into()),
            (Method::POST, "/api/auth/login".into()),
            (
                Method::POST,
                "/api/concert/events/not-uuid/media/uploads".into(),
            ),
        ] {
            assert!(!owns_receipt(&method, &path));
        }
    }
}

fn uuid(value: &str) -> Option<Uuid> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
}
fn single<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    let result = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        None
    } else {
        Some(result)
    }
}
pub fn error(status: StatusCode, reason: &'static str) -> Response {
    let mut response = (
        status,
        Json(serde_json::json!({
            "code": format!("CONCERT_{}", status.as_u16()),
            "message": "Concert request unavailable",
            "traceId": super::correlation_id_for_response(),
            "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "details": {"reason": reason},
        })),
    )
        .into_response();
    protect(&mut response);
    response
}
fn protect(response: &mut Response) {
    response.extensions_mut().insert(ProductRepresentation);
    let h = response.headers_mut();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.remove(header::ETAG);
    h.remove(header::LAST_MODIFIED);
}
#[derive(Clone)]
struct ProductRepresentation;

fn is_product(path: &str) -> bool {
    let Some(suffix) = path.strip_prefix("/api/concert/events/") else {
        return false;
    };
    let Some((_, tail)) = suffix.split_once('/') else {
        return false;
    };
    tail == "feed"
        || tail == "commands"
        || tail == "parties"
        || tail.starts_with("media/")
        || tail.starts_with("posts/")
        || tail.starts_with("parties/")
}
/// Cover outer transport rejections too, without discarding owner recovery details.
pub async fn protect_response(request: Request, next: axum::middleware::Next) -> Response {
    let protected = is_product(request.uri().path());
    let mut response = next.run(request).await;
    if !protected {
        return response;
    }
    if response
        .extensions()
        .get::<ProductRepresentation>()
        .is_none()
        && (response.status().is_client_error() || response.status().is_server_error())
    {
        // The shared browser boundary can reject before the product handler.
        // Retain only its closed semantic reason, never its arbitrary message,
        // diagnostic fields, headers or response body.
        let status = response.status();
        let forbidden_reason = if status == StatusCode::FORBIDDEN {
            to_bytes(response.into_body(), 4096)
                .await
                .ok()
                .and_then(|body| serde_json::from_slice::<serde_json::Value>(&body).ok())
                .and_then(|value| match value["details"]["reason"].as_str() {
                    Some("ORIGIN_NOT_ALLOWED") => Some("ORIGIN_NOT_ALLOWED"),
                    Some("BROWSER_INTENT_REQUIRED") => Some("BROWSER_INTENT_REQUIRED"),
                    _ => None,
                })
        } else {
            None
        };
        let (status, reason) = match status.as_u16() {
            400 | 405 | 422 => (StatusCode::BAD_REQUEST, "INVALID_INPUT"),
            401 => (StatusCode::UNAUTHORIZED, "AUTHENTICATION_REQUIRED"),
            403 => (
                StatusCode::FORBIDDEN,
                forbidden_reason.unwrap_or("PERMISSION_DENIED"),
            ),
            404 => (StatusCode::NOT_FOUND, "RESOURCE_UNAVAILABLE"),
            413 => (StatusCode::PAYLOAD_TOO_LARGE, "PAYLOAD_TOO_LARGE"),
            415 => (StatusCode::UNSUPPORTED_MEDIA_TYPE, "INVALID_MEDIA_TYPE"),
            429 => (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED"),
            504 => (StatusCode::GATEWAY_TIMEOUT, "DEADLINE_EXCEEDED"),
            _ => (StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE"),
        };
        response = error(status, reason);
    }
    protect(&mut response);
    response
}
fn rpc_error(status: tonic::Status) -> Response {
    let (code, reason) = match status.code() {
        tonic::Code::InvalidArgument => (StatusCode::BAD_REQUEST, "INVALID_INPUT"),
        tonic::Code::Unauthenticated => (StatusCode::UNAUTHORIZED, "AUTHENTICATION_REQUIRED"),
        tonic::Code::PermissionDenied => (StatusCode::FORBIDDEN, "PERMISSION_DENIED"),
        tonic::Code::NotFound => (StatusCode::NOT_FOUND, "RESOURCE_UNAVAILABLE"),
        tonic::Code::DeadlineExceeded => (StatusCode::GATEWAY_TIMEOUT, "DEADLINE_EXCEEDED"),
        _ => (StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE"),
    };
    error(code, reason)
}
async fn dispatch(
    State(state): State<Arc<AppState>>,
    OriginalUri(uri): OriginalUri,
    access: Result<HttpConcertAccess, GatewayError>,
    request: Request,
) -> Response {
    let access = match access {
        Ok(access) => access,
        Err(_) => return error(StatusCode::UNAUTHORIZED, "AUTHENTICATION_REQUIRED"),
    };
    let parts: Vec<_> = uri.path().split('/').collect();
    static CAPACITY: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    let _permit = match CAPACITY
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(4)))
        .clone()
        .try_acquire_owned()
    {
        Ok(permit) => permit,
        Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE"),
    };
    let Some(event) = parts.get(4).and_then(|v| uuid(v)) else {
        return error(StatusCode::BAD_REQUEST, "INVALID_INPUT");
    };
    let method = request.method().clone();
    let mutation = matches!(method, Method::POST | Method::PUT);
    let headers = request.headers();
    if mutation {
        if !single(headers, "origin")
            .is_some_and(|origin| state.browser_auth_origins.iter().any(|v| v == origin))
        {
            return error(StatusCode::FORBIDDEN, "ORIGIN_NOT_ALLOWED");
        }
        if single(headers, "x-mcr-intent") != Some("browser") {
            return error(StatusCode::FORBIDDEN, "BROWSER_INTENT_REQUIRED");
        }
    }
    let content_type = single(headers, "content-type").unwrap_or("").to_owned();
    let idempotency_key = single(headers, "idempotency-key").unwrap_or("").to_owned();
    if mutation && uuid(&idempotency_key).is_none() {
        return error(StatusCode::BAD_REQUEST, "INVALID_INPUT");
    }
    if mutation
        && !(if uri.path().ends_with("/bytes") {
            matches!(
                content_type.as_str(),
                "image/jpeg" | "image/png" | "image/webp"
            )
        } else {
            content_type == "application/json"
        })
    {
        return error(StatusCode::UNSUPPORTED_MEDIA_TYPE, "INVALID_MEDIA_TYPE");
    }
    if !mutation
        && (headers.contains_key("content-type") || headers.contains_key("idempotency-key"))
    {
        return error(StatusCode::BAD_REQUEST, "INVALID_INPUT");
    }
    let limit = if !mutation {
        0
    } else if uri.path().ends_with("/bytes") {
        MAX_BODY
    } else {
        65536
    };
    let body = match to_bytes(request.into_body(), limit).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return error(
                if mutation {
                    StatusCode::PAYLOAD_TOO_LARGE
                } else {
                    StatusCode::BAD_REQUEST
                },
                if mutation {
                    "PAYLOAD_TOO_LARGE"
                } else {
                    "INVALID_INPUT"
                },
            )
        }
    };
    let (Some(client), Some(reader)) = (&state.concert_product_client, &state.concert_identity)
    else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE");
    };
    let context = match access.borrowed() {
        Ok(context) => context,
        Err(_) => return error(StatusCode::UNAUTHORIZED, "AUTHENTICATION_REQUIRED"),
    };
    let result = Box::pin(
        client.dispatch(
            reader,
            context,
            ProductHttpRequest {
                event,
                request_id: Uuid::new_v4(),
                method: method.to_string(),
                target: uri
                    .path_and_query()
                    .map(|v| v.as_str())
                    .unwrap_or(uri.path())
                    .to_owned(),
                body: body.to_vec(),
                content_type,
                idempotency_key,
            },
        ),
    )
    .await;
    let result = match result {
        Ok(result) => result,
        Err(status) => return rpc_error(status),
    };
    let delivery = match result.delivery {
        Some(capability) => match client.delivery_token(capability) {
            Ok(token) => token,
            Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE"),
        },
        None => return error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE"),
    };
    if access.check().is_err() {
        return error(StatusCode::UNAUTHORIZED, "AUTHENTICATION_REQUIRED");
    }
    let status = match result.status {
        200 | 202 | 400 | 401 | 403 | 404 | 409 | 413 | 415 | 429 | 500 | 503 | 504 => {
            StatusCode::from_u16(result.status as u16).expect("closed status")
        }
        _ => return error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE"),
    };
    let image = matches!(
        result.content_type.as_str(),
        "image/jpeg" | "image/png" | "image/webp"
    );
    if image && (status != StatusCode::OK || !uri.path().ends_with("/content"))
        || (!image
            && (result.content_type != "application/json"
                || serde_json::from_slice::<serde_json::Value>(&result.body).is_err()))
    {
        return error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE");
    }
    let original = match access.rpc_deadline() {
        Ok(value) => value,
        Err(_) => return error(StatusCode::UNAUTHORIZED, "AUTHENTICATION_REQUIRED"),
    };
    let body = match disclosure::DisclosureBody::new(
        Body::from(result.body), result.content_expires_unix_micros, original,
    ) {
        Ok(body) => Body::new(body),
        Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE"),
    };
    let mut response = Response::new(body);
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&result.content_type).expect("closed content type"),
    );
    if image {
        response.headers_mut().insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("inline"),
        );
        response.headers_mut().insert(
            "cross-origin-resource-policy",
            HeaderValue::from_static("same-origin"),
        );
    }
    protect(&mut response);
    response.extensions_mut().insert(delivery);
    response
}
