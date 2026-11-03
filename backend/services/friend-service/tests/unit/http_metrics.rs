use axum::body::{to_bytes, Body};
use axum::http::{header::CONTENT_TYPE, Request, StatusCode};
use friend_service::config::retention_metrics::MutationReceiptRetentionMetrics;
use platform::edge::Edge;
use platform::edge_metrics::EdgeDeadlineMetrics;
use platform::recommendation_metrics::{
    RecommendationMetricOutcome, RecommendationMetricSurface, RecommendationMetrics,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tower::ServiceExt;

#[tokio::test]
async fn metrics_route_exports_bounded_recommendation_metrics_as_prometheus_text() {
    let metrics = Arc::new(RecommendationMetrics::default());
    metrics.record_request(
        RecommendationMetricSurface::People,
        RecommendationMetricOutcome::Success,
    );
    let receipt_metrics = Arc::new(MutationReceiptRetentionMetrics::default());
    receipt_metrics.record_success(2);
    receipt_metrics.record_failure();
    let edge_deadlines = EdgeDeadlineMetrics::default();
    edge_deadlines.record_deadline_exhausted(Edge::FriendProfile);
    let response = friend_service::adapters::r#in::http::metrics_router_with_edge_deadlines(
        metrics,
        receipt_metrics,
        edge_deadlines,
    )
    .oneshot(
        Request::builder()
            .uri("/metrics")
            .body(Body::empty())
            .expect("metrics request"),
    )
    .await
    .expect("metrics response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/plain; version=0.0.4; charset=utf-8")
    );
    let body = to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("metrics body");
    let body = std::str::from_utf8(&body).expect("utf8 metrics");
    assert!(
        body.contains("recommendation_requests_total{surface=\"people\",outcome=\"success\"} 1")
    );
    assert!(body.contains("friend_mutation_receipt_retention_runs_total{outcome=\"success\"} 1"));
    assert!(body.contains("friend_mutation_receipt_retention_runs_total{outcome=\"failure\"} 1"));
    assert!(body.contains("friend_mutation_receipt_outcome_bodies_purged_total 2"));
    assert!(body.contains("grpc_edge_deadline_exhaustions_total{edge=\"friend->profile\"} 1"));
}

#[tokio::test]
async fn application_router_does_not_expose_metrics() {
    // Given: the friend application router.
    let router = friend_service::adapters::r#in::http::router();

    // When: metrics are requested from the application surface.
    let response = router
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .expect("metrics request"),
        )
        .await
        .expect("application response");

    // Then: the application surface does not expose metrics.
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn metrics_router_rejects_relationship_and_recommendation_controls() {
    // Given: the dedicated friend metrics router.
    let router = friend_service::adapters::r#in::http::metrics_router(
        Arc::new(RecommendationMetrics::default()),
        Arc::new(MutationReceiptRetentionMetrics::default()),
    );

    // When: representative relationship and recommendation paths are requested.
    let relationship = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/relationships/00000000-0000-0000-0000-000000000001")
                .body(Body::empty())
                .expect("relationship request"),
        )
        .await
        .expect("relationship response");
    let recommendation = router
        .oneshot(
            Request::builder()
                .uri("/recommendations/people")
                .body(Body::empty())
                .expect("recommendation request"),
        )
        .await
        .expect("recommendation response");

    // Then: neither control surface is exposed on the metrics listener.
    assert_eq!(relationship.status(), StatusCode::NOT_FOUND);
    assert_eq!(recommendation.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn dual_listeners_serve_separate_surfaces_and_release_ephemeral_addresses() {
    // Given: independently bound ephemeral application and metrics listeners.
    let application_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("application bind");
    let application_addr = application_listener.local_addr().expect("application addr");
    let metrics_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("metrics bind");
    let metrics_addr = metrics_listener.local_addr().expect("metrics addr");
    let metrics = Arc::new(RecommendationMetrics::default());
    metrics.record_request(
        RecommendationMetricSurface::People,
        RecommendationMetricOutcome::Success,
    );
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = tokio::spawn(friend_service::adapters::r#in::http::serve_until(
        application_listener,
        friend_service::adapters::r#in::http::router(),
        metrics_listener,
        friend_service::adapters::r#in::http::metrics_router(
            metrics,
            Arc::new(MutationReceiptRetentionMetrics::default()),
        ),
        async move {
            match shutdown_rx.await {
                Ok(()) | Err(_) => {}
            }
        },
    ));

    // When: each live surface is scraped and graceful shutdown is requested.
    let application_response = scrape(application_addr, "/metrics").await;
    let metrics_response = scrape(metrics_addr, "/metrics").await;
    shutdown_tx.send(()).expect("shutdown receiver");
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .expect("server shutdown timeout")
        .expect("server task")
        .expect("server result");

    // Then: the surfaces are isolated and both addresses can be rebound.
    assert!(application_response.starts_with("HTTP/1.1 404"));
    assert!(metrics_response.starts_with("HTTP/1.1 200"));
    assert!(metrics_response
        .contains("recommendation_requests_total{surface=\"people\",outcome=\"success\"} 1"));
    let rebound_application = TcpListener::bind(application_addr)
        .await
        .expect("application rebind");
    let rebound_metrics = TcpListener::bind(metrics_addr)
        .await
        .expect("metrics rebind");
    drop((rebound_application, rebound_metrics));
}

#[tokio::test]
async fn duplicate_metrics_bind_returns_an_address_in_use_error() {
    // Given: an ephemeral address already owned by a listener.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("first bind");
    let addr = listener.local_addr().expect("listener addr");

    // When: another listener attempts to bind the same address.
    let error = friend_service::adapters::r#in::http::bind_listeners(
        "127.0.0.1:0".parse().expect("application addr"),
        addr,
    )
    .await
    .expect_err("duplicate bind must fail");

    // Then: the bind failure is explicit.
    assert!(error
        .to_string()
        .contains("failed to bind metrics listener"));
    assert!(error.to_string().contains(&addr.to_string()));
}

async fn scrape(addr: std::net::SocketAddr, path: &str) -> String {
    let mut stream = TcpStream::connect(addr).await.expect("connect listener");
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .await
        .expect("write request");
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .await
        .expect("read response");
    String::from_utf8(response).expect("http response utf8")
}
