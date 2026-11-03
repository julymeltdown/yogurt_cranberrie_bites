#![allow(dead_code)]

use anyhow::{Context, Result};
use notification_service::adapters::r#in::grpc::server::NotificationGrpcServer;
use notification_service::config::{build_state, connect_pool, AppConfig, AppState};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use std::net::IpAddr;
use std::sync::Arc;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;

const LOCAL_DATABASE_URL_ENV: &str = "NOTIFICATION_TEST_DATABASE_URL";
const LOCAL_DATABASE_WRITE_OPT_IN_ENV: &str = "NOTIFICATION_TEST_ALLOW_LOCAL_PG";
const LOCAL_DATABASE_PREFIX: &str = "port_rust_notification_test";
const LOCAL_SCHEMA_PREFIX: &str = "notification_fixture_";

#[must_use = "TestApp owns PostgreSQL fixture state; call TestApp::shutdown().await"]
pub struct TestApp {
    pub pool: PgPool,
    pub state: Arc<AppState>,
    cleanup: TestAppCleanup,
}

impl TestApp {
    pub async fn shutdown(self) -> Result<()> {
        let Self {
            pool,
            state,
            mut cleanup,
        } = self;
        drop((pool, state));
        cleanup.shutdown().await
    }
}

enum TestPostgresBacking {
    Container(Box<ContainerAsync<Postgres>>),
    LocalSchema {
        admin_options: PgConnectOptions,
        schema: String,
    },
    OwnedDatabase {
        admin_options: PgConnectOptions,
        database: String,
        system_identifier: String,
    },
}

struct TestAppCleanup {
    pool: Option<PgPool>,
    backing: Option<TestPostgresBacking>,
}

impl TestAppCleanup {
    async fn shutdown(&mut self) -> Result<()> {
        let Some(job) = self.take_job() else {
            return Ok(());
        };
        job.run().await
    }

    fn take_job(&mut self) -> Option<TestAppCleanupJob> {
        let backing = self.backing.take()?;
        Some(TestAppCleanupJob {
            pool: self.pool.take(),
            backing,
        })
    }
}

impl Drop for TestAppCleanup {
    fn drop(&mut self) {
        let Some(job) = self.take_job() else {
            return;
        };
        let cleanup = std::thread::Builder::new()
            .name("notification-test-cleanup".into())
            .spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("notification test cleanup runtime")
                    .block_on(job.run())
            });
        let panicking = std::thread::panicking();
        let result = match cleanup {
            Ok(thread) => match thread.join() {
                Ok(result) => result,
                Err(_) => Err(anyhow::anyhow!("notification test cleanup thread panicked")),
            },
            Err(error) => Err(anyhow::anyhow!(
                "failed to start notification test cleanup thread: {error}"
            )),
        };
        if let Err(error) = result {
            if panicking {
                eprintln!("notification test cleanup failed during panic: {error:#}");
            } else {
                panic!("notification test cleanup failed: {error:#}");
            }
        }
    }
}

struct TestAppCleanupJob {
    pool: Option<PgPool>,
    backing: TestPostgresBacking,
}

