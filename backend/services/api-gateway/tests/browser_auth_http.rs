//! C03 composed executable/router policy with counted Auth ports and no live database.
#[allow(dead_code)]
#[path = "unit/http_support.rs"]
mod http_support;
use api_gateway::adapters::r#in::http;
use api_gateway::application::dto::verification_dto::{
    ResendVerificationRequest, ResendVerificationResponse,
};
use api_gateway::application::verification::{VerificationClientPort, VerificationError};
use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use http_support::{RouteHarness, SessionAnswer, USER_ID};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

#[async_trait::async_trait]
impl VerificationClientPort for http_support::TestClients {
    async fn resend_verification(
        &self,
        _request: ResendVerificationRequest,
    ) -> Result<ResendVerificationResponse, VerificationError> {
        self.auth_calls.lock().unwrap().push("resend");
        Ok(ResendVerificationResponse::accepted())
    }
}

const ORIGIN: &str = "http://localhost:3000";

async fn harness() -> RouteHarness {
    let mut harness = RouteHarness::new().await;
    // Count downstream ports without depending on a live distributed quota store.
    Arc::make_mut(&mut harness.state).rate_limiter =
        api_gateway::config::InMemoryRateLimiter::new(1000, std::time::Duration::from_secs(60));
    let state = Arc::make_mut(&mut harness.state);
    state.auth_service = state
        .auth_service
        .clone()
        .with_verification_client(harness.clients.clone());
    harness
}

fn request(
    method: &str,
    path: &str,
    origin: Option<&str>,
    intent: Option<&str>,
    body: &str,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-correlation-id", "be03-request");
    if let Some(origin) = origin {
        builder = builder.header(header::ORIGIN, origin);
    }
    if let Some(intent) = intent {
        builder = builder.header("x-mcr-intent", intent);
    }
    builder.body(Body::from(body.to_owned())).unwrap()
}

async fn body(response: axum::response::Response, status: StatusCode) -> Value {
    assert_eq!(response.status(), status);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()["x-correlation-id"], "be03-request");
    assert_eq!(response.headers()["referrer-policy"], "no-referrer");
    assert!(!response.headers().contains_key(header::LOCATION));
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    }
}

