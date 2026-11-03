//! H3-A acceptance: explicit password and transaction ports over real PostgreSQL.
//!
//! The default route uses Testcontainers. Environments without Docker may opt
//! into an isolated loopback PostgreSQL schema with both:
//! `AUTH_H3A_TEST_DATABASE_URL` and `AUTH_H3A_TEST_ALLOW_LOCAL_PG=1`.

#[path = "support/owned_database.rs"]
mod owned_database;

use anyhow::{Context, Result};
use auth_service::adapters::out::cache::session_store::PgRefreshTokenStore;
use auth_service::adapters::out::security::admitted_argon2::{
    hash_password, AdmittedArgon2PasswordWork,
};
use auth_service::application::password_work::{PasswordWorkKind, PasswordWorkLimits};
use auth_service::application::ports::RefreshTokenStorePort;
use auth_service::config::{build_state, build_state_with_password_work, connect_pool, AppState};
use auth_service::domain::session::RefreshTokenRecord;
use auth_service::domain::user::{User, UserStatus};
use base64ct::{Base64UrlUnpadded, Encoding};
use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use std::net::IpAddr;
use std::sync::Arc;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;

const DATABASE_URL_ENV: &str = "AUTH_H3A_TEST_DATABASE_URL";
const WRITE_OPT_IN_ENV: &str = "AUTH_H3A_TEST_ALLOW_LOCAL_PG";
const DATABASE_PREFIX: &str = "port_rust_auth_test";
const SCHEMA_PREFIX: &str = "auth_h3a_fixture_";

struct Fixture {
    pool: PgPool,
    state: Arc<AppState>,
    cleanup: Cleanup,
}

impl Fixture {
    async fn start(password_work: Option<Arc<AdmittedArgon2PasswordWork>>) -> Result<Self> {
        let (backing, database_url) = provision_postgres().await?;
        let mut cleanup = Cleanup {
            pool: None,
            backing: Some(backing),
        };
        let config = test_config(database_url)?;
        let pool = connect_pool(&config).await?;
        cleanup.pool = Some(pool.clone());
        let state = Arc::new(match password_work {
            Some(password_work) => {
                build_state_with_password_work(&config, pool.clone(), password_work).await?
            }
            None => build_state(&config, pool.clone()).await?,
        });
        Ok(Self {
            pool,
            state,
            cleanup,
        })
    }

    async fn shutdown(mut self) -> Result<()> {
        self.cleanup.shutdown().await
    }
}

enum Backing {
    OwnedDatabase,
    Container(Box<ContainerAsync<Postgres>>),
    LocalSchema { admin_pool: PgPool, schema: String },
}

struct Cleanup {
    pool: Option<PgPool>,
    backing: Option<Backing>,
}

impl Cleanup {
    async fn shutdown(&mut self) -> Result<()> {
        match self.take_job() {
            Some(job) => job.run().await,
            None => Ok(()),
        }
    }

    fn take_job(&mut self) -> Option<CleanupJob> {
        Some(CleanupJob {
            pool: self.pool.take(),
            backing: self.backing.take()?,
        })
    }
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let Some(job) = self.take_job() else {
            return;
        };
        if let Err(error) = cleanup_sender().send(job) {
            drop(error.0);
        }
    }
}

struct CleanupJob {
    pool: Option<PgPool>,
    backing: Backing,
}

impl CleanupJob {
    async fn run(self) -> Result<()> {
        if let Some(pool) = self.pool {
            pool.close().await;
        }
        match self.backing {
            Backing::OwnedDatabase => Ok(()),
            Backing::Container(postgres) => (*postgres)
                .rm()
                .await
                .map_err(|error| anyhow::anyhow!("remove Auth H3-A Postgres: {error}")),
            Backing::LocalSchema { admin_pool, schema } => {
                anyhow::ensure!(schema.starts_with(SCHEMA_PREFIX), "unsafe fixture schema");
                sqlx::query(&format!(
                    "drop schema if exists {} cascade",
                    quote_identifier(&schema)
                ))
                .execute(&admin_pool)
                .await
                .context("drop Auth H3-A fixture schema")?;
                admin_pool.close().await;
                Ok(())
            }
        }
    }
}