impl TestAppCleanupJob {
    async fn run(self) -> Result<()> {
        if let Some(pool) = self.pool {
            pool.close().await;
        }
        match self.backing {
            TestPostgresBacking::OwnedDatabase {
                admin_options,
                database,
                system_identifier,
            } => {
                // Drop cleanup runs on its own reactor; reuse options, never a
                // pool whose sockets belong to the blocked parent runtime.
                let admin_pool = PgPoolOptions::new()
                    .max_connections(1)
                    .connect_with(admin_options)
                    .await?;
                anyhow::ensure!(
                    database.starts_with(&format!("{LOCAL_DATABASE_PREFIX}_")),
                    "refusing to drop non-fixture database"
                );
                let actual: String =
                    sqlx::query_scalar("select system_identifier::text from pg_control_system()")
                        .fetch_one(&admin_pool)
                        .await?;
                anyhow::ensure!(
                    actual == system_identifier,
                    "Notification cleanup lost owned cluster identity"
                );
                let result = sqlx::query(&format!("drop database {}", quote_identifier(&database)))
                    .execute(&admin_pool)
                    .await;
                admin_pool.close().await;
                result
                    .map(|_| ())
                    .context("remove owned Notification fixture database")
            }
            TestPostgresBacking::Container(postgres) => {
                let postgres = *postgres;
                postgres.rm().await.map_err(|error| {
                    anyhow::anyhow!("failed to remove notification test Postgres: {error}")
                })
            }
            TestPostgresBacking::LocalSchema {
                admin_options,
                schema,
            } => {
                let admin_pool = PgPoolOptions::new()
                    .max_connections(1)
                    .connect_with(admin_options)
                    .await?;
                anyhow::ensure!(
                    schema.starts_with(LOCAL_SCHEMA_PREFIX),
                    "refusing to drop non-fixture PostgreSQL schema {schema}"
                );
                let sql = format!(
                    "drop schema if exists {} cascade",
                    quote_identifier(&schema)
                );
                let result = sqlx::query(&sql)
                    .execute(&admin_pool)
                    .await
                    .with_context(|| {
                        format!("failed to drop notification fixture schema {schema}")
                    });
                admin_pool.close().await;
                result.map(|_| ())
            }
        }
    }
}

/// Callers accepted by the test server: the edge, the feed fan-out sender, and
/// the account-deletion coordinator.
fn test_caller_registry() -> platform::grpc_auth::CallerRegistry {
    platform::grpc_auth::CallerRegistry::new()
        .register(
            platform::grpc_auth::Workload::Gateway,
            "test-gateway-credential",
        )
        .expect("gateway caller")
        .register(platform::grpc_auth::Workload::Post, "test-post-credential")
        .expect("post caller")
        .register(platform::grpc_auth::Workload::Auth, "test-auth-credential")
        .expect("auth caller")
}

pub async fn start_postgres_app() -> Result<TestApp> {
    let (backing, database_url) = provision_postgres().await?;
    let mut cleanup = TestAppCleanup {
        pool: None,
        backing: Some(backing),
    };
    let config = AppConfig {
        database_migration_mode: migration_runtime::MigrationMode::Startup,
        http_addr: "127.0.0.1:18084".parse()?,
        metrics_addr: "127.0.0.1:19084".parse()?,
        grpc_addr: "127.0.0.1:15054".parse()?,
        database_pool_max_connections: 5,
        database_url,
        actor_delegation: platform::actor_delegation::ActorDelegationVerifier::local_parity(
            platform::grpc_auth::Workload::Notification,
        ),
        caller_registry: test_caller_registry(),
        tls_material: None,
        retention_purge_worker_enabled: true,
        retention_purge_worker_interval_ms: 1000,
        retention_purge_actor_id: "99999999-9999-9999-9999-999999999999".into(),
    };
    let pool = connect_pool(&config).await?;
    cleanup.pool = Some(pool.clone());
    let state = Arc::new(build_state(&config, pool.clone()).await?);
    Ok(TestApp {
        pool,
        state,
        cleanup,
    })
}

async fn provision_postgres() -> Result<(TestPostgresBacking, String)> {
    let local = local_database_url()?;
    let flag = std::env::var("NOTIFICATION_TEST_CREATE_ISOLATED_DATABASES").ok();
    if owned_database_mode(local.is_some(), flag.as_deref())? {
        return provision_owned_database(local.as_deref().expect("validated owned database URL"))
            .await;
    }
    match local {
        Some(database_url) => provision_local_schema(&database_url).await,
        None => provision_testcontainer().await,
    }
}

pub(crate) fn owned_database_mode(has_url: bool, flag: Option<&str>) -> Result<bool> {
    match (has_url, flag) {
        (_, None) => Ok(false),
        (true, Some("1")) => Ok(true),
        _ => anyhow::bail!(
            "Notification owned database mode requires a local test URL and explicit flag1"
        ),
    }
}

