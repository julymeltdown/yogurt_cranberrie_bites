//! Age HTTP admission only. The retained unconfigured-client harness must fail
//! closed; it does not qualify the provider, SQL owner, or a verified age claim.
#[path = "unit/http_support.rs"]
mod http_support;
use axum::{
    body::{to_bytes, Body},
    http::{header, HeaderValue, Request, StatusCode},
};
use http_support::{RouteHarness, USER_ID};
use tower::ServiceExt;

const EVENT: &str = "bb0f404c-61ce-4840-a907-e34788566425";
const ID: &str = "6862ce9a-00fe-42a6-b806-608586f87f9c";
const KEY: &str = "c2c3dd2f-30ec-48af-9220-794b6fe090b4";
const START: &str = "me/age-verifications";
const CALLBACK: &str = "me/age-verifications/callback";
const START_BODY: &str = r#"{"return_target":"AGE_PRIVACY"}"#;
fn callback(state: &str, provider: &str) -> String {
    serde_json::json!({"verification_id":ID,"state_token":state,"provider_response_token":provider})
        .to_string()
}
fn request(h: &RouteHarness, method: &str, tail: &str, body: impl Into<Body>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(format!("/api/concert/events/{EVENT}/{tail}"))
        .header(header::AUTHORIZATION, h.bearer_token(USER_ID));
    if method != "GET" {
        builder = builder
            .header(header::ORIGIN, "http://localhost:3000")
            .header("x-mcr-intent", "browser")
            .header("idempotency-key", KEY)
            .header(header::CONTENT_TYPE, "application/json");
    }
    builder.body(body.into()).unwrap()
}
async fn check(h: &RouteHarness, req: Request<Body>, status: u16, reason: &str) {
    let target = format!("{} {}", req.method(), req.uri());
    let response = h.transport_router().oneshot(req).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::from_u16(status).unwrap(),
        "{target}"
    );
    for (name, value) in [
        ("cache-control", "no-store"),
        ("pragma", "no-cache"),
        ("x-content-type-options", "nosniff"),
    ] {
        assert_eq!(response.headers()[name], value, "{target}");
    }
    assert!(!response.headers().contains_key(header::ETAG));
    assert!(!response.headers().contains_key(header::LAST_MODIFIED));
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["details"]["reason"], reason, "{target}: {value}");
    assert!(value["traceId"].is_string());
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    for secret in [
        "postgres",
        KEY,
        "callback-secret",
        "provider-secret",
        "completion_secret",
        "binding_digest",
    ] {
        assert!(!text.contains(secret), "{target}: secret leaked");
    }
}
fn operations() -> Vec<(&'static str, &'static str, String)> {
    vec![
        ("GET", "me/age-status", String::new()),
        ("POST", START, START_BODY.into()),
        (
            "POST",
            CALLBACK,
            callback("callback-secret", "provider-secret"),
        ),
    ]
}

