use api_gateway::config::RuntimeMode;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn shadow_login_returns_none_diff_for_matching_fixture() {
    let app = crate::support::start_gateway_app_with_runtime_mode(true, RuntimeMode::Parity)
        .await
        .expect("app");

    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/shadow/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"hana@example.com","password":"Passw0rd!"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");

    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["diffCategory"], "none");
    app.shutdown().await.expect("app shutdown");
}
