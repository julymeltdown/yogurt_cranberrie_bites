use anyhow::{Context, Result};
use auth_service::{
    adapters::out::grpc::user_deletion_dispatcher::GrpcUserDeletionDispatcher,
    application::ports::UserDeletionDispatcherPort, domain::user_deletion::UserDeletionTarget,
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use futures::FutureExt;
use notification_service::adapters::r#in::grpc::server::proto as notification_proto;
use platform::grpc_auth::{ServiceIdentityClientInterceptor, Workload};
use post_service::adapters::out::messaging::kafka_publisher::{stable_partition, EventEnvelope};
use reqwest::Method;
use rskafka::client::partition::{OffsetAt, UnknownTopicHandling};
use rskafka::client::ClientBuilder;
use serde_json::{json, Value};
use sqlx::{migrate::Migrator, postgres::PgPoolOptions, Row};
use std::{
    collections::{BTreeMap, BTreeSet},
    net::{SocketAddr, TcpListener as StdTcpListener},
    panic::AssertUnwindSafe,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Arc,
    time::Duration,
};
use test_support::{kafka, postgres, redis};
use testcontainers::core::{CmdWaitFor, ExecCommand};
use tokio::{
    net::TcpListener,
    sync::{oneshot, Mutex},
    task::JoinHandle,
    time::{sleep, timeout, Instant},
};
use uuid::Uuid;

#[path = "process_stack/external_migrations.rs"]
mod external_migrations;
#[path = "process_stack/forensics.rs"]
mod forensics;
#[path = "process_stack/runtime_roles.rs"]
mod runtime_roles;
#[path = "process_stack/setup_post.rs"]
mod setup_post;

type MailInbox = Arc<Mutex<Vec<Value>>>;
type MigrationLedger = Vec<(i64, Vec<u8>)>;
type NotificationSubjectMigrationState = (
    bool,
    bool,
    Option<String>,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
);
type NotificationRowMigrationState = (
    Option<DateTime<Utc>>,
    Option<Uuid>,
    Option<String>,
    Option<DateTime<Utc>>,
);
type LegacySuppressionMigrationState = (Uuid, String, DateTime<Utc>, Option<DateTime<Utc>>);

struct LegacyNotificationMigrationFixture {
    recipient_subject_id: Uuid,
    actor_subject_id: Uuid,
    recipient_notification_id: Uuid,
    actor_notification_id: Uuid,
    both_subjects_notification_id: Uuid,
    unaffected_notification_id: Uuid,
    recipient_suppressed_at: DateTime<Utc>,
    actor_suppressed_at: DateTime<Utc>,
    recipient_retention_until: DateTime<Utc>,
    actor_retention_until: DateTime<Utc>,
    existing_retention_until: DateTime<Utc>,
}

struct NotificationCompatibilityContext<'a> {
    readiness_client: &'a reqwest::Client,
    processes: &'a mut ManagedProcesses,
    pre_upgrade_addresses: ServiceAddresses,
    current_addresses: ServiceAddresses,
    service_addresses: &'a BTreeMap<&'a str, ServiceAddresses>,
    auth_environment: &'a BTreeMap<String, String>,
    post_environment: &'a BTreeMap<String, String>,
    gateway_environment: &'a BTreeMap<String, String>,
    notification_database_url: &'a str,
    legacy_fixture: &'a LegacyNotificationMigrationFixture,
}

const PROCESS_STACK_MIGRATION_SERVICES: [(&str, &str); 6] = [
    ("auth", "auth-service"),
    ("profile", "profile-service"),
    ("friend", "friend-service"),
    ("post", "post-service"),
    ("notification", "notification-service"),
    ("gateway", "api-gateway"),
];

const PROCESS_STACK_FEED_CACHE_EPOCH: u64 = 1;

struct ProcessUser {
    user_id: Uuid,
    access_token: String,
    refresh_cookie: String,
}

struct ProcessStackRelationshipGate {
    directory: PathBuf,
}

impl ProcessStackRelationshipGate {
    fn new() -> Result<Self> {
        let directory = std::env::temp_dir().join(format!(
            "port-rust-friend-relationship-gate-{}-{}",
            std::process::id(),
            Uuid::new_v4().simple()
        ));
        std::fs::create_dir(&directory)
            .context("could not create the process-stack relationship gate directory")?;
        Ok(Self { directory })
    }

    fn directory(&self) -> &PathBuf {
        &self.directory
    }

    fn arm(&self) -> Result<()> {
        std::fs::write(self.directory.join("armed"), b"arm")
            .context("could not arm the process-stack relationship gate")
    }

    fn release(&self) -> Result<()> {
        std::fs::write(self.directory.join("released"), b"release")
            .context("could not release the process-stack relationship gate")
    }

