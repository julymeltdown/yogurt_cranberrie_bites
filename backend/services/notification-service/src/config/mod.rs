pub mod concert_runtime;
pub fn load() {}
use crate::adapters::out::persistence::notification_repository::PgNotificationRepository;
use crate::adapters::r#in::messaging::retention_purge_worker::NotificationRetentionPurgeWorker;
use crate::application::use_cases::apply_user_deletion::ApplyUserDeletionUseCase;
use crate::application::use_cases::create_notification::CreateNotificationUseCase;
use crate::application::use_cases::delete_notification::DeleteNotificationUseCase;
use crate::application::use_cases::list_notifications::ListNotificationsUseCase;
use crate::application::use_cases::mark_all_read::MarkAllReadUseCase;
use crate::application::use_cases::mark_read::MarkReadUseCase;
use crate::application::use_cases::purge_expired_notifications::PurgeExpiredNotificationsUseCase;
use platform::config::{
    env_or, parse_bool_setting, parse_u32_setting, parse_u64_setting,
    require_production_postgres_url, require_production_safe_socket_addr,
    runtime_mode_is_production,
};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const LOCAL_RETENTION_PURGE_ACTOR_ID: &str = "99999999-9999-9999-9999-999999999999";

#[derive(Clone)]
pub struct AppConfig {
    pub http_addr: SocketAddr,
    pub metrics_addr: SocketAddr,
    pub grpc_addr: SocketAddr,
    pub database_url: String,
    pub database_migration_mode: migration_runtime::MigrationMode,
    /// Largest number of pooled database connections this service opens.
    ///
    /// A single hardcoded value forced every service to share one concurrency
    /// ceiling. That is wrong in both directions: a read-heavy service that
    /// fans out is throttled to the same limit as an edge that holds a
    /// connection per in-flight request, and raising it everywhere to suit one
    /// service multiplies the load on PostgreSQL instead. The bound is
    /// configuration so a deployment can size each service to its own work.
    pub database_pool_max_connections: u32,
    /// Callers this service accepts, each with its own credential.
    pub caller_registry: platform::grpc_auth::CallerRegistry,
    /// Verifies Gateway actor delegations on actor-bound methods.
    pub actor_delegation: platform::actor_delegation::ActorDelegationVerifier,
    /// Mutual TLS material for internal gRPC, resolved at load time.
    ///
    /// `None` means the documented plaintext development path. Production cannot
    /// reach a running service with `None` because `load` fails first.
    pub tls_material: Option<platform::grpc_tls::TlsMaterial>,
    pub retention_purge_worker_enabled: bool,
    pub retention_purge_worker_interval_ms: u64,
    pub retention_purge_actor_id: String,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let production_mode = runtime_mode_is_production(
            &env_or("NOTIFICATION_RUNTIME_MODE", "production"),
            "NOTIFICATION_RUNTIME_MODE",
        )?;
        let tls_material = platform::config::resolve_tls_material(production_mode, "notification")?;
        // The edge serves notification reads, post-service delivers fan-out
        // notifications, and the auth coordinator alone may apply deletion.
        let caller_registry = platform::config::build_caller_registry(
            production_mode,
            &[
                platform::grpc_auth::Workload::Gateway,
                platform::grpc_auth::Workload::Post,
                platform::grpc_auth::Workload::Auth,
            ],
            &[platform::grpc_auth::Workload::Gateway],
        )?;
        let retention_purge_actor_id = resolve_retention_purge_actor_id(
            production_mode,
            platform::config::optional_env("NOTIFICATION_RETENTION_PURGE_ACTOR_ID"),
        )?;

        let actor_delegation = platform::actor_delegation::ActorDelegationVerifier::from_env(
            production_mode,
            platform::grpc_auth::Workload::Notification,
        )?;
        let config = Self {
            database_migration_mode: migration_runtime::MigrationMode::from_env_value(
                std::env::var("NOTIFICATION_DATABASE_MIGRATION_MODE"),
            )?,
            http_addr: env_or("NOTIFICATION_HTTP_ADDR", "127.0.0.1:8084").parse()?,
            metrics_addr: env_or("NOTIFICATION_METRICS_ADDR", "127.0.0.1:9084").parse()?,
            grpc_addr: env_or("NOTIFICATION_GRPC_ADDR", "127.0.0.1:50054").parse()?,
            database_url: env_or(
                "NOTIFICATION_DATABASE_URL",
                "postgres://postgres:postgres@localhost:5432/notification_db",
            ),
            database_pool_max_connections: parse_u32_setting(
                "NOTIFICATION_DB_POOL_MAX_CONNECTIONS",
                &env_or("NOTIFICATION_DB_POOL_MAX_CONNECTIONS", "5"),
                1,
                512,
            )?,
            caller_registry,
            actor_delegation,
            tls_material,
            retention_purge_worker_enabled: parse_bool_setting(
                "NOTIFICATION_RETENTION_PURGE_WORKER_ENABLED",
                &env_or("NOTIFICATION_RETENTION_PURGE_WORKER_ENABLED", "true"),
            )?,
            retention_purge_worker_interval_ms: parse_u64_setting(
                "NOTIFICATION_RETENTION_PURGE_WORKER_INTERVAL_MS",
                &env_or("NOTIFICATION_RETENTION_PURGE_WORKER_INTERVAL_MS", "3600000"),
                1_000,
                86_400_000,
            )?,
            retention_purge_actor_id,
        };
        validate_production_config(production_mode, &config)?;
        Ok(config)
    }
}

