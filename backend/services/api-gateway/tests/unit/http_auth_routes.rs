use crate::http_support::{response_json, DeleteAccountCall, LoginCall, RouteHarness, USER_ID};
use api_gateway::adapters::r#in::http::{admin_routes, auth_routes, oauth_routes, refresh_routes};
use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn login_sets_cookie_strips_refresh_token_and_forwards_client_context() {
    let harness = RouteHarness::new().await;
    let app = auth_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::POST)
        .uri("/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-forwarded-for", "203.0.113.7")
        .header(header::USER_AGENT, "route-test")
        .body(Body::from(
            json!({"email": "hana@example.com", "password": "secret"}).to_string(),
        ))
        .expect("request");

    let response = app.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .expect("refresh cookie")
        .to_str()
        .expect("cookie header");
    assert!(cookie.contains("refresh_token=refresh-from-login"));
    assert!(cookie.contains("HttpOnly"));
    let body = response_json(response).await;
    assert_eq!(body["accessToken"], "access-token");
    assert_eq!(body["refreshToken"], "");
    assert_eq!(
        *harness.clients.login_calls.lock().expect("login calls"),
        vec![LoginCall {
            email: "hana@example.com".into(),
            ip_address: Some("203.0.113.7".into()),
            user_agent: Some("route-test".into()),
        }]
    );
}

#[tokio::test]
async fn refresh_without_cookie_returns_unauthorized_gateway_error() {
    let harness = RouteHarness::new().await;
    let app = refresh_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::POST)
        .uri("/auth/refresh")
        .body(Body::empty())
        .expect("request");

    let response = app.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = response_json(response).await;
    assert_eq!(body["code"], "GATEWAY_401");
    assert_eq!(body["message"], "Missing refresh token");
    assert!(body["traceId"].is_string());
    assert!(body["timestamp"].is_string());
}

#[tokio::test]
async fn refresh_reads_and_rotates_the_invariant_refresh_token_cookie() {
    let harness = RouteHarness::new().await;
    let app = refresh_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::POST)
        .uri("/auth/refresh")
        .header(header::COOKIE, "refresh_token=presented-refresh")
        .body(Body::empty())
        .expect("request");

    let response = app.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .expect("refresh cookie")
        .to_str()
        .expect("cookie header");
    assert!(cookie.starts_with("refresh_token=refresh-from-refresh;"));
    assert_eq!(
        harness
            .clients
            .refresh_calls
            .lock()
            .expect("refresh calls")
            .as_slice(),
        &["presented-refresh".to_owned()]
    );
}

#[tokio::test]
async fn logout_without_refresh_cookie_clears_cookie_without_downstream_call() {
    let harness = RouteHarness::new().await;
    let app = auth_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::POST)
        .uri("/auth/logout")
        .body(Body::empty())
        .expect("request");

    let response = app.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .expect("clear cookie")
        .to_str()
        .expect("cookie header");
    assert!(cookie.contains("refresh_token="));
    assert!(cookie.contains("Max-Age=0"));
    assert!(harness
        .clients
        .logout_calls
        .lock()
        .expect("logout calls")
        .is_empty());
}

#[tokio::test]
async fn account_delete_forwards_authenticated_context_clears_cookie_and_returns_saga() {
    let harness = RouteHarness::new().await;
    let app = auth_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::DELETE)
        .uri("/auth/account")
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .header("x-request-id", "delete-req")
        .header("x-forwarded-for", "203.0.113.9")
        .header(header::USER_AGENT, "account-route-test")
        .body(Body::empty())
        .expect("request");

    let response = app.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .expect("clear refresh cookie")
        .to_str()
        .expect("cookie");
    assert!(cookie.contains("Max-Age=0"));
    let body = response_json(response).await;
    assert_eq!(body["deleted"], true);
    assert_eq!(body["saga"]["status"], "IN_PROGRESS");
    assert_eq!(
        harness
            .clients
            .delete_account_calls
            .lock()
            .expect("delete account calls")
            .as_slice(),
        &[DeleteAccountCall {
            user_id: USER_ID.into(),
            request_id: Some("delete-req".into()),
            ip_address: Some("203.0.113.9".into()),
            user_agent: Some("account-route-test".into()),
        }]
    );
}

#[tokio::test]
async fn account_deletion_status_returns_current_authenticated_users_saga() {
    let harness = RouteHarness::new().await;
    let app = auth_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::GET)
        .uri("/auth/account/deletion")
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .body(Body::empty())
        .expect("request");

    let response = app.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["userId"], USER_ID);
    assert_eq!(body["steps"][0]["target"], "PROFILE");
    assert_eq!(
        harness
            .clients
            .get_deletion_calls
            .lock()
            .expect("get deletion calls")
            .as_slice(),
        &[USER_ID.to_owned()]
    );
}

