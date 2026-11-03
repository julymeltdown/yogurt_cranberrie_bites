use axum::{extract::State, http::StatusCode, routing::get, Router};
use notification_service::adapters::r#in::grpc::server::{
    proto::notification_service_server::NotificationServiceServer, NotificationGrpcServer,
};
use notification_service::adapters::r#in::http;
use notification_service::config::{build_state, connect_pool, AppConfig};
use platform::readiness::{Probe, Readiness, DATABASE_PROBE_TIMEOUT};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if migration_runtime::handle_command(
        std::env::args_os().skip(1),
        "NOTIFICATION_DATABASE_URL",
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations"),
    )
    .await?
    {
        return Ok(());
    }

    let telemetry = platform::telemetry::init("notification-service")?;
    platform::shutdown::install_signal_handlers()?;
    let config = AppConfig::load()?;
    let pool = connect_pool(&config).await?;
    let readiness = readiness(pool.clone());
    let health = health_router(readiness.clone());
    let state = Arc::new(build_state(&config, pool).await?);
    let (http_listener, metrics_listener) =
        http::bind_listeners(config.http_addr, config.metrics_addr).await?;
    let worker = state.retention_purge_worker.clone();
    let server = NotificationGrpcServer::new(state.clone());
    let application = http::router(state.clone());
    let metrics = http::metrics_router(state).merge(health);
    let result = platform::shutdown::supervise(async move {
        let http = async move {
            tokio::try_join!(
                async {
                    axum::serve(http_listener, application)
                        .with_graceful_shutdown(platform::shutdown::global().cancelled())
                        .await
                },
                async {
                    axum::serve(metrics_listener, metrics)
                        .with_graceful_shutdown(platform::shutdown::global().cancelled())
                        .await
                },
            )?;
            Ok::<(), anyhow::Error>(())
        };
        let grpc = async move {
            // Carry the caller request identifier into this service, so any call made
            // while handling the request continues the same chain.
            let mut builder = Server::builder()
                .layer(platform::correlation::CorrelationLayer)
                .layer(platform::call_deadline::CallDeadlineLayer)
                .layer(platform::trace_context::TraceContextLayer);
            if let Some(material) = config.tls_material.as_ref() {
                builder = builder
                    .tls_config(material.server_config()?)
                    .map_err(anyhow::Error::from)?;
            }
            builder
                .add_service(readiness.grpc_service(config.caller_registry.clone())?)
                .add_service(platform::grpc_method_policy::guard(
    &notification_service::adapters::r#in::grpc::method_policy::NOTIFICATION_METHOD_POLICY,
    NotificationServiceServer::new(server),
    config.caller_registry.clone(),
    config.actor_delegation.clone(),
)?)
                .serve_with_shutdown(config.grpc_addr, platform::shutdown::global().cancelled())
                .await?;
            Ok::<(), anyhow::Error>(())
        };
        let retention = async move {
            if config.retention_purge_worker_enabled {
                platform::shutdown::periodic(
                    "notification-retention",
                    Duration::from_millis(config.retention_purge_worker_interval_ms),
                    || worker.tick(),
                )
                .await
            } else {
                platform::shutdown::idle().await
            }
        };
        tokio::try_join!(http, grpc, retention)?;
        Ok(())
    })
    .await;
    telemetry.shutdown();
    result
}

fn readiness(pool: PgPool) -> Readiness {
    Readiness::new(Probe::new(
        "database",
        true,
        DATABASE_PROBE_TIMEOUT,
        move || {
            let pool = pool.clone();
            async move { sqlx::query("select 1").execute(&pool).await.is_ok() }
        },
    ))
}

fn health_router(readiness: Readiness) -> Router {
    Router::new()
        .route("/livez", get(|| async { StatusCode::OK }))
        .route("/readyz", get(ready))
        .with_state(readiness)
}

async fn ready(State(readiness): State<Readiness>) -> impl axum::response::IntoResponse {
    let report = readiness.report().await;
    let status = if report.ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        axum::Json(report),
    )
}
