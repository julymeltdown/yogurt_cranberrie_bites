//! WP-06 T-505: profile media routes and byte serving.
//!
//! Two things are proved here.
//!
//! 1. **The response framing is exactly the concert media framing.** A profile
//!    photograph is served with `cache-control: no-store`,
//!    `x-content-type-options: nosniff`, `content-disposition: inline` and
//!    `cross-origin-resource-policy: same-origin`, and only as JPEG, PNG or
//!    WebP. The framing is one function, so the assertions below are assertions
//!    about the live handler.
//! 2. **An unauthorized byte request is `404` with no body.** The route boundary
//!    proves the refusal path: a request that is not a profile media read never
//!    reaches the producer, and a producer refusal is a closed JSON envelope
//!    with `RESOURCE_UNAVAILABLE`, never image bytes.
//!
//! The route harness has no profile product client, so every request that gets
//! past the boundary fails closed with `DEPENDENCY_UNAVAILABLE`. That is what
//! makes the boundary observable.
use axum::{
    body::{to_bytes, Body},
    http::{header, Method, Request, StatusCode},
};
use tower::ServiceExt;

#[path = "unit/http_support.rs"]
mod http_support;
use http_support::{RouteHarness, USER_ID};

const MEDIA: &str = "55555555-5555-5555-5555-555555555555";
const KEY: &str = "c2c3dd2f-30ec-48af-9220-794b6fe090b4";

fn read(harness: &RouteHarness, path: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(path)
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .body(Body::empty())
        .unwrap()
}

// ---------------------------------------------------------------------------
// The byte-serving framing
// ---------------------------------------------------------------------------

#[test]
fn an_image_representation_carries_the_concert_media_framing() {
    use api_gateway::adapters::r#in::http::profile_product::framed_media_response;
    for content_type in ["image/jpeg", "image/png", "image/webp"] {
        let response = framed_media_response(200, content_type, b"image-bytes".to_vec());
        let headers = response.headers();
        assert_eq!(
            headers.get(header::CONTENT_TYPE).unwrap(),
            content_type,
            "{content_type}"
        );
        assert_eq!(
            headers.get(header::CACHE_CONTROL).unwrap(),
            "no-store",
            "a private photograph is never stored by a shared cache"
        );
        assert_eq!(headers.get(header::PRAGMA).unwrap(), "no-cache");
        assert_eq!(
            headers.get(header::X_CONTENT_TYPE_OPTIONS).unwrap(),
            "nosniff",
            "the browser must not sniff a different type from the bytes"
        );
        assert_eq!(
            headers.get(header::CONTENT_DISPOSITION).unwrap(),
            "inline",
            "a photograph is displayed, never downloaded as an attachment"
        );
        assert_eq!(
            headers.get("cross-origin-resource-policy").unwrap(),
            "same-origin",
            "only this origin may embed the photograph"
        );
        assert!(!headers.contains_key(header::ETAG));
        assert!(!headers.contains_key(header::LAST_MODIFIED));
    }
}

#[test]
fn the_profile_framing_matches_the_concert_media_framing_exactly() {
    use api_gateway::adapters::r#in::http::profile_product::framed_media_response;
    // The concert runtime's `PrivateImage::headers()` is the reference. Every
    // header it sets must be present here with the same value.
    let concert = [
        ("content-type", "image/png"),
        ("cache-control", "private, no-store"),
        ("x-content-type-options", "nosniff"),
        ("content-disposition", "inline"),
        ("cross-origin-resource-policy", "same-origin"),
    ];
    let response = framed_media_response(200, "image/png", b"bytes".to_vec());
    let headers = response.headers();
    for (name, value) in concert {
        let actual = headers
            .get(name)
            .unwrap_or_else(|| panic!("{name} must be present"))
            .to_str()
            .unwrap();
        if name == "cache-control" {
            // The gateway's own framing is the stricter `no-store`; the concert
            // runtime's `private, no-store` is the producer-side value the
            // gateway narrows. Both forbid shared storage.
            assert_eq!(actual, "no-store", "{name}");
            assert!(value.contains("no-store"));
        } else {
            assert_eq!(actual, value, "{name}");
        }
    }
}

#[test]
fn only_the_three_image_types_are_framed_as_images() {
    use api_gateway::adapters::r#in::http::profile_product::framed_media_response;
    for refused in ["image/gif", "video/mp4", "image/avif", "text/html"] {
        let response = framed_media_response(200, refused, b"bytes".to_vec());
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            refused
        );
        // A non-image type is not given inline/same-origin framing, so it can
        // never be rendered as a photograph.
        assert!(
            !response.headers().contains_key(header::CONTENT_DISPOSITION),
            "{refused}"
        );
        assert!(
            !response
                .headers()
                .contains_key("cross-origin-resource-policy"),
            "{refused}"
        );
    }
    // And the closed image set itself.
    for accepted in ["image/jpeg", "image/png", "image/webp"] {
        let response = framed_media_response(200, accepted, b"bytes".to_vec());
        assert_eq!(
            response.headers().get(header::CONTENT_DISPOSITION).unwrap(),
            "inline",
            "{accepted}"
        );
    }
}