#[tokio::test]
async fn account_deletion_retry_requires_admin_and_forwards_audited_resolution() {
    const ADMIN_ID: &str = "44444444-4444-4444-4444-444444444444";
    const SAGA_ID: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
    const RESOLUTION_ID: &str = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";
    let harness = RouteHarness::new().await;
    let app = admin_routes::router(harness.state.clone());

    let forbidden = Request::builder()
        .method(Method::POST)
        .uri(format!("/admin/account-deletions/{SAGA_ID}/retry"))
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "resolutionId": RESOLUTION_ID,
                "reason": "Reviewed the provider outage and approved retry",
                "evidenceReference": "INC-2026-0912"
            })
            .to_string(),
        ))
        .expect("request");
    let response = app.clone().oneshot(forbidden).await.expect("response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(harness
        .clients
        .retry_user_deletion_calls
        .lock()
        .expect("retry calls")
        .is_empty());

    let authorized = Request::builder()
        .method(Method::POST)
        .uri(format!("/admin/account-deletions/{SAGA_ID}/retry"))
        .header(
            header::AUTHORIZATION,
            harness
                .bearer_token_with_roles(ADMIN_ID, vec!["ROLE_USER".into(), "ROLE_ADMIN".into()]),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "resolutionId": RESOLUTION_ID,
                "reason": "Reviewed the provider outage and approved retry",
                "evidenceReference": "INC-2026-0912"
            })
            .to_string(),
        ))
        .expect("request");
    let response = app.oneshot(authorized).await.expect("response");
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body = response_json(response).await;
    assert_eq!(body["accepted"], true);
    assert_eq!(body["resolutionId"], RESOLUTION_ID);
    assert_eq!(body["saga"]["id"], SAGA_ID);

    let calls = harness
        .clients
        .retry_user_deletion_calls
        .lock()
        .expect("retry calls");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].actor_id, ADMIN_ID);
    assert_eq!(calls[0].saga_id, SAGA_ID);
    assert_eq!(calls[0].resolution_id, RESOLUTION_ID);
    assert_eq!(calls[0].evidence_reference, "INC-2026-0912");
}

#[tokio::test]
async fn dead_lettered_account_deletion_list_requires_admin_and_uses_paired_cursor() {
    const ADMIN_ID: &str = "44444444-4444-4444-4444-444444444444";
    const SAGA_ID: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
    let harness = RouteHarness::new().await;
    let app = admin_routes::router(harness.state.clone());

    let forbidden = Request::builder()
        .method(Method::GET)
        .uri("/admin/account-deletions")
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .body(Body::empty())
        .expect("request");
    let response = app.clone().oneshot(forbidden).await.expect("response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let partial_cursor = Request::builder()
        .method(Method::GET)
        .uri("/admin/account-deletions?afterUpdatedAt=2026-08-04T12%3A00%3A00Z")
        .header(
            header::AUTHORIZATION,
            harness.bearer_token_with_roles(ADMIN_ID, vec!["ROLE_ADMIN".into()]),
        )
        .body(Body::empty())
        .expect("request");
    let response = app.clone().oneshot(partial_cursor).await.expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let excessive_limit = Request::builder()
        .method(Method::GET)
        .uri("/admin/account-deletions?limit=101")
        .header(
            header::AUTHORIZATION,
            harness.bearer_token_with_roles(ADMIN_ID, vec!["ROLE_ADMIN".into()]),
        )
        .body(Body::empty())
        .expect("request");
    let response = app
        .clone()
        .oneshot(excessive_limit)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(harness
        .clients
        .list_dead_lettered_deletion_calls
        .lock()
        .expect("list deletion calls")
        .is_empty());

    let authorized = Request::builder()
        .method(Method::GET)
        .uri("/admin/account-deletions?limit=5")
        .header(
            header::AUTHORIZATION,
            harness.bearer_token_with_roles(ADMIN_ID, vec!["ROLE_ADMIN".into()]),
        )
        .body(Body::empty())
        .expect("request");
    let response = app.oneshot(authorized).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["sagas"][0]["id"], SAGA_ID);
    assert_eq!(body["sagas"][0]["status"], "DEAD_LETTERED");
    assert_eq!(body["nextCursor"]["sagaId"], SAGA_ID);
    assert_eq!(
        harness
            .clients
            .list_dead_lettered_deletion_calls
            .lock()
            .expect("list deletion calls")
            .as_slice(),
        &[(5, None, None)]
    );
}

#[tokio::test]
async fn account_deletion_admin_detail_looks_up_the_requested_saga_id() {
    const ADMIN_ID: &str = "44444444-4444-4444-4444-444444444444";
    const SAGA_ID: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
    let harness = RouteHarness::new().await;
    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("/admin/account-deletions/{SAGA_ID}"))
        .header(
            header::AUTHORIZATION,
            harness.bearer_token_with_roles(ADMIN_ID, vec!["ROLE_ADMIN".into()]),
        )
        .body(Body::empty())
        .expect("request");

    let response = admin_routes::router(harness.state.clone())
        .oneshot(request)
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["id"], SAGA_ID);
    assert_eq!(
        harness
            .clients
            .get_deletion_by_saga_id_calls
            .lock()
            .expect("detail calls")
            .as_slice(),
        &[SAGA_ID.to_owned()]
    );
}

