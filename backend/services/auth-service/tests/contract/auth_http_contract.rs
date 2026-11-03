use axum::body::Body;
use axum::http::{Method, Request, StatusCode};

#[tokio::test]
async fn login_contract_matches_expected_shape() {
    let app = crate::support::start_postgres_app(true).await.expect("app");
    let request = Request::builder()
        .method(Method::POST)
        .uri("/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"email":"hana@example.com","password":"Passw0rd!"}"#,
        ))
        .expect("request");

    let (status, body, _) = crate::support::json_response(app.router.clone(), request)
        .await
        .expect("response");

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["userId"], "11111111-1111-1111-1111-111111111111");
    assert_eq!(body["tokenType"], "Bearer");
    assert!(body["accessToken"].as_str().is_some());
    assert!(body["refreshToken"].as_str().is_some());
}

#[tokio::test]
async fn signup_contract_returns_pending_verification() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");
    let request = Request::builder()
        .method(Method::POST)
        .uri("/auth/email/signup")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"email":"parity@example.com","password":"Passw0rd!"}"#,
        ))
        .expect("request");

    let (status, body, _) = crate::support::json_response(app.router, request)
        .await
        .expect("response");
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "PENDING_VERIFICATION");
}

#[tokio::test]
async fn development_login_persistence_failure_is_server_error_without_database_detail() {
    let app = crate::support::start_postgres_app(true)
        .await
        .expect("isolated app");
    sqlx::query("drop table users cascade")
        .execute(&app.pool)
        .await
        .expect("remove users in isolated fixture");
    let repository =
        auth_service::adapters::out::persistence::user_repository::PgUserRepository::new(
            app.pool.clone(),
            std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
        );
    let failure = auth_service::application::ports::UserRepositoryPort::find_by_email(
        &repository,
        "hana@example.com",
    )
    .await
    .expect_err("actual SQL failure must cross the typed port");
    assert_eq!(
        failure,
        auth_service::application::errors::AuthError::Unexpected
    );
    assert!(std::error::Error::source(&failure).is_none());
    assert!(!format!("{failure:?}").contains("users"));
    let request = Request::builder()
        .method(Method::POST)
        .uri("/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"email":"hana@example.com","password":"Passw0rd!"}"#,
        ))
        .unwrap();
    let (status, body, _) = crate::support::json_response(app.router, request)
        .await
        .expect("response");
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        body,
        serde_json::json!({"code":"INTERNAL_ERROR","message":"Internal error"})
    );
    assert!(!body.to_string().contains("users"));
    assert!(!body.to_string().contains("hana@example.com"));
}
