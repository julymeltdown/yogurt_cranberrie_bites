use api_gateway::adapters::r#in::http::api_docs_routes;
use api_gateway::config::{DeveloperPortalLinkSettings, DeveloperPortalLinks, RuntimeMode};
use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn openapi_contract_is_served_from_the_gateway() {
    let response = api_docs_routes::router(
        RuntimeMode::Parity,
        DeveloperPortalLinks::defaults(RuntimeMode::Parity),
    )
    .oneshot(
        Request::builder()
            .uri("/openapi.json")
            .body(Body::empty())
            .expect("request"),
    )
    .await
    .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE),
        Some(&header::HeaderValue::from_static("application/json"))
    );
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-cache"))
    );

    let body = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .expect("body");
    let document: Value = serde_json::from_slice(&body).expect("OpenAPI JSON");
    assert_eq!(document["openapi"], "3.1.0");
    assert!(document["paths"]["/api/feed"].is_object());
}

#[tokio::test]
async fn developer_docs_index_points_to_swagger_ui_and_raw_contract() {
    let response = api_docs_routes::router(
        RuntimeMode::Parity,
        DeveloperPortalLinks::defaults(RuntimeMode::Parity),
    )
    .oneshot(
        Request::builder()
            .uri("/docs")
            .body(Body::empty())
            .expect("request"),
    )
    .await
    .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("body");
    let html = String::from_utf8(body.to_vec()).expect("utf8");
    assert!(html.contains("Port Rust Developer Portal"));
    assert!(html.contains("/openapi.json"));
    assert!(html.contains("http://localhost:8090"));
    assert!(html.contains("http://localhost:3000/\""));
    assert!(html.contains("grafana-pyroscope-app/explore"));
    assert!(html.contains("http://localhost:9090/\""));
    assert!(html.contains("http://localhost:4040/\""));
}

#[tokio::test]
async fn production_docs_index_contains_no_development_only_links_by_default() {
    // A localhost link in a production page points at the reader own machine, so
    // it either misleads them or invites a probe of a port that is not serving
    // anything. The development tools are real, but they are not part of this
    // deployment, so the production portal must not advertise them.
    let response = api_docs_routes::router(
        RuntimeMode::Production,
        DeveloperPortalLinks::defaults(RuntimeMode::Production),
    )
    .oneshot(
        Request::builder()
            .uri("/docs")
            .body(Body::empty())
            .expect("request"),
    )
    .await
    .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("body");
    let html = String::from_utf8(body.to_vec()).expect("utf8");
    assert!(
        !html.contains("localhost"),
        "the production portal must not link to localhost tools"
    );
    // The contract link stays: it is served by this deployment.
    assert!(html.contains("/openapi.json"));
    assert!(!html.contains("Swagger UI"));
    assert!(!html.contains("Grafana"));
    assert!(!html.contains("Prometheus"));
    assert!(!html.contains("Pyroscope"));
}

#[tokio::test]
async fn production_docs_index_displays_configured_external_links_safely() {
    let links = DeveloperPortalLinks::from_settings(
        RuntimeMode::Production,
        DeveloperPortalLinkSettings {
            swagger: Some("https://api.example.com/swagger".into()),
            grafana: Some("https://metrics.example.com/d/overview?orgId=4&from=now-1h".into()),
            profiles: Some("https://metrics.example.com/a/profiles/explore".into()),
            prometheus: Some("https://prometheus.example.com".into()),
            pyroscope: Some("https://profiles.example.com".into()),
        },
    )
    .expect("valid production URLs");
    let response = api_docs_routes::router(RuntimeMode::Production, links)
        .oneshot(
            Request::builder()
                .uri("/docs")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("body");
    let html = String::from_utf8(body.to_vec()).expect("utf8");
    assert!(html.contains("href=\"https://api.example.com/swagger\""));
    assert!(
        html.contains("href=\"https://metrics.example.com/d/overview?orgId=4&amp;from=now-1h\"")
    );
    assert!(html.contains("rel=\"noopener noreferrer\""));
    assert!(html.contains("/openapi.json"));
}
