//! Actual Auth handler and PostgreSQL tests. These require Docker and never skip.
#[path = "support/mod.rs"]
mod support;

use auth_service::adapters::out::persistence::user_deletion_saga_repository::PgUserDeletionSagaRepository;
use auth_service::adapters::out::persistence::user_repository::PgUserRepository;
use auth_service::adapters::r#in::grpc::server::{proto, AuthGrpcServer};
use auth_service::application::ports::{UserDeletionSagaPort, UserRepositoryPort};
use auth_service::domain::user::{User, UserStatus};
use auth_service::domain::user_deletion::{UserDeletionSagaStatus, UserDeletionTarget};
use chrono::Utc;
use platform::grpc_auth::{VerifiedCaller, Workload};
use proto::auth_service_server::AuthService;
use tonic::{Code, Request};
use uuid::Uuid;

async fn resend(
    server: &AuthGrpcServer,
    email: &str,
) -> Result<tonic::Response<proto::ResendVerificationResponse>, tonic::Status> {
    server
        .resend_verification(Request::new(proto::ResendVerificationRequest {
            email: email.into(),
        }))
        .await
}

#[tokio::test]
async fn account_states_and_quotas_have_identical_rpc_success_payloads() -> anyhow::Result<()> {
    let app = support::start_postgres_app(false).await?;
    let server = AuthGrpcServer::new(app.state.clone());
    let email = "grpc-resend@example.invalid";
    let user = app
        .state
        .signup
        .signup_email(email, "test-password-123")
        .await?;
    let before: i64 = sqlx::query_scalar("select count(*) from auth_mail_outbox")
        .fetch_one(&app.pool)
        .await?;
    for value in [
        email,
        "GRPC-RESEND@EXAMPLE.INVALID",
        "unknown@example.invalid",
    ] {
        let response = resend(&server, value).await?;
        assert_eq!(response.get_ref().status, "ACCEPTED");
        assert_eq!(
            response.metadata().get("cache-control").unwrap().to_str()?,
            "no-store"
        );
    }
    let after: i64 = sqlx::query_scalar("select count(*) from auth_mail_outbox")
        .fetch_one(&app.pool)
        .await?;
    assert_eq!(
        before, after,
        "initial signup is included in the resend cooldown"
    );
    let code: String =
        sqlx::query_scalar("select code from email_verifications where user_id = $1")
            .bind(user.user_id)
            .fetch_one(&app.pool)
            .await?;
    app.state.signup.verify_email(email, &code).await?;
    assert_eq!(
        resend(&server, email).await?.into_inner().status,
        "ACCEPTED"
    );
    let users = PgUserRepository::new(
        app.pool.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    users
        .soft_delete_user(user.user_id, user.user_id, "rpc test", Utc::now())
        .await?;
    assert_eq!(
        resend(&server, email).await?.into_inner().status,
        "ACCEPTED"
    );
    app.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn resend_rpc_rotates_the_challenge_and_enqueues_one_delivery() -> anyhow::Result<()> {
    let app = support::start_postgres_app(false).await?;
    let server = AuthGrpcServer::new(app.state.clone());
    let email = "grpc-rotation@example.invalid";
    let user = app
        .state
        .signup
        .signup_email(email, "test-password-123")
        .await?;
    let old_id: uuid::Uuid =
        sqlx::query_scalar("select id from email_verifications where user_id = $1")
            .bind(user.user_id)
            .fetch_one(&app.pool)
            .await?;
    sqlx::query(
        "update auth_mail_outbox set created_at = now() - interval '2 minutes' where user_id = $1",
    )
    .bind(user.user_id)
    .execute(&app.pool)
    .await?;
    assert_eq!(
        resend(&server, email).await?.into_inner().status,
        "ACCEPTED"
    );
    let new_id: uuid::Uuid =
        sqlx::query_scalar("select id from email_verifications where user_id = $1")
            .bind(user.user_id)
            .fetch_one(&app.pool)
            .await?;
    assert_ne!(old_id, new_id);
    let pending: i64 = sqlx::query_scalar("select count(*) from auth_mail_outbox where user_id = $1 and delivered_at is null and dead_lettered_at is null")
        .bind(user.user_id).fetch_one(&app.pool).await?;
    let scrubbed: i64 = sqlx::query_scalar("select count(*) from auth_mail_outbox where user_id = $1 and dead_lettered_at is not null and payload_json is null")
        .bind(user.user_id).fetch_one(&app.pool).await?;
    assert_eq!((pending, scrubbed), (1, 1));
    app.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn invalid_input_and_database_failure_have_distinct_sanitized_codes() -> anyhow::Result<()> {
    let app = support::start_postgres_app(false).await?;
    let server = AuthGrpcServer::new(app.state.clone());
    let email = "grpc-failure@example.invalid";
    let user = app
        .state
        .signup
        .signup_email(email, "test-password-123")
        .await?;
    sqlx::query(
        "update auth_mail_outbox set created_at = now() - interval '2 minutes' where user_id = $1",
    )
    .bind(user.user_id)
    .execute(&app.pool)
    .await?;
    sqlx::raw_sql("create function fail_rpc_resend() returns trigger language plpgsql as $$ begin raise exception 'PRIVATE_DATABASE_DETAIL'; end $$; create trigger fail_rpc_resend before insert on auth_mail_outbox for each row execute function fail_rpc_resend();")
        .execute(&app.pool).await?;
    let error = resend(&server, email).await.unwrap_err();
    assert_eq!(error.code(), Code::Internal);
    assert_eq!(error.message(), "Verification request failed");
    assert!(!format!("{error:?}").contains("PRIVATE_DATABASE_DETAIL"));
    let codes: i64 =
        sqlx::query_scalar("select count(*) from email_verifications where user_id = $1")
            .bind(user.user_id)
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(
        codes, 1,
        "the existing challenge survives failed resend persistence"
    );
    app.pool.close().await;
    // Validation precedes DB access, even when persistence is unavailable.
    let error = resend(&server, "malformed").await.unwrap_err();
    assert_eq!(error.code(), Code::InvalidArgument);
    assert_eq!(error.message(), "Invalid email");
    Ok(())
}

#[tokio::test]
async fn deletion_retry_rpc_requires_gateway_and_returns_the_committed_saga() -> anyhow::Result<()>
{
    let app = support::start_postgres_app(false).await?;
    let server = AuthGrpcServer::new(app.state.clone());
    let user_id = Uuid::new_v4();
    let admin_id = Uuid::new_v4();
    PgUserRepository::new(
        app.pool.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    )
    .save(
        &(&User {
            id: user_id,
            primary_email: "grpc-deletion-retry@example.invalid".into(),
            password_hash: "stored-hash".into(),
            status: UserStatus::Active,
            last_login_at: None,
        })
            .into(),
    )
    .await?;

    let mut delete_request = Request::new(proto::DeleteUserRequest {
        user_id: user_id.to_string(),
        actor_id: user_id.to_string(),
        request_id: "grpc-delete-retry-test".into(),
        ip_address: String::new(),
        user_agent: String::new(),
    });
    delete_request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Gateway));
    let deletion = server.delete_user(delete_request).await?.into_inner();
    let saga_id = Uuid::parse_str(&deletion.saga_id)?;
    let repository = PgUserDeletionSagaRepository::new(app.pool.clone());
    let now = Utc::now() + chrono::Duration::seconds(1);
    let claims = repository
        .claim_due(now, chrono::Duration::minutes(5), 10)
        .await?
        .into_iter()
        .map(auth_service::domain::user_deletion::UserDeletionStep::from)
        .collect::<Vec<_>>();
    let dead_letter_step = claims
        .iter()
        .find(|step| step.saga_id == saga_id && step.target == UserDeletionTarget::Profile)
        .expect("profile deletion claim");
    repository
        .mark_step_failed(
            &(dead_letter_step).into(),
            "profile cleanup unavailable",
            now + chrono::Duration::seconds(30),
            true,
            now,
        )
        .await?;

    let denied_list = server
        .list_dead_lettered_user_deletions({
            let mut request = Request::new(proto::ListDeadLetteredUserDeletionsRequest {
                limit: 20,
                after_updated_at: String::new(),
                after_saga_id: String::new(),
            });
            request
                .extensions_mut()
                .insert(VerifiedCaller(Workload::Post));
            request
        })
        .await
        .unwrap_err();
    assert_eq!(denied_list.code(), Code::PermissionDenied);

    let mut list_request = Request::new(proto::ListDeadLetteredUserDeletionsRequest {
        limit: 20,
        after_updated_at: String::new(),
        after_saga_id: String::new(),
    });
    list_request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Gateway));
    let listed = server
        .list_dead_lettered_user_deletions(list_request)
        .await?
        .into_inner();
    assert_eq!(listed.sagas.len(), 1);
    assert_eq!(listed.sagas[0].id, saga_id.to_string());
    assert!(!listed.has_more);
    assert!(listed.next_updated_at.is_empty());
    assert!(listed.next_saga_id.is_empty());

    let mut denied_detail = Request::new(proto::GetUserDeletionBySagaIdRequest {
        saga_id: saga_id.to_string(),
    });
    denied_detail
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Post));
    assert_eq!(
        server
            .get_user_deletion_by_saga_id(denied_detail)
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );

    let mut detail_request = Request::new(proto::GetUserDeletionBySagaIdRequest {
        saga_id: saga_id.to_string(),
    });
    detail_request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Gateway));
    let detail = server
        .get_user_deletion_by_saga_id(detail_request)
        .await?
        .into_inner();
    assert!(detail.found);
    assert_eq!(detail.saga.expect("saga detail").id, saga_id.to_string());

    let mut incomplete_cursor = Request::new(proto::ListDeadLetteredUserDeletionsRequest {
        limit: 20,
        after_updated_at: now.to_rfc3339(),
        after_saga_id: String::new(),
    });
    incomplete_cursor
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Gateway));
    assert_eq!(
        server
            .list_dead_lettered_user_deletions(incomplete_cursor)
            .await
            .unwrap_err()
            .code(),
        Code::InvalidArgument
    );
    let mut invalid_timestamp = Request::new(proto::ListDeadLetteredUserDeletionsRequest {
        limit: 20,
        after_updated_at: "not-rfc3339".into(),
        after_saga_id: saga_id.to_string(),
    });
    invalid_timestamp
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Gateway));
    assert_eq!(
        server
            .list_dead_lettered_user_deletions(invalid_timestamp)
            .await
            .unwrap_err()
            .code(),
        Code::InvalidArgument
    );

    let resolution_id = Uuid::new_v4();
    let request = || {
        let mut request = Request::new(proto::RetryUserDeletionRequest {
            saga_id: saga_id.to_string(),
            actor_id: admin_id.to_string(),
            resolution_id: resolution_id.to_string(),
            reason: "Reviewed the provider incident and approved a retry".into(),
            evidence_reference: "INC-2026-0912".into(),
        });
        request
            .extensions_mut()
            .insert(VerifiedCaller(Workload::Post));
        request
    };
    let denied = server.retry_user_deletion(request()).await.unwrap_err();
    assert_eq!(denied.code(), Code::PermissionDenied);
    let before_authorized_call: i64 = sqlx::query_scalar(
        "select count(*) from user_deletion_retry_resolutions where resolution_id = $1",
    )
    .bind(resolution_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(before_authorized_call, 0);

    let mut authorized = request();
    authorized
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Gateway));
    let accepted = server.retry_user_deletion(authorized).await?.into_inner();
    assert!(accepted.accepted);
    assert_eq!(accepted.resolution_id, resolution_id.to_string());
    let saga = accepted.saga.expect("saga result");
    assert_eq!(saga.id, saga_id.to_string());
    assert_eq!(saga.status, UserDeletionSagaStatus::InProgress.as_str());
    app.pool.close().await;
    Ok(())
}
