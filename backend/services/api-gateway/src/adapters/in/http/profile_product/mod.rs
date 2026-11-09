//! Profile product routes. Browser input never supplies canonical identity.
//!
//! These routes are the profile analogue of `concert_product`: the same browser
//! origin, `X-MCR-Intent`, CSRF/session and body-limit discipline, dispatched to
//! the profile product client instead of the concert one. What is deliberately
//! absent is the event. A profile operation has no event scope, so no route here
//! carries an `:event_id` segment and any request that names an event path is
//! refused before dispatch.
//!
//! The closed route set below is the profile subset of the frozen operation
//! register (`contracts/core/v1/operations.json`). A path outside it is not
//! matched here at all, so it can never reach the profile producer.
use super::{concert_access::HttpConcertAccess, profile_owner::ProfileOwner, GatewayError};
use crate::{
    adapters::out::profile_product_client::{
        concert_proto, proto, GrpcConcertProfileProductClient, GrpcProfileProductClient,
        ProfileHttpRequest, MAX_BODY,
    },
    config::AppState,
};
use axum::{
    body::{to_bytes, Body},
    extract::{OriginalUri, Request, State},
    http::{header, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post, put},
    Router,
};
use std::sync::{Arc, OnceLock};
use uuid::Uuid;

/// The closed profile product route table.
///
/// Each entry is `(method, axum path, operationId)`. The operation id is the
/// frozen contract name, so a test can prove this table and the contract agree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Route {
    pub method: &'static str,
    pub path: &'static str,
    pub operation: &'static str,
    /// Reads have a zero body limit; mutations carry the declared limit.
    pub body_limit: usize,
    /// `/bytes` uploads carry raw image bytes rather than JSON.
    pub bytes: bool,
}

const READ: usize = 0;
const JSON: usize = 65536;

