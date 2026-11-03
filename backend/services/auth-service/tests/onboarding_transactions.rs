//! Real PostgreSQL acceptance tests. A Docker daemon is required; no silent skip.
#[path = "support/owned_database.rs"]
mod owned_database;

use auth_service::adapters::out::persistence::user_repository::PgUserRepository;
use auth_service::adapters::out::security::admitted_argon2::AdmittedArgon2PasswordWork;
use auth_service::application::auth_transactions::AuthTransactionsPort;
use auth_service::application::onboarding::AuthOnboardingPort;
use auth_service::application::password_work::PasswordWorkLimits;
use auth_service::application::ports::UserRepositoryPort;
use auth_service::application::use_cases::signup::SignupUseCase;
use auth_service::domain::oauth_identity::OAuthUserInfo;
use auth_service::domain::user::{IdentityProvider, User, UserStatus};
use chrono::{Duration, Utc};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::path::PathBuf;
use std::sync::Arc;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;

struct Fixture {
    _postgres: Option<ContainerAsync<Postgres>>,
    pool: PgPool,
    users: PgUserRepository,
}

impl Fixture {
    async fn new() -> anyhow::Result<Self> {
        let (postgres, database_url) =
            if let Some(url) = owned_database::native_database_url().await? {
                (None, url)
            } else {
                let (postgres, database_url) =
                    test_support::postgres::start_postgres_with_url().await?;
                (Some(postgres), database_url)
            };
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&database_url)
            .await?;
        let migrations = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
        sqlx::migrate::Migrator::new(migrations.as_path())
            .await?
            .run(&pool)
            .await?;
        let users = PgUserRepository::new(
            pool.clone(),
            std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
        );
        Ok(Self {
            _postgres: postgres,
            pool,
            users,
        })
    }

    async fn pending(&self, email: &str) -> anyhow::Result<User> {
        let user = User {
            id: Uuid::new_v4(),
            primary_email: email.into(),
            password_hash: "test-hash".into(),
            status: UserStatus::PendingVerification,
            last_login_at: None,
        };
        self.users.register_email(&(&user).into(), "123456").await?;
        Ok(user)
    }

    async fn allow_resend(&self, user_id: Uuid) -> anyhow::Result<()> {
        sqlx::query("update auth_mail_outbox set created_at = clock_timestamp() - interval '2 minutes' where user_id = $1")
            .bind(user_id).execute(&self.pool).await?;
        Ok(())
    }

    async fn challenge(&self, user_id: Uuid) -> anyhow::Result<String> {
        Ok(
            sqlx::query_scalar("select code from email_verifications where user_id = $1")
                .bind(user_id)
                .fetch_one(&self.pool)
                .await?,
        )
    }

    async fn mail_count(&self, user_id: Uuid) -> anyhow::Result<i64> {
        Ok(
            sqlx::query_scalar("select count(*) from auth_mail_outbox where user_id = $1")
                .bind(user_id)
                .fetch_one(&self.pool)
                .await?,
        )
    }
}

fn proof(provider: IdentityProvider, subject: &str, email: &str) -> OAuthUserInfo {
    OAuthUserInfo {
        provider,
        subject: subject.into(),
        email: email.into(),
        email_verified: true,
    }
}

