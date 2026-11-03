use axum::http::{Method, StatusCode};
use serde_json::json;

use crate::support::http::{header_contains, json_request, json_request_ok};

#[tokio::test]
async fn auth_routes_preserve_cookies_and_public_payload_shapes() {
    let app = crate::support::start_gateway_app(true).await.expect("app");

    let (_, signup) = json_request_ok(
        &app,
        Method::POST,
        "/auth/email/signup",
        None,
        &[],
        Some(json!({"email":"hana@example.com","password":"secret-passphrase"})),
    )
    .await;
    assert_eq!(signup["status"], "PENDING_VERIFICATION");

    let (_, verify) = json_request_ok(
        &app,
        Method::POST,
        "/auth/email/verify",
        None,
        &[],
        Some(json!({"email":"hana@example.com","code":"123456"})),
    )
    .await;
    assert_eq!(verify["status"], "ACTIVE");

    let (_, availability) = json_request_ok(
        &app,
        Method::POST,
        "/auth/email/check",
        None,
        &[],
        Some(json!({"email":"hana@example.com"})),
    )
    .await;
    assert_eq!(availability["available"], true);

    let (_, reset_requested) = json_request_ok(
        &app,
        Method::POST,
        "/auth/password/reset/request",
        None,
        &[],
        Some(json!({"email":"hana@example.com"})),
    )
    .await;
    assert_eq!(reset_requested["status"], "RESET_REQUESTED");

    let (_, reset_confirmed) = json_request_ok(
        &app,
        Method::POST,
        "/auth/password/reset/confirm",
        None,
        &[],
        Some(json!({"token":"reset-token","newPassword":"new-secret-passphrase"})),
    )
    .await;
    assert_eq!(reset_confirmed["status"], "RESET_CONFIRMED");

    let (login_status, login_headers, login_body) = json_request(
        &app,
        Method::POST,
        "/auth/login",
        None,
        &[
            ("x-forwarded-for", "203.0.113.11"),
            ("user-agent", "gateway-it"),
        ],
        Some(json!({"email":"hana@example.com","password":"secret-passphrase"})),
    )
    .await;
    assert_eq!(login_status, StatusCode::OK);
    assert_eq!(login_body["refreshToken"], "");
    assert!(header_contains(
        &login_headers,
        "set-cookie",
        "refresh_token=refresh-token"
    ));

    let (refresh_headers, refresh_body) = json_request_ok(
        &app,
        Method::POST,
        "/auth/refresh",
        None,
        &[("cookie", "refresh_token=refresh-token")],
        None,
    )
    .await;
    assert_eq!(refresh_body["accessToken"], "access.token");
    assert!(header_contains(
        &refresh_headers,
        "set-cookie",
        "refresh_token=refresh-token"
    ));

    let (logout_status, logout_headers, _) = json_request(
        &app,
        Method::POST,
        "/auth/logout",
        None,
        &[("cookie", "refresh_token=refresh-token")],
        None,
    )
    .await;
    assert_eq!(logout_status, StatusCode::NO_CONTENT);
    assert!(header_contains(&logout_headers, "set-cookie", "Max-Age=0"));
    app.shutdown().await.expect("app shutdown");
}
