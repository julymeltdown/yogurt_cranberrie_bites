//! Public C05 ingress composition with the real loopback world client.
use super::*;
use crate::bootstrap_http_support::RouteHarness;
use axum::body::{to_bytes, Body};
use axum::http::{header, Request as HttpRequest, StatusCode};
use tower::ServiceExt;

async fn harness(owners: &Running, world: &RunningWorld) -> RouteHarness {
    let mut harness = RouteHarness::new().await;
    let mut state = (*harness.state).clone();
    state.jwt_verifier = JwtVerifier::from_jwks_json(
        "composition-auth",
        "composition-api",
        &json!({"keys":[key().jwk()]}).to_string(),
    )
    .unwrap();
    state.concert_identity = Some(owners.reader.clone());
    state.concert_world_client = Some(world.client.clone());
    harness.state = Arc::new(state);
    harness
}

fn request(
    path: &str,
    bearer: bool,
    body: &str,
    origin: Option<&str>,
    intent: Option<&str>,
) -> HttpRequest<Body> {
    let method = if path.ends_with("/world-tickets") {
        "POST"
    } else {
        "GET"
    };
    let mut builder = HttpRequest::builder().method(method).uri(path);
    if bearer {
        builder = builder.header(
            header::AUTHORIZATION,
            format!("Bearer {}", key().token(USER, Utc::now().timestamp() + 900)),
        );
    }
    if let Some(origin) = origin {
        builder = builder.header(header::ORIGIN, origin);
    }
    if let Some(intent) = intent {
        builder = builder.header("x-mcr-intent", intent);
    }
    let mut request = builder.body(Body::from(body.to_owned())).unwrap();
    request
        .extensions_mut()
        .insert(api_gateway::adapters::r#in::http::RequestDeadline(
            std::time::Instant::now() + std::time::Duration::from_secs(10),
        ));
    request.extensions_mut().insert(axum::extract::ConnectInfo(
        "127.0.0.1:43123".parse::<std::net::SocketAddr>().unwrap(),
    ));
    request
}

async fn json(response: axum::response::Response, status: StatusCode) -> Value {
    let actual = response.status();
    let cache_control = response.headers()[header::CACHE_CONTROL].clone();
    let bytes = to_bytes(response.into_body(), 65_536).await.unwrap();
    assert_eq!(
        actual,
        status,
        "unexpected boundary response: {}",
        String::from_utf8_lossy(&bytes)
    );
    assert_eq!(cache_control, "no-store");
    serde_json::from_slice(&bytes).unwrap()
}

fn ticket_body() -> String {
    json!({
        "room_id": room(),
        "map_id": "map_fixture",
        "map_revision": 1,
        "avatar_manifest_id": "avatar_fixture",
        "avatar_manifest_version": 1
    })
    .to_string()
}

#[tokio::test]
async fn public_ticket_guard_requires_exact_origin_intent_before_owner_reads() {
    let owners = Running::start().await;
    let world = RunningWorld::start().await;
    let h = harness(&owners, &world).await;
    let path = format!("/api/concert/events/{}/world-tickets", Uuid::new_v4());
    let body = ticket_body();
    for (origin, intent, reason) in [
        (None, Some("browser"), "ORIGIN_NOT_ALLOWED"),
        (
            Some("http://127.0.0.1:9999"),
            Some("browser"),
            "ORIGIN_NOT_ALLOWED",
        ),
        (
            Some("http://localhost:3000"),
            None,
            "BROWSER_INTENT_REQUIRED",
        ),
        (
            Some("http://localhost:3000"),
            Some("other"),
            "BROWSER_INTENT_REQUIRED",
        ),
    ] {
        let value = json(
            h.transport_router()
                .oneshot(request(&path, true, &body, origin, intent))
                .await
                .unwrap(),
            StatusCode::FORBIDDEN,
        )
        .await;
        assert_eq!(value["details"]["reason"], reason);
        assert!(!value.to_string().contains(PRIVATE));
    }
    assert_eq!(owners.auth.calls.load(Ordering::SeqCst), 0);
    assert_eq!(owners.profile.calls.load(Ordering::SeqCst), 0);
    assert!(world.service.seen.lock().unwrap().is_empty());
    world.close().await;
    owners.close().await;
}

#[tokio::test]
async fn public_room_catalog_is_bound_to_event_and_signed_chain() {
    let owners = Running::start().await;
    let world = RunningWorld::start().await;
    let h = harness(&owners, &world).await;
    *owners.auth.mode.lock().unwrap() = Mode::Reply(admission());
    *owners.profile.mode.lock().unwrap() = Mode::Reply(profile(USER, "Mira"));
    let event = Uuid::parse_str("00000000-0000-4000-8000-000000000010").unwrap();
    let path = format!("/api/concert/events/{event}/rooms");
    let response = h
        .transport_router()
        .oneshot(request(&path, true, "", None, None))
        .await
        .unwrap();
    let rooms = json(response, StatusCode::OK).await;
    assert_eq!(rooms["event_id"], event.to_string());
    assert_eq!(rooms["default_room_id"], room());
    assert_eq!(owners.auth.calls.load(Ordering::SeqCst), 1);
    assert_eq!(owners.profile.calls.load(Ordering::SeqCst), 1);
    assert_eq!(world.service.seen.lock().unwrap().len(), 1);
    h.pool.close().await;
    world.close().await;
    owners.close().await;
}

#[tokio::test]
async fn oversized_ticket_body_is_413_before_any_owner_read() {
    let owners = Running::start().await;
    let world = RunningWorld::start().await;
    let h = harness(&owners, &world).await;
    let path = format!("/api/concert/events/{}/world-tickets", Uuid::new_v4());
    let mut body = ticket_body().into_bytes();
    body.resize(4097, b' ');
    // The C05 router owns the 4096-byte ticket bound. The same router is
    // mounted under the composed Gateway, whose journal/admission layers need
    // the live stack; exercise the boundary that actually enforces the bound.
    let response =
        api_gateway::adapters::r#in::http::concert_world::router_with_boundary(h.state.clone())
            .oneshot(request(
                &path,
                true,
                std::str::from_utf8(&body).unwrap(),
                Some("http://localhost:3000"),
                Some("browser"),
            ))
            .await
            .unwrap();
    let value = json(response, StatusCode::PAYLOAD_TOO_LARGE).await;
    assert_eq!(value["details"]["reason"], "PAYLOAD_TOO_LARGE");
    assert!(world.service.seen.lock().unwrap().is_empty());
    assert_eq!(owners.auth.calls.load(Ordering::SeqCst), 0);
    world.close().await;
    owners.close().await;
}

#[tokio::test]
async fn ticket_guard_rejection_has_exact_core_error_shape_and_no_replay() {
    let owners = Running::start().await;
    let world = RunningWorld::start().await;
    let h = harness(&owners, &world).await;
    let path = format!("/api/concert/events/{}/world-tickets", Uuid::new_v4());
    let response = h
        .transport_router()
        .oneshot(request(
            &path,
            true,
            &ticket_body(),
            Some("http://localhost:3000"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65_536).await.unwrap()).unwrap();
    assert_eq!(value["details"]["reason"], "BROWSER_INTENT_REQUIRED");
    assert!(world.service.seen.lock().unwrap().is_empty());
    assert_eq!(owners.auth.calls.load(Ordering::SeqCst), 0);
    world.close().await;
    owners.close().await;
}
