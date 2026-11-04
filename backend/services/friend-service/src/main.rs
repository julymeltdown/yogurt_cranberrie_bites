use axum::{extract::State, http::StatusCode, routing::get, Router};
use friend_service::adapters::r#in::grpc::server::{
    proto::friend_service_server::FriendServiceServer, FriendGrpcServer,
};
use friend_service::adapters::r#in::http;
use friend_service::config::{build_state, connect_pool, AppConfig};
use platform::readiness::{Probe, Readiness, DATABASE_PROBE_TIMEOUT};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if migration_runtime::handle_command(
        std::env::args_os().skip(1),
        "FRIEND_DATABASE_URL",
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations"),
    )
    .await?
    {
        return Ok(());
    }

    let telemetry = platform::telemetry::init("friend-service")?;
    platform::shutdown::install_signal_handlers()?;
    let config = AppConfig::load()?;
    let pool = connect_pool(&config).await?;
    let relationship_verifier = platform::actor_delegation::relationship_read::RelationshipReadVerifier::configured_from_env(platform::grpc_auth::Workload::Friend, &config.actor_delegation)?;
    let relationship_service =
        friend_service::adapters::r#in::grpc::concert_relationship::RelationshipServer::from_pool(
            pool.clone(),
        );
    let invalidation_service = config
        .tls_material
        .as_ref()
        .map(|_| {
            friend_service::adapters::r#in::grpc::concert_invalidation::guarded_server(
                pool.clone(),
                config.caller_registry.clone(),
            )
        })
        .transpose()?;
    let authority_store = friend_service::concert_authority::configured_store(
        pool.clone(),
        config.tls_material.as_ref(),
        &config.outbound_identity,
        relationship_verifier.as_ref(),
    )
    .await?;
    let authority_service = authority_store
        .clone()
        .map(|store| {
            friend_service::adapters::r#in::grpc::concert_authority::guarded_server(
                store,
                friend_service::concert_authority::owner_caller_registry()?,
                relationship_verifier
                    .clone()
                    .expect("configured owner verifier"),
            )
        })
        .transpose()?;
    let readiness = readiness(pool.clone()).with_dependencies(vec![
        friend_service::concert_authority::readiness_probe(authority_store.clone()),
        Probe::grpc(
            platform::edge::Edge::FriendProfile,
            &config.profile_grpc_endpoint,
            &config.outbound_identity,
            config.tls_material.as_ref(),
        )?,
    ])?;
    let health = health_router(readiness.clone());
    let state = Arc::new(build_state(&config, pool).await?);
    let (http_listener, metrics_listener) =
        http::bind_listeners(config.http_addr, config.metrics_addr).await?;
    let application = http::router();
    let metrics = http::metrics_router_with_edge_deadlines(
        state.recommendation_metrics.clone(),
        state.mutation_receipt_retention_metrics.clone(),
        state.edge_deadline_metrics.clone(),
    )
    .merge(health);
    let worker = state.recommendation_refresh_worker.clone();
    let retention_worker = state.recommendation_retention_worker.clone();
    let mutation_receipt_retention_worker = state.mutation_receipt_retention_worker.clone();
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
                .add_optional_service(relationship_verifier.map(|verifier| platform::grpc_method_policy::guard(
                    &friend_service::adapters::r#in::grpc::concert_relationship::POLICY,
                    friend_service::adapters::r#in::grpc::concert_relationship::proto::friend_concert_relationship_service_server::FriendConcertRelationshipServiceServer::new(relationship_service)
                        .max_decoding_message_size(4096).max_encoding_message_size(4096),
                    config.caller_registry.clone(), verifier.into_inner(),
                )).transpose()?)
                .add_optional_service(invalidation_service)
                .add_optional_service(authority_service)
                .add_service(platform::grpc_method_policy::guard_with_forwarding(
                    &friend_service::adapters::r#in::grpc::method_policy::FRIEND_METHOD_POLICY,
                    friend_service::adapters::r#in::grpc::method_policy::FRIEND_FORWARDING,
                    FriendServiceServer::new(FriendGrpcServer::new(state)),
                    config.caller_registry.clone(),
                    config.actor_delegation.clone(),
                )?)
                .serve_with_shutdown(config.grpc_addr, platform::shutdown::global().cancelled())
                .await?;
            Ok::<(), anyhow::Error>(())
        };
        let refresh = async move {
            if config.recommendation_worker_enabled {
                platform::shutdown::periodic(
                    "people-refresh",
                    Duration::from_millis(config.recommendation_worker_interval_ms)
                        .max(Duration::from_millis(100)),
                    || worker.tick(chrono::Utc::now()),
                )
                .await
            } else {
                platform::shutdown::idle().await
            }
        };
        let retention = async move {
            if config.recommendation_retention_worker_enabled {
                platform::shutdown::periodic(
                    "people-retention",
                    Duration::from_millis(config.recommendation_retention_worker_interval_ms)
                        .max(Duration::from_secs(1)),
                    || retention_worker.tick(chrono::Utc::now()),
                )
                .await
            } else {
                platform::shutdown::idle().await
            }
        };
        let mutation_receipt_retention = async move {
            if config.mutation_receipt_retention_worker_enabled {
                platform::shutdown::periodic(
                    "friend-mutation-receipt-retention",
                    Duration::from_millis(config.mutation_receipt_retention_worker_interval_ms)
                        .max(Duration::from_secs(1)),
                    || mutation_receipt_retention_worker.tick(),
                )
                .await
            } else {
                platform::shutdown::idle().await
            }
        };
        tokio::try_join!(http, grpc, refresh, retention, mutation_receipt_retention, friend_service::concert_authority::run_recovery(authority_store))?;
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
