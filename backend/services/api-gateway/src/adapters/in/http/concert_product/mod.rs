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
    routing::{delete, get, post, put},
    Json, Router,
};
use std::sync::{Arc, OnceLock};
use uuid::Uuid;
mod command_routes;
mod disclosure;
mod feature_routes;
mod read_routes;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/concert/events/:event_id/feed", get(dispatch))
        .route(
            "/api/concert/events/:event_id/posts/:post_id",
            get(dispatch).patch(dispatch).delete(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/posts/:post_id/comments",
            get(dispatch).post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/parties",
            get(dispatch).post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/parties/:party_id",
            get(dispatch).patch(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/parties/:party_id/applications",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/party-applications/:application_id/decision",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/party-applications/:application_id",
            delete(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/parties/:party_id/members/:user_id",
            delete(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/parties/:party_id/cancel",
            post(dispatch),
        )
        .route("/api/concert/events/:event_id/commands", get(dispatch))
        .route(
            "/api/concert/events/:event_id/me/social-handles",
            get(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/me/social-handles/:platform",
            put(dispatch).delete(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/social-requests",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/me/social-requests",
            get(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/social-requests/:request_id",
            get(dispatch).delete(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/social-requests/:request_id/decision",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/social-requests/:request_id/revoke",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/social-requests/:request_id/reveal",
            get(dispatch),
        )
        .route("/api/concert/events/:event_id/me/blocks", get(dispatch))
        .route(
            "/api/concert/events/:event_id/me/blocks/:user_id",
            put(dispatch).delete(dispatch),
        )
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
        .merge(feature_routes::router())
        .merge(command_routes::router())
        .merge(read_routes::router())
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
    if let Some(route) = feature_routes::route(method, path) {
        return route.mutation();
    }
    if command_routes::owns(method, path) {
        return true;
    }
    if social_route(method, path).is_some() {
        return *method != Method::GET;
    }
    if method == Method::POST && matches!(tail, "media/uploads" | "parties") {
        return true;
    }
    let pieces: Vec<_> = tail.split('/').collect();
    if (method == Method::POST
        && matches!(pieces.as_slice(), ["parties", party, "applications"] if uuid(party).is_some()))
        || (method == Method::POST
            && matches!(pieces.as_slice(), ["parties", party, "cancel"] if uuid(party).is_some()))
        || (method == Method::POST
            && matches!(pieces.as_slice(), ["party-applications", application, "decision"] if uuid(application).is_some()))
        || (method == Method::DELETE
            && matches!(pieces.as_slice(), ["party-applications", application] if uuid(application).is_some()))
        || (method == Method::DELETE
            && matches!(pieces.as_slice(), ["parties", party, "members", user] if uuid(party).is_some() && uuid(user).is_some()))
    {
        return true;
    }
    let pieces: Vec<_> = tail.split('/').collect();
    if pieces.len() != 3 || pieces[0] != "media" || uuid(pieces[1]).is_none() {
        return false;
    }
    (method == Method::PUT && pieces[2] == "bytes")
        || (method == Method::POST && pieces[2] == "finalize")
}

/// HTTP paths only; canonical actor/session and all consent/grant checks belong
/// to the authenticated Concert producer, never a browser header or role.
fn social_route(method: &Method, path: &str) -> Option<&'static str> {
    let suffix = path.strip_prefix("/api/concert/events/")?;
    let (event, tail) = suffix.split_once('/')?;
    uuid(event)?;
    let parts: Vec<_> = tail.split('/').collect();
    match (method.as_str(), parts.as_slice()) {
        ("GET", ["me", "social-handles"]) => Some("listOwnConcertHandles"),
        ("PUT", ["me", "social-handles", p]) if matches!(*p, "INSTAGRAM" | "X") => {
            Some("setConcertHandle")
        }
        ("DELETE", ["me", "social-handles", p]) if matches!(*p, "INSTAGRAM" | "X") => {
            Some("removeConcertHandle")
        }
        ("POST", ["social-requests"]) => Some("createConcertSocialRequest"),
        ("GET", ["me", "social-requests"]) => Some("listConcertSocialRequests"),
        ("GET", ["social-requests", id]) if uuid(id).is_some() => Some("getConcertSocialRequest"),
        ("POST", ["social-requests", id, "decision"]) if uuid(id).is_some() => {
            Some("decideConcertSocialRequest")
        }
        ("DELETE", ["social-requests", id]) if uuid(id).is_some() => {
            Some("cancelConcertSocialRequest")
        }
        ("POST", ["social-requests", id, "revoke"]) if uuid(id).is_some() => {
            Some("revokeConcertSocialRequest")
        }
        ("GET", ["social-requests", id, "reveal"]) if uuid(id).is_some() => {
            Some("revealConcertSocialHandle")
        }
        ("GET", ["me", "blocks"]) => Some("listConcertBlocks"),
        ("PUT", ["me", "blocks", id]) if uuid(id).is_some() => Some("blockConcertUser"),
        ("DELETE", ["me", "blocks", id]) if uuid(id).is_some() => Some("unblockConcertUser"),
        _ => None,
    }
}

#[cfg(test)]
mod social_route_tests {
    use super::*;
    #[test]
    fn social_frozen_paths_have_exact_receipt_ownership_and_private_error_protection() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .unwrap();
        let rows: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("contracts/domain/v1/operations.json")).unwrap(),
        )
        .unwrap();
        let event = Uuid::new_v4();
        let id = Uuid::new_v4();
        let mut count = 0;
        for row in rows.as_array().unwrap() {
            let method = Method::from_bytes(
                row["method"]
                    .as_str()
                    .unwrap()
                    .to_ascii_uppercase()
                    .as_bytes(),
            )
            .unwrap();
            let path = row["path"]
                .as_str()
                .unwrap()
                .replace("{eventId}", &event.to_string())
                .replace("{requestId}", &id.to_string())
                .replace("{userId}", &id.to_string())
                .replace("{platform}", "INSTAGRAM");
            if let Some(name) = social_route(&method, &path) {
                count += 1;
                assert_eq!(name, row["operationId"].as_str().unwrap());
                assert_eq!(owns_receipt(&method, &path), method != Method::GET);
                assert!(is_product(&path));
                assert!(social_route(&method, &format!("{path}/unknown")).is_none());
            }
        }
        assert_eq!(count, 13);
        for path in [
            format!("/api/concert/events/{event}/me/social-handles/UNKNOWN"),
            format!("/api/concert/events/{event}/me/blocks/not-uuid"),
        ] {
            assert!(!owns_receipt(&Method::PUT, &path));
        }
    }
    #[test]
    fn social_bodyless_deletes_require_browser_intent_origin_and_exact_key() {
        let path = format!(
            "/api/concert/events/{}/me/social-handles/INSTAGRAM",
            Uuid::new_v4()
        );
        let mut headers = HeaderMap::new();
        headers.insert("origin", HeaderValue::from_static("https://mcr.example"));
        headers.insert("x-mcr-intent", HeaderValue::from_static("browser"));
        headers.insert(
            "idempotency-key",
            HeaderValue::from_str(&Uuid::new_v4().to_string()).unwrap(),
        );
        let origins = vec!["https://mcr.example".to_owned()];
        assert!(request_metadata(&Method::DELETE, &path, &headers, &origins).is_ok());
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        assert!(request_metadata(&Method::DELETE, &path, &headers, &origins).is_err());
        headers.remove("content-type");
        headers.remove("idempotency-key");
        assert!(request_metadata(&Method::DELETE, &path, &headers, &origins).is_err());
        headers.remove("origin");
        assert_eq!(
            request_metadata(&Method::DELETE, &path, &headers, &origins)
                .unwrap_err()
                .0,
            StatusCode::FORBIDDEN
        );
    }
}

#[cfg(test)]
mod receipt_scope_tests {
    use super::*;
    #[test]
    fn party_mutations_have_exact_receipt_ownership_and_protected_errors() {
        let root = format!("/api/concert/events/{}", Uuid::new_v4());
        let party = Uuid::new_v4();
        let application = Uuid::new_v4();
        let user = Uuid::new_v4();
        for (method, tail) in [
            (Method::POST, format!("parties/{party}/applications")),
            (
                Method::POST,
                format!("party-applications/{application}/decision"),
            ),
            (Method::DELETE, format!("party-applications/{application}")),
            (Method::DELETE, format!("parties/{party}/members/{user}")),
            (Method::POST, format!("parties/{party}/cancel")),
        ] {
            let path = format!("{root}/{tail}");
            assert!(owns_receipt(&method, &path));
            assert!(is_product(&path));
            assert!(!owns_receipt(&Method::GET, &path));
            assert!(!owns_receipt(&method, &format!("{path}/extra")));
        }
    }

    #[test]
    fn party_delete_requires_origin_intent_and_single_canonical_key_without_content_type() {
        let origins = vec!["https://mcr.example".to_owned()];
        let path = "/api/concert/events/00000000-0000-4000-8000-000000000001/party-applications/00000000-0000-4000-8000-000000000002";
        let mut h = HeaderMap::new();
        assert_eq!(
            request_metadata(&Method::DELETE, path, &h, &origins)
                .unwrap_err()
                .1,
            "ORIGIN_NOT_ALLOWED"
        );
        h.insert("origin", HeaderValue::from_static("https://mcr.example"));
        assert_eq!(
            request_metadata(&Method::DELETE, path, &h, &origins)
                .unwrap_err()
                .1,
            "BROWSER_INTENT_REQUIRED"
        );
        h.insert("x-mcr-intent", HeaderValue::from_static("browser"));
        assert_eq!(
            request_metadata(&Method::DELETE, path, &h, &origins)
                .unwrap_err()
                .1,
            "INVALID_INPUT"
        );
        h.insert(
            "idempotency-key",
            HeaderValue::from_str(&Uuid::new_v4().to_string()).unwrap(),
        );
        assert!(request_metadata(&Method::DELETE, path, &h, &origins).is_ok());
        h.append(
            "idempotency-key",
            HeaderValue::from_str(&Uuid::new_v4().to_string()).unwrap(),
        );
        assert!(request_metadata(&Method::DELETE, path, &h, &origins).is_err());
        h.remove("idempotency-key");
        h.insert(
            "idempotency-key",
            HeaderValue::from_str(&Uuid::new_v4().to_string()).unwrap(),
        );
        h.insert("content-type", HeaderValue::from_static("application/json"));
        assert!(request_metadata(&Method::DELETE, path, &h, &origins).is_err());
        h.remove("content-type");
        h.append("origin", HeaderValue::from_static("https://mcr.example"));
        assert_eq!(
            request_metadata(&Method::DELETE, path, &h, &origins)
                .unwrap_err()
                .1,
            "ORIGIN_NOT_ALLOWED"
        );
    }
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
    feature_routes::protects(path)
        || command_routes::protects(path)
        || read_routes::protects(path)
        || tail == "feed"
        || tail == "commands"
        || tail == "parties"
        || tail.starts_with("media/")
        || tail.starts_with("posts/")
        || tail.starts_with("parties/")
        || tail.starts_with("party-applications/")
        || tail == "me/social-handles"
        || tail.starts_with("me/social-handles/")
        || tail == "me/social-requests"
        || tail == "social-requests"
        || tail.starts_with("social-requests/")
        || tail == "me/blocks"
        || tail.starts_with("me/blocks/")
}
/// Cover outer transport rejections too, without discarding owner recovery details.
pub async fn protect_response(request: Request, next: axum::middleware::Next) -> Response {
    // Router::layer also wraps the shared fallback. Keep unmounted feature
    // suffixes out of unrelated service middleware and return a closed C04 error.
    if (feature_routes::protects(request.uri().path())
        || command_routes::protects(request.uri().path())
        || read_routes::protects(request.uri().path()))
        && request
            .extensions()
            .get::<axum::extract::MatchedPath>()
            .is_none()
    {
        return error(StatusCode::NOT_FOUND, "RESOURCE_UNAVAILABLE");
    }
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
// Shared by the live handler and focused boundary tests.
fn request_metadata(
    method: &Method,
    path: &str,
    headers: &HeaderMap,
    origins: &[String],
) -> Result<(String, String), (StatusCode, &'static str)> {
    let mutation = matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );
    if mutation {
        if !single(headers, "origin").is_some_and(|origin| origins.iter().any(|v| v == origin)) {
            return Err((StatusCode::FORBIDDEN, "ORIGIN_NOT_ALLOWED"));
        }
        if single(headers, "x-mcr-intent") != Some("browser") {
            return Err((StatusCode::FORBIDDEN, "BROWSER_INTENT_REQUIRED"));
        }
    }
    let bodyless_delete = *method == Method::DELETE;
    let content_type = if bodyless_delete {
        if headers.contains_key("content-type") {
            return Err((StatusCode::BAD_REQUEST, "INVALID_INPUT"));
        }
        "".to_owned()
    } else {
        single(headers, "content-type").unwrap_or("").to_owned()
    };
    let idempotency_key = single(headers, "idempotency-key").unwrap_or("").to_owned();
    if mutation && uuid(&idempotency_key).is_none() {
        return Err((StatusCode::BAD_REQUEST, "INVALID_INPUT"));
    }
    if mutation
        && !bodyless_delete
        && !(if path.ends_with("/bytes") {
            matches!(
                content_type.as_str(),
                "image/jpeg" | "image/png" | "image/webp"
            )
        } else {
            content_type == "application/json"
        })
    {
        return Err((StatusCode::UNSUPPORTED_MEDIA_TYPE, "INVALID_MEDIA_TYPE"));
    }
    if !mutation
        && (headers.contains_key("content-type") || headers.contains_key("idempotency-key"))
    {
        return Err((StatusCode::BAD_REQUEST, "INVALID_INPUT"));
    }
    Ok((content_type, idempotency_key))
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
    let feature = if feature_routes::protects(uri.path()) {
        match feature_routes::route(&method, uri.path()) {
            Some(route) if feature_routes::valid_target(route, &uri.to_string()) => Some(route),
            _ => return error(StatusCode::BAD_REQUEST, "INVALID_INPUT"),
        }
    } else {
        None
    };
    let client_address = request
        .extensions()
        .get::<super::client_identity::VerifiedClientAddress>()
        .copied();
    let mutation = matches!(
        method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );
    let bodyless_delete = method == Method::DELETE;
    let (content_type, idempotency_key) = match request_metadata(
        &method,
        uri.path(),
        request.headers(),
        &state.browser_auth_origins,
    ) {
        Ok(value) => value,
        Err((status, reason)) => return error(status, reason),
    };
    let limit = if !mutation {
        0
    } else if uri.path().ends_with("/bytes") {
        MAX_BODY
    } else if bodyless_delete {
        0
    } else if let Some(route) = feature {
        route.body_limit()
    } else if social_route(&method, uri.path()).is_some() {
        8192
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
    if feature.is_some_and(|route| !feature_routes::valid_body(route, &body)) {
        return error(StatusCode::BAD_REQUEST, "INVALID_INPUT");
    }
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
                client_address,
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
        Body::from(result.body),
        result.content_expires_unix_micros,
        original,
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
