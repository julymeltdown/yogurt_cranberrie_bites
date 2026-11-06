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

fn venue_get(h: &RouteHarness, user: Option<&str>, path: &str) -> Request<Body> {
    let mut request = Request::builder()
        .method("GET")
        .uri(path);
    if let Some(user) = user {
        request = request.header(header::AUTHORIZATION, h.bearer_token(user));
    }
    request.body(Body::empty()).unwrap()
}

fn venue_note_mutation(
    h: &RouteHarness,
    method: &str,
    path: &str,
    origin: bool,
    key: bool,
) -> Request<Body> {
    let body = if method == "POST" {
        json!({
            "text": "venue boundary note",
            "audience": "EVERYONE",
            "expiry": "ONE_HOUR",
            "landmark_selection": {
                "landmark_id": PEER,
                "catalog_revision": 1,
                "landmark_revision": 1
            }
        })
        .to_string()
    } else {
        String::new()
    };
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, h.bearer_token(http_support::USER_ID));
    if method == "POST" {
        request = request.header(header::CONTENT_TYPE, "application/json");
    }
    if origin {
        request = request.header(header::ORIGIN, ORIGIN);
    }
    if method == "POST" || origin {
        request = request.header("x-mcr-intent", "browser");
    }
    if key {
        request = request.header("idempotency-key", KEY);
    }
    request.body(Body::from(body)).unwrap()
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
async fn venue_notes_and_location_http_are_mounted_and_fail_closed() {
    let h = RouteHarness::new().await;
    let base = format!("/api/concert/events/{EVENT}/venue");
    let note = format!("{base}/landmarks/{PEER}/notes");
    let note_id = format!("{note}/00000000-0000-4000-8000-000000000301");
    for path in [
        format!("{base}/location"),
        note.clone(),
        note_id.clone(),
        format!("{note_id}/replies"),
    ] {
        let (status, _headers, value, text) =
            call(&h, venue_get(&h, None, &path)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}");
        assert_error(&value, &text, 401, "AUTHENTICATION_REQUIRED");
        let (status, _headers, value, text) =
            call(&h, venue_get(&h, Some(http_support::USER_ID), &path)).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path}");
        assert_error(&value, &text, 503, "DEPENDENCY_UNAVAILABLE");
    }
    for path in [
        format!("{note}?limit=0"),
        format!("{note}?limit=1&limit=2"),
        format!("{note}?before=0.00000000-0000-0000-0000-000000000000"),
    ] {
        let (status, _headers, value, text) =
            call(&h, venue_get(&h, Some(http_support::USER_ID), &path)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert_error(&value, &text, 400, "INVALID_INPUT");
    }
    h.pool.close().await;
}

#[tokio::test]
async fn venue_note_mutations_require_exact_transport_headers() {
    let h = RouteHarness::new().await;
    let path = format!("/api/concert/events/{EVENT}/venue/landmarks/{PEER}/notes");
    for (request, status, reason) in [
        (
            venue_note_mutation(&h, "POST", &path, false, true),
            403,
            "ORIGIN_NOT_ALLOWED",
        ),
        (
            venue_note_mutation(&h, "POST", &path, true, false),
            400,
            "INVALID_INPUT",
        ),
        (
            venue_note_mutation(&h, "POST", &path, true, true),
            503,
            "DEPENDENCY_UNAVAILABLE",
        ),
    ] {
        let (actual, _headers, value, text) = call(&h, request).await;
        assert_eq!(actual.as_u16(), status, "{reason}");
        assert_error(&value, &text, status, reason);
    }
    let delete = format!(
        "/api/concert/events/{EVENT}/venue/landmarks/{PEER}/notes/00000000-0000-4000-8000-000000000301?expected_revision=1"
    );
    let (status, _headers, value, text) = call(
        &h,
        venue_note_mutation(&h, "DELETE", &delete, true, true),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "delete");
    assert_error(&value, &text, 503, "DEPENDENCY_UNAVAILABLE");
    h.pool.close().await;
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
