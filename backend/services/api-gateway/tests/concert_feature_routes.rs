//! Public transport admission using the retained harness. No real producer,
//! canonical-owner, database, multiuser or Notification-target acceptance claim.
#[path = "unit/http_support.rs"]
mod http_support;
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use http_support::{RouteHarness, USER_ID};
use tower::ServiceExt;

const EVENT: &str = "bb0f404c-61ce-4840-a907-e34788566425";
const ID: &str = "6862ce9a-00fe-42a6-b806-608586f87f9c";
const KEY: &str = "c2c3dd2f-30ec-48af-9220-794b6fe090b4";
const ORIGIN: &str = "http://localhost:3000";

fn operations() -> Vec<(&'static str, String, &'static str)> {
    vec![
        ("GET", "intros".into(), ""),
        ("GET", format!("intros/{ID}"), ""),
        ("GET", "me/intro".into(), ""),
        (
            "PUT",
            "me/intro".into(),
            r#"{"expected_revision":0,"text":"Hello","age_bounds":{"minimum_age":null,"maximum_age_exclusive":null},"expires_at":null}"#,
        ),
        (
            "POST",
            "me/intro/publish".into(),
            r#"{"expected_revision":1}"#,
        ),
        (
            "POST",
            "me/intro/close".into(),
            r#"{"expected_revision":1}"#,
        ),
        ("GET", "me/notifications".into(), ""),
        ("GET", "me/notifications/unread-count".into(), ""),
        (
            "PATCH",
            format!("me/notifications/{ID}/read"),
            r#"{"expected_revision":1}"#,
        ),
        ("PATCH", "me/notifications/read-all".into(), "{}"),
    ]
}
fn request(h: &RouteHarness, method: &str, tail: &str, body: impl Into<Body>) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(format!("/api/concert/events/{EVENT}/{tail}"))
        .header(header::AUTHORIZATION, h.bearer_token(USER_ID));
    if method != "GET" {
        request = request
            .header(header::ORIGIN, ORIGIN)
            .header("x-mcr-intent", "browser")
            .header("idempotency-key", KEY)
            .header(header::CONTENT_TYPE, "application/json");
    }
    request.body(body.into()).unwrap()
}
async fn check(h: &RouteHarness, request: Request<Body>, status: StatusCode, reason: &str) {
    let target = format!("{} {}", request.method(), request.uri());
    let response = h.transport_router().oneshot(request).await.unwrap();
    assert_eq!(response.status(), status, "{target}");
    for (name, value) in [
        ("cache-control", "no-store"),
        ("pragma", "no-cache"),
        ("x-content-type-options", "nosniff"),
    ] {
        assert_eq!(response.headers()[name], value);
    }
    assert!(!response.headers().contains_key(header::ETAG));
    assert!(!response.headers().contains_key(header::LAST_MODIFIED));
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["details"]["reason"], reason, "{value}");
    assert!(value["traceId"].is_string());
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    for secret in ["postgres", KEY, "completion_secret", "binding_digest"] {
        assert!(!text.contains(secret));
    }
}

#[tokio::test]
async fn all_ten_routes_reach_the_existing_fail_closed_product_boundary() {
    let h = RouteHarness::new().await;
    for (method, tail, body) in operations() {
        check(
            &h,
            request(&h, method, &tail, body),
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
        )
        .await;
    }
    assert_eq!(h.pool.size(), 0);
    h.pool.close().await;
}

#[tokio::test]
async fn all_ten_routes_require_bearer_auth_and_cookie_identity_never_substitutes() {
    let h = RouteHarness::new().await;
    for (method, tail, body) in operations() {
        let mut r = request(&h, method, &tail, body);
        r.headers_mut().remove(header::AUTHORIZATION);
        r.headers_mut().insert(
            header::COOKIE,
            "user_id=foreign; access_token=foreign".parse().unwrap(),
        );
        r.headers_mut().insert("x-user-id", ID.parse().unwrap());
        check(&h, r, StatusCode::UNAUTHORIZED, "AUTHENTICATION_REQUIRED").await;
    }
    h.pool.close().await;
}

#[tokio::test]
async fn every_feature_mutation_including_patch_requires_origin_intent_json_and_one_canonical_key()
{
    let h = RouteHarness::new().await;
    for (method, tail, body) in operations().into_iter().filter(|v| v.0 != "GET") {
        for (name, status, reason) in [
            ("origin", StatusCode::FORBIDDEN, "ORIGIN_NOT_ALLOWED"),
            (
                "x-mcr-intent",
                StatusCode::FORBIDDEN,
                "BROWSER_INTENT_REQUIRED",
            ),
            ("idempotency-key", StatusCode::BAD_REQUEST, "INVALID_INPUT"),
            (
                "content-type",
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "INVALID_MEDIA_TYPE",
            ),
        ] {
            let mut r = request(&h, method, &tail, body);
            r.headers_mut().remove(name);
            check(&h, r, status, reason).await;
        }
        for key in [
            "not-a-uuid",
            "00000000-0000-0000-0000-000000000000",
            "C2C3DD2F-30EC-48AF-9220-794B6FE090B4",
        ] {
            let mut r = request(&h, method, &tail, body);
            r.headers_mut()
                .insert("idempotency-key", key.parse().unwrap());
            check(&h, r, StatusCode::BAD_REQUEST, "INVALID_INPUT").await;
        }
        for (name, value, status, reason) in [
            (
                "origin",
                ORIGIN,
                StatusCode::FORBIDDEN,
                "ORIGIN_NOT_ALLOWED",
            ),
            (
                "x-mcr-intent",
                "browser",
                StatusCode::FORBIDDEN,
                "BROWSER_INTENT_REQUIRED",
            ),
            (
                "idempotency-key",
                KEY,
                StatusCode::BAD_REQUEST,
                "INVALID_INPUT",
            ),
            (
                "content-type",
                "application/json",
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "INVALID_MEDIA_TYPE",
            ),
        ] {
            let mut r = request(&h, method, &tail, body);
            r.headers_mut().append(name, value.parse().unwrap());
            check(&h, r, status, reason).await;
        }
        let mut r = request(&h, method, &tail, body);
        r.headers_mut()
            .insert("origin", "https://foreign.invalid".parse().unwrap());
        check(&h, r, StatusCode::FORBIDDEN, "ORIGIN_NOT_ALLOWED").await;
    }
    h.pool.close().await;
}

