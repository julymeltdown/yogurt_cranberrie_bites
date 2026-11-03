use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use tower::util::ServiceExt;

#[tokio::test]
async fn apple_callback_accepts_the_cross_site_form_post() {
    let app = crate::support::start_postgres_app(true).await.expect("app");

    // Apple's response_mode=form_post posts the authorization result as a
    // form-encoded body rather than appending it as query parameters. The route
    // accepted only GET, so this POST was rejected with 405 and Apple sign-in
    // could never complete.
    let authorize_request = Request::builder()
        .method(Method::GET)
        .uri("/auth/oauth/apple/authorize")
        .body(Body::empty())
        .expect("request");
    let authorize_response = app
        .router
        .clone()
        .oneshot(authorize_request)
        .await
        .expect("response");
    let location = authorize_response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .expect("location")
        .to_owned();
    let state = extract_query_param(&location, "state").expect("state");

    let form_request = Request::builder()
        .method(Method::POST)
        .uri("/auth/oauth/apple/callback")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(format!("code=apple-hana-code&state={state}")))
        .expect("request");
    let (status, body, _) = crate::support::json_response(app.router, form_request)
        .await
        .expect("form-post callback");

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["userId"], "11111111-1111-1111-1111-111111111111");
}

#[tokio::test]
async fn apple_form_post_callback_still_enforces_state_binding() {
    let app = crate::support::start_postgres_app(true).await.expect("app");

    // Accepting the form post must not weaken state binding: the state is the
    // value that ties the authorization to the browser session, and a cross-site
    // POST is exactly where that check matters most.
    let form_request = Request::builder()
        .method(Method::POST)
        .uri("/auth/oauth/apple/callback")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("code=apple-hana-code&state=never-issued"))
        .expect("request");
    let (status, _, _) = crate::support::json_response(app.router, form_request)
        .await
        .expect("form-post callback");

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn oauth_http_contract_supports_authorize_callback_and_link() {
    let app = crate::support::start_postgres_app(true).await.expect("app");

    let authorize_request = Request::builder()
        .method(Method::GET)
        .uri("/auth/oauth/google/authorize")
        .body(Body::empty())
        .expect("request");
    let authorize_response = app
        .router
        .clone()
        .oneshot(authorize_request)
        .await
        .expect("response");
    assert_eq!(authorize_response.status(), StatusCode::FOUND);
    let location = authorize_response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .expect("location")
        .to_owned();
    let state = extract_query_param(&location, "state").expect("state");
    assert!(extract_query_param(&location, "code_challenge").is_some());
    assert_eq!(
        extract_query_param(&location, "code_challenge_method").as_deref(),
        Some("S256")
    );
    assert!(extract_query_param(&location, "nonce").is_some());

    let callback_request = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/auth/oauth/google/callback?code=google-hana-code&state={state}"
        ))
        .body(Body::empty())
        .expect("request");
    let (callback_status, callback_body, _) =
        crate::support::json_response(app.router.clone(), callback_request)
            .await
            .expect("callback");
    assert_eq!(callback_status, StatusCode::OK);
    assert_eq!(
        callback_body["userId"],
        "11111111-1111-1111-1111-111111111111"
    );
    assert!(callback_body["linkedProviders"]
        .as_array()
        .expect("providers")
        .iter()
        .any(|value| value == "GOOGLE"));

    let login_request = Request::builder()
        .method(Method::POST)
        .uri("/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"email":"hana@example.com","password":"Passw0rd!"}"#,
        ))
        .expect("request");
    let (_, login_body, _) = crate::support::json_response(app.router.clone(), login_request)
        .await
        .expect("login");
    let access_token = login_body["accessToken"].as_str().expect("access token");

    let apple_authorize_request = Request::builder()
        .method(Method::GET)
        .uri("/auth/oauth/apple/authorize")
        .body(Body::empty())
        .expect("request");
    let apple_authorize_response = app
        .router
        .clone()
        .oneshot(apple_authorize_request)
        .await
        .expect("apple authorize response");
    assert_eq!(apple_authorize_response.status(), StatusCode::FOUND);
    let apple_location = apple_authorize_response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .expect("apple location")
        .to_owned();
    let apple_state = extract_query_param(&apple_location, "state").expect("apple state");

    let link_request = Request::builder()
        .method(Method::POST)
        .uri("/auth/oauth/apple/link")
        .header("authorization", format!("Bearer {access_token}"))
        .header("content-type", "application/json")
        .body(Body::from(format!(
            r#"{{"code":"apple-hana-code","state":"{apple_state}"}}"#
        )))
        .expect("request");
    let (link_status, link_body, _) =
        crate::support::json_response(app.router.clone(), link_request)
            .await
            .expect("link");
    assert_eq!(link_status, StatusCode::OK);
    assert!(link_body["providers"]
        .as_array()
        .expect("providers")
        .iter()
        .any(|value| value == "APPLE"));

    let x_authorize_request = Request::builder()
        .method(Method::GET)
        .uri("/auth/oauth/x/authorize")
        .body(Body::empty())
        .expect("request");
    let x_authorize_response = app
        .router
        .clone()
        .oneshot(x_authorize_request)
        .await
        .expect("x authorize response");
    assert_eq!(x_authorize_response.status(), StatusCode::FOUND);
    let x_location = x_authorize_response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .expect("x location")
        .to_owned();
    let x_state = extract_query_param(&x_location, "state").expect("x state");

    let x_link_request = Request::builder()
        .method(Method::POST)
        .uri("/auth/oauth/x/link")
        .header("authorization", format!("Bearer {access_token}"))
        .header("content-type", "application/json")
        .body(Body::from(format!(
            r#"{{"code":"x-hana-code","state":"{x_state}"}}"#
        )))
        .expect("request");
    let (x_link_status, x_link_body, _) = crate::support::json_response(app.router, x_link_request)
        .await
        .expect("x link");
    assert_eq!(x_link_status, StatusCode::OK);
    assert!(x_link_body["providers"]
        .as_array()
        .expect("providers")
        .iter()
        .any(|value| value == "X"));
}