fn validate_production_config(production_mode: bool, config: &AppConfig) -> anyhow::Result<()> {
    require_production_safe_socket_addr(
        production_mode,
        "NOTIFICATION_HTTP_ADDR",
        config.http_addr,
    )?;
    require_production_safe_socket_addr(
        production_mode,
        "NOTIFICATION_GRPC_ADDR",
        config.grpc_addr,
    )?;
    require_production_safe_socket_addr(
        production_mode,
        "NOTIFICATION_METRICS_ADDR",
        config.metrics_addr,
    )?;
    require_production_postgres_url(
        production_mode,
        "NOTIFICATION_DATABASE_URL",
        &config.database_url,
    )?;
    Ok(())
}

fn resolve_retention_purge_actor_id(
    production_mode: bool,
    configured: Option<String>,
) -> anyhow::Result<String> {
    let actor_id = match configured
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        Some(value) => value,
        None if !production_mode => LOCAL_RETENTION_PURGE_ACTOR_ID.into(),
        None => anyhow::bail!("NOTIFICATION_RETENTION_PURGE_ACTOR_ID is required in production"),
    };
    Uuid::parse_str(&actor_id)
        .map_err(|_| anyhow::anyhow!("NOTIFICATION_RETENTION_PURGE_ACTOR_ID must be a UUID"))?;
    Ok(actor_id)
}

#[derive(Clone)]
pub struct AppState {
    pub apply_user_deletion: ApplyUserDeletionUseCase<Arc<PgNotificationRepository>>,
    pub create_notification: CreateNotificationUseCase<Arc<PgNotificationRepository>>,
    pub delete_notification: DeleteNotificationUseCase<Arc<PgNotificationRepository>>,
    pub purge_expired_notifications:
        PurgeExpiredNotificationsUseCase<Arc<PgNotificationRepository>>,
    pub retention_purge_worker: NotificationRetentionPurgeWorker<Arc<PgNotificationRepository>>,
    pub list_notifications: ListNotificationsUseCase<Arc<PgNotificationRepository>>,
    pub mark_read: MarkReadUseCase<Arc<PgNotificationRepository>>,
    pub mark_all_read: MarkAllReadUseCase<Arc<PgNotificationRepository>>,
    pub metrics: Arc<NotificationMetrics>,
}

#[derive(Clone, Default)]
pub struct NotificationMetrics {
    counters: Arc<Mutex<BTreeMap<String, i64>>>,
}

impl NotificationMetrics {
    pub fn record_retention_purge_run(&self, outcome: &str) {
        self.inc(
            "notification_retention_purge_runs_total",
            &[("outcome", outcome)],
            1,
        );
    }

    pub fn record_retention_purged_notifications(&self, outcome: &str, count: i64) {
        self.inc(
            "notification_retention_purged_notifications_total",
            &[("outcome", outcome)],
            count,
        );
    }

    pub fn render_prometheus(&self) -> String {
        let counters = self.counters.lock().expect("notification metrics");
        let mut output = String::new();
        for (sample, value) in counters.iter() {
            output.push_str(sample);
            output.push(' ');
            output.push_str(&value.to_string());
            output.push('\n');
        }
        output
    }

    fn inc(&self, name: &str, labels: &[(&str, &str)], amount: i64) {
        let sample = format!("{name}{{{}}}", label_set(labels));
        let mut counters = self.counters.lock().expect("notification metrics");
        *counters.entry(sample).or_insert(0) += amount;
    }
}

fn label_set(labels: &[(&str, &str)]) -> String {
    labels
        .iter()
        .map(|(name, value)| format!("{name}=\"{}\"", escape_label_value(value)))
        .collect::<Vec<_>>()
        .join(",")
}

fn escape_label_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