#[tokio::test]
async fn every_auth_mutation_denies_untrusted_origin_before_any_auth_port_or_cookie_effect() {
    let h = harness().await;
    let app = h.transport_router();
    for (method, path) in [
        ("POST", "/api/auth/email/signup"),
        ("POST", "/api/auth/email/verify"),
        ("POST", "/api/auth/email/resend"),
        ("POST", "/api/auth/login"),
        ("POST", "/api/auth/password/reset/request"),
        ("POST", "/api/auth/password/reset/confirm"),
        ("POST", "/api/auth/refresh"),
        ("POST", "/api/auth/logout"),
        ("DELETE", "/api/auth/account"),
        ("POST", "/auth/login"),
    ] {
        for origin in [
            None,
            Some("null"),
            Some("*"),
            Some("https://hostile.test"),
            Some("http://localhost:3000.hostile.test"),
            Some("http://localhost:3000/"),
        ] {
            let mut req = request(method, path, origin, Some("browser"), "not-json");
            req.headers_mut()
                .insert(header::COOKIE, "refresh_token=private".parse().unwrap());
            let response = app.clone().oneshot(req).await.unwrap();
            assert!(!response.headers().contains_key(header::SET_COOKIE));
            let value = body(response, StatusCode::FORBIDDEN).await;
            assert_eq!(value["traceId"], "be03-request");
            assert_eq!(value["details"]["reason"], "ORIGIN_NOT_ALLOWED");
        }
    }
    assert!(h.clients.auth_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn exact_intent_and_single_headers_are_required() {
    let h = harness().await;
    let app = h.transport_router();
    for intent in [
        None,
        Some("Browser"),
        Some("browser "),
        Some("browser, browser"),
    ] {
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                "/api/auth/login",
                Some(ORIGIN),
                intent,
                "{}",
            ))
            .await
            .unwrap();
        assert_eq!(
            body(response, StatusCode::FORBIDDEN).await["details"]["reason"],
            "BROWSER_INTENT_REQUIRED"
        );
    }
    for name in ["origin", "x-mcr-intent"] {
        let mut req = request(
            "POST",
            "/api/auth/login",
            Some(ORIGIN),
            Some("browser"),
            "{}",
        );
        let duplicate = req.headers()[name].clone();
        req.headers_mut().append(name, duplicate);
        body(
            app.clone().oneshot(req).await.unwrap(),
            StatusCode::FORBIDDEN,
        )
        .await;
    }
    assert!(h.clients.auth_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn legacy_oauth_and_availability_have_no_runtime_dispatch() {
    let h = harness().await;
    let app = h.transport_router();
    for prefix in ["/api/auth", "/auth"] {
        for (method, suffix) in [
            ("POST", "/email/check"),
            ("GET", "/oauth/google/authorize"),
            ("GET", "/oauth/x/callback?code=secret&state=secret"),
            ("POST", "/oauth/apple/callback"),
            ("POST", "/oauth/apple/callback/complete"),
            ("POST", "/oauth/google/link"),
        ] {
            let response = app
                .clone()
                .oneshot(request(
                    method,
                    &format!("{prefix}{suffix}"),
                    Some(ORIGIN),
                    Some("browser"),
                    "{}",
                ))
                .await
                .unwrap();
            assert!(!response.headers().contains_key(header::SET_COOKIE));
            body(response, StatusCode::NOT_FOUND).await;
        }
    }
    body(
        app.clone()
            .oneshot(request(
                "POST",
                "/auth/login",
                Some(ORIGIN),
                Some("browser"),
                "{}",
            ))
            .await
            .unwrap(),
        StatusCode::NOT_FOUND,
    )
    .await;
    assert!(h.clients.auth_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn capabilities_are_public_and_optional_providers_cannot_be_enabled() {
    let h = harness().await;
    let value = body(
        h.transport_router()
            .oneshot(request("GET", "/api/auth/capabilities", None, None, ""))
            .await
            .unwrap(),
        StatusCode::OK,
    )
    .await;
    assert_eq!(value["contract_version"], "0.1.0");
    assert_eq!(value["providers"].as_array().unwrap().len(), 4);
    assert_eq!(
        value["providers"][0],
        json!({"provider":"EMAIL","enabled":true,"disabled_reason":null})
    );
    for provider in value["providers"].as_array().unwrap().iter().skip(1) {
        assert_eq!(provider["enabled"], false);
        assert_eq!(provider["disabled_reason"], "HANDOFF_NOT_QUALIFIED");
    }
    assert!(h.clients.auth_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn every_qualified_email_and_account_path_dispatches_to_its_owning_port() {
    let h = harness().await;
    let app = h.transport_router();
    for (method, path, payload, status, operation) in [
        (
            "POST",
            "/api/auth/email/signup",
            r#"{"email":"synthetic@example.test","password":"password"}"#,
            StatusCode::OK,
            "signup",
        ),
        (
            "POST",
            "/api/auth/email/verify",
            r#"{"email":"synthetic@example.test","code":"123456"}"#,
            StatusCode::OK,
            "verify",
        ),
        (
            "POST",
            "/api/auth/email/resend",
            r#"{"email":"synthetic@example.test"}"#,
            StatusCode::ACCEPTED,
            "resend",
        ),
        (
            "POST",
            "/api/auth/password/reset/request",
            r#"{"email":"synthetic@example.test"}"#,
            StatusCode::OK,
            "request_password_reset",
        ),
        (
            "POST",
            "/api/auth/password/reset/confirm",
            r#"{"token":"private","newPassword":"password"}"#,
            StatusCode::OK,
            "confirm_password_reset",
        ),
        (
            "GET",
            "/api/auth/account",
            "",
            StatusCode::OK,
            "get_user_deletion",
        ),
        (
            "GET",
            "/api/auth/account/deletion",
            "",
            StatusCode::OK,
            "get_user_deletion",
        ),
        (
            "DELETE",
            "/api/auth/account",
            "",
            StatusCode::ACCEPTED,
            "delete_account",
        ),
    ] {
        h.clients.auth_calls.lock().unwrap().clear();
        let mut req = request(method, path, Some(ORIGIN), Some("browser"), payload);
        req.headers_mut().insert(
            header::AUTHORIZATION,
            h.bearer_token(USER_ID).parse().unwrap(),
        );
        body(app.clone().oneshot(req).await.unwrap(), status).await;
        assert!(
            h.clients.auth_calls.lock().unwrap().contains(&operation),
            "{method} {path}"
        );
    }
}

#[tokio::test]
async fn login_refresh_logout_preserve_cookie_contract_and_empty_refresh_body() {
    let h = harness().await;
    let app = h.transport_router();
    for (path, payload) in [
        (
            "/api/auth/login",
            r#"{"email":"user@example.test","password":"password"}"#,
        ),
        ("/api/auth/refresh", "{}"),
    ] {
        let mut req = request("POST", path, Some(ORIGIN), Some("browser"), payload);
        req.headers_mut()
            .insert(header::COOKIE, "refresh_token=old-refresh".parse().unwrap());
        let response = app.clone().oneshot(req).await.unwrap();
        let cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
        assert!(
            cookie.contains("HttpOnly")
                && cookie.contains("Path=/")
                && cookie.contains("SameSite=Lax")
        );
        let value = body(response, StatusCode::OK).await;
        assert_eq!(value["refreshToken"], "");
        assert_eq!(value["userId"], USER_ID);
    }
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/auth/logout",
            Some(ORIGIN),
            Some("browser"),
            "{}",
        ))
        .await
        .unwrap();
    assert!(response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .contains("Max-Age=0"));
    body(response, StatusCode::NO_CONTENT).await;
    assert!(h.clients.logout_calls.lock().unwrap().is_empty());
    assert_eq!(h.clients.login_calls.lock().unwrap().len(), 1);
    assert_eq!(*h.clients.refresh_calls.lock().unwrap(), ["old-refresh"]);
}

#[tokio::test]
async fn account_deletion_keeps_live_session_requirement_at_new_path() {
    let h = harness().await;
    let app = h.transport_router();
    let mut req = request(
        "DELETE",
        "/api/auth/account",
        Some(ORIGIN),
        Some("browser"),
        "",
    );
    req.headers_mut().insert(
        header::AUTHORIZATION,
        h.bearer_token_without_session(USER_ID, vec!["ROLE_USER".into()])
            .parse()
            .unwrap(),
    );
    body(
        app.clone().oneshot(req).await.unwrap(),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    assert!(h.clients.delete_account_calls.lock().unwrap().is_empty());
    h.clients.answer_sessions(SessionAnswer::Ended);
    let mut req = request(
        "DELETE",
        "/api/auth/account",
        Some(ORIGIN),
        Some("browser"),
        "",
    );
    req.headers_mut().insert(
        header::AUTHORIZATION,
        h.bearer_token(USER_ID).parse().unwrap(),
    );
    body(app.oneshot(req).await.unwrap(), StatusCode::UNAUTHORIZED).await;
    assert!(h.clients.delete_account_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn extractor_and_transport_errors_are_not_cached_and_guard_runs_on_bare_composition() {
    let h = harness().await;
    let response = http::router(h.state.clone())
        .oneshot(request("POST", "/api/auth/login", None, None, "{}"))
        .await
        .unwrap();
    body(response, StatusCode::FORBIDDEN).await;
    let app = h.transport_router();
    for (payload, status) in [
        ("{", StatusCode::BAD_REQUEST),
        ("{}", StatusCode::UNPROCESSABLE_ENTITY),
    ] {
        body(
            app.clone()
                .oneshot(request(
                    "POST",
                    "/api/auth/login",
                    Some(ORIGIN),
                    Some("browser"),
                    payload,
                ))
                .await
                .unwrap(),
            status,
        )
        .await;
    }
    let mut req = request(
        "POST",
        "/api/auth/login",
        Some(ORIGIN),
        Some("browser"),
        "{}",
    );
    req.headers_mut().insert(
        "idempotency-key",
        "unsupported-auth-replay".parse().unwrap(),
    );
    body(app.oneshot(req).await.unwrap(), StatusCode::BAD_REQUEST).await;
    assert!(h.clients.auth_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn other_public_mutations_cannot_bypass_the_browser_guard() {
    let h = harness().await;
    for (method, path) in [
        ("POST", "/api/posts"),
        ("PUT", "/api/profile/privacy"),
        ("POST", "/api/follows"),
        ("POST", "/admin/policies"),
    ] {
        let response = h
            .transport_router()
            .oneshot(request(method, path, None, None, "{}"))
            .await
            .unwrap();
        assert_eq!(
            body(response, StatusCode::FORBIDDEN).await["details"]["reason"],
            "ORIGIN_NOT_ALLOWED"
        );
    }
    assert!(h.clients.auth_calls.lock().unwrap().is_empty());
    assert!(h.clients.create_post_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn capabilities_quota_errors_use_the_core_reason_envelope() {
    let mut h = harness().await;
    Arc::make_mut(&mut h.state).rate_limiter =
        api_gateway::config::InMemoryRateLimiter::new(1, std::time::Duration::from_secs(60));
    let app = h.transport_router();
    let req = || request("GET", "/api/auth/capabilities", None, None, "");
    body(app.clone().oneshot(req()).await.unwrap(), StatusCode::OK).await;
    let value = body(
        app.oneshot(req()).await.unwrap(),
        StatusCode::TOO_MANY_REQUESTS,
    )
    .await;
    assert_eq!(value["details"], json!({"reason":"RATE_LIMITED"}));
    assert!(h.clients.auth_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn throttled_login_is_429_with_the_admission_window_as_retry_after() {
    let mut h = harness().await;
    Arc::make_mut(&mut h.state).rate_limiter =
        api_gateway::config::InMemoryRateLimiter::new(1, std::time::Duration::from_secs(60));
    let app = h.transport_router();
    let payload = r#"{"email":"user@example.test","password":"password"}"#;
    let login = || {
        request(
            "POST",
            "/api/auth/login",
            Some(ORIGIN),
            Some("browser"),
            payload,
        )
    };

    let first = app.clone().oneshot(login()).await.unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(h.clients.login_calls.lock().unwrap().len(), 1);

    let throttled = app.oneshot(login()).await.unwrap();
    assert_eq!(throttled.headers()[header::RETRY_AFTER], "60");
    let value = body(throttled, StatusCode::TOO_MANY_REQUESTS).await;
    // The admission middleware answers with the neutral Gateway envelope: the
    // refusal names no account, credential, route internals or owner metadata.
    assert_eq!(value["code"], "GATEWAY_429");
    assert_eq!(value["message"], "Rate limit exceeded");
    assert!(value["details"].is_null());
    let rendered = value.to_string();
    assert!(!rendered.contains("user@example.test"));
    assert!(!rendered.contains("password"));
    assert_eq!(
        h.clients.login_calls.lock().unwrap().len(),
        1,
        "a throttled login must never reach password verification"
    );
}

#[tokio::test]
async fn login_body_beyond_the_public_bound_is_413_before_auth_dispatch() {
    let h = harness().await;
    let app = h.transport_router();
    // Axum's public JSON bound is 2 MiB; the login handler must never see the
    // payload and the Auth port must never be called.
    let oversized = "x".repeat(2 * 1024 * 1024 + 1);
    let response = app
        .oneshot(request(
            "POST",
            "/api/auth/login",
            Some(ORIGIN),
            Some("browser"),
            &oversized,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert!(h.clients.login_calls.lock().unwrap().is_empty());
    assert!(h.clients.auth_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn secure_cookie_and_clear_attributes_match_when_explicitly_configured() {
    let mut h = harness().await;
    let state = Arc::make_mut(&mut h.state);
    state.refresh_cookie_secure = true;
    state.refresh_cookie_domain = "app.company.com".into();
    let app = h.transport_router();
    for (path, payload, status) in [
        (
            "/api/auth/login",
            r#"{"email":"user@example.test","password":"password"}"#,
            StatusCode::OK,
        ),
        ("/api/auth/logout", "{}", StatusCode::NO_CONTENT),
    ] {
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                path,
                Some(ORIGIN),
                Some("browser"),
                payload,
            ))
            .await
            .unwrap();
        let cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
        for attribute in [
            "Secure",
            "HttpOnly",
            "Path=/",
            "SameSite=Lax",
            "Domain=app.company.com",
        ] {
            assert!(cookie.contains(attribute), "{attribute}");
        }
        body(response, status).await;
    }
}
