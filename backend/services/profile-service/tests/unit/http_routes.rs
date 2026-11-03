use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use profile_service::config::ProfileMetrics;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tower::ServiceExt;

#[tokio::test]
async fn application_router_does_not_expose_metrics() {
    // Given: the profile application router.
    let router = profile_service::adapters::r#in::http::router();

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
async fn metrics_router_exposes_only_metrics_with_profile_counters() {
    // Given: profile metrics containing one retention run.
    let metrics = Arc::new(ProfileMetrics::default());
    metrics.record_retention_purge_run("success");
    metrics.record_mutation_receipt_retention_run("success");
    metrics.record_mutation_receipt_outcome_bodies_purged(3);
    let router = profile_service::adapters::r#in::http::metrics_router(metrics);

    // When: metrics and a representative profile path are requested.
    let metrics_response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .expect("metrics request"),
        )
        .await
        .expect("metrics response");
    let business_response = router
        .oneshot(
            Request::builder()
                .uri("/profiles/00000000-0000-0000-0000-000000000001")
                .body(Body::empty())
                .expect("profile request"),
        )
        .await
        .expect("profile response");

    // Then: only metrics are exposed and include the recorded counter.
    assert_eq!(metrics_response.status(), StatusCode::OK);
    let body = to_bytes(metrics_response.into_body(), 1 << 20)
        .await
        .expect("metrics body");
    assert!(std::str::from_utf8(&body)
        .expect("utf8 metrics")
        .contains("profile_retention_purge_runs_total{outcome=\"success\"} 1"));
    let metrics_text = std::str::from_utf8(&body).expect("utf8 metrics");
    assert!(metrics_text
        .contains("profile_mutation_receipt_retention_runs_total{outcome=\"success\"} 1"));
    assert!(metrics_text.contains("profile_mutation_receipt_outcome_bodies_purged_total{} 3"));
    assert_eq!(business_response.status(), StatusCode::NOT_FOUND);
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
    let metrics = Arc::new(ProfileMetrics::default());
    metrics.record_retention_purge_run("success");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = tokio::spawn(profile_service::adapters::r#in::http::serve_until(
        application_listener,
        profile_service::adapters::r#in::http::router(),
        metrics_listener,
        profile_service::adapters::r#in::http::metrics_router(metrics),
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
    assert!(metrics_response.contains("profile_retention_purge_runs_total{outcome=\"success\"} 1"));
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
    let error = profile_service::adapters::r#in::http::bind_listeners(
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
