use crate::http_support::{response_json, RouteHarness, OTHER_USER_ID, USER_ID};
use api_gateway::adapters::r#in::http::notification_routes;
use api_gateway::application::dto::notification_dto::NotificationResponse;
use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use serde_json::json;
use std::sync::atomic::Ordering;
use tower::ServiceExt;

const PUBLIC_POST: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa1";
const HIDDEN_POST: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa2";

#[tokio::test]
async fn injected_monotonic_time_bounds_projection_and_keeps_committed_read_redacted() {
    use crate::runtime::ScriptedRuntime;
    use api_gateway::application::service::NotificationGatewayService;
    use std::time::{Duration, Instant};
    let harness = RouteHarness::new().await;
    *harness.clients.notification_items.lock().unwrap() =
        Some(vec![notification("public", PUBLIC_POST)]);
    let start = Instant::now();
    let runtime = ScriptedRuntime::with_monotonic(vec![
        start,
        start + Duration::from_millis(25),
        start + Duration::from_millis(300),
        start + Duration::from_millis(450),
    ]);
    let service = NotificationGatewayService::new(
        harness.clients.clone(),
        harness.clients.clone(),
        runtime.clone(),
    )
    .with_projection_budget(Duration::from_millis(100));
    let page = service
        .list_notifications(USER_ID.into(), None, 25)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(
        *harness
            .clients
            .notification_authorization_budgets
            .lock()
            .unwrap(),
        vec![Duration::from_millis(75)]
    );
    let marked = service
        .mark_read(USER_ID.into(), "public".into())
        .await
        .unwrap();
    assert_eq!(marked.payload_json, "{}");
    assert_eq!(marked.read_at, "2026-10-11T00:00:00Z");
    assert_eq!(
        harness
            .clients
            .notification_mark_read_calls
            .lock()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        harness
            .clients
            .notification_authorization_batches
            .load(Ordering::SeqCst),
        1
    );
    assert_eq!(runtime.monotonic_reads.load(Ordering::SeqCst), 4);
    assert_eq!(runtime.utc_reads.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.id_reads.load(Ordering::SeqCst), 0);
}

fn notification(id: &str, post_id: &str) -> NotificationResponse {
    NotificationResponse {
        id: id.into(), user_id: USER_ID.into(), notification_type: "POST_CREATED".into(),
        payload_json: json!({"postId": post_id, "authorId": OTHER_USER_ID, "content": "historical copied content"}).to_string(),
        created_at: "2026-10-10T00:00:00Z".into(), read_at: String::new(),
    }
}

fn request(harness: &RouteHarness, method: Method, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .body(Body::empty())
        .expect("request")
}

#[tokio::test]
async fn inbox_reauthorizes_historical_content_and_preserves_owner_cursor() {
    let harness = RouteHarness::new().await;
    let public = notification("public", PUBLIC_POST);
    let mut duplicate = public.clone();
    duplicate.id = "duplicate".into();
    let hidden = notification("hidden", HIDDEN_POST);
    let mut foreign = public.clone();
    foreign.id = "foreign".into();
    foreign.user_id = OTHER_USER_ID.into();
    let mut malformed = notification("malformed", PUBLIC_POST);
    malformed.payload_json = "{bad historical payload".into();
    let mut friend = notification("friend", PUBLIC_POST);
    friend.notification_type = "FRIEND_ACCEPTED".into();
    friend.payload_json = json!({"userId": OTHER_USER_ID}).to_string();
    *harness.clients.notification_items.lock().unwrap() = Some(vec![
        hidden,
        public.clone(),
        duplicate,
        foreign,
        malformed,
        friend,
    ]);
    harness
        .clients
        .notification_post_access
        .lock()
        .unwrap()
        .insert(HIDDEN_POST.into(), false);
    let app = notification_routes::router(harness.state.clone());
    let response = app
        .clone()
        .oneshot(request(
            &harness,
            Method::GET,
            "/api/notifications?cursor=owner-next",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    let ids: Vec<_> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["public", "duplicate", "friend"]);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            body["items"][0]["payloadJson"].as_str().unwrap()
        )
        .unwrap(),
        json!({"payloadVersion": 1, "postId": PUBLIC_POST})
    );
    assert!(!body.to_string().contains("historical copied content"));
    assert_eq!(body["items"][0]["readAt"], "");
    assert_eq!(body["nextCursor"], "owner-next");
    assert_eq!(
        *harness.clients.notification_post_checks.lock().unwrap(),
        vec![
            (USER_ID.into(), PUBLIC_POST.into()),
            (USER_ID.into(), HIDDEN_POST.into())
        ]
    );
    // Access decisions are scoped to this response, never a cross-request cache.
    harness
        .clients
        .notification_post_access
        .lock()
        .unwrap()
        .insert(PUBLIC_POST.into(), false);
    let response = app
        .oneshot(request(
            &harness,
            Method::GET,
            "/api/notifications?cursor=owner-next",
        ))
        .await
        .unwrap();
    let body = response_json(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["items"][0]["id"], "friend");
}

