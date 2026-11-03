//! Actual reset persistence; fixed application time is distinct from DB wall time.
#[path = "support/auth_runtime_fixture.rs"]
mod auth_runtime_fixture;
use anyhow::Result;
use auth_runtime_fixture::Fixture;
use auth_service::adapters::out::cache::session_store::PgRefreshTokenStore;
use auth_service::adapters::out::persistence::user_repository::PgUserRepository;
use auth_service::adapters::out::security::admitted_argon2::AdmittedArgon2PasswordWork;
use auth_service::adapters::out::security::password_reset::SecurePasswordReset;
use auth_service::application::auth_runtime::AuthRuntimePort;
use auth_service::application::dto::password_reset_material_dto::PasswordResetMaterialDto;
use auth_service::application::password_reset_crypto::PasswordResetCryptoPort;
use auth_service::application::password_work::{
    PasswordWorkKind, PasswordWorkLimits, PasswordWorkPort, PasswordWorkStats,
};
use auth_service::application::ports::{RefreshTokenStorePort, UserRepositoryPort};
use auth_service::application::use_cases::password_reset::{
    PasswordResetLinkBaseUrl, PasswordResetUseCase,
};
use auth_service::domain::session::RefreshTokenRecord;
use auth_service::domain::user::{User, UserStatus};
use chrono::{DateTime, Duration, Utc};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