async fn provision_owned_database(database_url: &str) -> Result<(TestPostgresBacking, String)> {
    let options = validate_local_database_url(database_url)?;
    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(options.clone())
        .await?;
    let system_identifier: String =
        sqlx::query_scalar("select system_identifier::text from pg_control_system()")
            .fetch_one(&admin_pool)
            .await?;
    anyhow::ensure!(
        std::env::var("NOTIFICATION_TEST_INITDB_SYSTEM_IDENTIFIER").as_deref()
            == Ok(&system_identifier),
        "Notification fixture must use runner-owned initdb cluster"
    );
    let database = format!("{LOCAL_DATABASE_PREFIX}_{}", Uuid::new_v4().simple());
    sqlx::query(&format!(
        "create database {} template template0",
        quote_identifier(&database)
    ))
    .execute(&admin_pool)
    .await?;
    let backing = TestPostgresBacking::OwnedDatabase {
        admin_options: options,
        database: database.clone(),
        system_identifier: system_identifier.clone(),
    };
    // Own cleanup before any fallible verification after CREATE DATABASE.
    let mut cleanup = TestAppCleanup {
        pool: None,
        backing: Some(backing),
    };
    admin_pool.close().await;
    let base = database_url
        .rsplit_once('/')
        .ok_or_else(|| anyhow::anyhow!("Notification fixture URL"))?
        .0;
    let isolated_url = format!("{base}/{database}");
    let observer = PgPool::connect(&isolated_url).await?;
    cleanup.pool = Some(observer.clone());
    let empty: bool =
        sqlx::query_scalar("select not exists(select 1 from pg_tables where schemaname='public')")
            .fetch_one(&observer)
            .await?;
    observer.close().await;
    anyhow::ensure!(
        empty,
        "Notification fixture must be empty before migrations"
    );
    println!(
        "NOTIFICATION_FRESH_FIXTURE {}",
        serde_json::json!({"database":database,"systemIdentifier":system_identifier,"template":"template0","emptyBeforeMigrations":empty})
    );
    Ok((
        cleanup.backing.take().expect("owned fixture backing"),
        isolated_url,
    ))
}

fn local_database_url() -> Result<Option<String>> {
    let database_url = std::env::var(LOCAL_DATABASE_URL_ENV)
        .ok()
        .and_then(|value| {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_owned())
        });
    let write_opt_in = std::env::var(LOCAL_DATABASE_WRITE_OPT_IN_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty());
    match (database_url, write_opt_in.as_deref()) {
        (None, None) => Ok(None),
        (Some(database_url), Some("1")) => Ok(Some(database_url)),
        (Some(_), _) => anyhow::bail!(
            "{LOCAL_DATABASE_URL_ENV} requires explicit {LOCAL_DATABASE_WRITE_OPT_IN_ENV}=1"
        ),
        (None, Some(_)) => anyhow::bail!(
            "{LOCAL_DATABASE_WRITE_OPT_IN_ENV} was set without {LOCAL_DATABASE_URL_ENV}"
        ),
    }
}

async fn provision_testcontainer() -> Result<(TestPostgresBacking, String)> {
    let (postgres, base_url) = test_support::postgres::start_postgres_with_url().await?;
    let _: PgConnectOptions = base_url
        .parse()
        .context("parse notification testcontainer PostgreSQL URL")?;
    let application_name = fixture_name();
    let database_url = scoped_database_url(&base_url, &application_name, None);
    Ok((
        TestPostgresBacking::Container(Box::new(postgres)),
        database_url,
    ))
}

async fn provision_local_schema(database_url: &str) -> Result<(TestPostgresBacking, String)> {
    let options = validate_local_database_url(database_url)?;

    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(options.clone())
        .await
        .map_err(|_| anyhow::anyhow!("failed to connect to local notification test PostgreSQL"))?;
    let schema = fixture_name();
    let create_sql = format!("create schema {}", quote_identifier(&schema));
    sqlx::query(&create_sql)
        .execute(&admin_pool)
        .await
        .with_context(|| format!("create notification fixture schema {schema}"))?;
    let fixture_url = scoped_database_url(database_url, &schema, Some(&schema));
    let mut cleanup = TestAppCleanup {
        pool: None,
        backing: Some(TestPostgresBacking::LocalSchema {
            admin_options: options,
            schema,
        }),
    };
    admin_pool.close().await;
    Ok((
        cleanup.backing.take().expect("local fixture backing"),
        fixture_url,
    ))
}