    async fn wait_until_entered(&self) -> Result<()> {
        timeout(Duration::from_secs(5), async {
            loop {
                if self.directory.join("entered").exists() {
                    return;
                }
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .context("FriendService did not reach the relationship-policy gate")
    }
}

impl Drop for ProcessStackRelationshipGate {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

struct ServiceProcessRestart<'a> {
    processes: &'a mut ManagedProcesses,
    binary_dir: &'a PathBuf,
    environment: &'a BTreeMap<String, String>,
    /// Production-valid journal budget used for the ordinary journey.
    ordinary_environment: &'a BTreeMap<String, String>,
    /// Short journal budget used only by the durable-queue recovery block.
    queue_environment: &'a BTreeMap<String, String>,
    auth_environment: &'a BTreeMap<String, String>,
    metrics_address: SocketAddr,
    auth_database_url: &'a str,
    auth_metrics_address: SocketAddr,
    gateway_database_url: &'a str,
    gateway_database_name: &'a str,
    gateway_restored_database_url: &'a str,
    gateway_restored_database_name: &'a str,
    postgres: &'a postgres::PostgresContainer,
    profile_database_url: &'a str,
    profile_metrics_address: SocketAddr,
    notification_database_url: &'a str,
    notification_grpc_address: SocketAddr,
    notification_metrics_address: SocketAddr,
    redis: &'a redis::RedisContainer,
    friend_database_url: &'a str,
    friend_environment: &'a BTreeMap<String, String>,
    friend_metrics_address: SocketAddr,
    post_database_url: &'a str,
    post_environment: &'a BTreeMap<String, String>,
    post_metrics_address: SocketAddr,
    kafka: &'a kafka::KafkaContainer,
    kafka_bootstrap: &'a str,
    kafka_topic: &'a str,
    kafka_partition_count: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PostKafkaDeliveryState {
    published: bool,
    dead_lettered: bool,
    locked: bool,
    external_job_leased: bool,
    external_lease_live: bool,
    external_job_attempts: i32,
    external_starts: i64,
    external_failures: i64,
    external_successes: i64,
    timeline_successes: i64,
    cache_successes: i64,
    internal_successes: i64,
    external_publications: i64,
    timeline_publications: i64,
    cache_publications: i64,
    internal_publications: i64,
}

#[derive(Clone)]
struct MailSinkState {
    inbox: MailInbox,
    bearer_token: String,
}

#[derive(Clone, Copy)]
struct ServiceAddresses {
    http: SocketAddr,
    metrics: SocketAddr,
    grpc: SocketAddr,
}

impl ServiceAddresses {
    fn allocate() -> Result<Self> {
        Ok(Self {
            http: ephemeral_loopback_addr()?,
            metrics: ephemeral_loopback_addr()?,
            grpc: ephemeral_loopback_addr()?,
        })
    }
}

struct ManagedProcess {
    name: &'static str,
    child: Child,
}

#[derive(Default)]
struct ManagedProcesses {
    children: Vec<ManagedProcess>,
}

impl ManagedProcesses {
    fn spawn(
        &mut self,
        name: &'static str,
        binary: PathBuf,
        environment: &BTreeMap<String, String>,
    ) -> Result<()> {
        anyhow::ensure!(
            binary.is_file(),
            "service executable is missing: {}",
            binary.display()
        );
        let child = Command::new(&binary)
            .env_clear()
            .envs(environment)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("failed to start {name} at {}", binary.display()))?;
        eprintln!("started {name} as pid {}", child.id());
        self.children.push(ManagedProcess { name, child });
        Ok(())
    }

    fn ensure_running(&mut self, name: &str) -> Result<()> {
        let process = self
            .children
            .iter_mut()
            .find(|process| process.name == name)
            .with_context(|| format!("managed process {name} was not started"))?;
        match process.child.try_wait()? {
            None => Ok(()),
            Some(status) => anyhow::bail!("{name} exited before becoming ready: {status}"),
        }
    }

    fn stop(&mut self, name: &str) -> Result<()> {
        let index = self
            .children
            .iter()
            .position(|process| process.name == name)
            .with_context(|| format!("managed process {name} was not started"))?;
        let process = self
            .children
            .get_mut(index)
            .context("managed process disappeared during stop")?;
        anyhow::ensure!(
            process.child.try_wait()?.is_none(),
            "{name} exited before the requested process stop"
        );
        process
            .child
            .kill()
            .with_context(|| format!("failed to stop {name}"))?;
        process
            .child
            .wait()
            .with_context(|| format!("failed to reap stopped process {name}"))?;
        self.children.remove(index);
        Ok(())
    }

    fn suspend(&mut self, name: &str) -> Result<()> {
        self.signal(name, "STOP")
    }

    fn resume(&mut self, name: &str) -> Result<()> {
        self.signal(name, "CONT")
    }

    fn signal(&mut self, name: &str, signal: &str) -> Result<()> {
        let process = self
            .children
            .iter_mut()
            .find(|process| process.name == name)
            .with_context(|| format!("managed process {name} was not started"))?;
        anyhow::ensure!(
            process.child.try_wait()?.is_none(),
            "{name} exited before signal {signal} was sent"
        );
        let status = Command::new("kill")
            .arg(format!("-{signal}"))
            .arg(process.child.id().to_string())
            .status()
            .with_context(|| format!("failed to send signal {signal} to {name}"))?;
        anyhow::ensure!(
            status.success(),
            "signal {signal} failed for managed process {name}"
        );
        Ok(())
    }
}

impl Drop for ManagedProcesses {
    fn drop(&mut self) {
        for process in self.children.iter_mut().rev() {
            let running = process
                .child
                .try_wait()
                .map(|status| status.is_none())
                .unwrap_or(true);
            if running {
                let _ = process.child.kill();
            }
            // `kill` is issued before the blocking wait, so teardown does not
            // wait indefinitely for a service that ignored graceful shutdown.
            let _ = process.child.wait();
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires release service binaries; use scripts/parity/run-process-stack-e2e.sh"]
async fn six_service_process_stack_runs_with_real_dependencies() -> Result<()> {
    let external_migrations = match std::env::var("PORT_RUST_PROCESS_MIGRATION_MODE") {
        Ok(value) if value == "startup" => false,
        Ok(value) if value == "external" => true,
        Err(std::env::VarError::NotPresent) => false,
        _ => anyhow::bail!("PORT_RUST_PROCESS_MIGRATION_MODE must be startup or external"),
    };
    eprintln!(
        "PROCESS_STACK_MIGRATION_MODE={}",
        if external_migrations {
            "external"
        } else {
            "startup"
        }
    );

    let focus_feed_block_race = match std::env::var("PORT_RUST_PROCESS_STACK_FOCUS") {
        Ok(value) if value == "feed-block-race" => true,
        Ok(value) => anyhow::bail!(
            "PORT_RUST_PROCESS_STACK_FOCUS must be feed-block-race when set, got {value:?}"
        ),
        Err(std::env::VarError::NotPresent) => false,
        Err(error) => return Err(error.into()),
    };
    let run_id = match std::env::var("PORT_RUST_PROCESS_RUN_ID") {
        Ok(value) => Uuid::parse_str(&value).context("PORT_RUST_PROCESS_RUN_ID must be a UUID")?,
        Err(std::env::VarError::NotPresent) => Uuid::new_v4(),
        Err(error) => return Err(error.into()),
    }
    .simple()
    .to_string();
    let relationship_gate = ProcessStackRelationshipGate::new()?;
    let binary_dir = std::env::var_os("PORT_RUST_BINARY_DIR")
        .map(PathBuf::from)
        .context("PORT_RUST_BINARY_DIR is unset; run scripts/parity/run-process-stack-e2e.sh")?;
    let migration_base_dir = std::env::var_os("PORT_RUST_MIGRATION_BASE_DIR")
        .map(PathBuf::from)
        .context(
            "PORT_RUST_MIGRATION_BASE_DIR is unset; run scripts/parity/run-process-stack-e2e.sh",
        )?;
    let migration_base_commit = std::env::var("PORT_RUST_MIGRATION_BASE_SHA").context(
        "PORT_RUST_MIGRATION_BASE_SHA is unset; run scripts/parity/run-process-stack-e2e.sh",
    )?;
    let expected_migration_deltas = match std::env::var("PORT_RUST_EXPECT_MIGRATION_DELTAS") {
        Ok(value) => Some(parse_expected_migration_deltas(&value)?),
        Err(std::env::VarError::NotPresent) => None,
        Err(error) => return Err(error.into()),
    };
    let pre_upgrade_notification_binary =
        std::env::var_os("PORT_RUST_PRE_UPGRADE_NOTIFICATION_BINARY").map(PathBuf::from);
    let expects_notification_four_delta = expected_migration_deltas
        .as_ref()
        .and_then(|deltas| deltas.get("notification"))
        .is_some_and(|versions| versions == &BTreeSet::from([4]));
    if let Some(binary) = pre_upgrade_notification_binary.as_ref() {
        anyhow::ensure!(
            migration_base_commit == "093b8bc007179d480a19360aab6a9e0120ec6304",
            "pre-upgrade Notification compatibility requires its pinned baseline commit"
        );
        anyhow::ensure!(
            expects_notification_four_delta,
            "pre-upgrade Notification compatibility requires notification:4 in the expected migration deltas"
        );
        anyhow::ensure!(
            binary.is_file(),
            "pre-upgrade Notification executable is missing: {}",
            binary.display()
        );
    }
    anyhow::ensure!(
        !expects_notification_four_delta || pre_upgrade_notification_binary.is_some(),
        "notification:4 migration acceptance requires the pre-upgrade Notification executable"
    );
    anyhow::ensure!(
        migration_base_dir.is_dir(),
        "migration baseline directory does not exist: {}",
        migration_base_dir.display()
    );
    anyhow::ensure!(
        !migration_base_commit.trim().is_empty(),
        "PORT_RUST_MIGRATION_BASE_SHA must not be empty"
    );
    eprintln!("process-stack migration baseline commit: {migration_base_commit}");
    let host_start = forensics::host_resources().await;
    eprintln!("PROCESS_STACK_HOST_START={host_start}");
    let pg = postgres::start_postgres()
        .await
        .context("starting the process-stack PostgreSQL container")?;
    eprintln!("PROCESS_STACK_POSTGRES_CONTAINER={}", pg.id());
    let redis_container = redis::start_redis()
        .await
        .context("starting the process-stack Redis container")?;
    let kafka_container = kafka::start_kafka()
        .await
        .context("starting the process-stack Kafka container")?;
    let kafka_bootstrap = kafka::kafka_bootstrap(&kafka_container).await?;
    kafka::await_kafka_ready(&kafka_bootstrap, Duration::from_secs(45)).await?;

    let pg_host = pg.get_host().await?;
    let pg_port = pg.get_host_port_ipv4(5432).await?;
    let admin_url = database_url(&pg_host, pg_port, "postgres");
    let admin_pool =
        postgres::with_startup_diagnostics(Some(&pg), "process-stack initial admin pool", async {
            Ok(PgPoolOptions::new()
                .max_connections(1)
                .connect(&admin_url)
                .await?)
        })
        .await?;
    let postgres_version: String = sqlx::query_scalar("show server_version")
        .fetch_one(&admin_pool)
        .await?;
    let postgres_version_number: String = sqlx::query_scalar("show server_version_num")
        .fetch_one(&admin_pool)
        .await?;
    let postgres_version_number: i32 = postgres_version_number.parse()?;
    anyhow::ensure!(
        (160_000..170_000).contains(&postgres_version_number),
        "process-stack PostgreSQL must match the PostgreSQL 16 parity substrate"
    );
    eprintln!(
        "PROCESS_STACK_POSTGRES_RUNTIME={}",
        serde_json::json!({
            "containerId": pg.id(),
            "requestedImage": "postgres:16",
            "serverVersion": postgres_version,
            "serverVersionNumber": postgres_version_number,
        })
    );
    let mut db_names = BTreeMap::from([
        ("auth", format!("pw16_auth_{}", &run_id[..12])),
        ("profile", format!("pw16_profile_{}", &run_id[..12])),
        ("friend", format!("pw16_friend_{}", &run_id[..12])),
        ("post", format!("pw16_post_{}", &run_id[..12])),
        (
            "notification",
            format!("pw16_notification_{}", &run_id[..12]),
        ),
        ("gateway", format!("pw16_gateway_{}", &run_id[..12])),
        (
            "gateway_restored",
            format!("pw16_gateway_restored_{}", &run_id[..12]),
        ),
    ]);
    if expects_notification_four_delta {
        db_names.insert(
            "notification_pre_upgrade",
            format!("pw16_notification_pre_upgrade_{}", &run_id[..12]),
        );
    }
    for name in db_names.values() {
        sqlx::query(&format!("create database {name}"))
            .execute(&admin_pool)
            .await?;
    }
    admin_pool.close().await;

    let migration_baselines =
        apply_migration_baselines(&migration_base_dir, &pg_host, pg_port, &db_names).await?;
    let legacy_notification_fixture = if expects_notification_four_delta {
        let source_database_url =
            database_url(&pg_host, pg_port, &db_names["notification_pre_upgrade"]);
        let fixture = seed_pre_upgrade_notification_business_data(&source_database_url).await?;
        restore_database_snapshot(
            &pg,
            &db_names["notification_pre_upgrade"],
            &db_names["notification"],
            &run_id,
            "notification",
        )
        .await?;
        assert_restored_pre_upgrade_notification_snapshot(
            &database_url(&pg_host, pg_port, &db_names["notification"]),
            migration_baselines
                .get("notification")
                .context("Notification migration baseline was not initialized")?,
            &fixture,
        )
        .await?;
        Some(fixture)
    } else {
        None
    };

    let redis_host = redis_container.get_host().await?;
    let redis_port = redis_container.get_host_port_ipv4(6379).await?;
    let redis_url = format!("redis://{redis_host}:{redis_port}/");
    let addresses = BTreeMap::from([
        ("profile", ServiceAddresses::allocate()?),
        ("notification", ServiceAddresses::allocate()?),
        ("friend", ServiceAddresses::allocate()?),
        ("post", ServiceAddresses::allocate()?),
        ("auth", ServiceAddresses::allocate()?),
        ("gateway", ServiceAddresses::allocate()?),
    ]);

    let mail_token = format!("mail-{run_id}");
    let inbox = Arc::new(Mutex::new(Vec::new()));
    let (mail_addr, mail_shutdown, mail_task) = start_mail_sink(MailSinkState {
        inbox: inbox.clone(),
        bearer_token: mail_token.clone(),
    })
    .await?;
    let mail_endpoint = format!("http://{mail_addr}/verification");

    let mut common = BTreeMap::new();
    common.insert("RUST_LOG".into(), "info,platform::readiness=debug".into());
    for workload in [
        "GATEWAY",
        "AUTH",
        "PROFILE",
        "FRIEND",
        "POST",
        "NOTIFICATION",
    ] {
        common.insert(
            format!("{workload}_SERVICE_CREDENTIAL"),
            format!("wp16-{run_id}-{workload}"),
        );
    }
    for service in [
        "AUTH",
        "PROFILE",
        "FRIEND",
        "POST",
        "NOTIFICATION",
        "GATEWAY",
    ] {
        common.insert(format!("{service}_RUNTIME_MODE"), "parity".into());
    }

    let mut profile_env = common.clone();
    insert_database(
        &mut profile_env,
        "PROFILE",
        &pg_host,
        pg_port,
        &db_names["profile"],
    );
    insert_addresses(&mut profile_env, "PROFILE", addresses["profile"]);

    let mut notification_env = common.clone();
    insert_database(
        &mut notification_env,
        "NOTIFICATION",
        &pg_host,
        pg_port,
        &db_names["notification"],
    );
    insert_addresses(
        &mut notification_env,
        "NOTIFICATION",
        addresses["notification"],
    );
    // Keep the production worker enabled, but tick quickly so this acceptance
    // can exercise the real purge path without waiting an hour.
    notification_env.insert(
        "NOTIFICATION_RETENTION_PURGE_WORKER_INTERVAL_MS".into(),
        "1000".into(),
    );

    let mut friend_env = common.clone();
    insert_database(
        &mut friend_env,
        "FRIEND",
        &pg_host,
        pg_port,
        &db_names["friend"],
    );
    insert_addresses(&mut friend_env, "FRIEND", addresses["friend"]);
    friend_env.insert(
        "FRIEND_PROFILE_GRPC_ENDPOINT".into(),
        grpc_endpoint(addresses["profile"].grpc),
    );
    friend_env.insert(
        "FRIEND_PROCESS_STACK_RELATIONSHIP_GATE_DIR".into(),
        relationship_gate.directory().display().to_string(),
    );

    let mut post_env = common.clone();
    insert_database(&mut post_env, "POST", &pg_host, pg_port, &db_names["post"]);
    insert_addresses(&mut post_env, "POST", addresses["post"]);
    post_env.insert("POST_REDIS_URL".into(), redis_url.clone());
    post_env.insert(
        "POST_FEED_CACHE_EPOCH".into(),
        PROCESS_STACK_FEED_CACHE_EPOCH.to_string(),
    );
    post_env.insert(
        "POST_PROFILE_GRPC_ENDPOINT".into(),
        grpc_endpoint(addresses["profile"].grpc),
    );
    post_env.insert(
        "POST_FRIEND_GRPC_ENDPOINT".into(),
        grpc_endpoint(addresses["friend"].grpc),
    );
    post_env.insert(
        "POST_NOTIFICATION_GRPC_ENDPOINT".into(),
        grpc_endpoint(addresses["notification"].grpc),
    );
    post_env.insert(
        "POST_GATEWAY_GRPC_ENDPOINT".into(),
        grpc_endpoint(addresses["gateway"].grpc),
    );
    let kafka_topic = format!("wp16-events-{run_id}");
    let kafka_partition_count = 6;
    post_env.insert("POST_EXTERNAL_PUBLISHER".into(), "kafka".into());
    post_env.insert(
        "POST_KAFKA_BOOTSTRAP_BROKERS".into(),
        kafka_bootstrap.clone(),
    );
    post_env.insert("POST_KAFKA_CREATE_TOPIC".into(), "true".into());
    post_env.insert("POST_KAFKA_TOPIC".into(), kafka_topic.clone());
    post_env.insert(
        "POST_KAFKA_PARTITION_COUNT".into(),
        kafka_partition_count.to_string(),
    );
    post_env.insert("POST_OUTBOX_WORKER_ENABLED".into(), "true".into());

    let mut auth_env = common.clone();
    insert_database(&mut auth_env, "AUTH", &pg_host, pg_port, &db_names["auth"]);
    insert_addresses(&mut auth_env, "AUTH", addresses["auth"]);
    // Keep a failed deletion step due far enough into the future to stop Auth
    // deterministically before restarting it. The test advances this one
    // disposable row after Friend is healthy again.
    auth_env.insert(
        "AUTH_USER_DELETION_WORKER_BASE_RETRY_MS".into(),
        "60000".into(),
    );
    for dependency in ["profile", "friend", "post", "notification"] {
        auth_env.insert(
            format!("AUTH_{}_GRPC_ENDPOINT", dependency.to_ascii_uppercase()),
            grpc_endpoint(addresses[dependency].grpc),
        );
    }
    auth_env.insert("AUTH_VERIFICATION_MAILER_ENDPOINT".into(), mail_endpoint);
    auth_env.insert(
        "AUTH_VERIFICATION_MAILER_BEARER_TOKEN".into(),
        mail_token.clone(),
    );

    let mut gateway_env = common;
    insert_database(
        &mut gateway_env,
        "GATEWAY",
        &pg_host,
        pg_port,
        &db_names["gateway"],
    );
    gateway_env.insert(
        "GATEWAY_HTTP_ADDR".into(),
        addresses["gateway"].http.to_string(),
    );
    gateway_env.insert(
        "GATEWAY_METRICS_ADDR".into(),
        addresses["gateway"].metrics.to_string(),
    );
    gateway_env.insert(
        "GATEWAY_GRPC_ADDR".into(),
        addresses["gateway"].grpc.to_string(),
    );
    for dependency in ["auth", "profile", "friend", "post", "notification"] {
        gateway_env.insert(
            format!("GATEWAY_{}_GRPC_ENDPOINT", dependency.to_ascii_uppercase()),
            grpc_endpoint(addresses[dependency].grpc),
        );
    }
    gateway_env.insert(
        "GATEWAY_ASYNC_OPERATION_KEYRING".into(),
        format!("1:{}", "a".repeat(64)),
    );
    // The ordinary journey uses a production-valid journal budget: with the
    // keyring enabled and the default 1000 ms request timeout, the runbook
    // allows at most 729 ms.
    gateway_env.insert("GATEWAY_JOURNAL_CLIENT_TIMEOUT_MS".into(), "700".into());

    for (environment, prefix, role) in [
        (&mut profile_env, "profile", "profile"),
        (&mut notification_env, "notification", "notification"),
        (&mut friend_env, "friend", "friend"),
        (&mut post_env, "post", "post"),
        (&mut auth_env, "auth", "auth"),
        (&mut gateway_env, "gateway", "gateway"),
    ] {
        insert_tls_material(environment, prefix, role)?;
    }
    if external_migrations {
        for (environment, prefix) in [
            (&mut auth_env, "AUTH"),
            (&mut profile_env, "PROFILE"),
            (&mut friend_env, "FRIEND"),
            (&mut post_env, "POST"),
            (&mut notification_env, "NOTIFICATION"),
            (&mut gateway_env, "GATEWAY"),
        ] {
            external_migrations::configure(environment, prefix)?;
        }
    }
    // The durable-queue recovery block needs a blocked mutation to be queued
    // before the owner receipt's 500 ms database lock timeout or the
    // Gateway-to-service edge deadline can win, so only that block runs on a
    // Gateway restarted with the short budget.
    let mut gateway_queue_env = gateway_env.clone();
    gateway_queue_env.insert("GATEWAY_JOURNAL_CLIENT_TIMEOUT_MS".into(), "300".into());

    let pre_upgrade_notification = if let Some(binary) = pre_upgrade_notification_binary {
        let addresses = ServiceAddresses::allocate()?;
        let mut environment = notification_env.clone();
        environment.remove("NOTIFICATION_DATABASE_MIGRATION_MODE");
        environment.insert(
            "NOTIFICATION_DATABASE_URL".into(),
            database_url(&pg_host, pg_port, &db_names["notification"]),
        );

        // Let the current Notification process own the accelerated purge tick
        // in the mixed-version acceptance; the rollback binary keeps its
        // normal hourly interval.
        environment.insert(
            "NOTIFICATION_RETENTION_PURGE_WORKER_INTERVAL_MS".into(),
            "3600000".into(),
        );
        insert_addresses(&mut environment, "NOTIFICATION", addresses);
        Some((binary, addresses, environment))
    } else {
        None
    };

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(10))
        .build()?;
    let mut processes = ManagedProcesses::default();
    let request_ledger = forensics::RequestLedger::default();
    // Fixture owners remain outside the future, including on unwinding. Capture
    // runs before ManagedProcesses and the dependency containers can be dropped.
    let journey = forensics::scope(
        request_ledger.clone(),
        AssertUnwindSafe(async {
            if external_migrations {
                for (package, prefix, environment) in [
                    ("auth-service", "AUTH", &mut auth_env),
                    ("profile-service", "PROFILE", &mut profile_env),
                    ("friend-service", "FRIEND", &mut friend_env),
                    ("post-service", "POST", &mut post_env),
                    ("api-gateway", "GATEWAY", &mut gateway_env),
                ] {
                    external_migrations::apply(&binary_dir.join(package), prefix, environment)
                        .await?;
                }
                gateway_queue_env.insert(
                    "GATEWAY_DATABASE_URL".into(),
                    gateway_env["GATEWAY_DATABASE_URL"].clone(),
                );
            }
            processes.spawn(
                "profile-service",
                binary_dir.join("profile-service"),
                &profile_env,
            )?;
            if let Some((binary, old_addresses, old_environment)) =
                pre_upgrade_notification.as_ref()
            {
                processes.spawn(
                    "notification-service-pre-upgrade",
                    binary.clone(),
                    old_environment,
                )?;
                await_ready(
                    &client,
                    &mut processes,
                    "notification-service-pre-upgrade",
                    old_addresses.metrics,
                    Duration::from_secs(45),
                )
                .await?;
            }

            // Keep the old binary running on the baseline before applying the
            // new Notification revision, including in external mode.
            if external_migrations {
                external_migrations::apply(
                    &binary_dir.join("notification-service"),
                    "NOTIFICATION",
                    &mut notification_env,
                )
                .await?;
                runtime_roles::cross_database_denials([
                    &auth_env["AUTH_DATABASE_URL"],
                    &profile_env["PROFILE_DATABASE_URL"],
                    &friend_env["FRIEND_DATABASE_URL"],
                    &post_env["POST_DATABASE_URL"],
                    &notification_env["NOTIFICATION_DATABASE_URL"],
                    &gateway_env["GATEWAY_DATABASE_URL"],
                ])
                .await?;
            }
            processes.spawn(
                "notification-service",
                binary_dir.join("notification-service"),
                &notification_env,
            )?;

            await_ready(
                &client,
                &mut processes,
                "profile-service",
                addresses["profile"].metrics,
                Duration::from_secs(45),
            )
            .await?;
            await_ready(
                &client,
                &mut processes,
                "notification-service",
                addresses["notification"].metrics,
                Duration::from_secs(45),
            )
            .await?;

            processes.spawn(
                "friend-service",
                binary_dir.join("friend-service"),
                &friend_env,
            )?;
            await_ready(
                &client,
                &mut processes,
                "friend-service",
                addresses["friend"].metrics,
                Duration::from_secs(45),
            )
            .await?;

            processes.spawn("post-service", binary_dir.join("post-service"), &post_env)?;
            // Post request paths depend on the Gateway exchange edge and report
            // it in readiness, so the Gateway must be running before Post can be
            // awaited ready. The Gateway health reports its own local state, so
            // starting it first cannot deadlock the two.
            processes.spawn("api-gateway", binary_dir.join("api-gateway"), &gateway_env)?;
            processes.spawn("auth-service", binary_dir.join("auth-service"), &auth_env)?;
            await_ready(
                &client,
                &mut processes,
                "post-service",
                addresses["post"].metrics,
                Duration::from_secs(60),
            )
            .await?;
            await_ready(
                &client,
                &mut processes,
                "auth-service",
                addresses["auth"].metrics,
                Duration::from_secs(60),
            )
            .await?;
            await_ready(
                &client,
                &mut processes,
                "api-gateway",
                addresses["gateway"].metrics,
                Duration::from_secs(60),
            )
            .await?;

            for (service, name) in [
                ("auth", "auth-service"),
                ("profile", "profile-service"),
                ("friend", "friend-service"),
                ("post", "post-service"),
                ("notification", "notification-service"),
                ("gateway", "api-gateway"),
            ] {
                processes.ensure_running(name)?;
                assert_migrations(
                    &database_url(&pg_host, pg_port, &db_names[service]),
                    service,
                    &migration_base_commit,
                    migration_baselines.get(service).with_context(|| {
                        format!("{service} migration baseline was not initialized")
                    })?,
                    expected_migration_deltas.as_ref(),
                )
                .await?;
            }

            if let Some((_, old_addresses, _)) = pre_upgrade_notification.as_ref() {
                let legacy_notification_fixture = legacy_notification_fixture
                    .as_ref()
                    .context("historical Notification migration fixture was not seeded")?;
                verify_pre_upgrade_notification_compatibility(NotificationCompatibilityContext {
                    readiness_client: &client,
                    processes: &mut processes,
                    pre_upgrade_addresses: *old_addresses,
                    current_addresses: addresses["notification"],
                    service_addresses: &addresses,
                    auth_environment: &auth_env,
                    post_environment: &post_env,
                    gateway_environment: &gateway_env,
                    notification_database_url: &database_url(
                        &pg_host,
                        pg_port,
                        &db_names["notification"],
                    ),
                    legacy_fixture: legacy_notification_fixture,
                })
                .await?;
            }

            let gateway_base = format!("http://{}", addresses["gateway"].http);
            let auth_database_url = database_url(&pg_host, pg_port, &db_names["auth"]);
            let gateway_database_url = database_url(&pg_host, pg_port, &db_names["gateway"]);
            let gateway_restored_database_url =
                database_url(&pg_host, pg_port, &db_names["gateway_restored"]);
            let profile_database_url = database_url(&pg_host, pg_port, &db_names["profile"]);
            let notification_database_url =
                database_url(&pg_host, pg_port, &db_names["notification"]);
            let friend_database_url = database_url(&pg_host, pg_port, &db_names["friend"]);
            let post_database_url = database_url(&pg_host, pg_port, &db_names["post"]);
            let mut service_restart = ServiceProcessRestart {
                processes: &mut processes,
                binary_dir: &binary_dir,
                environment: &gateway_env,
                ordinary_environment: &gateway_env,
                queue_environment: &gateway_queue_env,
                auth_environment: &auth_env,
                metrics_address: addresses["gateway"].metrics,
                auth_database_url: &auth_database_url,
                auth_metrics_address: addresses["auth"].metrics,
                gateway_database_url: &gateway_database_url,
                gateway_database_name: &db_names["gateway"],
                gateway_restored_database_url: &gateway_restored_database_url,
                gateway_restored_database_name: &db_names["gateway_restored"],
                postgres: &pg,
                profile_database_url: &profile_database_url,
                profile_metrics_address: addresses["profile"].metrics,
                notification_database_url: &notification_database_url,
                notification_grpc_address: addresses["notification"].grpc,
                notification_metrics_address: addresses["notification"].metrics,
                redis: &redis_container,
                friend_database_url: &friend_database_url,
                friend_environment: &friend_env,
                friend_metrics_address: addresses["friend"].metrics,
                post_database_url: &post_database_url,
                post_environment: &post_env,
                post_metrics_address: addresses["post"].metrics,
                kafka: &kafka_container,
                kafka_bootstrap: &kafka_bootstrap,
                kafka_topic: &kafka_topic,
                kafka_partition_count,
            };
            run_process_social_lifecycle(
                &client,
                &gateway_base,
                &inbox,
                &run_id,
                &relationship_gate,
                &mut service_restart,
                focus_feed_block_race,
            )
            .await?;

            Ok::<(), anyhow::Error>(())
        })
        .catch_unwind(),
    )
    .await;
    let gateway_database_url = database_url(&pg_host, pg_port, &db_names["gateway"]);
    let profile_database_url = database_url(&pg_host, pg_port, &db_names["profile"]);
    // Observe the still-owned database before SQL snapshots or teardown can
    // lose its state. A timeout alone cannot establish container termination.
    let postgres_failure = if matches!(&journey, Ok(Ok(()))) {
        None
    } else {
        let observed = postgres::startup_failure_diagnostics(&pg).await;
        Some(json!({
            "capturedAt": Utc::now(),
            "containerId": observed.container_id,
            "metadata": observed.metadata,
            "endpoint": observed.endpoint,
            "stdout": observed.stdout,
            "stderr": observed.stderr,
            "scope": "bounded observations before teardown; not a root-cause conclusion",
        }))
    };
    let evidence = forensics::capture(
        forensics::CaptureContext {
            run_id: &run_id,
            postgres_failure,
            migration_mode: if external_migrations {
                "external"
            } else {
                "startup"
            },
            gateway_database_url: &gateway_database_url,
            profile_database_url: &profile_database_url,
            gateway_metrics_url: format!("http://{}/metrics", addresses["gateway"].metrics),
            service_pids: processes
                .children
                .iter()
                .map(|process| (process.name, process.child.id()))
                .collect(),
            dependency_containers: vec![
                ("postgres", pg.id().to_owned()),
                ("redis", redis_container.id().to_owned()),
                ("kafka", kafka_container.id().to_owned()),
            ],
        },
        &request_ledger,
    )
    .await;
    match &evidence {
        Ok(path) => eprintln!(
            "process-stack evidence before teardown: {} (SHA-256 in adjacent .sha256 file)",
            path.display()
        ),
        Err(_) => eprintln!(
            "process-stack failure evidence could not be persisted; original failure is preserved"
        ),
    }
    match journey {
        Ok(result) => {
            result?;
            forensics::verify_complete(&evidence?)?;
        }
        Err(panic) => std::panic::resume_unwind(panic),
    }

    let mail_shutdown_result = mail_shutdown.send(());
    let mail_task_result = mail_task.await.context("mail sink task panicked")?;
    anyhow::ensure!(
        mail_shutdown_result.is_ok(),
        "mail sink shutdown signal was lost"
    );
    mail_task_result?;
    if focus_feed_block_race {
        eprintln!(
            "process-stack focused check passed: scope=Gateway→Post→Friend live block/feed race with real PostgreSQL and Redis in the six-service process harness; full process-stack acceptance was not run"
        );
        return Ok(());
    }
    eprintln!(
        "process-stack acceptance passed: run_id={run_id}, six actual service executables, six migrated service PostgreSQL databases plus a Gateway restore target, Redis, Kafka publisher, generated workload mTLS, delayed Gateway read timeout/recovery for Auth, Profile, Friend, Post, and Notification, real Redis outage/cache-fallback/recovery through the ranked feed, real Kafka broker outage with durable Post outbox recovery across Post process restart, Post Kafka-ack crash recovery, separate-process Post and Friend database-pool saturation and recovery, durable 202 recovery for Post, Comment, Like, UnlikePost, DeletePost, Follow, Unfollow, Profile Privacy, and Profile Edit owner routes across Gateway process restarts, pending-operation recovery from a real Gateway pg_dump/pg_restore backup and source-journal replay deduplication, Auth account-deletion retry recovery across an Auth process restart, and process-level profile/avatar, follows, post/comment/like, notifications, privacy, search, moderation, profile hiding, refresh/logout, and account-deletion workflows; provider double=local HTTP verification-mail sink"
    );
    Ok(())
}

async fn run_process_social_lifecycle(
    client: &reqwest::Client,
    base: &str,
    inbox: &MailInbox,
    run_id: &str,
    relationship_gate: &ProcessStackRelationshipGate,
    gateway_restart: &mut ServiceProcessRestart<'_>,
    focus_feed_block_race: bool,
) -> Result<()> {
    let owner_email = format!("process-owner-{run_id}@example.invalid");
    let follower_email = format!("process-follower-{run_id}@example.invalid");
    let mut owner = process_signup_verify_login(client, base, inbox, &owner_email).await?;
    let mut follower = process_signup_verify_login(client, base, inbox, &follower_email).await?;
    refresh_process_user(client, base, &mut follower).await?;
    verify_gateway_auth_deadline_when_auth_is_suspended(
        client,
        base,
        gateway_restart,
        &owner_email,
    )
    .await?;

    let owner_username = format!("owner{}", &run_id[..12]);
    let follower_username = format!("follower{}", &run_id[..12]);
    let (_, owner_profile) = process_json_request(
        client,
        Method::PUT,
        base,
        &format!("/api/profile/{}", owner.user_id),
        Some(&owner.access_token),
        None,
        Some(json!({
            "username": owner_username,
            "nickname": "Process Owner",
            "avatarUrl": "https://example.invalid/process-owner.png",
            "bio": "Independent process acceptance owner"
        })),
    )
    .await?;
    anyhow::ensure!(
        owner_profile["username"] == owner_username,
        "owner profile update did not persist: {owner_profile}"
    );

    let (_, follower_profile) = process_json_request(
        client,
        Method::PUT,
        base,
        &format!("/api/profile/{}", follower.user_id),
        Some(&follower.access_token),
        None,
        Some(json!({
            "username": follower_username,
            "nickname": "Process Follower",
            "avatarUrl": "https://example.invalid/process-follower.png",
            "bio": "Independent process acceptance follower"
        })),
    )
    .await?;
    anyhow::ensure!(
        follower_profile["username"] == follower_username,
        "follower profile update did not persist: {follower_profile}"
    );

    verify_gateway_profile_deadline_when_profile_is_suspended(
        client,
        base,
        gateway_restart,
        &owner,
        &owner_username,
    )
    .await?;

    upload_process_avatar(client, base, &follower.access_token).await?;
    let (avatar_content_type, avatar_bytes) =
        fetch_process_avatar(client, base, &owner.access_token, follower.user_id).await?;
    anyhow::ensure!(
        avatar_content_type == "image/png" && !avatar_bytes.is_empty(),
        "avatar did not round-trip across processes: type={avatar_content_type}, bytes={}",
        avatar_bytes.len()
    );

    for (actor, target) in [(&owner, &follower), (&follower, &owner)] {
        let (status, response) = process_json_request(
            client,
            Method::POST,
            base,
            "/api/follows",
            Some(&actor.access_token),
            None,
            Some(json!({"targetUserId": target.user_id.to_string()})),
        )
        .await?;
        anyhow::ensure!(
            status.is_success() && response["following"] == true,
            "cross-process follow did not activate: {status} {response}"
        );
    }

    verify_gateway_friend_deadline_when_friend_is_suspended(
        client,
        base,
        gateway_restart,
        &follower.access_token,
        owner.user_id,
    )
    .await?;

    let post_content = format!("process-social-{run_id}");
    let (_, post) = process_json_request(
        client,
        Method::POST,
        base,
        "/api/posts",
        Some(&owner.access_token),
        None,
        Some(json!({"content": post_content, "visibility": "PUBLIC"})),
    )
    .await?;
    let post_id: Uuid = post["id"]
        .as_str()
        .context("process post response omitted its ID")?
        .parse()?;

    verify_gateway_feed_when_redis_is_unavailable(
        client,
        base,
        gateway_restart,
        &follower.access_token,
        post_id,
        &post_content,
    )
    .await?;

    verify_gateway_feed_block_race(
        client,
        base,
        relationship_gate,
        gateway_restart.friend_database_url,
        &follower,
        &owner,
        post_id,
        &post_content,
    )
    .await?;
    if focus_feed_block_race {
        return Ok(());
    }

    recover_post_after_kafka_broker_outage(client, base, gateway_restart, run_id, &owner).await?;

    let (_, follower_post) = process_json_request(
        client,
        Method::GET,
        base,
        &format!("/api/posts/{post_id}"),
        Some(&follower.access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        follower_post["post"]["content"] == post_content,
        "follower could not read the persisted post: {follower_post}"
    );

    verify_gateway_post_deadline_when_post_is_suspended(
        client,
        base,
        gateway_restart,
        &follower.access_token,
        post_id,
        &post_content,
    )
    .await?;

    let comment_content = format!("process-comment-{run_id}");
    let (_, comment) = process_json_request(
        client,
        Method::POST,
        base,
        &format!("/api/posts/{post_id}/comments"),
        Some(&follower.access_token),
        None,
        Some(json!({"content": comment_content})),
    )
    .await?;
    anyhow::ensure!(
        comment["id"].as_str().is_some(),
        "cross-process comment returned no ID: {comment}"
    );

    let (_, like) = process_json_request(
        client,
        Method::POST,
        base,
        &format!("/api/posts/{post_id}/likes"),
        Some(&follower.access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(like["liked"] == true, "cross-process like failed: {like}");

    let (_, persisted_post) = process_json_request(
        client,
        Method::GET,
        base,
        &format!("/api/posts/{post_id}"),
        Some(&owner.access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        persisted_post["post"]["commentCount"] == 1 && persisted_post["post"]["likeCount"] == 1,
        "comment/like state did not persist through PostService: {persisted_post}"
    );

    let post_created_notification =
        wait_for_process_notification(client, base, &follower.access_token, "POST_CREATED").await?;
    let notification_id = post_created_notification["id"]
        .as_str()
        .context("process notification response omitted its ID")?;
    wait_for_process_notification(client, base, &owner.access_token, "POST_COMMENTED").await?;
    wait_for_process_notification(client, base, &owner.access_token, "POST_LIKED").await?;
    verify_gateway_notification_deadline_when_notification_is_suspended(
        client,
        base,
        gateway_restart,
        &follower.access_token,
        notification_id,
    )
    .await?;

    let (_, profile_search) = process_json_request(
        client,
        Method::GET,
        base,
        &format!("/api/profile/search?q={follower_username}"),
        Some(&owner.access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        profile_search["profiles"]
            .as_array()
            .is_some_and(|items| items
                .iter()
                .any(|item| item["userId"] == follower.user_id.to_string())),
        "profile search omitted the process-created follower: {profile_search}"
    );

    let (_, post_search) = process_json_request(
        client,
        Method::GET,
        base,
        &format!("/api/posts/search?q={post_content}"),
        Some(&follower.access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        post_search["posts"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["content"] == post_content)),
        "post search omitted the process-created post: {post_search}"
    );

    let (_, marked_read) = process_json_request(
        client,
        Method::PATCH,
        base,
        "/api/notifications/read-all",
        Some(&owner.access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        marked_read["updatedCount"]
            .as_i64()
            .is_some_and(|count| count >= 2),
        "process notifications were not persisted/read: {marked_read}"
    );

    let (privacy_status, privacy) = process_json_request(
        client,
        Method::PUT,
        base,
        "/api/profile/privacy",
        Some(&follower.access_token),
        None,
        Some(json!({"accountVisibility": "PRIVATE"})),
    )
    .await?;
    anyhow::ensure!(
        privacy_status == StatusCode::OK && privacy["accountVisibility"] == "PRIVATE",
        "synchronous private-profile change did not return its successful Profile response: {privacy_status} {privacy}"
    );
    verify_privacy_timeout_after_owner_commit(client, base, gateway_restart, &follower).await?;
    let (anonymous_status, _) = process_json_request(
        client,
        Method::GET,
        base,
        &format!("/api/profile/{}", follower.user_id),
        None,
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        anonymous_status == StatusCode::NOT_FOUND,
        "private profile was not concealed from an anonymous viewer: {anonymous_status}"
    );
    let (_, visible_to_follower) = process_json_request(
        client,
        Method::GET,
        base,
        &format!("/api/profile/{}", follower.user_id),
        Some(&owner.access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        visible_to_follower["accountVisibility"] == "PRIVATE",
        "existing follower lost access to the private profile: {visible_to_follower}"
    );

    let (report_status, report) = process_json_request(
        client,
        Method::POST,
        base,
        "/api/moderation/reports",
        Some(&owner.access_token),
        None,
        Some(json!({
            "targetType": "POST",
            "targetId": post_id.to_string(),
            "category": "SPAM",
            "reason": "Process-level moderation acceptance report"
        })),
    )
    .await?;
    anyhow::ensure!(
        report_status == StatusCode::CREATED && report["status"] == "OPEN",
        "process moderation report was not persisted: {report_status} {report}"
    );
    let (_, saved_reports) = process_json_request(
        client,
        Method::GET,
        base,
        "/api/moderation/reports/me",
        Some(&owner.access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        saved_reports["reports"]
            .as_array()
            .is_some_and(|reports| reports.iter().any(|saved| saved["id"] == report["id"])),
        "created process moderation report was not readable afterward: {saved_reports}"
    );

    restart_gateway_with_journal_budget(client, gateway_restart, GatewayJournalBudget::Queue)
        .await?;
    recover_accepted_comment_after_gateway_process_restart(
        client,
        base,
        gateway_restart,
        run_id,
        post_id,
        &follower,
        &owner,
    )
    .await?;

    recover_accepted_like_after_gateway_process_restart(
        client,
        base,
        gateway_restart,
        run_id,
        &follower,
        &owner,
    )
    .await?;

    recover_accepted_unlike_after_gateway_process_restart(
        client,
        base,
        gateway_restart,
        run_id,
        &follower,
        &owner,
    )
    .await?;

    recover_accepted_follow_after_gateway_process_restart(
        client,
        base,
        gateway_restart,
        run_id,
        &owner,
        &follower,
    )
    .await?;

    recover_moderation_after_gateway_process_restart(
        client,
        base,
        gateway_restart,
        &owner_email,
        &mut owner,
        post_id,
        report["id"]
            .as_str()
            .context("moderation report response omitted its ID")?
            .parse()?,
    )
    .await?;

    recover_accepted_post_after_gateway_process_restart(
        client,
        base,
        gateway_restart,
        run_id,
        &owner,
        &follower,
    )
    .await?;

    recover_accepted_post_after_gateway_backup_restore(
        client,
        base,
        gateway_restart,
        run_id,
        &owner,
        &follower,
    )
    .await?;

    recover_post_after_kafka_ack_process_crash(client, base, gateway_restart, run_id, &owner)
        .await?;

    recover_post_database_pool_saturation(client, base, gateway_restart, run_id, &owner).await?;

    recover_accepted_post_delete_after_gateway_process_restart(
        client,
        base,
        gateway_restart,
        run_id,
        &owner,
        &follower,
    )
    .await?;

    // Unfollow mutates the relationship used by the preceding Post recovery
    // journeys, so exercise the delete-side route only after those assertions.
    recover_accepted_unfollow_after_gateway_process_restart(
        client,
        base,
        gateway_restart,
        run_id,
        &owner,
        &follower,
    )
    .await?;

    recover_accepted_profile_privacy_after_gateway_process_restart(
        client,
        base,
        gateway_restart,
        run_id,
        &follower,
        &owner,
    )
    .await?;

    // The privacy recovery above makes the Friend target public again. Keep
    // pool saturation after that owner journey so this command exercises an
    // active edge mutation rather than the private-account request path.
    recover_friend_database_pool_saturation(
        client,
        base,
        gateway_restart,
        run_id,
        &owner,
        &follower,
    )
    .await?;

    // Exercise the separate service-delay boundary after pool saturation has
    // restored both directions of the relationship. The delayed Unfollow leaves
    // the fixture in its original inactive state before profile hiding.
    recover_unfollow_after_friend_service_delay(
        client,
        base,
        gateway_restart,
        run_id,
        &owner,
        &follower,
    )
    .await?;

    recover_accepted_profile_edit_after_gateway_process_restart(
        client,
        base,
        gateway_restart,
        run_id,
        &follower,
        &owner,
        &follower_username,
    )
    .await?;
    restart_gateway_with_journal_budget(client, gateway_restart, GatewayJournalBudget::Ordinary)
        .await?;

    let (hide_status, hidden_profile) = process_json_request(
        client,
        Method::DELETE,
        base,
        &format!("/api/profile/{}", follower.user_id),
        Some(&follower.access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        hide_status == StatusCode::OK
            && hidden_profile["deleted"] == true
            && hidden_profile["accountDeleted"] == false,
        "profile hiding was not kept distinct from account deletion: {hidden_profile}"
    );
    let (hidden_status, _) = process_json_request(
        client,
        Method::GET,
        base,
        &format!("/api/profile/{}", follower.user_id),
        Some(&owner.access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        hidden_status == StatusCode::NOT_FOUND,
        "hidden profile remained visible across service processes: {hidden_status}"
    );

    logout_process_user(client, base, &follower).await?;

    // Make the Friend cleanup RPC fail once while keeping the other service
    // processes available. The committed retry must survive an Auth process
    // restart rather than relying on in-memory worker state.
    gateway_restart.processes.stop("friend-service")?;
    let (delete_status, deletion) = process_json_request(
        client,
        Method::DELETE,
        base,
        "/auth/account",
        Some(&owner.access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        delete_status == StatusCode::ACCEPTED && deletion["deleted"] == true,
        "cross-process account deletion was not accepted: {delete_status} {deletion}"
    );
    let saga_id: Uuid = deletion["saga"]["id"]
        .as_str()
        .context("account deletion response omitted the saga ID")?
        .parse()?;

    let auth_pool = sqlx::PgPool::connect(gateway_restart.auth_database_url)
        .await
        .context("failed to inspect account-deletion process-recovery state")?;
    let retry_deadline = Instant::now() + Duration::from_secs(20);
    let before_restart_steps = loop {
        let saga_status: String =
            sqlx::query_scalar("select status from user_deletion_sagas where id = $1")
                .bind(saga_id)
                .fetch_one(&auth_pool)
                .await?;
        let steps: Vec<(String, String, i32)> = sqlx::query_as(
            "select target, status, attempt_count from user_deletion_steps where saga_id = $1 order by target",
        )
        .bind(saga_id)
        .fetch_all(&auth_pool)
        .await?;
        let friend_retry = steps.iter().any(|(target, status, attempts)| {
            target == "FRIEND" && status == "RETRY" && *attempts == 1
        });
        let other_steps_succeeded = steps.len() == 4
            && steps.iter().all(|(target, status, attempts)| {
                if target == "FRIEND" {
                    status == "RETRY" && *attempts == 1
                } else {
                    status == "SUCCEEDED" && *attempts == 1
                }
            });
        if saga_status == "IN_PROGRESS" && friend_retry && other_steps_succeeded {
            break steps;
        }
        if Instant::now() >= retry_deadline {
            anyhow::bail!(
                "Auth did not persist one Friend retry while completing the other deletion steps: saga={saga_status}, steps={steps:?}"
            );
        }
        sleep(Duration::from_millis(50)).await;
    };

    gateway_restart.processes.stop("auth-service")?;
    let saga_status_while_auth_stopped: String =
        sqlx::query_scalar("select status from user_deletion_sagas where id = $1")
            .bind(saga_id)
            .fetch_one(&auth_pool)
            .await?;
    anyhow::ensure!(
        saga_status_while_auth_stopped == "IN_PROGRESS",
        "stopping Auth changed the durable deletion saga state: {saga_status_while_auth_stopped}"
    );

    gateway_restart.processes.spawn(
        "friend-service",
        gateway_restart.binary_dir.join("friend-service"),
        gateway_restart.friend_environment,
    )?;
    await_ready(
        client,
        gateway_restart.processes,
        "friend-service",
        gateway_restart.friend_metrics_address,
        Duration::from_secs(30),
    )
    .await?;

    let made_due = sqlx::query(
        "update user_deletion_steps set next_attempt_at = now() where saga_id = $1 and target = 'FRIEND' and status = 'RETRY' and locked_at is null",
    )
    .bind(saga_id)
    .execute(&auth_pool)
    .await?
    .rows_affected();
    anyhow::ensure!(
        made_due == 1,
        "expected exactly one durable Friend retry to advance in the disposable test database; changed {made_due} rows"
    );

    gateway_restart.processes.spawn(
        "auth-service",
        gateway_restart.binary_dir.join("auth-service"),
        gateway_restart.auth_environment,
    )?;
    await_ready(
        client,
        gateway_restart.processes,
        "auth-service",
        gateway_restart.auth_metrics_address,
        Duration::from_secs(60),
    )
    .await?;

    let deletion_deadline = Instant::now() + Duration::from_secs(45);
    loop {
        let (status, saga) = process_json_request(
            client,
            Method::GET,
            base,
            "/auth/account/deletion",
            Some(&owner.access_token),
            None,
            None,
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::OK,
            "deletion status returned {status}: {saga}"
        );
        if saga["status"] == "COMPLETED" {
            anyhow::ensure!(
                saga["steps"].as_array().is_some_and(|steps| {
                    steps.len() == 4 && steps.iter().all(|step| step["status"] == "SUCCEEDED")
                }),
                "deletion saga did not complete all four real service RPCs: {saga}"
            );
            break;
        }
        if Instant::now() >= deletion_deadline {
            anyhow::bail!("cross-process deletion saga did not complete within 45s: {saga}");
        }
        sleep(Duration::from_millis(250)).await;
    }

    let after_restart_steps: Vec<(String, String, i32)> = sqlx::query_as(
        "select target, status, attempt_count from user_deletion_steps where saga_id = $1 order by target",
    )
    .bind(saga_id)
    .fetch_all(&auth_pool)
    .await?;
    anyhow::ensure!(
        after_restart_steps.len() == 4
            && after_restart_steps.iter().all(|(target, status, attempts)| {
                status == "SUCCEEDED"
                    && if target == "FRIEND" {
                        *attempts == 2
                    } else {
                        *attempts == 1
                    }
            }),
        "Auth restart did not recover only the failed Friend step exactly once: {after_restart_steps:?}; initial={before_restart_steps:?}"
    );
    eprintln!(
        "account-deletion retry survived Auth process restart: saga_id={saga_id}, initial={before_restart_steps:?}, terminal={after_restart_steps:?}"
    );

    verify_process_notification_tombstone_purge(
        owner.user_id,
        gateway_restart.notification_database_url,
        gateway_restart.notification_grpc_address,
        gateway_restart.post_environment,
    )
    .await?;

    Ok(())
}

/// Session advisory-lock keys for the privacy completion barrier. They are
/// distinct from every other failpoint key in this harness.
const PRIVACY_COMPLETION_BARRIER_KEYS: (i32, i32) = (20_261_009, 1_689);
const GATEWAY_COMPLETION_ACQUIRES: &str =
    "gateway_db_pool_acquire_total{pool=\"journal\",caller=\"completion\"}";
const GATEWAY_COMPLETION_HOLDERS: &str =
    "gateway_db_pool_observed_holders{pool=\"journal\",caller=\"completion\"}";
/// Fast completion attempts the Gateway makes before its background phase.
const GATEWAY_FAST_COMPLETION_ATTEMPTS: i64 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivacyBarrierRelease {
    /// Release as soon as the timed-out request is proven to be blocked only
    /// on Gateway completion.
    AfterCallerTimeout,
    /// Keep the barrier until every fast attempt has failed and the background
    /// retry phase has started, then release.
    AfterFastAttempts,
}

async fn verify_privacy_timeout_after_owner_commit(
    client: &reqwest::Client,
    base: &str,
    gateway: &ServiceProcessRestart<'_>,
    owner: &ProcessUser,
) -> Result<()> {
    // Keep the ordinary successful PRIVATE update separate. This failpoint
    // pauses only this owner's privacy completion audit insert, proving that a
    // committed owner effect behind a Gateway-bookkeeping timeout still
    // converges to the caller-scoped COMMITTED outcome, including after the
    // Gateway's fast completion attempts are exhausted.
    let observer = labelled_pool(gateway.gateway_database_url, "e2e-privacy-observer", 2).await?;
    let barrier_pool =
        labelled_pool(gateway.gateway_database_url, "e2e-privacy-barrier", 1).await?;
    let profile_pool =
        labelled_pool(gateway.profile_database_url, "e2e-privacy-observer", 1).await?;
    let (key_a, key_b) = PRIVACY_COMPLETION_BARRIER_KEYS;
    let function_sql = format!(
        "CREATE OR REPLACE FUNCTION e2e_pause_privacy_completion() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.actor_id = '{actor}' AND NEW.action = 'PUT /api/profile/privacy' THEN PERFORM pg_advisory_xact_lock({key_a}, {key_b}); END IF; RETURN NEW; END; $$",
        actor = owner.user_id
    );
    sqlx::query(&function_sql).execute(&observer).await?;
    sqlx::query(
        "create trigger e2e_pause_privacy_completion before insert on admin_audit_events for each row execute function e2e_pause_privacy_completion()",
    )
    .execute(&observer)
    .await?;
    let mut barrier = barrier_pool.acquire().await?;
    let outcome = async {
        let mut cases = Vec::new();
        for release in [
            PrivacyBarrierRelease::AfterCallerTimeout,
            PrivacyBarrierRelease::AfterFastAttempts,
        ] {
            cases.push(
                privacy_completion_barrier_case(
                    client,
                    base,
                    gateway,
                    owner,
                    &observer,
                    &profile_pool,
                    &mut barrier,
                    release,
                )
                .await?,
            );
        }
        Ok::<_, anyhow::Error>(cases)
    }
    .await;
    let unlock = sqlx::query("select pg_advisory_unlock_all()")
        .execute(&mut *barrier)
        .await;
    drop(barrier);
    let cleanup = async {
        sqlx::query("drop trigger if exists e2e_pause_privacy_completion on admin_audit_events")
            .execute(&observer)
            .await?;
        sqlx::query("drop function if exists e2e_pause_privacy_completion()")
            .execute(&observer)
            .await?;
        Ok::<(), anyhow::Error>(())
    }
    .await;
    observer.close().await;
    barrier_pool.close().await;
    profile_pool.close().await;
    let cases = outcome?;
    unlock?;
    cleanup?;
    let summary = cases
        .iter()
        .map(|(release, operation_id, attempts, held)| {
            format!("{release:?}: operation_id={operation_id}, completion_attempts={attempts}, barrier_held={held:?}")
        })
        .collect::<Vec<_>>()
        .join("; ");
    eprintln!("privacy committed-owner/completion-delay acceptance passed: owner_visibility=PRIVATE, owner_receipts=1, owner_audits=1, gateway_audits=1, queue_rows=0, status=504 then caller-scoped COMMITTED/200, completion_holders=0; {summary}");
    Ok(())
}

async fn labelled_pool(url: &str, application_name: &str, size: u32) -> Result<sqlx::PgPool> {
    let options = url
        .parse::<sqlx::postgres::PgConnectOptions>()?
        .application_name(application_name);
    Ok(PgPoolOptions::new()
        .max_connections(size)
        .connect_with(options)
        .await?)
}

/// Read one Gateway metric sample; an absent sample reads as zero.
async fn gateway_metric_value(
    client: &reqwest::Client,
    metrics_address: SocketAddr,
    sample: &str,
) -> Result<i64> {
    let metrics = client
        .get(format!("http://{metrics_address}/metrics"))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    Ok(metrics
        .lines()
        .find_map(|line| {
            line.strip_prefix(sample)
                .and_then(|rest| rest.strip_prefix(' '))
                .and_then(|value| value.trim().parse::<f64>().ok())
        })
        .map(|value| value as i64)
        .unwrap_or(0))
}

async fn await_privacy_barrier_waiter(observer: &sqlx::PgPool) -> Result<()> {
    let (key_a, key_b) = PRIVACY_COMPLETION_BARRIER_KEYS;
    timeout(Duration::from_secs(3), async {
        loop {
            let waiting: i64 = sqlx::query_scalar(
                "select count(*) from pg_locks where locktype = 'advisory' and not granted and classid::bigint = $1 and objid::bigint = $2 and objsubid = 2",
            )
            .bind(i64::from(key_a))
            .bind(i64::from(key_b))
            .fetch_one(observer)
            .await?;
            if waiting > 0 {
                break Ok::<(), anyhow::Error>(());
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("privacy completion never waited on its operation-scoped barrier")?
}

async fn privacy_pending_journal(
    observer: &sqlx::PgPool,
    operation_id: Uuid,
) -> Result<(String, String, i64)> {
    Ok(sqlx::query_as(
        r#"
        select state, outcome,
            (select count(*) from gateway_async_operation_queue where operation_id = $1)
        from gateway_request_journal where id = $1
        "#,
    )
    .bind(operation_id)
    .fetch_one(observer)
    .await?)
}

#[allow(clippy::too_many_arguments)]
async fn privacy_completion_barrier_case(
    client: &reqwest::Client,
    base: &str,
    gateway: &ServiceProcessRestart<'_>,
    owner: &ProcessUser,
    observer: &sqlx::PgPool,
    profile_pool: &sqlx::PgPool,
    barrier: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
    release: PrivacyBarrierRelease,
) -> Result<(PrivacyBarrierRelease, Uuid, i64, Duration)> {
    let (public_status, public) = process_json_request(
        client,
        Method::PUT,
        base,
        "/api/profile/privacy",
        Some(&owner.access_token),
        None,
        Some(json!({"accountVisibility": "PUBLIC"})),
    )
    .await?;
    anyhow::ensure!(
        public_status == StatusCode::OK && public["accountVisibility"] == "PUBLIC",
        "privacy completion-delay fixture could not establish PUBLIC: {public_status} {public}"
    );
    let attempts_before =
        gateway_metric_value(client, gateway.metrics_address, GATEWAY_COMPLETION_ACQUIRES).await?;
    let (key_a, key_b) = PRIVACY_COMPLETION_BARRIER_KEYS;
    sqlx::query("select pg_advisory_lock($1, $2)")
        .bind(key_a)
        .bind(key_b)
        .execute(&mut **barrier)
        .await?;
    let barrier_taken = Instant::now();
    let (status, body, operation_id) = process_json_request_with_operation_id(
        client,
        Method::PUT,
        base,
        "/api/profile/privacy",
        Some(&owner.access_token),
        None,
        Some(json!({"accountVisibility": "PRIVATE"})),
    )
    .await?;
    let operation_id =
        operation_id.context("privacy completion timeout omitted its operation id")?;
    anyhow::ensure!(
        status == StatusCode::GATEWAY_TIMEOUT
            && body["code"] == "GATEWAY_504"
            && barrier_taken.elapsed() < Duration::from_secs(2),
        "blocked Gateway completion did not return a bounded operation-id timeout: {status} {body}"
    );
    let owner_state: (String, i64, i64) = sqlx::query_as(r#"
        select account_visibility,
            (select count(*) from profile_mutation_receipts where operation_id = $2
                and actor_id = $1 and scope = 'profile.v1.UpdateProfilePrivacy' and outcome_json is not null),
            (select count(*) from profile_audit_events where actor_id = $1 and profile_user_id = $1
                and request_id = $3 and action = 'UPDATE_PRIVACY' and outcome = 'SUCCESS')
        from profiles where user_id = $1 and deleted_at is null
    "#).bind(owner.user_id).bind(operation_id).bind(operation_id.to_string()).fetch_one(profile_pool).await?;
    anyhow::ensure!(owner_state == ("PRIVATE".into(), 1, 1),
        "privacy owner had not committed one effect/receipt/audit behind its Gateway timeout: {owner_state:?}");
    let journal = privacy_pending_journal(observer, operation_id).await?;
    anyhow::ensure!(
        journal == ("IN_PROGRESS".into(), "PENDING".into(), 0),
        "privacy timeout was not isolated to unkeyed Gateway completion: journal={journal:?}"
    );
    await_privacy_barrier_waiter(observer).await?;
    if release == PrivacyBarrierRelease::AfterFastAttempts {
        timeout(Duration::from_secs(10), async {
            loop {
                let attempts = gateway_metric_value(
                    client,
                    gateway.metrics_address,
                    GATEWAY_COMPLETION_ACQUIRES,
                )
                .await?
                    - attempts_before;
                if attempts > GATEWAY_FAST_COMPLETION_ATTEMPTS {
                    break Ok::<(), anyhow::Error>(());
                }
                sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .context("Gateway did not enter its background completion phase while blocked")??;
        let journal = privacy_pending_journal(observer, operation_id).await?;
        anyhow::ensure!(
            journal == ("IN_PROGRESS".into(), "PENDING".into(), 0),
            "exhausted fast completion changed the journal before the barrier lifted: {journal:?}"
        );
    }
    sqlx::query("select pg_advisory_unlock($1, $2)")
        .bind(key_a)
        .bind(key_b)
        .execute(&mut **barrier)
        .await?;
    let barrier_held = barrier_taken.elapsed();
    timeout(Duration::from_secs(10), async {
        loop {
            let final_state: (String, String, Option<i32>, i64) = sqlx::query_as(
                r#"
                select state, outcome, response_status,
                    (select count(*) from admin_audit_events where id = $1 and outcome = 'SUCCESS')
                from gateway_request_journal where id = $1
            "#,
            )
            .bind(operation_id)
            .fetch_one(observer)
            .await?;
            if final_state == ("COMPLETED".into(), "COMMITTED".into(), Some(200), 1) {
                break Ok::<(), anyhow::Error>(());
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .with_context(|| {
        format!("privacy journal did not settle after a {barrier_held:?} completion barrier ({release:?})")
    })??;
    let (status, operation) = process_json_request(
        client,
        Method::GET,
        base,
        &format!("/api/operations/{operation_id}"),
        Some(&owner.access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        status == StatusCode::OK
            && operation["state"] == "COMPLETED"
            && operation["outcome"] == "COMMITTED"
            && operation["responseStatus"] == 200,
        "caller-scoped privacy status did not expose the committed outcome: {status} {operation}"
    );
    let journal = privacy_pending_journal(observer, operation_id).await?;
    anyhow::ensure!(
        journal.2 == 0,
        "settled privacy operation left a durable queue row: {journal:?}"
    );
    timeout(Duration::from_secs(2), async {
        loop {
            let holders =
                gateway_metric_value(client, gateway.metrics_address, GATEWAY_COMPLETION_HOLDERS)
                    .await?;
            anyhow::ensure!(
                holders >= 0,
                "completion holder gauge went negative: {holders}"
            );
            if holders == 0 {
                break Ok::<(), anyhow::Error>(());
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .context("privacy completion retained its observed database connection after settling")??;
    let attempts =
        gateway_metric_value(client, gateway.metrics_address, GATEWAY_COMPLETION_ACQUIRES).await?
            - attempts_before;
    match release {
        PrivacyBarrierRelease::AfterCallerTimeout => anyhow::ensure!(
            (1..=GATEWAY_FAST_COMPLETION_ATTEMPTS).contains(&attempts),
            "completion released during the caller window needed {attempts} attempts"
        ),
        PrivacyBarrierRelease::AfterFastAttempts => anyhow::ensure!(
            attempts > GATEWAY_FAST_COMPLETION_ATTEMPTS,
            "completion held past the fast attempts committed after only {attempts} attempts"
        ),
    }
    Ok((release, operation_id, attempts, barrier_held))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GatewayJournalBudget {
    /// Production-valid budget for ordinary journeys.
    Ordinary,
    /// Short budget required by the durable-queue recovery scenarios.
    Queue,
}

/// Restart the real Gateway executable with the requested journal budget. Later
/// scenario restarts reuse the selected environment.
async fn restart_gateway_with_journal_budget(
    client: &reqwest::Client,
    gateway: &mut ServiceProcessRestart<'_>,
    budget: GatewayJournalBudget,
) -> Result<()> {
    gateway.environment = match budget {
        GatewayJournalBudget::Ordinary => gateway.ordinary_environment,
        GatewayJournalBudget::Queue => gateway.queue_environment,
    };
    gateway.processes.stop("api-gateway")?;
    gateway.processes.spawn(
        "api-gateway",
        gateway.binary_dir.join("api-gateway"),
        gateway.environment,
    )?;
    await_ready(
        client,
        gateway.processes,
        "api-gateway",
        gateway.metrics_address,
        Duration::from_secs(60),
    )
    .await?;
    eprintln!(
        "Gateway restarted for {budget:?} journal budget: GATEWAY_JOURNAL_CLIENT_TIMEOUT_MS={}",
        gateway
            .environment
            .get("GATEWAY_JOURNAL_CLIENT_TIMEOUT_MS")
            .map(String::as_str)
            .unwrap_or("default")
    );
    Ok(())
}

async fn verify_process_notification_tombstone_purge(
    deleted_subject_id: Uuid,
    notification_database_url: &str,
    notification_grpc_address: SocketAddr,
    post_environment: &BTreeMap<String, String>,
) -> Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(notification_database_url)
        .await?;
    let soft_deleted_rows: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from notifications
        where (user_id = $1 or actor_id = $1)
          and deleted_at is not null
          and delete_reason = 'account_deletion'
        "#,
    )
    .bind(deleted_subject_id)
    .fetch_one(&pool)
    .await?;
    anyhow::ensure!(
        soft_deleted_rows > 0,
        "account deletion did not leave notification content eligible for retention purge"
    );

    // Advance content retention only; leave the permanent subject tombstone
    // untouched so the real worker can prove it survives physical row purge.
    let expired_deadline = Utc::now() - ChronoDuration::seconds(1);
    let expired_rows = sqlx::query(
        r#"
        update notifications
        set retention_until = $2
        where (user_id = $1 or actor_id = $1)
          and deleted_at is not null
          and delete_reason = 'account_deletion'
        "#,
    )
    .bind(deleted_subject_id)
    .bind(expired_deadline)
    .execute(&pool)
    .await?
    .rows_affected();
    anyhow::ensure!(
        expired_rows > 0,
        "test could not advance deleted notification content to its expiry deadline"
    );

    let purge_deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let remaining: i64 = sqlx::query_scalar(
            r#"
            select count(*)
            from notifications
            where (user_id = $1 or actor_id = $1)
              and deleted_at is not null
              and delete_reason = 'account_deletion'
            "#,
        )
        .bind(deleted_subject_id)
        .fetch_one(&pool)
        .await?;
        if remaining == 0 {
            break;
        }
        if Instant::now() >= purge_deadline {
            anyhow::bail!(
                "Notification retention worker left {remaining} expired account-deletion rows"
            );
        }
        sleep(Duration::from_millis(50)).await;
    }

    let tombstone: Option<(bool, bool, Option<String>)> = sqlx::query_as(
        r#"
        select suppress_as_recipient, suppress_as_actor, reason
        from notification_subject_states
        where subject_id = $1
        "#,
    )
    .bind(deleted_subject_id)
    .fetch_optional(&pool)
    .await?;
    anyhow::ensure!(
        tombstone == Some((true, true, Some("account_deletion".into()))),
        "Notification purge removed or weakened the account-deletion tombstone: {tombstone:?}"
    );

    let post_tls = load_tls_material(post_environment, "post")?;
    let post_token = post_environment
        .get("POST_SERVICE_CREDENTIAL")
        .context("POST_SERVICE_CREDENTIAL is missing from the process environment")?;
    let channel = platform::grpc_tls::lazy_channel_with_optional_tls(
        Some(&post_tls),
        &grpc_endpoint(notification_grpc_address),
        Workload::Notification,
    )?;
    let interceptor = ServiceIdentityClientInterceptor::new(Workload::Post, post_token)?;
    let mut client = notification_proto::notification_service_client::NotificationServiceClient::
        with_interceptor(channel, interceptor);

    let late_recipient_event = Uuid::new_v4();
    let late_actor_event = Uuid::new_v4();
    for (recipient_id, actor_id, event_id) in [
        (deleted_subject_id, Uuid::new_v4(), late_recipient_event),
        (Uuid::new_v4(), deleted_subject_id, late_actor_event),
    ] {
        let outcome = client
            .create_notification(notification_proto::CreateNotificationRequest {
                user_id: recipient_id.to_string(),
                r#type: "SYSTEM".into(),
                payload_json: r#"{"kind":"late-account-deletion-event"}"#.into(),
                event_id: event_id.to_string(),
                actor_id: actor_id.to_string(),
                occurred_at: Utc::now().to_rfc3339(),
            })
            .await
            .context("Post workload could not submit a late Notification event")?
            .into_inner();
        anyhow::ensure!(
            outcome.suppressed && outcome.notification.is_none(),
            "Notification accepted a late event involving a deleted subject: {outcome:?}"
        );
    }

    let late_event_rows: i64 =
        sqlx::query_scalar("select count(*) from notifications where event_id = any($1::uuid[])")
            .bind(vec![late_recipient_event, late_actor_event])
            .fetch_one(&pool)
            .await?;
    anyhow::ensure!(
        late_event_rows == 0,
        "suppressed late process events left notification content behind"
    );
    pool.close().await;

    eprintln!(
        "process Notification tombstone/purge acceptance passed: deleted_subject_id={deleted_subject_id}, expired_rows={expired_rows}, late_recipient_and_actor_events_suppressed=true"
    );
    Ok(())
}

async fn recover_moderation_after_gateway_process_restart(
    client: &reqwest::Client,
    base: &str,
    gateway: &mut ServiceProcessRestart<'_>,
    owner_email: &str,
    owner: &mut ProcessUser,
    post_id: Uuid,
    report_id: Uuid,
) -> Result<()> {
    let auth_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.auth_database_url)
        .await?;
    let gateway_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.gateway_database_url)
        .await?;
    let post_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.post_database_url)
        .await?;

    sqlx::query(
        "insert into auth_user_roles (user_id, role, granted_by) values ($1, 'ROLE_ADMIN', $1) on conflict do nothing",
    )
    .bind(owner.user_id)
    .execute(&auth_pool)
    .await?;
    let (login_status, login) = process_json_request(
        client,
        Method::POST,
        base,
        "/auth/login",
        None,
        None,
        Some(json!({
            "email": owner_email,
            "password": "Valid-passphrase-42"
        })),
    )
    .await?;
    anyhow::ensure!(
        login_status == StatusCode::OK,
        "moderation admin re-login failed: {login_status} {login}"
    );
    owner.access_token = login["accessToken"]
        .as_str()
        .context("moderation admin login omitted its access token")?
        .to_owned();

    sqlx::query(
        r#"
        create function e2e_pause_moderation_decision_insert()
        returns trigger language plpgsql as $$
        begin
            perform pg_sleep(30);
            return new;
        end;
        $$
        "#,
    )
    .execute(&gateway_pool)
    .await?;
    sqlx::query(
        "create trigger e2e_pause_moderation_decision_insert before insert on moderation_decisions for each row execute function e2e_pause_moderation_decision_insert()",
    )
    .execute(&gateway_pool)
    .await?;

    let request_client = client.clone();
    let request_base = base.to_owned();
    let request_token = owner.access_token.clone();
    let request_path = format!("/admin/moderation/reports/{report_id}/decisions");
    let decision_request = tokio::spawn(async move {
        process_json_request(
            &request_client,
            Method::POST,
            &request_base,
            &request_path,
            Some(&request_token),
            None,
            Some(json!({
                "decisionType": "HIDE_CONTENT",
                "note": "E2E_GATEWAY_KILLED_DURING_FINALIZATION"
            })),
        )
        .await
    });

    let mut post_effect = None;
    let mut finalization_paused = false;
    let request_deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < request_deadline && !decision_request.is_finished() {
        post_effect = sqlx::query_scalar(
            "select moderation_report_id from posts where id = $1 and moderation_hidden_at is not null",
        )
        .bind(post_id)
        .fetch_optional(&post_pool)
        .await?;
        if post_effect == Some(report_id) {
            finalization_paused = sqlx::query_scalar(
                r#"
                select exists (
                    select 1 from pg_stat_activity
                    where datname = current_database()
                      and state = 'active'
                      and wait_event_type = 'Timeout'
                      and wait_event = 'PgSleep'
                      and lower(query) like '%insert into moderation_decisions%'
                )
                "#,
            )
            .fetch_one(&gateway_pool)
            .await?;
            if finalization_paused {
                break;
            }
        }
        sleep(Duration::from_millis(25)).await;
    }
    anyhow::ensure!(
        post_effect == Some(report_id),
        "PostService did not commit the moderation action before the Gateway request ended: {post_effect:?}"
    );
    anyhow::ensure!(
        finalization_paused && !decision_request.is_finished(),
        "Gateway was not observed in the in-flight decision-finalization pause"
    );

    // The PostService action has committed and Gateway is still inside the HTTP
    // handler, blocked in its decision insert. Kill the actual executable before
    // any HTTP response can resolve the caller's view of the outcome.
    gateway.processes.stop("api-gateway")?;
    let request_outcome = timeout(Duration::from_secs(5), decision_request)
        .await
        .context("in-flight moderation request did not observe Gateway termination")?
        .context("in-flight moderation request task panicked")?;
    let request_error = request_outcome.err().context(
        "Gateway returned an HTTP response even though the process was killed during finalization",
    )?;
    eprintln!("in-flight moderation request ended on Gateway death: {request_error:#}");

    sqlx::query("drop trigger e2e_pause_moderation_decision_insert on moderation_decisions")
        .execute(&gateway_pool)
        .await?;
    sqlx::query("drop function e2e_pause_moderation_decision_insert()")
        .execute(&gateway_pool)
        .await?;

    anyhow::ensure!(
        post_effect == Some(report_id),
        "PostService moderation action was not durable before Gateway process death: {post_effect:?}"
    );
    let gateway_decision_count: i64 =
        sqlx::query_scalar("select count(*) from moderation_decisions where report_id = $1")
            .bind(report_id)
            .fetch_one(&gateway_pool)
            .await?;
    anyhow::ensure!(
        gateway_decision_count == 0,
        "Gateway persisted a decision despite the injected finalization failure"
    );
    let outbox_state: String = sqlx::query_scalar(
        "select state from moderation_content_decision_outbox where report_id = $1",
    )
    .bind(report_id)
    .fetch_one(&gateway_pool)
    .await?;
    anyhow::ensure!(
        outbox_state == "PENDING",
        "Gateway did not retain retryable intent after finalization failure: {outbox_state}"
    );

    // The expired lease makes post-restart recovery deterministic without
    // waiting for the production 30-second lease duration.
    sqlx::query(
        r#"
        update moderation_content_decision_outbox
        set lease_owner = coalesce(lease_owner, $2),
            lease_expires_at = now() - interval '1 second',
            next_attempt_at = now() - interval '1 second'
        where report_id = $1 and state = 'PENDING'
        "#,
    )
    .bind(report_id)
    .bind(Uuid::new_v4())
    .execute(&gateway_pool)
    .await?;

    gateway.processes.spawn(
        "api-gateway",
        gateway.binary_dir.join("api-gateway"),
        gateway.environment,
    )?;
    await_ready(
        client,
        gateway.processes,
        "api-gateway",
        gateway.metrics_address,
        Duration::from_secs(60),
    )
    .await?;

    let mut reconciled = false;
    for _ in 0..120 {
        let outbox_state: String = sqlx::query_scalar(
            "select state from moderation_content_decision_outbox where report_id = $1",
        )
        .bind(report_id)
        .fetch_one(&gateway_pool)
        .await?;
        let decision_count: i64 =
            sqlx::query_scalar("select count(*) from moderation_decisions where report_id = $1")
                .bind(report_id)
                .fetch_one(&gateway_pool)
                .await?;
        if outbox_state == "COMPLETED" && decision_count == 1 {
            reconciled = true;
            break;
        }
        sleep(Duration::from_millis(500)).await;
    }
    anyhow::ensure!(
        reconciled,
        "fresh Gateway process did not reconcile the persisted moderation intent"
    );

    let post_effect_count: i64 = sqlx::query_scalar(
        "select count(*) from post_moderation_actions where report_id = $1 and action = 'HIDE'",
    )
    .bind(report_id)
    .fetch_one(&post_pool)
    .await?;
    anyhow::ensure!(
        post_effect_count == 1,
        "Gateway process restart duplicated the PostService action: {post_effect_count}"
    );
    let resolved_status: String =
        sqlx::query_scalar("select status from moderation_reports where id = $1")
            .bind(report_id)
            .fetch_one(&gateway_pool)
            .await?;
    anyhow::ensure!(
        resolved_status == "RESOLVED",
        "restarted Gateway did not resolve the moderation report: {resolved_status}"
    );

    auth_pool.close().await;
    gateway_pool.close().await;
    post_pool.close().await;
    eprintln!(
        "moderation in-flight process-restart acceptance passed: report_id={report_id}, post_effects={post_effect_count}, Gateway decisions=1"
    );
    Ok(())
}

async fn recover_accepted_post_after_gateway_process_restart(
    client: &reqwest::Client,
    base: &str,
    gateway: &mut ServiceProcessRestart<'_>,
    run_id: &str,
    owner: &ProcessUser,
    other_user: &ProcessUser,
) -> Result<()> {
    let gateway_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.gateway_database_url)
        .await?;
    let post_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.post_database_url)
        .await?;

    // Hold the business table so the owner request cannot complete before the
    // encrypted queue entry is committed and the real Gateway process is killed.
    let mut post_lock = post_pool.begin().await?;
    timeout(
        Duration::from_secs(10),
        sqlx::query("lock table posts in access exclusive mode").execute(&mut *post_lock),
    )
    .await
    .context("timed out acquiring the Post write barrier")??;

    let content = format!("gateway-restart-async-{run_id}");
    let idempotency_key = format!("gateway-restart-{run_id}");
    let accepted = client
        .post(format!("{base}/api/posts"))
        .bearer_auth(&owner.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .json(&json!({ "content": content, "visibility": "PUBLIC" }))
        .send()
        .await?;
    let accepted_status = accepted.status();
    let accepted_headers = accepted.headers().clone();
    let accepted_body: Value = accepted.json().await?;
    anyhow::ensure!(
        accepted_status == StatusCode::ACCEPTED
            && accepted_headers
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store")
            && accepted_headers.contains_key(reqwest::header::RETRY_AFTER),
        "blocked Post mutation was not durably accepted: {accepted_status} {accepted_body}"
    );
    let operation_id: Uuid = accepted_body["operationId"]
        .as_str()
        .context("durable acceptance omitted its operation id")?
        .parse()?;

    let pending: (String, String, i32, bool) = sqlx::query_as(
        r#"
        select journal.state, queue.state, queue.attempt_count,
               queue.payload_ciphertext is not null
        from gateway_request_journal journal
        join gateway_async_operation_queue queue on queue.operation_id = journal.id
        where journal.id = $1 and journal.actor_id = $2
        "#,
    )
    .bind(operation_id)
    .bind(owner.user_id.to_string())
    .fetch_one(&gateway_pool)
    .await?;
    anyhow::ensure!(
        pending.0 == "QUEUED" && matches!(pending.1.as_str(), "READY" | "LEASED") && pending.3,
        "202 did not leave a durable encrypted operation pending: {pending:?}"
    );

    gateway.processes.stop("api-gateway")?;
    post_lock.rollback().await?;

    // A killed process cannot renew its lease. Make recovery due now rather than
    // sleeping through the production 60-second lease in this acceptance.
    let reset_lease = sqlx::query(
        r#"
        update gateway_async_operation_queue
        set state = 'READY', available_at = now() - interval '1 second',
            lease_owner = null, lease_expires_at = null, updated_at = now()
        where operation_id = $1 and state in ('READY', 'LEASED')
        "#,
    )
    .bind(operation_id)
    .execute(&gateway_pool)
    .await?
    .rows_affected();
    anyhow::ensure!(
        reset_lease == 1,
        "pending operation disappeared after Gateway process termination"
    );

    gateway.processes.spawn(
        "api-gateway",
        gateway.binary_dir.join("api-gateway"),
        gateway.environment,
    )?;
    await_ready(
        client,
        gateway.processes,
        "api-gateway",
        gateway.metrics_address,
        Duration::from_secs(60),
    )
    .await?;

    let recovery_deadline = Instant::now() + Duration::from_secs(45);
    let recovered = loop {
        let state: (String, String, i32, bool, Option<i32>) = sqlx::query_as(
            r#"
            select queue.state, journal.state, queue.attempt_count,
                   queue.payload_ciphertext is null, journal.response_status
            from gateway_async_operation_queue queue
            join gateway_request_journal journal on journal.id = queue.operation_id
            where queue.operation_id = $1
            "#,
        )
        .bind(operation_id)
        .fetch_one(&gateway_pool)
        .await?;
        if state.0 == "DONE" && state.1 == "COMPLETED" {
            break state;
        }
        anyhow::ensure!(
            state.0 != "DEAD",
            "Gateway recovery dead-lettered the accepted operation: {state:?}"
        );
        if Instant::now() >= recovery_deadline {
            anyhow::bail!("restarted Gateway did not complete the queued operation: {state:?}");
        }
        sleep(Duration::from_millis(200)).await;
    };
    anyhow::ensure!(
        recovered.2 > pending.2 && recovered.3 && recovered.4 == Some(200),
        "Gateway restart did not reclaim and complete the encrypted operation: {recovered:?}"
    );

    let post_count: i64 =
        sqlx::query_scalar("select count(*) from posts where author_id = $1 and content = $2")
            .bind(owner.user_id)
            .bind(&content)
            .fetch_one(&post_pool)
            .await?;
    let receipt_count: i64 =
        sqlx::query_scalar("select count(*) from post_mutation_receipts where operation_id = $1")
            .bind(operation_id)
            .fetch_one(&post_pool)
            .await?;
    anyhow::ensure!(
        post_count == 1 && receipt_count == 1,
        "recovered operation must have exactly one business effect and owner receipt: posts={post_count}, receipts={receipt_count}"
    );

    let status_path = format!("/api/operations/{operation_id}");
    let owner_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&owner.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        owner_status.status() == StatusCode::OK
            && owner_status
                .headers()
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store"),
        "operation owner could not read the private recovered status"
    );
    let owner_status_body: Value = owner_status.json().await?;
    anyhow::ensure!(
        owner_status_body["state"] == "COMPLETED"
            && owner_status_body["responseStatus"] == 200
            && owner_status_body["replayable"] == true,
        "recovered owner status omitted its terminal result: {owner_status_body}"
    );

    let other_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&other_user.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        other_status.status() == StatusCode::NOT_FOUND,
        "another caller learned an operation status they do not own: {}",
        other_status.status()
    );

    gateway_pool.close().await;
    post_pool.close().await;
    eprintln!(
        "durable owner-operation process-restart acceptance passed: operation_id={operation_id}, attempts={}, business_effects={post_count}, owner_receipts={receipt_count}",
        recovered.2
    );
    Ok(())
}

async fn recover_accepted_post_after_gateway_backup_restore(
    client: &reqwest::Client,
    base: &str,
    gateway: &mut ServiceProcessRestart<'_>,
    run_id: &str,
    owner: &ProcessUser,
    other_user: &ProcessUser,
) -> Result<()> {
    let gateway_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.gateway_database_url)
        .await?;
    let post_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.post_database_url)
        .await?;

    // Keep the business effect blocked until the real Gateway has durably
    // accepted the request and stopped, so the backup contains pending work.
    let mut post_lock = post_pool.begin().await?;
    timeout(
        Duration::from_secs(10),
        sqlx::query("lock table posts in access exclusive mode").execute(&mut *post_lock),
    )
    .await
    .context("timed out acquiring the Post write barrier for Gateway restore")??;

    let content = format!("gateway-backup-restore-{run_id}");
    let idempotency_key = format!("gateway-backup-restore-{run_id}");
    let accepted = client
        .post(format!("{base}/api/posts"))
        .bearer_auth(&owner.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .json(&json!({ "content": content, "visibility": "PUBLIC" }))
        .send()
        .await?;
    let accepted_status = accepted.status();
    let accepted_headers = accepted.headers().clone();
    let accepted_body: Value = accepted.json().await?;
    anyhow::ensure!(
        accepted_status == StatusCode::ACCEPTED
            && accepted_headers
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store")
            && accepted_headers.contains_key(reqwest::header::RETRY_AFTER),
        "backup-restore Post mutation was not durably accepted: {accepted_status} {accepted_body}"
    );
    let operation_id: Uuid = accepted_body["operationId"]
        .as_str()
        .context("durable acceptance omitted its operation id")?
        .parse()?;

    let initial_pending: (String, String, i32, bool) = sqlx::query_as(
        r#"
        select journal.state, queue.state, queue.attempt_count,
               queue.payload_ciphertext is not null
        from gateway_request_journal journal
        join gateway_async_operation_queue queue on queue.operation_id = journal.id
        where journal.id = $1 and journal.actor_id = $2
        "#,
    )
    .bind(operation_id)
    .bind(owner.user_id.to_string())
    .fetch_one(&gateway_pool)
    .await?;
    anyhow::ensure!(
        initial_pending.0 == "QUEUED"
            && matches!(initial_pending.1.as_str(), "READY" | "LEASED")
            && initial_pending.3,
        "202 did not leave an encrypted operation pending for backup: {initial_pending:?}"
    );

    gateway.processes.stop("api-gateway")?;
    post_lock.rollback().await?;

    // Normalize only the test clock/lease: the pending command remains
    // encrypted and queued, but is immediately eligible after restore.
    let reset_lease = sqlx::query(
        r#"
        update gateway_async_operation_queue
        set state = 'READY', available_at = now() - interval '1 second',
            lease_owner = null, lease_expires_at = null, updated_at = now()
        where operation_id = $1 and state in ('READY', 'LEASED')
        "#,
    )
    .bind(operation_id)
    .execute(&gateway_pool)
    .await?
    .rows_affected();
    anyhow::ensure!(
        reset_lease == 1,
        "pending operation disappeared before the Gateway backup"
    );
    let source_pending = gateway_pending_snapshot(&gateway_pool, operation_id).await?;
    anyhow::ensure!(
        source_pending.0 == "QUEUED"
            && source_pending.1 == "READY"
            && source_pending.3.is_some()
            && source_pending.4.is_some(),
        "Gateway source database was not backup-ready with encrypted queued work"
    );
    gateway_pool.close().await;
    post_pool.close().await;

    restore_database_snapshot(
        gateway.postgres,
        gateway.gateway_database_name,
        gateway.gateway_restored_database_name,
        run_id,
        "gateway",
    )
    .await?;

    let source_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.gateway_database_url)
        .await?;
    let restored_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.gateway_restored_database_url)
        .await?;
    let source_ledger: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("select version, checksum from _sqlx_migrations order by version")
            .fetch_all(&source_pool)
            .await?;
    let restored_ledger: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("select version, checksum from _sqlx_migrations order by version")
            .fetch_all(&restored_pool)
            .await?;
    anyhow::ensure!(
        !source_ledger.is_empty() && restored_ledger == source_ledger,
        "restored Gateway database migration ledger differs from its source"
    );
    let restored_pending = gateway_pending_snapshot(&restored_pool, operation_id).await?;
    anyhow::ensure!(
        restored_pending == source_pending,
        "Gateway backup did not preserve the exact pending operation record"
    );

    let mut restored_environment = gateway.environment.clone();
    restored_environment.insert(
        "GATEWAY_DATABASE_URL".into(),
        if gateway
            .environment
            .get("GATEWAY_DATABASE_MIGRATION_MODE")
            .is_some_and(|mode| mode == "external")
        {
            runtime_roles::restored_gateway_url(
                &gateway.environment["GATEWAY_DATABASE_URL"],
                gateway.gateway_restored_database_url,
            )
            .await?
        } else {
            gateway.gateway_restored_database_url.to_owned()
        },
    );
    gateway.processes.spawn(
        "api-gateway",
        gateway.binary_dir.join("api-gateway"),
        &restored_environment,
    )?;
    await_ready(
        client,
        gateway.processes,
        "api-gateway",
        gateway.metrics_address,
        Duration::from_secs(60),
    )
    .await?;
    let restored_result =
        wait_for_completed_gateway_operation(&restored_pool, operation_id, "restored").await?;
    anyhow::ensure!(
        restored_result.0 > restored_pending.2 && restored_result.1 && restored_result.2 == Some(200),
        "restored Gateway did not reclaim and complete the encrypted operation: {restored_result:?}"
    );

    let post_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.post_database_url)
        .await?;
    let restored_effect_count: i64 =
        sqlx::query_scalar("select count(*) from posts where author_id = $1 and content = $2")
            .bind(owner.user_id)
            .bind(&content)
            .fetch_one(&post_pool)
            .await?;
    let restored_receipt_count: i64 =
        sqlx::query_scalar("select count(*) from post_mutation_receipts where operation_id = $1")
            .bind(operation_id)
            .fetch_one(&post_pool)
            .await?;
    anyhow::ensure!(
        restored_effect_count == 1 && restored_receipt_count == 1,
        "restored operation must create exactly one effect and receipt: posts={restored_effect_count}, receipts={restored_receipt_count}"
    );

    let restored_status = client
        .get(format!("{base}/api/operations/{operation_id}"))
        .bearer_auth(&owner.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        restored_status.status() == StatusCode::OK
            && restored_status
                .headers()
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store"),
        "owner could not read the operation after restoring the Gateway backup"
    );
    let restored_status_body: Value = restored_status.json().await?;
    anyhow::ensure!(
        restored_status_body["state"] == "COMPLETED"
            && restored_status_body["responseStatus"] == 200
            && restored_status_body["replayable"] == true,
        "restored owner status omitted the recovered result: {restored_status_body}"
    );

    // The pre-restore source database still contains the pending journal. Replay
    // it too: the durable Post receipt must prevent a second business effect.
    gateway.processes.stop("api-gateway")?;
    gateway.processes.spawn(
        "api-gateway",
        gateway.binary_dir.join("api-gateway"),
        gateway.environment,
    )?;
    await_ready(
        client,
        gateway.processes,
        "api-gateway",
        gateway.metrics_address,
        Duration::from_secs(60),
    )
    .await?;
    let source_result =
        wait_for_completed_gateway_operation(&source_pool, operation_id, "source replay").await?;
    anyhow::ensure!(
        source_result.0 > source_pending.2 && source_result.1 && source_result.2 == Some(200),
        "original Gateway source did not safely replay the restored operation: {source_result:?}"
    );

    let final_effect_count: i64 =
        sqlx::query_scalar("select count(*) from posts where author_id = $1 and content = $2")
            .bind(owner.user_id)
            .bind(&content)
            .fetch_one(&post_pool)
            .await?;
    let final_receipt_count: i64 =
        sqlx::query_scalar("select count(*) from post_mutation_receipts where operation_id = $1")
            .bind(operation_id)
            .fetch_one(&post_pool)
            .await?;
    anyhow::ensure!(
        final_effect_count == 1 && final_receipt_count == 1,
        "replaying both restored and source journals duplicated the effect: posts={final_effect_count}, receipts={final_receipt_count}"
    );

    let other_status = client
        .get(format!("{base}/api/operations/{operation_id}"))
        .bearer_auth(&other_user.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        other_status.status() == StatusCode::NOT_FOUND,
        "another caller learned a restored operation status they do not own: {}",
        other_status.status()
    );

    source_pool.close().await;
    restored_pool.close().await;
    post_pool.close().await;
    eprintln!(
        "durable owner-operation backup-restore acceptance passed: operation_id={operation_id}, restored_attempts={}, source_replay_attempts={}, business_effects={final_effect_count}, owner_receipts={final_receipt_count}",
        restored_result.0,
        source_result.0,
    );
    Ok(())
}

async fn gateway_pending_snapshot(
    pool: &sqlx::PgPool,
    operation_id: Uuid,
) -> Result<(String, String, i32, Option<Vec<u8>>, Option<Vec<u8>>)> {
    Ok(sqlx::query_as(
        r#"
        select journal.state, queue.state, queue.attempt_count,
               queue.payload_nonce, queue.payload_ciphertext
        from gateway_request_journal journal
        join gateway_async_operation_queue queue on queue.operation_id = journal.id
        where journal.id = $1
        "#,
    )
    .bind(operation_id)
    .fetch_one(pool)
    .await?)
}

async fn wait_for_completed_gateway_operation(
    pool: &sqlx::PgPool,
    operation_id: Uuid,
    context: &str,
) -> Result<(i32, bool, Option<i32>)> {
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        let state: (String, String, i32, bool, Option<i32>) = sqlx::query_as(
            r#"
            select queue.state, journal.state, queue.attempt_count,
                   queue.payload_ciphertext is null, journal.response_status
            from gateway_async_operation_queue queue
            join gateway_request_journal journal on journal.id = queue.operation_id
            where queue.operation_id = $1
            "#,
        )
        .bind(operation_id)
        .fetch_one(pool)
        .await?;
        if state.0 == "DONE" && state.1 == "COMPLETED" {
            return Ok((state.2, state.3, state.4));
        }
        anyhow::ensure!(
            state.0 != "DEAD",
            "{context} Gateway dead-lettered the accepted operation: {state:?}"
        );
        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for {context} Gateway operation recovery: {state:?}");
        }
        sleep(Duration::from_millis(200)).await;
    }
}

async fn restore_database_snapshot(
    postgres: &postgres::PostgresContainer,
    source_database: &str,
    restored_database: &str,
    run_id: &str,
    database_label: &str,
) -> Result<()> {
    let backup_path = format!("/tmp/port-rust-{database_label}-{run_id}.dump");
    run_postgres_container_command(
        postgres,
        vec![
            "pg_dump".into(),
            "--username=postgres".into(),
            "--format=custom".into(),
            "--no-owner".into(),
            "--no-acl".into(),
            format!("--file={backup_path}"),
            format!("--dbname={source_database}"),
        ],
        &format!("dumping {database_label} database snapshot"),
    )
    .await?;
    run_postgres_container_command(
        postgres,
        vec![
            "pg_restore".into(),
            "--username=postgres".into(),
            format!("--dbname={restored_database}"),
            "--no-owner".into(),
            "--no-acl".into(),
            "--exit-on-error".into(),
            backup_path.clone(),
        ],
        &format!("restoring {database_label} database snapshot"),
    )
    .await?;
    run_postgres_container_command(
        postgres,
        vec!["rm".into(), "-f".into(), "--".into(), backup_path],
        &format!("removing the run-owned {database_label} database backup"),
    )
    .await
}

async fn run_postgres_container_command(
    postgres: &postgres::PostgresContainer,
    command: Vec<String>,
    description: &str,
) -> Result<()> {
    let mut result = postgres
        .exec(ExecCommand::new(command).with_cmd_ready_condition(CmdWaitFor::exit()))
        .await
        .with_context(|| format!("could not start {description}"))?;
    let exit_code = result.exit_code().await?;
    let stdout = String::from_utf8(result.stdout_to_vec().await?)?;
    let stderr = String::from_utf8(result.stderr_to_vec().await?)?;
    anyhow::ensure!(
        exit_code == Some(0),
        "{description} failed with exit code {exit_code:?}; stdout={}; stderr={}",
        stdout.trim(),
        stderr.trim()
    );
    Ok(())
}

async fn recover_accepted_comment_after_gateway_process_restart(
    client: &reqwest::Client,
    base: &str,
    gateway: &mut ServiceProcessRestart<'_>,
    run_id: &str,
    post_id: Uuid,
    commenter: &ProcessUser,
    other_user: &ProcessUser,
) -> Result<()> {
    let gateway_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.gateway_database_url)
        .await?;
    let post_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.post_database_url)
        .await?;

    // Keep reads available to Post while the comment INSERT waits, so the
    // queue acceptance is caused by the write barrier rather than a missing
    // parent post or a separate dependency outage.
    let mut comment_lock = post_pool.begin().await?;
    timeout(
        Duration::from_secs(10),
        sqlx::query("lock table comments in share mode").execute(&mut *comment_lock),
    )
    .await
    .context("timed out acquiring the comment-write barrier")??;

    let path = format!("/api/posts/{post_id}/comments");
    let content = format!("gateway-restart-comment-{run_id}");
    let idempotency_key = format!("gateway-restart-comment-{run_id}");
    let observation = forensics::RequestObservation::begin(&Method::POST, &path);
    let accepted = client
        .post(format!("{base}{path}"))
        .header("x-request-id", observation.request_id.to_string())
        .bearer_auth(&commenter.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .json(&json!({ "content": content }))
        .send()
        .await?;
    let accepted_status = accepted.status();
    let accepted_headers = accepted.headers().clone();
    observation.response(accepted_status, &accepted_headers);
    drop(observation);
    let accepted_body: Value = accepted.json().await?;
    anyhow::ensure!(
        accepted_status == StatusCode::ACCEPTED
            && accepted_headers
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store")
            && accepted_headers.contains_key(reqwest::header::RETRY_AFTER),
        "blocked keyed comment was not durably accepted: {accepted_status} {accepted_body}"
    );
    let operation_id: Uuid = accepted_body["operationId"]
        .as_str()
        .context("durable comment acceptance omitted its operation id")?
        .parse()?;

    let pending: (String, String, i32, bool) = sqlx::query_as(
        r#"
        select journal.state, queue.state, queue.attempt_count,
               queue.payload_ciphertext is not null
        from gateway_request_journal journal
        join gateway_async_operation_queue queue on queue.operation_id = journal.id
        where journal.id = $1 and journal.actor_id = $2
        "#,
    )
    .bind(operation_id)
    .bind(commenter.user_id.to_string())
    .fetch_one(&gateway_pool)
    .await?;
    anyhow::ensure!(
        pending.0 == "QUEUED" && matches!(pending.1.as_str(), "READY" | "LEASED") && pending.3,
        "202 did not leave a durable encrypted comment command pending: {pending:?}"
    );

    let blocked_backend_pid = timeout(Duration::from_secs(20), async {
        loop {
            let waiting_pid: Option<i32> = sqlx::query_scalar(
                r#"
                select pid
                from pg_stat_activity
                where pid <> pg_backend_pid()
                  and datname = current_database()
                  and state = 'active'
                  and wait_event_type = 'Lock'
                  and query ilike '%insert into comments (%'
                order by query_start
                limit 1
                "#,
            )
            .fetch_optional(&post_pool)
            .await?;
            if let Some(pid) = waiting_pid {
                break Ok::<i32, anyhow::Error>(pid);
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .context("Post did not reach the blocked comment INSERT")??;

    gateway.processes.stop("api-gateway")?;
    comment_lock.rollback().await?;

    // The killed worker cannot renew its lease. Make the durable command
    // immediately claimable instead of waiting through the production lease.
    let reset_lease = sqlx::query(
        r#"
        update gateway_async_operation_queue
        set state = 'READY', available_at = now() - interval '1 second',
            lease_owner = null, lease_expires_at = null, updated_at = now()
        where operation_id = $1 and state in ('READY', 'LEASED')
        "#,
    )
    .bind(operation_id)
    .execute(&gateway_pool)
    .await?
    .rows_affected();
    anyhow::ensure!(
        reset_lease == 1,
        "comment queue item disappeared after Gateway termination"
    );

    gateway.processes.spawn(
        "api-gateway",
        gateway.binary_dir.join("api-gateway"),
        gateway.environment,
    )?;
    await_ready(
        client,
        gateway.processes,
        "api-gateway",
        gateway.metrics_address,
        Duration::from_secs(60),
    )
    .await?;

    let recovery_deadline = Instant::now() + Duration::from_secs(45);
    let recovered = loop {
        let state: (String, String, i32, bool, Option<i32>) = sqlx::query_as(
            r#"
            select queue.state, journal.state, queue.attempt_count,
                   queue.payload_ciphertext is null, journal.response_status
            from gateway_async_operation_queue queue
            join gateway_request_journal journal on journal.id = queue.operation_id
            where queue.operation_id = $1
            "#,
        )
        .bind(operation_id)
        .fetch_one(&gateway_pool)
        .await?;
        if state.0 == "DONE" && state.1 == "COMPLETED" {
            break state;
        }
        anyhow::ensure!(
            state.0 != "DEAD",
            "Gateway recovery dead-lettered the accepted comment: {state:?}"
        );
        if Instant::now() >= recovery_deadline {
            anyhow::bail!("restarted Gateway did not complete the queued comment: {state:?}");
        }
        sleep(Duration::from_millis(200)).await;
    };
    anyhow::ensure!(
        recovered.2 > pending.2 && recovered.3 && recovered.4 == Some(200),
        "Gateway restart did not durably complete the queued comment: {recovered:?}"
    );

    let comment_id: Uuid = sqlx::query_scalar(
        "select id from comments where post_id = $1 and author_id = $2 and content = $3",
    )
    .bind(post_id)
    .bind(commenter.user_id)
    .bind(&content)
    .fetch_one(&post_pool)
    .await?;
    let replay = client
        .post(format!("{base}{path}"))
        .bearer_auth(&commenter.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .json(&json!({ "content": content }))
        .send()
        .await?;
    let replay_status = replay.status();
    let replay_body: Value = replay.json().await?;
    anyhow::ensure!(
        replay_status.is_success() && replay_body["id"] == comment_id.to_string(),
        "replaying the recovered comment changed its result: {replay_status} {replay_body}"
    );

    let effects: (i64, i64, i64) = sqlx::query_as(
        r#"
        select
            (select count(*) from comments
             where post_id = $1 and author_id = $2 and content = $3),
            (select count(*) from post_event_outbox
             where event_type = 'POST_COMMENTED' and aggregate_id = $1
               and actor_id = $2 and payload like '%' || $3 || '%'),
            (select count(*) from post_mutation_receipts where operation_id = $4)
        "#,
    )
    .bind(post_id)
    .bind(commenter.user_id)
    .bind(&content)
    .bind(operation_id)
    .fetch_one(&post_pool)
    .await?;
    anyhow::ensure!(
        effects == (1, 1, 1),
        "comment recovery and replay did not leave exactly one row, event, and receipt: {effects:?}"
    );

    let status_path = format!("/api/operations/{operation_id}");
    let owner_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&commenter.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        owner_status.status() == StatusCode::OK
            && owner_status
                .headers()
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store"),
        "commenter could not read the private recovered operation status"
    );
    let owner_status_body: Value = owner_status.json().await?;
    anyhow::ensure!(
        owner_status_body["state"] == "COMPLETED"
            && owner_status_body["responseStatus"] == 200
            && owner_status_body["replayable"] == true,
        "recovered comment status omitted its terminal result: {owner_status_body}"
    );
    let other_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&other_user.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        other_status.status() == StatusCode::NOT_FOUND,
        "another caller learned a comment operation status they do not own: {}",
        other_status.status()
    );

    gateway_pool.close().await;
    post_pool.close().await;
    eprintln!(
        "durable owner-comment process-restart acceptance passed: operation_id={operation_id}, comment_id={comment_id}, effects={effects:?}, blocked_backend_pid={blocked_backend_pid}"
    );
    Ok(())
}

async fn recover_accepted_like_after_gateway_process_restart(
    client: &reqwest::Client,
    base: &str,
    gateway: &mut ServiceProcessRestart<'_>,
    run_id: &str,
    liker: &ProcessUser,
    post_author: &ProcessUser,
) -> Result<()> {
    let gateway_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.gateway_database_url)
        .await?;
    let post_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.post_database_url)
        .await?;

    let test_post_content = format!("gateway-restart-like-post-{run_id}");
    let (_, test_post) = process_json_request(
        client,
        Method::POST,
        base,
        "/api/posts",
        Some(&post_author.access_token),
        None,
        Some(json!({ "content": test_post_content, "visibility": "PUBLIC" })),
    )
    .await?;
    let liked_post_id: Uuid = test_post["id"]
        .as_str()
        .context("dedicated like-recovery post omitted its ID")?
        .parse()?;

    // Reads remain available while the owner insert is blocked, isolating the
    // durable acceptance to the real post_likes write path.
    let mut like_lock = post_pool.begin().await?;
    timeout(
        Duration::from_secs(10),
        sqlx::query("lock table post_likes in share mode").execute(&mut *like_lock),
    )
    .await
    .context("timed out acquiring the like-write barrier")??;

    let path = format!("/api/posts/{liked_post_id}/likes");
    let idempotency_key = format!("gateway-restart-like-{run_id}");
    let accepted = client
        .post(format!("{base}{path}"))
        .bearer_auth(&liker.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .send()
        .await?;
    let accepted_status = accepted.status();
    let accepted_headers = accepted.headers().clone();
    let accepted_body: Value = accepted.json().await?;
    anyhow::ensure!(
        accepted_status == StatusCode::ACCEPTED
            && accepted_headers
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store")
            && accepted_headers.contains_key(reqwest::header::RETRY_AFTER),
        "blocked keyed like was not durably accepted: {accepted_status} {accepted_body}"
    );
    let operation_id: Uuid = accepted_body["operationId"]
        .as_str()
        .context("durable like acceptance omitted its operation id")?
        .parse()?;

    let pending: (String, String, i32, bool) = sqlx::query_as(
        r#"
        select journal.state, queue.state, queue.attempt_count,
               queue.payload_ciphertext is not null
        from gateway_request_journal journal
        join gateway_async_operation_queue queue on queue.operation_id = journal.id
        where journal.id = $1 and journal.actor_id = $2
        "#,
    )
    .bind(operation_id)
    .bind(liker.user_id.to_string())
    .fetch_one(&gateway_pool)
    .await?;
    anyhow::ensure!(
        pending.0 == "QUEUED" && matches!(pending.1.as_str(), "READY" | "LEASED") && pending.3,
        "202 did not leave a durable encrypted like command pending: {pending:?}"
    );

    let blocked_backend_pid = timeout(Duration::from_secs(20), async {
        loop {
            let waiting_pid: Option<i32> = sqlx::query_scalar(
                r#"
                select pid
                from pg_stat_activity
                where pid <> pg_backend_pid()
                  and datname = current_database()
                  and state = 'active'
                  and wait_event_type = 'Lock'
                  and query ilike '%insert into post_likes (%'
                order by query_start
                limit 1
                "#,
            )
            .fetch_optional(&post_pool)
            .await?;
            if let Some(pid) = waiting_pid {
                break Ok::<i32, anyhow::Error>(pid);
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .context("Post did not reach the blocked like INSERT")??;

    gateway.processes.stop("api-gateway")?;
    like_lock.rollback().await?;

    let reset_lease = sqlx::query(
        r#"
        update gateway_async_operation_queue
        set state = 'READY', available_at = now() - interval '1 second',
            lease_owner = null, lease_expires_at = null, updated_at = now()
        where operation_id = $1 and state in ('READY', 'LEASED')
        "#,
    )
    .bind(operation_id)
    .execute(&gateway_pool)
    .await?
    .rows_affected();
    anyhow::ensure!(
        reset_lease == 1,
        "like queue item disappeared after Gateway termination"
    );

    gateway.processes.spawn(
        "api-gateway",
        gateway.binary_dir.join("api-gateway"),
        gateway.environment,
    )?;
    await_ready(
        client,
        gateway.processes,
        "api-gateway",
        gateway.metrics_address,
        Duration::from_secs(60),
    )
    .await?;

    let recovery_deadline = Instant::now() + Duration::from_secs(45);
    let recovered = loop {
        let state: (String, String, i32, bool, Option<i32>) = sqlx::query_as(
            r#"
            select queue.state, journal.state, queue.attempt_count,
                   queue.payload_ciphertext is null, journal.response_status
            from gateway_async_operation_queue queue
            join gateway_request_journal journal on journal.id = queue.operation_id
            where queue.operation_id = $1
            "#,
        )
        .bind(operation_id)
        .fetch_one(&gateway_pool)
        .await?;
        if state.0 == "DONE" && state.1 == "COMPLETED" {
            break state;
        }
        anyhow::ensure!(
            state.0 != "DEAD",
            "Gateway recovery dead-lettered the accepted like: {state:?}"
        );
        if Instant::now() >= recovery_deadline {
            anyhow::bail!("restarted Gateway did not complete the queued like: {state:?}");
        }
        sleep(Duration::from_millis(200)).await;
    };
    anyhow::ensure!(
        recovered.2 > pending.2 && recovered.3 && recovered.4 == Some(200),
        "Gateway restart did not durably complete the queued like: {recovered:?}"
    );

    let replay = client
        .post(format!("{base}{path}"))
        .bearer_auth(&liker.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .send()
        .await?;
    let replay_status = replay.status();
    let replay_body: Value = replay.json().await?;
    anyhow::ensure!(
        replay_status.is_success()
            && replay_body["postId"] == liked_post_id.to_string()
            && replay_body["liked"] == true
            && replay_body["likeCount"] == 1,
        "replaying the recovered like changed its result: {replay_status} {replay_body}"
    );

    let effects: (i64, i64, i64, i64) = sqlx::query_as(
        r#"
        select
            (select count(*) from post_likes where post_id = $1 and user_id = $2),
            (select count(*) from post_event_outbox
             where event_type = 'POST_LIKED' and aggregate_id = $1 and actor_id = $2),
            (select count(*) from post_mutation_receipts where operation_id = $3),
            (select like_count from posts where id = $1)
        "#,
    )
    .bind(liked_post_id)
    .bind(liker.user_id)
    .bind(operation_id)
    .fetch_one(&post_pool)
    .await?;
    anyhow::ensure!(
        effects == (1, 1, 1, 1),
        "like recovery and replay did not leave exactly one row, event, receipt, and count: {effects:?}"
    );

    let status_path = format!("/api/operations/{operation_id}");
    let owner_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&liker.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        owner_status.status() == StatusCode::OK
            && owner_status
                .headers()
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store"),
        "liker could not read the private recovered operation status"
    );
    let owner_status_body: Value = owner_status.json().await?;
    anyhow::ensure!(
        owner_status_body["state"] == "COMPLETED"
            && owner_status_body["responseStatus"] == 200
            && owner_status_body["replayable"] == true,
        "recovered like status omitted its terminal result: {owner_status_body}"
    );
    let other_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&post_author.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        other_status.status() == StatusCode::NOT_FOUND,
        "another caller learned a like operation status they do not own: {}",
        other_status.status()
    );

    gateway_pool.close().await;
    post_pool.close().await;
    eprintln!(
        "durable owner-like process-restart acceptance passed: operation_id={operation_id}, post_id={liked_post_id}, effects={effects:?}, blocked_backend_pid={blocked_backend_pid}"
    );
    Ok(())
}

async fn wait_for_timeline_like_projection(
    post_pool: &sqlx::PgPool,
    event_id: Uuid,
    post_id: Uuid,
    expected_like_events: i64,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let state: (bool, Option<i64>) = sqlx::query_as(
            r#"
            select exists(
                       select 1 from event_publications
                       where event_id = $1 and target = 'TIMELINE'
                   ),
                   (select like_events from post_engagement_velocity where post_id = $2)
            "#,
        )
        .bind(event_id)
        .bind(post_id)
        .fetch_one(post_pool)
        .await?;
        if state.0 && state.1 == Some(expected_like_events) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            anyhow::bail!(
                "timeline did not project event {event_id} for post {post_id}: {state:?}"
            );
        }
        sleep(Duration::from_millis(100)).await;
    }
}

async fn recover_accepted_unlike_after_gateway_process_restart(
    client: &reqwest::Client,
    base: &str,
    gateway: &mut ServiceProcessRestart<'_>,
    run_id: &str,
    liker: &ProcessUser,
    post_author: &ProcessUser,
) -> Result<()> {
    let gateway_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.gateway_database_url)
        .await?;
    let post_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.post_database_url)
        .await?;

    let test_post_content = format!("gateway-restart-unlike-post-{run_id}");
    let (_, test_post) = process_json_request(
        client,
        Method::POST,
        base,
        "/api/posts",
        Some(&post_author.access_token),
        None,
        Some(json!({ "content": test_post_content, "visibility": "PUBLIC" })),
    )
    .await?;
    let unliked_post_id: Uuid = test_post["id"]
        .as_str()
        .context("dedicated unlike-recovery post omitted its ID")?
        .parse()?;

    let like_path = format!("/api/posts/{unliked_post_id}/likes");
    let (like_status, initial_like) = process_json_request(
        client,
        Method::POST,
        base,
        &like_path,
        Some(&liker.access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        like_status == StatusCode::OK
            && initial_like["postId"] == unliked_post_id.to_string()
            && initial_like["liked"] == true
            && initial_like["likeCount"] == 1,
        "unlike recovery fixture did not start with exactly one like: {like_status} {initial_like}"
    );
    let like_event_id: Uuid = sqlx::query_scalar(
        "select event_id from post_event_outbox where aggregate_id = $1 and actor_id = $2 and event_type = 'POST_LIKED'",
    )
    .bind(unliked_post_id)
    .bind(liker.user_id)
    .fetch_one(&post_pool)
    .await?;
    wait_for_timeline_like_projection(&post_pool, like_event_id, unliked_post_id, 1).await?;

    // SHARE permits reads but blocks the actual owner DELETE, so acceptance and
    // recovery are proven against the write path rather than a synthetic error.
    let mut unlike_lock = post_pool.begin().await?;
    timeout(
        Duration::from_secs(10),
        sqlx::query("lock table post_likes in share mode").execute(&mut *unlike_lock),
    )
    .await
    .context("timed out acquiring the post-unlike write barrier")??;

    let idempotency_key = format!("gateway-restart-unlike-{run_id}");
    let accepted = client
        .delete(format!("{base}{like_path}"))
        .bearer_auth(&liker.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .send()
        .await?;
    let accepted_status = accepted.status();
    let accepted_headers = accepted.headers().clone();
    let accepted_body: Value = accepted.json().await?;
    anyhow::ensure!(
        accepted_status == StatusCode::ACCEPTED
            && accepted_headers
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store")
            && accepted_headers.contains_key(reqwest::header::RETRY_AFTER)
            && accepted_headers.contains_key(reqwest::header::LOCATION)
            && accepted_body["status"] == "ACCEPTED",
        "blocked keyed unlike was not durably accepted: {accepted_status} {accepted_body}"
    );
    let operation_id: Uuid = accepted_body["operationId"]
        .as_str()
        .context("durable unlike acceptance omitted its operation id")?
        .parse()?;

    let pending: (String, String, i32, bool) = sqlx::query_as(
        r#"
        select journal.state, queue.state, queue.attempt_count,
               queue.payload_ciphertext is not null
        from gateway_request_journal journal
        join gateway_async_operation_queue queue on queue.operation_id = journal.id
        where journal.id = $1 and journal.actor_id = $2
        "#,
    )
    .bind(operation_id)
    .bind(liker.user_id.to_string())
    .fetch_one(&gateway_pool)
    .await?;
    anyhow::ensure!(
        pending.0 == "QUEUED" && matches!(pending.1.as_str(), "READY" | "LEASED") && pending.3,
        "202 did not leave a durable encrypted unlike command pending: {pending:?}"
    );

    let blocked_backend_pid = timeout(Duration::from_secs(20), async {
        loop {
            let waiting_pid: Option<i32> = sqlx::query_scalar(
                r#"
                select pid
                from pg_stat_activity
                where pid <> pg_backend_pid()
                  and datname = current_database()
                  and state = 'active'
                  and wait_event_type = 'Lock'
                  and query ilike '%delete from post_likes%'
                order by query_start
                limit 1
                "#,
            )
            .fetch_optional(&post_pool)
            .await?;
            if let Some(pid) = waiting_pid {
                break Ok::<i32, anyhow::Error>(pid);
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .context("Post did not reach the blocked unlike DELETE")??;

    gateway.processes.stop("api-gateway")?;
    unlike_lock.rollback().await?;

    let reset_lease = sqlx::query(
        r#"
        update gateway_async_operation_queue
        set state = 'READY', available_at = now() - interval '1 second',
            lease_owner = null, lease_expires_at = null, updated_at = now()
        where operation_id = $1 and state in ('READY', 'LEASED')
        "#,
    )
    .bind(operation_id)
    .execute(&gateway_pool)
    .await?
    .rows_affected();
    anyhow::ensure!(
        reset_lease == 1,
        "unlike queue item disappeared after Gateway termination"
    );

    gateway.processes.spawn(
        "api-gateway",
        gateway.binary_dir.join("api-gateway"),
        gateway.environment,
    )?;
    await_ready(
        client,
        gateway.processes,
        "api-gateway",
        gateway.metrics_address,
        Duration::from_secs(60),
    )
    .await?;

    let recovery_deadline = Instant::now() + Duration::from_secs(45);
    let recovered = loop {
        let state: (String, String, i32, bool, Option<i32>) = sqlx::query_as(
            r#"
            select queue.state, journal.state, queue.attempt_count,
                   queue.payload_ciphertext is null, journal.response_status
            from gateway_async_operation_queue queue
            join gateway_request_journal journal on journal.id = queue.operation_id
            where queue.operation_id = $1
            "#,
        )
        .bind(operation_id)
        .fetch_one(&gateway_pool)
        .await?;
        if state.0 == "DONE" && state.1 == "COMPLETED" {
            break state;
        }
        anyhow::ensure!(
            state.0 != "DEAD",
            "Gateway recovery dead-lettered the accepted unlike: {state:?}"
        );
        if Instant::now() >= recovery_deadline {
            anyhow::bail!("restarted Gateway did not complete the queued unlike: {state:?}");
        }
        sleep(Duration::from_millis(200)).await;
    };
    anyhow::ensure!(
        recovered.2 > pending.2 && recovered.3 && recovered.4 == Some(200),
        "Gateway restart did not durably complete the queued unlike: {recovered:?}"
    );

    let replay = client
        .delete(format!("{base}{like_path}"))
        .bearer_auth(&liker.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .send()
        .await?;
    let replay_status = replay.status();
    let replay_headers = replay.headers().clone();
    let replay_body: Value = replay.json().await?;
    anyhow::ensure!(
        replay_status == StatusCode::OK
            && replay_headers
                .get("Idempotency-Replayed")
                .is_some_and(|value| value == "true")
            && replay_body["postId"] == unliked_post_id.to_string()
            && replay_body["liked"] == false
            && replay_body["likeCount"] == 0,
        "replaying the recovered unlike changed its result: {replay_status} {replay_body}"
    );

    let effects: (i64, i64, i64, i64) = sqlx::query_as(
        r#"
        select
            (select count(*) from post_likes where post_id = $1 and user_id = $2),
            (select like_count from posts where id = $1),
            (select count(*) from post_event_outbox
             where event_type = 'POST_UNLIKED' and aggregate_id = $1 and actor_id = $2),
            (select count(*) from post_mutation_receipts
             where operation_id = $3 and actor_id = $2
               and scope = 'post.v1.UnlikePost'
               and outcome_json->>'kind' = 'like'
               and outcome_json #>> '{outcome,post_id}' = $1::text
               and outcome_json #>> '{outcome,changed}' = 'true'
               and outcome_json #>> '{outcome,like_count}' = '0'
               and outcome_json #>> '{outcome,liked}' = 'false')
        "#,
    )
    .bind(unliked_post_id)
    .bind(liker.user_id)
    .bind(operation_id)
    .fetch_one(&post_pool)
    .await?;
    anyhow::ensure!(
        effects == (0, 0, 1, 1),
        "unlike recovery and replay did not leave exactly one removal, counter, event, and receipt: {effects:?}"
    );
    let unlike_event_id: Uuid = sqlx::query_scalar(
        "select event_id from post_event_outbox where aggregate_id = $1 and actor_id = $2 and event_type = 'POST_UNLIKED'",
    )
    .bind(unliked_post_id)
    .bind(liker.user_id)
    .fetch_one(&post_pool)
    .await?;
    wait_for_timeline_like_projection(&post_pool, unlike_event_id, unliked_post_id, 0).await?;
    let timeline_attempts: (i64, i64) = sqlx::query_as(
        "select count(*) filter (where phase = 'FAILED'), count(*) filter (where phase = 'SUCCEEDED') from post_event_delivery_attempts where event_id = $1 and target = 'TIMELINE'",
    )
    .bind(unlike_event_id)
    .fetch_one(&post_pool)
    .await?;
    anyhow::ensure!(
        timeline_attempts == (0, 1),
        "unlike timeline projection did not complete exactly once without retries: {timeline_attempts:?}"
    );

    let status_path = format!("/api/operations/{operation_id}");
    let owner_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&liker.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        owner_status.status() == StatusCode::OK
            && owner_status
                .headers()
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store"),
        "liker could not read the private recovered unlike status"
    );
    let owner_status_body: Value = owner_status.json().await?;
    anyhow::ensure!(
        owner_status_body["state"] == "COMPLETED"
            && owner_status_body["responseStatus"] == 200
            && owner_status_body["replayable"] == true,
        "recovered unlike status omitted its terminal result: {owner_status_body}"
    );
    let other_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&post_author.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        other_status.status() == StatusCode::NOT_FOUND,
        "another caller learned an unlike operation status they do not own: {}",
        other_status.status()
    );

    let (visible_status, visible_post) = process_json_request(
        client,
        Method::GET,
        base,
        &format!("/api/posts/{unliked_post_id}"),
        Some(&post_author.access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        visible_status == StatusCode::OK && visible_post["post"]["likeCount"] == 0,
        "unliking changed post visibility or left a stale public counter: {visible_status} {visible_post}"
    );

    gateway_pool.close().await;
    post_pool.close().await;
    eprintln!(
        "durable owner-unlike process-restart acceptance passed: operation_id={operation_id}, post_id={unliked_post_id}, effects={effects:?}, blocked_backend_pid={blocked_backend_pid}"
    );
    Ok(())
}

async fn recover_accepted_post_delete_after_gateway_process_restart(
    client: &reqwest::Client,
    base: &str,
    gateway: &mut ServiceProcessRestart<'_>,
    run_id: &str,
    author: &ProcessUser,
    other_user: &ProcessUser,
) -> Result<()> {
    let gateway_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.gateway_database_url)
        .await?;
    let post_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.post_database_url)
        .await?;

    let test_post_content = format!("gateway-restart-delete-post-{run_id}");
    let deleted_post_id = setup_post::create(
        client,
        base,
        &gateway_pool,
        &post_pool,
        author,
        &test_post_content,
    )
    .await?;

    // SHARE permits the ownership read but conflicts with Post's real UPDATE,
    // leaving Gateway with an accepted command whose owner write is unresolved.
    let mut delete_lock = post_pool.begin().await?;
    timeout(
        Duration::from_secs(10),
        sqlx::query("lock table posts in share mode").execute(&mut *delete_lock),
    )
    .await
    .context("timed out acquiring the post-delete write barrier")??;

    let path = format!("/api/posts/{deleted_post_id}");
    let idempotency_key = format!("gateway-restart-delete-{run_id}");
    let accepted = client
        .delete(format!("{base}{path}"))
        .bearer_auth(&author.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .send()
        .await?;
    let accepted_status = accepted.status();
    let accepted_headers = accepted.headers().clone();
    let accepted_body: Value = accepted.json().await?;
    anyhow::ensure!(
        accepted_status == StatusCode::ACCEPTED
            && accepted_headers
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store")
            && accepted_headers.contains_key(reqwest::header::RETRY_AFTER)
            && accepted_headers.contains_key(reqwest::header::LOCATION)
            && accepted_body["status"] == "ACCEPTED",
        "blocked keyed delete was not durably accepted: {accepted_status} {accepted_body}"
    );
    let operation_id: Uuid = accepted_body["operationId"]
        .as_str()
        .context("durable delete acceptance omitted its operation id")?
        .parse()?;

    let pending: (String, String, i32, bool) = sqlx::query_as(
        r#"
        select journal.state, queue.state, queue.attempt_count,
               queue.payload_ciphertext is not null
        from gateway_request_journal journal
        join gateway_async_operation_queue queue on queue.operation_id = journal.id
        where journal.id = $1 and journal.actor_id = $2
        "#,
    )
    .bind(operation_id)
    .bind(author.user_id.to_string())
    .fetch_one(&gateway_pool)
    .await?;
    anyhow::ensure!(
        pending.0 == "QUEUED" && matches!(pending.1.as_str(), "READY" | "LEASED") && pending.3,
        "202 did not leave a durable encrypted delete command pending: {pending:?}"
    );

    let blocked_backend_pid = timeout(Duration::from_secs(20), async {
        loop {
            let waiting_pid: Option<i32> = sqlx::query_scalar(
                r#"
                select pid
                from pg_stat_activity
                where pid <> pg_backend_pid()
                  and datname = current_database()
                  and state = 'active'
                  and wait_event_type = 'Lock'
                  and query ilike '%update posts%'
                order by query_start
                limit 1
                "#,
            )
            .fetch_optional(&post_pool)
            .await?;
            if let Some(pid) = waiting_pid {
                break Ok::<i32, anyhow::Error>(pid);
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .context("Post did not reach the blocked soft-delete UPDATE")??;

    gateway.processes.stop("api-gateway")?;
    delete_lock.rollback().await?;

    let reset_lease = sqlx::query(
        r#"
        update gateway_async_operation_queue
        set state = 'READY', available_at = now() - interval '1 second',
            lease_owner = null, lease_expires_at = null, updated_at = now()
        where operation_id = $1 and state in ('READY', 'LEASED')
        "#,
    )
    .bind(operation_id)
    .execute(&gateway_pool)
    .await?
    .rows_affected();
    anyhow::ensure!(
        reset_lease == 1,
        "delete queue item disappeared after Gateway termination"
    );

    gateway.processes.spawn(
        "api-gateway",
        gateway.binary_dir.join("api-gateway"),
        gateway.environment,
    )?;
    await_ready(
        client,
        gateway.processes,
        "api-gateway",
        gateway.metrics_address,
        Duration::from_secs(60),
    )
    .await?;

    let recovery_deadline = Instant::now() + Duration::from_secs(45);
    let recovered = loop {
        let state: (String, String, i32, bool, Option<i32>) = sqlx::query_as(
            r#"
            select queue.state, journal.state, queue.attempt_count,
                   queue.payload_ciphertext is null, journal.response_status
            from gateway_async_operation_queue queue
            join gateway_request_journal journal on journal.id = queue.operation_id
            where queue.operation_id = $1
            "#,
        )
        .bind(operation_id)
        .fetch_one(&gateway_pool)
        .await?;
        if state.0 == "DONE" && state.1 == "COMPLETED" {
            break state;
        }
        anyhow::ensure!(
            state.0 != "DEAD",
            "Gateway recovery dead-lettered the accepted delete: {state:?}"
        );
        if Instant::now() >= recovery_deadline {
            anyhow::bail!("restarted Gateway did not complete the queued delete: {state:?}");
        }
        sleep(Duration::from_millis(200)).await;
    };
    anyhow::ensure!(
        recovered.2 > pending.2 && recovered.3 && recovered.4 == Some(200),
        "Gateway restart did not durably complete the queued delete: {recovered:?}"
    );

    let replay = client
        .delete(format!("{base}{path}"))
        .bearer_auth(&author.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .send()
        .await?;
    let replay_status = replay.status();
    let replay_headers = replay.headers().clone();
    let replay_body: Value = replay.json().await?;
    anyhow::ensure!(
        replay_status == StatusCode::OK
            && replay_headers
                .get("Idempotency-Replayed")
                .is_some_and(|value| value == "true")
            && replay_body["postId"] == deleted_post_id.to_string()
            && replay_body["deleted"] == true,
        "replaying the recovered delete changed its result: {replay_status} {replay_body}"
    );

    let effects: (i64, i64, i64, i64) = sqlx::query_as(
        r#"
        select
            (select count(*) from posts
             where id = $1 and deleted_at is not null and deleted_by = $2
               and delete_reason = 'user_request' and retention_until > deleted_at),
            (select count(*) from post_event_outbox
             where event_type = 'POST_DELETED' and aggregate_id = $1 and actor_id = $2),
            (select count(*) from post_audit_events
             where action = 'POST_DELETED' and outcome = 'SUCCESS'
               and post_id = $1 and actor_id = $2),
            (select count(*) from post_mutation_receipts
             where operation_id = $3 and actor_id = $2
               and scope = 'post.v1.DeletePost'
               and outcome_json->>'kind' = 'deleted'
               and outcome_json #>> '{outcome,post_id}' = $1::text)
        "#,
    )
    .bind(deleted_post_id)
    .bind(author.user_id)
    .bind(operation_id)
    .fetch_one(&post_pool)
    .await?;
    anyhow::ensure!(
        effects == (1, 1, 1, 1),
        "delete recovery and replay did not leave exactly one soft-delete, event, audit, and receipt: {effects:?}"
    );

    let status_path = format!("/api/operations/{operation_id}");
    let owner_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&author.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        owner_status.status() == StatusCode::OK
            && owner_status
                .headers()
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store"),
        "post author could not read the private recovered delete status"
    );
    let owner_status_body: Value = owner_status.json().await?;
    anyhow::ensure!(
        owner_status_body["state"] == "COMPLETED"
            && owner_status_body["responseStatus"] == 200
            && owner_status_body["replayable"] == true,
        "recovered delete status omitted its terminal result: {owner_status_body}"
    );
    let other_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&other_user.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        other_status.status() == StatusCode::NOT_FOUND,
        "another caller learned a delete operation status they do not own: {}",
        other_status.status()
    );
    let hidden_post = client
        .get(format!("{base}{path}"))
        .bearer_auth(&other_user.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        hidden_post.status() == StatusCode::NOT_FOUND,
        "deleted public post remained visible after restart: {}",
        hidden_post.status()
    );

    gateway_pool.close().await;
    post_pool.close().await;
    eprintln!(
        "durable owner-delete process-restart acceptance passed: operation_id={operation_id}, post_id={deleted_post_id}, effects={effects:?}, blocked_backend_pid={blocked_backend_pid}"
    );
    Ok(())
}

async fn recover_accepted_follow_after_gateway_process_restart(
    client: &reqwest::Client,
    base: &str,
    gateway: &mut ServiceProcessRestart<'_>,
    run_id: &str,
    follower: &ProcessUser,
    followee: &ProcessUser,
) -> Result<()> {
    let gateway_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.gateway_database_url)
        .await?;
    let friend_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.friend_database_url)
        .await?;

    // This relationship already exists from the social journey. The keyed
    // Follow command still takes the receipt-backed owner path and executes its
    // upsert, so a table barrier deterministically holds the real write.
    let mut follow_lock = friend_pool.begin().await?;
    timeout(
        Duration::from_secs(10),
        sqlx::query("lock table follows in share mode").execute(&mut *follow_lock),
    )
    .await
    .context("timed out acquiring the Friend follow-write barrier")??;

    let path = "/api/follows";
    let idempotency_key = format!("gateway-restart-follow-{run_id}");
    let request_body = json!({ "targetUserId": followee.user_id.to_string() });
    let accepted = client
        .post(format!("{base}{path}"))
        .bearer_auth(&follower.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .json(&request_body)
        .send()
        .await?;
    let accepted_status = accepted.status();
    let accepted_headers = accepted.headers().clone();
    let accepted_body: Value = accepted.json().await?;
    anyhow::ensure!(
        accepted_status == StatusCode::ACCEPTED
            && accepted_headers
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store")
            && accepted_headers.contains_key(reqwest::header::RETRY_AFTER),
        "blocked keyed follow was not durably accepted: {accepted_status} {accepted_body}"
    );
    let operation_id: Uuid = accepted_body["operationId"]
        .as_str()
        .context("durable follow acceptance omitted its operation id")?
        .parse()?;

    let pending: (String, String, i32, bool) = sqlx::query_as(
        r#"
        select journal.state, queue.state, queue.attempt_count,
               queue.payload_ciphertext is not null
        from gateway_request_journal journal
        join gateway_async_operation_queue queue on queue.operation_id = journal.id
        where journal.id = $1 and journal.actor_id = $2
        "#,
    )
    .bind(operation_id)
    .bind(follower.user_id.to_string())
    .fetch_one(&gateway_pool)
    .await?;
    anyhow::ensure!(
        pending.0 == "QUEUED" && matches!(pending.1.as_str(), "READY" | "LEASED") && pending.3,
        "202 did not leave a durable encrypted follow command pending: {pending:?}"
    );

    let blocked_backend_pid = timeout(Duration::from_secs(20), async {
        loop {
            let waiting_pid: Option<i32> = sqlx::query_scalar(
                r#"
                select pid
                from pg_stat_activity
                where pid <> pg_backend_pid()
                  and datname = current_database()
                  and state = 'active'
                  and wait_event_type = 'Lock'
                  and query ilike '%insert into follows (%'
                order by query_start
                limit 1
                "#,
            )
            .fetch_optional(&friend_pool)
            .await?;
            if let Some(pid) = waiting_pid {
                break Ok::<i32, anyhow::Error>(pid);
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .context("FriendService did not reach the blocked follows upsert")??;

    gateway.processes.stop("api-gateway")?;
    follow_lock.rollback().await?;

    let reset_lease = sqlx::query(
        r#"
        update gateway_async_operation_queue
        set state = 'READY', available_at = now() - interval '1 second',
            lease_owner = null, lease_expires_at = null, updated_at = now()
        where operation_id = $1 and state in ('READY', 'LEASED')
        "#,
    )
    .bind(operation_id)
    .execute(&gateway_pool)
    .await?
    .rows_affected();
    anyhow::ensure!(
        reset_lease == 1,
        "follow queue item disappeared after Gateway termination"
    );

    gateway.processes.spawn(
        "api-gateway",
        gateway.binary_dir.join("api-gateway"),
        gateway.environment,
    )?;
    await_ready(
        client,
        gateway.processes,
        "api-gateway",
        gateway.metrics_address,
        Duration::from_secs(60),
    )
    .await?;

    let recovery_deadline = Instant::now() + Duration::from_secs(45);
    let recovered = loop {
        let state: (String, String, i32, bool, Option<i32>) = sqlx::query_as(
            r#"
            select queue.state, journal.state, queue.attempt_count,
                   queue.payload_ciphertext is null, journal.response_status
            from gateway_async_operation_queue queue
            join gateway_request_journal journal on journal.id = queue.operation_id
            where queue.operation_id = $1
            "#,
        )
        .bind(operation_id)
        .fetch_one(&gateway_pool)
        .await?;
        if state.0 == "DONE" && state.1 == "COMPLETED" {
            break state;
        }
        anyhow::ensure!(
            state.0 != "DEAD",
            "Gateway recovery dead-lettered the accepted follow: {state:?}"
        );
        if Instant::now() >= recovery_deadline {
            anyhow::bail!("restarted Gateway did not complete the queued follow: {state:?}");
        }
        sleep(Duration::from_millis(200)).await;
    };
    anyhow::ensure!(
        recovered.2 > pending.2 && recovered.3 && recovered.4 == Some(200),
        "Gateway restart did not durably complete the queued follow: {recovered:?}"
    );

    let replay = client
        .post(format!("{base}{path}"))
        .bearer_auth(&follower.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .json(&request_body)
        .send()
        .await?;
    let replay_status = replay.status();
    let replay_body: Value = replay.json().await?;
    anyhow::ensure!(
        replay_status.is_success()
            && replay_body["following"] == true
            && replay_body["status"] == "ACTIVE",
        "replaying the recovered follow changed its result: {replay_status} {replay_body}"
    );

    let effects: (i64, i64) = sqlx::query_as(
        r#"
        select
            (select count(*) from follows
             where follower_id = $1 and followee_id = $2 and deleted_at is null),
            (select count(*) from friend_mutation_receipts
             where operation_id = $3 and actor_id = $1
               and scope = 'friend.v1.FollowUser' and outcome_json is not null)
        "#,
    )
    .bind(follower.user_id)
    .bind(followee.user_id)
    .bind(operation_id)
    .fetch_one(&friend_pool)
    .await?;
    anyhow::ensure!(
        effects == (1, 1),
        "follow recovery and replay did not leave one active edge and owner receipt: {effects:?}"
    );

    let status_path = format!("/api/operations/{operation_id}");
    let owner_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&follower.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        owner_status.status() == StatusCode::OK
            && owner_status
                .headers()
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store"),
        "follower could not read the private recovered operation status"
    );
    let owner_status_body: Value = owner_status.json().await?;
    anyhow::ensure!(
        owner_status_body["state"] == "COMPLETED"
            && owner_status_body["responseStatus"] == 200
            && owner_status_body["replayable"] == true,
        "recovered follow status omitted its terminal result: {owner_status_body}"
    );
    let other_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&followee.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        other_status.status() == StatusCode::NOT_FOUND,
        "another caller learned a follow operation status they do not own: {}",
        other_status.status()
    );

    gateway_pool.close().await;
    friend_pool.close().await;
    eprintln!(
        "durable owner-follow process-restart acceptance passed: operation_id={operation_id}, follower_id={}, followee_id={}, effects={effects:?}, blocked_backend_pid={blocked_backend_pid}",
        follower.user_id,
        followee.user_id
    );
    Ok(())
}

async fn recover_accepted_unfollow_after_gateway_process_restart(
    client: &reqwest::Client,
    base: &str,
    gateway: &mut ServiceProcessRestart<'_>,
    run_id: &str,
    follower: &ProcessUser,
    followee: &ProcessUser,
) -> Result<()> {
    let gateway_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.gateway_database_url)
        .await?;
    let friend_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.friend_database_url)
        .await?;

    let active_before: i64 = sqlx::query_scalar(
        "select count(*) from follows where follower_id = $1 and followee_id = $2 and deleted_at is null",
    )
    .bind(follower.user_id)
    .bind(followee.user_id)
    .fetch_one(&friend_pool)
    .await?;
    anyhow::ensure!(
        active_before == 1,
        "unfollow restart fixture did not begin with exactly one active edge: {active_before}"
    );

    let mut follow_lock = friend_pool.begin().await?;
    timeout(
        Duration::from_secs(10),
        sqlx::query("lock table follows in share mode").execute(&mut *follow_lock),
    )
    .await
    .context("timed out acquiring the Friend follow-write barrier for unfollow")??;

    let path = format!("/api/follows/{}", followee.user_id);
    let idempotency_key = format!("gateway-restart-unfollow-{run_id}");
    let accepted = client
        .delete(format!("{base}{path}"))
        .bearer_auth(&follower.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .send()
        .await?;
    let accepted_status = accepted.status();
    let accepted_headers = accepted.headers().clone();
    let accepted_body: Value = accepted.json().await?;
    anyhow::ensure!(
        accepted_status == StatusCode::ACCEPTED
            && accepted_headers
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store")
            && accepted_headers.contains_key(reqwest::header::RETRY_AFTER),
        "blocked keyed unfollow was not durably accepted: {accepted_status} {accepted_body}"
    );
    let operation_id: Uuid = accepted_body["operationId"]
        .as_str()
        .context("durable unfollow acceptance omitted its operation id")?
        .parse()?;

    let pending: (String, String, i32, bool) = sqlx::query_as(
        r#"
        select journal.state, queue.state, queue.attempt_count,
               queue.payload_ciphertext is not null
        from gateway_request_journal journal
        join gateway_async_operation_queue queue on queue.operation_id = journal.id
        where journal.id = $1 and journal.actor_id = $2
        "#,
    )
    .bind(operation_id)
    .bind(follower.user_id.to_string())
    .fetch_one(&gateway_pool)
    .await?;
    anyhow::ensure!(
        pending.0 == "QUEUED" && matches!(pending.1.as_str(), "READY" | "LEASED") && pending.3,
        "202 did not leave a durable encrypted unfollow command pending: {pending:?}"
    );

    let blocked_backend_pid = timeout(Duration::from_secs(20), async {
        loop {
            let waiting_pid: Option<i32> = sqlx::query_scalar(
                r#"
                select pid
                from pg_stat_activity
                where pid <> pg_backend_pid()
                  and datname = current_database()
                  and state = 'active'
                  and wait_event_type = 'Lock'
                  and query ilike '%update follows%'
                order by query_start
                limit 1
                "#,
            )
            .fetch_optional(&friend_pool)
            .await?;
            if let Some(pid) = waiting_pid {
                break Ok::<i32, anyhow::Error>(pid);
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .context("FriendService did not reach the blocked follows update")??;

    gateway.processes.stop("api-gateway")?;
    follow_lock.rollback().await?;

    let reset_lease = sqlx::query(
        r#"
        update gateway_async_operation_queue
        set state = 'READY', available_at = now() - interval '1 second',
            lease_owner = null, lease_expires_at = null, updated_at = now()
        where operation_id = $1 and state in ('READY', 'LEASED')
        "#,
    )
    .bind(operation_id)
    .execute(&gateway_pool)
    .await?
    .rows_affected();
    anyhow::ensure!(
        reset_lease == 1,
        "unfollow queue item disappeared after Gateway termination"
    );

    gateway.processes.spawn(
        "api-gateway",
        gateway.binary_dir.join("api-gateway"),
        gateway.environment,
    )?;
    await_ready(
        client,
        gateway.processes,
        "api-gateway",
        gateway.metrics_address,
        Duration::from_secs(60),
    )
    .await?;

    let recovery_deadline = Instant::now() + Duration::from_secs(45);
    let recovered = loop {
        let state: (String, String, i32, bool, Option<i32>) = sqlx::query_as(
            r#"
            select queue.state, journal.state, queue.attempt_count,
                   queue.payload_ciphertext is null, journal.response_status
            from gateway_async_operation_queue queue
            join gateway_request_journal journal on journal.id = queue.operation_id
            where queue.operation_id = $1
            "#,
        )
        .bind(operation_id)
        .fetch_one(&gateway_pool)
        .await?;
        if state.0 == "DONE" && state.1 == "COMPLETED" {
            break state;
        }
        anyhow::ensure!(
            state.0 != "DEAD",
            "Gateway recovery dead-lettered the accepted unfollow: {state:?}"
        );
        if Instant::now() >= recovery_deadline {
            anyhow::bail!("restarted Gateway did not complete the queued unfollow: {state:?}");
        }
        sleep(Duration::from_millis(200)).await;
    };
    anyhow::ensure!(
        recovered.2 > pending.2 && recovered.3 && recovered.4 == Some(200),
        "Gateway restart did not durably complete the queued unfollow: {recovered:?}"
    );

    let replay = client
        .delete(format!("{base}{path}"))
        .bearer_auth(&follower.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .send()
        .await?;
    let replay_status = replay.status();
    let replayed = replay
        .headers()
        .get("idempotency-replayed")
        .is_some_and(|value| value == "true");
    let replay_body: Value = replay.json().await?;
    anyhow::ensure!(
        replay_status == StatusCode::OK
            && replayed
            && replay_body["following"] == false
            && replay_body["status"] == "INACTIVE",
        "replaying the recovered unfollow changed its result: {replay_status} {replay_body}"
    );

    let effects: (i64, i64, i64, i64) = sqlx::query_as(
        r#"
        select
            count(*) filter (where deleted_at is null),
            count(*) filter (where deleted_at is not null),
            count(*) filter (where deleted_by = $1 and delete_reason = 'unfollow'),
            (select count(*) from friend_mutation_receipts
             where operation_id = $3 and actor_id = $1
               and scope = 'friend.v1.UnfollowUser' and outcome_json is not null)
        from follows
        where follower_id = $1 and followee_id = $2
        "#,
    )
    .bind(follower.user_id)
    .bind(followee.user_id)
    .bind(operation_id)
    .fetch_one(&friend_pool)
    .await?;
    anyhow::ensure!(
        effects == (0, 1, 1, 1),
        "unfollow recovery and replay did not leave one tombstone and owner receipt: {effects:?}"
    );

    let status_path = format!("/api/operations/{operation_id}");
    let owner_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&follower.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        owner_status.status() == StatusCode::OK
            && owner_status
                .headers()
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store"),
        "follower could not read the private recovered unfollow status"
    );
    let owner_status_body: Value = owner_status.json().await?;
    anyhow::ensure!(
        owner_status_body["state"] == "COMPLETED"
            && owner_status_body["responseStatus"] == 200
            && owner_status_body["replayable"] == true,
        "recovered unfollow status omitted its terminal result: {owner_status_body}"
    );
    let other_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&followee.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        other_status.status() == StatusCode::NOT_FOUND,
        "another caller learned an unfollow operation status they do not own: {}",
        other_status.status()
    );

    gateway_pool.close().await;
    friend_pool.close().await;
    eprintln!(
        "durable owner-unfollow process-restart acceptance passed: operation_id={operation_id}, follower_id={}, followee_id={}, effects={effects:?}, blocked_backend_pid={blocked_backend_pid}",
        follower.user_id,
        followee.user_id
    );
    Ok(())
}

async fn recover_accepted_profile_privacy_after_gateway_process_restart(
    client: &reqwest::Client,
    base: &str,
    gateway: &mut ServiceProcessRestart<'_>,
    run_id: &str,
    profile_owner: &ProcessUser,
    other_user: &ProcessUser,
) -> Result<()> {
    let gateway_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.gateway_database_url)
        .await?;
    let profile_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.profile_database_url)
        .await?;

    let current_visibility: Option<String> = sqlx::query_scalar(
        "select account_visibility from profiles where user_id = $1 and deleted_at is null",
    )
    .bind(profile_owner.user_id)
    .fetch_optional(&profile_pool)
    .await?;
    anyhow::ensure!(
        current_visibility.as_deref() == Some("PRIVATE"),
        "profile privacy restart fixture must begin private: {current_visibility:?}"
    );

    let mut profile_lock = profile_pool.begin().await?;
    timeout(
        Duration::from_secs(10),
        sqlx::query("lock table profiles in share mode").execute(&mut *profile_lock),
    )
    .await
    .context("timed out acquiring the Profile privacy-write barrier")??;
    let blocker_backend_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *profile_lock)
        .await?;

    let path = "/api/profile/privacy";
    let idempotency_key = format!("gateway-restart-profile-privacy-{run_id}");
    let request_body = json!({ "accountVisibility": "PUBLIC" });
    let accepted = client
        .put(format!("{base}{path}"))
        .bearer_auth(&profile_owner.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .json(&request_body)
        .send()
        .await?;
    let accepted_status = accepted.status();
    let accepted_headers = accepted.headers().clone();
    let accepted_body: Value = accepted.json().await?;
    anyhow::ensure!(
        accepted_status == StatusCode::ACCEPTED
            && accepted_headers
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store")
            && accepted_headers.contains_key(reqwest::header::RETRY_AFTER),
        "blocked keyed Profile privacy update was not durably accepted: {accepted_status} {accepted_body}"
    );
    let operation_id: Uuid = accepted_body["operationId"]
        .as_str()
        .context("durable Profile privacy acceptance omitted its operation id")?
        .parse()?;

    let pending: (String, String, i32, bool) = sqlx::query_as(
        r#"
        select journal.state, queue.state, queue.attempt_count,
               queue.payload_ciphertext is not null
        from gateway_request_journal journal
        join gateway_async_operation_queue queue on queue.operation_id = journal.id
        where journal.id = $1 and journal.actor_id = $2
        "#,
    )
    .bind(operation_id)
    .bind(profile_owner.user_id.to_string())
    .fetch_one(&gateway_pool)
    .await?;
    anyhow::ensure!(
        pending.0 == "QUEUED" && matches!(pending.1.as_str(), "READY" | "LEASED") && pending.3,
        "202 did not leave an encrypted Profile privacy command pending: {pending:?}"
    );

    let blocked_backend_pid = timeout(Duration::from_secs(20), async {
        loop {
            let waiting: Option<(i32, bool)> = sqlx::query_as(
                r#"
                select activity.pid,
                       $1 = any(pg_blocking_pids(activity.pid)) as blocked_by_barrier
                from pg_stat_activity activity
                where activity.pid <> pg_backend_pid()
                  and activity.datname = current_database()
                  and activity.state = 'active'
                  and activity.wait_event_type = 'Lock'
                  and activity.query ilike '%update profiles%'
                  and activity.query ilike '%account_visibility%'
                order by activity.query_start
                limit 1
                "#,
            )
            .bind(blocker_backend_pid)
            .fetch_optional(&profile_pool)
            .await?;
            if let Some((pid, true)) = waiting {
                break Ok::<i32, anyhow::Error>(pid);
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .context("ProfileService did not reach the UPDATE blocked by the privacy barrier")??;

    gateway.processes.stop("api-gateway")?;
    profile_lock.rollback().await?;

    let reset_lease = sqlx::query(
        r#"
        update gateway_async_operation_queue
        set state = 'READY', available_at = now() - interval '1 second',
            lease_owner = null, lease_expires_at = null, updated_at = now()
        where operation_id = $1 and state in ('READY', 'LEASED')
        "#,
    )
    .bind(operation_id)
    .execute(&gateway_pool)
    .await?
    .rows_affected();
    anyhow::ensure!(
        reset_lease == 1,
        "Profile privacy queue item disappeared after Gateway termination"
    );

    gateway.processes.spawn(
        "api-gateway",
        gateway.binary_dir.join("api-gateway"),
        gateway.environment,
    )?;
    await_ready(
        client,
        gateway.processes,
        "api-gateway",
        gateway.metrics_address,
        Duration::from_secs(60),
    )
    .await?;

    let recovery_deadline = Instant::now() + Duration::from_secs(45);
    let recovered = loop {
        let state: (String, String, i32, bool, Option<i32>) = sqlx::query_as(
            r#"
            select queue.state, journal.state, queue.attempt_count,
                   queue.payload_ciphertext is null, journal.response_status
            from gateway_async_operation_queue queue
            join gateway_request_journal journal on journal.id = queue.operation_id
            where queue.operation_id = $1
            "#,
        )
        .bind(operation_id)
        .fetch_one(&gateway_pool)
        .await?;
        if state.0 == "DONE" && state.1 == "COMPLETED" {
            break state;
        }
        anyhow::ensure!(
            state.0 != "DEAD",
            "Gateway recovery dead-lettered the accepted Profile privacy update: {state:?}"
        );
        if Instant::now() >= recovery_deadline {
            anyhow::bail!(
                "restarted Gateway did not complete the queued Profile privacy update: {state:?}"
            );
        }
        sleep(Duration::from_millis(200)).await;
    };
    anyhow::ensure!(
        recovered.2 > pending.2 && recovered.3 && recovered.4 == Some(200),
        "Gateway restart did not durably complete the Profile privacy update: {recovered:?}"
    );

    let replay = client
        .put(format!("{base}{path}"))
        .bearer_auth(&profile_owner.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .json(&request_body)
        .send()
        .await?;
    let replay_status = replay.status();
    let replay_headers = replay.headers().clone();
    let replay_body: Value = replay.json().await?;
    anyhow::ensure!(
        replay_status == StatusCode::OK
            && replay_headers
                .get("idempotency-replayed")
                .is_some_and(|value| value == "true")
            && replay_body["accountVisibility"] == "PUBLIC",
        "replaying the recovered Profile privacy update changed its response: {replay_status} {replay_body}"
    );

    let effects: (String, i64, i64) = sqlx::query_as(
        r#"
        select profile.account_visibility,
               (select count(*) from profile_mutation_receipts
                where operation_id = $2 and actor_id = $1
                  and scope = 'profile.v1.UpdateProfilePrivacy'
                  and outcome_json is not null),
               (select count(*) from profile_audit_events
                where request_id = $3 and actor_id = $1 and profile_user_id = $1
                  and action = 'UPDATE_PRIVACY' and outcome = 'SUCCESS')
        from profiles profile
        where profile.user_id = $1 and profile.deleted_at is null
        "#,
    )
    .bind(profile_owner.user_id)
    .bind(operation_id)
    .bind(operation_id.to_string())
    .fetch_one(&profile_pool)
    .await?;
    anyhow::ensure!(
        effects == ("PUBLIC".to_owned(), 1, 1),
        "Profile privacy recovery and replay did not leave one public profile, owner receipt, and audit event: {effects:?}"
    );

    let status_path = format!("/api/operations/{operation_id}");
    let owner_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&profile_owner.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        owner_status.status() == StatusCode::OK
            && owner_status
                .headers()
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store"),
        "Profile owner could not read the private recovered operation status"
    );
    let owner_status_body: Value = owner_status.json().await?;
    anyhow::ensure!(
        owner_status_body["state"] == "COMPLETED"
            && owner_status_body["responseStatus"] == 200
            && owner_status_body["replayable"] == true,
        "recovered Profile privacy status omitted its terminal result: {owner_status_body}"
    );
    let other_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&other_user.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        other_status.status() == StatusCode::NOT_FOUND,
        "another caller learned a Profile privacy operation status they do not own: {}",
        other_status.status()
    );

    let (public_status, public_profile) = process_json_request(
        client,
        Method::GET,
        base,
        &format!("/api/profile/{}", profile_owner.user_id),
        None,
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        public_status == StatusCode::OK && public_profile["accountVisibility"] == "PUBLIC",
        "recovered public profile did not become visible anonymously: {public_status} {public_profile}"
    );

    gateway_pool.close().await;
    profile_pool.close().await;
    eprintln!(
        "durable Profile privacy process-restart acceptance passed: operation_id={operation_id}, profile_owner_id={}, effects={effects:?}, blocked_backend_pid={blocked_backend_pid}, blocker_backend_pid={blocker_backend_pid}",
        profile_owner.user_id
    );
    Ok(())
}

async fn recover_accepted_profile_edit_after_gateway_process_restart(
    client: &reqwest::Client,
    base: &str,
    gateway: &mut ServiceProcessRestart<'_>,
    run_id: &str,
    profile_owner: &ProcessUser,
    other_user: &ProcessUser,
    username: &str,
) -> Result<()> {
    let gateway_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.gateway_database_url)
        .await?;
    let profile_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(gateway.profile_database_url)
        .await?;

    let fixture_profile: Option<(String, String, String)> = sqlx::query_as(
        "select username, nickname, account_visibility from profiles where user_id = $1 and deleted_at is null",
    )
    .bind(profile_owner.user_id)
    .fetch_optional(&profile_pool)
    .await?;
    anyhow::ensure!(
        fixture_profile.as_ref().is_some_and(|profile| {
            profile.0 == username && profile.1 == "Process Follower" && profile.2 == "PUBLIC"
        }),
        "Profile edit restart fixture did not begin with the expected public profile: {fixture_profile:?}"
    );

    let mut profile_lock = profile_pool.begin().await?;
    timeout(
        Duration::from_secs(10),
        sqlx::query("lock table profiles in share mode").execute(&mut *profile_lock),
    )
    .await
    .context("timed out acquiring the Profile edit-write barrier")??;
    let blocker_backend_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *profile_lock)
        .await?;

    let path = format!("/api/profile/{}", profile_owner.user_id);
    let idempotency_key = format!("gateway-restart-profile-edit-{run_id}");
    let nickname = "Recovered Process Follower";
    let avatar_url = "https://example.invalid/recovered-process-follower.png";
    let bio = "Profile edit recovered after Gateway restart";
    let request_body = json!({
        "username": username,
        "nickname": nickname,
        "avatarUrl": avatar_url,
        "bio": bio
    });

    // Exercise the bounded enqueue budget under database latency. The owner
    // receipt has a 500 ms lock timeout; this 60 ms queue-write pause must fit
    // after the 300 ms caller wait without converting acceptance to a 504.
    sqlx::query("create table e2e_profile_edit_enqueue_probe (operation_id uuid primary key)")
        .execute(&gateway_pool)
        .await?;
    sqlx::query(
        r#"
        create function e2e_delay_profile_edit_enqueue() returns trigger
        language plpgsql security definer set search_path=pg_catalog,public,pg_temp as $$
        begin
            if new.request_uri like '/api/profile/%' then
                perform pg_sleep(0.060);
                insert into e2e_profile_edit_enqueue_probe (operation_id)
                values (new.operation_id);
            end if;
            return new;
        end;
        $$
        "#,
    )
    .execute(&gateway_pool)
    .await?;
    sqlx::query(
        "create trigger e2e_delay_profile_edit_enqueue before insert on gateway_async_operation_queue for each row execute function e2e_delay_profile_edit_enqueue()",
    )
    .execute(&gateway_pool)
    .await?;

    let accepted = client
        .put(format!("{base}{path}"))
        .bearer_auth(&profile_owner.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .json(&request_body)
        .send()
        .await?;
    let accepted_status = accepted.status();
    let accepted_headers = accepted.headers().clone();
    let accepted_body: Value = accepted.json().await?;
    anyhow::ensure!(
        accepted_status == StatusCode::ACCEPTED
            && accepted_headers
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store")
            && accepted_headers.contains_key(reqwest::header::RETRY_AFTER),
        "blocked keyed Profile edit was not durably accepted: {accepted_status} {accepted_body}"
    );
    let operation_id: Uuid = accepted_body["operationId"]
        .as_str()
        .context("durable Profile edit acceptance omitted its operation id")?
        .parse()?;
    let delayed_enqueue_count: i64 = sqlx::query_scalar(
        "select count(*) from e2e_profile_edit_enqueue_probe where operation_id = $1",
    )
    .bind(operation_id)
    .fetch_one(&gateway_pool)
    .await?;
    anyhow::ensure!(
        delayed_enqueue_count == 1,
        "Profile edit was not accepted through the injected queue-write delay: {delayed_enqueue_count}"
    );
    sqlx::query("drop trigger e2e_delay_profile_edit_enqueue on gateway_async_operation_queue")
        .execute(&gateway_pool)
        .await?;
    sqlx::query("drop function e2e_delay_profile_edit_enqueue()")
        .execute(&gateway_pool)
        .await?;
    sqlx::query("drop table e2e_profile_edit_enqueue_probe")
        .execute(&gateway_pool)
        .await?;

    let pending: (String, String, i32, bool) = sqlx::query_as(
        r#"
        select journal.state, queue.state, queue.attempt_count,
               queue.payload_ciphertext is not null
        from gateway_request_journal journal
        join gateway_async_operation_queue queue on queue.operation_id = journal.id
        where journal.id = $1 and journal.actor_id = $2
        "#,
    )
    .bind(operation_id)
    .bind(profile_owner.user_id.to_string())
    .fetch_one(&gateway_pool)
    .await?;
    anyhow::ensure!(
        pending.0 == "QUEUED" && matches!(pending.1.as_str(), "READY" | "LEASED") && pending.3,
        "202 did not leave an encrypted Profile edit command pending: {pending:?}"
    );

    let blocked_backend_pid = timeout(Duration::from_secs(20), async {
        loop {
            let waiting: Option<(i32, bool)> = sqlx::query_as(
                r#"
                select activity.pid,
                       $1 = any(pg_blocking_pids(activity.pid)) as blocked_by_barrier
                from pg_stat_activity activity
                where activity.pid <> pg_backend_pid()
                  and activity.datname = current_database()
                  and activity.state = 'active'
                  and activity.wait_event_type = 'Lock'
                  and activity.query ilike '%insert into profiles%'
                  and activity.query ilike '%on conflict%'
                order by activity.query_start
                limit 1
                "#,
            )
            .bind(blocker_backend_pid)
            .fetch_optional(&profile_pool)
            .await?;
            if let Some((pid, true)) = waiting {
                break Ok::<i32, anyhow::Error>(pid);
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .context("ProfileService did not reach the upsert blocked by the edit barrier")??;

    gateway.processes.stop("api-gateway")?;
    profile_lock.rollback().await?;

    let reset_lease = sqlx::query(
        r#"
        update gateway_async_operation_queue
        set state = 'READY', available_at = now() - interval '1 second',
            lease_owner = null, lease_expires_at = null, updated_at = now()
        where operation_id = $1 and state in ('READY', 'LEASED')
        "#,
    )
    .bind(operation_id)
    .execute(&gateway_pool)
    .await?
    .rows_affected();
    anyhow::ensure!(
        reset_lease == 1,
        "Profile edit queue item disappeared after Gateway termination"
    );

    gateway.processes.spawn(
        "api-gateway",
        gateway.binary_dir.join("api-gateway"),
        gateway.environment,
    )?;
    await_ready(
        client,
        gateway.processes,
        "api-gateway",
        gateway.metrics_address,
        Duration::from_secs(60),
    )
    .await?;

    let recovery_deadline = Instant::now() + Duration::from_secs(45);
    let recovered = loop {
        let state: (String, String, i32, bool, Option<i32>) = sqlx::query_as(
            r#"
            select queue.state, journal.state, queue.attempt_count,
                   queue.payload_ciphertext is null, journal.response_status
            from gateway_async_operation_queue queue
            join gateway_request_journal journal on journal.id = queue.operation_id
            where queue.operation_id = $1
            "#,
        )
        .bind(operation_id)
        .fetch_one(&gateway_pool)
        .await?;
        if state.0 == "DONE" && state.1 == "COMPLETED" {
            break state;
        }
        anyhow::ensure!(
            state.0 != "DEAD",
            "Gateway recovery dead-lettered the accepted Profile edit: {state:?}"
        );
        if Instant::now() >= recovery_deadline {
            anyhow::bail!("restarted Gateway did not complete the queued Profile edit: {state:?}");
        }
        sleep(Duration::from_millis(200)).await;
    };
    anyhow::ensure!(
        recovered.2 > pending.2 && recovered.3 && recovered.4 == Some(200),
        "Gateway restart did not durably complete the Profile edit: {recovered:?}"
    );

    let replay = client
        .put(format!("{base}{path}"))
        .bearer_auth(&profile_owner.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .json(&request_body)
        .send()
        .await?;
    let replay_status = replay.status();
    let replay_headers = replay.headers().clone();
    let replay_body: Value = replay.json().await?;
    anyhow::ensure!(
        replay_status == StatusCode::OK
            && replay_headers
                .get("idempotency-replayed")
                .is_some_and(|value| value == "true")
            && replay_body["username"] == username
            && replay_body["nickname"] == nickname
            && replay_body["avatarUrl"] == avatar_url
            && replay_body["bio"] == bio,
        "replaying the recovered Profile edit changed its response: {replay_status} {replay_body}"
    );

    let effects: (String, String, String, String, String, i64) = sqlx::query_as(
        r#"
        select profile.username, profile.nickname, profile.avatar_url,
               profile.bio, profile.account_visibility,
               (select count(*) from profile_mutation_receipts
                where operation_id = $2 and actor_id = $1
                  and scope = 'profile.v1.UpdateProfile'
                  and outcome_json is not null)
        from profiles profile
        where profile.user_id = $1 and profile.deleted_at is null
        "#,
    )
    .bind(profile_owner.user_id)
    .bind(operation_id)
    .fetch_one(&profile_pool)
    .await?;
    anyhow::ensure!(
        effects
            == (
                username.to_owned(),
                nickname.to_owned(),
                avatar_url.to_owned(),
                bio.to_owned(),
                "PUBLIC".to_owned(),
                1,
            ),
        "Profile edit recovery and replay did not leave one expected profile and owner receipt: {effects:?}"
    );

    let status_path = format!("/api/operations/{operation_id}");
    let owner_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&profile_owner.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        owner_status.status() == StatusCode::OK
            && owner_status
                .headers()
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store"),
        "Profile editor could not read the private recovered operation status"
    );
    let owner_status_body: Value = owner_status.json().await?;
    anyhow::ensure!(
        owner_status_body["state"] == "COMPLETED"
            && owner_status_body["responseStatus"] == 200
            && owner_status_body["replayable"] == true,
        "recovered Profile edit status omitted its terminal result: {owner_status_body}"
    );
    let other_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&other_user.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        other_status.status() == StatusCode::NOT_FOUND,
        "another caller learned a Profile edit operation status they do not own: {}",
        other_status.status()
    );

    let (public_status, public_profile) =
        process_json_request(client, Method::GET, base, &path, None, None, None).await?;
    anyhow::ensure!(
        public_status == StatusCode::OK
            && public_profile["username"] == username
            && public_profile["nickname"] == nickname
            && public_profile["avatarUrl"] == avatar_url
            && public_profile["bio"] == bio
            && public_profile["accountVisibility"] == "PUBLIC",
        "recovered Profile edit was not visible with its committed state: {public_status} {public_profile}"
    );

    gateway_pool.close().await;
    profile_pool.close().await;
    eprintln!(
        "durable Profile edit process-restart acceptance passed: operation_id={operation_id}, profile_owner_id={}, receipt_count={}, blocked_backend_pid={blocked_backend_pid}, blocker_backend_pid={blocker_backend_pid}",
        profile_owner.user_id,
        effects.5
    );
    Ok(())
}

async fn recover_post_after_kafka_ack_process_crash(
    client: &reqwest::Client,
    base: &str,
    restart: &mut ServiceProcessRestart<'_>,
    run_id: &str,
    owner: &ProcessUser,
) -> Result<()> {
    let post_pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(restart.post_database_url)
        .await?;

    let result =
        exercise_post_kafka_ack_crash(client, base, restart, run_id, owner, &post_pool).await;
    // Each run owns this disposable database. On error stop its worker before
    // removing the test trigger; never return a session advisory lock to a pool.
    if result.is_err() {
        let _ = restart.processes.stop("post-service");
    }
    let cleanup = timeout(Duration::from_secs(10), async {
        sqlx::query("drop trigger if exists e2e_pause_post_external_success on post_event_delivery_attempts")
            .execute(&post_pool).await?;
        sqlx::query("drop function if exists e2e_pause_post_external_success()")
            .execute(&post_pool).await?;
        Ok::<_, anyhow::Error>(())
    }).await.context("Kafka crash barrier cleanup timed out").and_then(|value| value);
    post_pool.close().await;
    result?;
    cleanup
}

async fn exercise_post_kafka_ack_crash(
    client: &reqwest::Client,
    base: &str,
    restart: &mut ServiceProcessRestart<'_>,
    run_id: &str,
    owner: &ProcessUser,
    post_pool: &sqlx::PgPool,
) -> Result<()> {
    use post_service::{
        adapters::out::persistence::delivery_schedule::PgDeliverySchedule,
        application::{
            dto::delivery_schedule_dto::{DeliveryLeaseOutcomeDto, DeliveryScheduleConfigDto},
            ports::DeliverySchedulePort,
        },
        domain::post::DeliveryTarget,
    };

    timeout(Duration::from_secs(45), async {
        loop {
            restart.processes.ensure_running("post-service")?;
            let pending: i64 = sqlx::query_scalar("select count(*) from post_event_outbox where not published and dead_lettered_at is null")
                .fetch_one(post_pool).await?;
            if pending == 0 { return Ok::<_, anyhow::Error>(()); }
            sleep(Duration::from_millis(100)).await;
        }
    }).await.context("Post outbox did not drain before Kafka ACK crash setup")??;
    let mut paused = restart.post_environment.clone();
    paused.insert("POST_OUTBOX_WORKER_ENABLED".into(), "false".into());
    restart.processes.stop("post-service")?;
    restart.processes.spawn(
        "post-service",
        restart.binary_dir.join("post-service"),
        &paused,
    )?;
    await_ready(
        client,
        restart.processes,
        "post-service",
        restart.post_metrics_address,
        Duration::from_secs(60),
    )
    .await?;

    let content = format!("kafka-ack-crash-{run_id}");
    let (create_status, created_post) = process_json_request(
        client,
        Method::POST,
        base,
        "/api/posts",
        Some(&owner.access_token),
        None,
        Some(json!({ "content": content, "visibility": "PUBLIC" })),
    )
    .await?;
    anyhow::ensure!(
        create_status.is_success(),
        "could not create the Post event used by the Kafka-ack crash test: {create_status} {created_post}"
    );
    let post_id: Uuid = created_post["id"]
        .as_str()
        .context("Post create response omitted its ID")?
        .parse()?;
    let event_id: Uuid = sqlx::query_scalar(
        "select event_id from post_event_outbox where aggregate_id = $1 and event_type = 'POST_CREATED'",
    )
    .bind(post_id)
    .fetch_one(post_pool)
    .await?;

    // A real automatic CACHE claim, deliberately orphaned by the controller.
    // This is a controlled sibling-lease scenario, not a diagnosis of a prior run.
    let schedule = PgDeliverySchedule::new(post_pool.clone());
    // Eligibility compares the stored creation time (supplied by the owner).
    // Use that exact cutoff without assuming host and database clocks agree.
    let claim_cutoff: DateTime<Utc> =
        sqlx::query_scalar("select created_at from post_event_outbox where event_id=$1")
            .bind(event_id)
            .fetch_one(post_pool)
            .await?;
    let mut batch = schedule
        .claim_target(
            DeliveryTarget::Cache,
            claim_cutoff,
            1,
            &DeliveryScheduleConfigDto {
                external_enabled: true,
                ..Default::default()
            },
        )
        .await?;
    let cache_claim = batch
        .claims
        .pop()
        .context("missing controlled CACHE claim")?;
    anyhow::ensure!(
        batch.claims.is_empty() && cache_claim.event.event_id == event_id,
        "CACHE claim did not belong to the isolated crash event"
    );
    // Let real TIMELINE and INTERNAL effects finish before EXTERNAL takes the
    // parent lock at its success barrier. No sibling checkpoint is fabricated.
    let delayed = sqlx::query("update post_event_delivery_jobs set next_attempt_at=clock_timestamp()+interval '5 minutes' where event_id=$1 and target='EXTERNAL' and state='PENDING' and attempt_count=0")
        .bind(event_id).execute(post_pool).await?.rows_affected();
    anyhow::ensure!(
        delayed == 1,
        "EXTERNAL was already attempted during controlled setup"
    );
    let mut setup = post_pool.begin().await?;

    let key_a =
        (u32::from_be_bytes(event_id.as_bytes()[..4].try_into()?) & 0x7fff_ffff).max(1) as i32;
    let key_b =
        (u32::from_be_bytes(event_id.as_bytes()[4..8].try_into()?) & 0x7fff_ffff).max(1) as i32;
    let mut advisory_lock = post_pool.acquire().await?.detach();
    sqlx::query("select pg_advisory_lock($1, $2)")
        .bind(key_a)
        .bind(key_b)
        .execute(&mut advisory_lock)
        .await?;
    let lock_holder_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut advisory_lock)
        .await?;

    let kafka = ClientBuilder::new(vec![restart.kafka_bootstrap.to_owned()])
        .build()
        .await?;
    let partition = kafka
        .partition_client(
            restart.kafka_topic.to_owned(),
            stable_partition(post_id, restart.kafka_partition_count),
            UnknownTopicHandling::Retry,
        )
        .await?;
    let mut next_offset = partition.get_offset(OffsetAt::Latest).await?;

    let function_sql = format!(
        "CREATE OR REPLACE FUNCTION e2e_pause_post_external_success() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.event_id = '{event_id}'::uuid AND NEW.target = 'EXTERNAL' AND NEW.phase = 'SUCCEEDED' THEN PERFORM pg_advisory_xact_lock({key_a}, {key_b}); END IF; RETURN NEW; END; $$"
    );
    sqlx::query(&function_sql).execute(&mut *setup).await?;
    sqlx::query(
        "create trigger e2e_pause_post_external_success after insert on post_event_delivery_attempts for each row execute function e2e_pause_post_external_success()",
    )
    .execute(&mut *setup)
    .await?;
    setup.commit().await?;

    restart.processes.stop("post-service")?;
    restart.processes.spawn(
        "post-service",
        restart.binary_dir.join("post-service"),
        restart.post_environment,
    )?;
    await_ready(
        client,
        restart.processes,
        "post-service",
        restart.post_metrics_address,
        Duration::from_secs(60),
    )
    .await?;
    timeout(Duration::from_secs(45), async {
        loop {
            let successes: i64 = sqlx::query_scalar("select count(*) from post_event_delivery_jobs where event_id=$1 and target in ('TIMELINE','INTERNAL') and state='SUCCEEDED'")
                .bind(event_id).fetch_one(post_pool).await?;
            if successes == 2 { return Ok::<_, anyhow::Error>(()); }
            sleep(Duration::from_millis(100)).await;
        }
    }).await.context("local sibling targets did not complete before the Kafka ACK barrier")??;
    let released = sqlx::query("update post_event_delivery_jobs set next_attempt_at=clock_timestamp() where event_id=$1 and target='EXTERNAL' and state='PENDING' and attempt_count=0")
        .bind(event_id).execute(post_pool).await?.rows_affected();
    anyhow::ensure!(
        released == 1,
        "EXTERNAL delay expired before controlled release"
    );

    let blocked_backend_pid = timeout(Duration::from_secs(45), async {
        loop {
            let waiting_pid: Option<i32> = sqlx::query_scalar(
                "select pid from pg_locks where locktype = 'advisory' and granted = false and classid::bigint = $1 and objid::bigint = $2 and objsubid = 2 and pid <> $3 limit 1",
            )
            .bind(i64::from(key_a))
            .bind(i64::from(key_b))
            .bind(lock_holder_pid)
            .fetch_optional(post_pool)
            .await?;
            if let Some(pid) = waiting_pid {
                break Ok::<i32, anyhow::Error>(pid);
            }
            sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .context("Post worker did not reach the external-success checkpoint trigger")??;

    let first_delivery = wait_for_kafka_event_records(
        &partition,
        &mut next_offset,
        event_id,
        1,
        Duration::from_secs(45),
    )
    .await?;
    anyhow::ensure!(
        first_delivery.len() == 1
            && first_delivery[0].event_type == "POST_CREATED"
            && first_delivery[0].aggregate_id == post_id,
        "Kafka did not contain exactly one valid target event before the Post crash: {first_delivery:?}"
    );

    let blocked_state: (bool, i64, i64, i64) = sqlx::query_as(
        r#"
        select published,
               (select count(*) from post_event_delivery_attempts where event_id = $1 and target = 'EXTERNAL' and phase = 'STARTED'),
               (select count(*) from post_event_delivery_attempts where event_id = $1 and target = 'EXTERNAL' and phase = 'SUCCEEDED'),
               (select count(*) from event_publications where event_id = $1 and target = 'EXTERNAL')
        from post_event_outbox where event_id = $1
        "#,
    )
    .bind(event_id)
    .fetch_one(post_pool)
    .await?;
    anyhow::ensure!(
        !blocked_state.0
            && blocked_state.1 == 1
            && blocked_state.2 == 0
            && blocked_state.3 == 0,
        "the Kafka record was not isolated between broker acknowledgement and local checkpoint: {blocked_state:?}"
    );

    let external_claim: (Uuid, Uuid) = sqlx::query_as("select lease_token, attempt_id from post_event_delivery_jobs where event_id=$1 and target='EXTERNAL' and state='LEASED'")
        .bind(event_id).fetch_one(post_pool).await?;

    restart.processes.stop("post-service")?;
    // The test trigger deliberately suspends the checkpoint INSERT inside the
    // database. Killing Post closes the client, but PostgreSQL may not observe
    // that disconnect while the statement is blocked in the trigger; terminate
    // that one backend to abort the in-flight checkpoint transaction.
    let _backend_terminated: bool = sqlx::query_scalar("select pg_terminate_backend($1)")
        .bind(blocked_backend_pid)
        .fetch_one(post_pool)
        .await?;
    timeout(Duration::from_secs(10), async {
        loop {
            let backend_state: (bool, bool) = sqlx::query_as(
                "select exists (select 1 from pg_locks where locktype = 'advisory' and granted = false and classid::bigint = $1 and objid::bigint = $2 and objsubid = 2 and pid = $3), exists (select 1 from pg_stat_activity where pid = $3)",
            )
            .bind(i64::from(key_a))
            .bind(i64::from(key_b))
            .bind(blocked_backend_pid)
            .fetch_one(post_pool)
            .await?;
            if !backend_state.0 && !backend_state.1 {
                break Ok::<(), anyhow::Error>(());
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .context("blocked Post checkpoint backend did not terminate after process death")??;

    let unlocked: bool = sqlx::query_scalar("select pg_advisory_unlock($1, $2)")
        .bind(key_a)
        .bind(key_b)
        .fetch_one(&mut advisory_lock)
        .await?;
    anyhow::ensure!(
        unlocked,
        "test advisory barrier was not held by its session"
    );
    drop(advisory_lock);

    sqlx::query("drop trigger e2e_pause_post_external_success on post_event_delivery_attempts")
        .execute(post_pool)
        .await?;
    sqlx::query("drop function e2e_pause_post_external_success()")
        .execute(post_pool)
        .await?;

    let crashed_state: (bool, i64, i64, i64) = sqlx::query_as(
        r#"
        select published,
               (select count(*) from post_event_delivery_attempts where event_id = $1 and target = 'EXTERNAL' and phase = 'STARTED'),
               (select count(*) from post_event_delivery_attempts where event_id = $1 and target = 'EXTERNAL' and phase = 'SUCCEEDED'),
               (select count(*) from event_publications where event_id = $1 and target = 'EXTERNAL')
        from post_event_outbox where event_id = $1
        "#,
    )
    .bind(event_id)
    .fetch_one(post_pool)
    .await?;
    anyhow::ensure!(
        !crashed_state.0
            && crashed_state.1 == 1
            && crashed_state.2 == 0
            && crashed_state.3 == 0,
        "Post terminated after Kafka acknowledgement but committed local checkpoint state: {crashed_state:?}"
    );

    // The broker acknowledgement is uncertain to Post, so its EXTERNAL lease
    // survives the crash. Age only that lease past expiry in database time so
    // the restarted worker reclaims it through normal stale-lease recovery.
    let expired_lease = sqlx::query(
        "update post_event_delivery_jobs set lease_expires_at = clock_timestamp() - interval '1 second' where event_id = $1 and target = 'EXTERNAL' and state = 'LEASED' and lease_token=$2 and attempt_id=$3 and lease_expires_at > clock_timestamp()",
    )
    .bind(event_id)
    .bind(external_claim.0)
    .bind(external_claim.1)
    .execute(post_pool)
    .await?
    .rows_affected();
    anyhow::ensure!(
        expired_lease == 1,
        "the interrupted EXTERNAL delivery lease was not left available for recovery"
    );

    restart.processes.spawn(
        "post-service",
        restart.binary_dir.join("post-service"),
        restart.post_environment,
    )?;
    await_ready(
        client,
        restart.processes,
        "post-service",
        restart.post_metrics_address,
        Duration::from_secs(60),
    )
    .await?;

    let recovery_deadline = Instant::now() + Duration::from_secs(45);
    let recovery = tokio::time::timeout_at(recovery_deadline, async {
    log_post_delivery_recovery(post_pool, event_id, "after-restart").await;
    timeout(recovery_deadline.saturating_duration_since(Instant::now()), async {
        loop {
            let state = read_post_kafka_delivery_state(post_pool, event_id).await?;
            anyhow::ensure!(!state.published && !state.dead_lettered,
                "parent finalized before its held CACHE claim recovered: {state:?}");
            if state.external_starts == 2 && state.external_successes == 1 && state.external_publications == 1 {
                anyhow::ensure!(state.timeline_successes == 1 && state.internal_successes == 1
                    && state.cache_successes == 0 && state.cache_publications == 0,
                    "sibling preconditions changed during EXTERNAL recovery: {state:?}");
                let held: bool = sqlx::query_scalar("select exists(select 1 from post_event_delivery_jobs where event_id=$1 and target='CACHE' and state='LEASED' and lease_token=$2 and attempt_id=$3 and lease_expires_at > clock_timestamp()+interval '45 seconds')")
                    .bind(event_id).bind(cache_claim.lease_token).bind(cache_claim.attempt_id).fetch_one(post_pool).await?;
                anyhow::ensure!(held, "controlled CACHE lease was not live beyond the recovery budget");
                break Ok::<_, anyhow::Error>(());
            }
            sleep(Duration::from_millis(100)).await;
        }
    }).await.context("EXTERNAL did not recover while the CACHE sibling stayed leased")??;
    log_post_delivery_recovery(post_pool, event_id, "external-recovered-cache-still-live").await;
    let expired_cache = sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and target='CACHE' and state='LEASED' and lease_token=$2 and attempt_id=$3 and lease_expires_at>clock_timestamp()")
        .bind(event_id).bind(cache_claim.lease_token).bind(cache_claim.attempt_id).execute(post_pool).await?.rows_affected();
    anyhow::ensure!(
        expired_cache == 1,
        "controlled CACHE ownership changed before expiry"
    );
    anyhow::ensure!(
        schedule.complete_target(&cache_claim).await? == DeliveryLeaseOutcomeDto::Lost,
        "expired controller claim retained completion authority"
    );

    loop {
        let state: (bool, i32, bool) = sqlx::query_as(
            "select published, attempt_count, dead_lettered_at is not null from post_event_outbox where event_id = $1",
        )
        .bind(event_id)
        .fetch_one(post_pool)
        .await?;
        if state.0 {
            break;
        }
        anyhow::ensure!(
            !state.2,
            "restarted Post dead-lettered the Kafka-acknowledged event: {state:?}"
        );
        sleep(Duration::from_millis(200)).await;
    }

    Ok::<_, anyhow::Error>(())
    }).await;
    if !matches!(&recovery, Ok(Ok(()))) {
        log_post_delivery_recovery(post_pool, event_id, "recovery-failed").await;
    }
    recovery.context("Post did not complete both recovery phases within 45 seconds")??;

    let replay_delivery = wait_for_kafka_event_records(
        &partition,
        &mut next_offset,
        event_id,
        1,
        Duration::from_secs(30),
    )
    .await?;
    let mut observed = first_delivery;
    observed.extend(replay_delivery);
    anyhow::ensure!(
        observed.len() == 2
            && observed.iter().all(|event| {
                event.event_id == event_id
                    && event.event_type == "POST_CREATED"
                    && event.aggregate_id == post_id
            }),
        "Post restart did not replay exactly the same Kafka event ID: {observed:?}"
    );

    let recovered_state: (i64, i64, i64, i64) = sqlx::query_as(
        r#"
        select (select count(*) from post_event_delivery_attempts where event_id = $1 and target = 'EXTERNAL' and phase = 'STARTED'),
               (select count(*) from post_event_delivery_attempts where event_id = $1 and target = 'EXTERNAL' and phase = 'SUCCEEDED'),
               (select count(*) from event_publications where event_id = $1 and target = 'EXTERNAL'),
               (select count(*) from post_event_outbox where event_id = $1 and published = true)
        "#,
    )
    .bind(event_id)
    .fetch_one(post_pool)
    .await?;
    anyhow::ensure!(
        recovered_state == (2, 1, 1, 1),
        "Post restart did not durably checkpoint one replay after the ambiguous broker acknowledgement: {recovered_state:?}"
    );

    let complete = read_post_kafka_delivery_state(post_pool, event_id).await?;
    anyhow::ensure!(
        complete.published
            && !complete.dead_lettered
            && complete.timeline_successes == 1
            && complete.internal_successes == 1
            && complete.cache_successes == 1
            && complete.timeline_publications == 1
            && complete.internal_publications == 1
            && complete.cache_publications == 1,
        "recovery did not complete every required target: {complete:?}"
    );
    let cache_recovery: (i64, i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from post_event_delivery_jobs where event_id=$1 and state='SUCCEEDED'), (select count(*) from post_event_delivery_attempts where event_id=$1 and target='CACHE' and phase='STARTED'), (select count(*) from post_event_delivery_attempts where event_id=$1 and target='CACHE' and phase='SUCCEEDED'), (select count(*) from post_event_delivery_attempts where attempt_id=$2 and phase<>'STARTED')")
        .bind(event_id).bind(cache_claim.attempt_id).fetch_one(post_pool).await?;
    anyhow::ensure!(
        cache_recovery == (4, 2, 1, 0),
        "CACHE recovery did not fence the old claim and complete all four jobs: {cache_recovery:?}"
    );
    sleep(Duration::from_millis(2_200)).await;
    restart.processes.ensure_running("post-service")?;
    let stable = read_post_kafka_delivery_state(post_pool, event_id).await?;
    let latest = partition.get_offset(OffsetAt::Latest).await?;
    anyhow::ensure!(stable == complete && latest == next_offset,
        "Kafka ACK recovery changed on later worker ticks: before={complete:?}, after={stable:?}, latest={latest}, observed={next_offset}");
    eprintln!(
        "Post Kafka-ack process-crash acceptance passed: event_id={event_id}, broker_records=2, same_event_id=true, durable_external_checkpoint=1"
    );
    Ok(())
}

// Read only the controlled event's scheduling facts; never payloads or credentials.
// Failure to capture must not replace the original recovery assertion.
async fn log_post_delivery_recovery(pool: &sqlx::PgPool, event_id: Uuid, stage: &str) {
    let observation = timeout(
        Duration::from_secs(3),
        sqlx::query_scalar::<_, Value>(
            r#"select jsonb_build_object(
            'eventId', event.event_id, 'databaseNow', clock_timestamp(),
            'published', event.published, 'requiredTargets', event.delivery_required_targets,
            'deadLettered', event.dead_lettered_at is not null,
            'jobs', coalesce((select jsonb_agg(jsonb_build_object(
                'target', job.target, 'state', job.state, 'leaseKind', job.lease_kind,
                'leaseExpiresAt', job.lease_expires_at, 'nextAttemptAt', job.next_attempt_at,
                'attemptCount', job.attempt_count, 'progressCredits', job.progress_credits,
                'maxAttempts', job.max_attempts,
                'hasPublication', exists(select 1 from event_publications p
                    where p.event_id=job.event_id and p.target=job.target),
                'started', (select count(*) from post_event_delivery_attempts a
                    where a.event_id=job.event_id and a.target=job.target and a.phase='STARTED'),
                'succeeded', (select count(*) from post_event_delivery_attempts a
                    where a.event_id=job.event_id and a.target=job.target and a.phase='SUCCEEDED')
            ) order by job.target) from post_event_delivery_jobs job
              where job.event_id=event.event_id), '[]'::jsonb)
        ) from post_event_outbox event where event.event_id=$1"#,
        )
        .bind(event_id)
        .fetch_one(pool),
    )
    .await;
    use std::io::Write;
    match observation {
        Ok(Ok(value)) => {
            let _ = writeln!(
                std::io::stderr().lock(),
                "POST_DELIVERY_RECOVERY stage={stage} {value}"
            );
        }
        Ok(Err(_)) => {
            let _ = writeln!(
                std::io::stderr().lock(),
                "POST_DELIVERY_RECOVERY stage={stage} event_id={event_id} observation=unavailable"
            );
        }
        Err(_) => {
            let _ = writeln!(
                std::io::stderr().lock(),
                "POST_DELIVERY_RECOVERY stage={stage} event_id={event_id} observation=timed_out"
            );
        }
    }
}

async fn recover_post_database_pool_saturation(
    client: &reqwest::Client,
    base: &str,
    restart: &mut ServiceProcessRestart<'_>,
    run_id: &str,
    owner: &ProcessUser,
) -> Result<()> {
    const ACQUIRE_TIMEOUT_MS: &str = "150";

    let gateway_pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(restart.gateway_database_url)
        .await?;
    let post_pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(restart.post_database_url)
        .await?;

    // Keep only request work on the one-connection pool so unrelated periodic
    // workers cannot create a false saturation or steal the connection under
    // test. The normal service environment is restored before this journey
    // continues.
    let mut constrained_environment = restart.post_environment.clone();
    constrained_environment.insert("POST_DB_POOL_MAX_CONNECTIONS".into(), "1".into());
    constrained_environment.insert(
        "POST_DB_POOL_ACQUIRE_TIMEOUT_MS".into(),
        ACQUIRE_TIMEOUT_MS.into(),
    );
    for worker in [
        "POST_OUTBOX_WORKER_ENABLED",
        "POST_RETENTION_PURGE_WORKER_ENABLED",
        "POST_TIMELINE_FANOUT_WORKER_ENABLED",
        "POST_FEED_RETENTION_WORKER_ENABLED",
    ] {
        constrained_environment.insert(worker.into(), "false".into());
    }

    restart.processes.stop("post-service")?;
    restart.processes.spawn(
        "post-service",
        restart.binary_dir.join("post-service"),
        &constrained_environment,
    )?;
    await_ready(
        client,
        restart.processes,
        "post-service",
        restart.post_metrics_address,
        Duration::from_secs(60),
    )
    .await?;

    // SHARE permits inspection reads but conflicts with Post's INSERT, leaving
    // one real Post process request holding its sole pool connection.
    let mut post_write_barrier = post_pool.begin().await?;
    timeout(
        Duration::from_secs(10),
        sqlx::query("lock table posts in share mode").execute(&mut *post_write_barrier),
    )
    .await
    .context("timed out acquiring the Post pool-saturation write barrier")??;

    let blocked_content = format!("pool-saturation-blocked-{run_id}");
    let blocked_key = format!("pool-saturation-blocked-{run_id}");
    let accepted = client
        .post(format!("{base}/api/posts"))
        .bearer_auth(&owner.access_token)
        .header("Idempotency-Key", &blocked_key)
        .json(&json!({ "content": blocked_content, "visibility": "PUBLIC" }))
        .send()
        .await?;
    let accepted_status = accepted.status();
    let accepted_headers = accepted.headers().clone();
    let accepted_body: Value = accepted.json().await?;
    anyhow::ensure!(
        accepted_status == StatusCode::ACCEPTED
            && accepted_headers
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store")
            && accepted_headers.contains_key(reqwest::header::RETRY_AFTER),
        "the blocked Post create was not durably accepted: {accepted_status} {accepted_body}"
    );
    let blocked_operation_id: Uuid = accepted_body["operationId"]
        .as_str()
        .context("durable pool-saturation acceptance omitted its operation id")?
        .parse()?;

    let pending: (String, String, i32, bool) = sqlx::query_as(
        r#"
        select journal.state, queue.state, queue.attempt_count,
               queue.payload_ciphertext is not null
        from gateway_request_journal journal
        join gateway_async_operation_queue queue on queue.operation_id = journal.id
        where journal.id = $1 and journal.actor_id = $2
        "#,
    )
    .bind(blocked_operation_id)
    .bind(owner.user_id.to_string())
    .fetch_one(&gateway_pool)
    .await?;
    anyhow::ensure!(
        pending.0 == "QUEUED" && matches!(pending.1.as_str(), "READY" | "LEASED") && pending.3,
        "202 did not leave the first operation durably queued: {pending:?}"
    );

    let blocked_backend_pid = timeout(Duration::from_secs(20), async {
        loop {
            let waiting_pid: Option<i32> = sqlx::query_scalar(
                r#"
                select pid
                from pg_stat_activity
                where pid <> pg_backend_pid()
                  and datname = current_database()
                  and state = 'active'
                  and wait_event_type = 'Lock'
                  and query ilike '%insert into posts (%'
                order by query_start
                limit 1
                "#,
            )
            .fetch_optional(&post_pool)
            .await?;
            if let Some(pid) = waiting_pid {
                break Ok::<i32, anyhow::Error>(pid);
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .context("Post did not hold its sole connection on the blocked create")??;

    let rejected_content = format!("pool-saturation-rejected-{run_id}");
    // Keep the competing request unkeyed: a keyed owner mutation is eligible
    // for the durable queue and may correctly return 202 after the Gateway's
    // caller budget, which would not exercise the synchronous saturation 503.
    let rejected_request_started_at = chrono::Utc::now();
    let request_started = Instant::now();
    let rejected = client
        .post(format!("{base}/api/posts"))
        .bearer_auth(&owner.access_token)
        .json(&json!({ "content": rejected_content, "visibility": "PUBLIC" }))
        .send()
        .await?;
    let rejected_status = rejected.status();
    let rejected_body: Value = rejected.json().await?;
    let rejected_elapsed = request_started.elapsed();
    anyhow::ensure!(
        rejected_status == StatusCode::SERVICE_UNAVAILABLE
            && rejected_body["message"] == "Service unavailable"
            && rejected_elapsed >= Duration::from_millis(100)
            && rejected_elapsed < Duration::from_secs(2)
            && !rejected_body
                .to_string()
                .to_ascii_lowercase()
                .contains("pooltimedout"),
        "saturated Post pool did not fail promptly with a sanitized 503: status={rejected_status}, elapsed={rejected_elapsed:?}, body={rejected_body}"
    );
    restart.processes.ensure_running("post-service")?;

    let blocked_effects: (i64, i64, i64, i64) = sqlx::query_as(
        r#"
        select
            (select count(*) from posts
             where author_id = $1 and content in ($2, $3)),
            (select count(*) from post_event_outbox outbox
             join posts saved_post on saved_post.id = outbox.aggregate_id
             where saved_post.author_id = $1 and saved_post.content in ($2, $3)),
            (select count(*) from post_mutation_receipts where operation_id = $4),
            (select count(*) from post_mutation_receipts
             where actor_id = $1 and idempotency_key is null
               and committed_at >= $5)
        "#,
    )
    .bind(owner.user_id)
    .bind(&blocked_content)
    .bind(&rejected_content)
    .bind(blocked_operation_id)
    .bind(rejected_request_started_at)
    .fetch_one(&post_pool)
    .await?;
    anyhow::ensure!(
        blocked_effects == (0, 0, 0, 0),
        "pool saturation wrote a Post, outbox event, or mutation receipt before recovery: {blocked_effects:?}"
    );

    post_write_barrier.rollback().await?;

    let recovery_deadline = Instant::now() + Duration::from_secs(45);
    let recovered = loop {
        let state: (String, String, i32, bool, Option<i32>) = sqlx::query_as(
            r#"
            select queue.state, journal.state, queue.attempt_count,
                   queue.payload_ciphertext is null, journal.response_status
            from gateway_async_operation_queue queue
            join gateway_request_journal journal on journal.id = queue.operation_id
            where queue.operation_id = $1
            "#,
        )
        .bind(blocked_operation_id)
        .fetch_one(&gateway_pool)
        .await?;
        if state.0 == "DONE" && state.1 == "COMPLETED" {
            break state;
        }
        anyhow::ensure!(
            state.0 != "DEAD",
            "the queued Post operation dead-lettered after database recovery: {state:?}"
        );
        if Instant::now() >= recovery_deadline {
            anyhow::bail!("the queued Post operation did not finish after lock release: {state:?}");
        }
        sleep(Duration::from_millis(100)).await;
    };
    anyhow::ensure!(
        recovered.2 > pending.2 && recovered.3 && recovered.4 == Some(200),
        "Post did not durably complete the queued create after pool recovery: {recovered:?}"
    );

    let blocked_post_id: Uuid =
        sqlx::query_scalar("select id from posts where author_id = $1 and content = $2")
            .bind(owner.user_id)
            .bind(&blocked_content)
            .fetch_one(&post_pool)
            .await?;
    let recovered_effects: (i64, i64) = sqlx::query_as(
        "select (select count(*) from post_event_outbox where aggregate_id = $1), (select count(*) from post_mutation_receipts where operation_id = $2)",
    )
    .bind(blocked_post_id)
    .bind(blocked_operation_id)
    .fetch_one(&post_pool)
    .await?;
    anyhow::ensure!(
        recovered_effects == (1, 1),
        "the accepted operation did not recover exactly one outbox event and receipt: {recovered_effects:?}"
    );

    let recovered_replay = client
        .post(format!("{base}/api/posts"))
        .bearer_auth(&owner.access_token)
        .header("Idempotency-Key", &blocked_key)
        .json(&json!({ "content": blocked_content, "visibility": "PUBLIC" }))
        .send()
        .await?;
    let recovered_replay_status = recovered_replay.status();
    let recovered_replay_body: Value = recovered_replay.json().await?;
    anyhow::ensure!(
        recovered_replay_status.is_success()
            && recovered_replay_body["id"] == blocked_post_id.to_string(),
        "retrying the accepted operation did not replay its recovered result: {recovered_replay_status} {recovered_replay_body}"
    );

    let fresh_content = format!("pool-saturation-fresh-{run_id}");
    let fresh_key = format!("pool-saturation-fresh-{run_id}");
    let fresh = client
        .post(format!("{base}/api/posts"))
        .bearer_auth(&owner.access_token)
        .header("Idempotency-Key", &fresh_key)
        .json(&json!({ "content": fresh_content, "visibility": "PUBLIC" }))
        .send()
        .await?;
    let fresh_status = fresh.status();
    let fresh_body: Value = fresh.json().await?;
    anyhow::ensure!(
        fresh_status.is_success(),
        "a fresh Post create failed after pool capacity returned: {fresh_status} {fresh_body}"
    );
    let fresh_post_id: Uuid = fresh_body["id"]
        .as_str()
        .context("fresh Post response omitted its ID")?
        .parse()?;

    let fresh_replay = client
        .post(format!("{base}/api/posts"))
        .bearer_auth(&owner.access_token)
        .header("Idempotency-Key", &fresh_key)
        .json(&json!({ "content": fresh_content, "visibility": "PUBLIC" }))
        .send()
        .await?;
    let fresh_replay_status = fresh_replay.status();
    let fresh_replay_body: Value = fresh_replay.json().await?;
    anyhow::ensure!(
        fresh_replay_status.is_success() && fresh_replay_body["id"] == fresh_post_id.to_string(),
        "fresh-request replay changed its committed Post result: {fresh_replay_status} {fresh_replay_body}"
    );

    let fresh_effects: (i64, i64, i64) = sqlx::query_as(
        r#"
        select
            (select count(*) from posts where id = $1),
            (select count(*) from post_event_outbox
             where aggregate_id = $1 and event_type = 'POST_CREATED'),
            (select count(*) from post_mutation_receipts
             where actor_id = $2 and idempotency_key = $3)
        "#,
    )
    .bind(fresh_post_id)
    .bind(owner.user_id)
    .bind(&fresh_key)
    .fetch_one(&post_pool)
    .await?;
    anyhow::ensure!(
        fresh_effects == (1, 1, 1),
        "recovered pool did not preserve one fresh post/outbox/receipt across replay: {fresh_effects:?}"
    );

    gateway_pool.close().await;
    post_pool.close().await;
    restart.processes.stop("post-service")?;
    restart.processes.spawn(
        "post-service",
        restart.binary_dir.join("post-service"),
        restart.post_environment,
    )?;
    await_ready(
        client,
        restart.processes,
        "post-service",
        restart.post_metrics_address,
        Duration::from_secs(60),
    )
    .await?;

    eprintln!(
        "separate-process Post pool-saturation acceptance passed: blocked_backend_pid={blocked_backend_pid}, acquire_timeout_ms={ACQUIRE_TIMEOUT_MS}, saturated_status={rejected_status}, recovered_operation={blocked_operation_id}, fresh_post={fresh_post_id}"
    );
    Ok(())
}

async fn recover_friend_database_pool_saturation(
    client: &reqwest::Client,
    base: &str,
    restart: &mut ServiceProcessRestart<'_>,
    run_id: &str,
    follower: &ProcessUser,
    followee: &ProcessUser,
) -> Result<()> {
    const ACQUIRE_TIMEOUT_MS: &str = "150";

    let gateway_pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(restart.gateway_database_url)
        .await?;
    let friend_pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(restart.friend_database_url)
        .await?;

    let starting_edges: (i64, i64) = sqlx::query_as(
        r#"
        select
            (select count(*) from follows
             where follower_id = $1 and followee_id = $2 and deleted_at is null),
            (select count(*) from follows
             where follower_id = $2 and followee_id = $1 and deleted_at is null)
        "#,
    )
    .bind(follower.user_id)
    .bind(followee.user_id)
    .fetch_one(&friend_pool)
    .await?;
    anyhow::ensure!(
        starting_edges == (0, 1),
        "Friend pool-saturation fixture must have only the reverse edge active after the earlier unfollow: {starting_edges:?}"
    );

    let mut constrained_environment = restart.friend_environment.clone();
    constrained_environment.insert("FRIEND_DB_POOL_MAX_CONNECTIONS".into(), "1".into());
    constrained_environment.insert(
        "FRIEND_DB_POOL_ACQUIRE_TIMEOUT_MS".into(),
        ACQUIRE_TIMEOUT_MS.into(),
    );
    for worker in [
        "FRIEND_RECOMMENDATION_WORKER_ENABLED",
        "FRIEND_RECOMMENDATION_RETENTION_WORKER_ENABLED",
        "FRIEND_MUTATION_RECEIPT_RETENTION_WORKER_ENABLED",
    ] {
        constrained_environment.insert(worker.into(), "false".into());
    }

    restart.processes.stop("friend-service")?;
    restart.processes.spawn(
        "friend-service",
        restart.binary_dir.join("friend-service"),
        &constrained_environment,
    )?;
    await_ready(
        client,
        restart.processes,
        "friend-service",
        restart.friend_metrics_address,
        Duration::from_secs(60),
    )
    .await?;

    // The existing inactive row is reactivated by Follow. SHARE permits the
    // inspection queries below but blocks Friend's actual upsert while it
    // holds its only service-pool connection.
    let mut follow_write_barrier = friend_pool.begin().await?;
    timeout(
        Duration::from_secs(10),
        sqlx::query("lock table follows in share mode").execute(&mut *follow_write_barrier),
    )
    .await
    .context("timed out acquiring the Friend pool-saturation write barrier")??;

    let idempotency_key = format!("friend-pool-saturation-{run_id}");
    let follow_request = json!({ "targetUserId": followee.user_id.to_string() });
    let accepted = client
        .post(format!("{base}/api/follows"))
        .bearer_auth(&follower.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .json(&follow_request)
        .send()
        .await?;
    let accepted_status = accepted.status();
    let accepted_headers = accepted.headers().clone();
    let accepted_body: Value = accepted.json().await?;
    anyhow::ensure!(
        accepted_status == StatusCode::ACCEPTED
            && accepted_headers
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store")
            && accepted_headers.contains_key(reqwest::header::RETRY_AFTER)
            && accepted_headers.contains_key(reqwest::header::LOCATION),
        "the saturated keyed Follow was not durably accepted: {accepted_status} {accepted_body}"
    );
    let operation_id: Uuid = accepted_body["operationId"]
        .as_str()
        .context("durable Friend saturation acceptance omitted its operation id")?
        .parse()?;

    let pending: (String, String, i32, bool) = sqlx::query_as(
        r#"
        select journal.state, queue.state, queue.attempt_count,
               queue.payload_ciphertext is not null
        from gateway_request_journal journal
        join gateway_async_operation_queue queue on queue.operation_id = journal.id
        where journal.id = $1 and journal.actor_id = $2
        "#,
    )
    .bind(operation_id)
    .bind(follower.user_id.to_string())
    .fetch_one(&gateway_pool)
    .await?;
    anyhow::ensure!(
        pending.0 == "QUEUED" && matches!(pending.1.as_str(), "READY" | "LEASED") && pending.3,
        "202 did not leave the Friend mutation encrypted and durably queued: {pending:?}"
    );

    let blocked_backend_pid = timeout(Duration::from_secs(20), async {
        loop {
            let waiting_pid: Option<i32> = sqlx::query_scalar(
                r#"
                select pid
                from pg_stat_activity
                where pid <> pg_backend_pid()
                  and datname = current_database()
                  and state = 'active'
                  and wait_event_type = 'Lock'
                  and query ilike '%insert into follows (%'
                order by query_start
                limit 1
                "#,
            )
            .fetch_optional(&friend_pool)
            .await?;
            if let Some(pid) = waiting_pid {
                break Ok::<i32, anyhow::Error>(pid);
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .context("Friend did not hold its sole connection on the blocked follow upsert")??;

    // This unkeyed opposite-direction Unfollow must fail before obtaining a
    // database connection; unlike the keyed request, it cannot be queued.
    let rejected_started = Instant::now();
    let rejected = client
        .delete(format!("{base}/api/follows/{}", follower.user_id))
        .bearer_auth(&followee.access_token)
        .send()
        .await?;
    let rejected_status = rejected.status();
    let rejected_headers = rejected.headers().clone();
    let rejected_body: Value = rejected.json().await?;
    let rejected_elapsed = rejected_started.elapsed();
    anyhow::ensure!(
        rejected_status == StatusCode::SERVICE_UNAVAILABLE
            && rejected_body["message"] == "Service unavailable"
            && rejected_elapsed >= Duration::from_millis(100)
            && rejected_elapsed < Duration::from_secs(2)
            && !rejected_body
                .to_string()
                .to_ascii_lowercase()
                .contains("pooltimedout"),
        "saturated Friend pool did not fail promptly with a sanitized 503: status={rejected_status}, elapsed={rejected_elapsed:?}, body={rejected_body}"
    );
    anyhow::ensure!(
        rejected_headers.contains_key("x-correlation-id"),
        "saturated Friend rejection omitted its correlation identifier"
    );

    let before_recovery: (i64, i64, i64) = sqlx::query_as(
        r#"
        select
            (select count(*) from follows
             where follower_id = $1 and followee_id = $2 and deleted_at is null),
            (select count(*) from follows
             where follower_id = $2 and followee_id = $1 and deleted_at is null),
            (select count(*) from friend_mutation_receipts
             where operation_id = $3 and actor_id = $1
               and scope = 'friend.v1.FollowUser' and outcome_json is not null)
        "#,
    )
    .bind(follower.user_id)
    .bind(followee.user_id)
    .bind(operation_id)
    .fetch_one(&friend_pool)
    .await?;
    anyhow::ensure!(
        before_recovery == (0, 1, 0),
        "pool saturation committed a Friend edge or owner receipt before recovery: {before_recovery:?}"
    );

    follow_write_barrier.rollback().await?;

    let recovery_deadline = Instant::now() + Duration::from_secs(45);
    let recovered = loop {
        let state: (String, String, i32, bool, Option<i32>) = sqlx::query_as(
            r#"
            select queue.state, journal.state, queue.attempt_count,
                   queue.payload_ciphertext is null, journal.response_status
            from gateway_async_operation_queue queue
            join gateway_request_journal journal on journal.id = queue.operation_id
            where queue.operation_id = $1
            "#,
        )
        .bind(operation_id)
        .fetch_one(&gateway_pool)
        .await?;
        if state.0 == "DONE" && state.1 == "COMPLETED" {
            break state;
        }
        anyhow::ensure!(
            state.0 != "DEAD",
            "the queued Friend Follow dead-lettered after pool recovery: {state:?}"
        );
        if Instant::now() >= recovery_deadline {
            anyhow::bail!("the queued Friend Follow did not finish after pool recovery: {state:?}");
        }
        sleep(Duration::from_millis(100)).await;
    };
    anyhow::ensure!(
        recovered.2 > pending.2 && recovered.3 && recovered.4 == Some(200),
        "Friend did not complete the queued Follow after pool recovery: {recovered:?}"
    );

    let replay = client
        .post(format!("{base}/api/follows"))
        .bearer_auth(&follower.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .json(&follow_request)
        .send()
        .await?;
    let replay_status = replay.status();
    let replayed = replay
        .headers()
        .get("idempotency-replayed")
        .is_some_and(|value| value == "true");
    let replay_body: Value = replay.json().await?;
    anyhow::ensure!(
        replay_status == StatusCode::OK
            && replayed
            && replay_body["following"] == true
            && replay_body["status"] == "ACTIVE",
        "same-key Friend Follow did not replay the committed result: {replay_status} {replay_body}"
    );

    let effects: (i64, i64, i64, i64) = sqlx::query_as(
        r#"
        select
            (select count(*) from follows
             where follower_id = $1 and followee_id = $2 and deleted_at is null),
            (select count(*) from follows
             where follower_id = $2 and followee_id = $1 and deleted_at is null),
            (select count(*) from friend_mutation_receipts
             where operation_id = $3 and actor_id = $1
               and scope = 'friend.v1.FollowUser' and outcome_json is not null),
            (select count(*) from follows
             where follower_id = $2 and followee_id = $1 and deleted_at is not null
               and deleted_by = $2 and delete_reason = 'unfollow')
        "#,
    )
    .bind(follower.user_id)
    .bind(followee.user_id)
    .bind(operation_id)
    .fetch_one(&friend_pool)
    .await?;
    anyhow::ensure!(
        effects == (1, 1, 1, 0),
        "Friend pool recovery/replay duplicated an edge or allowed the rejected Unfollow: {effects:?}"
    );

    let status_path = format!("/api/operations/{operation_id}");
    let owner_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&follower.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        owner_status.status() == StatusCode::OK
            && owner_status
                .headers()
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store"),
        "Friend Follow owner could not read its private operation status"
    );
    let owner_status_body: Value = owner_status.json().await?;
    anyhow::ensure!(
        owner_status_body["state"] == "COMPLETED"
            && owner_status_body["responseStatus"] == 200
            && owner_status_body["replayable"] == true,
        "Friend Follow owner status omitted its terminal result: {owner_status_body}"
    );
    let other_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&followee.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        other_status.status() == StatusCode::NOT_FOUND,
        "another caller learned a Friend Follow operation status they do not own: {}",
        other_status.status()
    );

    gateway_pool.close().await;
    friend_pool.close().await;
    restart.processes.stop("friend-service")?;
    restart.processes.spawn(
        "friend-service",
        restart.binary_dir.join("friend-service"),
        restart.friend_environment,
    )?;
    await_ready(
        client,
        restart.processes,
        "friend-service",
        restart.friend_metrics_address,
        Duration::from_secs(60),
    )
    .await?;

    eprintln!(
        "separate-process Friend pool-saturation acceptance passed: blocked_backend_pid={blocked_backend_pid}, acquire_timeout_ms={ACQUIRE_TIMEOUT_MS}, saturated_status={rejected_status}, recovered_operation={operation_id}, edges={effects:?}"
    );
    Ok(())
}

async fn recover_unfollow_after_friend_service_delay(
    client: &reqwest::Client,
    base: &str,
    restart: &mut ServiceProcessRestart<'_>,
    run_id: &str,
    follower: &ProcessUser,
    followee: &ProcessUser,
) -> Result<()> {
    let gateway_pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(restart.gateway_database_url)
        .await?;
    let friend_pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(restart.friend_database_url)
        .await?;

    let active_before: i64 = sqlx::query_scalar(
        "select count(*) from follows where follower_id = $1 and followee_id = $2 and deleted_at is null",
    )
    .bind(follower.user_id)
    .bind(followee.user_id)
    .fetch_one(&friend_pool)
    .await?;
    anyhow::ensure!(
        active_before == 1,
        "Friend-delay Unfollow fixture must begin with one active edge: {active_before}"
    );

    // Keep the real listener and established HTTP/2 channel present, but stop
    // Friend from processing requests long enough to exceed Gateway's 300 ms
    // caller budget and the 800 ms Gateway→Friend RPC deadline.
    restart.processes.suspend("friend-service")?;

    let path = format!("/api/follows/{}", followee.user_id);
    let idempotency_key = format!("friend-delay-unfollow-{run_id}");
    let started = Instant::now();
    let accepted = client
        .delete(format!("{base}{path}"))
        .bearer_auth(&follower.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .send()
        .await?;
    let accepted_elapsed = started.elapsed();
    let accepted_status = accepted.status();
    let accepted_headers = accepted.headers().clone();
    let accepted_body: Value = accepted.json().await?;
    anyhow::ensure!(
        accepted_status == StatusCode::ACCEPTED
            && accepted_elapsed >= Duration::from_millis(200)
            && accepted_elapsed < Duration::from_secs(2)
            && accepted_headers
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store")
            && accepted_headers.contains_key(reqwest::header::RETRY_AFTER)
            && accepted_headers.contains_key(reqwest::header::LOCATION)
            && accepted_headers.contains_key("x-correlation-id"),
        "delayed Friend Unfollow was not accepted within the bounded durable-queue contract: status={accepted_status}, elapsed={accepted_elapsed:?}, headers={accepted_headers:?}, body={accepted_body}"
    );
    let operation_id: Uuid = accepted_body["operationId"]
        .as_str()
        .context("durable Friend-delay acceptance omitted its operation id")?
        .parse()?;

    let queued: (String, String, i32, bool, Option<String>) = sqlx::query_as(
        r#"
        select journal.state, queue.state, queue.attempt_count,
               queue.payload_ciphertext is not null, queue.last_error_code
        from gateway_request_journal journal
        join gateway_async_operation_queue queue on queue.operation_id = journal.id
        where journal.id = $1 and journal.actor_id = $2
        "#,
    )
    .bind(operation_id)
    .bind(follower.user_id.to_string())
    .fetch_one(&gateway_pool)
    .await?;
    anyhow::ensure!(
        queued.0 == "QUEUED" && matches!(queued.1.as_str(), "READY" | "LEASED") && queued.3,
        "202 did not preserve the Unfollow as an encrypted durable operation: {queued:?}"
    );

    // Wait for the worker to observe the real Friend RPC deadline and schedule
    // a retry. A mere 202 is insufficient evidence that the dependency boundary
    // was exercised.
    let outage_deadline = Instant::now() + Duration::from_secs(15);
    let failed_attempt = loop {
        let state: (String, String, i32, bool, Option<String>) = sqlx::query_as(
            r#"
            select queue.state, journal.state, queue.attempt_count,
                   queue.payload_ciphertext is not null, queue.last_error_code
            from gateway_async_operation_queue queue
            join gateway_request_journal journal on journal.id = queue.operation_id
            where queue.operation_id = $1
            "#,
        )
        .bind(operation_id)
        .fetch_one(&gateway_pool)
        .await?;
        if state.0 == "READY"
            && state.1 == "QUEUED"
            && state.2 > 0
            && state.3
            && state.4.as_deref() == Some("OWNER_OR_GATEWAY_5XX")
        {
            break state;
        }
        anyhow::ensure!(
            state.0 != "DEAD",
            "the delayed Friend Unfollow dead-lettered before service recovery: {state:?}"
        );
        if Instant::now() >= outage_deadline {
            anyhow::bail!(
                "Gateway did not persist a retry after the Friend RPC deadline: {state:?}"
            );
        }
        sleep(Duration::from_millis(50)).await;
    };

    let before_recovery: (i64, i64, i64) = sqlx::query_as(
        r#"
        select
            count(*) filter (where deleted_at is null),
            count(*) filter (where deleted_at is not null),
            (select count(*) from friend_mutation_receipts
             where operation_id = $3 and actor_id = $1
               and scope = 'friend.v1.UnfollowUser' and outcome_json is not null)
        from follows
        where follower_id = $1 and followee_id = $2
        "#,
    )
    .bind(follower.user_id)
    .bind(followee.user_id)
    .bind(operation_id)
    .fetch_one(&friend_pool)
    .await?;
    anyhow::ensure!(
        before_recovery == (1, 0, 0),
        "the delayed or cancelled RPC changed Friend state before recovery: {before_recovery:?}"
    );

    restart.processes.resume("friend-service")?;
    await_ready(
        client,
        restart.processes,
        "friend-service",
        restart.friend_metrics_address,
        Duration::from_secs(30),
    )
    .await?;

    let recovery_deadline = Instant::now() + Duration::from_secs(45);
    let recovered = loop {
        let state: (String, String, i32, bool, Option<i32>) = sqlx::query_as(
            r#"
            select queue.state, journal.state, queue.attempt_count,
                   queue.payload_ciphertext is null, journal.response_status
            from gateway_async_operation_queue queue
            join gateway_request_journal journal on journal.id = queue.operation_id
            where queue.operation_id = $1
            "#,
        )
        .bind(operation_id)
        .fetch_one(&gateway_pool)
        .await?;
        if state.0 == "DONE" && state.1 == "COMPLETED" {
            break state;
        }
        anyhow::ensure!(
            state.0 != "DEAD",
            "Friend recovery dead-lettered the accepted Unfollow: {state:?}"
        );
        if Instant::now() >= recovery_deadline {
            anyhow::bail!("Gateway did not complete Friend Unfollow after recovery: {state:?}");
        }
        sleep(Duration::from_millis(100)).await;
    };
    anyhow::ensure!(
        recovered.2 > failed_attempt.2 && recovered.3 && recovered.4 == Some(200),
        "the retried Friend Unfollow did not complete from the durable command: {recovered:?}"
    );

    let replay = client
        .delete(format!("{base}{path}"))
        .bearer_auth(&follower.access_token)
        .header("Idempotency-Key", &idempotency_key)
        .send()
        .await?;
    let replay_status = replay.status();
    let replayed = replay
        .headers()
        .get("idempotency-replayed")
        .is_some_and(|value| value == "true");
    let replay_body: Value = replay.json().await?;
    anyhow::ensure!(
        replay_status == StatusCode::OK
            && replayed
            && replay_body["following"] == false
            && replay_body["status"] == "INACTIVE",
        "same-key Unfollow did not replay the recovered result: {replay_status} {replay_body}"
    );

    let effects: (i64, i64, i64, i64) = sqlx::query_as(
        r#"
        select
            count(*) filter (where deleted_at is null),
            count(*) filter (where deleted_at is not null),
            count(*) filter (where deleted_by = $1 and delete_reason = 'unfollow'),
            (select count(*) from friend_mutation_receipts
             where operation_id = $3 and actor_id = $1
               and scope = 'friend.v1.UnfollowUser' and outcome_json is not null)
        from follows
        where follower_id = $1 and followee_id = $2
        "#,
    )
    .bind(follower.user_id)
    .bind(followee.user_id)
    .bind(operation_id)
    .fetch_one(&friend_pool)
    .await?;
    anyhow::ensure!(
        effects == (0, 1, 1, 1),
        "Friend delay recovery/replay did not commit exactly one owner Unfollow: {effects:?}"
    );

    let status_path = format!("/api/operations/{operation_id}");
    let owner_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&follower.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        owner_status.status() == StatusCode::OK
            && owner_status
                .headers()
                .get(reqwest::header::CACHE_CONTROL)
                .is_some_and(|value| value == "no-store"),
        "Unfollow owner could not read its no-store recovered operation status"
    );
    let status_body: Value = owner_status.json().await?;
    anyhow::ensure!(
        status_body["state"] == "COMPLETED"
            && status_body["responseStatus"] == 200
            && status_body["replayable"] == true,
        "recovered Unfollow status omitted its terminal result: {status_body}"
    );
    let other_status = client
        .get(format!("{base}{status_path}"))
        .bearer_auth(&followee.access_token)
        .send()
        .await?;
    anyhow::ensure!(
        other_status.status() == StatusCode::NOT_FOUND,
        "another caller learned a delayed Friend operation status: {}",
        other_status.status()
    );

    gateway_pool.close().await;
    friend_pool.close().await;
    eprintln!(
        "Friend RPC-delay recovery acceptance passed: operation_id={operation_id}, attempts_before_recovery={}, effects={effects:?}, accepted_elapsed={accepted_elapsed:?}",
        failed_attempt.2
    );
    Ok(())
}

async fn wait_for_kafka_event_records(
    partition: &rskafka::client::partition::PartitionClient,
    next_offset: &mut i64,
    event_id: Uuid,
    required_count: usize,
    wait: Duration,
) -> Result<Vec<EventEnvelope>> {
    timeout(wait, async {
        let mut matching = Vec::new();
        loop {
            let (records, _) = partition
                .fetch_records(*next_offset, 1..1_048_576, 250)
                .await?;
            for record in records {
                *next_offset = record.offset + 1;
                let Some(value) = record.record.value.as_deref() else {
                    continue;
                };
                let envelope = EventEnvelope::decode(value)?;
                if envelope.event_id == event_id {
                    matching.push(envelope);
                }
            }
            if matching.len() >= required_count {
                return Ok::<_, anyhow::Error>(matching);
            }
        }
    })
    .await
    .context("timed out waiting for the requested Kafka event record")?
}

fn parse_gateway_edge_deadline_count(exposition: &str, edge: &str) -> Result<u64> {
    let sample_prefix = format!("grpc_edge_deadline_exhaustions_total{{edge=\"{edge}\"}} ");
    for line in exposition.lines() {
        if let Some(value) = line.strip_prefix(&sample_prefix) {
            return Ok(value.parse()?);
        }
    }
    Ok(0)
}

#[test]
fn gateway_edge_deadline_metric_parser_is_label_specific() {
    let exposition = concat!(
        "grpc_edge_deadline_exhaustions_total{edge=\"gateway->post\"} 2\n",
        "grpc_edge_deadline_exhaustions_total{edge=\"gateway->profile\"} 3\n"
    );

    assert_eq!(
        parse_gateway_edge_deadline_count(exposition, "gateway->profile").expect("valid sample"),
        3
    );
    assert_eq!(
        parse_gateway_edge_deadline_count(exposition, "gateway->auth").expect("missing means zero"),
        0
    );
}

async fn gateway_edge_deadline_count(
    client: &reqwest::Client,
    metrics_address: SocketAddr,
    edge: &str,
) -> Result<u64> {
    let response = client
        .get(format!("http://{metrics_address}/metrics"))
        .send()
        .await?;
    anyhow::ensure!(
        response.status().is_success(),
        "Gateway metrics endpoint returned {}",
        response.status()
    );
    let exposition = response.text().await?;
    parse_gateway_edge_deadline_count(&exposition, edge)
}

async fn verify_gateway_profile_deadline_when_profile_is_suspended(
    client: &reqwest::Client,
    base: &str,
    restart: &mut ServiceProcessRestart<'_>,
    owner: &ProcessUser,
    expected_username: &str,
) -> Result<()> {
    // Suspend the real Profile process after the Gateway has already completed
    // profile writes, so its authenticated gRPC channel is warm and the test
    // measures a delayed reply rather than initial connection establishment.
    let deadlines_before =
        gateway_edge_deadline_count(client, restart.metrics_address, "gateway->profile").await?;
    restart.processes.suspend("profile-service")?;

    let started = Instant::now();
    let response = client
        .get(format!("{base}/api/profile/{}", owner.user_id))
        .bearer_auth(&owner.access_token)
        .send()
        .await?;
    let elapsed = started.elapsed();
    let status = response.status();
    let headers = response.headers().clone();
    let body: Value = response.json().await?;
    anyhow::ensure!(
        status == StatusCode::GATEWAY_TIMEOUT
            && elapsed >= Duration::from_millis(600)
            && elapsed < Duration::from_secs(2)
            && headers.contains_key("x-correlation-id")
            && body["code"] == "GATEWAY_504"
            && body["message"] == "Downstream request timed out"
            && body["details"].is_null(),
        "suspended Profile did not produce a bounded, sanitized Gateway 504: status={status}, elapsed={elapsed:?}, body={body}"
    );
    let deadlines_after =
        gateway_edge_deadline_count(client, restart.metrics_address, "gateway->profile").await?;
    anyhow::ensure!(
        deadlines_after > deadlines_before,
        "Gateway->Profile timeout did not increment its edge-deadline metric: before={deadlines_before}, after={deadlines_after}"
    );

    restart.processes.resume("profile-service")?;
    await_ready(
        client,
        restart.processes,
        "profile-service",
        restart.profile_metrics_address,
        Duration::from_secs(30),
    )
    .await?;

    let (recovered_status, profile) = process_json_request(
        client,
        Method::GET,
        base,
        &format!("/api/profile/{}", owner.user_id),
        Some(&owner.access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        recovered_status == StatusCode::OK && profile["username"] == expected_username,
        "Gateway→Profile read did not recover after the process resumed: {recovered_status} {profile}"
    );

    eprintln!(
        "Gateway→Profile delayed-process acceptance passed: status={status}, elapsed_ms={}, recovered_user={}",
        elapsed.as_millis(),
        owner.user_id
    );
    Ok(())
}

async fn verify_gateway_post_deadline_when_post_is_suspended(
    client: &reqwest::Client,
    base: &str,
    restart: &mut ServiceProcessRestart<'_>,
    access_token: &str,
    post_id: Uuid,
    expected_content: &str,
) -> Result<()> {
    // The successful read immediately before this check warms Gateway→Post,
    // so pausing the real service measures a delayed reply on a live channel.
    let deadlines_before =
        gateway_edge_deadline_count(client, restart.metrics_address, "gateway->post").await?;
    restart.processes.suspend("post-service")?;

    let started = Instant::now();
    let response = client
        .get(format!("{base}/api/posts/{post_id}"))
        .bearer_auth(access_token)
        .send()
        .await?;
    let elapsed = started.elapsed();
    let status = response.status();
    let headers = response.headers().clone();
    let body: Value = response.json().await?;
    anyhow::ensure!(
        status == StatusCode::GATEWAY_TIMEOUT
            && elapsed >= Duration::from_millis(600)
            && elapsed < Duration::from_secs(2)
            && headers.contains_key("x-correlation-id")
            && body["code"] == "GATEWAY_504"
            && body["message"] == "Downstream request timed out"
            && body["details"].is_null(),
        "suspended Post did not produce a bounded, sanitized Gateway 504: status={status}, elapsed={elapsed:?}, body={body}"
    );
    let deadlines_after =
        gateway_edge_deadline_count(client, restart.metrics_address, "gateway->post").await?;
    anyhow::ensure!(
        deadlines_after > deadlines_before,
        "Gateway->Post timeout did not increment its edge-deadline metric: before={deadlines_before}, after={deadlines_after}"
    );

    restart.processes.resume("post-service")?;
    await_ready(
        client,
        restart.processes,
        "post-service",
        restart.post_metrics_address,
        Duration::from_secs(30),
    )
    .await?;

    let (recovered_status, recovered_post) = process_json_request(
        client,
        Method::GET,
        base,
        &format!("/api/posts/{post_id}"),
        Some(access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        recovered_status == StatusCode::OK
            && recovered_post["post"]["content"] == expected_content,
        "Gateway→Post read did not recover after the process resumed: {recovered_status} {recovered_post}"
    );

    eprintln!(
        "Gateway→Post delayed-process acceptance passed: status={status}, elapsed_ms={}, post_id={post_id}",
        elapsed.as_millis(),
    );
    Ok(())
}

async fn verify_gateway_notification_deadline_when_notification_is_suspended(
    client: &reqwest::Client,
    base: &str,
    restart: &mut ServiceProcessRestart<'_>,
    access_token: &str,
    expected_notification_id: &str,
) -> Result<()> {
    // The caller fetched this persisted notification immediately before the
    // check, so Gateway→Notification is already connected and authenticated.
    let deadlines_before =
        gateway_edge_deadline_count(client, restart.metrics_address, "gateway->notification")
            .await?;
    restart.processes.suspend("notification-service")?;

    let started = Instant::now();
    let response = client
        .get(format!("{base}/api/notifications?limit=50"))
        .bearer_auth(access_token)
        .send()
        .await?;
    let elapsed = started.elapsed();
    let status = response.status();
    let headers = response.headers().clone();
    let body: Value = response.json().await?;
    anyhow::ensure!(
        status == StatusCode::GATEWAY_TIMEOUT
            && elapsed >= Duration::from_millis(600)
            && elapsed < Duration::from_secs(2)
            && headers.contains_key("x-correlation-id")
            && body["code"] == "GATEWAY_504"
            && body["message"] == "Downstream request timed out"
            && body["details"].is_null(),
        "suspended Notification did not produce a bounded, sanitized Gateway 504: status={status}, elapsed={elapsed:?}, body={body}"
    );
    let deadlines_after =
        gateway_edge_deadline_count(client, restart.metrics_address, "gateway->notification")
            .await?;
    anyhow::ensure!(
        deadlines_after > deadlines_before,
        "Gateway->Notification timeout did not increment its edge-deadline metric: before={deadlines_before}, after={deadlines_after}"
    );

    restart.processes.resume("notification-service")?;
    await_ready(
        client,
        restart.processes,
        "notification-service",
        restart.notification_metrics_address,
        Duration::from_secs(30),
    )
    .await?;

    let (recovered_status, notifications) = process_json_request(
        client,
        Method::GET,
        base,
        "/api/notifications?limit=50",
        Some(access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        recovered_status == StatusCode::OK
            && notifications["items"].as_array().is_some_and(|items| items.iter().any(
                |item| item["id"] == expected_notification_id && item["type"] == "POST_CREATED"
            )),
        "Gateway→Notification read did not recover with the same persisted item: {recovered_status} {notifications}"
    );

    eprintln!(
        "Gateway→Notification delayed-process acceptance passed: status={status}, elapsed_ms={}, notification_id={expected_notification_id}",
        elapsed.as_millis(),
    );
    Ok(())
}

async fn verify_gateway_auth_deadline_when_auth_is_suspended(
    client: &reqwest::Client,
    base: &str,
    restart: &mut ServiceProcessRestart<'_>,
    existing_email: &str,
) -> Result<()> {
    // Signup/login above already warmed the Gateway→Auth channel. Reuse the
    // read-only email-check RPC here so the timeout and recovery add no session.
    let (warm_status, warm_body) = process_json_request(
        client,
        Method::POST,
        base,
        "/auth/email/check",
        None,
        None,
        Some(json!({"email": existing_email})),
    )
    .await?;
    anyhow::ensure!(
        warm_status == StatusCode::OK
            && warm_body["status"] == "TAKEN"
            && warm_body["available"] == false,
        "Gateway→Auth channel warm-up returned an unexpected email check: {warm_status} {warm_body}"
    );

    let deadlines_before =
        gateway_edge_deadline_count(client, restart.metrics_address, "gateway->auth").await?;
    restart.processes.suspend("auth-service")?;
    let started = Instant::now();
    let response = client
        .post(format!("{base}/auth/email/check"))
        .json(&json!({"email": existing_email}))
        .send()
        .await?;
    let elapsed = started.elapsed();
    let status = response.status();
    let headers = response.headers().clone();
    let body: Value = response.json().await?;
    anyhow::ensure!(
        status == StatusCode::GATEWAY_TIMEOUT
            && elapsed >= Duration::from_millis(700)
            && elapsed < Duration::from_secs(2)
            && headers.contains_key("x-correlation-id")
            && body["code"] == "GATEWAY_504"
            && body["message"] == "Downstream request timed out"
            && body["details"].is_null(),
        "suspended Auth did not produce a bounded, sanitized Gateway 504: status={status}, elapsed={elapsed:?}, body={body}"
    );
    let deadlines_after =
        gateway_edge_deadline_count(client, restart.metrics_address, "gateway->auth").await?;
    anyhow::ensure!(
        deadlines_after > deadlines_before,
        "Gateway->Auth timeout did not increment its edge-deadline metric: before={deadlines_before}, after={deadlines_after}"
    );

    restart.processes.resume("auth-service")?;
    await_ready(
        client,
        restart.processes,
        "auth-service",
        restart.auth_metrics_address,
        Duration::from_secs(30),
    )
    .await?;

    let (recovered_status, recovered_body) = process_json_request(
        client,
        Method::POST,
        base,
        "/auth/email/check",
        None,
        None,
        Some(json!({"email": existing_email})),
    )
    .await?;
    anyhow::ensure!(
        recovered_status == StatusCode::OK
            && recovered_body["status"] == "TAKEN"
            && recovered_body["available"] == false,
        "Gateway→Auth read did not recover after the process resumed: {recovered_status} {recovered_body}"
    );

    eprintln!(
        "Gateway→Auth delayed-process acceptance passed: status={status}, elapsed_ms={}, existing_email_availability={}",
        elapsed.as_millis(),
        recovered_body["status"]
    );
    Ok(())
}

async fn verify_gateway_friend_deadline_when_friend_is_suspended(
    client: &reqwest::Client,
    base: &str,
    restart: &mut ServiceProcessRestart<'_>,
    access_token: &str,
    expected_followee_id: Uuid,
) -> Result<()> {
    // The relationship was just created through this process stack; read it
    // once first so the measured request uses a warm Gateway→Friend channel.
    let (warm_status, warm_following) = process_json_request(
        client,
        Method::GET,
        base,
        "/api/follows/following",
        Some(access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        warm_status == StatusCode::OK
            && warm_following["edges"].as_array().is_some_and(|edges| edges
                .iter()
                .any(|edge| edge["userId"] == expected_followee_id.to_string())),
        "Gateway→Friend warm-up did not return the persisted follow edge: {warm_status} {warm_following}"
    );

    let deadlines_before =
        gateway_edge_deadline_count(client, restart.metrics_address, "gateway->friend").await?;
    restart.processes.suspend("friend-service")?;
    let started = Instant::now();
    let response = client
        .get(format!("{base}/api/follows/following"))
        .bearer_auth(access_token)
        .send()
        .await?;
    let elapsed = started.elapsed();
    let status = response.status();
    let headers = response.headers().clone();
    let body: Value = response.json().await?;
    anyhow::ensure!(
        status == StatusCode::GATEWAY_TIMEOUT
            && elapsed >= Duration::from_millis(600)
            && elapsed < Duration::from_secs(2)
            && headers.contains_key("x-correlation-id")
            && body["code"] == "GATEWAY_504"
            && body["message"] == "Downstream request timed out"
            && body["details"].is_null(),
        "suspended Friend did not produce a bounded, sanitized Gateway 504: status={status}, elapsed={elapsed:?}, body={body}"
    );
    let deadlines_after =
        gateway_edge_deadline_count(client, restart.metrics_address, "gateway->friend").await?;
    anyhow::ensure!(
        deadlines_after > deadlines_before,
        "Gateway->Friend timeout did not increment its edge-deadline metric: before={deadlines_before}, after={deadlines_after}"
    );

    restart.processes.resume("friend-service")?;
    await_ready(
        client,
        restart.processes,
        "friend-service",
        restart.friend_metrics_address,
        Duration::from_secs(30),
    )
    .await?;

    let (recovered_status, recovered_following) = process_json_request(
        client,
        Method::GET,
        base,
        "/api/follows/following",
        Some(access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        recovered_status == StatusCode::OK
            && recovered_following["edges"].as_array().is_some_and(|edges| edges
                .iter()
                .any(|edge| edge["userId"] == expected_followee_id.to_string())),
        "Gateway→Friend read did not recover with the same persisted edge: {recovered_status} {recovered_following}"
    );

    eprintln!(
        "Gateway→Friend delayed-process acceptance passed: status={status}, elapsed_ms={}, recovered_followee={expected_followee_id}",
        elapsed.as_millis(),
    );
    Ok(())
}

async fn verify_gateway_feed_when_redis_is_unavailable(
    client: &reqwest::Client,
    base: &str,
    restart: &mut ServiceProcessRestart<'_>,
    access_token: &str,
    expected_post_id: Uuid,
    expected_content: &str,
) -> Result<()> {
    const FEED_PATH: &str = "/api/feed?limit=50";

    let warm_started = Instant::now();
    let (warm_status, warm_feed) = process_json_request(
        client,
        Method::GET,
        base,
        FEED_PATH,
        Some(access_token),
        None,
        None,
    )
    .await?;
    let warm_elapsed = warm_started.elapsed();
    anyhow::ensure!(
        warm_status == StatusCode::OK
            && feed_response_has_contract_shape(&warm_feed)
            && feed_contains_post(&warm_feed, expected_post_id, expected_content),
        "ranked-feed warm-up omitted the persisted followed post or response metadata: {warm_status} {warm_feed}"
    );
    eprintln!(
        "Gateway→Post ranked-feed warm-up completed: status={warm_status}, elapsed_ms={}",
        warm_elapsed.as_millis()
    );

    // This case tests read fallback. Finish preceding writes while Redis is
    // healthy: interrupting a live CACHE delivery can legitimately retain its
    // five-minute uncertainty lease and contaminate the later Kafka case.
    // Observe real publication; never expire a lease or fabricate a checkpoint.
    let post_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(restart.post_database_url)
        .await?;
    wait_for_post_outbox_drain(&post_pool, restart, "pre-redis-read-outage").await?;
    post_pool.close().await;
    restart.redis.stop_with_timeout(Some(0)).await?;
    let outage_result = timeout(Duration::from_secs(3), async {
        let started = Instant::now();
        let response = client
            .get(format!("{base}{FEED_PATH}"))
            .bearer_auth(access_token)
            .send()
            .await?;
        let elapsed = started.elapsed();
        let status = response.status();
        let headers = response.headers().clone();
        let body: Value = response.json().await?;
        Ok::<_, anyhow::Error>((status, headers, body, elapsed))
    })
    .await;

    // Always restore the test-owned dependency before asserting the response,
    // so a failed expectation does not leave the remaining process journey
    // with Redis stopped.
    restart.redis.start().await?;
    redis::await_redis_ready_with_timeout(restart.redis, Duration::from_secs(30)).await?;

    let (status, headers, feed, elapsed) = outage_result
        .context("Gateway feed request exceeded the Redis-outage acceptance deadline")??;
    anyhow::ensure!(
        status == StatusCode::OK
            && elapsed < Duration::from_secs(2)
            && headers.contains_key("x-correlation-id")
            && feed_response_has_contract_shape(&feed)
            && feed_contains_post(&feed, expected_post_id, expected_content),
        "ranked feed did not serve the same persisted post within the Gateway deadline during Redis outage: status={status}, elapsed={elapsed:?}, body={feed}"
    );
    restart.processes.ensure_running("post-service")?;

    let recovered_started = Instant::now();
    let (recovered_status, recovered_feed) = process_json_request(
        client,
        Method::GET,
        base,
        FEED_PATH,
        Some(access_token),
        None,
        None,
    )
    .await?;
    let recovered_elapsed = recovered_started.elapsed();
    anyhow::ensure!(
        recovered_status == StatusCode::OK
            && recovered_elapsed < Duration::from_secs(2)
            && feed_response_has_contract_shape(&recovered_feed)
            && feed_contains_post(&recovered_feed, expected_post_id, expected_content),
        "ranked feed did not recover after Redis resumed: status={recovered_status}, elapsed={recovered_elapsed:?}, body={recovered_feed}"
    );

    eprintln!(
        "Gateway→Post Redis outage acceptance passed: status={status}, outage_elapsed_ms={}, recovery_elapsed_ms={}",
        elapsed.as_millis(),
        recovered_elapsed.as_millis(),
    );
    Ok(())
}

async fn verify_gateway_feed_block_race(
    client: &reqwest::Client,
    base: &str,
    gate: &ProcessStackRelationshipGate,
    friend_database_url: &str,
    viewer: &ProcessUser,
    author: &ProcessUser,
    expected_post_id: Uuid,
    expected_content: &str,
) -> Result<()> {
    let friend_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(friend_database_url)
        .await
        .context("could not inspect the process-stack Friend block transition")?;
    gate.arm()?;
    let request_client = client.clone();
    let request_base = base.to_owned();
    let request_token = viewer.access_token.clone();
    let mut feed_request = tokio::spawn(async move {
        process_json_request(
            &request_client,
            Method::GET,
            &request_base,
            "/api/feed?limit=50",
            Some(&request_token),
            None,
            None,
        )
        .await
    });

    if let Err(error) = gate.wait_until_entered().await {
        let _ = gate.release();
        feed_request.abort();
        let _ = feed_request.await;
        return Err(error);
    }

    // The feed has already collected ranked candidates and is paused inside the
    // real FriendService policy lookup. Gateway uses its own Friend connection,
    // so this mutation can commit while Post's privacy check is held.
    let block_client = client.clone();
    let block_base = base.to_owned();
    let block_token = viewer.access_token.clone();
    let block_path = format!("/api/relationships/blocks/{}", author.user_id);
    let block_started = Instant::now();
    let block_request = tokio::spawn(async move {
        process_json_request(
            &block_client,
            Method::POST,
            &block_base,
            &block_path,
            Some(&block_token),
            None,
            None,
        )
        .await
    });
    // Release the held Post -> Friend policy lookup as soon as the real Friend
    // transaction commits. Waiting for the full Gateway response first can
    // consume the Gateway -> Post deadline even though the privacy mutation is
    // already durable.
    let block_commit_result = timeout(Duration::from_secs(2), async {
        loop {
            let committed: bool = sqlx::query_scalar(
                "select exists(select 1 from user_blocks where blocker_id = $1 and blocked_id = $2 and deleted_at is null)",
            )
            .bind(viewer.user_id)
            .bind(author.user_id)
            .fetch_one(&friend_pool)
            .await?;
            if committed {
                return Ok::<(), anyhow::Error>(());
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("Gateway block did not commit within the process-stack deadline"))
    .and_then(|result| result);
    if block_commit_result.is_ok() {
        eprintln!(
            "Gateway block row committed while ranked feed was held: elapsed_ms={}",
            block_started.elapsed().as_millis()
        );
    }
    let feed_was_held_until_release = !feed_request.is_finished();
    eprintln!(
        "ranked feed gate state before release: finished={}",
        !feed_was_held_until_release
    );
    let release_result = gate.release();
    let (feed_status, feed) = timeout(Duration::from_secs(5), &mut feed_request)
        .await
        .context("feed did not finish after releasing the FriendService gate")?
        .context("feed request task panicked")??;
    release_result?;

    block_commit_result?;
    let block_result = block_request
        .await
        .context("Gateway block request task panicked")?;
    let (block_status, block_policy) = block_result?;
    anyhow::ensure!(
        block_status == StatusCode::OK
            && block_policy["blockedByViewer"] == true
            && block_policy["following"] == false,
        "live Gateway block did not commit while the feed policy RPC was paused: {block_status} {block_policy}"
    );
    anyhow::ensure!(
        feed_was_held_until_release,
        "ranked feed completed before the held FriendService policy call was released"
    );
    anyhow::ensure!(
        feed_status == StatusCode::OK
            && feed_response_has_contract_shape(&feed)
            && feed["posts"].is_array()
            && feed["failedSources"].as_array().is_some_and(Vec::is_empty)
            && !feed_contains_post(&feed, expected_post_id, expected_content),
        "ranked feed exposed a collected post after the real FriendService block committed: {feed_status} {feed}"
    );

    let (unblock_status, unblock_policy) = process_json_request(
        client,
        Method::DELETE,
        base,
        &format!("/api/relationships/blocks/{}", author.user_id),
        Some(&viewer.access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        unblock_status == StatusCode::OK && unblock_policy["blockedByViewer"] == false,
        "process-stack block fixture could not be restored: {unblock_status} {unblock_policy}"
    );
    for (actor, target) in [(viewer, author), (author, viewer)] {
        let (follow_status, follow_response) = process_json_request(
            client,
            Method::POST,
            base,
            "/api/follows",
            Some(&actor.access_token),
            None,
            Some(json!({"targetUserId": target.user_id.to_string()})),
        )
        .await?;
        anyhow::ensure!(
            follow_status.is_success() && follow_response["following"] == true,
            "could not restore process-stack follow state after block race: {follow_status} {follow_response}"
        );
    }

    eprintln!(
        "Gateway→Post→Friend live block/feed race passed: viewer={}, author={}, blocked_post_hidden=true, feed_failed_sources=0",
        viewer.user_id,
        author.user_id
    );
    Ok(())
}

async fn wait_for_post_outbox_drain(
    post_pool: &sqlx::PgPool,
    restart: &mut ServiceProcessRestart<'_>,
    stage: &str,
) -> Result<()> {
    let drain_deadline = Instant::now() + Duration::from_secs(45);
    loop {
        restart.processes.ensure_running("post-service")?;
        let pending: i64 = sqlx::query_scalar(
            "select count(*) from post_event_outbox where published = false and dead_lettered_at is null",
        )
        .fetch_one(post_pool)
        .await?;
        if pending == 0 {
            break;
        }
        if Instant::now() >= drain_deadline {
            // Capture scheduling facts before the owned database is torn down.
            // Bound the whole observation, including discovery, to three seconds.
            let _ = timeout(Duration::from_secs(3), async {
                let events = sqlx::query_scalar::<_, Uuid>("select event_id from post_event_outbox where not published and dead_lettered_at is null order by created_at,event_id limit 8")
                    .fetch_all(post_pool).await?;
                for event in events {
                    log_post_delivery_recovery(post_pool, event, stage).await;
                }
                Ok::<_, sqlx::Error>(())
            }).await;
            anyhow::bail!("Post outbox did not drain at {stage}: pending={pending}");
        }
        sleep(Duration::from_millis(100)).await;
    }

    eprintln!("POST_OUTBOX_DRAINED stage={stage}");
    Ok(())
}

async fn recover_post_after_kafka_broker_outage(
    client: &reqwest::Client,
    base: &str,
    restart: &mut ServiceProcessRestart<'_>,
    run_id: &str,
    owner: &ProcessUser,
) -> Result<()> {
    let post_pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(restart.post_database_url)
        .await?;

    // Drain earlier lifecycle events first so the broker outage can be tied to
    // one new event and its Kafka offset is an unambiguous recovery boundary.
    wait_for_post_outbox_drain(&post_pool, restart, "pre-kafka-outage").await?;

    // A broker outage makes every Kafka publication outcome uncertain: the
    // client cannot prove that a timed-out produce request was never written.
    // The fenced worker therefore records no terminal marker. It keeps the
    // EXTERNAL lease and its STARTED marker for expiry-based recovery while the
    // three local targets complete independently.
    const EXTERNAL_FAILURE_SAMPLE: &str =
        "post_outbox_worker_target_delivery_failures_total{target=\"EXTERNAL\"}";
    let failures_before = gateway_metric_value(
        client,
        restart.post_metrics_address,
        EXTERNAL_FAILURE_SAMPLE,
    )
    .await?;

    let content = format!("kafka-outage-{run_id}");
    let outage_started = Instant::now();
    restart.kafka.stop_with_timeout(Some(0)).await?;

    // Post creation commits locally before the asynchronous publisher reaches
    // Kafka. Keep the public mutation bounded and preserve its correlation ID
    // while the external broker is unavailable.
    let request_started = Instant::now();
    let response = timeout(
        Duration::from_secs(3),
        client
            .post(format!("{base}/api/posts"))
            .bearer_auth(&owner.access_token)
            .json(&json!({ "content": content, "visibility": "PUBLIC" }))
            .send(),
    )
    .await
    .context("Post creation exceeded the Kafka-outage request deadline")??;
    let request_elapsed = request_started.elapsed();
    let status = response.status();
    let headers = response.headers().clone();
    let created: Value = response.json().await?;
    anyhow::ensure!(
        status == StatusCode::OK
            && request_elapsed < Duration::from_secs(2)
            && headers.contains_key("x-correlation-id"),
        "Post creation did not remain bounded and successful during Kafka outage: status={status}, elapsed={request_elapsed:?}, body={created}"
    );
    let post_id: Uuid = created["id"]
        .as_str()
        .context("Post creation during Kafka outage omitted its ID")?
        .parse()?;
    let event_id: Uuid = sqlx::query_scalar(
        "select event_id from post_event_outbox where aggregate_id = $1 and event_type = 'POST_CREATED'",
    )
    .bind(post_id)
    .fetch_one(&post_pool)
    .await?;

    // Wait until the worker has observed the failed publication (metric) and
    // every local target has completed. The publish timeout is five seconds.
    let failure_state = timeout(Duration::from_secs(45), async {
        loop {
            restart.processes.ensure_running("post-service")?;
            let failures = gateway_metric_value(
                client,
                restart.post_metrics_address,
                EXTERNAL_FAILURE_SAMPLE,
            )
            .await?;
            let state = read_post_kafka_delivery_state(&post_pool, event_id).await?;
            if failures > failures_before
                && state.timeline_publications == 1
                && state.cache_publications == 1
                && state.internal_publications == 1
            {
                break Ok::<PostKafkaDeliveryState, anyhow::Error>(state);
            }
            sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .context("Post worker did not observe the Kafka outage and complete its local targets")??;
    anyhow::ensure!(
        !failure_state.published
            && !failure_state.dead_lettered
            && failure_state.external_job_leased
            && failure_state.external_lease_live
            && failure_state.external_job_attempts == 1
            && failure_state.external_starts == 1
            && failure_state.external_failures == 0
            && failure_state.external_successes == 0
            && failure_state.external_publications == 0
            && failure_state.timeline_successes == 1
            && failure_state.cache_successes == 1
            && failure_state.internal_successes == 1
            && failure_state.timeline_publications == 1
            && failure_state.cache_publications == 1
            && failure_state.internal_publications == 1,
        "uncertain Kafka outcome was not preserved as a live lease isolated from the completed targets: {failure_state:?}"
    );

    // Crash Post while it still owns the uncertain lease, then age only that
    // lease past its expiry in database time. This exercises the same
    // stale-lease reclamation as a real crash without waiting five minutes.
    restart.processes.stop("post-service")?;
    let crashed_state = read_post_kafka_delivery_state(&post_pool, event_id).await?;
    anyhow::ensure!(
        crashed_state == failure_state,
        "Post process termination changed the uncertain Kafka delivery state: before={failure_state:?}, after={crashed_state:?}"
    );
    let expired_lease = sqlx::query(
        "update post_event_delivery_jobs set lease_expires_at = clock_timestamp() - interval '1 second' where event_id = $1 and target = 'EXTERNAL' and state = 'LEASED' and lease_expires_at > clock_timestamp()",
    )
    .bind(event_id)
    .execute(&post_pool)
    .await?
    .rows_affected();
    anyhow::ensure!(
        expired_lease == 1,
        "interrupted EXTERNAL delivery lease was not available to age past its expiry"
    );
    let expired_state = read_post_kafka_delivery_state(&post_pool, event_id).await?;
    anyhow::ensure!(
        expired_state.external_job_leased && !expired_state.external_lease_live,
        "test did not preserve the claim as an expired lease: {expired_state:?}"
    );

    let failure_elapsed = outage_started.elapsed();
    let recovery_started = Instant::now();
    kafka::start_stopped_kafka(restart.kafka).await?;
    kafka::await_kafka_ready(restart.kafka_bootstrap, Duration::from_secs(60)).await?;

    let kafka_client = ClientBuilder::new(vec![restart.kafka_bootstrap.to_owned()])
        .build()
        .await?;
    let partition = kafka_client
        .partition_client(
            restart.kafka_topic.to_owned(),
            stable_partition(post_id, restart.kafka_partition_count),
            UnknownTopicHandling::Retry,
        )
        .await?;
    let mut next_offset = partition.get_offset(OffsetAt::Latest).await?;

    restart.processes.spawn(
        "post-service",
        restart.binary_dir.join("post-service"),
        restart.post_environment,
    )?;
    await_ready(
        client,
        restart.processes,
        "post-service",
        restart.post_metrics_address,
        Duration::from_secs(60),
    )
    .await?;

    let recovery_deadline = Instant::now() + Duration::from_secs(45);
    loop {
        let state = read_post_kafka_delivery_state(&post_pool, event_id).await?;
        if state.published {
            anyhow::ensure!(
                !state.dead_lettered && !state.locked,
                "recovered Post event has invalid terminal outbox state: {state:?}"
            );
            break;
        }
        anyhow::ensure!(
            !state.dead_lettered,
            "restarted Post dead-lettered the Kafka-outage event: {state:?}"
        );
        if Instant::now() >= recovery_deadline {
            anyhow::bail!("restarted Post did not recover the Kafka-outage event: {state:?}");
        }
        sleep(Duration::from_millis(100)).await;
    }

    let broker_records = wait_for_kafka_event_records(
        &partition,
        &mut next_offset,
        event_id,
        1,
        Duration::from_secs(30),
    )
    .await?;
    let recovery_elapsed = recovery_started.elapsed();
    anyhow::ensure!(
        broker_records.len() == 1
            && broker_records[0].event_id == event_id
            && broker_records[0].event_type == "POST_CREATED"
            && broker_records[0].aggregate_id == post_id,
        "Kafka recovery did not publish exactly one matching event: {broker_records:?}"
    );

    let recovered_state = read_post_kafka_delivery_state(&post_pool, event_id).await?;
    anyhow::ensure!(
        recovered_state.published
            && !recovered_state.dead_lettered
            && !recovered_state.locked
            && !recovered_state.external_job_leased
            && recovered_state.external_job_attempts == 2
            && recovered_state.external_starts == 2
            && recovered_state.external_failures == 0
            && recovered_state.external_successes == 1
            && recovered_state.external_publications == 1
            && recovered_state.timeline_successes == 1
            && recovered_state.cache_successes == 1
            && recovered_state.internal_successes == 1
            && recovered_state.timeline_publications == 1
            && recovered_state.cache_publications == 1
            && recovered_state.internal_publications == 1,
        "Kafka recovery repeated or omitted a durable target checkpoint: {recovered_state:?}"
    );

    let (read_status, read_post) = process_json_request(
        client,
        Method::GET,
        base,
        &format!("/api/posts/{post_id}"),
        Some(&owner.access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        read_status == StatusCode::OK && read_post["post"]["content"] == content,
        "Post read did not recover after broker and Post process restart: {read_status} {read_post}"
    );

    // Observe more than one default worker interval. The published event and
    // all per-target checkpoints must remain stable, with no duplicate Kafka
    // record after the broker-acknowledged delivery.
    sleep(Duration::from_millis(2_200)).await;
    restart.processes.ensure_running("post-service")?;
    let stable_state = read_post_kafka_delivery_state(&post_pool, event_id).await?;
    let latest_offset = partition.get_offset(OffsetAt::Latest).await?;
    anyhow::ensure!(
        stable_state == recovered_state && latest_offset == next_offset,
        "recovered Kafka delivery changed on later worker ticks: initial={recovered_state:?}, later={stable_state:?}, next_offset={next_offset}, latest_offset={latest_offset}"
    );

    post_pool.close().await;
    eprintln!(
        "Post Kafka-broker outage acceptance passed: event_id={event_id}, post_id={post_id}, request_ms={}, outage_failure_ms={}, broker_and_post_recovery_ms={}, external_failure_attempts={}, kafka_records={}",
        request_elapsed.as_millis(),
        failure_elapsed.as_millis(),
        recovery_elapsed.as_millis(),
        recovered_state.external_failures,
        broker_records.len()
    );
    Ok(())
}

async fn read_post_kafka_delivery_state(
    pool: &sqlx::PgPool,
    event_id: Uuid,
) -> Result<PostKafkaDeliveryState> {
    let row = sqlx::query(
        r#"
        select published,
               dead_lettered_at is not null as dead_lettered,
               locked_at is not null as locked,
               coalesce((select state = 'LEASED' from post_event_delivery_jobs where event_id = $1 and target = 'EXTERNAL'), false) as external_job_leased,
               coalesce((select state = 'LEASED' and lease_expires_at > clock_timestamp() from post_event_delivery_jobs where event_id = $1 and target = 'EXTERNAL'), false) as external_lease_live,
               coalesce((select attempt_count from post_event_delivery_jobs where event_id = $1 and target = 'EXTERNAL'), 0) as external_job_attempts,
               (select count(*) from post_event_delivery_attempts where event_id = $1 and target = 'EXTERNAL' and phase = 'STARTED') as external_starts,
               (select count(*) from post_event_delivery_attempts where event_id = $1 and target = 'EXTERNAL' and phase = 'FAILED') as external_failures,
               (select count(*) from post_event_delivery_attempts where event_id = $1 and target = 'EXTERNAL' and phase = 'SUCCEEDED') as external_successes,
               (select count(*) from post_event_delivery_attempts where event_id = $1 and target = 'TIMELINE' and phase = 'SUCCEEDED') as timeline_successes,
               (select count(*) from post_event_delivery_attempts where event_id = $1 and target = 'CACHE' and phase = 'SUCCEEDED') as cache_successes,
               (select count(*) from post_event_delivery_attempts where event_id = $1 and target = 'INTERNAL' and phase = 'SUCCEEDED') as internal_successes,
               (select count(*) from event_publications where event_id = $1 and target = 'EXTERNAL') as external_publications,
               (select count(*) from event_publications where event_id = $1 and target = 'TIMELINE') as timeline_publications,
               (select count(*) from event_publications where event_id = $1 and target = 'CACHE') as cache_publications,
               (select count(*) from event_publications where event_id = $1 and target = 'INTERNAL') as internal_publications
        from post_event_outbox
        where event_id = $1
        "#,
    )
    .bind(event_id)
    .fetch_one(pool)
    .await?;
    Ok(PostKafkaDeliveryState {
        published: row.try_get("published")?,
        dead_lettered: row.try_get("dead_lettered")?,
        locked: row.try_get("locked")?,
        external_job_leased: row.try_get("external_job_leased")?,
        external_lease_live: row.try_get("external_lease_live")?,
        external_job_attempts: row.try_get("external_job_attempts")?,
        external_starts: row.try_get("external_starts")?,
        external_failures: row.try_get("external_failures")?,
        external_successes: row.try_get("external_successes")?,
        timeline_successes: row.try_get("timeline_successes")?,
        cache_successes: row.try_get("cache_successes")?,
        internal_successes: row.try_get("internal_successes")?,
        external_publications: row.try_get("external_publications")?,
        timeline_publications: row.try_get("timeline_publications")?,
        cache_publications: row.try_get("cache_publications")?,
        internal_publications: row.try_get("internal_publications")?,
    })
}

fn feed_response_has_contract_shape(feed: &Value) -> bool {
    (feed["nextCursor"].is_null() || feed["nextCursor"].is_string())
        && (feed["snapshotId"].is_null() || feed["snapshotId"].is_string())
        && (feed["rankVersion"].is_null() || feed["rankVersion"].is_string())
        && (feed["requestId"].is_null() || feed["requestId"].is_string())
        && feed["fallbackUsed"].is_boolean()
        && feed["recommendationMetadata"].is_array()
        && feed["failedSources"].is_array()
}

fn feed_contains_post(feed: &Value, post_id: Uuid, content: &str) -> bool {
    feed["posts"].as_array().is_some_and(|posts| {
        posts
            .iter()
            .any(|post| post["id"] == post_id.to_string() && post["content"] == content)
    })
}

async fn process_signup_verify_login(
    client: &reqwest::Client,
    base: &str,
    inbox: &MailInbox,
    email: &str,
) -> Result<ProcessUser> {
    let (signup_status, signup) = process_json_request(
        client,
        Method::POST,
        base,
        "/auth/email/signup",
        None,
        None,
        Some(json!({"email": email, "password": "Valid-passphrase-42"})),
    )
    .await?;
    anyhow::ensure!(
        signup_status == StatusCode::OK && signup["status"] == "PENDING_VERIFICATION",
        "signup did not create a pending account for {email}: {signup_status} {signup}"
    );
    let user_id: Uuid = signup["userId"]
        .as_str()
        .context("signup response omitted userId")?
        .parse()?;

    let verification = timeout(Duration::from_secs(30), async {
        loop {
            if let Some(message) = inbox
                .lock()
                .await
                .iter()
                .rev()
                .find(|message| message["email"] == email)
                .cloned()
            {
                break message;
            }
            sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .with_context(|| format!("Auth process did not deliver verification mail for {email}"))?;
    let code = verification["verification_code"]
        .as_str()
        .context("verification mail omitted verification_code")?;

    let (verify_status, verified) = process_json_request(
        client,
        Method::POST,
        base,
        "/auth/email/verify",
        None,
        None,
        Some(json!({"email": email, "code": code})),
    )
    .await?;
    anyhow::ensure!(
        verify_status == StatusCode::OK && verified["status"] == "VERIFIED",
        "email verification failed for {email}: {verify_status} {verified}"
    );

    let login_response = client
        .post(format!("{base}/auth/login"))
        .json(&json!({"email": email, "password": "Valid-passphrase-42"}))
        .send()
        .await?;
    anyhow::ensure!(
        login_response.status() == StatusCode::OK,
        "login failed for {email}: {}",
        login_response.status()
    );
    let refresh_cookie = process_refresh_cookie(login_response.headers())?;
    let login: Value = login_response.json().await?;
    let access_token = login["accessToken"]
        .as_str()
        .context("login response omitted accessToken")?
        .to_owned();
    Ok(ProcessUser {
        user_id,
        access_token,
        refresh_cookie,
    })
}

async fn process_json_request(
    client: &reqwest::Client,
    method: Method,
    base: &str,
    path: &str,
    bearer: Option<&str>,
    cookie: Option<&str>,
    body: Option<Value>,
) -> Result<(StatusCode, Value)> {
    let (status, value, _) =
        process_json_request_with_operation_id(client, method, base, path, bearer, cookie, body)
            .await?;
    Ok((status, value))
}

async fn process_json_request_with_operation_id(
    client: &reqwest::Client,
    method: Method,
    base: &str,
    path: &str,
    bearer: Option<&str>,
    cookie: Option<&str>,
    body: Option<Value>,
) -> Result<(StatusCode, Value, Option<Uuid>)> {
    let observation = forensics::RequestObservation::begin(&method, path);
    let mut request = client
        .request(method.clone(), format!("{base}{path}"))
        .header("x-request-id", observation.request_id.to_string());
    if let Some(token) = bearer {
        request = request.bearer_auth(token);
    }
    if let Some(cookie) = cookie {
        request = request.header(reqwest::header::COOKIE, cookie);
    }
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await?;
    let status = response.status();
    observation.response(status, response.headers());
    let operation_id = response
        .headers()
        .get("x-operation-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok());
    let raw = response.text().await?;
    let value = if raw.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&raw)
            .with_context(|| format!("{method} {path} returned invalid JSON"))?
    };
    Ok((status, value, operation_id))
}

async fn refresh_process_user(
    client: &reqwest::Client,
    base: &str,
    user: &mut ProcessUser,
) -> Result<()> {
    let response = client
        .post(format!("{base}/auth/refresh"))
        .header(reqwest::header::COOKIE, &user.refresh_cookie)
        .send()
        .await?;
    anyhow::ensure!(
        response.status() == StatusCode::OK,
        "process refresh returned {}",
        response.status()
    );
    let refresh_cookie = process_refresh_cookie(response.headers())?;
    let body: Value = response.json().await?;
    user.access_token = body["accessToken"]
        .as_str()
        .context("refresh response omitted accessToken")?
        .to_owned();
    user.refresh_cookie = refresh_cookie;
    Ok(())
}

async fn logout_process_user(
    client: &reqwest::Client,
    base: &str,
    user: &ProcessUser,
) -> Result<()> {
    let logout = client
        .post(format!("{base}/auth/logout"))
        .header(reqwest::header::COOKIE, &user.refresh_cookie)
        .send()
        .await?;
    anyhow::ensure!(
        logout.status() == StatusCode::NO_CONTENT,
        "process logout returned {}",
        logout.status()
    );
    let refresh = client
        .post(format!("{base}/auth/refresh"))
        .header(reqwest::header::COOKIE, &user.refresh_cookie)
        .send()
        .await?;
    anyhow::ensure!(
        refresh.status() == StatusCode::UNAUTHORIZED,
        "logout left the process refresh credential usable: {}",
        refresh.status()
    );
    Ok(())
}

fn process_refresh_cookie(headers: &reqwest::header::HeaderMap) -> Result<String> {
    let raw = headers
        .get(reqwest::header::SET_COOKIE)
        .context("Auth response omitted the refresh cookie")?
        .to_str()?;
    let cookie = raw
        .split(';')
        .next()
        .context("Auth Set-Cookie header had no cookie pair")?
        .trim();
    anyhow::ensure!(
        cookie.starts_with("refresh_token="),
        "Auth returned an unexpected refresh cookie name"
    );
    Ok(cookie.to_owned())
}

async fn upload_process_avatar(
    client: &reqwest::Client,
    base: &str,
    access_token: &str,
) -> Result<()> {
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        2,
        2,
        image::Rgba([0x28, 0x71, 0x9c, 0xff]),
    ));
    let mut encoded = std::io::Cursor::new(Vec::new());
    image.write_to(&mut encoded, image::ImageFormat::Png)?;
    let part = reqwest::multipart::Part::bytes(encoded.into_inner())
        .file_name("process-avatar.png")
        .mime_str("image/png")?;
    let form = reqwest::multipart::Form::new().part("avatar", part);
    let response = client
        .post(format!("{base}/api/profile/avatar"))
        .bearer_auth(access_token)
        .multipart(form)
        .send()
        .await?;
    anyhow::ensure!(
        response.status().is_success(),
        "cross-process avatar upload returned {}",
        response.status()
    );
    Ok(())
}

async fn fetch_process_avatar(
    client: &reqwest::Client,
    base: &str,
    access_token: &str,
    user_id: Uuid,
) -> Result<(String, Vec<u8>)> {
    let response = client
        .get(format!("{base}/api/profile/avatar/{user_id}"))
        .bearer_auth(access_token)
        .send()
        .await?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .context("avatar response omitted Content-Type")?
        .to_str()?
        .to_owned();
    let bytes = response.bytes().await?.to_vec();
    anyhow::ensure!(
        status == StatusCode::OK,
        "cross-process avatar fetch returned {status}: {} bytes",
        bytes.len()
    );
    Ok((content_type, bytes))
}

async fn wait_for_process_notification(
    client: &reqwest::Client,
    base: &str,
    access_token: &str,
    expected_type: &str,
) -> Result<Value> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let (status, notifications) = process_json_request(
            client,
            Method::GET,
            base,
            "/api/notifications?limit=50",
            Some(access_token),
            None,
            None,
        )
        .await?;
        anyhow::ensure!(
            status == StatusCode::OK,
            "notification read returned {status}: {notifications}"
        );
        if let Some(notification) = notifications["items"].as_array().and_then(|items| {
            items
                .iter()
                .find(|item| item["type"] == expected_type)
                .cloned()
        }) {
            return Ok(notification);
        }
        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for process notification {expected_type}");
        }
        sleep(Duration::from_millis(250)).await;
    }
}
fn insert_database(
    environment: &mut BTreeMap<String, String>,
    service: &str,
    host: &impl std::fmt::Display,
    port: u16,
    database: &str,
) {
    environment.insert(
        format!("{service}_DATABASE_URL"),
        database_url(host, port, database),
    );
}

fn insert_addresses(
    environment: &mut BTreeMap<String, String>,
    service: &str,
    addresses: ServiceAddresses,
) {
    environment.insert(format!("{service}_HTTP_ADDR"), addresses.http.to_string());
    environment.insert(
        format!("{service}_METRICS_ADDR"),
        addresses.metrics.to_string(),
    );
    environment.insert(format!("{service}_GRPC_ADDR"), addresses.grpc.to_string());
}

fn insert_tls_material(
    environment: &mut BTreeMap<String, String>,
    service_prefix: &str,
    workload_role: &str,
) -> Result<()> {
    let certificate = test_support::tls::issue(workload_role)?;
    let values = certificate.env_values();
    for (name, value) in test_support::tls::env_names(service_prefix)
        .into_iter()
        .zip(values)
    {
        environment.insert(name, value);
    }
    Ok(())
}

fn database_url(host: &impl std::fmt::Display, port: u16, database: &str) -> String {
    format!("postgres://postgres:postgres@{host}:{port}/{database}")
}

fn grpc_endpoint(address: SocketAddr) -> String {
    format!("https://{address}")
}

fn ephemeral_loopback_addr() -> Result<SocketAddr> {
    let listener = StdTcpListener::bind(("127.0.0.1", 0))?;
    let address = listener.local_addr()?;
    drop(listener);
    Ok(address)
}

async fn seed_pre_upgrade_notification_business_data(
    notification_database_url: &str,
) -> Result<LegacyNotificationMigrationFixture> {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(notification_database_url)
        .await
        .context("could not connect to baseline Notification database for migration fixture")?;
    let baseline = load_migration_ledger(&pool).await?;
    anyhow::ensure!(
        migration_versions(&baseline) == [1, 2, 3],
        "Notification business-data fixture requires the exact three-migration baseline"
    );
    let subject_state_exists: bool =
        sqlx::query_scalar("select to_regclass('notification_subject_states') is not null")
            .fetch_one(&pool)
            .await?;
    anyhow::ensure!(
        !subject_state_exists,
        "Notification subject state exists before migration 0004"
    );

    let fixture_now = Utc::now();
    let fixture = LegacyNotificationMigrationFixture {
        recipient_subject_id: Uuid::new_v4(),
        actor_subject_id: Uuid::new_v4(),
        recipient_notification_id: Uuid::new_v4(),
        actor_notification_id: Uuid::new_v4(),
        both_subjects_notification_id: Uuid::new_v4(),
        unaffected_notification_id: Uuid::new_v4(),
        recipient_suppressed_at: fixture_now - ChronoDuration::days(30),
        actor_suppressed_at: fixture_now - ChronoDuration::days(20),
        recipient_retention_until: fixture_now + ChronoDuration::days(30),
        actor_retention_until: fixture_now + ChronoDuration::days(60),
        existing_retention_until: fixture_now + ChronoDuration::days(90),
    };
    let other_recipient_id = Uuid::new_v4();
    let other_actor_id = Uuid::new_v4();
    sqlx::query(
        r#"
        insert into notification_recipient_suppressions (
            user_id, reason, suppressed_at, retention_until
        ) values
          ($1, 'account_deletion', $2, $3),
          ($4, 'account_deletion', $5, $6)
        "#,
    )
    .bind(fixture.recipient_subject_id)
    .bind(fixture.recipient_suppressed_at)
    .bind(fixture.recipient_retention_until)
    .bind(fixture.actor_subject_id)
    .bind(fixture.actor_suppressed_at)
    .bind(fixture.actor_retention_until)
    .execute(&pool)
    .await
    .context("could not seed legacy Notification suppression rows")?;

    let notification_rows = [
        (
            fixture.recipient_notification_id,
            fixture.recipient_subject_id,
            Uuid::new_v4(),
            other_actor_id,
            None,
        ),
        (
            fixture.actor_notification_id,
            other_recipient_id,
            Uuid::new_v4(),
            fixture.actor_subject_id,
            None,
        ),
        (
            fixture.both_subjects_notification_id,
            fixture.recipient_subject_id,
            Uuid::new_v4(),
            fixture.actor_subject_id,
            Some(fixture.existing_retention_until),
        ),
        (
            fixture.unaffected_notification_id,
            other_recipient_id,
            Uuid::new_v4(),
            other_actor_id,
            None,
        ),
    ];
    for (notification_id, user_id, event_id, actor_id, retention_until) in notification_rows {
        let created_at = Utc::now();
        sqlx::query(
            r#"
            insert into notifications (
                id, user_id, type, payload_json, created_at, read_at,
                event_id, actor_id, occurred_at, retention_until
            ) values ($1, $2, 'POST_LIKED', '{}', $3, null, $4, $5, $3, $6)
            "#,
        )
        .bind(notification_id)
        .bind(user_id)
        .bind(created_at)
        .bind(event_id)
        .bind(actor_id)
        .bind(retention_until)
        .execute(&pool)
        .await
        .context("could not seed a legacy Notification business row")?;
    }
    let live_rows: i64 =
        sqlx::query_scalar("select count(*) from notifications where deleted_at is null")
            .fetch_one(&pool)
            .await?;
    anyhow::ensure!(
        live_rows == 4,
        "historical Notification fixture was not intact before migration 0004: {live_rows} live rows"
    );
    pool.close().await;
    Ok(fixture)
}

async fn assert_restored_pre_upgrade_notification_snapshot(
    notification_database_url: &str,
    expected_ledger: &MigrationLedger,
    fixture: &LegacyNotificationMigrationFixture,
) -> Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(notification_database_url)
        .await
        .context("could not connect to restored pre-upgrade Notification database")?;
    let actual_ledger = load_migration_ledger(&pool).await?;
    anyhow::ensure!(
        actual_ledger.as_slice() == expected_ledger.as_slice(),
        "restored Notification backup changed its baseline SQLx migration ledger"
    );

    let suppression_states: Vec<LegacySuppressionMigrationState> = sqlx::query_as(
        r#"
            select user_id, reason, suppressed_at, retention_until
            from notification_recipient_suppressions
            where user_id in ($1, $2)
            order by user_id
            "#,
    )
    .bind(fixture.recipient_subject_id)
    .bind(fixture.actor_subject_id)
    .fetch_all(&pool)
    .await?;
    let mut expected_suppression_states = vec![
        (
            fixture.recipient_subject_id,
            "account_deletion".to_owned(),
            fixture.recipient_suppressed_at,
            Some(fixture.recipient_retention_until),
        ),
        (
            fixture.actor_subject_id,
            "account_deletion".to_owned(),
            fixture.actor_suppressed_at,
            Some(fixture.actor_retention_until),
        ),
    ];
    expected_suppression_states.sort_by_key(|state| state.0);
    let suppression_count: i64 =
        sqlx::query_scalar("select count(*) from notification_recipient_suppressions")
            .fetch_one(&pool)
            .await?;
    anyhow::ensure!(
        suppression_states == expected_suppression_states && suppression_count == 2,
        "restored Notification backup did not preserve both legacy suppression rows: states={suppression_states:?}, total={suppression_count}"
    );

    let mut expected_notification_ids = vec![
        fixture.recipient_notification_id,
        fixture.actor_notification_id,
        fixture.both_subjects_notification_id,
        fixture.unaffected_notification_id,
    ];
    expected_notification_ids.sort_unstable();
    let notification_ids: Vec<Uuid> =
        sqlx::query_scalar("select id from notifications order by id")
            .fetch_all(&pool)
            .await?;
    let live_count: i64 =
        sqlx::query_scalar("select count(*) from notifications where deleted_at is null")
            .fetch_one(&pool)
            .await?;
    anyhow::ensure!(
        notification_ids == expected_notification_ids && live_count == 4,
        "restored Notification backup did not preserve the four live business rows: ids={notification_ids:?}, live={live_count}"
    );
    pool.close().await;
    eprintln!(
        "historical Notification backup restore verified: baseline_migrations=3, legacy_suppressions=2, live_notifications=4"
    );
    Ok(())
}

async fn assert_pre_upgrade_notification_backfill(
    pool: &sqlx::PgPool,
    fixture: &LegacyNotificationMigrationFixture,
) -> Result<()> {
    for (subject_id, expected_suppressed_at, expected_retention_until) in [
        (
            fixture.recipient_subject_id,
            fixture.recipient_suppressed_at,
            fixture.recipient_retention_until,
        ),
        (
            fixture.actor_subject_id,
            fixture.actor_suppressed_at,
            fixture.actor_retention_until,
        ),
    ] {
        let state: NotificationSubjectMigrationState = sqlx::query_as(
            r#"
            select suppress_as_recipient, suppress_as_actor, reason,
                   suppressed_at, retention_until
            from notification_subject_states
            where subject_id = $1
            "#,
        )
        .bind(subject_id)
        .fetch_optional(pool)
        .await?
        .with_context(|| format!("legacy subject {subject_id} was not backfilled"))?;
        anyhow::ensure!(
            state
                == (
                    true,
                    true,
                    Some("account_deletion".to_owned()),
                    Some(expected_suppressed_at),
                    Some(expected_retention_until),
                ),
            "legacy suppression state was not preserved for subject {subject_id}: {state:?}"
        );
    }

    let both_subjects_deleted_at = fixture
        .recipient_suppressed_at
        .min(fixture.actor_suppressed_at);
    for (notification_id, deleted_at, retention_until) in [
        (
            fixture.recipient_notification_id,
            fixture.recipient_suppressed_at,
            fixture.recipient_retention_until,
        ),
        (
            fixture.actor_notification_id,
            fixture.actor_suppressed_at,
            fixture.actor_retention_until,
        ),
        (
            fixture.both_subjects_notification_id,
            both_subjects_deleted_at,
            fixture.existing_retention_until,
        ),
    ] {
        let state: NotificationRowMigrationState = sqlx::query_as(
            r#"
            select deleted_at, deleted_by, delete_reason, retention_until
            from notifications
            where id = $1
            "#,
        )
        .bind(notification_id)
        .fetch_optional(pool)
        .await?
        .with_context(|| {
            format!("legacy notification {notification_id} disappeared during upgrade")
        })?;
        anyhow::ensure!(
            state
                == (
                    Some(deleted_at),
                    None,
                    Some("account_deletion".to_owned()),
                    Some(retention_until),
                ),
            "legacy notification was not repaired as expected: id={notification_id}, state={state:?}"
        );
    }

    let unaffected: NotificationRowMigrationState = sqlx::query_as(
        r#"
        select deleted_at, deleted_by, delete_reason, retention_until
        from notifications
        where id = $1
        "#,
    )
    .bind(fixture.unaffected_notification_id)
    .fetch_optional(pool)
    .await?
    .context("unaffected legacy notification disappeared during upgrade")?;
    anyhow::ensure!(
        unaffected == (None, None, None, None),
        "migration changed an unaffected historical notification: {unaffected:?}"
    );
    eprintln!(
        "historical Notification data upgrade verified: legacy_suppressions=2, repaired_notifications=3, preserved_live_notifications=1"
    );
    Ok(())
}

async fn apply_migration_baselines(
    base_directory: &std::path::Path,
    postgres_host: &impl std::fmt::Display,
    postgres_port: u16,
    database_names: &BTreeMap<&str, String>,
) -> Result<BTreeMap<String, MigrationLedger>> {
    let mut baselines = BTreeMap::new();
    for (service, package_directory) in PROCESS_STACK_MIGRATION_SERVICES {
        let migration_directory = base_directory
            .join("services")
            .join(package_directory)
            .join("migrations");
        anyhow::ensure!(
            migration_directory.is_dir(),
            "{service} base-revision migration directory is missing: {}",
            migration_directory.display()
        );
        let migrator = Migrator::new(migration_directory.as_path())
            .await
            .with_context(|| format!("could not load {service} base-revision migrations"))?;
        let expected = expected_migration_ledger(&migrator);
        anyhow::ensure!(
            !expected.is_empty(),
            "{service} base revision has no up migrations"
        );
        let database_name_key = if service == "notification"
            && database_names.contains_key("notification_pre_upgrade")
        {
            "notification_pre_upgrade"
        } else {
            service
        };
        let database_name = database_names.get(database_name_key).with_context(|| {
            format!("{database_name_key} process-stack database was not created")
        })?;
        let url = database_url(postgres_host, postgres_port, database_name);
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .with_context(|| format!("could not connect to {service} baseline database"))?;
        migrator
            .run(&pool)
            .await
            .with_context(|| format!("could not apply {service} base-revision migrations"))?;
        let actual = load_migration_ledger(&pool).await?;
        pool.close().await;
        anyhow::ensure!(
            actual == expected,
            "{service} base-revision SQLx ledger differs from its migration source (expected_versions={:?}, actual_versions={:?})",
            migration_versions(&expected),
            migration_versions(&actual)
        );
        eprintln!(
            "process-stack schema baseline ready: service={service}, migrations={}, database={database_name}",
            expected.len()
        );
        baselines.insert(service.to_owned(), expected);
    }
    Ok(baselines)
}

async fn assert_migrations(
    database_url: &str,
    service: &str,
    base_commit: &str,
    baseline: &MigrationLedger,
    expected_deltas: Option<&BTreeMap<String, BTreeSet<i64>>>,
) -> Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await
        .with_context(|| format!("could not inspect {service} migration database"))?;
    let failed_count: i64 =
        sqlx::query_scalar("select count(*) from _sqlx_migrations where success = false")
            .fetch_one(&pool)
            .await
            .with_context(|| format!("{service} did not install the SQLx migration ledger"))?;
    let actual = load_migration_ledger(&pool).await?;
    pool.close().await;
    anyhow::ensure!(
        failed_count == 0,
        "{service} database contains {failed_count} failed SQLx migration records"
    );
    let current_directory = current_migration_directory(service)?;
    let current_migrator = Migrator::new(current_directory.as_path())
        .await
        .with_context(|| format!("could not load {service} current migrations"))?;
    let current = expected_migration_ledger(&current_migrator);
    anyhow::ensure!(
        !current.is_empty(),
        "{service} has no current up migrations"
    );
    anyhow::ensure!(
        actual == current,
        "{service} applied SQLx migration ledger differs from current source (expected_versions={:?}, actual_versions={:?})",
        migration_versions(&current),
        migration_versions(&actual)
    );
    for (base_version, base_checksum) in baseline {
        anyhow::ensure!(
            current
                .iter()
                .any(|(version, checksum)| version == base_version && checksum == base_checksum),
            "{service} migration {base_version} was removed or changed after base commit {base_commit}"
        );
    }
    anyhow::ensure!(
        current.len() >= baseline.len(),
        "{service} current migration ledger is smaller than its base-revision ledger"
    );
    let newly_applied = current.len() - baseline.len();
    let baseline_versions: BTreeSet<_> = baseline.iter().map(|(version, _)| *version).collect();
    let newly_applied_versions: BTreeSet<_> = current
        .iter()
        .map(|(version, _)| *version)
        .filter(|version| !baseline_versions.contains(version))
        .collect();
    if let Some(expected_deltas) = expected_deltas {
        let expected_versions = expected_deltas.get(service).cloned().unwrap_or_default();
        anyhow::ensure!(
            newly_applied_versions == expected_versions,
            "{service} applied unexpected schema migration delta from base commit {base_commit} (expected={expected_versions:?}, actual={newly_applied_versions:?})"
        );
    }
    let outcome = if newly_applied == 0 {
        "no_schema_delta"
    } else {
        "forward_upgrade_applied"
    };
    eprintln!(
        "process-stack schema upgrade verified: service={service}, base_commit={base_commit}, baseline_migrations={}, current_migrations={}, newly_applied={newly_applied}, outcome={outcome}",
        baseline.len(),
        current.len()
    );
    Ok(())
}

async fn verify_pre_upgrade_notification_compatibility(
    context: NotificationCompatibilityContext<'_>,
) -> Result<()> {
    let NotificationCompatibilityContext {
        readiness_client,
        processes,
        pre_upgrade_addresses,
        current_addresses,
        service_addresses,
        auth_environment,
        post_environment,
        gateway_environment,
        notification_database_url,
        legacy_fixture,
    } = context;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(notification_database_url)
        .await
        .context("could not inspect mixed-version Notification database state")?;
    assert_pre_upgrade_notification_backfill(&pool, legacy_fixture).await?;

    let auth_tls = load_tls_material(auth_environment, "auth")?;
    let auth_token = auth_environment
        .get("AUTH_SERVICE_CREDENTIAL")
        .context("AUTH_SERVICE_CREDENTIAL is missing from the process environment")?;
    let dispatcher = GrpcUserDeletionDispatcher::connect(
        grpc_endpoint(service_addresses["profile"].grpc),
        grpc_endpoint(service_addresses["friend"].grpc),
        grpc_endpoint(service_addresses["post"].grpc),
        grpc_endpoint(pre_upgrade_addresses.grpc),
        auth_token,
        Some(&auth_tls),
    )?;

    let deleted_subject_id = Uuid::new_v4();
    let deletion_actor_id = Uuid::new_v4();
    let saga_id = Uuid::new_v4();
    let retention_until = Utc::now() + ChronoDuration::days(30);
    dispatcher
        .apply(
            UserDeletionTarget::Notification.into(),
            saga_id,
            deleted_subject_id,
            deletion_actor_id,
            retention_until,
        )
        .await
        .context("pre-upgrade Notification did not accept the Auth deletion-saga command")?;

    let mirrored_state = sqlx::query_as::<_, (bool, bool, bool)>(
        r#"
        select
            exists(
                select 1
                from notification_recipient_suppressions
                where user_id = $1
            ),
            state.suppress_as_recipient,
            state.suppress_as_actor
        from notification_subject_states as state
        where state.subject_id = $1
        "#,
    )
    .bind(deleted_subject_id)
    .fetch_optional(&pool)
    .await?
    .context("legacy deletion did not produce mirrored subject state")?;
    anyhow::ensure!(
        mirrored_state == (true, true, true),
        "legacy deletion was not mirrored to both Notification suppression flags: {mirrored_state:?}"
    );

    let post_tls = load_tls_material(post_environment, "post")?;
    let post_token = post_environment
        .get("POST_SERVICE_CREDENTIAL")
        .context("POST_SERVICE_CREDENTIAL is missing from the process environment")?;
    let old_channel = platform::grpc_tls::lazy_channel_with_optional_tls(
        Some(&post_tls),
        &grpc_endpoint(pre_upgrade_addresses.grpc),
        Workload::Notification,
    )?;
    let current_channel = platform::grpc_tls::lazy_channel_with_optional_tls(
        Some(&post_tls),
        &grpc_endpoint(current_addresses.grpc),
        Workload::Notification,
    )?;
    let post_identity = ServiceIdentityClientInterceptor::new(Workload::Post, post_token)?;
    let mut old_client = notification_proto::notification_service_client::NotificationServiceClient::with_interceptor(
        old_channel,
        post_identity.clone(),
    );
    let mut current_client = notification_proto::notification_service_client::NotificationServiceClient::with_interceptor(
        current_channel,
        post_identity,
    );

    // A deployment rollback changes the executable, not the schema. Exercise
    // the pre-0004 writer against the expanded schema and prove the current
    // executable can read the row it committed before testing the guard below.
    let rollback_recipient_id = Uuid::new_v4();
    let rollback_actor_id = Uuid::new_v4();
    let rollback_event_id = Uuid::new_v4();
    let rollback_request = notification_proto::CreateNotificationRequest {
        user_id: rollback_recipient_id.to_string(),
        r#type: "SYSTEM".into(),
        payload_json: r#"{"kind":"rolling-upgrade-rollback-compatible"}"#.into(),
        event_id: rollback_event_id.to_string(),
        actor_id: rollback_actor_id.to_string(),
        occurred_at: Utc::now().to_rfc3339(),
    };
    let rollback_create = old_client
        .create_notification(rollback_request)
        .await
        .context("pre-upgrade Notification could not write against the expanded schema")?
        .into_inner();
    anyhow::ensure!(
        !rollback_create.suppressed && rollback_create.notification.is_some(),
        "pre-upgrade Notification did not create an unsuppressed row against the expanded schema"
    );
    // ListNotifications is an edge-only method that requires the recipient's
    // signed actor delegation. Read through the same Gateway identity, mTLS
    // material and delegation signer that the Gateway process uses.
    let gateway_tls = load_tls_material(gateway_environment, "gateway")?;
    let gateway_token = gateway_environment
        .get("GATEWAY_SERVICE_CREDENTIAL")
        .context("GATEWAY_SERVICE_CREDENTIAL is missing from the process environment")?;
    let edge_channel = platform::grpc_tls::lazy_channel_with_optional_tls(
        Some(&gateway_tls),
        &grpc_endpoint(current_addresses.grpc),
        Workload::Notification,
    )?;
    let mut edge_reader =
        notification_proto::notification_service_client::NotificationServiceClient::with_interceptor(
            platform::actor_delegation::ActorDelegationClient::new(
                edge_channel,
                platform::actor_delegation::ActorDelegationSigner::local_parity(Workload::Gateway),
                Workload::Notification,
            ),
            ServiceIdentityClientInterceptor::new(Workload::Gateway, gateway_token)?,
        );
    let rollback_read = edge_reader
        .list_notifications(platform::actor_delegation::request_for(
            notification_proto::ListNotificationsRequest {
                user_id: rollback_recipient_id.to_string(),
                cursor: String::new(),
                limit: 20,
            },
            &rollback_recipient_id.to_string(),
        )?)
        .await
        .context("current Notification could not read a row written by the pre-upgrade binary")?
        .into_inner();
    anyhow::ensure!(
        rollback_read
            .items
            .iter()
            .any(|notification| notification.event_id == rollback_event_id.to_string()),
        "current Notification did not observe the pre-upgrade writer's committed row"
    );

    let request = notification_proto::CreateNotificationRequest {
        user_id: Uuid::new_v4().to_string(),
        r#type: "SYSTEM".into(),
        payload_json: r#"{"kind":"rolling-upgrade-compatibility"}"#.into(),
        event_id: Uuid::new_v4().to_string(),
        actor_id: deleted_subject_id.to_string(),
        occurred_at: Utc::now().to_rfc3339(),
    };

    let old_status = old_client
        .create_notification(request.clone())
        .await
        .expect_err("database guard must reject a legacy actor-suppression bypass");
    anyhow::ensure!(
        old_status.code() == tonic::Code::InvalidArgument,
        "pre-upgrade Notification write returned unexpected gRPC status: {} ({})",
        old_status.code(),
        old_status.message()
    );

    let current_outcome = current_client
        .create_notification(request.clone())
        .await
        .context("current Notification did not handle deleted-actor suppression")?
        .into_inner();
    anyhow::ensure!(
        current_outcome.suppressed && current_outcome.notification.is_none(),
        "current Notification did not report actor suppression without creating a notification"
    );
    let persisted_count: i64 =
        sqlx::query_scalar("select count(*) from notifications where event_id = $1")
            .bind(Uuid::parse_str(&request.event_id)?)
            .fetch_one(&pool)
            .await?;
    pool.close().await;
    anyhow::ensure!(
        persisted_count == 0,
        "a mixed-version Notification writer persisted a deleted-actor notification"
    );

    for (name, metrics_address) in [
        (
            "notification-service-pre-upgrade",
            pre_upgrade_addresses.metrics,
        ),
        ("notification-service", current_addresses.metrics),
    ] {
        await_ready(
            readiness_client,
            processes,
            name,
            metrics_address,
            Duration::from_secs(10),
        )
        .await?;
    }
    eprintln!(
        "process-stack mixed-version Notification upgrade verified: old binary remains usable with expanded schema; legacy suppression mirrored; suppressed old writer rejected; current writer suppressed; persisted_notifications={persisted_count}"
    );
    Ok(())
}

fn load_tls_material(
    environment: &BTreeMap<String, String>,
    service_prefix: &str,
) -> Result<platform::grpc_tls::TlsMaterial> {
    let [certificate_name, key_name, ca_name] = test_support::tls::env_names(service_prefix);
    let certificate = environment
        .get(&certificate_name)
        .with_context(|| format!("{certificate_name} is missing from the process environment"))?;
    let key = environment
        .get(&key_name)
        .with_context(|| format!("{key_name} is missing from the process environment"))?;
    let ca = environment
        .get(&ca_name)
        .with_context(|| format!("{ca_name} is missing from the process environment"))?;
    platform::grpc_tls::TlsMaterial::load(certificate, key, ca)
}

fn parse_expected_migration_deltas(value: &str) -> Result<BTreeMap<String, BTreeSet<i64>>> {
    anyhow::ensure!(
        !value.trim().is_empty(),
        "PORT_RUST_EXPECT_MIGRATION_DELTAS must not be empty when set"
    );
    let mut expected = BTreeMap::<String, BTreeSet<i64>>::new();
    for entry in value.split(',') {
        let (service, version) = entry.split_once(':').with_context(|| {
            format!("invalid expected migration delta {entry:?}; expected service:version")
        })?;
        anyhow::ensure!(
            PROCESS_STACK_MIGRATION_SERVICES
                .iter()
                .any(|(known_service, _)| *known_service == service),
            "unknown service in expected migration delta: {service}"
        );
        let version: i64 = version
            .parse()
            .with_context(|| format!("invalid migration version in delta {entry:?}"))?;
        anyhow::ensure!(version > 0, "migration version must be positive: {entry:?}");
        anyhow::ensure!(
            expected
                .entry(service.to_owned())
                .or_default()
                .insert(version),
            "duplicate expected migration delta: {entry:?}"
        );
    }
    Ok(expected)
}

async fn load_migration_ledger(pool: &sqlx::PgPool) -> Result<MigrationLedger> {
    Ok(sqlx::query_as(
        "select version, checksum from _sqlx_migrations where success = true order by version",
    )
    .fetch_all(pool)
    .await?)
}

fn expected_migration_ledger(migrator: &Migrator) -> MigrationLedger {
    let mut ledger: MigrationLedger = migrator
        .iter()
        .filter(|migration| migration.migration_type.is_up_migration())
        .map(|migration| (migration.version, migration.checksum.to_vec()))
        .collect();
    ledger.sort_by_key(|(version, _)| *version);
    ledger
}

fn migration_versions(ledger: &MigrationLedger) -> Vec<i64> {
    ledger.iter().map(|(version, _)| *version).collect()
}

fn current_migration_directory(service: &str) -> Result<PathBuf> {
    let package_directory = PROCESS_STACK_MIGRATION_SERVICES
        .iter()
        .find_map(|(name, package)| (*name == service).then_some(*package))
        .with_context(|| format!("unknown process-stack migration service: {service}"))?;
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../services")
        .join(package_directory)
        .join("migrations"))
}

async fn await_ready(
    client: &reqwest::Client,
    processes: &mut ManagedProcesses,
    name: &str,
    metrics_address: SocketAddr,
    wait: Duration,
) -> Result<()> {
    let endpoint = format!("http://{metrics_address}/readyz");
    let deadline = Instant::now() + wait;
    loop {
        processes.ensure_running(name)?;
        let last_observation = match client.get(&endpoint).send().await {
            Ok(response) if response.status() == StatusCode::OK => {
                eprintln!("{name} is ready at {endpoint}");
                return Ok(());
            }
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                format!("readiness returned {status}: {body}")
            }
            Err(error) => error.to_string(),
        };
        if Instant::now() >= deadline {
            anyhow::bail!("{name} did not become ready within {wait:?}: {last_observation}");
        }
        sleep(Duration::from_millis(250)).await;
    }
}

async fn start_mail_sink(
    state: MailSinkState,
) -> Result<(SocketAddr, oneshot::Sender<()>, JoinHandle<Result<()>>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let (shutdown, stopped) = oneshot::channel();
    let app = Router::new()
        .route("/verification", post(capture_verification_mail))
        .with_state(state);
    let task = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await?;
        Ok::<(), anyhow::Error>(())
    });
    Ok((address, shutdown, task))
}

async fn capture_verification_mail(
    State(state): State<MailSinkState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> StatusCode {
    let expected = format!("Bearer {}", state.bearer_token);
    if headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        != Some(expected.as_str())
    {
        return StatusCode::UNAUTHORIZED;
    }
    state.inbox.lock().await.push(payload);
    StatusCode::ACCEPTED
}

#[cfg(test)]
mod migration_delta_tests {
    use super::{parse_expected_migration_deltas, BTreeMap, BTreeSet};

    #[test]
    fn parses_service_migration_delta_expectations() {
        assert_eq!(
            parse_expected_migration_deltas(
                "auth:9,auth:10,auth:11,auth:12,friend:11,gateway:14,gateway:15,notification:4,post:15,post:16,profile:7",
            )
            .unwrap(),
            BTreeMap::from([
                ("auth".to_owned(), BTreeSet::from([9, 10, 11, 12])),
                ("friend".to_owned(), BTreeSet::from([11])),
                ("gateway".to_owned(), BTreeSet::from([14, 15])),
                ("notification".to_owned(), BTreeSet::from([4])),
                ("post".to_owned(), BTreeSet::from([15, 16])),
                ("profile".to_owned(), BTreeSet::from([7])),
            ])
        );
    }

    #[test]
    fn rejects_unknown_services_and_duplicate_versions() {
        assert!(parse_expected_migration_deltas("unknown:4").is_err());
        assert!(parse_expected_migration_deltas("notification:4,notification:4").is_err());
    }

    #[test]
    fn rejects_empty_or_malformed_expectations() {
        assert!(parse_expected_migration_deltas("").is_err());
        assert!(parse_expected_migration_deltas("notification").is_err());
        assert!(parse_expected_migration_deltas("notification:0").is_err());
    }
}
