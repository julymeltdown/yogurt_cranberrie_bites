use notification_service::adapters::r#in::http as http_routes;
use notification_service::config::NotificationMetrics;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::oneshot;

#[tokio::test]
async fn metrics_router_registers_single_route_with_notification_state() {
    let state = crate::support::lazy_app_state().await;
    state.metrics.record_retention_purge_run("success");
    state
        .metrics
        .record_retention_purged_notifications("success", 2);

    let _router = http_routes::metrics_router(Arc::clone(&state));

    assert_eq!(http_routes::route_count(), 1);
    let body = state.metrics.render_prometheus();
    assert!(body.contains("notification_retention_purge_runs_total{outcome=\"success\"} 1"));
    assert!(
        body.contains("notification_retention_purged_notifications_total{outcome=\"success\"} 2")
    );
}

#[tokio::test]
async fn dedicated_listeners_serve_only_their_routes_and_release_addresses_on_shutdown() {
    let state = crate::support::lazy_app_state().await;
    let (application_listener, metrics_listener) = http_routes::bind_listeners(
        "127.0.0.1:0".parse().expect("application addr"),
        "127.0.0.1:0".parse().expect("metrics addr"),
    )
    .await
    .expect("listeners");
    let application_addr = application_listener.local_addr().expect("application addr");
    let metrics_addr = metrics_listener.local_addr().expect("metrics addr");
    let application_state = state.clone();
    let (application_shutdown_tx, application_shutdown_rx) = oneshot::channel();
    let (metrics_shutdown_tx, metrics_shutdown_rx) = oneshot::channel();
    let application_task = tokio::spawn(async move {
        axum::serve(application_listener, http_routes::router(application_state))
            .with_graceful_shutdown(async move {
                let _ = application_shutdown_rx.await;
            })
            .await
    });
    let metrics_task = tokio::spawn(async move {
        axum::serve(metrics_listener, http_routes::metrics_router(state))
            .with_graceful_shutdown(async move {
                let _ = metrics_shutdown_rx.await;
            })
            .await
    });

    let application_response = get(application_addr, "/metrics").await;
    let metrics_response = get(metrics_addr, "/metrics").await;
    let notification_response = get(metrics_addr, "/notifications").await;

    assert!(application_response.starts_with("HTTP/1.1 404 Not Found"));
    assert!(metrics_response.starts_with("HTTP/1.1 200 OK"));
    assert!(notification_response.starts_with("HTTP/1.1 404 Not Found"));

    application_shutdown_tx
        .send(())
        .expect("application shutdown");
    metrics_shutdown_tx.send(()).expect("metrics shutdown");
    application_task
        .await
        .expect("application task")
        .expect("application serve");
    metrics_task
        .await
        .expect("metrics task")
        .expect("metrics serve");

    let rebound_application = tokio::net::TcpListener::bind(application_addr)
        .await
        .expect("application address released");
    let rebound_metrics = tokio::net::TcpListener::bind(metrics_addr)
        .await
        .expect("metrics address released");
    drop((rebound_application, rebound_metrics));
}

#[tokio::test]
async fn duplicate_metrics_bind_returns_a_loud_error() {
    let occupied = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("occupied listener");
    let occupied_addr = occupied.local_addr().expect("occupied addr");

    let error = http_routes::bind_listeners(
        "127.0.0.1:0".parse().expect("application addr"),
        occupied_addr,
    )
    .await
    .expect_err("duplicate metrics bind must fail");

    assert!(error
        .to_string()
        .contains("failed to bind metrics listener"));
    assert!(error.to_string().contains(&occupied_addr.to_string()));
}

async fn get(addr: SocketAddr, path: &str) -> String {
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .await
        .expect("write request");
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .await
        .expect("read response");
    String::from_utf8(response).expect("utf8 response")
}

#[test]
fn notification_metrics_escape_prometheus_label_values() {
    let metrics = NotificationMetrics::default();

    metrics.record_retention_purge_run("quote\" slash\\ newline\n");

    assert!(metrics.render_prometheus().contains(
        "notification_retention_purge_runs_total{outcome=\"quote\\\" slash\\\\ newline\\n\"} 1"
    ));
}