fn cleanup_sender() -> &'static tokio::sync::mpsc::UnboundedSender<CleanupJob> {
    static SENDER: std::sync::OnceLock<tokio::sync::mpsc::UnboundedSender<CleanupJob>> =
        std::sync::OnceLock::new();
    SENDER.get_or_init(|| {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<CleanupJob>();
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .thread_name("auth-h3a-test-cleanup")
                .enable_all()
                .build()
                .expect("Auth H3-A cleanup runtime")
                .block_on(async move {
                    while let Some(job) = receiver.recv().await {
                        let _ = job.run().await;
                    }
                });
        });
        sender
    })
}

async fn provision_postgres() -> Result<(Backing, String)> {
    if let Some(url) = owned_database::native_database_url().await? {
        return Ok((Backing::OwnedDatabase, url));
    }
    match local_database_url()? {
        Some(database_url) => provision_local_schema(&database_url).await,
        None => {
            let (postgres, database_url) =
                test_support::postgres::start_postgres_with_url().await?;
            Ok((Backing::Container(Box::new(postgres)), database_url))
        }
    }
}

fn local_database_url() -> Result<Option<String>> {
    let database_url = std::env::var(DATABASE_URL_ENV)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    let opt_in = std::env::var(WRITE_OPT_IN_ENV).ok();
    match (database_url, opt_in.as_deref()) {
        (None, None) => Ok(None),
        (Some(url), Some("1")) => Ok(Some(url)),
        (Some(_), _) => anyhow::bail!("local Auth H3-A PostgreSQL requires explicit write opt-in"),
        (None, Some(_)) => anyhow::bail!("Auth H3-A PostgreSQL opt-in has no database URL"),
    }
}

async fn provision_local_schema(database_url: &str) -> Result<(Backing, String)> {
    let options = validate_local_url(database_url)?;
    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(options)
        .await
        .map_err(|_| anyhow::anyhow!("failed to connect to local Auth H3-A PostgreSQL"))?;
    let schema = format!("{SCHEMA_PREFIX}{}", Uuid::new_v4().simple());
    sqlx::query(&format!("create schema {}", quote_identifier(&schema)))
        .execute(&admin_pool)
        .await?;
    let url = format!(
        "{database_url}?application_name={schema}&options=-c%20search_path%3D{schema}%2Cpg_catalog"
    );
    Ok((Backing::LocalSchema { admin_pool, schema }, url))
}

fn validate_local_url(database_url: &str) -> Result<PgConnectOptions> {
    anyhow::ensure!(
        !database_url.contains('?') && !database_url.contains('#'),
        "local Auth H3-A database URL must not contain overrides"
    );
    let (_, remainder) = database_url
        .split_once("://")
        .ok_or_else(|| anyhow::anyhow!("invalid local Auth H3-A database URL"))?;
    let (_, database) = remainder
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("local Auth H3-A URL must name a test database"))?;
    anyhow::ensure!(
        database.is_ascii()
            && database.len() <= 63
            && database.starts_with(DATABASE_PREFIX)
            && database[DATABASE_PREFIX.len()..]
                .strip_prefix('_')
                .map(|suffix| {
                    !suffix.is_empty()
                        && suffix.split('_').all(|part| {
                            !part.is_empty()
                                && part
                                    .bytes()
                                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                        })
                })
                .unwrap_or(database == DATABASE_PREFIX),
        "local Auth H3-A URL must name a canonical test database"
    );
    let options: PgConnectOptions = database_url
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid local Auth H3-A database URL"))?;
    anyhow::ensure!(options.get_socket().is_none(), "loopback TCP is required");
    let host = options
        .get_host()
        .trim_start_matches('[')
        .trim_end_matches(']');
    anyhow::ensure!(
        host.eq_ignore_ascii_case("localhost")
            || host
                .parse::<IpAddr>()
                .map(|address| address.is_loopback())
                .unwrap_or(false),
        "loopback TCP is required"
    );
    anyhow::ensure!(
        options.get_database() == Some(database),
        "unexpected database"
    );
    anyhow::ensure!(
        options.get_options().is_none(),
        "connection overrides are forbidden"
    );
    Ok(options)
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

