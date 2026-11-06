use axum::{extract::State, http::StatusCode, routing::get, Router};
use platform::readiness::{Probe, Readiness, DATABASE_PROBE_TIMEOUT};
use post_service::adapters::r#in::grpc::server::{
    proto::post_service_server::PostServiceServer, PostGrpcServer,
};
use post_service::adapters::r#in::http;
use post_service::config::{build_state, connect_pool, AppConfig};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if migration_runtime::handle_command(
        std::env::args_os().skip(1),
        "POST_DATABASE_URL",
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations"),
    )
    .await?
    {
        return Ok(());
    }

    let telemetry = platform::telemetry::init("post-service")?;
    platform::shutdown::install_signal_handlers()?;
    let config = AppConfig::load()?;
    let concert_startup = post_service::config::concert_runtime::ReadStartup::load(&config)?;
    let pool = connect_pool(&config).await?;
    let (concert_read, concert_settlement, concert_command) = concert_startup
        .map(|startup| startup.compose(pool.clone()))
        .transpose()?
        .map_or((None, None, None), |(read, settlement, command)| {
            (Some(read), Some(settlement), command)
        });
    let readiness = readiness(pool.clone()).with_dependencies(vec![
        Probe::grpc(
            platform::edge::Edge::PostProfile,
            &config.profile_grpc_endpoint,
            &config.outbound_identity,
            config.tls_material.as_ref(),
        )?,
        Probe::grpc(
            platform::edge::Edge::PostFriend,
            &config.friend_grpc_endpoint,
            &config.outbound_identity,
            config.tls_material.as_ref(),
        )?,
        Probe::grpc(
            platform::edge::Edge::PostNotification,
            &config.notification_grpc_endpoint,
            &config.outbound_identity,
            config.tls_material.as_ref(),
        )?,
        // Request paths that read Friend or Profile first exchange authority at
        // the Gateway. The probe asks the Gateway's own gRPC health only, which
        // does not recurse into the Gateway's dependencies, so the two services'
        // readiness checks do not wait on each other.
        Probe::grpc(
            platform::edge::Edge::PostGateway,
            &config.gateway_grpc_endpoint,
            &config.outbound_identity,
            config.tls_material.as_ref(),
        )?,
    ])?;
    let health = health_router(readiness.clone());
    // The Gateway exchange fetches owner evidence here, independently of the
    // presenter. The lookup reads already published witness stages only.
    let resource_authority =
        post_service::adapters::r#in::grpc::resource_authority::PostResourceAuthorityGrpcServer::new(
            Arc::new(
                post_service::adapters::out::persistence::authority_witness_repository::PgAuthorityWitnessRepository::new(
                    pool.clone(),
                ),
            ),
            config.actor_delegation.clone(),
            config.resource_evidence_signer.clone(),
        )?;
    let state = Arc::new(build_state(&config, pool).await?);
    let (http_listener, metrics_listener) =
        http::bind_listeners(config.http_addr, config.metrics_addr).await?;
    let server = PostGrpcServer::new(state.clone());
    let application = http::router(state.clone(), config.runtime_mode);
    let metrics = http::metrics_router(state.clone()).merge(health);
    let outbox_worker = state.outbox_worker.clone();
    let retention_worker = state.retention_purge_worker.clone();
    let fanout_worker = state.timeline_fanout_worker.clone();
    let recommendation_worker = state.recommendation_retention_worker.clone();
    let min_event_age =
        chrono::Duration::from_std(Duration::from_millis(config.outbox_worker_min_event_age_ms))?;
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
            // Internal RPC is mutually authenticated when TLS material is
            // configured. The absence of material is only reachable outside
            // production, because config loading fails closed in production.
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
                    &post_service::adapters::r#in::grpc::method_policy::POST_METHOD_POLICY,
                    PostServiceServer::new(server),
                    config.caller_registry.clone(),
                    config.actor_delegation.clone(),
                )?)
                .add_service(platform::grpc_method_policy::guard_without_delegation(
                    &post_service::adapters::r#in::grpc::resource_authority::POST_RESOURCE_AUTHORITY_POLICY,
                    post_service::adapters::r#in::grpc::resource_authority::proto::post_resource_authority_service_server::PostResourceAuthorityServiceServer::new(
                        resource_authority,
                    ),
                    config.caller_registry.clone(),
                )?)
                .add_service(platform::grpc_method_policy::guard_without_delegation(
                    &post_service::adapters::r#in::grpc::concert_feed::POST_CONCERT_FEED_POLICY,
                    post_service::adapters::r#in::grpc::concert_feed::proto::post_concert_feed_service_server::PostConcertFeedServiceServer::new(
                        post_service::adapters::r#in::grpc::concert_feed::PostConcertFeedGrpcServer,
                    ),
                    config.caller_registry.clone(),
                )?)
                .add_optional_service(concert_read)
                .add_optional_service(concert_settlement)
                .add_optional_service(concert_command)
                .serve_with_shutdown(config.grpc_addr, platform::shutdown::global().cancelled())
                .await?;
            Ok::<(), anyhow::Error>(())
        };
        let outbox = async move {
            if config.outbox_worker_enabled {
                let timeline = outbox_worker.clone();
                let cache = outbox_worker.clone();
                let internal = outbox_worker.clone();
                let external = outbox_worker;
                let interval = Duration::from_millis(config.outbox_worker_interval_ms);
                let limit = config.outbox_worker_batch_size;
                use post_service::domain::post::DeliveryTarget;
                tokio::try_join!(
                    platform::shutdown::periodic("post-outbox-timeline", interval, || timeline
                        .tick_target(
                            DeliveryTarget::Timeline,
                            chrono::Utc::now() - min_event_age,
                            limit
                        )),
                    platform::shutdown::periodic("post-outbox-cache", interval, || cache
                        .tick_target(
                            DeliveryTarget::Cache,
                            chrono::Utc::now() - min_event_age,
                            limit
                        )),
                    platform::shutdown::periodic("post-outbox-internal", interval, || internal
                        .tick_target(
                            DeliveryTarget::Internal,
                            chrono::Utc::now() - min_event_age,
                            limit
                        )),
                    platform::shutdown::periodic("post-outbox-external", interval, || external
                        .tick_target(
                            DeliveryTarget::External,
                            chrono::Utc::now() - min_event_age,
                            limit
                        )),
                )?;
                Ok(())
            } else {
                platform::shutdown::idle().await
            }
        };
        let retention = async move {
            if config.retention_purge_worker_enabled {
                platform::shutdown::periodic(
                    "post-retention",
                    Duration::from_millis(config.retention_purge_worker_interval_ms),
                    || retention_worker.tick(),
                )
                .await
            } else {
                platform::shutdown::idle().await
            }
        };
        let fanout = async move {
            if config.timeline_fanout_worker_enabled {
                platform::shutdown::periodic(
                    "timeline-fanout",
                    Duration::from_millis(config.timeline_fanout_worker_interval_ms),
                    || fanout_worker.tick(),
                )
                .await
            } else {
                platform::shutdown::idle().await
            }
        };
        let recommendation_retention = async move {
            if config.feed_retention_worker_enabled {
                platform::shutdown::periodic(
                    "feed-retention",
                    Duration::from_millis(config.feed_retention_worker_interval_ms)
                        .max(Duration::from_secs(1)),
                    || recommendation_worker.tick(chrono::Utc::now()),
                )
                .await
            } else {
                platform::shutdown::idle().await
            }
        };
        tokio::try_join!(
            http,
            grpc,
            outbox,
            retention,
            fanout,
            recommendation_retention
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
