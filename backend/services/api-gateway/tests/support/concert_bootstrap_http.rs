use super::*;
use crate::bootstrap_http_support::RouteHarness;
use api_gateway::adapters::r#in::http;
use axum::body::{to_bytes, Body};
use axum::http::{header, Request as HttpRequest, StatusCode};
use tower::ServiceExt;

async fn harness(owners: &Running, concert: &RunningConcert) -> RouteHarness {
    let mut harness = RouteHarness::new().await;
    let mut state = (*harness.state).clone();
    state.jwt_verifier = JwtVerifier::from_jwks_json(
        "composition-auth",
        "composition-api",
        &json!({"keys":[key().jwk()]}).to_string(),
    )
    .unwrap();
    state.concert_identity = Some(owners.reader.clone());
    state.concert_client = Some(concert.client.clone());
    harness.state = Arc::new(state);
    harness
}

fn request(path: &str, bearer: bool, body: &'static str) -> HttpRequest<Body> {
    let mut r = HttpRequest::builder().uri(path);
    if bearer {
        r = r.header(
            header::AUTHORIZATION,
            format!("Bearer {}", key().token(USER, Utc::now().timestamp() + 900)),
        );
    }
    let mut request = r.body(Body::from(body)).unwrap();
    // The production listener supplies this extension; forwarded headers alone
    // intentionally cannot establish a peer identity.
    request.extensions_mut().insert(axum::extract::ConnectInfo(
        "127.0.0.1:43123".parse::<std::net::SocketAddr>().unwrap(),
    ));
    request
}

async fn read(
    response: axum::response::Response,
    expected: StatusCode,
    reason: Option<&str>,
) -> Value {
    assert_eq!(response.status(), expected);
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );
    let value: Value = serde_json::from_slice(
        &to_bytes(response.into_body(), 4 * 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap();
    if let Some(reason) = reason {
        assert_eq!(value["details"]["reason"], reason);
        assert!(value.get("code").and_then(Value::as_str).is_some());
        assert!(!value.to_string().contains(PRIVATE));
    }
    value
}

#[tokio::test]
async fn public_http_route_dispatches_fresh_signed_chain_and_never_caches() {
    let owners = Running::start().await;
    let concert = RunningConcert::start().await;
    let h = harness(&owners, &concert).await;
    let event = Uuid::new_v4();
    let path = format!("/api/concert/events/{event}/bootstrap");
    let json = read(
        h.transport_router()
            .oneshot(request(&path, true, ""))
            .await
            .unwrap(),
        StatusCode::OK,
        None,
    )
    .await;
    assert_eq!(json["event"]["event_id"], event.to_string());
    assert_eq!(json["participant"]["user_id"], USER);
    assert_eq!(concert.service.seen.lock().unwrap().len(), 1);
    assert_eq!(owners.auth.calls.load(Ordering::SeqCst), 1);
    assert_eq!(owners.profile.calls.load(Ordering::SeqCst), 1);
    *concert.service.reply.lock().unwrap() = Reply::Foreign;
    read(
        h.transport_router()
            .oneshot(request(&path, true, ""))
            .await
            .unwrap(),
        StatusCode::SERVICE_UNAVAILABLE,
        Some("DEPENDENCY_UNAVAILABLE"),
    )
    .await;
    h.pool.close().await;
    concert.close().await;
    owners.close().await;
}

#[tokio::test]
async fn public_http_rejections_are_core_errors_before_owner_dispatch() {
    let owners = Running::start().await;
    let concert = RunningConcert::start().await;
    let h = harness(&owners, &concert).await;
    let path = format!("/api/concert/events/{}/bootstrap", Uuid::new_v4());
    for (uri, token, body, status, reason) in [
        (
            path.clone(),
            false,
            "",
            StatusCode::UNAUTHORIZED,
            "AUTHENTICATION_REQUIRED",
        ),
        (
            format!("{path}?actor={OTHER}"),
            true,
            "",
            StatusCode::BAD_REQUEST,
            "INVALID_INPUT",
        ),
        (
            "/api/concert/events/not-a-uuid/bootstrap".into(),
            true,
            "",
            StatusCode::BAD_REQUEST,
            "INVALID_INPUT",
        ),
        (
            path.clone(),
            true,
            "{}",
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
        ),
    ] {
        read(
            h.transport_router()
                .oneshot(request(&uri, token, body))
                .await
                .unwrap(),
            status,
            Some(reason),
        )
        .await;
    }
    assert_eq!(owners.auth.calls.load(Ordering::SeqCst), 0);
    assert!(concert.service.seen.lock().unwrap().is_empty());
    let mut state = (*h.state).clone();
    state.concert_client = None;
    read(
        http::router(Arc::new(state))
            .oneshot(request(&path, true, ""))
            .await
            .unwrap(),
        StatusCode::SERVICE_UNAVAILABLE,
        Some("DEPENDENCY_UNAVAILABLE"),
    )
    .await;
    h.pool.close().await;
    concert.close().await;
    owners.close().await;
}

#[tokio::test]
async fn outer_http_deadline_is_c03_and_cannot_return_late_bootstrap() {
    let owners = Running::start().await;
    let concert = RunningConcert::start().await;
    let mut h = harness(&owners, &concert).await;
    let mut state = (*h.state).clone();
    state.request_timeout = Duration::from_millis(120);
    h.state = Arc::new(state);
    *concert.service.reply.lock().unwrap() = Reply::Hold;
    let path = format!("/api/concert/events/{}/bootstrap", Uuid::new_v4());
    read(
        h.transport_router()
            .oneshot(request(&path, true, ""))
            .await
            .unwrap(),
        StatusCode::GATEWAY_TIMEOUT,
        Some("DEADLINE_EXCEEDED"),
    )
    .await;
    assert!(concert.service.seen.lock().unwrap().len() <= 1);
    h.pool.close().await;
    concert.close().await;
    owners.close().await;
}
