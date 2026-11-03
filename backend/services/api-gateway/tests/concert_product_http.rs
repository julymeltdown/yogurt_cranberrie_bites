//! Transport-boundary fixtures, not canonical producer acceptance evidence.
#[path = "unit/http_support.rs"]
mod http_support;
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use http_support::{RouteHarness, USER_ID};
use tower::ServiceExt;

const EVENT: &str = "bb0f404c-61ce-4840-a907-e34788566425";
const MEDIA: &str = "6862ce9a-00fe-42a6-b806-608586f87f9c";
const KEY: &str = "c2c3dd2f-30ec-48af-9220-794b6fe090b4";

async fn json(response: axum::response::Response, status: StatusCode) -> serde_json::Value {
    assert_eq!(response.status(), status);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        response.headers()[header::X_CONTENT_TYPE_OPTIONS],
        "nosniff"
    );
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(value["details"]["reason"].is_string());
    assert!(!String::from_utf8(bytes.to_vec())
        .unwrap()
        .contains("postgres"));
    value
}

#[tokio::test]
async fn actual_transport_keeps_product_reads_authenticated_and_closed_without_producers() {
    let harness = RouteHarness::new().await;
    let router = harness.transport_router();
    let uri = format!("/api/concert/events/{EVENT}/feed");
    let request = Request::builder().uri(&uri).body(Body::empty()).unwrap();
    let value = json(
        router.clone().oneshot(request).await.unwrap(),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    assert_eq!(value["details"]["reason"], "AUTHENTICATION_REQUIRED");
    let request = Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .body(Body::empty())
        .unwrap();
    let value = json(
        router.oneshot(request).await.unwrap(),
        StatusCode::SERVICE_UNAVAILABLE,
    )
    .await;
    assert_eq!(value["details"]["reason"], "DEPENDENCY_UNAVAILABLE");
    harness.pool.close().await;
}

#[tokio::test]
async fn upload_uses_concert_receipts_without_gateway_two_megabyte_journaling() {
    let harness = RouteHarness::new().await;
    let router = harness.transport_router();
    // The retained journal would reject this >2MiB request before dispatch and
    // attempt a database write. Product admission must instead reach its own
    // fail-closed unconfigured producer boundary without any database connection.
    let request = Request::builder()
        .method("PUT")
        .uri(format!("/api/concert/events/{EVENT}/media/{MEDIA}/bytes"))
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .header(header::ORIGIN, "http://localhost:3000")
        .header("x-mcr-intent", "browser")
        .header("idempotency-key", KEY)
        .header(header::CONTENT_TYPE, "image/png")
        .body(Body::from(vec![0u8; 3 * 1024 * 1024]))
        .unwrap();
    let value = json(
        router.oneshot(request).await.unwrap(),
        StatusCode::SERVICE_UNAVAILABLE,
    )
    .await;
    assert_eq!(value["details"]["reason"], "DEPENDENCY_UNAVAILABLE");
    harness.pool.close().await;
}

#[tokio::test]
async fn product_mutations_reject_missing_origin_bad_key_and_oversize_before_rpc() {
    let harness = RouteHarness::new().await;
    for (origin, key, size, expected, reason) in [
        (None, KEY, 1, StatusCode::FORBIDDEN, "ORIGIN_NOT_ALLOWED"),
        (
            Some("http://localhost:3000"),
            "not-a-uuid",
            1,
            StatusCode::BAD_REQUEST,
            "INVALID_INPUT",
        ),
        (
            Some("http://localhost:3000"),
            KEY,
            10 * 1024 * 1024 + 1,
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
        ),
    ] {
        let mut request = Request::builder()
            .method("PUT")
            .uri(format!("/api/concert/events/{EVENT}/media/{MEDIA}/bytes"))
            .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
            .header("x-mcr-intent", "browser")
            .header("idempotency-key", key)
            .header(header::CONTENT_TYPE, "image/png");
        if let Some(origin) = origin {
            request = request.header(header::ORIGIN, origin);
        }
        let value = json(
            harness
                .transport_router()
                .oneshot(request.body(Body::from(vec![0u8; size])).unwrap())
                .await
                .unwrap(),
            expected,
        )
        .await;
        assert_eq!(value["details"]["reason"], reason);
    }
    harness.pool.close().await;
}