#[path = "support/auth_fixture_config.rs"]
mod auth_fixture_config;
use auth_fixture_config::test_config;

fn raw_reset_hash(token: &str) -> String {
    Base64UrlUnpadded::encode_string(Sha256::digest(token.as_bytes()).as_slice())
}

async fn insert_active_user(pool: &PgPool, email: &str) -> Result<User> {
    let user = User {
        id: Uuid::new_v4(),
        primary_email: email.into(),
        password_hash: hash_password("Passw0rd!")?,
        status: UserStatus::Active,
        last_login_at: None,
    };
    sqlx::query("insert into users (id,primary_email,password_hash,status,last_login_at) values ($1,$2,$3,'ACTIVE',null)")
        .bind(user.id).bind(&user.primary_email).bind(&user.password_hash)
        .execute(pool).await?;
    Ok(user)
}

struct OccupiedSlot {
    release: std::sync::mpsc::Sender<()>,
    task: tokio::task::JoinHandle<Result<()>>,
}

impl OccupiedSlot {
    async fn release(self) {
        let _ = self.release.send(());
        self.task.await.expect("occupier").expect("occupier result");
    }
}

async fn occupy(class: &platform::cpu::AdmissionClass) -> OccupiedSlot {
    let class = class.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release, release_rx) = std::sync::mpsc::channel();
    let task = tokio::spawn(async move {
        class
            .run(move || {
                let _ = started_tx.send(());
                release_rx.recv()?;
                Ok(())
            })
            .await
    });
    started_rx.await.expect("started");
    OccupiedSlot { release, task }
}

#[tokio::test]
async fn signup_use_case_rolls_back_every_write_when_delivery_intent_fails() -> Result<()> {
    let fixture = Fixture::start(None).await?;
    sqlx::raw_sql("create function h3a_reject_signup_mail() returns trigger language plpgsql as $$ begin raise exception 'injected signup mail failure'; end $$; create trigger h3a_reject_signup_mail before insert on auth_mail_outbox for each row execute function h3a_reject_signup_mail();")
        .execute(&fixture.pool).await?;
    assert!(fixture
        .state
        .signup
        .signup_email("h3a-signup@example.invalid", "Passw0rd!")
        .await
        .is_err());
    let counts: (i64, i64, i64) = sqlx::query_as("select (select count(*) from users where primary_email='h3a-signup@example.invalid'), (select count(*) from user_identities where email='h3a-signup@example.invalid'), (select count(*) from email_verifications where email='h3a-signup@example.invalid')")
        .fetch_one(&fixture.pool).await?;
    assert_eq!(counts, (0, 0, 0));
    fixture.shutdown().await
}

#[tokio::test]
async fn login_use_case_rolls_back_session_and_last_login_when_audit_fails() -> Result<()> {
    let fixture = Fixture::start(None).await?;
    let user = insert_active_user(&fixture.pool, "h3a-login@example.invalid").await?;
    sqlx::raw_sql("create function h3a_reject_login_audit() returns trigger language plpgsql as $$ begin raise exception 'injected login audit failure'; end $$; create trigger h3a_reject_login_audit before insert on login_audit for each row execute function h3a_reject_login_audit();")
        .execute(&fixture.pool).await?;
    assert!(fixture
        .state
        .login
        .login(&user.primary_email, "Passw0rd!", None, None)
        .await
        .is_err());
    let state: (i64, Option<chrono::DateTime<Utc>>) = sqlx::query_as(
        "select (select count(*) from refresh_tokens where user_id=$1), last_login_at from users where id=$1",
    )
    .bind(user.id)
    .fetch_one(&fixture.pool)
    .await?;
    assert_eq!(state, (0, None));
    fixture.shutdown().await
}

