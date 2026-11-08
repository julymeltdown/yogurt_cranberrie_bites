use axum::{extract::State, http::StatusCode, routing::get, Router};
use platform::readiness::{Probe, Readiness, DATABASE_PROBE_TIMEOUT};
use profile_service::adapters::r#in::grpc::server::{
    proto::profile_service_server::ProfileServiceServer, ProfileGrpcServer,
};
use profile_service::adapters::r#in::http;
use profile_service::config::{build_state, connect_pool, AppConfig};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if migration_runtime::handle_command(
        std::env::args_os().skip(1),
        "PROFILE_DATABASE_URL",
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations"),
    )
    .await?
    {
        return Ok(());
    }

    let telemetry = platform::telemetry::init("profile-service")?;
    platform::shutdown::install_signal_handlers()?;
    let config = AppConfig::load()?;
    let pool = connect_pool(&config).await?;
    let readiness = readiness(pool.clone());
    let health = health_router(readiness.clone());
    // Post confirms a post author through this minimal projection, presenting a
    // Gateway-issued child bound to that author. It exposes no profile content.
    let author_authority =
        profile_service::adapters::r#in::grpc::author_authority::ProfileAuthorAuthorityGrpcServer::new(
            Arc::new(
                profile_service::adapters::out::persistence::profile_repository::PgProfileRepository::new(
                    pool.clone(),
                ),
            ),
        );
    let world_profile_server =
        profile_service::adapters::r#in::grpc::world_identity::WorldProfileServer::from_pool(
            pool.clone(),
        );
    let public_profile_server = profile_service::adapters::r#in::grpc::concert_public_profile::PublicProfileServer::from_pool(pool.clone());
    let production = platform::config::runtime_mode_is_production(
        &std::env::var("PROFILE_RUNTIME_MODE").unwrap_or_else(|_| "production".into()),
        "PROFILE_RUNTIME_MODE",
    )?;
    let self_profile =
        profile_service::concert_self_profile::server::SelfProfileService::configured(
            pool.clone(),
            &config.actor_delegation,
            production,
            config.tls_material.as_ref(),
        )?;
    let self_recovery = self_profile.as_ref().map(|(service, _)| service.clone());
    let self_profile_servers = self_profile
        .map(|(service, verifier)| {
            Ok::<_, anyhow::Error>((
                platform::grpc_method_policy::guard(
                    &profile_service::concert_self_profile::server::SELF_POLICY,
                    service.clone().server(),
                    config.caller_registry.clone(),
                    verifier,
                )?,
                platform::grpc_method_policy::guard_without_delegation(
                    &profile_service::concert_self_profile::settlement::SETTLEMENT_POLICY,
                    service.settlement_server(),
                    config.caller_registry.clone(),
                )?,
            ))
        })
        .transpose()?;
    let (self_profile_server, self_settlement_server) = match self_profile_servers {
        Some((profile, settlement)) => (Some(profile), Some(settlement)),
        None => (None, None),
    };
    let state = Arc::new(build_state(&config, pool.clone()).await?);
    // The profile product transport serves profile-owned operations that carry
    // no event. It is registered beside the Profile service, not inside it,
    // because its request shape and its authority binding are its own.
    let profile_product = profile_service::profile_product::runtime(
        pool.clone(),
        config.caller_registry.clone(),
        config.actor_delegation.clone(),
    )?;
    let world_profile_verifier = platform::actor_delegation::relationship_read::RelationshipReadVerifier::configured_from_env(
        platform::grpc_auth::Workload::Profile, &config.actor_delegation,
    )?;
    let public_profile = world_profile_verifier
        .clone()
        .map(|verifier| {
            use profile_service::adapters::r#in::grpc::concert_public_profile::{
                proto::concert_public_profile_service_server::ConcertPublicProfileServiceServer,
                PUBLIC_PROFILE_POLICY,
            };
            platform::grpc_method_policy::guard(
                &PUBLIC_PROFILE_POLICY,
                ConcertPublicProfileServiceServer::new(public_profile_server),
                config.caller_registry.clone(),
                verifier.into_inner(),
            )
        })
        .transpose()?;
    let world_profile = world_profile_verifier
        .map(|verifier| {
            use profile_service::adapters::r#in::grpc::world_identity::{
                proto::concert_world_profile_service_server::ConcertWorldProfileServiceServer,
                WORLD_PROFILE_POLICY,
            };
            platform::grpc_method_policy::guard(
                &WORLD_PROFILE_POLICY,
                ConcertWorldProfileServiceServer::new(world_profile_server),
                config.caller_registry.clone(),
                verifier.into_inner(),
            )
        })
        .transpose()?;
    let (http_listener, metrics_listener) =
        http::bind_listeners(config.http_addr, config.metrics_addr).await?;
    let worker = state.retention_purge_worker.clone();
    let mutation_receipt_retention_worker = state.mutation_receipt_retention_worker.clone();
    let server = ProfileGrpcServer::new(state.clone());
    let application = http::router();
    let metrics = http::metrics_router(state.metrics.clone()).merge(health);
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
                .add_optional_service(world_profile)
                .add_optional_service(public_profile)
                .add_optional_service(self_profile_server)
                .add_optional_service(self_settlement_server)
                .add_service(platform::grpc_method_policy::guard(
                    &profile_service::adapters::r#in::grpc::method_policy::PROFILE_METHOD_POLICY,
                    ProfileServiceServer::new(server),
                    config.caller_registry.clone(),
                    config.actor_delegation.clone(),
                )?)
                .add_service(profile_product.dispatch)
                .add_service(profile_product.settlement)
                .add_service(platform::grpc_method_policy::guard_with_forwarding(
                    &profile_service::adapters::r#in::grpc::author_authority::PROFILE_AUTHOR_POLICY,
                    profile_service::adapters::r#in::grpc::author_authority::PROFILE_AUTHOR_FORWARDING,
                    profile_service::adapters::r#in::grpc::author_authority::proto::profile_author_authority_service_server::ProfileAuthorAuthorityServiceServer::new(
                        author_authority,
                    ),
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
                    "profile-retention",
                    Duration::from_millis(config.retention_purge_worker_interval_ms),
                    || worker.tick(),
                )
                .await
            } else {
                platform::shutdown::idle().await
            }
        };
        let mutation_receipt_retention = async move {
            if config.mutation_receipt_retention_worker_enabled {
                platform::shutdown::periodic(
                    "profile-mutation-receipt-retention",
                    Duration::from_millis(config.mutation_receipt_retention_worker_interval_ms),
                    || mutation_receipt_retention_worker.tick(),
                )
                .await
            } else {
                platform::shutdown::idle().await
            }
        };
        let self_profile_recovery = async move {
            if let Some(service)=self_recovery {
                platform::shutdown::periodic(
                    "profile-self-recovery",Duration::from_secs(5),||service.reconcile_pending(),
                ).await
            } else {
                platform::shutdown::idle().await
            }
        };
        tokio::try_join!(http, grpc, retention, mutation_receipt_retention,self_profile_recovery)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_routes_report_real_postgres_outage_without_failing_liveness(
    ) -> anyhow::Result<()> {
        let database = test_support::postgres::start_postgres().await?;
        let host = database.get_host().await?;
        let port = database.get_host_port_ipv4(5432).await?;
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&format!(
                "postgres://postgres:postgres@{host}:{port}/postgres"
            ))
            .await?;
        let app = health_router(readiness(pool.clone()));
        let response = app
            .clone()
            .oneshot(Request::builder().uri("/readyz").body(Body::empty())?)
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let body: serde_json::Value =
            serde_json::from_slice(&response.into_body().collect().await?.to_bytes())?;
        assert_eq!(body["checks"]["database"]["status"], "up");

        database.stop().await?;
        tokio::time::sleep(platform::readiness::PROBE_CACHE_TTL + Duration::from_millis(10)).await;
        let response = app
            .clone()
            .oneshot(Request::builder().uri("/readyz").body(Body::empty())?)
            .await?;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body: serde_json::Value =
            serde_json::from_slice(&response.into_body().collect().await?.to_bytes())?;
        assert_eq!(body["ready"], false);
        assert_ne!(body["checks"]["database"]["status"], "up");
        let response = app
            .oneshot(Request::builder().uri("/livez").body(Body::empty())?)
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        pool.close().await;
        Ok(())
    }
}
