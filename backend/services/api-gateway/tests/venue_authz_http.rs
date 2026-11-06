#[path = "unit/http_support.rs"]
mod http_support;
use axum::{
    body::{to_bytes, Body},
    http::{header, HeaderMap, Request, StatusCode},
};
use http_support::RouteHarness;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use tower::ServiceExt;

const EVENT: &str = "bb0f404c-61ce-4840-a907-e34788566425";
const PEER: &str = "6862ce9a-00fe-42a6-b806-608586f87f9c";
const KEY: &str = "c2c3dd2f-30ec-48af-9220-794b6fe090b4";
const ORIGIN: &str = "http://localhost:3000";
const PRIVATE_BODY: &str = "friends only plan";

fn get(h: &RouteHarness, user: &str, tail: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(format!("/api/concert/events/{EVENT}/venue-meeting/{tail}"))
        .header(header::AUTHORIZATION, h.bearer_token(user))
        .body(Body::empty())
        .unwrap()
}

fn invitation(
    h: &RouteHarness,
    user: &str,
    origin: bool,
    intent: bool,
    key: bool,
) -> Request<Body> {
    let body = json!({
        "selection": {
            "selectedFriendId": PEER,
            "landmark": {"landmarkId": "gate-a", "catalogRevision": 3, "landmarkRevision": 4}
        }
    })
    .to_string();
    let mut request = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/concert/events/{EVENT}/venue-meeting/invitations"
        ))
        .header(header::AUTHORIZATION, h.bearer_token(user))
        .header(header::CONTENT_TYPE, "application/json");
    if origin {
        request = request.header(header::ORIGIN, ORIGIN);
    }
    if intent {
        request = request.header("x-mcr-intent", "browser");
    }
    if key {
        request = request.header("idempotency-key", KEY);
    }
    request.body(Body::from(body)).unwrap()
}

async fn call(h: &RouteHarness, request: Request<Body>) -> (StatusCode, HeaderMap, Value, String) {
    let response = h.transport_router().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    for (name, value) in [
        ("cache-control", "no-store"),
        ("pragma", "no-cache"),
        ("x-content-type-options", "nosniff"),
    ] {
        assert_eq!(headers[name], value);
    }
    assert!(!headers.contains_key(header::ETAG));
    assert!(!headers.contains_key(header::LAST_MODIFIED));
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let value = serde_json::from_str(&text).unwrap();
    (status, headers, value, text)
}

fn assert_error(value: &Value, text: &str, status: u16, reason: &str) {
    let keys: BTreeSet<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        BTreeSet::from(["code", "message", "traceId", "timestamp", "details"])
    );
    assert_eq!(value["code"], format!("CONCERT_{status}"));
    assert_eq!(value["message"], "Concert request unavailable");
    assert_eq!(value["details"], json!({"reason": reason}));
    let trace = value["traceId"].as_str().unwrap();
    assert_eq!(trace.len(), 36);
    assert!(value["timestamp"].as_str().unwrap().ends_with('Z'));
    for secret in [PEER, KEY, PRIVATE_BODY, "gate-a", "selectedFriendId"] {
        assert!(!text.contains(secret), "error leaked {secret}: {text}");
    }
}

#[tokio::test]
async fn venue_meeting_http_is_fail_closed_for_every_role() {
    let h = RouteHarness::new().await;
    let friend = "22222222-2222-2222-2222-222222222222";
    let stranger = "33333333-3333-3333-3333-333333333333";
    let blocked = "44444444-4444-4444-4444-444444444444";
    let suspended = "55555555-5555-5555-5555-555555555555";
    let expired = "66666666-6666-6666-6666-666666666666";
    let admin = "77777777-7777-7777-7777-777777777777";
    for user in [friend, stranger, blocked, suspended, expired, admin] {
        let (status, _headers, value, text) = call(
            &h,
            get(&h, user, &format!("context?selectedFriendId={PEER}")),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{user}");
        assert_error(&value, &text, 503, "DEPENDENCY_UNAVAILABLE");
        assert!(!text.contains(user));
    }
    h.pool.close().await;
}

#[tokio::test]
async fn venue_meeting_http_rejects_missing_origin_intent_and_key() {
    let h = RouteHarness::new().await;
    for (request, status, reason) in [
        (
            invitation(&h, http_support::USER_ID, false, false, false),
            403,
            "ORIGIN_NOT_ALLOWED",
        ),
        (
            invitation(&h, http_support::USER_ID, true, false, false),
            403,
            "BROWSER_INTENT_REQUIRED",
        ),
        (
            invitation(&h, http_support::USER_ID, true, true, false),
            400,
            "INVALID_INPUT",
        ),
        (
            invitation(&h, http_support::USER_ID, true, true, true),
            503,
            "DEPENDENCY_UNAVAILABLE",
        ),
    ] {
        let (actual, _headers, value, text) = call(&h, request).await;
        assert_eq!(actual.as_u16(), status, "{reason}");
        assert_error(&value, &text, status, reason);
    }
    h.pool.close().await;
}

#[tokio::test]
async fn venue_meeting_http_rejects_malformed_targets_before_owner_reads() {
    let h = RouteHarness::new().await;
    for tail in [
        "context".to_owned(),
        "context?selectedFriendId=".to_owned(),
        "context?selectedFriendId=not-a-uuid".to_owned(),
        format!("context?selectedFriendId={PEER}&selectedFriendId={PEER}"),
        "invitations".to_owned(),
        format!("messages?selectedFriendId={PEER}&cursor=01"),
    ] {
        let (status, _headers, value, text) = call(&h, get(&h, http_support::USER_ID, &tail)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{tail}");
        assert_error(&value, &text, 400, "INVALID_INPUT");
    }
    h.pool.close().await;
}
