use axum::body::Body;
use axum::http::{Method, Request, StatusCode};

#[tokio::test]
async fn login_parity_fixture_matches_seeded_user_shape() {
    let app = crate::support::start_postgres_app(true).await.expect("app");
    let fixture: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../scripts/parity/auth-service/login-signup.json"),
        )
        .expect("fixture"),
    )
    .expect("json");

    let request = Request::builder()
        .method(Method::POST)
        .uri("/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"email":"hana@example.com","password":"Passw0rd!"}"#,
        ))
        .expect("request");
    let (status, body, _) = crate::support::json_response(app.router, request)
        .await
        .expect("response");

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["userId"], fixture["login"]["body"]["userId"]);
    assert_eq!(body["tokenType"], fixture["login"]["body"]["tokenType"]);
    assert_eq!(
        body["linkedProviders"],
        fixture["login"]["body"]["linkedProviders"]
    );
}
