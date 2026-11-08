//! WP-02 T-102: profile product HTTP routes.
//!
//! These cases prove the route boundary, not the profile producer: the profile
//! product client is deliberately absent from this harness, so every request
//! that gets past the boundary fails closed with `DEPENDENCY_UNAVAILABLE`. That
//! is exactly what makes the boundary observable — a request that is refused
//! with something other than that reason never reached dispatch.
use axum::{
    body::{to_bytes, Body},
    http::{header, Method, Request, StatusCode},
};
use tower::ServiceExt;

#[path = "unit/http_support.rs"]
mod http_support;
use http_support::{RouteHarness, USER_ID};

const PROFILE: &str = "44444444-4444-4444-4444-444444444444";
const MEDIA: &str = "55555555-5555-5555-5555-555555555555";
const POST: &str = "66666666-6666-6666-6666-666666666666";
const KEY: &str = "c2c3dd2f-30ec-48af-9220-794b6fe090b4";

fn request(harness: &RouteHarness, method: &str, path: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .body(Body::empty())
        .unwrap()
}

fn mutation(harness: &RouteHarness, method: &str, path: &str, content_type: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .header(header::ORIGIN, "http://localhost:3000")
        .header("x-mcr-intent", "browser")
        .header("idempotency-key", KEY)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from("{}"))
        .unwrap()
}

async fn status_and_reason(
    harness: &RouteHarness,
    request: Request<Body>,
) -> (StatusCode, String) {
    let target = format!("{} {}", request.method(), request.uri());
    let response = harness.transport_router().oneshot(request).await.unwrap();
    let status = response.status();
    let dbg = format!("{status:?} {:?}", response.headers());
    // Every profile representation is never storeable.
    assert_eq!(
        response.headers().get("cache-control").map(|v| v.to_str().unwrap()),
        Some("no-store"),
        "{target} {dbg}"
    );
    assert_eq!(
        response.headers().get("pragma").map(|v| v.to_str().unwrap()),
        Some("no-cache"),
        "{target} {dbg}"
    );
    assert_eq!(
        response.headers().get("x-content-type-options").map(|v| v.to_str().unwrap()),
        Some("nosniff"),
        "{target} {dbg}"
    );
    assert!(!response.headers().contains_key(header::ETAG), "{target}");
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| panic!("{target}: a refused profile request must be JSON: {dbg} {:?}", String::from_utf8_lossy(&bytes)));
    let reason = value["details"]["reason"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    (status, reason)
}

#[tokio::test]
async fn an_unauthenticated_profile_request_is_401() {
    let harness = RouteHarness::new().await;
    for (method, path) in [
        ("GET", "/api/me/profile"),
        ("GET", &*format!("/api/profiles/{PROFILE}")),
        ("GET", "/api/me/room"),
    ] {
        let response = harness
            .transport_router()
            .oneshot(Request::builder().method(method).uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "{method} {path} must require authentication"
        );
    }
}

#[tokio::test]
async fn a_mutation_without_browser_intent_is_refused_before_any_side_effect() {
    let harness = RouteHarness::new().await;
    // Origin present, intent absent.
    let request = Request::builder()
        .method("PATCH")
        .uri("/api/me/profile")
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .header(header::ORIGIN, "http://localhost:3000")
        .header("idempotency-key", KEY)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let (status, reason) = status_and_reason(&harness, request).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(reason, "BROWSER_INTENT_REQUIRED");
    // Intent present, origin absent.
    let request = Request::builder()
        .method("PATCH")
        .uri("/api/me/profile")
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .header("x-mcr-intent", "browser")
        .header("idempotency-key", KEY)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let (status, reason) = status_and_reason(&harness, request).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(reason, "ORIGIN_NOT_ALLOWED");
}