pub const ROUTES: &[Route] = &[
    Route {
        method: "GET",
        path: "/api/me/profile",
        operation: "getOwnProfile",
        body_limit: READ,
        bytes: false,
    },
    Route {
        method: "PATCH",
        path: "/api/me/profile",
        operation: "updateOwnProfile",
        body_limit: JSON,
        bytes: false,
    },
    Route {
        method: "PUT",
        path: "/api/me/profile/favorites",
        operation: "setOwnFavorites",
        body_limit: JSON,
        bytes: false,
    },
    Route {
        method: "GET",
        path: "/api/profiles/:profile_id",
        operation: "getProfile",
        body_limit: READ,
        bytes: false,
    },
    // WP-03 left these two unmounted. `searchCatalog` is answered by the
    // profile service's catalog module; `discoverProfiles` reads a concert
    // service projection and is mounted here so the route exists, while the
    // producer refuses it until that composition is owned (see
    // `profile_product::coverage`).
    Route {
        method: "GET",
        path: "/api/catalog/:category/search",
        operation: "searchCatalog",
        body_limit: READ,
        bytes: false,
    },
    Route {
        method: "GET",
        path: "/api/discover/people",
        operation: "discoverProfiles",
        body_limit: READ,
        bytes: false,
    },
    Route {
        method: "GET",
        path: "/api/me/identity/:provider",
        operation: "getOwnIdentityVerification",
        body_limit: READ,
        bytes: false,
    },
    Route {
        method: "POST",
        path: "/api/me/identity/:provider/verifications",
        operation: "startIdentityVerification",
        body_limit: JSON,
        bytes: false,
    },
    Route {
        method: "DELETE",
        path: "/api/me/identity/:provider",
        operation: "disconnectIdentityVerification",
        body_limit: READ,
        bytes: false,
    },
    Route {
        method: "GET",
        path: "/api/me/room",
        operation: "getOwnRoom",
        body_limit: READ,
        bytes: false,
    },
    Route {
        method: "PUT",
        path: "/api/me/room",
        operation: "saveOwnRoom",
        body_limit: JSON,
        bytes: false,
    },
    Route {
        method: "PATCH",
        path: "/api/me/room/visibility",
        operation: "setOwnRoomVisibility",
        body_limit: JSON,
        bytes: false,
    },
    Route {
        method: "POST",
        path: "/api/me/room/posters",
        operation: "uploadRoomPoster",
        body_limit: JSON,
        bytes: false,
    },
    Route {
        method: "GET",
        path: "/api/me/avatar",
        operation: "getOwnProfileAvatar",
        body_limit: READ,
        bytes: false,
    },
    Route {
        method: "PUT",
        path: "/api/me/avatar",
        operation: "setOwnProfileAvatar",
        body_limit: JSON,
        bytes: false,
    },
    Route {
        method: "POST",
        path: "/api/me/media/uploads",
        operation: "initProfileMediaUpload",
        body_limit: JSON,
        bytes: false,
    },
    Route {
        method: "PUT",
        path: "/api/me/media/:media_id/bytes",
        operation: "putProfileMediaBytes",
        body_limit: MAX_BODY,
        bytes: true,
    },
    Route {
        method: "POST",
        path: "/api/me/media/:media_id/finalize",
        operation: "finalizeProfileMedia",
        body_limit: JSON,
        bytes: false,
    },
    Route {
        method: "GET",
        path: "/api/media/:media_id",
        operation: "getProfileMedia",
        body_limit: READ,
        bytes: false,
    },
    Route {
        method: "GET",
        path: "/api/media/:media_id/content",
        operation: "readProfileMedia",
        body_limit: READ,
        bytes: false,
    },
    Route {
        method: "POST",
        path: "/api/me/posts",
        operation: "createProfilePost",
        body_limit: JSON,
        bytes: false,
    },
    Route {
        method: "DELETE",
        path: "/api/me/posts/:post_id",
        operation: "deleteProfilePost",
        body_limit: READ,
        bytes: false,
    },
    Route {
        method: "GET",
        path: "/api/profiles/:profile_id/posts",
        operation: "listProfilePosts",
        body_limit: READ,
        bytes: false,
    },
    Route {
        method: "GET",
        path: "/api/profiles/:profile_id/room",
        operation: "getProfileRoom",
        body_limit: READ,
        bytes: false,
    },
    // ── WP-05 intro posts ──────────────────────────────────────────────────
    //
    // The two discovery reads are mounted: WP-04's visibility predicate and the
    // page/detail queries now exist, so these answer instead of failing.
    Route {
        method: "GET",
        path: "/api/discover/intro-posts",
        operation: "listIntroPosts",
        body_limit: READ,
        bytes: false,
    },
    Route {
        method: "GET",
        path: "/api/intro-posts/:intro_id",
        operation: "getIntroPost",
        body_limit: READ,
        bytes: false,
    },
    //
    // The intro tables (`intro_posts`, `intro_attachments`, `intro_likes`,
    // `intro_comments`) live in `mcr_concert`, so these route to the concert
    // profile transport like the profile-post operations above.
    //
    Route {
        method: "GET",
        path: "/api/me/intro-posts",
        operation: "listOwnIntroPosts",
        body_limit: READ,
        bytes: false,
    },
    Route {
        method: "GET",
        path: "/api/me/intro-posts/:intro_id",
        operation: "getOwnIntroPost",
        body_limit: READ,
        bytes: false,
    },
    Route {
        method: "PUT",
        path: "/api/me/intro-posts/:intro_id",
        operation: "saveIntroPostDraft",
        body_limit: JSON,
        bytes: false,
    },
    Route {
        method: "POST",
        path: "/api/me/intro-posts/:intro_id/publish",
        operation: "publishIntroPost",
        body_limit: JSON,
        bytes: false,
    },
    Route {
        method: "DELETE",
        path: "/api/me/intro-posts/:intro_id",
        operation: "deleteIntroPost",
        body_limit: READ,
        bytes: false,
    },
    Route {
        method: "PUT",
        path: "/api/intro-posts/:intro_id/like",
        operation: "setIntroPostLike",
        body_limit: READ,
        bytes: false,
    },
    Route {
        method: "DELETE",
        path: "/api/intro-posts/:intro_id/like",
        operation: "clearIntroPostLike",
        body_limit: READ,
        bytes: false,
    },
    Route {
        method: "GET",
        path: "/api/intro-posts/:intro_id/likes",
        operation: "listIntroPostLikers",
        body_limit: READ,
        bytes: false,
    },
    Route {
        method: "GET",
        path: "/api/intro-posts/:intro_id/comments",
        operation: "listIntroPostComments",
        body_limit: READ,
        bytes: false,
    },
    Route {
        method: "POST",
        path: "/api/intro-posts/:intro_id/comments",
        operation: "createIntroPostComment",
        body_limit: JSON,
        bytes: false,
    },
    Route {
        method: "DELETE",
        path: "/api/intro-posts/:intro_id/comments/:comment_id",
        operation: "deleteIntroPostComment",
        body_limit: READ,
        bytes: false,
    },
];