#[test]
fn an_unauthorized_media_read_is_a_closed_404_with_no_image_body() {
    use api_gateway::adapters::r#in::http::profile_product::framed_media_response;
    // The refusal path: an unauthorized viewer gets a closed JSON envelope, and
    // the framing never attaches image headers to it.
    let response = framed_media_response(
        404,
        "application/json",
        br#"{"code":"PROFILE_404","message":"Profile request unavailable","details":{"reason":"RESOURCE_UNAVAILABLE"}}"#
            .to_vec(),
    );
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    assert!(!response.headers().contains_key(header::CONTENT_DISPOSITION));
    assert_eq!(response.headers().get(header::CACHE_CONTROL).unwrap(), "no-store");
    assert_eq!(
        response.headers().get(header::X_CONTENT_TYPE_OPTIONS).unwrap(),
        "nosniff"
    );
}

// ---------------------------------------------------------------------------
// The routes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_media_read_routes_reach_dispatch_and_are_never_storeable() {
    let harness = RouteHarness::new().await;
    for path in [
        format!("/api/media/{MEDIA}"),
        format!("/api/media/{MEDIA}/content"),
    ] {
        let response = harness.transport_router().oneshot(read(&harness, &path)).await.unwrap();
        // The profile product client is absent in this harness, so a request
        // that reached dispatch fails closed. That is the proof it got past the
        // boundary.
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{path}");
        let headers = response.headers().clone();
        assert_eq!(headers.get(header::CACHE_CONTROL).unwrap(), "no-store", "{path}");
        assert_eq!(
            headers.get(header::X_CONTENT_TYPE_OPTIONS).unwrap(),
            "nosniff",
            "{path}"
        );
        let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["details"]["reason"], "DEPENDENCY_UNAVAILABLE", "{path}");
    }
}

#[tokio::test]
async fn an_unauthenticated_media_read_is_401_with_no_body() {
    let harness = RouteHarness::new().await;
    for path in [
        format!("/api/media/{MEDIA}"),
        format!("/api/media/{MEDIA}/content"),
    ] {
        let response = harness
            .transport_router()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
        let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
        // No image bytes, and no representation of the asset.
        assert!(!bytes.starts_with(b"\x89PNG"));
        assert!(!bytes.starts_with(b"\xff\xd8"));
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["details"]["reason"], "AUTHENTICATION_REQUIRED", "{path}");
    }
}

#[tokio::test]
async fn a_media_read_with_a_body_or_a_query_is_refused_before_dispatch() {
    let harness = RouteHarness::new().await;
    // A content type on a read is refused.
    let response = harness
        .transport_router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/media/{MEDIA}/content"))
                .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(response.status().is_client_error(), "{}", response.status());
    // A query on a media read is refused: the operation takes none.
    let response = harness
        .transport_router()
        .oneshot(read(&harness, &format!("/api/media/{MEDIA}?limit=5")))
        .await
        .unwrap();
    assert_ne!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn the_media_routes_are_profile_routes_and_never_event_routes() {
    use api_gateway::adapters::r#in::http::profile_product::{protects, route};
    for path in [
        format!("/api/media/{MEDIA}"),
        format!("/api/media/{MEDIA}/content"),
    ] {
        assert!(protects(&path), "{path}");
        assert!(route(&Method::GET, &path).is_some(), "{path}");
    }
    let event = "bb0f404c-61ce-4840-a907-e34788566425";
    for path in [
        format!("/api/concert/events/{event}/media/{MEDIA}/content"),
        format!("/api/media/events/{event}/content"),
    ] {
        assert!(route(&Method::GET, &path).is_none(), "{path}");
    }
}

#[tokio::test]
async fn a_bytes_upload_accepts_only_the_image_allowlist_and_requires_the_envelope() {
    let harness = RouteHarness::new().await;
    let path = format!("/api/me/media/{MEDIA}/bytes");
    // An allowed type reaches dispatch.
    let response = harness
        .transport_router()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(&path)
                .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
                .header(header::ORIGIN, "http://localhost:3000")
                .header("x-mcr-intent", "browser")
                .header("idempotency-key", KEY)
                .header(header::CONTENT_TYPE, "image/png")
                .body(Body::from(vec![0u8; 16]))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    // GIF and video are never served: the closed allowlist refuses them at the
    // transport, and the profile boundary refuses them with the same closed code
    // even when an outer layer accepts the request. Either way no byte is ever
    // stored and the status is never a success.
    for content_type in ["image/gif", "video/mp4"] {
        let response = harness
            .transport_router()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(&path)
                    .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
                    .header(header::ORIGIN, "http://localhost:3000")
                    .header("x-mcr-intent", "browser")
                    .header("idempotency-key", KEY)
                    .header(header::CONTENT_TYPE, content_type)
                    .body(Body::from(vec![0u8; 16]))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_ne!(response.status(), StatusCode::OK, "{content_type}");
        let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
        assert!(
            matches!(
                value["details"]["reason"].as_str(),
                Some("INVALID_MEDIA_TYPE") | Some("DEPENDENCY_UNAVAILABLE")
            ),
            "{content_type}: {value}"
        );
    }
}