#[tokio::test]
async fn feature_body_limits_are_enforced_before_any_producer_dispatch() {
    let h = RouteHarness::new().await;
    for (method, tail, _) in operations().into_iter().filter(|v| v.0 != "GET") {
        let limit = if method == "PATCH" { 8192 } else { 16 * 1024 };
        let mut body = vec![b' '; limit];
        body[..2].copy_from_slice(b"{}");
        check(
            &h,
            request(&h, method, &tail, body.clone()),
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
        )
        .await;
        body.push(b' ');
        check(
            &h,
            request(&h, method, &tail, body),
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
        )
        .await;
    }
    for (method, tail, _) in operations().into_iter().filter(|v| v.0 == "GET") {
        check(
            &h,
            request(&h, method, &tail, "x"),
            StatusCode::BAD_REQUEST,
            "INVALID_INPUT",
        )
        .await;
        for name in ["content-type", "idempotency-key"] {
            let mut r = request(&h, method, &tail, "");
            r.headers_mut()
                .insert(name, "application/json".parse().unwrap());
            check(&h, r, StatusCode::BAD_REQUEST, "INVALID_INPUT").await;
        }
    }
    h.pool.close().await;
}

#[tokio::test]
async fn frozen_list_queries_allow_encoding_but_reject_unknown_duplicate_or_invalid_fields() {
    let h = RouteHarness::new().await;
    for tail in [
        "intros?limit=100&cursor=Ab_9-0&q=Hello+%E2%99%AB",
        "me/notifications?limit=1&cursor=Ab_9-0&unread_only=false",
    ] {
        check(
            &h,
            request(&h, "GET", tail, ""),
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
        )
        .await;
    }
    for base in ["intros", "me/notifications"] {
        for query in [
            "",
            "actor=foreign",
            "limit=0",
            "limit=101",
            "limit=-1",
            "limit=1&%6cimit=2",
            "cursor=",
            "cursor=a%2Fb",
            "limit=%",
            "cursor=%FF",
            "limit=1&",
            "q=%00",
        ] {
            check(
                &h,
                request(&h, "GET", &format!("{base}?{query}"), ""),
                StatusCode::BAD_REQUEST,
                "INVALID_INPUT",
            )
            .await;
        }
    }
    for tail in [
        "intros?unread_only=true",
        "intros?q=",
        "me/notifications?q=hello",
        "me/notifications?unread_only=1",
        "me/notifications/unread-count?limit=1",
        "me/intro?actor=foreign",
    ] {
        check(
            &h,
            request(&h, "GET", tail, ""),
            StatusCode::BAD_REQUEST,
            "INVALID_INPUT",
        )
        .await;
    }
    for tail in [
        format!("intros?q={}", "a".repeat(101)),
        format!("intros?cursor={}", "a".repeat(1025)),
        format!("intros?q={}", "a".repeat(4096)),
    ] {
        check(
            &h,
            request(&h, "GET", &tail, ""),
            StatusCode::BAD_REQUEST,
            "INVALID_INPUT",
        )
        .await;
    }
    for (method, tail, body) in operations().into_iter().filter(|v| v.0 != "GET") {
        check(
            &h,
            request(&h, method, &format!("{tail}?actor={ID}"), body),
            StatusCode::BAD_REQUEST,
            "INVALID_INPUT",
        )
        .await;
    }
    h.pool.close().await;
}

#[tokio::test]
async fn invalid_ids_unknown_suffixes_and_wrong_methods_do_not_expand_product_routes() {
    let h = RouteHarness::new().await;
    for tail in [
        "intros/not-a-uuid",
        "intros/00000000-0000-0000-0000-000000000000",
    ] {
        check(
            &h,
            request(&h, "GET", tail, ""),
            StatusCode::BAD_REQUEST,
            "INVALID_INPUT",
        )
        .await;
    }
    for id in ["not-a-uuid", "00000000-0000-0000-0000-000000000000"] {
        check(
            &h,
            request(
                &h,
                "PATCH",
                &format!("me/notifications/{id}/read"),
                "{\"expected_revision\":1}",
            ),
            StatusCode::BAD_REQUEST,
            "INVALID_INPUT",
        )
        .await;
    }
    for tail in [
        format!("intros/{ID}/extra"),
        "me/notifications/private".into(),
        format!("intros/{ID}/sharing-offer"),
    ] {
        check(
            &h,
            request(&h, "GET", &tail, ""),
            StatusCode::NOT_FOUND,
            "RESOURCE_UNAVAILABLE",
        )
        .await;
    }
    // PATCH is frozen, so GET read-all must not be admitted.
    check(
        &h,
        request(&h, "GET", "me/notifications/read-all", ""),
        StatusCode::BAD_REQUEST,
        "INVALID_INPUT",
    )
    .await;
    let mut r = request(&h, "GET", "me/intro", "");
    *r.uri_mut() = "/api/concert/events/not-a-uuid/me/intro".parse().unwrap();
    check(&h, r, StatusCode::BAD_REQUEST, "INVALID_INPUT").await;
    h.pool.close().await;
}