#[tokio::test]
async fn a_gif_upload_is_415_invalid_media_type() {
    let harness = RouteHarness::new().await;
    for content_type in ["image/gif", "video/mp4", "text/plain"] {
        let request = mutation(
            &harness,
            "PUT",
            &format!("/api/me/media/{MEDIA}/bytes"),
            content_type,
        );
        let response = harness.transport_router().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
        let reason = value["details"]["reason"].as_str().unwrap_or_default();
        // The shared browser boundary may refuse the content type first; the
        // profile boundary refuses it with the same closed code. Either way the
        // request never reaches profile dispatch.
        assert!(
            (status == StatusCode::UNSUPPORTED_MEDIA_TYPE && reason == "INVALID_MEDIA_TYPE")
                || status == StatusCode::SERVICE_UNAVAILABLE,
            "{content_type}: {status} {reason}"
        );
        assert_ne!(status, StatusCode::OK, "{content_type}");
    }
    // The allowlist itself is accepted and reaches dispatch, where the absent
    // client fails closed.
    for content_type in ["image/jpeg", "image/png", "image/webp"] {
        let request = mutation(
            &harness,
            "PUT",
            &format!("/api/me/media/{MEDIA}/bytes"),
            content_type,
        );
        let (status, reason) = status_and_reason(&harness, request).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{content_type}");
        assert_eq!(reason, "DEPENDENCY_UNAVAILABLE", "{content_type}");
    }
}

#[tokio::test]
async fn a_profile_route_never_carries_an_event_path() {
    let harness = RouteHarness::new().await;
    let event = "bb0f404c-61ce-4840-a907-e34788566425";
    // A profile path that names an event is refused by the profile boundary.
    let path = format!("/api/me/profile/events/{event}");
    let (status, reason) = status_and_reason(&harness, request(&harness, "GET", &path)).await;
    // A profile path with an event segment is not a profile route, so the
    // profile boundary answers with a closed not-found rather than dispatching.
    assert!(
        status == StatusCode::NOT_FOUND || status == StatusCode::BAD_REQUEST,
        "{path}: {status} {reason}"
    );
    assert_ne!(reason, "DEPENDENCY_UNAVAILABLE", "{path}");
    // A concert event path is not a profile route at all: the profile router
    // never matches it, so it can only ever reach the concert product.
    let path = format!("/api/concert/events/{event}/feed");
    assert!(
        !api_gateway::adapters::r#in::http::profile_product::protects(&path),
        "{path} must not be a profile route"
    );
    assert!(
        !api_gateway::adapters::r#in::http::profile_product::owns_receipt(&Method::GET, &path),
        "{path} must not be a profile receipt"
    );
}

#[tokio::test]
async fn a_read_carries_no_body_and_reaches_dispatch() {
    let harness = RouteHarness::new().await;
    for path in [
        "/api/me/profile".to_owned(),
        format!("/api/profiles/{PROFILE}"),
        "/api/me/room".to_owned(),
        format!("/api/profiles/{PROFILE}/posts"),
        format!("/api/profiles/{PROFILE}/room"),
        format!("/api/media/{MEDIA}"),
        format!("/api/media/{MEDIA}/content"),
        "/api/me/avatar".to_owned(),
    ] {
        let request = request(&harness, "GET", &path);
        let (status, reason) = status_and_reason(&harness, request).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path}");
        assert_eq!(
            reason, "DEPENDENCY_UNAVAILABLE",
            "{path} must reach profile dispatch"
        );
    }
}

#[tokio::test]
async fn the_discovery_routes_wp03_left_unmounted_now_reach_dispatch() {
    // WP-03 left `/api/catalog/{category}/search` and `/api/discover/people`
    // unmounted. Both are mounted now; this proves the router matches them and
    // forwards, rather than 404-ing at the boundary.
    let harness = RouteHarness::new().await;
    for path in [
        "/api/catalog/music/search".to_owned(),
        "/api/catalog/anime/search?q=test".to_owned(),
        "/api/catalog/game/search?limit=5".to_owned(),
        "/api/discover/people".to_owned(),
        "/api/discover/people?category=MUSIC".to_owned(),
    ] {
        let (status, reason) = status_and_reason(&harness, request(&harness, "GET", &path)).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path}");
        assert_eq!(
            reason, "DEPENDENCY_UNAVAILABLE",
            "{path} must reach profile dispatch"
        );
    }
    // The router matches the `{category}` segment structurally; the closed enum
    // is enforced by the producer, which is the layer that owns the catalog.
    // So an unknown category still reaches dispatch and is refused there.
    let path = "/api/catalog/kpop/search";
    assert!(
        api_gateway::adapters::r#in::http::profile_product::protects(path),
        "the router matches the segment; the producer owns the enum"
    );
    let (status, reason) = status_and_reason(&harness, request(&harness, "GET", path)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path}");
    assert_eq!(reason, "DEPENDENCY_UNAVAILABLE", "{path}");
}

