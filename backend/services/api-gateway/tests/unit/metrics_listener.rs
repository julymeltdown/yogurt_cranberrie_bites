use crate::http_support::RouteHarness;
use api_gateway::adapters::r#in::http;
use api_gateway::adapters::r#in::http::server::{bind_http_listeners, serve_http_listeners};
use api_gateway::config::{GatewayMetrics, DEFAULT_METRICS_ADDR};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Router;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tower::ServiceExt;

#[test]
fn metrics_address_default_is_loopback_port_9080() {
    // Given: no gateway metrics address override.
    // When: the configuration default is inspected.
    // Then: metrics remain on the exact release-readiness loopback address.
    assert_eq!(DEFAULT_METRICS_ADDR, "127.0.0.1:9080");
}

#[tokio::test]
async fn application_router_does_not_serve_metrics() {
    // Given: the public gateway application router.
    let harness = RouteHarness::new().await;
    let router = http::router(harness.state);

    // When: a caller requests the metrics path on the application surface.
    let response = router
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    // Then: the public surface does not expose metrics.
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn metrics_router_serves_only_metrics() {
    // Given: a dedicated metrics router with one observable counter.
    let metrics = GatewayMetrics::default();
    metrics.inc_rate_limit_rejection("/auth/login");
    let router = http::metrics_router(metrics);

    // When/Then: the scrape succeeds on `/metrics`.
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);

    // When/Then: representative public routes are absent.
    for path in ["/docs", "/auth/login", "/.well-known/jwks.json"] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "path={path}");
    }
}

#[test]
fn content_moderation_backlog_metrics_are_bounded_scalar_gauges() {
    // Given: aggregate queue health with no identifier or variable labels.
    let metrics = GatewayMetrics::default();
    metrics.set_content_moderation_backlog(4, 321, 2, 1, 1_790_000_000);

    // When: the Prometheus exposition is rendered.
    let rendered = metrics.render_prometheus();

    // Then: each queue-health gauge is exported as one scalar series.
    assert!(rendered.contains("gateway_moderation_decision_outbox_pending{} 4\n"));
    assert!(rendered.contains("gateway_moderation_decision_outbox_oldest_age_seconds{} 321\n"));
    assert!(rendered.contains("gateway_moderation_decision_outbox_target_unavailable{} 2\n"));
    assert!(rendered.contains("gateway_moderation_decision_outbox_retry_exhausted{} 1\n"));
    assert!(rendered.contains(
        "gateway_moderation_decision_outbox_metrics_last_success_timestamp_seconds{} 1790000000\n"
    ));
    assert!(!rendered.contains("report_id"));
}

#[tokio::test]
async fn paired_listeners_serve_and_release_both_addresses_on_shutdown() {
    // Given: independently bound ephemeral application and metrics listeners.
    let listeners = bind_http_listeners(
        "127.0.0.1:0".parse().expect("application address"),
        "127.0.0.1:0".parse().expect("metrics address"),
    )
    .await
    .expect("bind listeners");
    let application_addr = listeners.application_addr().expect("application address");
    let metrics_addr = listeners.metrics_addr().expect("metrics address");
    let application_router = Router::new().route("/docs", get(|| async { StatusCode::OK }));
    let metrics_router = http::metrics_router(GatewayMetrics::default());
    let server = tokio::spawn(serve_http_listeners(
        listeners,
        application_router,
        metrics_router,
    ));

    // When: both live HTTP surfaces are requested.
    let docs = get_status(application_addr, "/docs").await;
    let scrape = get_status(metrics_addr, "/metrics").await;

    // Then: both listeners served, and cancellation releases both sockets.
    assert_eq!(docs, StatusCode::OK);
    assert_eq!(scrape, StatusCode::OK);
    server.abort();
    let _cancelled = server.await.expect_err("server task cancelled");
    let application_rebind = TcpListener::bind(application_addr)
        .await
        .expect("application address released");
    let metrics_rebind = TcpListener::bind(metrics_addr)
        .await
        .expect("metrics address released");
    drop((application_rebind, metrics_rebind));
}

#[tokio::test]
async fn address_collision_returns_loud_error_and_releases_partial_bind() {
    // Given: one ephemeral address selected for both listener roles.
    let reservation = TcpListener::bind("127.0.0.1:0").await.expect("reserve");
    let addr = reservation.local_addr().expect("address");
    drop(reservation);

    // When: both listeners attempt to bind the same address.
    let error = bind_http_listeners(addr, addr)
        .await
        .expect_err("collision must fail startup");

    // Then: the error names the metrics bind and the partial app bind is released.
    assert!(error.to_string().contains("metrics HTTP listener"));
    let rebound = TcpListener::bind(addr)
        .await
        .expect("partial application bind released");
    drop(rebound);
}

async fn get_status(addr: std::net::SocketAddr, path: &str) -> StatusCode {
    let mut stream = tokio::net::TcpStream::connect(addr).await.expect("connect");
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
    let status = String::from_utf8(response)
        .expect("HTTP response")
        .lines()
        .next()
        .expect("status line")
        .split_whitespace()
        .nth(1)
        .expect("status code")
        .parse::<u16>()
        .expect("numeric status");
    StatusCode::from_u16(status).expect("valid status")
}