struct Clock(Mutex<DateTime<Utc>>);
impl AuthRuntimePort for Clock {
    fn now(&self) -> DateTime<Utc> {
        *self.0.lock().unwrap()
    }
    fn new_id(&self) -> Uuid {
        Uuid::new_v4()
    }
}
struct FailingCrypto;
impl PasswordResetCryptoPort for FailingCrypto {
    fn generate(&self) -> auth_service::application::errors::AuthResult<PasswordResetMaterialDto> {
        let result: anyhow::Result<_> = (|| anyhow::bail!("fixture entropy failure"))();
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
    fn fingerprint(&self, token: &str) -> String {
        SecurePasswordReset.fingerprint(token)
    }
}
struct ExpiringWork(Arc<Clock>);
#[async_trait::async_trait]
impl PasswordWorkPort for ExpiringWork {
    async fn verify_login(
        &self,
        _: &str,
        _: &str,
    ) -> auth_service::application::errors::AuthResult<bool> {
        let result: anyhow::Result<_> = async { anyhow::bail!("unexpected login") }.await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
    async fn equalize_login(&self, _: &str) -> auth_service::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async { anyhow::bail!("unexpected login") }.await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
    async fn hash_for_signup(
        &self,
        _: &str,
    ) -> auth_service::application::errors::AuthResult<String> {
        let result: anyhow::Result<_> = async { anyhow::bail!("unexpected signup") }.await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
    async fn hash_for_reset(
        &self,
        _: &str,
    ) -> auth_service::application::errors::AuthResult<String> {
        let result: anyhow::Result<_> = async {
            *self.0 .0.lock().unwrap() += Duration::hours(1);
            Ok("expired-password-must-not-commit".into())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
    fn stats(&self, _: PasswordWorkKind) -> PasswordWorkStats {
        Default::default()
    }
}
async fn seed(f: &Fixture) -> Result<(Arc<PgUserRepository>, Arc<Clock>, User)> {
    f.empty_effects().await?;
    let clock = Arc::new(Clock(Mutex::new("2070-01-01T00:00:00Z".parse()?)));
    let users = Arc::new(PgUserRepository::new(f.pool.clone(), clock.clone()));
    let user = User {
        id: Uuid::new_v4(),
        primary_email: "owned-reset@example.invalid".into(),
        password_hash: "unchanged-fixture-password".into(),
        status: UserStatus::Active,
        last_login_at: None,
    };
    users.save(&(&user).into()).await?;
    Ok((users, clock, user))
}
fn use_case(
    users: Arc<PgUserRepository>,
    clock: Arc<Clock>,
    work: Arc<dyn PasswordWorkPort>,
    crypto: Arc<dyn PasswordResetCryptoPort>,
) -> PasswordResetUseCase<Arc<PgUserRepository>> {
    PasswordResetUseCase::new(
        users.clone(),
        users,
        work,
        PasswordResetLinkBaseUrl::new("https://fixture.invalid/reset").unwrap(),
        clock,
        crypto,
    )
}
async fn count(f: &Fixture, table: &str) -> Result<i64> {
    Ok(sqlx::query_scalar(&format!("select count(*) from {table}"))
        .fetch_one(&f.pool)
        .await?)
}
async fn delivered_token(f: &Fixture) -> Result<String> {
    let payload: String =
        sqlx::query_scalar("select payload_json from auth_mail_outbox where kind='PASSWORD_RESET'")
            .fetch_one(&f.pool)
            .await?;
    let value: serde_json::Value = serde_json::from_str(&payload)?;
    Ok(value["link"]
        .as_str()
        .unwrap()
        .split_once("#token=")
        .unwrap()
        .1
        .to_owned())
}
fn observe(f: &Fixture, scenario: &str) {
    eprintln!(
        "AUTH_RUNTIME_OBSERVATION {}",
        serde_json::json!({"database":f.database,"scenario":scenario,"effectsVerified":true})
    );
}
#[tokio::test]
async fn reset_crypto_failure_leaves_actual_token_mail_audit_tables_empty() -> Result<()> {
    let f = Fixture::new().await?;
    let (users, clock, _) = seed(&f).await?;
    let work = Arc::new(AdmittedArgon2PasswordWork::new(
        PasswordWorkLimits::default(),
    )?);
    let reset = use_case(users, clock, work, Arc::new(FailingCrypto));
    assert_eq!(
        reset.request_reset("unknown@example.invalid").await?.status,
        "SENT"
    );
    assert!(reset
        .request_reset("owned-reset@example.invalid")
        .await
        .is_err());
    for table in [
        "password_reset_tokens",
        "auth_mail_outbox",
        "auth_audit_events",
        "refresh_tokens",
    ] {
        assert_eq!(count(&f, table).await?, 0, "{table}");
    }
    observe(&f, "reset-entropy-failure-no-effects");
    f.pool.close().await;
    Ok(())
}
#[tokio::test]
async fn reset_real_argon2_audit_failure_rolls_back_then_retry_consumes_once() -> Result<()> {
    let f = Fixture::new().await?;
    let (users, clock, user) = seed(&f).await?;
    let work = Arc::new(AdmittedArgon2PasswordWork::new(
        PasswordWorkLimits::default(),
    )?);
    let store = PgRefreshTokenStore::new(f.pool.clone(), clock.clone());
    store
        .store(
            &(&RefreshTokenRecord::new_family(
                uuid::Uuid::new_v4(),
                "owned-reset-jti".into(),
                "fixture-fingerprint".into(),
                user.id,
                clock.now() + Duration::days(1),
            ))
                .into(),
        )
        .await?;
    let reset = use_case(
        users.clone(),
        clock.clone(),
        work.clone(),
        Arc::new(SecurePasswordReset),
    );
    assert_eq!(
        reset.request_reset(&user.primary_email).await?.status,
        "SENT"
    );
    let token = delivered_token(&f).await?;
    assert_eq!(token.len(), 43);
    let expiry: DateTime<Utc> = sqlx::query_scalar("select expires_at from password_reset_tokens")
        .fetch_one(&f.pool)
        .await?;
    assert_eq!(expiry, clock.now() + Duration::hours(1));
    sqlx::query("create function fail_reset_audit() returns trigger language plpgsql as $$ begin if new.action='PASSWORD_RESET_COMPLETED' then raise exception 'controlled reset audit failure'; end if; return new; end $$").execute(&f.pool).await?;
    sqlx::query("create trigger fail_reset_audit before insert on auth_audit_events for each row execute function fail_reset_audit()").execute(&f.pool).await?;
    assert!(reset.confirm_reset(&token, "NewPassw0rd!").await.is_err());
    assert_eq!(
        users
            .find_by_id(user.id)
            .await?
            .map(auth_service::domain::user::User::from)
            .unwrap()
            .password_hash,
        user.password_hash
    );
    assert_eq!(count(&f, "password_reset_tokens").await?, 1);
    assert_eq!(count(&f, "auth_audit_events").await?, 1);
    let live: i64 =
        sqlx::query_scalar("select count(*) from refresh_tokens where revoked_at is null")
            .fetch_one(&f.pool)
            .await?;
    assert_eq!(live, 1);
    let payloads:i64=sqlx::query_scalar("select count(*) from auth_mail_outbox where payload_json is not null and dead_lettered_at is null").fetch_one(&f.pool).await?;
    assert_eq!(payloads, 1);
    sqlx::query("drop trigger fail_reset_audit on auth_audit_events")
        .execute(&f.pool)
        .await?;
    assert_eq!(
        reset.confirm_reset(&token, "NewPassw0rd!").await?.status,
        "RESET"
    );
    assert!(
        work.verify_login(
            "NewPassw0rd!",
            &users
                .find_by_id(user.id)
                .await?
                .map(auth_service::domain::user::User::from)
                .unwrap()
                .password_hash
        )
        .await?
    );
    assert_eq!(count(&f, "password_reset_tokens").await?, 0);
    assert_eq!(count(&f, "auth_audit_events").await?, 2);
    let live: i64 =
        sqlx::query_scalar("select count(*) from refresh_tokens where revoked_at is null")
            .fetch_one(&f.pool)
            .await?;
    assert_eq!(live, 0);
    assert!(reset
        .confirm_reset(&token, "AnotherPassw0rd!")
        .await
        .is_err());
    assert_eq!(count(&f, "auth_audit_events").await?, 2);
    observe(&f, "reset-real-argon2-audit-rollback-retry-once");
    f.pool.close().await;
    Ok(())
}
#[tokio::test]
async fn reset_exact_expiry_after_password_work_preserves_actual_effects() -> Result<()> {
    let f = Fixture::new().await?;
    let (users, clock, user) = seed(&f).await?;
    let reset = use_case(
        users.clone(),
        clock.clone(),
        Arc::new(ExpiringWork(clock)),
        Arc::new(SecurePasswordReset),
    );
    reset.request_reset(&user.primary_email).await?;
    let token = delivered_token(&f).await?;
    assert!(reset.confirm_reset(&token, "NewPassw0rd!").await.is_err());
    assert_eq!(
        users
            .find_by_id(user.id)
            .await?
            .map(auth_service::domain::user::User::from)
            .unwrap()
            .password_hash,
        user.password_hash
    );
    assert_eq!(count(&f, "password_reset_tokens").await?, 1);
    assert_eq!(count(&f, "auth_audit_events").await?, 1);
    let payloads:i64=sqlx::query_scalar("select count(*) from auth_mail_outbox where payload_json is not null and dead_lettered_at is null").fetch_one(&f.pool).await?;
    assert_eq!(payloads, 1);
    observe(&f, "reset-controlled-clock-exact-expiry-no-consumption");
    f.pool.close().await;
    Ok(())
}