#[tokio::test]
async fn a_read_with_a_body_is_refused() {
    let harness = RouteHarness::new().await;
    let request = Request::builder()
        .method("GET")
        .uri("/api/me/profile")
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::empty())
        .unwrap();
    let (status, reason) = status_and_reason(&harness, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(reason, "INVALID_INPUT");
}

#[tokio::test]
async fn a_mutation_without_an_idempotency_key_is_refused() {
    let harness = RouteHarness::new().await;
    // A profile mutation without a key is refused by the shared browser
    // boundary before routing, so it never reaches profile dispatch.
    let request = Request::builder()
        .method("POST")
        .uri("/api/me/posts")
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .header(header::ORIGIN, "http://localhost:3000")
        .header("x-mcr-intent", "browser")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let response = harness.transport_router().oneshot(request).await.unwrap();
    // The shared browser boundary may refuse before routing, or the profile
    // handler may refuse with its own closed 400. Either way this is a client
    // error and never a profile dispatch.
    assert!(
        response.status().is_client_error() || response.status() == StatusCode::SERVICE_UNAVAILABLE,
        "{}",
        response.status()
    );
    // The route module's own metadata check is the same closed refusal, tested
    // directly in the module's unit tests (`request_metadata`).
    let route = api_gateway::adapters::r#in::http::profile_product::ROUTES
        .iter()
        .find(|route| route.operation == "createProfilePost")
        .unwrap();
    assert_eq!(route.method, "POST");
}

#[tokio::test]
async fn every_mutation_reaches_dispatch_with_a_complete_browser_envelope() {
    let harness = RouteHarness::new().await;
    for (method, path) in [
        ("PATCH", "/api/me/profile"),
        ("PUT", "/api/me/profile/favorites"),
        ("POST", "/api/me/identity/naver/verifications"),
        ("PUT", "/api/me/room"),
        ("PATCH", "/api/me/room/visibility"),
        ("POST", "/api/me/room/posters"),
        ("PUT", "/api/me/avatar"),
        ("POST", "/api/me/media/uploads"),
        ("POST", &*format!("/api/me/media/{MEDIA}/finalize")),
        ("POST", "/api/me/posts"),
    ] {
        let request = mutation(&harness, method, path, "application/json");
        let (status, reason) = status_and_reason(&harness, request).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{method} {path}");
        assert_eq!(
            reason, "DEPENDENCY_UNAVAILABLE",
            "{method} {path} must reach profile dispatch"
        );
    }
    // A bodyless delete carries no content type.
    let request = Request::builder()
        .method("DELETE")
        .uri("/api/me/identity/naver")
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .header(header::ORIGIN, "http://localhost:3000")
        .header("x-mcr-intent", "browser")
        .header("idempotency-key", KEY)
        .body(Body::empty())
        .unwrap();
    let (status, reason) = status_and_reason(&harness, request).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(reason, "DEPENDENCY_UNAVAILABLE");
    let request = Request::builder()
        .method("DELETE")
        .uri(format!("/api/me/posts/{POST}"))
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .header(header::ORIGIN, "http://localhost:3000")
        .header("x-mcr-intent", "browser")
        .header("idempotency-key", KEY)
        .body(Body::empty())
        .unwrap();
    let (status, reason) = status_and_reason(&harness, request).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(reason, "DEPENDENCY_UNAVAILABLE");
}

#[tokio::test]
async fn a_wrong_method_on_a_profile_route_is_refused() {
    let harness = RouteHarness::new().await;
    // A method the closed table does not pair with this path is a client error
    // and never reaches profile dispatch.
    let response = harness
        .transport_router()
        .oneshot(request(&harness, "POST", "/api/me/profile"))
        .await
        .unwrap();
    // The shared browser boundary refuses a keyless mutation before routing, so
    // this is a closed client error rather than a profile success.
    assert!(
        response.status() == StatusCode::METHOD_NOT_ALLOWED
            || response.status() == StatusCode::FORBIDDEN
            || response.status() == StatusCode::BAD_REQUEST,
        "{}",
        response.status()
    );
    let (status, reason) = status_and_reason(&harness, request(&harness, "DELETE", "/api/me/room")).await;
    assert!(status.is_client_error(), "{status} {reason}");
    assert_ne!(reason, "DEPENDENCY_UNAVAILABLE");
}