#[tokio::test]
async fn oauth_authorize_rejects_unsupported_provider() {
    let harness = RouteHarness::new().await;
    let app = oauth_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::GET)
        .uri("/auth/oauth/github/authorize")
        .body(Body::empty())
        .expect("request");

    let response = app.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert_eq!(body["code"], "GATEWAY_400");
    assert_eq!(body["message"], "Unsupported identity provider");
}

#[tokio::test]
async fn oauth_transport_stays_hidden_under_the_default_policy() {
    let harness = RouteHarness::new().await;
    let app = oauth_routes::router(harness.state.clone());
    for provider in ["google", "apple", "x"] {
        let request = Request::builder()
            .method(Method::GET)
            .uri(format!("/auth/oauth/{provider}/authorize"))
            .body(Body::empty())
            .expect("request");

        let response = app.clone().oneshot(request).await.expect("response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{provider}");
    }
}

#[tokio::test]
async fn oauth_authorize_accepts_x_provider_and_sets_x_state_cookie() {
    let harness = RouteHarness::with_oauth_transport().await;
    let app = oauth_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::GET)
        .uri("/auth/oauth/x/authorize")
        .body(Body::empty())
        .expect("request");

    let response = app.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::FOUND);
    let location = response
        .headers()
        .get(header::LOCATION)
        .expect("location")
        .to_str()
        .expect("location header");
    assert!(location.contains("state=x-state"));
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .expect("state cookie")
        .to_str()
        .expect("cookie header");
    assert!(cookie.starts_with("oauth_state_x=v1."), "{cookie}");
    assert!(cookie.contains("HttpOnly"));
}

#[tokio::test]
async fn oauth_authorize_and_callback_round_trip_each_provider_state_cookie() {
    for (provider, state_cookie, state_value) in [
        ("google", "oauth_state_google", "google-state"),
        ("apple", "oauth_state_apple", "apple-state"),
        ("x", "oauth_state_x", "x-state"),
    ] {
        let harness = RouteHarness::with_oauth_transport().await;
        let app = oauth_routes::router(harness.state.clone());
        let authorize_request = Request::builder()
            .method(Method::GET)
            .uri(format!("/auth/oauth/{provider}/authorize"))
            .body(Body::empty())
            .expect("authorize request");

        let authorize_response = app
            .clone()
            .oneshot(authorize_request)
            .await
            .expect("authorize response");

        assert_eq!(
            authorize_response.status(),
            StatusCode::FOUND,
            "provider {provider}"
        );
        let cookie = authorize_response
            .headers()
            .get(header::SET_COOKIE)
            .expect("state cookie")
            .to_str()
            .expect("cookie header")
            .to_owned();
        assert!(
            cookie.starts_with(&format!("{state_cookie}=v1.")),
            "provider {provider}"
        );
        let callback_request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/auth/oauth/{provider}/callback?code=provider-code&state={state_value}"
            ))
            .header(header::COOKIE, cookie)
            .body(Body::empty())
            .expect("callback request");

        let response = app
            .oneshot(callback_request)
            .await
            .expect("callback response");

        assert_eq!(
            response.status(),
            StatusCode::SEE_OTHER,
            "provider {provider}"
        );
        assert_eq!(
            response.headers()[header::LOCATION],
            "/auth/callback?status=success&returnTo=%2Fworld",
            "provider {provider}"
        );
        assert_eq!(
            harness
                .clients
                .oauth_callback_calls
                .lock()
                .expect("oauth callback calls")
                .len(),
            1,
            "provider {provider}"
        );
    }
}

#[tokio::test]
async fn google_callback_rejects_wrong_provider_state_cookies_without_downstream_call() {
    for wrong_cookie in ["oauth_state_apple", "oauth_state_x"] {
        let harness = RouteHarness::with_oauth_transport().await;
        let app = oauth_routes::router(harness.state.clone());
        let request = Request::builder()
            .method(Method::GET)
            .uri("/auth/oauth/google/callback?code=google-code&state=google-state")
            .header(header::COOKIE, format!("{wrong_cookie}=google-state"))
            .body(Body::empty())
            .expect("request");

        let response = app.oneshot(request).await.expect("response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response_json(response).await;
        assert_eq!(body["code"], "GATEWAY_400");
        assert_eq!(body["message"], "Invalid OAuth state");
        assert!(harness
            .clients
            .oauth_callback_calls
            .lock()
            .expect("oauth callback calls")
            .is_empty());
    }
}
