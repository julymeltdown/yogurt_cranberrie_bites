use crate::config_behavior::oauth_config;
use crate::support::shared_test_rsa_private_key_pem;
use auth_service::adapters::r#in::http;
use auth_service::adapters::r#in::http::server::{bind_http_listeners, serve_http_listeners};
use auth_service::config::{build_state, AuthMetrics, RuntimeMode, DEFAULT_METRICS_ADDR};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Router;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tower::ServiceExt;

#[test]
fn metrics_address_default_is_loopback_port_9081() {
    // Given: no auth metrics address override.
    // When: the configuration default is inspected.
    // Then: metrics remain on the exact release-readiness loopback address.
    assert_eq!(DEFAULT_METRICS_ADDR, "127.0.0.1:9081");
}

#[tokio::test]
async fn application_router_does_not_serve_metrics() {
    // Given: the public auth application router with dependency connections kept lazy.
    let mut config = oauth_config(RuntimeMode::Parity);
    config.jwt_private_key_pem = Some(shared_test_rsa_private_key_pem().to_owned());
    config.jwt_key_id = Some("metrics-listener-test".into());
    let pool = PgPoolOptions::new()
        .connect_lazy(&config.database_url)
        .expect("lazy pool");
    let state = Arc::new(build_state(&config, pool).await.expect("state"));
    let router = http::router(state);

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
    // Given: the dedicated auth metrics router.
    let router = http::metrics_router(Arc::new(AuthMetrics::default()));

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
    let application_router = Router::new().route("/auth", get(|| async { StatusCode::OK }));
    let metrics_router = http::metrics_router(Arc::new(AuthMetrics::default()));
    let server = tokio::spawn(serve_http_listeners(
        listeners,
        application_router,
        metrics_router,
    ));

    // When: both live HTTP surfaces are requested.
    let application = get_status(application_addr, "/auth").await;
    let scrape = get_status(metrics_addr, "/metrics").await;

    // Then: both listeners served, and cancellation releases both sockets.
    assert_eq!(application, StatusCode::OK);
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

#[tokio::test]
async fn production_listener_does_not_serve_the_duplicate_auth_routes() {
    // Given: the production application listener, where the Gateway owns the
    // public auth surface.
    let mut config = oauth_config(RuntimeMode::Production);
    config.jwt_private_key_pem = Some(shared_test_rsa_private_key_pem().to_owned());
    config.jwt_key_id = Some("surface-test".into());
    config.public_application_routes_enabled = false;
    let pool = PgPoolOptions::new()
        .connect_lazy(&config.database_url)
        .expect("lazy pool");
    let state = Arc::new(build_state(&config, pool).await.expect("state"));
    let router = http::router(state);

    // When/Then: the duplicate auth entry points are absent, so a caller cannot
    // bypass Gateway rate limiting, identity normalization, the request journal,
    // or OAuth state-cookie binding by reaching this service directly.
    for path in [
        "/auth/login",
        "/auth/email/signup",
        "/auth/password/reset/request",
        "/auth/password/reset/confirm",
        "/auth/oauth/google/authorize",
        "/auth/oauth/google/callback",
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "production must not serve {path}"
        );
    }

    // JWKS stays reachable so token validation and key rotation keep working.
    let jwks = router
        .oneshot(
            Request::builder()
                .uri("/.well-known/jwks.json")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(jwks.status(), StatusCode::OK);
}

#[tokio::test]
async fn development_listener_still_serves_the_application_routes() {
    // Given: an explicit opt-in used when the Gateway is not in front.
    let mut config = oauth_config(RuntimeMode::Parity);
    config.jwt_private_key_pem = Some(shared_test_rsa_private_key_pem().to_owned());
    config.jwt_key_id = Some("surface-dev-test".into());
    config.public_application_routes_enabled = true;
    let pool = PgPoolOptions::new()
        .connect_lazy(&config.database_url)
        .expect("lazy pool");
    let state = Arc::new(build_state(&config, pool).await.expect("state"));
    let router = http::router(state);

    // When: a malformed request reaches the real login route.
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from("not-json"))
                .expect("request"),
        )
        .await
        .expect("response");

    // Then: the route exists and rejects the body instead of 404ing.
    assert_ne!(response.status(), StatusCode::NOT_FOUND);
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
