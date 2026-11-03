use crate::support::{stored_event, uuid, Harness};
use axum::body::Body;
use http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use post_service::adapters::r#in::http::{metrics_router, router};
use post_service::config::RuntimeMode;
use post_service::domain::post::OutboxMetricsSnapshot;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tower::ServiceExt;

#[tokio::test]
async fn metrics_route_renders_outbox_snapshot_as_prometheus_text() {
    let harness = Harness::new();
    *harness.outbox.snapshot.lock().unwrap() = OutboxMetricsSnapshot {
        unpublished_total: 4,
        oldest_unpublished_age_seconds: 15,
        dead_lettered_total: 2,
    };
    let app = metrics_router(harness.state());

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/metrics")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("metrics response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-type"],
        "text/plain; version=0.0.4; charset=utf-8"
    );
    let body = body_text(response).await;
    assert!(body.contains("post_outbox_unpublished_total 4\n"));
    assert!(body.contains("post_outbox_oldest_unpublished_age_seconds 15\n"));
    assert!(body.contains("post_outbox_dead_lettered_total 2\n"));
}

#[tokio::test]
async fn application_router_does_not_expose_metrics() {
    let harness = Harness::new();

    let response = router(harness.state(), RuntimeMode::Parity)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/metrics")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("metrics response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn metrics_router_rejects_post_and_replay_paths_without_dispatch() {
    let harness = Harness::new();
    let app = metrics_router(harness.state());

    let post_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/posts/44444444-4444-4444-4444-444444444401")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("post response");
    let replay_response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/events/replay")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .expect("request"),
        )
        .await
        .expect("replay response");

    assert_eq!(post_response.status(), StatusCode::NOT_FOUND);
    assert_eq!(replay_response.status(), StatusCode::NOT_FOUND);
    assert!(harness.outbox.find_events_calls.lock().unwrap().is_empty());
    assert!(harness.publisher.published.lock().unwrap().is_empty());
}

#[tokio::test]
async fn dedicated_listeners_isolate_metrics_from_parity_replay_and_release_addresses() {
    let harness = Harness::new();
    let (application_listener, metrics_listener) =
        post_service::adapters::r#in::http::bind_listeners(
            "127.0.0.1:0".parse().expect("application addr"),
            "127.0.0.1:0".parse().expect("metrics addr"),
        )
        .await
        .expect("listeners");
    let application_addr = application_listener.local_addr().expect("application addr");
    let metrics_addr = metrics_listener.local_addr().expect("metrics addr");
    let (application_shutdown_tx, application_shutdown_rx) = oneshot::channel();
    let (metrics_shutdown_tx, metrics_shutdown_rx) = oneshot::channel();
    let application_state = harness.state();
    let metrics_state = harness.state();
    let application_task = tokio::spawn(async move {
        axum::serve(
            application_listener,
            router(application_state, RuntimeMode::Parity),
        )
        .with_graceful_shutdown(async move {
            let _ = application_shutdown_rx.await;
        })
        .await
    });
    let metrics_task = tokio::spawn(async move {
        axum::serve(metrics_listener, metrics_router(metrics_state))
            .with_graceful_shutdown(async move {
                let _ = metrics_shutdown_rx.await;
            })
            .await
    });

    let application_metrics_response = request(application_addr, Method::GET, "/metrics").await;
    let metrics_response = request(metrics_addr, Method::GET, "/metrics").await;
    let replay_response = request(metrics_addr, Method::POST, "/admin/events/replay").await;

    assert!(application_metrics_response.starts_with("HTTP/1.1 404 Not Found"));
    assert!(metrics_response.starts_with("HTTP/1.1 200 OK"));
    assert!(replay_response.starts_with("HTTP/1.1 404 Not Found"));
    assert!(harness.outbox.find_events_calls.lock().unwrap().is_empty());
    assert!(harness.publisher.published.lock().unwrap().is_empty());

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

    let error = post_service::adapters::r#in::http::bind_listeners(
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

async fn request(addr: SocketAddr, method: Method, path: &str) -> String {
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
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

#[tokio::test]
async fn replay_route_is_available_in_parity_mode_and_dispatches_request() {
    let harness = Harness::new();
    let event_id = uuid("44444444-4444-4444-4444-444444444401");
    *harness.outbox.events.lock().unwrap() = vec![stored_event(
        event_id,
        "POST_CREATED",
        serde_json::json!({
            "authorId": "44444444-4444-4444-4444-444444444402"
        })
        .to_string(),
    )];
    let app = router(harness.state(), RuntimeMode::Parity);
    let body = serde_json::json!({
        "eventType": "POST_CREATED",
        "target": "EXTERNAL",
        "limit": 3
    });

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/events/replay")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("request"),
        )
        .await
        .expect("replay response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_text(response).await, "{\"replayedCount\":1}");
    assert_eq!(*harness.publisher.published.lock().unwrap(), vec![event_id]);
    {
        let calls = harness.outbox.find_events_calls.lock().unwrap();
        assert_eq!(calls[0].0.event_type.as_deref(), Some("POST_CREATED"));
        assert_eq!(calls[0].1, 3);
    }
}

#[tokio::test]
async fn replay_route_is_not_mounted_in_production_mode() {
    let harness = Harness::new();
    let app = router(harness.state(), RuntimeMode::Production);

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/events/replay")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .expect("request"),
        )
        .await
        .expect("replay response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

async fn body_text(response: axum::response::Response) -> String {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    String::from_utf8(bytes.to_vec()).expect("utf8 body")
}