#[tokio::test]
async fn inaccessible_inbox_page_keeps_continuation_and_post_owner_outage_fails_closed() {
    let harness = RouteHarness::new().await;
    *harness.clients.notification_items.lock().unwrap() =
        Some(vec![notification("hidden", HIDDEN_POST)]);
    harness
        .clients
        .notification_post_access
        .lock()
        .unwrap()
        .insert(HIDDEN_POST.into(), false);
    let app = notification_routes::router(harness.state.clone());
    let response = app
        .clone()
        .oneshot(request(
            &harness,
            Method::GET,
            "/api/notifications?cursor=owner-next",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["items"], json!([]));
    assert_eq!(body["nextCursor"], "owner-next");
    harness
        .clients
        .notification_post_unavailable
        .store(true, Ordering::SeqCst);
    let response = app
        .oneshot(request(&harness, Method::GET, "/api/notifications"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = response_json(response).await;
    assert!(!body.to_string().contains("historical copied content"));
    assert!(!body.to_string().contains(HIDDEN_POST));
}

#[tokio::test]
async fn mark_read_reports_success_without_returning_unproven_historical_payload() {
    let harness = RouteHarness::new().await;
    *harness.clients.notification_items.lock().unwrap() =
        Some(vec![notification("hidden", HIDDEN_POST)]);
    harness
        .clients
        .notification_post_access
        .lock()
        .unwrap()
        .insert(HIDDEN_POST.into(), false);
    let app = notification_routes::router(harness.state.clone());
    for unavailable in [false, true] {
        harness
            .clients
            .notification_post_unavailable
            .store(unavailable, Ordering::SeqCst);
        let response = app
            .clone()
            .oneshot(request(
                &harness,
                Method::PATCH,
                "/api/notifications/hidden/read",
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["id"], "hidden");
        assert_eq!(body["payloadJson"], "{}");
        assert_eq!(body["readAt"], "2026-10-11T00:00:00Z");
    }
    assert_eq!(
        harness
            .clients
            .notification_mark_read_calls
            .lock()
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn notification_authorization_is_bounded_and_missing_bearer_never_checks_post() {
    let harness = RouteHarness::new().await;
    let app = notification_routes::router(harness.state.clone());
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/notifications")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(harness
        .clients
        .notification_post_checks
        .lock()
        .unwrap()
        .is_empty());
    *harness.clients.notification_items.lock().unwrap() = Some(
        (0..101)
            .map(|index| notification(&index.to_string(), PUBLIC_POST))
            .collect(),
    );
    let response = app
        .oneshot(request(
            &harness,
            Method::GET,
            "/api/notifications?limit=100",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(harness
        .clients
        .notification_post_checks
        .lock()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn delayed_batch_authorization_returns_before_production_outer_timeout_and_redacts_mark_read()
{
    use api_gateway::adapters::r#in::http;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    let harness = RouteHarness::new().await;
    *harness.clients.notification_items.lock().unwrap() = Some(
        (0..100)
            .map(|index| notification(&index.to_string(), &uuid::Uuid::new_v4().to_string()))
            .collect(),
    );
    *harness
        .clients
        .notification_authorization_delay
        .lock()
        .unwrap() = Duration::from_secs(2);
    let mut state = (*harness.state).clone();
    state.request_timeout = Duration::from_millis(250);
    state.notification_service = state
        .notification_service
        .with_projection_budget(Duration::from_millis(75));
    // This is the executable's request middleware composition, with application
    // ports doubled and durable transport intentionally disabled (no DB here).
    let app = http::router(Arc::new(state));
    let started = Instant::now();
    let response = app
        .clone()
        .oneshot(request(
            &harness,
            Method::GET,
            "/api/notifications?limit=100",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    assert!(started.elapsed() < Duration::from_millis(225));
    assert_eq!(
        harness
            .clients
            .notification_authorization_batches
            .load(Ordering::SeqCst),
        1,
        "one batch for 100 distinct references"
    );
    assert!(harness
        .clients
        .notification_post_checks
        .lock()
        .unwrap()
        .is_empty());
    let started = Instant::now();
    let response = app
        .oneshot(request(
            &harness,
            Method::PATCH,
            "/api/notifications/0/read",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(started.elapsed() < Duration::from_millis(225));
    let body = response_json(response).await;
    assert_eq!(body["payloadJson"], "{}");
    assert_eq!(body["readAt"], "2026-10-11T00:00:00Z");
    assert_eq!(
        harness
            .clients
            .notification_mark_read_calls
            .lock()
            .unwrap()
            .len(),
        1
    );
}
