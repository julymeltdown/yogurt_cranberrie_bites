//! Operations read routes must reach the authenticated product dispatch, not a
//! method-mismatch rejection. The absent producer is the fail-closed boundary.
#[path = "unit/http_support.rs"]
mod http_support;
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use http_support::{RouteHarness, USER_ID};
use tower::ServiceExt;

const EVENT: &str = "bb0f404c-61ce-4840-a907-e34788566425";
const ID: &str = "6862ce9a-00fe-42a6-b806-608586f87f9c";

fn read_routes() -> Vec<String> {
    [
        "admin/settings".to_owned(),
        "admin/places".to_owned(),
        "admin/notices".to_owned(),
        "admin/reports".to_owned(),
        format!("admin/reports/{ID}"),
        "notices".to_owned(),
        format!("notices/{ID}"),
    ]
    .into_iter()
    .map(|tail| format!("/api/concert/events/{EVENT}/{tail}"))
    .collect()
}

async fn check(harness: &RouteHarness, request: Request<Body>, status: StatusCode, reason: &str) {
    let target = format!("{} {}", request.method(), request.uri());
    let response = harness.transport_router().oneshot(request).await.unwrap();
    assert_eq!(response.status(), status, "{target}");
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        response.headers()[header::X_CONTENT_TYPE_OPTIONS],
        "nosniff"
    );
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["details"]["reason"], reason, "{target}: {value}");
}

#[tokio::test]
async fn every_operations_read_requires_bearer_authentication() {
    let harness = RouteHarness::new().await;
    for uri in read_routes() {
        check(
            &harness,
            Request::builder().uri(uri).body(Body::empty()).unwrap(),
            StatusCode::UNAUTHORIZED,
            "AUTHENTICATION_REQUIRED",
        )
        .await;
    }
    harness.pool.close().await;
}

#[tokio::test]
async fn every_operations_read_reaches_the_product_boundary_instead_of_method_rejection() {
    let harness = RouteHarness::new().await;
    for uri in read_routes() {
        let request = Request::builder()
            .uri(uri)
            .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
            .body(Body::empty())
            .unwrap();
        check(
            &harness,
            request,
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
        )
        .await;
    }
    harness.pool.close().await;
}