#[tokio::test]
async fn oauth_metrics_report_provider_exchange_success_and_failure() {
    let app = crate::support::start_postgres_app(true).await.expect("app");

    let success_state = authorize_state(app.router.clone(), "google").await;
    let success_request = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/auth/oauth/google/callback?code=google-hana-code&state={success_state}"
        ))
        .body(Body::empty())
        .expect("request");
    let (success_status, _, _) = crate::support::json_response(app.router.clone(), success_request)
        .await
        .expect("success callback");
    assert_eq!(success_status, StatusCode::OK);

    let failure_state = authorize_state(app.router.clone(), "google").await;
    let failure_request = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/auth/oauth/google/callback?code=unknown-provider-code&state={failure_state}"
        ))
        .body(Body::empty())
        .expect("request");
    let (failure_status, _, _) = crate::support::json_response(app.router.clone(), failure_request)
        .await
        .expect("failure callback");
    assert_eq!(failure_status, StatusCode::BAD_REQUEST);

    let metrics_response =
        auth_service::adapters::r#in::http::metrics_router(app.state.metrics.clone())
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/metrics")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("metrics response");
    assert_eq!(metrics_response.status(), StatusCode::OK);
    let metrics_body = String::from_utf8(
        metrics_response
            .into_body()
            .collect()
            .await
            .expect("metrics body")
            .to_bytes()
            .to_vec(),
    )
    .expect("utf8 metrics");

    assert!(metrics_body.contains(
        "auth_oauth_provider_exchanges_total{provider=\"google\",outcome=\"SUCCESS\"} 1"
    ));
    assert!(metrics_body.contains(
        "auth_oauth_provider_exchanges_total{provider=\"google\",outcome=\"FAILURE\"} 1"
    ));
}

#[tokio::test]
async fn oauth_provider_exchange_audit_records_request_context_without_provider_codes() {
    let app = crate::support::start_postgres_app(true).await.expect("app");

    let success_state = authorize_state(app.router.clone(), "google").await;
    let success_request = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/auth/oauth/google/callback?code=google-hana-code&state={success_state}"
        ))
        .header("x-request-id", "req-oauth-provider-success")
        .header("x-forwarded-for", "198.51.100.20, 10.0.0.1")
        .header("user-agent", "oauth-audit-test/1.0")
        .body(Body::empty())
        .expect("request");
    let (success_status, _, _) = crate::support::json_response(app.router.clone(), success_request)
        .await
        .expect("success callback");
    assert_eq!(success_status, StatusCode::OK);

    let failure_state = authorize_state(app.router.clone(), "google").await;
    let failure_request = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/auth/oauth/google/callback?code=unknown-provider-code&state={failure_state}"
        ))
        .header("x-request-id", "req-oauth-provider-failure")
        .header("x-forwarded-for", "198.51.100.21")
        .header("user-agent", "oauth-audit-test/1.0")
        .body(Body::empty())
        .expect("request");
    let (failure_status, _, _) = crate::support::json_response(app.router.clone(), failure_request)
        .await
        .expect("failure callback");
    assert_eq!(failure_status, StatusCode::BAD_REQUEST);

    let rows = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, String)>(
        r#"
        select outcome, request_id, ip_address, user_agent, metadata_json
        from auth_audit_events
        where action = 'OAUTH_PROVIDER_EXCHANGE'
        order by occurred_at asc
        "#,
    )
    .fetch_all(&app.pool)
    .await
    .expect("provider exchange audit rows");

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, "SUCCESS");
    assert_eq!(rows[0].1, "req-oauth-provider-success");
    assert_eq!(rows[0].2.as_deref(), Some("198.51.100.20"));
    assert_eq!(rows[0].3.as_deref(), Some("oauth-audit-test/1.0"));
    assert!(rows[0].4.contains(r#""provider":"GOOGLE""#));
    assert!(!rows[0].4.contains("google-hana-code"));

    assert_eq!(rows[1].0, "FAILURE");
    assert_eq!(rows[1].1, "req-oauth-provider-failure");
    assert_eq!(rows[1].2.as_deref(), Some("198.51.100.21"));
    assert_eq!(rows[1].3.as_deref(), Some("oauth-audit-test/1.0"));
    assert!(rows[1].4.contains(r#""provider":"GOOGLE""#));
    assert!(rows[1].4.contains(r#""flow":"callback""#));
    assert!(rows[1].4.contains(r#""error":"PROVIDER_CODE_INVALID""#));
    assert!(!rows[1].4.contains("unknown-provider-code"));
}

async fn authorize_state(router: axum::Router, provider: &str) -> String {
    let authorize_request = Request::builder()
        .method(Method::GET)
        .uri(format!("/auth/oauth/{provider}/authorize"))
        .body(Body::empty())
        .expect("request");
    let authorize_response = router
        .oneshot(authorize_request)
        .await
        .expect("authorize response");
    assert_eq!(authorize_response.status(), StatusCode::FOUND);
    let location = authorize_response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .expect("location");
    extract_query_param(location, "state").expect("state")
}

fn extract_query_param(uri: &str, key: &str) -> Option<String> {
    let query = uri.split('?').nth(1)?;
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=')?;
        if name == key {
            return Some(value.to_owned());
        }
    }
    None
}
