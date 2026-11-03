//! HTTP adapter tests use a recording Auth double. Real Auth state consumption
//! and browser SameSite behavior require separate runtime acceptance.
use crate::http_support::{response_json, RouteHarness};
use api_gateway::adapters::r#in::http::oauth_routes;
use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use tower::ServiceExt;

fn form_request(path: &str, body: impl Into<Body>, cookie: Option<&str>) -> Request<Body> {
    let mut request = Request::builder()
        .method("POST")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    request.body(body.into()).expect("form request")
}

#[tokio::test]
async fn transport_rejection_is_protected_before_the_oauth_handler_runs() {
    let harness = RouteHarness::new().await;
    for path in [
        "/auth/oauth/apple/callback",
        "/auth/oauth/apple/callback/complete",
    ] {
        let mut request = form_request(path, "code=fixture-code&state=apple-state", None);
        request
            .headers_mut()
            .insert("idempotency-key", "not-supported-for-auth".parse().unwrap());
        let response = harness.transport_router().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(response.headers()["referrer-policy"], "no-referrer");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        assert!(!response.headers().contains_key(header::SET_COOKIE));
        assert!(response_json(response).await["message"]
            .as_str()
            .unwrap()
            .contains("Idempotency-Key"));
    }
    assert!(harness
        .clients
        .oauth_callback_calls
        .lock()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn composed_quota_saturation_and_deadline_errors_keep_security_headers() {
    use api_gateway::domain::admission::{AdmissionPolicies, RouteClass};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio_stream::StreamExt;

    fn assert_protected(response: &axum::response::Response, status: StatusCode) {
        assert_eq!(response.status(), status);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(response.headers()["referrer-policy"], "no-referrer");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        assert!(!response.headers().contains_key(header::SET_COOKIE));
    }

    for path in [
        "/auth/oauth/apple/callback",
        "/auth/oauth/apple/callback/complete",
    ] {
        let harness = RouteHarness::with_admission(
            AdmissionPolicies::default().with_budget(RouteClass::Login, 1),
        )
        .await
        .unwrap();
        let app = harness.transport_router();
        // The invalid form consumes the class quota but cannot reach Auth.
        let first = app
            .clone()
            .oneshot(form_request(
                path,
                "code=&state=apple-state",
                Some("oauth_state_apple=apple-state"),
            ))
            .await
            .unwrap();
        assert_protected(&first, StatusCode::BAD_REQUEST);
        let second = app
            .oneshot(form_request(
                path,
                "code=x&state=apple-state",
                Some("oauth_state_apple=apple-state"),
            ))
            .await
            .unwrap();
        assert_protected(&second, StatusCode::TOO_MANY_REQUESTS);
        assert!(harness
            .clients
            .oauth_callback_calls
            .lock()
            .unwrap()
            .is_empty());

        let harness = RouteHarness::with_admission(
            AdmissionPolicies::default().with_max_in_flight(RouteClass::Login, 1),
        )
        .await
        .unwrap();
        let app = harness.transport_router();
        let readable = Arc::new(tokio::sync::Notify::new());
        let signal = readable.clone();
        let (sender, receiver) = tokio::sync::mpsc::channel::<axum::body::Bytes>(1);
        sender
            .send(axum::body::Bytes::from_static(b"code=x&state="))
            .await
            .unwrap();
        let stream = tokio_stream::wrappers::ReceiverStream::new(receiver).map(move |bytes| {
            signal.notify_one();
            Ok::<_, std::io::Error>(bytes)
        });
        let request = form_request(
            path,
            Body::from_stream(stream),
            Some("oauth_state_apple=apple-state"),
        );
        let first_app = app.clone();
        let pending = tokio::spawn(async move { first_app.oneshot(request).await.unwrap() });
        // Polling the body proves the first request passed production admission;
        // keep its stream open to hold the actual per-class permit.
        tokio::time::timeout(Duration::from_secs(3), readable.notified())
            .await
            .unwrap();
        let second = app
            .oneshot(form_request(
                path,
                "code=x&state=apple-state",
                Some("oauth_state_apple=apple-state"),
            ))
            .await
            .unwrap();
        assert_protected(&second, StatusCode::SERVICE_UNAVAILABLE);
        let first = tokio::time::timeout(Duration::from_secs(3), pending)
            .await
            .unwrap()
            .unwrap();
        assert_protected(&first, StatusCode::GATEWAY_TIMEOUT);
        drop(sender);
        assert!(harness
            .clients
            .oauth_callback_calls
            .lock()
            .unwrap()
            .is_empty());
    }
}

#[tokio::test]
async fn staging_does_not_exchange_code_or_modify_cookies_and_escapes_attributes() {
    let harness = RouteHarness::new().await;
    let response = oauth_routes::router(harness.state.clone()).oneshot(form_request(
        "/auth/oauth/apple/callback",
        "code=%22%3E%3Cscript%3Eattack%3C%2Fscript%3E%26%27&state=apple-state&id_token=IGNORED_SECRET&user=IGNORED_PROFILE",
        None,
    )).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    assert!(!response.headers().contains_key(header::LOCATION));
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()["referrer-policy"], "no-referrer");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    let csp = response.headers()["content-security-policy"]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(csp.contains("form-action 'self'; frame-ancestors 'none'"));
    let nonce = csp.split("'nonce-").nth(1).unwrap().trim_end_matches('\'');
    assert_eq!(nonce.len(), 32);
    let body = to_bytes(response.into_body(), 65536).await.unwrap();
    let body = std::str::from_utf8(&body).unwrap();
    assert!(body.contains("action=\"/auth/oauth/apple/callback/complete\""));
    assert!(body.contains("value=\"&quot;&gt;&lt;script&gt;attack&lt;/script&gt;&amp;&#39;\""));
    assert!(body.contains(&format!("<script nonce=\"{nonce}\">")));
    assert!(!body.contains("IGNORED_SECRET"));
    assert!(!body.contains("IGNORED_PROFILE"));
    assert!(!body.contains("<script>attack"));
    assert!(harness
        .clients
        .oauth_callback_calls
        .lock()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn invalid_forms_and_other_providers_never_reach_auth() {
    let harness = RouteHarness::new().await;
    let app = oauth_routes::router(harness.state.clone());
    let oversized = format!("code={}&state=apple-state", "x".repeat(32768));
    for (provider, body) in [
        ("google", "code=x&state=y"),
        ("x", "code=x&state=y"),
        ("apple", "code=x&code=y&state=z"),
        ("apple", "code=x&st%61te=y&state=y"),
        ("apple", "code=%GG&state=y"),
        ("apple", "code=%FF&state=y"),
        ("apple", "code=x&state=%00"),
        ("apple", "code=x&state="),
        ("apple", "code=x&state=y&extra=bad"),
        (
            "apple",
            "error=access_denied&state=y&error_description=PRIVATE_PROVIDER_TEXT",
        ),
        ("apple", "code=x&state=y&error=bad"),
        ("apple", oversized.as_str()),
    ] {
        let response = app
            .clone()
            .oneshot(form_request(
                &format!("/auth/oauth/{provider}/callback"),
                body.to_owned(),
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{provider}");
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert!(!response.headers().contains_key(header::SET_COOKIE));
        let value = response_json(response).await;
        assert_eq!(value["code"], "GATEWAY_400");
        assert!(!value.to_string().contains("PRIVATE_PROVIDER_TEXT"));
    }
    let request = Request::builder()
        .method("POST")
        .uri("/auth/oauth/apple/callback")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("code=x&state=y"))
        .unwrap();
    assert_eq!(
        app.oneshot(request).await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );
    assert!(harness
        .clients
        .oauth_callback_calls
        .lock()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn completion_requires_exact_unambiguous_apple_cookie_before_auth() {
    let harness = RouteHarness::new().await;
    let app = oauth_routes::router(harness.state.clone());
    for cookie in [
        None,
        Some("oauth_state_apple=other-browser"),
        Some("oauth_state_google=apple-state"),
        Some("oauth_state_apple=apple-state; oauth_state_apple=apple-state"),
    ] {
        let response = app
            .clone()
            .oneshot(form_request(
                "/auth/oauth/apple/callback/complete",
                "code=provider-code&state=apple-state",
                cookie,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(!response.headers().contains_key(header::SET_COOKIE));
    }
    assert!(harness
        .clients
        .oauth_callback_calls
        .lock()
        .unwrap()
        .is_empty());
    let response = app
        .oneshot(form_request(
            "/auth/oauth/apple/callback/complete",
            "code=provider-code&state=apple-state",
            Some("oauth_state_apple=apple-state"),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookies: Vec<_> = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|header| header.to_str().unwrap())
        .collect();
    assert!(cookies
        .iter()
        .any(|cookie| cookie.contains("refresh-from-oauth") && cookie.contains("HttpOnly")));
    assert!(cookies
        .iter()
        .any(|cookie| cookie.starts_with("oauth_state_apple=;") && cookie.contains("Max-Age=0")));
    assert_eq!(response_json(response).await["refreshToken"], "");
    assert_eq!(
        harness.clients.oauth_callback_calls.lock().unwrap().len(),
        1
    );
}