pub fn router(state: Arc<AppState>) -> Router {
    let mut router = Router::new();
    for route in ROUTES {
        router = router.route(
            route.path,
            match route.method {
                "GET" => get(dispatch),
                "POST" => post(dispatch),
                "PUT" => put(dispatch),
                "PATCH" => patch(dispatch),
                "DELETE" => delete(dispatch),
                _ => unreachable!("closed method set"),
            },
        );
    }
    router.with_state(state)
}

/// Profile owns its own command receipts for every mutation it serves, exactly
/// as the concert product owns C04 command receipts. The Gateway's replay
/// journal must not claim a profile mutation.
pub fn owns_receipt(method: &Method, path: &str) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    ) && route(method, path).is_some()
}

/// The operation id for an exact `(method, path)`, when the path matches a
/// closed route. `path` must be the request path without the query.
pub fn route(method: &Method, path: &str) -> Option<&'static Route> {
    ROUTES
        .iter()
        .find(|route| route.method == method.as_str() && template_matches(route.path, path))
}

/// Whether a path is covered by this router at all. Used by the response
/// protection layer to keep unmounted profile suffixes out of other middleware.
pub fn protects(path: &str) -> bool {
    ROUTES.iter().any(|route| template_matches(route.path, path))
}

/// Whether a path is a profile product path, including a suffix that no route
/// matches. Such a suffix must still receive a closed profile error rather than
/// falling through to an unrelated handler's envelope.
pub fn is_profile_path(path: &str) -> bool {
    protects(path) || path.starts_with("/api/me/") || path.starts_with("/api/profiles/")
        || path.starts_with("/api/media/")
}

/// Match an axum template against a concrete path. Segments starting with `:`
/// match exactly one non-empty segment; every other segment must be equal.
fn template_matches(template: &str, path: &str) -> bool {
    let mut expected = template.split('/');
    let mut actual = path.split('/');
    loop {
        match (expected.next(), actual.next()) {
            (None, None) => return true,
            (Some(_), None) | (None, Some(_)) => return false,
            (Some(want), Some(got)) => {
                if let Some(_parameter) = want.strip_prefix(':') {
                    if got.is_empty() {
                        return false;
                    }
                } else if want != got {
                    return false;
                }
            }
        }
    }
}

fn uuid(value: &str) -> Option<Uuid> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
}

