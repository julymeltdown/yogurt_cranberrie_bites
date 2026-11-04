use auth_service::adapters::r#in::grpc::server::{
    proto::auth_service_server::AuthServiceServer, AuthGrpcServer,
};
use auth_service::adapters::r#in::http;
use auth_service::adapters::r#in::http::server::{bind_http_listeners, serve_http_listeners};
use auth_service::adapters::r#in::messaging::mail_outbox_worker::EmailOutboxWorker;
use auth_service::config::{
    build_state_with_deletion_runtime, concert_deletion::ConcertDeletionSettings, connect_pool,
    AppConfig, RuntimeMode,
};
use axum::{extract::State, http::StatusCode, routing::get, Router};
use platform::readiness::{Probe, Readiness, DATABASE_PROBE_TIMEOUT};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if migration_runtime::handle_command(
        std::env::args_os().skip(1),
        "AUTH_DATABASE_URL",
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations"),
    )
    .await?
    {
        return Ok(());
    }

    let telemetry = platform::telemetry::init("auth-service")?;
    platform::shutdown::install_signal_handlers()?;
    let config = AppConfig::load()?;
    let deletion_settings = ConcertDeletionSettings::load()?;
    let deletion_source_registry = deletion_settings.startup_source_registry(&config)?;
    let pool = connect_pool(&config).await?;
    let relationship_verifier = platform::actor_delegation::relationship_read::RelationshipReadVerifier::configured_from_env(platform::grpc_auth::Workload::Auth, &config.actor_delegation)?;
    let relationship_service =
        auth_service::adapters::r#in::grpc::concert_relationship::RelationshipServer::from_pool(
            pool.clone(),
        );
    let invalidation_service = config
        .tls_material
        .as_ref()
        .map(|_| {
            auth_service::adapters::r#in::grpc::concert_invalidation::guarded_server(
                pool.clone(),
                config.caller_registry.clone(),
            )
        })
        .transpose()?;
    let authority_store = auth_service::concert_authority::configured_store(
        pool.clone(),
        config.tls_material.as_ref(),
        &config.outbound_identity,
        relationship_verifier.as_ref(),
    )
    .await?;
    let authority_service = authority_store
        .clone()
        .map(|store| {
            auth_service::adapters::r#in::grpc::concert_authority::guarded_server(
                store,
                auth_service::concert_authority::owner_caller_registry()?,
                relationship_verifier
                    .clone()
                    .expect("configured owner verifier"),
            )
        })
        .transpose()?;
    let deletion_source_service = deletion_source_registry
        .map(|registry| {
            auth_service::adapters::r#in::grpc::concert_deletion_source::guarded_server(
                pool.clone(),
                registry,
            )
        })
        .transpose()?;
    let readiness = readiness(pool.clone()).with_dependencies(vec![
        auth_service::concert_authority::readiness_probe(authority_store.clone()),
        Probe::grpc(
            platform::edge::Edge::AuthProfile,
            &config.profile_grpc_endpoint,
            &config.outbound_identity,
            config.tls_material.as_ref(),
        )?,
        Probe::grpc(
            platform::edge::Edge::AuthFriend,
            &config.friend_grpc_endpoint,
            &config.outbound_identity,
            config.tls_material.as_ref(),
        )?,
        Probe::grpc(
            platform::edge::Edge::AuthPost,
            &config.post_grpc_endpoint,
            &config.outbound_identity,
            config.tls_material.as_ref(),
        )?,
        Probe::grpc(
            platform::edge::Edge::AuthNotification,
            &config.notification_grpc_endpoint,
            &config.outbound_identity,
            config.tls_material.as_ref(),
        )?,
    ])?;
    let health = health_router(readiness.clone());
    let mail_worker = EmailOutboxWorker::from_env(
        pool.clone(),
        matches!(config.runtime_mode, RuntimeMode::Production),
    )?;
    let state =
        Arc::new(build_state_with_deletion_runtime(&config, pool, deletion_settings).await?);
    let http_router: Router = http::router(state.clone());
    let metrics_router = http::metrics_router(state.metrics.clone()).merge(health);
    let http_listeners = bind_http_listeners(config.http_addr, config.metrics_addr).await?;
    let grpc_addr = config.grpc_addr;
    let caller_registry = config.caller_registry.clone();
    let actor_delegation = config.actor_delegation.clone();
    let lifecycle_cleanup_worker = state.lifecycle_cleanup_worker.clone();
    let lifecycle_cleanup_worker_enabled = config.lifecycle_cleanup_worker_enabled;
    let lifecycle_cleanup_worker_interval =
        Duration::from_millis(config.lifecycle_cleanup_worker_interval_ms);
    let user_deletion_worker = state.user_deletion_worker.clone();
    let user_deletion_worker_enabled = config.user_deletion_worker_enabled;
    let user_deletion_worker_interval =
        Duration::from_millis(config.user_deletion_worker_interval_ms);
    let grpc_service = AuthGrpcServer::new(state);

    let result = platform::shutdown::supervise(async move {
        let http = serve_http_listeners(http_listeners, http_router, metrics_router);
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
                .add_optional_service(relationship_verifier.map(|verifier| platform::grpc_method_policy::guard(
                    &auth_service::adapters::r#in::grpc::concert_relationship::POLICY,
                    auth_service::adapters::r#in::grpc::concert_relationship::proto::auth_concert_relationship_service_server::AuthConcertRelationshipServiceServer::new(relationship_service)
                        .max_decoding_message_size(4096).max_encoding_message_size(4096),
                    config.caller_registry.clone(), verifier.into_inner(),
                )).transpose()?)
                .add_optional_service(invalidation_service)
                .add_optional_service(authority_service)
                .add_optional_service(deletion_source_service)
                .add_service(platform::grpc_method_policy::guard(
                    &auth_service::adapters::r#in::grpc::method_policy::AUTH_METHOD_POLICY,
                    AuthServiceServer::new(grpc_service),
                    caller_registry,
                    actor_delegation,
                )?)
                .serve_with_shutdown(grpc_addr, platform::shutdown::global().cancelled())
                .await
                .map_err(anyhow::Error::from)
        };
        let lifecycle_cleanup = async move {
            if lifecycle_cleanup_worker_enabled {
                lifecycle_cleanup_worker
                    .run(lifecycle_cleanup_worker_interval)
                    .await
            } else {
                platform::shutdown::global().cancelled().await;
                Ok(())
            }
        };
        let user_deletion = async move {
            if user_deletion_worker_enabled {
                user_deletion_worker
                    .run(user_deletion_worker_interval)
                    .await
            } else {
                platform::shutdown::global().cancelled().await;
                Ok(())
            }
        };
        tokio::try_join!(
            http,
            grpc,
            lifecycle_cleanup,
            user_deletion,
            mail_worker.run(),
            auth_service::concert_authority::run_recovery(authority_store)
        )?;
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
