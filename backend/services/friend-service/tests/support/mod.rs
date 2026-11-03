#![allow(dead_code)]

pub mod profile_grpc;

use anyhow::Result;
use friend_service::adapters::r#in::grpc::server::FriendGrpcServer;
use friend_service::application::ports::{ProfileDiscoveryPort, ProfilePrivacyPort};
use friend_service::config::{build_state_with_dependencies, connect_pool, AppConfig, AppState};
use friend_service::domain::follow_request::AccountVisibility;
use friend_service::domain::recommendation::ProfileDiscoverySnapshot;
use std::sync::Arc;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;

pub struct TestApp {
    pub _postgres: Option<ContainerAsync<Postgres>>,
    pub pool: sqlx::PgPool,
    pub state: Arc<AppState>,
    cleanup: Option<OwnedDatabaseCleanup>,
}

struct OwnedDatabaseCleanup {
    pool: Option<sqlx::PgPool>,
    admin_options: sqlx::postgres::PgConnectOptions,
    database: String,
    system_identifier: String,
    armed: bool,
}

impl OwnedDatabaseCleanup {
    async fn shutdown(&mut self) -> Result<()> {
        if !self.armed {
            return Ok(());
        }
        if let Some(pool) = self.pool.take() {
            close_fixture_connections(&pool).await?;
        }
        let admin = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with(self.admin_options.clone())
            .await?;
        let actual: String =
            sqlx::query_scalar("select system_identifier::text from pg_control_system()")
                .fetch_one(&admin)
                .await?;
        anyhow::ensure!(
            actual == self.system_identifier,
            "Friend cleanup lost owned cluster identity"
        );
        anyhow::ensure!(
            self.database.starts_with("port_rust_friend_fixture_")
                && self
                    .database
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
            "refusing to drop non-fixture database"
        );
        let result = sqlx::query(&format!("drop database {}", self.database))
            .execute(&admin)
            .await;
        admin.close().await;
        result?;
        self.armed = false;
        Ok(())
    }
}

impl Drop for OwnedDatabaseCleanup {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let mut job = Self {
            pool: self.pool.take(),
            admin_options: self.admin_options.clone(),
            database: self.database.clone(),
            system_identifier: self.system_identifier.clone(),
            armed: true,
        };
        self.armed = false;
        let result = std::thread::Builder::new()
            .name("friend-test-cleanup".into())
            .spawn(move || {
                let result = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("friend cleanup runtime")
                    .block_on(job.shutdown());
                // A failed job is terminal; never recursively spawn cleanup on drop.
                job.armed = false;
                result
            })
            .map_err(anyhow::Error::from)
            .and_then(|thread| {
                thread
                    .join()
                    .map_err(|_| anyhow::anyhow!("friend cleanup thread panicked"))?
            });
        if let Err(error) = result {
            if std::thread::panicking() {
                eprintln!("Friend cleanup failed during panic: {error:#}");
            } else {
                panic!("Friend cleanup failed: {error:#}");
            }
        }
    }
}

// Quiesce every pool permit before graceful protocol termination. Pool::close
// alone can race SQLx's spawned return-to-pool ping following a failed query.
// Keep the ordinary acquire timeout and surface every connection close error.
async fn close_fixture_connections(pool: &sqlx::PgPool) -> Result<()> {
    if !pool.is_closed() {
        let mut connections = Vec::new();
        for _ in 0..pool.options().get_max_connections() {
            connections.push(pool.acquire().await?);
        }
        for connection in connections {
            connection.close().await?;
        }
    }
    pool.close().await;
    Ok(())
}

impl TestApp {
    pub async fn shutdown(self) -> Result<()> {
        let Self {
            _postgres,
            pool,
            state,
            mut cleanup,
        } = self;
        drop(state);
        close_fixture_connections(&pool).await?;
        if let Some(cleanup) = cleanup.as_mut() {
            cleanup.shutdown().await?;
        }
        drop(_postgres);
        Ok(())
    }
}