#[tokio::test]
async fn busy_reset_admission_preserves_the_one_time_token() -> Result<()> {
    let password_work = Arc::new(AdmittedArgon2PasswordWork::new(PasswordWorkLimits {
        workers: 3,
        login_queue: 0,
        signup_workers: 1,
        signup_queue: 0,
        reset_workers: 1,
        reset_queue: 0,
    })?);
    let fixture = Fixture::start(Some(password_work.clone())).await?;
    let user = insert_active_user(&fixture.pool, "h3a-busy-reset@example.invalid").await?;
    let token = "h3a-busy-reset-token";
    let token_hash = raw_reset_hash(token);
    sqlx::query("insert into password_reset_tokens (id,user_id,token_hash,expires_at) values ($1,$2,$3,now()+interval '1 hour')")
        .bind(Uuid::new_v4()).bind(user.id).bind(&token_hash)
        .execute(&fixture.pool).await?;

    let slot = occupy(password_work.admission(PasswordWorkKind::Reset)).await;
    let error = fixture
        .state
        .password_reset
        .confirm_reset(token, "NewPassw0rd!")
        .await
        .expect_err("reset admission is full");
    assert!(matches!(
        Some(&error),
        Some(auth_service::application::errors::AuthError::Busy)
    ));
    let remaining: i64 =
        sqlx::query_scalar("select count(*) from password_reset_tokens where token_hash=$1")
            .bind(&token_hash)
            .fetch_one(&fixture.pool)
            .await?;
    assert_eq!(remaining, 1);

    slot.release().await;
    fixture
        .state
        .password_reset
        .confirm_reset(token, "NewPassw0rd!")
        .await?;
    fixture.shutdown().await
}

#[tokio::test]
async fn reset_use_case_rolls_back_token_password_and_sessions_on_audit_failure() -> Result<()> {
    let fixture = Fixture::start(None).await?;
    let user = insert_active_user(&fixture.pool, "h3a-reset@example.invalid").await?;
    let token = "h3a-reset-token";
    let token_hash = raw_reset_hash(token);
    sqlx::query("insert into password_reset_tokens (id,user_id,token_hash,expires_at) values ($1,$2,$3,now()+interval '1 hour')")
        .bind(Uuid::new_v4()).bind(user.id).bind(&token_hash)
        .execute(&fixture.pool).await?;
    let refresh = PgRefreshTokenStore::new(
        fixture.pool.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    let record = RefreshTokenRecord {
        jti: "h3a-before-reset".into(),
        token_hash: "h3a-refresh-hash".into(),
        user_id: user.id,
        family_id: Uuid::new_v4(),
        expires_at: Utc::now() + Duration::hours(1),
    };
    refresh.store(&(&record).into()).await?;
    sqlx::raw_sql("create function h3a_reject_reset_audit() returns trigger language plpgsql as $$ begin if new.action='PASSWORD_RESET_COMPLETED' then raise exception 'injected reset audit failure'; end if; return new; end $$; create trigger h3a_reject_reset_audit before insert on auth_audit_events for each row execute function h3a_reject_reset_audit();")
        .execute(&fixture.pool).await?;

    assert!(fixture
        .state
        .password_reset
        .confirm_reset(token, "NewPassw0rd!")
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, String>("select password_hash from users where id=$1")
            .bind(user.id)
            .fetch_one(&fixture.pool)
            .await?,
        user.password_hash
    );
    assert!(refresh.find(&record.jti).await?.is_some());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from password_reset_tokens where token_hash=$1"
        )
        .bind(&token_hash)
        .fetch_one(&fixture.pool)
        .await?,
        1
    );

    sqlx::raw_sql("drop trigger h3a_reject_reset_audit on auth_audit_events; drop function h3a_reject_reset_audit();")
        .execute(&fixture.pool).await?;
    fixture
        .state
        .password_reset
        .confirm_reset(token, "NewPassw0rd!")
        .await?;
    assert!(refresh.find(&record.jti).await?.is_none());
    fixture.shutdown().await
}
