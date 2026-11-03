use auth_service::adapters::r#in::grpc::server::proto;
use tonic::Request;

#[tokio::test]
async fn grpc_login_contract_matches_proto_shape() {
    let app = crate::support::start_postgres_app(true).await.expect("app");
    let grpc = crate::support::grpc_server(app.state);

    let response = proto::auth_service_server::AuthService::login_email(
        &grpc,
        Request::new(proto::LoginEmailRequest {
            email: "hana@example.com".into(),
            password: "Passw0rd!".into(),
            ip_address: "127.0.0.1".into(),
            user_agent: "grpc-test".into(),
        }),
    )
    .await
    .expect("grpc login")
    .into_inner();

    assert_eq!(response.user_id, "11111111-1111-1111-1111-111111111111");
    assert_eq!(response.token_type, "Bearer");
    assert!(!response.access_token.is_empty());
    assert!(!response.refresh_token.is_empty());
}

fn confirm_request(token: &str) -> Request<proto::ConfirmPasswordResetRequest> {
    Request::new(proto::ConfirmPasswordResetRequest {
        token: token.into(),
        new_password: "NewPassw0rd!".into(),
    })
}

fn login_request() -> Request<proto::LoginEmailRequest> {
    Request::new(proto::LoginEmailRequest {
        email: "hana@example.com".into(),
        password: "Passw0rd!".into(),
        ip_address: "127.0.0.1".into(),
        user_agent: "grpc-test".into(),
    })
}

#[tokio::test]
async fn grpc_reset_confirmation_rejects_unknown_tokens_before_any_hashing() {
    use base64ct::{Base64UrlUnpadded, Encoding};
    use proto::auth_service_server::AuthService;
    use sha2::{Digest, Sha256};

    let app = crate::support::start_postgres_app(true).await.expect("app");
    let grpc = crate::support::grpc_server(app.state.clone());

    // When: guessed tokens are confirmed over the RPC surface.
    for token in ["not-a-real-token", "", "another-guess"] {
        let status = AuthService::confirm_password_reset(&grpc, confirm_request(token))
            .await
            .expect_err("an unknown token must be rejected");
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    // Then: the reset class never admitted a hash. This is the production
    // metric, so an operator sees the same evidence the test relies on.
    let metrics = app.metrics().await.expect("metrics");
    assert!(
        metrics.contains("auth_password_work_admitted_total{class=\"reset\"} 0\n"),
        "{metrics}"
    );

    // And: the same oracle observes a hash when a real token is confirmed, so
    // the zero above is not an artifact of a metric that never moves.
    let token = "grpc-boundary-reset-token";
    let token_hash = Base64UrlUnpadded::encode_string(Sha256::digest(token.as_bytes()).as_slice());
    sqlx::query(
        "insert into password_reset_tokens (id, user_id, token_hash, expires_at)
         values ($1, $2, $3, now() + interval '1 hour')",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("seeded user"))
    .bind(&token_hash)
    .execute(&app.pool)
    .await
    .expect("insert reset token");
    AuthService::confirm_password_reset(&grpc, confirm_request(token))
        .await
        .expect("a real token resets the password");
    let metrics = app.metrics().await.expect("metrics");
    assert!(
        metrics.contains("auth_password_work_admitted_total{class=\"reset\"} 1\n"),
        "{metrics}"
    );
}

#[tokio::test]
async fn grpc_login_reports_saturation_as_busy_never_as_bad_credentials() {
    use auth_service::application::password_work::PasswordWorkKind;
    use proto::auth_service_server::AuthService;

    let password_work = crate::support::small_password_work(0);
    let app = crate::support::start_postgres_app_with_password_work(true, password_work.clone())
        .await
        .expect("app");
    let grpc = crate::support::grpc_server(app.state.clone());

    // Given: every login slot is busy.
    let mut slots = Vec::new();
    for _ in 0..3 {
        slots.push(crate::support::occupy(password_work.admission(PasswordWorkKind::Login)).await);
    }

    // When: a correct password arrives. Then: the RPC answers unavailable with
    // the busy code, which the Gateway turns into 503 with a retry hint.
    let status = AuthService::login_email(&grpc, login_request())
        .await
        .expect_err("no login capacity is left");
    assert_eq!(status.code(), tonic::Code::Unavailable);
    assert_eq!(status.message(), "AUTH_BUSY");
    let metrics = app.metrics().await.expect("metrics");
    assert!(
        metrics.contains("auth_password_work_rejected_total{class=\"login\"} 1\n"),
        "{metrics}"
    );

    for slot in slots {
        slot.release().await;
    }
    AuthService::login_email(&grpc, login_request())
        .await
        .expect("login succeeds once capacity returns");
}