fn single<'a>(headers: &'a axum::http::HeaderMap, name: &str) -> Option<&'a str> {
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
        axum::Json(serde_json::json!({
            "code": format!("PROFILE_{}", status.as_u16()),
            "message": "Profile request unavailable",
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
    response.extensions_mut().insert(ProfileRepresentation);
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.remove(header::ETAG);
    headers.remove(header::LAST_MODIFIED);
}

#[derive(Clone)]
struct ProfileRepresentation;

/// Shared by the live handler and the focused boundary tests.
///
/// This is the same discipline as the concert product: browser origin and
/// `X-MCR-Intent` before a mutation, a single canonical `Idempotency-Key`, a
/// JSON content type for JSON mutations, and the image allowlist for `/bytes`.
fn request_metadata(
    method: &Method,
    route: &Route,
    headers: &axum::http::HeaderMap,
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
        String::new()
    } else {
        single(headers, "content-type").unwrap_or("").to_owned()
    };
    let idempotency_key = single(headers, "idempotency-key").unwrap_or("").to_owned();
    if mutation && uuid(&idempotency_key).is_none() {
        return Err((StatusCode::BAD_REQUEST, "INVALID_INPUT"));
    }
    if mutation
        && !bodyless_delete
        && !(if route.bytes {
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
    if !mutation && (headers.contains_key("content-type") || headers.contains_key("idempotency-key"))
    {
        return Err((StatusCode::BAD_REQUEST, "INVALID_INPUT"));
    }
    Ok((content_type, idempotency_key))
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
    // Profile operations never carry an event path. A request that names one is
    // refused here, before any identity composition or dispatch.
    if uri.path().contains("/events/") {
        return error(StatusCode::BAD_REQUEST, "INVALID_INPUT");
    }
    let method = request.method().clone();
    let Some(route) = route(&method, uri.path()) else {
        return error(StatusCode::BAD_REQUEST, "INVALID_INPUT");
    };
    static CAPACITY: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    let _permit = match CAPACITY
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(4)))
        .clone()
        .try_acquire_owned()
    {
        Ok(permit) => permit,
        Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE"),
    };
    let (content_type, idempotency_key) =
        match request_metadata(&method, route, request.headers(), &state.browser_auth_origins) {
            Ok(value) => value,
            Err((status, reason)) => return error(status, reason),
        };
    let mutation = matches!(
        method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );
    let body = match to_bytes(request.into_body(), route.body_limit).await {
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
    // ── The routing rule ───────────────────────────────────────────────────
    //
    // A profile operation is served by the service that owns its tables.
    // `profile_owner::owner_of` is that rule, as data. An operation with no
    // owner is refused rather than defaulted: sending it to the wrong service
    // would answer it from a database that does not hold its rows.
    let owner = match super::profile_owner::owner_of(route.operation) {
        Some(owner) => owner,
        None => return error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE"),
    };
    let Some(reader) = &state.concert_identity else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE");
    };
    let context = match access.borrowed() {
        Ok(context) => context,
        Err(_) => return error(StatusCode::UNAUTHORIZED, "AUTHENTICATION_REQUIRED"),
    };
    let http = ProfileHttpRequest {
        request_id: Uuid::new_v4(),
        method: method.to_string(),
        target: uri
            .path_and_query()
            .map(|value| value.as_str())
            .unwrap_or(uri.path())
            .to_owned(),
        body: body.to_vec(),
        content_type,
        idempotency_key,
    };
    // Both branches produce the same closed representation; only the serving
    // workload and the message type differ.
    let representation = match owner {
        ProfileOwner::ProfileService => {
            let Some(client) = &state.profile_product_client else {
                return error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE");
            };
            match Box::pin(client.dispatch(reader, context, http)).await {
                Ok(response) => {
                    let delivery = match response.delivery {
                        Some(capability) => match client.delivery_token(capability) {
                            Ok(token) => token,
                            Err(_) => {
                                return error(
                                    StatusCode::SERVICE_UNAVAILABLE,
                                    "DEPENDENCY_UNAVAILABLE",
                                )
                            }
                        },
                        None => {
                            return error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE")
                        }
                    };
                    Representation {
                        status: response.status,
                        content_type: response.content_type,
                        body: response.body,
                        content_expires_unix_micros: response.content_expires_unix_micros,
                        delivery,
                    }
                }
                Err(status) => return rpc_error(status),
            }
        }
        ProfileOwner::ConcertService => {
            let Some(client) = &state.concert_profile_product_client else {
                return error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE");
            };
            match Box::pin(client.dispatch(reader, context, http)).await {
                Ok(response) => {
                    let delivery = match response.delivery {
                        Some(capability) => match client.delivery_token(capability) {
                            Ok(token) => token,
                            Err(_) => {
                                return error(
                                    StatusCode::SERVICE_UNAVAILABLE,
                                    "DEPENDENCY_UNAVAILABLE",
                                )
                            }
                        },
                        None => {
                            return error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE")
                        }
                    };
                    Representation {
                        status: response.status,
                        content_type: response.content_type,
                        body: response.body,
                        content_expires_unix_micros: response.content_expires_unix_micros,
                        delivery,
                    }
                }
                Err(status) => return rpc_error(status),
            }
        }
    };
    if access.check().is_err() {
        return error(StatusCode::UNAUTHORIZED, "AUTHENTICATION_REQUIRED");
    }
    let status = match representation.status {
        200 | 201 | 202 | 400 | 401 | 403 | 404 | 409 | 413 | 415 | 429 | 500 | 503 | 504 => {
            StatusCode::from_u16(representation.status as u16).expect("closed status")
        }
        _ => return error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE"),
    };
    let image = is_image_type(&representation.content_type);
    if image && (status != StatusCode::OK || !uri.path().ends_with("/content"))
        || (!image
            && (representation.content_type != "application/json"
                || serde_json::from_slice::<serde_json::Value>(&representation.body).is_err()))
    {
        return error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE");
    }
    let original = match access.rpc_deadline() {
        Ok(value) => value,
        Err(_) => return error(StatusCode::UNAUTHORIZED, "AUTHENTICATION_REQUIRED"),
    };
    let body = match super::response_disclosure::DisclosureBody::new(
        Body::from(representation.body),
        representation.content_expires_unix_micros,
        original,
    ) {
        Ok(body) => Body::new(body),
        Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE"),
    };
    let mut response = Response::new(body);
    *response.status_mut() = status;
    media_headers(&mut response, &representation.content_type);
    protect(&mut response);
    response.extensions_mut().insert(representation.delivery);
    response
}

/// One closed profile representation, whichever service produced it.
///
/// The two transports carry identical fields, so the framing below is shared:
/// a browser cannot tell which service answered, and neither can a cache.
struct Representation {
    status: u32,
    content_type: String,
    body: Vec<u8>,
    content_expires_unix_micros: i64,
    delivery: platform::actor_delegation::response_scope::ResponseScopeToken,
}

/// The exact response framing of one profile representation.
///
/// A profile media response has to be indistinguishable from the concert media
/// response (`concert_product`), because the browser treats both as private
/// images. The framing is therefore one function, used by the live handler and
/// asserted by the boundary tests:
///
/// * `content-type` is the closed image type, or `application/json`;
/// * an image is `inline` and `cross-origin-resource-policy: same-origin`, so it
///   can only be embedded by this origin;
/// * every representation (image or JSON) is `cache-control: no-store` and
///   `x-content-type-options: nosniff` through [`protect`], so a shared cache
///   never stores a private photograph and a browser never sniffs a different
///   type from the bytes.
fn media_headers(response: &mut Response, content_type: &str) {
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type).expect("closed content type"),
    );
    if is_image_type(content_type) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, HeaderValue::from_static("inline"));
        response.headers_mut().insert(
            "cross-origin-resource-policy",
            HeaderValue::from_static("same-origin"),
        );
    }
}

