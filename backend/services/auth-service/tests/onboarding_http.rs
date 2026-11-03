#[path = "support/mod.rs"]
mod support;

use auth_service::adapters::out::persistence::user_repository::PgUserRepository;
use auth_service::application::auth_transactions::AuthTransactionsPort;
use auth_service::domain::user::{User, UserStatus};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn auth_http_resend_returns_same_acceptance_for_pending_and_unknown_accounts(
) -> anyhow::Result<()> {
    let app = support::start_postgres_app(false).await?;
    let users = PgUserRepository::new(
        app.pool.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    let pending = User {
        id: Uuid::new_v4(),
        primary_email: "resend-http@example.invalid".into(),
        password_hash: "test-hash".into(),
        status: UserStatus::PendingVerification,
        last_login_at: None,
    };
    users.register_email(&(&pending).into(), "123456").await?;
    for email in ["RESEND-HTTP@example.invalid", "unknown@example.invalid"] {
        let request = Request::builder()
            .method("POST")
            .uri("/auth/email/resend")
            .header("content-type", "application/json")
            .body(Body::from(json!({"email": email}).to_string()))?;
        let (status, body, headers) = support::json_response(app.router.clone(), request).await?;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(
            headers.get("cache-control").expect("cache control"),
            "no-store"
        );
        assert_eq!(body, json!({"status": "ACCEPTED"}));
    }
    let request = Request::builder()
        .method("POST")
        .uri("/auth/email/resend")
        .header("content-type", "application/json")
        .body(Body::from(json!({"email": "malformed"}).to_string()))?;
    let (status, body, _) = support::json_response(app.router.clone(), request).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["message"], "Invalid email");
    app.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn database_failure_is_not_reported_as_email_acceptance() -> anyhow::Result<()> {
    let app = support::start_postgres_app(false).await?;
    app.pool.close().await;
    let request = Request::builder()
        .method("POST")
        .uri("/auth/email/resend")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"email": "someone@example.invalid"}).to_string(),
        ))?;
    let (status, body, _) = support::json_response(app.router.clone(), request).await?;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["message"], "Internal error");
    Ok(())
}
