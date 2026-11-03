//! Acceptance regressions require real PostgreSQL; no in-memory persistence.

#[path = "support/owned_database.rs"]
mod owned_database;

use anyhow::Context;
use auth_service::adapters::out::cache::session_store::PgRefreshTokenStore;
use auth_service::adapters::out::persistence::user_repository::PgUserRepository;
use auth_service::adapters::r#in::messaging::mail_outbox_worker::{
    EmailOutboxWorker, MailDestination,
};
use auth_service::application::auth_transactions::AuthTransactionsPort;
use auth_service::application::ports::{RefreshTokenStorePort, UserRepositoryPort};
use auth_service::domain::password_reset::PasswordResetToken;
use auth_service::domain::session::{LoginAudit, RefreshTokenRecord, RotationOutcome};
use auth_service::domain::user::{User, UserStatus};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use chrono::{Duration, Utc};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::path::PathBuf;
use std::sync::Arc;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use tokio::sync::Mutex;
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
            .await
            .context("load atomic-auth PostgreSQL migrations")?
            .run(&pool)
            .await
            .context("migrate atomic-auth PostgreSQL fixture")?;
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
}

fn pending_user(email: &str) -> User {
    User {
        id: Uuid::new_v4(),
        primary_email: email.into(),
        password_hash: "original-hash".into(),
        status: UserStatus::PendingVerification,
        last_login_at: None,
    }
}

fn session(user_id: Uuid, jti: &str) -> RefreshTokenRecord {
    // A deterministic family keeps this fixture focused on rotation bookkeeping
    // rather than on family allocation, which the login and OAuth tests cover.
    RefreshTokenRecord {
        user_id,
        jti: jti.into(),
        token_hash: format!("hash-{jti}"),
        family_id: user_id,
        expires_at: Utc::now() + Duration::hours(1),
    }
}

fn audit(user_id: Uuid) -> LoginAudit {
    LoginAudit::success(
        uuid::Uuid::new_v4(),
        user_id,
        None,
        Some("atomic-auth-test".to_owned()),
        Utc::now(),
    )
}

