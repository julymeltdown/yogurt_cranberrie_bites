use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use platform::{
    readiness::{Probe, Readiness},
    shutdown::Shutdown,
};
use tower::ServiceExt;
#[tokio::test]
async fn operational_health_keeps_liveness_while_draining_and_exposes_no_product_route() {
    let shutdown = Shutdown::new();
    let readiness = Readiness::with_shutdown(
        Probe::new(
            "database",
            true,
            std::time::Duration::from_millis(50),
            || async { true },
        ),
        shutdown.clone(),
    );
    let app = concert_service::health::router(readiness);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/readyz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    shutdown.request();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/readyz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let value: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(value["draining"], true);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/livez")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/concert/events")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