fn validate_local_database_url(database_url: &str) -> Result<PgConnectOptions> {
    let raw_database = validate_raw_local_database_url(database_url)?;
    let options: PgConnectOptions = database_url
        .parse()
        .map_err(|_| anyhow::anyhow!("{LOCAL_DATABASE_URL_ENV} must be a valid PostgreSQL URL"))?;
    validate_local_database_options(&options, raw_database)?;
    Ok(options)
}

fn validate_raw_local_database_url(database_url: &str) -> Result<&str> {
    anyhow::ensure!(
        !database_url.contains('?') && !database_url.contains('#'),
        "{LOCAL_DATABASE_URL_ENV} must not contain query parameters or a fragment"
    );
    let (scheme, remainder) = database_url.split_once("://").ok_or_else(|| {
        anyhow::anyhow!("{LOCAL_DATABASE_URL_ENV} must be a valid PostgreSQL URL")
    })?;
    anyhow::ensure!(
        matches!(scheme, "postgres" | "postgresql"),
        "{LOCAL_DATABASE_URL_ENV} must be a valid PostgreSQL URL"
    );
    let (_, raw_database) = remainder.split_once('/').ok_or_else(|| {
        anyhow::anyhow!("{LOCAL_DATABASE_URL_ENV} must name a canonical test database")
    })?;
    anyhow::ensure!(
        is_canonical_test_database_name(raw_database),
        "{LOCAL_DATABASE_URL_ENV} must name a canonical test database"
    );
    Ok(raw_database)
}

fn is_canonical_test_database_name(database: &str) -> bool {
    if database.len() > 63 || !database.is_ascii() || !database.starts_with(LOCAL_DATABASE_PREFIX) {
        return false;
    }
    let suffix = &database[LOCAL_DATABASE_PREFIX.len()..];
    if suffix.is_empty() {
        return true;
    }
    suffix.strip_prefix('_').is_some_and(|suffix| {
        !suffix.is_empty()
            && suffix.split('_').all(|segment| {
                !segment.is_empty()
                    && segment
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            })
    })
}

fn validate_local_database_options(options: &PgConnectOptions, raw_database: &str) -> Result<()> {
    anyhow::ensure!(
        options.get_socket().is_none(),
        "{LOCAL_DATABASE_URL_ENV} must use TCP loopback"
    );
    let host = options.get_host();
    let normalized_host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    let loopback = normalized_host.eq_ignore_ascii_case("localhost")
        || normalized_host
            .parse::<IpAddr>()
            .map(|address| address.is_loopback())
            .unwrap_or(false);
    anyhow::ensure!(loopback, "{LOCAL_DATABASE_URL_ENV} must use TCP loopback");
    anyhow::ensure!(
        options.get_database() == Some(raw_database),
        "{LOCAL_DATABASE_URL_ENV} must name a canonical test database"
    );
    anyhow::ensure!(
        options.get_options().is_none(),
        "{LOCAL_DATABASE_URL_ENV} must not provide connection overrides"
    );
    anyhow::ensure!(
        options.get_application_name().is_none(),
        "{LOCAL_DATABASE_URL_ENV} must not provide connection overrides"
    );
    Ok(())
}

pub(crate) fn validate_local_database_url_for_test(database_url: &str) -> Result<PgConnectOptions> {
    validate_local_database_url(database_url)
}

pub(crate) fn scoped_database_url_for_test(
    base_url: &str,
    application_name: &str,
    schema: Option<&str>,
) -> String {
    scoped_database_url(base_url, application_name, schema)
}

fn scoped_database_url(base_url: &str, application_name: &str, schema: Option<&str>) -> String {
    let separator = if base_url.contains('?') {
        if base_url.ends_with('?') || base_url.ends_with('&') {
            ""
        } else {
            "&"
        }
    } else {
        "?"
    };
    let mut url = format!("{base_url}{separator}application_name={application_name}");
    if let Some(schema) = schema {
        url.push_str("&options=-c%20search_path%3D");
        url.push_str(schema);
        url.push_str("%2Cpg_catalog");
    }
    url
}

fn fixture_name() -> String {
    format!("{LOCAL_SCHEMA_PREFIX}{}", Uuid::new_v4().simple())
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

pub fn grpc_server(state: Arc<AppState>) -> NotificationGrpcServer {
    NotificationGrpcServer::new(state)
}
