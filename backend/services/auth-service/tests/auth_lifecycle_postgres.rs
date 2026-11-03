//! Actual persistence with controlled application capabilities, not live mail/browser proof.
#[path = "support/auth_runtime_fixture.rs"]
mod auth_runtime_fixture;
#[path = "support/controlled_runtime.rs"]
mod controlled_runtime;

use anyhow::Result;
use auth_runtime_fixture::Fixture;
use auth_service::adapters::out::persistence::auth_lifecycle_cleanup_repository::PgAuthLifecycleCleanupRepository;
use auth_service::adapters::out::persistence::user_deletion_saga_repository::PgUserDeletionSagaRepository;
use auth_service::adapters::out::persistence::user_repository::{
    PgAuthAuditRepository, PgUserRepository,
};
use auth_service::adapters::out::security::admitted_argon2::AdmittedArgon2PasswordWork;
use auth_service::application::auth_runtime::AuthRuntimePort;
use auth_service::application::auth_transactions::AuthTransactionsPort;
use auth_service::application::dto::auth_dto::{DeleteUserDto, RetryUserDeletionDto};
use auth_service::application::dto::verification_code_dto::VerificationCodeDto;
use auth_service::application::errors::AuthError;
use auth_service::application::password_work::{
    PasswordWorkKind, PasswordWorkLimits, PasswordWorkPort, PasswordWorkStats,
};
use auth_service::application::ports::UserRepositoryPort;
use auth_service::application::use_cases::{
    delete_user::DeleteUserUseCase, lifecycle_cleanup::AuthLifecycleCleanupUseCase,
    retry_user_deletion::RetryUserDeletionUseCase, signup::SignupUseCase,
};
use auth_service::application::verification_crypto::VerificationCryptoPort;
use auth_service::domain::user::{User, UserStatus};
use chrono::{DateTime, Duration, Utc};
use controlled_runtime::ControlledRuntime;
use sqlx::Row;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use uuid::Uuid;