#[tokio::test]
async fn three_frozen_routes_reach_only_the_existing_fail_closed_product_boundary() {
    let h = RouteHarness::new().await;
    for (method, tail, body) in operations() {
        check(
            &h,
            request(&h, method, tail, body),
            503,
            "DEPENDENCY_UNAVAILABLE",
        )
        .await;
    }
    assert_eq!(h.pool.size(), 0);
    h.pool.close().await;
}
#[tokio::test]
async fn authenticated_canonical_identity_cannot_be_replaced_by_body_cookie_or_actor_headers() {
    let h = RouteHarness::new().await;
    for (method, tail, body) in operations() {
        for bearer in [None, Some("Bearer invalid")] {
            let mut req = request(&h, method, tail, body.clone());
            req.headers_mut().remove(header::AUTHORIZATION);
            if let Some(value) = bearer {
                req.headers_mut()
                    .insert(header::AUTHORIZATION, HeaderValue::from_static(value));
            }
            req.headers_mut()
                .insert("x-user-id", HeaderValue::from_static(USER_ID));
            req.headers_mut().insert(
                header::COOKIE,
                HeaderValue::from_static("access_token=forged"),
            );
            check(&h, req, 401, "AUTHENTICATION_REQUIRED").await;
        }
    }
    for tail in [START, CALLBACK] {
        check(
            &h,
            request(&h, "POST", tail, r#"{"user_id":"forged","verified":true}"#),
            400,
            "INVALID_INPUT",
        )
        .await;
    }
    h.pool.close().await;
}
#[tokio::test]
async fn mutations_require_exact_single_origin_browser_intent_key_and_json_type() {
    let h = RouteHarness::new().await;
    for (tail, body) in [
        (START, START_BODY.into()),
        (CALLBACK, callback("state", "proof")),
    ] {
        for (name, bad, status, reason) in [
            (
                "origin",
                "http://localhost:3000/",
                403,
                "ORIGIN_NOT_ALLOWED",
            ),
            ("x-mcr-intent", "Browser", 403, "BROWSER_INTENT_REQUIRED"),
            (
                "idempotency-key",
                "00000000-0000-0000-0000-000000000000",
                400,
                "INVALID_INPUT",
            ),
            (
                "idempotency-key",
                "C2C3DD2F-30EC-48AF-9220-794B6FE090B4",
                400,
                "INVALID_INPUT",
            ),
            (
                "content-type",
                "application/json; charset=utf-8",
                415,
                "INVALID_MEDIA_TYPE",
            ),
        ] {
            for variant in 0..3 {
                let mut req = request(&h, "POST", tail, body.clone());
                if variant == 0 {
                    req.headers_mut().remove(name);
                } else if variant == 1 {
                    req.headers_mut()
                        .insert(name, HeaderValue::from_static(bad));
                } else {
                    let value = req.headers()[name].clone();
                    req.headers_mut().append(name, value);
                }
                check(&h, req, status, reason).await;
            }
        }
    }
    h.pool.close().await;
}
#[tokio::test]
async fn start_requires_the_frozen_string_target_and_closed_duplicate_free_object() {
    let h = RouteHarness::new().await;
    for body in [
        "",
        "null",
        "[]",
        r#"["AGE_PRIVACY"]"#,
        "{}",
        r#"{"return_target":null}"#,
        r#"{"return_target":{"type":"AGE_PRIVACY"}}"#,
        r#"{"return_target":"https://evil.invalid"}"#,
        r#"{"return_target":"AGE_PRIVACY","extra":1}"#,
        r#"{"return_target":"AGE_PRIVACY","return_target":"AGE_PRIVACY"}"#,
        r#"{"return_target":"AGE_PRIVACY","return_\u0074arget":"AGE_PRIVACY"}"#,
        r#"{"return_target":"AGE_PRIVACY"}{}"#,
    ] {
        check(&h, request(&h, "POST", START, body), 400, "INVALID_INPUT").await;
    }
    check(
        &h,
        request(&h, "POST", START, vec![0xff]),
        400,
        "INVALID_INPUT",
    )
    .await;
    check(
        &h,
        request(
            &h,
            "POST",
            START,
            r#"{ "return_\u0074arget" : "AGE_PRIVACY" }"#,
        ),
        503,
        "DEPENDENCY_UNAVAILABLE",
    )
    .await;
    h.pool.close().await;
}
#[tokio::test]
async fn callback_rejects_invalid_shape_duplicate_aliases_ids_and_tokens_before_dispatch() {
    let h = RouteHarness::new().await;
    let good = callback("state", "proof");
    let mut invalid = vec![
        "null".into(),
        "[]".into(),
        "{}".into(),
        format!("{good} {{}}"),
    ];
    for key in ["verification_id", "state_token", "provider_response_token"] {
        for value in [serde_json::Value::Null, serde_json::json!(7)] {
            let mut object: serde_json::Value = serde_json::from_str(&good).unwrap();
            object[key] = value;
            invalid.push(object.to_string());
        }
        let mut object: serde_json::Value = serde_json::from_str(&good).unwrap();
        object.as_object_mut().unwrap().remove(key);
        invalid.push(object.to_string());
        invalid.push(format!(
            "{},\"{key}\":\"duplicate\"}}",
            good.trim_end_matches('}')
        ));
    }
    invalid.push(good.replace(ID, "00000000-0000-0000-0000-000000000000"));
    invalid.push(good.replace(ID, &ID.to_uppercase()));
    invalid.push(good.replace(ID, "not-a-uuid"));
    invalid.push(format!(
        "{},\"state_\\u0074oken\":\"duplicate\"}}",
        good.trim_end_matches('}')
    ));
    invalid.push(format!(
        "{},\"verified\":true}}",
        good.trim_end_matches('}')
    ));
    for (state, proof) in [
        ("".into(), "proof".into()),
        ("state".into(), "".into()),
        ("x".repeat(513), "proof".into()),
        ("state".into(), "x".repeat(4097)),
        ("a\nb".into(), "proof".into()),
        ("state".into(), "\u{7f}".into()),
    ] {
        invalid.push(callback(&state, &proof));
    }
    for body in invalid {
        check(
            &h,
            request(&h, "POST", CALLBACK, body),
            400,
            "INVALID_INPUT",
        )
        .await;
    }
    check(
        &h,
        request(&h, "POST", CALLBACK, vec![0xff]),
        400,
        "INVALID_INPUT",
    )
    .await;
    h.pool.close().await;
}
#[tokio::test]
async fn token_character_limits_and_original_byte_body_bounds_are_independent() {
    let h = RouteHarness::new().await;
    for body in [
        callback(&"é".repeat(512), &"x".repeat(4096)),
        callback("state", &"x".repeat(4096)),
    ] {
        check(
            &h,
            request(&h, "POST", CALLBACK, body),
            503,
            "DEPENDENCY_UNAVAILABLE",
        )
        .await;
    }
    for (tail, body) in [
        (START, START_BODY.into()),
        (CALLBACK, callback("state", "proof")),
    ] {
        let exact = format!("{body}{}", " ".repeat(8192 - body.len()));
        check(
            &h,
            request(&h, "POST", tail, exact.clone()),
            503,
            "DEPENDENCY_UNAVAILABLE",
        )
        .await;
        check(
            &h,
            request(&h, "POST", tail, format!("{exact} ")),
            413,
            "PAYLOAD_TOO_LARGE",
        )
        .await;
    }
    h.pool.close().await;
}
#[tokio::test]
async fn empty_query_and_every_query_alias_are_rejected_and_reads_are_bodyless() {
    let h = RouteHarness::new().await;
    for (method, tail, body) in operations() {
        for query in [
            "?",
            "?limit=1",
            "?state=secret",
            "?%73tate=secret",
            "?x=1&x=2",
        ] {
            check(
                &h,
                request(&h, method, &format!("{tail}{query}"), body.clone()),
                400,
                "INVALID_INPUT",
            )
            .await;
        }
    }
    check(
        &h,
        request(&h, "GET", "me/age-status", "{}"),
        400,
        "INVALID_INPUT",
    )
    .await;
    for name in ["content-type", "idempotency-key"] {
        let mut req = request(&h, "GET", "me/age-status", "");
        req.headers_mut().insert(
            name,
            HeaderValue::from_static(if name == "content-type" {
                "application/json"
            } else {
                KEY
            }),
        );
        check(&h, req, 400, "INVALID_INPUT").await;
    }
    h.pool.close().await;
}
#[tokio::test]
async fn raw_paths_and_methods_never_open_extra_age_operations() {
    let h = RouteHarness::new().await;
    for (method, tail, body) in operations() {
        for bad in [
            "not-uuid".to_string(),
            "00000000-0000-0000-0000-000000000000".into(),
            EVENT.to_uppercase(),
            EVENT.replace("bb", "%62b"),
        ] {
            let mut req = request(&h, method, tail, body.clone());
            *req.uri_mut() = format!("/api/concert/events/{bad}/{tail}").parse().unwrap();
            check(&h, req, 400, "INVALID_INPUT").await;
        }
        for suffix in ["/", "/extra"] {
            check(
                &h,
                request(&h, method, &format!("{tail}{suffix}"), body.clone()),
                404,
                "RESOURCE_UNAVAILABLE",
            )
            .await;
        }
        check(&h, request(&h, "DELETE", tail, ""), 400, "INVALID_INPUT").await;
    }
    // No public erasure route or encoded callback alias is mounted.
    for tail in [
        "me/age-verifications/erase",
        "me/age-verifications/%63allback",
    ] {
        check(
            &h,
            request(&h, "POST", tail, "{}"),
            404,
            "RESOURCE_UNAVAILABLE",
        )
        .await;
    }
    h.pool.close().await;
}