/// Build a fully framed profile media response.
///
/// This is the same framing the live handler applies, exposed so the boundary
/// tests assert the real function rather than a copy of it. `body` is the
/// representation the producer returned: image bytes for a `READY` asset, or the
/// closed JSON envelope for a refusal.
pub fn framed_media_response(status: u16, content_type: &str, body: Vec<u8>) -> Response {
    let mut response = Response::new(Body::from(body));
    *response.status_mut() =
        StatusCode::from_u16(status).expect("profile media response status is closed");
    media_headers(&mut response, content_type);
    protect(&mut response);
    response
}

/// Whether a content type is one of the three image types the foundation
/// supports. GIF and video are not, and are never served by this surface.
fn is_image_type(content_type: &str) -> bool {
    matches!(
        content_type,
        "image/jpeg" | "image/png" | "image/webp"
    )
}

/// Cover outer transport rejections too, without discarding owner recovery
/// details. Mirrors `concert_product::protect_response`.
pub async fn protect_response(request: Request, next: axum::middleware::Next) -> Response {
    if is_profile_path(request.uri().path())
        && request
            .extensions()
            .get::<axum::extract::MatchedPath>()
            .is_none()
    {
        return error(StatusCode::NOT_FOUND, "RESOURCE_UNAVAILABLE");
    }
    let protected = is_profile_path(request.uri().path());
    let path = request.uri().path().to_owned();
    let mut response = next.run(request).await;
    if !protected {
        return response;
    }

    if response
        .extensions()
        .get::<ProfileRepresentation>()
        .is_none()
        && (response.status().is_client_error() || response.status().is_server_error())
    {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> axum::http::HeaderMap {
        let mut headers = axum::http::HeaderMap::new();
        for (name, value) in pairs {
            headers.insert(
                axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        headers
    }

    const ORIGINS: &[&str] = &["https://mcr.example"];

    fn origins() -> Vec<String> {
        ORIGINS.iter().map(|origin| (*origin).to_owned()).collect()
    }

    /// A bodyless `DELETE` is still a mutation, so it needs an
    /// `Idempotency-Key` before the request is forwarded.
    ///
    /// The end-to-end gate's intro-delete probe sent no key and got
    /// `INVALID_INPUT`. The product is right to refuse: a delete is not
    /// replay-safe without a key, and the gateway cannot tell a retry from a
    /// second delete. This test pins the requirement and records the cause next
    /// to the code that produces it, so the next reader does not have to
    /// rediscover that the 400 never reached the intro parser.
    #[test]
    fn a_delete_requires_an_idempotency_key() {
        let route = ROUTES
            .iter()
            .find(|route| route.operation == "deleteIntroPost")
            .expect("the intro delete route exists");
        let method = Method::DELETE;
        let without = request_metadata(
            &method,
            route,
            &headers(&[
                ("origin", "https://mcr.example"),
                ("x-mcr-intent", "browser"),
            ]),
            &origins(),
        );
        assert_eq!(
            without,
            Err((StatusCode::BAD_REQUEST, "INVALID_INPUT")),
            "a bodyless delete without a key is refused before dispatch"
        );
        // With a UUID key the same request is accepted, and no content type is
        // required because the body is empty.
        let with = request_metadata(
            &method,
            route,
            &headers(&[
                ("origin", "https://mcr.example"),
                ("x-mcr-intent", "browser"),
                ("idempotency-key", "00000000-0000-4000-8000-000000000001"),
            ]),
            &origins(),
        );
        assert_eq!(
            with,
            Ok((String::new(), "00000000-0000-4000-8000-000000000001".to_owned()))
        );
    }

    #[test]
    fn every_route_is_a_frozen_operation_and_no_route_names_an_event() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .unwrap();
        // The frozen register is split across the core and domain bundles.
        let mut rows: Vec<serde_json::Value> = Vec::new();
        for bundle in ["contracts/core/v1/operations.json", "contracts/domain/v1/operations.json"] {
            rows.extend(
                serde_json::from_slice::<Vec<serde_json::Value>>(
                    &std::fs::read(root.join(bundle)).unwrap(),
                )
                .unwrap(),
            );
        }
        for route in ROUTES {
            assert!(
                !route.path.contains("events") && !route.path.contains(":event_id"),
                "a profile route must never carry an event: {route:?}"
            );
            let row = rows
                .iter()
                .find(|row| row["operationId"] == route.operation)
                .unwrap_or_else(|| panic!("{} is not a frozen operation", route.operation));
            assert_eq!(
                row["method"].as_str().unwrap().to_ascii_uppercase(),
                route.method,
                "{} method",
                route.operation
            );
            // The contract path and the axum template must describe the same
            // route: identical segments, with `{param}` and `:param` equivalent.
            let contract: Vec<&str> = row["path"].as_str().unwrap().split('/').collect();
            let template: Vec<&str> = route.path.split('/').collect();
            assert_eq!(contract.len(), template.len(), "{} path", route.operation);
            for (want, got) in contract.iter().zip(template.iter()) {
                if want.starts_with('{') {
                    assert!(got.starts_with(':'), "{} path", route.operation);
                } else {
                    assert_eq!(want, got, "{} path", route.operation);
                }
            }
        }
    }

    #[test]
    fn the_route_table_is_unique_and_matches_concrete_paths() {
        for (index, entry) in ROUTES.iter().enumerate() {
            for other in &ROUTES[index + 1..] {
                assert!(
                    !(entry.method == other.method && entry.path == other.path),
                    "duplicate route {entry:?}"
                );
            }
            let concrete = entry
                .path
                .replace(":profile_id", &Uuid::new_v4().to_string())
                .replace(":media_id", &Uuid::new_v4().to_string())
                .replace(":post_id", &Uuid::new_v4().to_string())
                .replace(":provider", "naver");
            let method = Method::from_bytes(entry.method.as_bytes()).unwrap();
            assert_eq!(
                route(&method, &concrete).map(|found| found.operation),
                Some(entry.operation),
                "{concrete}"
            );
            assert!(protects(&concrete));
            // A suffix is never matched.
            assert!(route(&method, &format!("{concrete}/extra")).is_none());
        }
    }

    #[test]
    fn event_paths_are_never_matched() {
        let event = Uuid::new_v4();
        for path in [
            format!("/api/concert/events/{event}/feed"),
            format!("/api/me/profile/events/{event}"),
            "/api/profiles/events/".to_owned(),
        ] {
            assert!(route(&Method::GET, &path).is_none(), "{path}");
            assert!(!protects(&path), "{path}");
            assert!(path.contains("/events/"));
        }
    }

    #[test]
    fn reads_carry_no_body_and_mutations_require_intent_and_origin() {
        let read = ROUTES
            .iter()
            .find(|route| route.operation == "getOwnProfile")
            .unwrap();
        assert_eq!(
            request_metadata(&Method::GET, read, &headers(&[]), &origins()),
            Ok((String::new(), String::new()))
        );
        // A read with a content type is refused.
        assert_eq!(
            request_metadata(
                &Method::GET,
                read,
                &headers(&[("content-type", "application/json")]),
                &origins()
            ),
            Err((StatusCode::BAD_REQUEST, "INVALID_INPUT"))
        );
        let mutation = ROUTES
            .iter()
            .find(|route| route.operation == "updateOwnProfile")
            .unwrap();
        let key = Uuid::new_v4().to_string();
        // No origin.
        assert_eq!(
            request_metadata(
                &Method::PATCH,
                mutation,
                &headers(&[
                    ("x-mcr-intent", "browser"),
                    ("idempotency-key", &key),
                    ("content-type", "application/json"),
                ]),
                &origins()
            ),
            Err((StatusCode::FORBIDDEN, "ORIGIN_NOT_ALLOWED"))
        );
        // No intent.
        assert_eq!(
            request_metadata(
                &Method::PATCH,
                mutation,
                &headers(&[
                    ("origin", "https://mcr.example"),
                    ("idempotency-key", &key),
                    ("content-type", "application/json"),
                ]),
                &origins()
            ),
            Err((StatusCode::FORBIDDEN, "BROWSER_INTENT_REQUIRED"))
        );
        // No idempotency key.
        assert_eq!(
            request_metadata(
                &Method::PATCH,
                mutation,
                &headers(&[
                    ("origin", "https://mcr.example"),
                    ("x-mcr-intent", "browser"),
                    ("content-type", "application/json"),
                ]),
                &origins()
            ),
            Err((StatusCode::BAD_REQUEST, "INVALID_INPUT"))
        );
        // Complete.
        assert_eq!(
            request_metadata(
                &Method::PATCH,
                mutation,
                &headers(&[
                    ("origin", "https://mcr.example"),
                    ("x-mcr-intent", "browser"),
                    ("idempotency-key", &key),
                    ("content-type", "application/json"),
                ]),
                &origins()
            ),
            Ok(("application/json".to_owned(), key))
        );
    }

    #[test]
    fn a_bytes_upload_accepts_only_the_image_allowlist() {
        let bytes = ROUTES
            .iter()
            .find(|route| route.operation == "putProfileMediaBytes")
            .unwrap();
        assert!(bytes.bytes);
        assert_eq!(bytes.body_limit, MAX_BODY);
        let key = Uuid::new_v4().to_string();
        let base = [
            ("origin", "https://mcr.example"),
            ("x-mcr-intent", "browser"),
            ("idempotency-key", key.as_str()),
        ];
        for content_type in ["image/jpeg", "image/png", "image/webp"] {
            let mut pairs = base.to_vec();
            pairs.push(("content-type", content_type));
            assert!(
                request_metadata(&Method::PUT, bytes, &headers(&pairs), &origins()).is_ok(),
                "{content_type}"
            );
        }
        let mut pairs = base.to_vec();
        pairs.push(("content-type", "image/gif"));
        assert_eq!(
            request_metadata(&Method::PUT, bytes, &headers(&pairs), &origins()),
            Err((StatusCode::UNSUPPORTED_MEDIA_TYPE, "INVALID_MEDIA_TYPE")),
            "GIF is not supported by the foundation and must be refused"
        );
    }

    #[test]
    fn a_bodyless_delete_carries_no_content_type() {
        let delete = ROUTES
            .iter()
            .find(|route| route.operation == "disconnectIdentityVerification")
            .unwrap();
        let key = Uuid::new_v4().to_string();
        assert!(request_metadata(
            &Method::DELETE,
            delete,
            &headers(&[
                ("origin", "https://mcr.example"),
                ("x-mcr-intent", "browser"),
                ("idempotency-key", &key),
            ]),
            &origins()
        )
        .is_ok());
        assert_eq!(
            request_metadata(
                &Method::DELETE,
                delete,
                &headers(&[
                    ("origin", "https://mcr.example"),
                    ("x-mcr-intent", "browser"),
                    ("idempotency-key", &key),
                    ("content-type", "application/json"),
                ]),
                &origins()
            ),
            Err((StatusCode::BAD_REQUEST, "INVALID_INPUT"))
        );
    }

    #[test]
    fn every_profile_representation_is_never_storeable() {
        let mut response = Response::new(Body::from(
            serde_json::json!({"nickname": "private"}).to_string(),
        ));
        response
            .headers_mut()
            .insert(header::ETAG, HeaderValue::from_static("\"stale\""));
        response
            .headers_mut()
            .insert(header::LAST_MODIFIED, HeaderValue::from_static("today"));
        protect(&mut response);
        let headers = response.headers();
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
        assert_eq!(headers[header::PRAGMA], "no-cache");
        assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert!(!headers.contains_key(header::ETAG));
        assert!(!headers.contains_key(header::LAST_MODIFIED));
    }
}
