use notification_service::config::{build_state, AppConfig, AppState};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

pub(crate) async fn lazy_app_state() -> Arc<AppState> {
    let config = AppConfig {
        database_migration_mode: migration_runtime::MigrationMode::Startup,
        http_addr: "127.0.0.1:18084".parse().expect("http addr"),
        metrics_addr: "127.0.0.1:19084".parse().expect("metrics addr"),
        grpc_addr: "127.0.0.1:15054".parse().expect("grpc addr"),
        database_url: "postgres://notification:secret@db.internal:5432/notification_db".into(),
        database_pool_max_connections: 5,
        actor_delegation: platform::actor_delegation::ActorDelegationVerifier::local_parity(
            platform::grpc_auth::Workload::Notification,
        ),
        caller_registry: platform::grpc_auth::CallerRegistry::new()
            .register(
                platform::grpc_auth::Workload::Gateway,
                "test-gateway-credential",
            )
            .expect("gateway caller"),
        tls_material: None,
        retention_purge_worker_enabled: true,
        retention_purge_worker_interval_ms: 1_000,
        retention_purge_actor_id: "99999999-9999-9999-9999-999999999999".into(),
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy(&config.database_url)
        .expect("lazy notification pool");
    Arc::new(build_state(&config, pool).await.expect("app state"))
}
