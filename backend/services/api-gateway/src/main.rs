use anyhow::Context;
use api_gateway::adapters::out::jwt::JwtVerifier;
use api_gateway::adapters::r#in::http;
use api_gateway::adapters::r#in::http::client_identity::TrustedProxies;
use api_gateway::adapters::r#in::http::request_journal;
use api_gateway::adapters::r#in::http::server::{
    bind_http_listeners, serve_http_listeners, with_transport_layers,
};
use api_gateway::config::{
    build_state, connect_pool, connect_quota_pool, AppConfig, DatabasePoolRole, RuntimeMode,
};
use axum::{extract::State, http::StatusCode, routing::get, Router};
use platform::readiness::{Probe, Readiness, DATABASE_PROBE_TIMEOUT};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

/// Deadline for the signing-key readiness check. It reads in-memory state, so
/// anything slower than this means the process itself is unwell.
const SIGNING_KEYS_PROBE_TIMEOUT: Duration = Duration::from_millis(50);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if migration_runtime::handle_command(
        std::env::args_os().skip(1),
        "GATEWAY_DATABASE_URL",
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations"),
    )
    .await?
    {
        return Ok(());
    }

    let telemetry = platform::telemetry::init("api-gateway")?;
    platform::shutdown::install_signal_handlers()?;
    let config = AppConfig::load()?;
    let trusted_proxies = TrustedProxies::from_env()?;
    let pool = connect_pool(&config).await?;
    let mut state = build_state(&config, pool.clone())
        .await
        .context("Gateway startup: build application state")?;
    state
        .metrics
        .register_database_pool(DatabasePoolRole::Main, pool.clone());
    let readiness = readiness(pool).with_dependencies(vec![
        signing_keys_probe(state.jwt_verifier.clone()),
        Probe::grpc(
            platform::edge::Edge::GatewayAuth,
            &config.auth_grpc_endpoint,
            &config.outbound_identity,
            config.tls_material.as_ref(),
        )?,
        Probe::grpc(
            platform::edge::Edge::GatewayPost,
            &config.post_grpc_endpoint,
            &config.outbound_identity,
            config.tls_material.as_ref(),
        )?,
        Probe::grpc(
            platform::edge::Edge::GatewayProfile,
            &config.profile_grpc_endpoint,
            &config.outbound_identity,
            config.tls_material.as_ref(),
        )?,
        Probe::grpc(
            platform::edge::Edge::GatewayFriend,
            &config.friend_grpc_endpoint,
            &config.outbound_identity,
            config.tls_material.as_ref(),
        )?,
        Probe::grpc(
            platform::edge::Edge::GatewayNotification,
            &config.notification_grpc_endpoint,
            &config.outbound_identity,
            config.tls_material.as_ref(),
        )?,
    ])?;
    let health = health_router(readiness.clone());
    // Security control traffic cannot exhaust the telemetry/business pool. The
    // journal pool serves only request-path journal work so a request can always
    // record its outcome within its own budget.
    let audit_pool = PgPoolOptions::new()
        .max_connections(config.journal_pool_max_connections)
        .acquire_timeout(Duration::from_millis(200))
        .connect_with(migration_runtime::serving_options(
            sqlx::postgres::PgConnectOptions::from_str(&config.database_url)
                .context("Gateway startup: parse journal database options")?
                .application_name("gateway-journal"),
            config.database_migration_mode,
        ))
        .await
        .context("Gateway startup: connect journal database pool")?;
    state.journal = api_gateway::config::journal_runtime_ports(&audit_pool, &state.metrics);
    state
        .metrics
        .register_database_pool(DatabasePoolRole::Journal, audit_pool.clone());
    // Durable-operation polling and stale-operation maintenance share a separate
    // pool. They tolerate waiting, so their acquire limit is longer, and they can
    // no longer occupy the connections a request completion depends on.
    let background_pool = PgPoolOptions::new()
        .max_connections(config.background_pool_max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(migration_runtime::serving_options(
            sqlx::postgres::PgConnectOptions::from_str(&config.database_url)
                .context("Gateway startup: parse background database options")?
                .application_name("gateway-background"),
            config.database_migration_mode,
        ))
        .await
        .context("Gateway startup: connect background database pool")?;
    state
        .metrics
        .register_database_pool(DatabasePoolRole::Background, background_pool.clone());
    // Background consumers receive neutral ports bound to the background pool.
    let background_journal =
        api_gateway::config::journal_runtime_ports(&background_pool, &state.metrics);
    if matches!(config.runtime_mode, RuntimeMode::Production) {
        let quota_pool = connect_quota_pool(&config).await?;
        state
            .metrics
            .register_database_pool(DatabasePoolRole::Quota, quota_pool.clone());
        state.rate_limiter = state.rate_limiter.with_database(quota_pool);
    }
    // Refreshes the issuer's signing keys on a schedule. Absent when keys come
    // only from configuration (GATEWAY_JWT_KEYS_REFRESH_SECONDS=0).
    let signing_key_refresh = state.signing_keys.clone();
    // The internal listener serves only the authorization exchange to Post. It
    // reuses the internal CA and caller registry of every other gRPC owner.
    let exchange_service =
        api_gateway::config::authority_exchange_service(&config, &state.metrics)?;
    let grpc_addr = config.grpc_addr;
    let grpc_tls = config.tls_material.clone();
    let grpc_registry = config.caller_registry.clone();
    let grpc_readiness = readiness.clone();
    let state = Arc::new(state);
    let metrics_router = http::metrics_router(state.metrics.clone()).merge(health);
    // The maintenance loop reports the reconciliation backlog, which is a gauge on
    // the same registry the /metrics listener serves.
    let maintenance_metrics = state.metrics.clone();
    let application_router =
        with_transport_layers(http::router(state.clone()), &config, trusted_proxies);
    let listeners = bind_http_listeners(config.http_addr, config.metrics_addr).await?;
    let async_worker_journal = background_journal.clone();
    let async_worker_state = state.clone();
    let async_worker_cipher = state.async_operation_cipher.clone();
    let async_operation_worker = async move {
        match async_worker_cipher {
            Some(cipher) => {
                http::operation_worker::run(async_worker_journal, cipher, async_worker_state).await
            }
            None => platform::shutdown::idle().await,
        }
    };
    let moderation_decision_service = state.moderation_service.clone();
    let owner_receipt_reconciler =
        http::owner_receipts::run(state.clone(), background_journal.clone());
    let moderation_decision_metrics = state.metrics.clone();
    let moderation_decision_worker = async move {
        http::moderation_decision_worker::run(
            moderation_decision_service,
            moderation_decision_metrics,
        )
        .await
    };
    let result = platform::shutdown::supervise(async move {
        let grpc = async move {
            let mut builder = tonic::transport::Server::builder()
                .layer(platform::correlation::CorrelationLayer)
                .layer(platform::call_deadline::CallDeadlineLayer)
                .layer(platform::trace_context::TraceContextLayer);
            if let Some(material) = grpc_tls.as_ref() {
                builder = builder
                    .tls_config(material.server_config()?)
                    .map_err(anyhow::Error::from)?;
            }
            builder
                .add_service(grpc_readiness.grpc_service(grpc_registry)?)
                .add_service(exchange_service)
                .serve_with_shutdown(grpc_addr, platform::shutdown::global().cancelled())
                .await?;
            Ok::<(), anyhow::Error>(())
        };
        tokio::try_join!(
            serve_http_listeners(listeners, application_router, metrics_router),
            grpc,
            request_journal::maintenance_with_retention(
                background_journal,
                request_journal::RetentionWindows {
                    telemetry_days: config.telemetry_retention_days,
                    audit_days: config.audit_retention_days,
                },
                Some(Arc::new(maintenance_metrics)),
            ),
            async move {
                match signing_key_refresh {
                    Some(refresher) => refresher.run().await,
                    None => Ok(()),
                }
            },
            async_operation_worker,
            moderation_decision_worker,
            owner_receipt_reconciler,
        )?;
        request_journal::drain().await
    })
    .await;
    telemetry.shutdown();
    result
}

/// Ready only while at least one signing key is installed.
///
/// A Gateway without keys rejects every authenticated request, so it must not
/// receive traffic. Once installed, keys are kept through failed refreshes,
/// so an issuer outage does not make this check fail.
fn signing_keys_probe(verifier: JwtVerifier) -> Probe {
    Probe::new(
        "signing-keys",
        true,
        SIGNING_KEYS_PROBE_TIMEOUT,
        move || {
            let verifier = verifier.clone();
            async move { verifier.key_count() > 0 }
        },
    )
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