#[tokio::test]
async fn oauth_audit_failure_rolls_back_user_and_identity() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    sqlx::raw_sql("create function reject_oauth_audit() returns trigger language plpgsql as $$ begin if new.action = 'OAUTH_ACCOUNT_CREATED' then raise exception 'injected audit failure'; end if; return new; end $$; create trigger reject_oauth_audit before insert on auth_audit_events for each row execute function reject_oauth_audit();")
        .execute(&f.pool).await?;
    let identity = proof(
        IdentityProvider::Google,
        "subject-rollback",
        "rollback@example.invalid",
    );
    assert!(f
        .users
        .resolve_oauth_user(&(&identity).into())
        .await
        .is_err());
    let counts: (i64, i64) = sqlx::query_as(
        "select (select count(*) from users), (select count(*) from user_identities)",
    )
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(counts, (0, 0));
    sqlx::raw_sql(
        "drop trigger reject_oauth_audit on auth_audit_events; drop function reject_oauth_audit();",
    )
    .execute(&f.pool)
    .await?;
    let user = f.users.resolve_oauth_user(&(&identity).into()).await?;
    assert_eq!(user.status, UserStatus::Active.into());
    let audits: i64 = sqlx::query_scalar("select count(*) from auth_audit_events where subject_user_id = $1 and action = 'OAUTH_ACCOUNT_CREATED'")
        .bind(user.id).fetch_one(&f.pool).await?;
    assert_eq!(audits, 1);
    f.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn simultaneous_first_callbacks_share_one_identity() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let identity = proof(
        IdentityProvider::Google,
        "subject-concurrent",
        "parallel@example.invalid",
    );
    let proof = auth_service::application::dto::repository_dto::AuthOAuthProofDto::from(&identity);
    let (first, second) = tokio::join!(
        f.users.resolve_oauth_user(&proof),
        f.users.resolve_oauth_user(&proof),
    );
    assert_eq!(first?.id, second?.id);
    let counts: (i64, i64) = sqlx::query_as(
        "select (select count(*) from users), (select count(*) from user_identities)",
    )
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(counts, (1, 1));
    f.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn concurrent_providers_with_one_verified_email_share_one_user() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let google = proof(
        IdentityProvider::Google,
        "google-subject",
        "shared@example.invalid",
    );
    let apple = proof(
        IdentityProvider::Apple,
        "apple-subject",
        "SHARED@example.invalid",
    );
    let google_proof =
        auth_service::application::dto::repository_dto::AuthOAuthProofDto::from(&google);
    let apple_proof =
        auth_service::application::dto::repository_dto::AuthOAuthProofDto::from(&apple);
    let (first, second) = tokio::join!(
        f.users.resolve_oauth_user(&google_proof),
        f.users.resolve_oauth_user(&apple_proof),
    );
    assert_eq!(first?.id, second?.id);
    let counts: (i64, i64) = sqlx::query_as(
        "select (select count(*) from users), (select count(*) from user_identities)",
    )
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(counts, (1, 2));
    f.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn existing_subject_does_not_depend_on_a_changed_email_claim() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let mut identity = proof(
        IdentityProvider::Google,
        "stable-subject",
        "original@example.invalid",
    );
    let user = f.users.resolve_oauth_user(&(&identity).into()).await?;
    identity.email.clear();
    identity.email_verified = false;
    let same = f.users.resolve_oauth_user(&(&identity).into()).await?;
    assert_eq!(same.id, user.id);
    assert_eq!(same.primary_email, "original@example.invalid");
    assert!(
        f.users
            .soft_delete_user(user.id, user.id, "test", Utc::now() + Duration::days(1))
            .await?
    );
    assert!(f
        .users
        .resolve_oauth_user(&(&identity).into())
        .await
        .is_err());
    assert_eq!(
        f.users
            .find_by_id(user.id)
            .await?
            .map(auth_service::domain::user::User::from)
            .expect("user")
            .status,
        UserStatus::Deleted
    );
    f.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn unverified_or_empty_first_email_leaves_no_pending_oauth_user() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let mut identity = proof(
        IdentityProvider::Google,
        "unverified-subject",
        "unverified@example.invalid",
    );
    identity.email_verified = false;
    assert!(f
        .users
        .resolve_oauth_user(&(&identity).into())
        .await
        .is_err());
    identity.email_verified = true;
    identity.email.clear();
    assert!(f
        .users
        .resolve_oauth_user(&(&identity).into())
        .await
        .is_err());
    identity.subject.clear();
    assert!(f
        .users
        .resolve_oauth_user(&(&identity).into())
        .await
        .is_err());
    let users: i64 = sqlx::query_scalar("select count(*) from users")
        .fetch_one(&f.pool)
        .await?;
    assert_eq!(users, 0);
    f.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn matching_primary_email_is_not_enough_to_link_an_account() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let pending = f.pending("pending@example.invalid").await?;
    let identity = proof(
        IdentityProvider::Google,
        "new-subject",
        &pending.primary_email,
    );
    assert!(f
        .users
        .resolve_oauth_user(&(&identity).into())
        .await
        .is_err());
    assert_eq!(
        f.users
            .find_by_id(pending.id)
            .await?
            .map(auth_service::domain::user::User::from)
            .expect("user")
            .status,
        UserStatus::PendingVerification
    );
    // Even an active row is insufficient without a verified live identity.
    let active = User {
        id: Uuid::new_v4(),
        primary_email: "bare@example.invalid".into(),
        status: UserStatus::Active,
        ..pending.clone()
    };
    f.users.save(&(&active).into()).await?;
    assert!(f
        .users
        .resolve_oauth_user(
            &(&proof(
                IdentityProvider::Apple,
                "bare-subject",
                &active.primary_email
            ))
                .into()
        )
        .await
        .is_err());
    assert_eq!(
        f.users
            .verify_email(&pending.primary_email, "123456")
            .await?,
        Some(pending.id)
    );
    assert_eq!(
        f.users.resolve_oauth_user(&(&identity).into()).await?.id,
        pending.id
    );
    f.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn resend_rotates_challenge_and_cancels_superseded_delivery() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let pending = f.pending("resend@example.invalid").await?;
    f.users
        .resend_verification(&pending.primary_email, "234567")
        .await?;
    assert_eq!(f.challenge(pending.id).await?, "123456");
    assert_eq!(f.mail_count(pending.id).await?, 1);
    f.allow_resend(pending.id).await?;
    f.users
        .resend_verification(&pending.primary_email, "234567")
        .await?;
    assert_eq!(f.challenge(pending.id).await?, "234567");
    let scrubbed: i64 = sqlx::query_scalar("select count(*) from auth_mail_outbox where user_id = $1 and last_error_code = 'superseded' and payload_json is null and dead_lettered_at is not null")
        .bind(pending.id).fetch_one(&f.pool).await?;
    assert_eq!(scrubbed, 1);
    assert_eq!(
        f.users
            .verify_email(&pending.primary_email, "123456")
            .await?,
        None
    );
    assert_eq!(
        f.users
            .verify_email(&pending.primary_email, "234567")
            .await?,
        Some(pending.id)
    );
    f.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn concurrent_resends_only_admit_one_new_delivery() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let pending = f.pending("resend-concurrent@example.invalid").await?;
    f.allow_resend(pending.id).await?;
    let (first, second) = tokio::join!(
        f.users
            .resend_verification(&pending.primary_email, "234567"),
        f.users
            .resend_verification(&pending.primary_email, "345678"),
    );
    first?;
    second?;
    assert_eq!(f.mail_count(pending.id).await?, 2);
    assert!(["234567", "345678"].contains(&f.challenge(pending.id).await?.as_str()));
    let audits: i64 = sqlx::query_scalar("select count(*) from auth_audit_events where subject_user_id = $1 and action = 'EMAIL_VERIFICATION_RESENT'")
        .bind(pending.id).fetch_one(&f.pool).await?;
    assert_eq!(audits, 1);
    f.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn resend_audit_failure_restores_original_challenge_and_delivery() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let pending = f.pending("resend-rollback@example.invalid").await?;
    f.allow_resend(pending.id).await?;
    sqlx::raw_sql("create function reject_resend_audit() returns trigger language plpgsql as $$ begin if new.action = 'EMAIL_VERIFICATION_RESENT' then raise exception 'injected audit failure'; end if; return new; end $$; create trigger reject_resend_audit before insert on auth_audit_events for each row execute function reject_resend_audit();")
        .execute(&f.pool).await?;
    assert!(f
        .users
        .resend_verification(&pending.primary_email, "234567")
        .await
        .is_err());
    assert_eq!(f.challenge(pending.id).await?, "123456");
    assert_eq!(f.mail_count(pending.id).await?, 1);
    let live: bool = sqlx::query_scalar("select payload_json is not null and dead_lettered_at is null from auth_mail_outbox where user_id = $1")
        .bind(pending.id).fetch_one(&f.pool).await?;
    assert!(live);
    f.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn resend_daily_limit_and_public_result_do_not_reveal_admission() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let pending = f.pending("resend-daily@example.invalid").await?;
    for _ in 0..19 {
        sqlx::query("insert into auth_mail_outbox (id, user_id, kind, created_at, expires_at, payload_json, delivered_at) values ($1, $2, 'VERIFY_EMAIL', now() - interval '2 hours', now() + interval '1 hour', null, now())")
            .bind(Uuid::new_v4()).bind(pending.id).execute(&f.pool).await?;
    }
    f.allow_resend(pending.id).await?;
    let signup = SignupUseCase::new(
        f.users.clone(),
        Arc::new(f.users.clone()),
        Arc::new(AdmittedArgon2PasswordWork::new(
            PasswordWorkLimits::default(),
        )?),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
        std::sync::Arc::new(
            auth_service::adapters::out::security::email_verification::SecureEmailVerification,
        ),
    );
    let limited = signup.resend_verification(&pending.primary_email).await?;
    let unknown = signup
        .resend_verification("unknown@example.invalid")
        .await?;
    assert_eq!(limited, unknown);
    assert_eq!(limited.status, "ACCEPTED");
    assert_eq!(f.mail_count(pending.id).await?, 20);
    assert_eq!(f.challenge(pending.id).await?, "123456");
    assert!(signup.resend_verification("malformed").await.is_err());
    f.pool.close().await;
    Ok(())
}