pub async fn connect_pool(config: &AppConfig) -> anyhow::Result<PgPool> {
    let options = migration_runtime::serving_options(
        config.database_url.parse()?,
        config.database_migration_mode,
    );
    let pool = PgPoolOptions::new()
        .max_connections(config.database_pool_max_connections)
        .connect_with(options)
        .await?;
    let migrations = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    migration_runtime::prepare_database(
        &pool,
        migrations.as_path(),
        config.database_migration_mode,
        &[migration_runtime::SchemaProbe {
            name: "NOTIFICATION required schema objects",
            sql: include_str!("schema-probe.sql"),
        }],
    )
    .await?;
    Ok(pool)
}

pub async fn build_state(config: &AppConfig, pool: PgPool) -> anyhow::Result<AppState> {
    let repository = Arc::new(PgNotificationRepository::new(pool));
    let runtime = Arc::new(crate::adapters::out::runtime::SystemNotificationRuntime);
    let purge_expired_notifications =
        PurgeExpiredNotificationsUseCase::new(repository.clone(), runtime.clone());
    let metrics = Arc::new(NotificationMetrics::default());
    let retention_purge_worker = NotificationRetentionPurgeWorker::new(
        purge_expired_notifications.clone(),
        config.retention_purge_actor_id.clone(),
        metrics.clone(),
    );

    Ok(AppState {
        apply_user_deletion: ApplyUserDeletionUseCase::new(repository.clone()),
        create_notification: CreateNotificationUseCase::new(repository.clone(), runtime.clone()),
        delete_notification: DeleteNotificationUseCase::new(repository.clone(), runtime.clone()),
        purge_expired_notifications,
        retention_purge_worker,
        list_notifications: ListNotificationsUseCase::new(repository.clone()),
        mark_read: MarkReadUseCase::new(repository.clone(), runtime.clone()),
        mark_all_read: MarkAllReadUseCase::new(repository, runtime.clone()),
        metrics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_config_rejects_unsafe_notification_defaults() {
        let mut config = production_config();
        config.http_addr = "127.0.0.1:8084".parse().expect("addr");
        assert!(validate_production_config(true, &config)
            .expect_err("loopback HTTP bind must fail")
            .to_string()
            .contains("NOTIFICATION_HTTP_ADDR"));

        let mut config = production_config();
        config.database_url = "postgres://postgres:postgres@localhost:5432/notification_db".into();
        assert!(validate_production_config(true, &config)
            .expect_err("local database must fail")
            .to_string()
            .contains("NOTIFICATION_DATABASE_URL"));
    }

    #[test]
    fn production_config_accepts_explicit_remote_notification_settings() {
        validate_production_config(true, &production_config()).expect("production config");
    }

    #[test]
    fn parity_config_allows_local_notification_defaults() {
        let mut config = production_config();
        config.http_addr = "127.0.0.1:8084".parse().expect("addr");
        config.grpc_addr = "127.0.0.1:50054".parse().expect("addr");
        config.metrics_addr = "127.0.0.1:9084".parse().expect("addr");
        config.database_url = "postgres://postgres:postgres@localhost:5432/notification_db".into();
        validate_production_config(false, &config).expect("parity config");
    }

    fn production_config() -> AppConfig {
        AppConfig {
            database_migration_mode: migration_runtime::MigrationMode::Startup,
            http_addr: "0.0.0.0:8084".parse().expect("addr"),
            metrics_addr: "0.0.0.0:9084".parse().expect("addr"),
            grpc_addr: "0.0.0.0:50054".parse().expect("addr"),
            database_pool_max_connections: 5,
            database_url: "postgres://notification:secret@postgres.internal:5432/notification_db"
                .into(),
            actor_delegation: platform::actor_delegation::ActorDelegationVerifier::local_parity(
                platform::grpc_auth::Workload::Notification,
            ),
            caller_registry: platform::grpc_auth::CallerRegistry::new()
                .register(
                    platform::grpc_auth::Workload::Gateway,
                    "production-service-token",
                )
                .expect("production caller registry"),
            tls_material: None,
            retention_purge_worker_enabled: true,
            retention_purge_worker_interval_ms: 3_600_000,
            retention_purge_actor_id: LOCAL_RETENTION_PURGE_ACTOR_ID.into(),
        }
    }

    mod external_pool_acceptance {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/migration-runtime/tests/support/service_pool.rs"
        ));
    }

    #[tokio::test]
    async fn external_migration_mode_rejects_schema_damage_without_owner_privileges(
    ) -> anyhow::Result<()> {
        external_pool_acceptance::verify(
            |url, mode| async move {
                let mut config = production_config();
                config.database_url = url;
                config.database_migration_mode = mode;
                connect_pool(&config).await
            },
            "notifications",
            "id",
            "notification_subject_state_guard",
        )
        .await
    }
}