/// Callers accepted by the test server: the edge, the feed reader, and the
/// account-deletion coordinator.
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

pub async fn start_postgres_app(seed_enabled: bool) -> Result<TestApp> {
    start_postgres_app_with_dependencies(
        seed_enabled,
        Arc::new(DeterministicProfilePrivacy::public()),
        Arc::new(EmptyProfileDiscovery),
    )
    .await
}

pub async fn start_postgres_app_with_privacy(
    seed_enabled: bool,
    response: ProfilePrivacyResponse,
) -> Result<TestApp> {
    start_postgres_app_with_dependencies(
        seed_enabled,
        Arc::new(DeterministicProfilePrivacy { response }),
        Arc::new(EmptyProfileDiscovery),
    )
    .await
}

pub async fn start_postgres_app_with_dependencies(
    seed_enabled: bool,
    privacy: Arc<dyn ProfilePrivacyPort>,
    discovery: Arc<dyn ProfileDiscoveryPort>,
) -> Result<TestApp> {
    // Testcontainers remains the default. A dedicated external database lets
    // real-PostgreSQL acceptance run in environments where Docker is absent.
    let external_database_url = std::env::var("FRIEND_TEST_DATABASE_URL")
        .ok()
        .filter(|url| !url.trim().is_empty());
    let (postgres, database_url) = match external_database_url {
        Some(database_url) => (None, database_url),
        None => {
            let (postgres, database_url) =
                test_support::postgres::start_postgres_with_url().await?;
            (Some(postgres), database_url)
        }
    };
    let mut cleanup = None;
    let database_url = if postgres.is_none()
        && std::env::var("FRIEND_TEST_CREATE_ISOLATED_DATABASES").as_deref() == Ok("1")
    {
        use sqlx::postgres::PgConnectOptions;
        use std::net::IpAddr;
        let options: PgConnectOptions = database_url.parse()?;
        let host = options
            .get_host()
            .trim_start_matches('[')
            .trim_end_matches(']');
        anyhow::ensure!(
            host == "localhost" || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()),
            "isolated Friend fixtures require loopback"
        );
        anyhow::ensure!(
            options.get_socket().is_none()
                && options.get_options().is_none()
                && !database_url.contains('?')
                && !database_url.contains('#'),
            "fixture overrides forbidden"
        );
        anyhow::ensure!(
            options
                .get_database()
                .is_some_and(|db| db.starts_with("port_rust_friend_test_")),
            "owned Friend parent database required"
        );
        let admin = sqlx::PgPool::connect(&database_url).await?;
        let observed: String =
            sqlx::query_scalar("select system_identifier::text from pg_control_system()")
                .fetch_one(&admin)
                .await?;
        anyhow::ensure!(
            std::env::var("FRIEND_TEST_INITDB_SYSTEM_IDENTIFIER").as_deref()
                == Ok(observed.as_str()),
            "observed initdb lineage differs"
        );
        let database = format!("port_rust_friend_fixture_{}", Uuid::new_v4().simple());
        sqlx::query(&format!("create database {database} template template0"))
            .execute(&admin)
            .await?;
        cleanup = Some(OwnedDatabaseCleanup {
            pool: None,
            admin_options: options.clone(),
            database: database.clone(),
            system_identifier: observed.clone(),
            armed: true,
        });
        admin.close().await;
        let isolated = options.database(&database);
        let probe = sqlx::PgPool::connect_with(isolated.clone()).await?;
        let empty: bool = sqlx::query_scalar(
            "select not exists(select 1 from pg_tables where schemaname='public')",
        )
        .fetch_one(&probe)
        .await?;
        anyhow::ensure!(empty, "new fixture contains tables before migrations");
        probe.close().await;
        println!("FRIEND_FRESH_FIXTURE {{\"database\":\"{database}\",\"systemIdentifier\":\"{observed}\",\"template\":\"template0\",\"emptyBeforeMigrations\":true}}");
        // Preserve the validated parent URL's credential and authority, changing
        // only its controlled database path. These native URLs have no overrides.
        let mut url = database_url
            .rsplit_once('/')
            .ok_or_else(|| anyhow::anyhow!("invalid fixture URL"))?
            .0
            .to_string();
        url.push('/');
        url.push_str(&database);
        url
    } else {
        database_url
    };
    let config = AppConfig {
        database_migration_mode: Default::default(), // Compatibility startup mode; also included by full-stack-e2e.
        http_addr: "127.0.0.1:18083".parse()?,
        metrics_addr: "127.0.0.1:0".parse()?,
        grpc_addr: "127.0.0.1:15053".parse()?,
        database_pool_max_connections: 5,
        database_pool_acquire_timeout_ms: 30_000,
        database_url,
        profile_grpc_endpoint: "http://profile.invalid:50052".into(),
        seed_enabled,
        actor_delegation: platform::actor_delegation::ActorDelegationVerifier::local_parity(
            platform::grpc_auth::Workload::Friend,
        ),
        caller_registry: test_caller_registry(),
        outbound_identity: "test-internal-service-token".into(),
        tls_material: None,
        recommendation_worker_enabled: false,
        recommendation_worker_interval_ms: 5_000,
        recommendation_worker_batch_size: 100,
        recommendation_worker_lease_timeout_ms: 300_000,
        recommendation_worker_max_attempts: 5,
        recommendation_worker_base_retry_ms: 1_000,
        recommendation_retention_worker_enabled: false,
        recommendation_retention_worker_interval_ms: 3_600_000,
        recommendation_retention_batch_size: 500,
        recommendation_event_retention_days: 90,
        mutation_receipt_retention_worker_enabled: false,
        mutation_receipt_retention_worker_interval_ms: 3_600_000,
        mutation_receipt_retention_batch_size: 1_000,
        people_rank_version: "people-linear-v1".into(),
        people_ranker_adapter: "LINEAR".into(),
        people_embedding_source_enabled: false,
        people_recent_sequence_enabled: false,
        people_recent_sequence_limit: 20,
    };
    let pool = connect_pool(&config).await?;
    if let Some(cleanup) = cleanup.as_mut() {
        cleanup.pool = Some(pool.clone());
    }
    let state =
        Arc::new(build_state_with_dependencies(&config, pool.clone(), privacy, discovery).await?);
    Ok(TestApp {
        _postgres: postgres,
        pool,
        state,
        cleanup,
    })
}