fn clock() -> Arc<ControlledRuntime> {
    Arc::new(ControlledRuntime::new(
        "2070-02-03T04:05:06Z".parse().unwrap(),
    ))
}
struct Crypto {
    calls: AtomicUsize,
    fail: bool,
}
impl VerificationCryptoPort for Crypto {
    fn generate(&self) -> auth_service::application::errors::AuthResult<VerificationCodeDto> {
        let result: anyhow::Result<_> = (|| {
            self.calls.fetch_add(1, Ordering::SeqCst);
            anyhow::ensure!(!self.fail, "fixture entropy failure");
            Ok(VerificationCodeDto {
                code: "000042".into(),
            })
        })();
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
}
struct Work {
    real: Arc<AdmittedArgon2PasswordWork>,
    clock: Arc<ControlledRuntime>,
    busy: bool,
}
#[async_trait::async_trait]
impl PasswordWorkPort for Work {
    async fn hash_for_signup(
        &self,
        password: &str,
    ) -> auth_service::application::errors::AuthResult<String> {
        let result: anyhow::Result<_> = async {
            if self.busy {
                return Err(AuthError::Busy.into());
            }
            let hash = self.real.hash_for_signup(password).await?;
            self.clock.advance(Duration::seconds(30));
            Ok(hash)
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
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
    async fn hash_for_reset(
        &self,
        _: &str,
    ) -> auth_service::application::errors::AuthResult<String> {
        let result: anyhow::Result<_> = async { anyhow::bail!("unexpected reset") }.await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
    fn stats(&self, kind: PasswordWorkKind) -> PasswordWorkStats {
        self.real.stats(kind)
    }
}
fn signup(
    users: Arc<PgUserRepository>,
    clock: Arc<ControlledRuntime>,
    crypto: Arc<Crypto>,
    busy: bool,
) -> Result<SignupUseCase<Arc<PgUserRepository>>> {
    Ok(SignupUseCase::new(
        users.clone(),
        users,
        Arc::new(Work {
            real: Arc::new(AdmittedArgon2PasswordWork::new(
                PasswordWorkLimits::default(),
            )?),
            clock: clock.clone(),
            busy,
        }),
        clock,
        crypto,
    ))
}
async fn counts(f: &Fixture) -> Result<Vec<i64>> {
    let mut result = vec![];
    for table in [
        "users",
        "user_identities",
        "email_verifications",
        "auth_mail_outbox",
        "auth_audit_events",
        "refresh_tokens",
        "user_deletion_sagas",
        "user_deletion_steps",
    ] {
        result.push(
            sqlx::query_scalar(&format!("select count(*) from {table}"))
                .fetch_one(&f.pool)
                .await?,
        );
    }
    Ok(result)
}
async fn observe(f: &Fixture, case: &str) {
    eprintln!(
        "AUTH_RUNTIME_OBSERVATION {}",
        serde_json::json!({"database":f.database,"case":case,"effectsVerified":true})
    );
    f.pool.close().await;
}

#[tokio::test]
async fn verification_entropy_failure_leaves_signup_and_resend_without_effects() -> Result<()> {
    let f = Fixture::new().await?;
    f.empty_effects().await?;
    let runtime = clock();
    let users = Arc::new(PgUserRepository::new(f.pool.clone(), runtime.clone()));
    let crypto = Arc::new(Crypto {
        calls: AtomicUsize::new(0),
        fail: true,
    });
    let signup = signup(users.clone(), runtime.clone(), crypto.clone(), false)?;
    assert!(signup.signup_email("malformed", "Passw0rd!").await.is_err());
    assert_eq!(crypto.calls.load(Ordering::SeqCst), 0);
    assert!(signup
        .signup_email("owned@example.invalid", "Passw0rd!")
        .await
        .is_err());
    assert_eq!(runtime.issued_ids(), 0);
    assert_eq!(counts(&f).await?, vec![0; 8]);
    let pending = User {
        id: Uuid::from_u128(99),
        primary_email: "pending@example.invalid".into(),
        password_hash: "unused-fixture-hash".into(),
        status: UserStatus::PendingVerification,
        last_login_at: None,
    };
    users.register_email(&(&pending).into(), "123456").await?;
    let before = counts(&f).await?;
    let known = signup
        .resend_verification(&pending.primary_email)
        .await
        .err()
        .unwrap();
    let unknown = signup
        .resend_verification("unknown@example.invalid")
        .await
        .err()
        .unwrap();
    assert_eq!(known.to_string(), unknown.to_string());
    assert_eq!(counts(&f).await?, before);
    assert_eq!(crypto.calls.load(Ordering::SeqCst), 3);
    let code: String = sqlx::query_scalar("select code from email_verifications where user_id=$1")
        .bind(pending.id)
        .fetch_one(&f.pool)
        .await?;
    assert_eq!(code, "123456");
    observe(&f, "entropy-no-effects-and-nonenumerating-resend").await;
    Ok(())
}

#[tokio::test]
async fn injected_signup_busy_spends_no_challenge_entropy_or_identity() -> Result<()> {
    let f = Fixture::new().await?;
    f.empty_effects().await?;
    let runtime = clock();
    let users = Arc::new(PgUserRepository::new(f.pool.clone(), runtime.clone()));
    let crypto = Arc::new(Crypto {
        calls: AtomicUsize::new(0),
        fail: false,
    });
    let signup = signup(users, runtime.clone(), crypto.clone(), true)?;
    let error = signup
        .signup_email("busy@example.invalid", "Passw0rd!")
        .await
        .err()
        .unwrap();
    assert!(matches!(Some(&error), Some(AuthError::Busy)));
    assert_eq!(crypto.calls.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.issued_ids(), 0);
    assert_eq!(counts(&f).await?, vec![0; 8]);
    observe(&f, "injected-busy-zero-effects").await;
    Ok(())
}

#[tokio::test]
async fn signup_samples_post_hash_time_and_preserves_atomic_audit_rollback() -> Result<()> {
    let f = Fixture::new().await?;
    f.empty_effects().await?;
    let runtime = clock();
    let users = Arc::new(PgUserRepository::new(f.pool.clone(), runtime.clone()));
    let crypto = Arc::new(Crypto {
        calls: AtomicUsize::new(0),
        fail: false,
    });
    let signup = signup(users.clone(), runtime.clone(), crypto, false)?;
    sqlx::raw_sql("create function reject_audit() returns trigger language plpgsql as $$ begin raise exception 'fixture audit rejection'; end $$; create trigger reject_audit before insert on auth_audit_events for each row execute function reject_audit();").execute(&f.pool).await?;
    assert!(signup
        .signup_email("new@example.invalid", "Passw0rd!")
        .await
        .is_err());
    assert_eq!(counts(&f).await?, vec![0; 8]);
    sqlx::raw_sql("drop trigger reject_audit on auth_audit_events; drop function reject_audit();")
        .execute(&f.pool)
        .await?;
    let response = signup
        .signup_email(" NEW@EXAMPLE.INVALID ", "Passw0rd!")
        .await?;
    let user = users
        .find_by_id(response.user_id)
        .await?
        .map(auth_service::domain::user::User::from)
        .unwrap();
    assert_eq!(user.last_login_at, Some(runtime.now()));
    assert_eq!(user.last_login_at, Some("2070-02-03T04:06:06Z".parse()?));
    assert_eq!(user.status, UserStatus::PendingVerification);
    assert!(user.password_hash.starts_with("$argon2"));
    let code: String = sqlx::query_scalar("select code from email_verifications where user_id=$1")
        .bind(user.id)
        .fetch_one(&f.pool)
        .await?;
    assert_eq!(code, "000042");
    assert_eq!(counts(&f).await?, vec![1, 1, 1, 1, 1, 0, 0, 0]);
    assert_eq!(
        signup
            .verify_email(&user.primary_email, &code)
            .await?
            .status,
        "VERIFIED"
    );
    assert!(signup
        .verify_email(&user.primary_email, &code)
        .await
        .is_err());
    observe(
        &f,
        "post-hash-clock-real-argon2-audit-rollback-retry-verification-once",
    )
    .await;
    Ok(())
}

#[tokio::test]
async fn deletion_and_retry_bind_runtime_and_preserve_atomic_replay() -> Result<()> {
    let f = Fixture::new().await?;
    f.empty_effects().await?;
    let runtime = clock();
    let users = Arc::new(PgUserRepository::new(f.pool.clone(), runtime.clone()));
    let user = User {
        id: Uuid::from_u128(101),
        primary_email: "delete@example.invalid".into(),
        password_hash: "unused-fixture-hash".into(),
        status: UserStatus::Active,
        last_login_at: None,
    };
    users.save(&(&user).into()).await?;
    let sagas = Arc::new(PgUserDeletionSagaRepository::new(f.pool.clone()));
    let deletion = DeleteUserUseCase::new(
        users.clone(),
        Arc::new(PgAuthAuditRepository::new(f.pool.clone())),
        sagas.clone(),
        runtime.clone(),
    );
    let request = DeleteUserDto {
        user_id: user.id.to_string(),
        actor_id: user.id.to_string(),
        request_id: Some("owned-delete".into()),
        ip_address: None,
        user_agent: None,
    };
    sqlx::raw_sql("create function reject_audit() returns trigger language plpgsql as $$ begin raise exception 'fixture audit rejection'; end $$; create trigger reject_audit before insert on auth_audit_events for each row execute function reject_audit();").execute(&f.pool).await?;
    assert!(deletion.execute(request.clone()).await.is_err());
    assert_eq!(
        users
            .find_by_id(user.id)
            .await?
            .map(auth_service::domain::user::User::from)
            .unwrap()
            .status,
        UserStatus::Active
    );
    assert_eq!(counts(&f).await?, vec![1, 0, 0, 0, 0, 0, 0, 0]);
    sqlx::raw_sql("drop trigger reject_audit on auth_audit_events; drop function reject_audit();")
        .execute(&f.pool)
        .await?;
    let response = deletion.execute(request.clone()).await?;
    let saga_id: Uuid = response.saga_id.parse()?;
    assert_eq!(saga_id, Uuid::from_u128(0x23450000000000000000000000000002));
    let row = sqlx::query("select a.id,a.occurred_at,u.deleted_at,u.retention_until from auth_audit_events a join users u on u.id=a.subject_user_id where a.action='USER_DELETION_REQUESTED'").fetch_one(&f.pool).await?;
    assert_eq!(
        row.get::<Uuid, _>("id"),
        Uuid::from_u128(0x23450000000000000000000000000003)
    );
    assert_eq!(row.get::<DateTime<Utc>, _>("occurred_at"), runtime.now());
    assert_eq!(row.get::<DateTime<Utc>, _>("deleted_at"), runtime.now());
    assert_eq!(
        row.get::<DateTime<Utc>, _>("retention_until"),
        runtime.now() + Duration::days(30)
    );
    runtime.advance(Duration::days(1));
    assert_eq!(deletion.execute(request).await?.saga_id, response.saga_id);
    assert_eq!(runtime.issued_ids(), 4);
    let other = User {
        id: Uuid::from_u128(404),
        primary_email: "other@example.invalid".into(),
        password_hash: "unused-fixture-hash".into(),
        status: UserStatus::Active,
        last_login_at: None,
    };
    users.save(&(&other).into()).await?;
    let forbidden = DeleteUserDto {
        user_id: user.id.to_string(),
        actor_id: other.id.to_string(),
        request_id: Some("owned-forbidden".into()),
        ip_address: None,
        user_agent: None,
    };
    let error = deletion.execute(forbidden).await.err().unwrap();
    assert!(matches!(Some(&error), Some(AuthError::Forbidden)));
    let audit =
        sqlx::query("select id,occurred_at from auth_audit_events where outcome='FORBIDDEN'")
            .fetch_one(&f.pool)
            .await?;
    assert_eq!(
        audit.get::<Uuid, _>("id"),
        Uuid::from_u128(0x23450000000000000000000000000004)
    );
    assert_eq!(audit.get::<DateTime<Utc>, _>("occurred_at"), runtime.now());
    assert_eq!(
        users
            .find_by_id(other.id)
            .await?
            .map(auth_service::domain::user::User::from)
            .unwrap()
            .status,
        UserStatus::Active
    );
    assert_eq!(runtime.issued_ids(), 5);
    sqlx::query("update user_deletion_sagas set status='DEAD_LETTERED' where id=$1")
        .bind(saga_id)
        .execute(&f.pool)
        .await?;
    sqlx::query(
        "update user_deletion_steps set status='DEAD_LETTERED',attempt_count=8 where saga_id=$1",
    )
    .bind(saga_id)
    .execute(&f.pool)
    .await?;
    let retry = RetryUserDeletionUseCase::new(sagas, runtime.clone());
    let resolution_id = Uuid::from_u128(202);
    let retry_request = RetryUserDeletionDto {
        saga_id: saga_id.to_string(),
        resolution_id: resolution_id.to_string(),
        actor_id: user.id.to_string(),
        reason: "owned fixture restoration evidence".into(),
        evidence_reference: "fixture:ticket-17".into(),
    };
    retry.execute(retry_request.clone()).await?;
    let created: DateTime<Utc> = sqlx::query_scalar(
        "select created_at from user_deletion_retry_resolutions where resolution_id=$1",
    )
    .bind(resolution_id)
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(created, runtime.now());
    let before = counts(&f).await?;
    runtime.advance(Duration::hours(1));
    retry.execute(retry_request).await?;
    assert_eq!(counts(&f).await?, before);
    let resolutions: i64 =
        sqlx::query_scalar("select count(*) from user_deletion_retry_resolutions")
            .fetch_one(&f.pool)
            .await?;
    assert_eq!(resolutions, 1);
    observe(
        &f,
        "runtime-deletion-audit-rollback-replay-and-resolution-once",
    )
    .await;
    Ok(())
}

#[tokio::test]
async fn cleanup_obeys_injected_exact_expiry_in_actual_postgres() -> Result<()> {
    let f = Fixture::new().await?;
    f.empty_effects().await?;
    let runtime = clock();
    let users = PgUserRepository::new(f.pool.clone(), runtime.clone());
    let user = User {
        id: Uuid::from_u128(303),
        primary_email: "cleanup@example.invalid".into(),
        password_hash: "unused-fixture-hash".into(),
        status: UserStatus::Active,
        last_login_at: None,
    };
    users.save(&(&user).into()).await?;
    let expiry = runtime.now() + Duration::seconds(1);
    sqlx::query("insert into email_verifications(id,user_id,email,code,expires_at) values($1,$2,$3,'000042',$4)").bind(Uuid::from_u128(304)).bind(user.id).bind(&user.primary_email).bind(expiry).execute(&f.pool).await?;
    let cleanup = AuthLifecycleCleanupUseCase::new(
        PgAuthLifecycleCleanupRepository::new(f.pool.clone()),
        runtime.clone(),
    );
    assert_eq!(cleanup.execute().await?.deleted_email_verifications, 0);
    runtime.advance(Duration::seconds(1));
    assert_eq!(cleanup.execute().await?.deleted_email_verifications, 1);
    assert_eq!(cleanup.execute().await?.deleted_email_verifications, 0);
    assert_eq!(runtime.issued_ids(), 0);
    assert!(users
        .find_by_id(user.id)
        .await?
        .map(auth_service::domain::user::User::from)
        .is_some());
    observe(&f, "injected-exact-expiry-cleanup-once").await;
    Ok(())
}
