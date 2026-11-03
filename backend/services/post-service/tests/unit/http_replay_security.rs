use crate::support::Harness;
use axum::body::Body;
use http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use post_service::adapters::r#in::http::router;
use post_service::config::RuntimeMode;
use tower::ServiceExt;

#[tokio::test]
async fn production_router_does_not_expose_ordinary_replay() {
    let harness = Harness::new();
    let response = router(harness.state(), RuntimeMode::Production)
        .oneshot(replay_request(1))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(harness.outbox.find_events_calls.lock().unwrap().is_empty());
    assert!(harness.internal.dispatched.lock().unwrap().is_empty());
    assert!(harness.publisher.published.lock().unwrap().is_empty());
}

#[tokio::test]
async fn replay_route_accepts_the_exact_maximum_limit() {
    let harness = Harness::new();
    let response = router(harness.state(), RuntimeMode::Parity)
        .oneshot(replay_request(500))
        .await
        .expect("maximum replay response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_text(response).await, "{\"replayedCount\":0}");
    assert_eq!(harness.outbox.find_events_calls.lock().unwrap()[0].1, 500);
    assert!(harness.internal.dispatched.lock().unwrap().is_empty());
    assert!(harness.publisher.published.lock().unwrap().is_empty());
}

#[tokio::test]
async fn replay_route_rejects_an_oversized_limit_before_any_side_effect() {
    let harness = Harness::new();
    let response = router(harness.state(), RuntimeMode::Parity)
        .oneshot(replay_request(501))
        .await
        .expect("oversized replay response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(harness.outbox.find_events_calls.lock().unwrap().is_empty());
    assert!(harness.internal.dispatched.lock().unwrap().is_empty());
    assert!(harness.publisher.published.lock().unwrap().is_empty());
}

fn replay_request(limit: u64) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/admin/events/replay")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({"target": "BOTH", "limit": limit}).to_string(),
        ))
        .expect("request")
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