pub fn grpc_server(state: Arc<AppState>) -> FriendGrpcServer {
    FriendGrpcServer::new(state)
}

#[derive(Clone, Copy)]
pub enum ProfilePrivacyResponse {
    Public,
    Private,
    Unavailable,
}

#[derive(Clone, Copy)]
struct DeterministicProfilePrivacy {
    response: ProfilePrivacyResponse,
}

impl DeterministicProfilePrivacy {
    fn public() -> Self {
        Self {
            response: ProfilePrivacyResponse::Public,
        }
    }
}

#[async_trait::async_trait]
impl ProfilePrivacyPort for DeterministicProfilePrivacy {
    async fn account_visibility(&self, _user_id: Uuid) -> Result<AccountVisibility> {
        match self.response {
            ProfilePrivacyResponse::Public => Ok(AccountVisibility::Public),
            ProfilePrivacyResponse::Private => Ok(AccountVisibility::Private),
            ProfilePrivacyResponse::Unavailable => anyhow::bail!("profile privacy unavailable"),
        }
    }
}

#[derive(Clone, Copy)]
struct EmptyProfileDiscovery;

#[async_trait::async_trait]
impl ProfileDiscoveryPort for EmptyProfileDiscovery {
    async fn batch_profiles(&self, _user_ids: &[Uuid]) -> Result<Vec<ProfileDiscoverySnapshot>> {
        Ok(Vec::new())
    }
}