#[tokio::test]
async fn signup_and_verification_roll_back_all_partial_writes() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    fn requires_atomic_port<T: AuthTransactionsPort>(_value: &T) {}
    requires_atomic_port(&fixture.users);
    let user = pending_user("atomic-signup@example.invalid");
    sqlx::raw_sql(
        "create function reject_mail() returns trigger language plpgsql as $$ begin raise exception 'injected mail failure'; end $$; create trigger reject_mail before insert on auth_mail_outbox for each row execute function reject_mail();"
    ).execute(&fixture.pool).await?;
    assert!(fixture
        .users
        .register_email(&(&user).into(), "123456")
        .await
        .is_err());
    let users: i64 = sqlx::query_scalar("select count(*) from users")
        .fetch_one(&fixture.pool)
        .await?;
    let identities: i64 = sqlx::query_scalar("select count(*) from user_identities")
        .fetch_one(&fixture.pool)
        .await?;
    let codes: i64 = sqlx::query_scalar("select count(*) from email_verifications")
        .fetch_one(&fixture.pool)
        .await?;
    assert_eq!((users, identities, codes), (0, 0, 0));
    sqlx::raw_sql("drop trigger reject_mail on auth_mail_outbox; drop function reject_mail();")
        .execute(&fixture.pool)
        .await?;
    fixture
        .users
        .register_email(&(&user).into(), "123456")
        .await?;
    sqlx::raw_sql(
        "create function reject_activation() returns trigger language plpgsql as $$ begin if new.status = 'ACTIVE' then raise exception 'injected activation failure'; end if; return new; end $$; create trigger reject_activation before update on users for each row execute function reject_activation();"
    ).execute(&fixture.pool).await?;
    assert!(fixture
        .users
        .verify_email(&user.primary_email, "123456")
        .await
        .is_err());
    let remaining: i64 =
        sqlx::query_scalar("select count(*) from email_verifications where user_id = $1")
            .bind(user.id)
            .fetch_one(&fixture.pool)
            .await?;
    assert_eq!(remaining, 1);
    assert_eq!(
        fixture
            .users
            .find_by_id(user.id)
            .await?
            .map(auth_service::domain::user::User::from)
            .expect("user")
            .status,
        UserStatus::PendingVerification
    );
    sqlx::raw_sql("drop trigger reject_activation on users; drop function reject_activation();")
        .execute(&fixture.pool)
        .await?;
    assert_eq!(
        fixture
            .users
            .verify_email(&user.primary_email, "123456")
            .await?,
        Some(user.id)
    );
    assert_eq!(
        fixture
            .users
            .verify_email(&user.primary_email, "123456")
            .await?,
        None
    );
    fixture.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn verification_attempts_and_reset_admission_are_bounded() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let user = pending_user("attempts@example.invalid");
    fixture
        .users
        .register_email(&(&user).into(), "123456")
        .await?;
    for _ in 0..5 {
        assert_eq!(
            fixture
                .users
                .verify_email(&user.primary_email, "999999")
                .await?,
            None
        );
    }
    assert_eq!(
        fixture
            .users
            .verify_email(&user.primary_email, "123456")
            .await?,
        None
    );
    let attempts: i32 =
        sqlx::query_scalar("select attempts from email_verifications where user_id = $1")
            .bind(user.id)
            .fetch_one(&fixture.pool)
            .await?;
    assert_eq!(attempts, 5);
    let active = User {
        status: UserStatus::Active,
        ..pending_user("reset-limit@example.invalid")
    };
    fixture.users.save(&(&active).into()).await?;
    let token = PasswordResetToken {
        user_id: active.id,
        token_hash: "reset-hash".into(),
        expires_at: Utc::now() + Duration::hours(1),
    };
    assert!(
        fixture
            .users
            .queue_password_reset(
                &(&active).into(),
                &(&token).into(),
                "https://client.invalid/reset#token=one"
            )
            .await?
    );
    assert!(
        !fixture
            .users
            .queue_password_reset(
                &(&active).into(),
                &(&token).into(),
                "https://client.invalid/reset#token=two"
            )
            .await?
    );
    let count: i64 =
        sqlx::query_scalar("select count(*) from password_reset_tokens where user_id = $1")
            .bind(active.id)
            .fetch_one(&fixture.pool)
            .await?;
    assert_eq!(count, 1);
    fixture.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn password_reset_failure_preserves_token_password_and_sessions() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let user = User {
        status: UserStatus::Active,
        ..pending_user("reset-rollback@example.invalid")
    };
    fixture.users.save(&(&user).into()).await?;
    let store = PgRefreshTokenStore::new(
        fixture.pool.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    store
        .store(&(&session(user.id, "before-reset")).into())
        .await?;
    let token = PasswordResetToken {
        user_id: user.id,
        token_hash: "reset-hash".into(),
        expires_at: Utc::now() + Duration::hours(1),
    };
    assert!(
        fixture
            .users
            .queue_password_reset(
                &(&user).into(),
                &(&token).into(),
                "https://client.invalid/reset#token=one"
            )
            .await?
    );
    sqlx::raw_sql(
        "create function reject_reset_audit() returns trigger language plpgsql as $$ begin if new.action = 'PASSWORD_RESET_COMPLETED' then raise exception 'injected audit failure'; end if; return new; end $$; create trigger reject_reset_audit before insert on auth_audit_events for each row execute function reject_reset_audit();"
    ).execute(&fixture.pool).await?;
    assert!(fixture
        .users
        .reset_password("reset-hash", "new-hash", Utc::now())
        .await
        .is_err());
    assert_eq!(
        fixture
            .users
            .find_by_id(user.id)
            .await?
            .map(auth_service::domain::user::User::from)
            .expect("user")
            .password_hash,
        "original-hash"
    );
    assert!(store.find("before-reset").await?.is_some());
    let tokens: i64 =
        sqlx::query_scalar("select count(*) from password_reset_tokens where user_id = $1")
            .bind(user.id)
            .fetch_one(&fixture.pool)
            .await?;
    assert_eq!(tokens, 1);
    sqlx::raw_sql(
        "drop trigger reject_reset_audit on auth_audit_events; drop function reject_reset_audit();",
    )
    .execute(&fixture.pool)
    .await?;
    assert_eq!(
        fixture
            .users
            .reset_password("reset-hash", "new-hash", Utc::now())
            .await?,
        Some(user.id)
    );
    assert!(store.find("before-reset").await?.is_none());
    assert_eq!(
        fixture
            .users
            .reset_password("reset-hash", "another-hash", Utc::now())
            .await?,
        None
    );
    fixture.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn session_commit_rejects_stale_credentials_roles_and_deleted_accounts() -> anyhow::Result<()>
{
    let fixture = Fixture::new().await?;
    let user = User {
        status: UserStatus::Active,
        ..pending_user("session-race@example.invalid")
    };
    fixture.users.save(&(&user).into()).await?;
    let mut tx = fixture.pool.begin().await?;
    sqlx::query("select id from users where id = $1 for update")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    let repository = fixture.users.clone();
    let snapshot = user.clone();
    let mut concurrent_login = tokio::spawn(async move {
        repository
            .begin_session(
                &(&snapshot).into(),
                &(&session(snapshot.id, "stale-login")).into(),
                &["ROLE_USER".into()],
                &(&audit(snapshot.id)).into(),
            )
            .await
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut concurrent_login)
            .await
            .is_err()
    );
    sqlx::query("update users set password_hash = 'changed-hash' where id = $1")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    assert!(!concurrent_login.await??);
    let current = fixture
        .users
        .find_by_id(user.id)
        .await?
        .map(auth_service::domain::user::User::from)
        .expect("current user");
    sqlx::query(
        "insert into auth_user_roles (user_id, role, granted_by) values ($1, 'ROLE_ADMIN', $1)",
    )
    .bind(user.id)
    .execute(&fixture.pool)
    .await?;
    assert!(
        !fixture
            .users
            .begin_session(
                &(&current).into(),
                &(&session(user.id, "stale-role")).into(),
                &["ROLE_USER".into()],
                &(&audit(user.id)).into()
            )
            .await?
    );
    let roles = fixture.users.roles(user.id).await?;
    assert_eq!(roles, vec!["ROLE_ADMIN".to_owned(), "ROLE_USER".to_owned()]);
    assert!(
        fixture
            .users
            .begin_session(
                &(&current).into(),
                &(&session(user.id, "admin-session")).into(),
                &roles,
                &(&audit(user.id)).into()
            )
            .await?
    );
    sqlx::query("delete from auth_user_roles where user_id = $1 and role = 'ROLE_ADMIN'")
        .bind(user.id)
        .execute(&fixture.pool)
        .await?;
    let store = PgRefreshTokenStore::new(
        fixture.pool.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    assert!(store.find("admin-session").await?.is_none());
    assert!(
        fixture
            .users
            .soft_delete_user(user.id, user.id, "test", Utc::now())
            .await?
    );
    assert!(
        !fixture
            .users
            .begin_session(
                &(&current).into(),
                &(&session(user.id, "after-delete")).into(),
                &["ROLE_USER".into()],
                &(&audit(user.id)).into()
            )
            .await?
    );
    assert!(store
        .store(&(&session(user.id, "direct-after-delete")).into())
        .await
        .is_err());
    sqlx::query("delete from users where id = $1")
        .bind(user.id)
        .execute(&fixture.pool)
        .await?;
    assert!(fixture.users.save(&(&current).into()).await.is_err());
    assert!(fixture
        .users
        .find_by_id(user.id)
        .await?
        .map(auth_service::domain::user::User::from)
        .is_none());
    fixture.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn failed_refresh_replacement_rolls_back_original_revocation() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let user = User {
        status: UserStatus::Active,
        ..pending_user("rotation@example.invalid")
    };
    fixture.users.save(&(&user).into()).await?;
    let store = PgRefreshTokenStore::new(
        fixture.pool.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    store.store(&(&session(user.id, "old")).into()).await?;
    store
        .store(&(&session(user.id, "collision")).into())
        .await?;
    assert!(store
        .rotate("old", "hash-old", &(&session(user.id, "collision")).into())
        .await
        .is_err());
    assert!(store.find("old").await?.is_some());
    assert_eq!(
        store
            .rotate("old", "hash-old", &(&session(user.id, "new")).into())
            .await?,
        RotationOutcome::Rotated.into()
    );
    assert!(store.find("old").await?.is_none());
    assert!(store.find("new").await?.is_some());
    // A second presentation of the superseded token reports *why* it failed,
    // which is what lets a caller tell a concurrent refresh apart from reuse.
    assert!(matches!(
        store
            .rotate("old", "hash-old", &(&session(user.id, "reused")).into())
            .await?
            .into(),
        RotationOutcome::AlreadyRotated { .. }
    ));
    fixture.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn simultaneous_refresh_transactions_create_exactly_one_successor() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let user = User {
        status: UserStatus::Active,
        ..pending_user("concurrent-rotation@example.invalid")
    };
    fixture.users.save(&(&user).into()).await?;
    let roles = fixture.users.roles(user.id).await?;
    let tokens = PgRefreshTokenStore::new(
        fixture.pool.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    tokens.store(&(&session(user.id, "old")).into()).await?;

    // Release both database transactions together. The winner must commit one
    // successor; the transaction that wakes on the row lock must observe that
    // same successor instead of creating a second active branch.
    let barrier = Arc::new(tokio::sync::Barrier::new(3));
    let first_repository = fixture.users.clone();
    let first_user = user.clone();
    let first_roles = roles.clone();
    let first_barrier = barrier.clone();
    let first = tokio::spawn(async move {
        first_barrier.wait().await;
        first_repository
            .rotate_session(
                &(&first_user).into(),
                "old",
                "hash-old",
                &(&session(first_user.id, "replacement-a")).into(),
                &first_roles,
            )
            .await
    });

    let second_repository = fixture.users.clone();
    let second_user = user.clone();
    let second_roles = roles.clone();
    let second_barrier = barrier.clone();
    let second = tokio::spawn(async move {
        second_barrier.wait().await;
        second_repository
            .rotate_session(
                &(&second_user).into(),
                "old",
                "hash-old",
                &(&session(second_user.id, "replacement-b")).into(),
                &second_roles,
            )
            .await
    });

    barrier.wait().await;
    let first = first.await??;
    let second = second.await??;
    let (winner_jti, reported_successor) =
        match (RotationOutcome::from(first), RotationOutcome::from(second)) {
            (RotationOutcome::Rotated, RotationOutcome::AlreadyRotated { successor_jti, .. }) => {
                ("replacement-a", successor_jti)
            }
            (RotationOutcome::AlreadyRotated { successor_jti, .. }, RotationOutcome::Rotated) => {
                ("replacement-b", successor_jti)
            }
            (first, second) => anyhow::bail!(
            "expected one successful rotation and one retryable loser, got {first:?} and {second:?}"
        ),
        };
    anyhow::ensure!(
        reported_successor == winner_jti,
        "the losing transaction must identify the one committed successor"
    );

    let successor = tokens
        .find(winner_jti)
        .await?
        .expect("the winning successor remains active");
    assert_eq!(successor.token_hash, format!("hash-{winner_jti}"));
    assert!(tokens.find("old").await?.is_none());
    let token_count: i64 =
        sqlx::query_scalar("select count(*) from refresh_tokens where user_id = $1")
            .bind(user.id)
            .fetch_one(&fixture.pool)
            .await?;
    assert_eq!(token_count, 2, "only the old token and one successor exist");

    fixture.pool.close().await;
    Ok(())
}

#[derive(Clone, Default)]
struct MailReceiver {
    requests: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
}

async fn receive_mail(
    State(state): State<MailReceiver>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> StatusCode {
    assert_eq!(
        headers.get("authorization").expect("mailer auth"),
        "Bearer test-mail-token"
    );
    let id = headers
        .get("idempotency-key")
        .expect("delivery identity")
        .to_str()
        .expect("ASCII key")
        .to_owned();
    let mut requests = state.requests.lock().await;
    requests.push((id, body));
    if requests.len() == 1 {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    }
}

#[tokio::test]
async fn mail_retry_reuses_delivery_identity_and_scrubs_payload_after_success() -> anyhow::Result<()>
{
    let fixture = Fixture::new().await?;
    let user = pending_user("mail@example.invalid");
    fixture
        .users
        .register_email(&(&user).into(), "123456")
        .await
        .context("register user and enqueue mail in the delivery retry test")?;
    let receiver = MailReceiver::default();
    let router = Router::new()
        .route("/mail", post(receive_mail))
        .with_state(receiver.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}/mail", listener.local_addr()?);
    let server = tokio::spawn(async move { axum::serve(listener, router).await });
    let worker = EmailOutboxWorker::new(
        fixture.pool.clone(),
        Some(MailDestination::new(&endpoint, "test-mail-token", false)?),
        None,
    )?;
    assert_eq!(
        worker
            .run_once()
            .await
            .context("first mail delivery attempt")?,
        0
    );
    sqlx::query("update auth_mail_outbox set next_attempt_at = now() where user_id = $1")
        .bind(user.id)
        .execute(&fixture.pool)
        .await
        .context("make the failed mail delivery eligible for retry")?;
    assert_eq!(worker.run_once().await.context("retry mail delivery")?, 1);
    assert_eq!(
        worker
            .run_once()
            .await
            .context("check delivered mail is not repeated")?,
        0
    );
    let delivered: bool = sqlx::query_scalar("select delivered_at is not null and payload_json is null and attempts = 2 from auth_mail_outbox where user_id = $1")
        .bind(user.id).fetch_one(&fixture.pool).await.context("verify delivered mail payload was scrubbed")?;
    assert!(delivered);
    let requests = receiver.requests.lock().await;
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].0, requests[1].0);
    assert_eq!(requests[0].1["verification_code"], "123456");
    drop(requests);
    server.abort();
    let _ = server.await;
    fixture.pool.close().await;
    Ok(())
}
