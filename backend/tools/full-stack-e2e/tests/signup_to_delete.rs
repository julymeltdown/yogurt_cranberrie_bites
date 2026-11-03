use anyhow::{Context, Result};
use api_gateway::adapters::r#in::http::{
    self, client_identity::TrustedProxies, server::with_transport_layers,
};
use auth_service::adapters::r#in::{
    grpc::server::{proto::auth_service_server::AuthServiceServer, AuthGrpcServer},
    messaging::mail_outbox_worker::{EmailOutboxWorker, MailDestination},
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use chrono::Utc;
use friend_service::adapters::r#in::grpc::server::{
    proto::friend_service_server::FriendServiceServer, FriendGrpcServer,
};
use notification_service::adapters::r#in::grpc::server::{
    proto::notification_service_server::NotificationServiceServer, NotificationGrpcServer,
};
use platform::grpc_method_policy::{guard, guard_with_forwarding, guard_without_delegation};
use post_service::adapters::r#in::grpc::server::{
    proto::post_service_server::PostServiceServer, PostGrpcServer,
};
use profile_service::adapters::r#in::grpc::server::{
    proto::profile_service_server::ProfileServiceServer, ProfileGrpcServer,
};
use reqwest::Method;
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{
    collections::HashSet,
    io::Cursor,
    net::SocketAddr,
    process::Command,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use test_support::{postgres, redis};
use tokio::{
    net::TcpListener,
    sync::{oneshot, Barrier, Mutex},
    task::{JoinHandle, JoinSet},
};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use uuid::Uuid;

type Shutdown = oneshot::Sender<()>;
type ServerTask = JoinHandle<Result<()>>;
type MailInbox = Arc<Mutex<Vec<Value>>>;
const E17_5_GATEWAY_READ_BUDGET_PER_MINUTE: u32 = 10_000;

#[derive(Clone)]
struct MailSinkState {
    inbox: MailInbox,
    failures_remaining: Arc<AtomicUsize>,
    processed_keys: Arc<Mutex<HashSet<String>>>,
    attempted_keys: Arc<Mutex<Vec<(String, String)>>>,
}

struct SignedIn {
    user_id: Uuid,
    access_token: String,
    refresh_cookie: String,
}

struct RefreshedSession {
    access_token: String,
    refresh_cookie: String,
}

macro_rules! spawn_grpc_server {
    ($listener:ident, $service:expr $(, $more:expr)* $(,)?) => {{
        let address = $listener.local_addr()?;
        let (shutdown, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            Server::builder()
                .layer(platform::correlation::CorrelationLayer)
                .add_service($service)
                $(.add_service($more))*
                .serve_with_incoming_shutdown(TcpListenerStream::new($listener), async {
                    let _ = stopped.await;
                })
                .await?;
            Ok::<(), anyhow::Error>(())
        });
        (address, shutdown, task)
    }};
}

#[test]
fn signup_sessions_social_features_privacy_and_deletion_cross_real_services() -> Result<()> {
    // This integration-test binary contains only this test. Load the real service
    // configs in their documented non-production mode and give each workload its
    // own credential before any Tokio or database threads are started.
    init_test_tracing();
    configure_parity_environment();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(4)
        .build()?
        .block_on(run_journey())
}

fn init_test_tracing() {
    if std::env::var_os("E2E_TRACE").is_none() {
        return;
    }
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("api_gateway=error"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

fn configure_parity_environment() {
    for (name, value) in [
        ("AUTH_RUNTIME_MODE", "parity"),
        ("PROFILE_RUNTIME_MODE", "parity"),
        ("FRIEND_RUNTIME_MODE", "parity"),
        ("POST_RUNTIME_MODE", "parity"),
        ("NOTIFICATION_RUNTIME_MODE", "parity"),
        ("GATEWAY_RUNTIME_MODE", "parity"),
        ("GATEWAY_SERVICE_CREDENTIAL", "e2e-gateway-credential"),
        ("AUTH_SERVICE_CREDENTIAL", "e2e-auth-credential"),
        ("PROFILE_SERVICE_CREDENTIAL", "e2e-profile-credential"),
        ("FRIEND_SERVICE_CREDENTIAL", "e2e-friend-credential"),
        ("POST_SERVICE_CREDENTIAL", "e2e-post-credential"),
        (
            "NOTIFICATION_SERVICE_CREDENTIAL",
            "e2e-notification-credential",
        ),
    ] {
        std::env::set_var(name, value);
    }
    // The benchmark issues 6,144 measured reads plus warmups. Keep this
    // test-only network budget above that sweep so the measurement describes
    // request-path capacity rather than the separate default 600/minute quota.
    std::env::set_var(
        "GATEWAY_RATE_LIMIT_PER_MINUTE",
        E17_5_GATEWAY_READ_BUDGET_PER_MINUTE.to_string(),
    );
}

async fn run_journey() -> Result<()> {
    // E2E_NATIVE_POSTGRES_ADDR and E2E_NATIVE_REDIS_URL select an owned local
    // PostgreSQL cluster (trust or postgres/postgres login, empty of the e2e
    // databases) and Redis server instead of containers. Set both or neither.
    let mut _containers = None;
    let (pg_host, pg_port, redis_url) = match (
        std::env::var("E2E_NATIVE_POSTGRES_ADDR").ok(),
        std::env::var("E2E_NATIVE_REDIS_URL").ok(),
    ) {
        (Some(address), Some(redis_url)) => {
            let address: SocketAddr = address
                .parse()
                .context("E2E_NATIVE_POSTGRES_ADDR must be host:port")?;
            (address.ip().to_string(), address.port(), redis_url)
        }
        (None, None) => {
            let postgres = postgres::start_postgres().await?;
            let redis_container = redis::start_redis().await?;
            let pg_host = postgres.get_host().await?.to_string();
            let pg_port = postgres.get_host_port_ipv4(5432).await?;
            let redis_host = redis_container.get_host().await?;
            let redis_port = redis_container.get_host_port_ipv4(6379).await?;
            _containers = Some((postgres, redis_container));
            (
                pg_host,
                pg_port,
                format!("redis://{redis_host}:{redis_port}/"),
            )
        }
        _ => {
            anyhow::bail!("set both E2E_NATIVE_POSTGRES_ADDR and E2E_NATIVE_REDIS_URL, or neither")
        }
    };
    let native_dependencies = _containers.is_none();
    let admin_url = format!("postgres://postgres:postgres@{pg_host}:{pg_port}/postgres");
    create_service_databases(&admin_url).await?;

    let mut profile_config = profile_service::config::AppConfig::load()?;
    profile_config.database_url = database_url(&pg_host, pg_port, "profile_e2e");
    profile_config.database_pool_max_connections = 4;
    let profile_pool = profile_service::config::connect_pool(&profile_config).await?;
    let profile_state = Arc::new(
        profile_service::config::build_state(&profile_config, profile_pool.clone()).await?,
    );
    let profile_listener = TcpListener::bind("127.0.0.1:0").await?;
    // Every owner is registered exactly as its production binary registers it:
    // default-deny method policy, actor delegation, and child-proof forwarding.
    let profile_service = guard(
        &profile_service::adapters::r#in::grpc::method_policy::PROFILE_METHOD_POLICY,
        ProfileServiceServer::new(ProfileGrpcServer::new(profile_state)),
        profile_config.caller_registry.clone(),
        profile_config.actor_delegation.clone(),
    )?;
    let profile_author_service = guard_with_forwarding(
        &profile_service::adapters::r#in::grpc::author_authority::PROFILE_AUTHOR_POLICY,
        profile_service::adapters::r#in::grpc::author_authority::PROFILE_AUTHOR_FORWARDING,
        profile_service::adapters::r#in::grpc::author_authority::proto::profile_author_authority_service_server::ProfileAuthorAuthorityServiceServer::new(
            profile_service::adapters::r#in::grpc::author_authority::ProfileAuthorAuthorityGrpcServer::new(
                Arc::new(
                    profile_service::adapters::out::persistence::profile_repository::PgProfileRepository::new(
                        profile_pool.clone(),
                    ),
                ),
            ),
        ),
        profile_config.caller_registry.clone(),
        profile_config.actor_delegation.clone(),
    )?;
    let (profile_addr, profile_stop, profile_task) =
        spawn_grpc_server!(profile_listener, profile_service, profile_author_service);

    let mut notification_config = notification_service::config::AppConfig::load()?;
    notification_config.database_url = database_url(&pg_host, pg_port, "notification_e2e");
    notification_config.database_pool_max_connections = 4;
    let notification_pool =
        notification_service::config::connect_pool(&notification_config).await?;
    let notification_state = Arc::new(
        notification_service::config::build_state(&notification_config, notification_pool.clone())
            .await?,
    );
    let notification_listener = TcpListener::bind("127.0.0.1:0").await?;
    let notification_service = guard(
        &notification_service::adapters::r#in::grpc::method_policy::NOTIFICATION_METHOD_POLICY,
        NotificationServiceServer::new(NotificationGrpcServer::new(notification_state)),
        notification_config.caller_registry.clone(),
        notification_config.actor_delegation.clone(),
    )?;
    let (notification_addr, notification_stop, notification_task) =
        spawn_grpc_server!(notification_listener, notification_service);

    let mut friend_config = friend_service::config::AppConfig::load()?;
    friend_config.database_url = database_url(&pg_host, pg_port, "friend_e2e");
    friend_config.database_pool_max_connections = 4;
    friend_config.profile_grpc_endpoint = grpc_endpoint(profile_addr);
    let friend_pool = friend_service::config::connect_pool(&friend_config).await?;
    let friend_state =
        Arc::new(friend_service::config::build_state(&friend_config, friend_pool.clone()).await?);
    let friend_listener = TcpListener::bind("127.0.0.1:0").await?;
    let friend_service = guard_with_forwarding(
        &friend_service::adapters::r#in::grpc::method_policy::FRIEND_METHOD_POLICY,
        friend_service::adapters::r#in::grpc::method_policy::FRIEND_FORWARDING,
        FriendServiceServer::new(FriendGrpcServer::new(friend_state)),
        friend_config.caller_registry.clone(),
        friend_config.actor_delegation.clone(),
    )?;
    let (friend_addr, friend_stop, friend_task) =
        spawn_grpc_server!(friend_listener, friend_service);

    let mut post_config = post_service::config::AppConfig::load()?;
    post_config.database_url = database_url(&pg_host, pg_port, "post_e2e");
    post_config.database_pool_max_connections = 4;
    post_config.redis_url = redis_url;
    post_config.profile_grpc_endpoint = grpc_endpoint(profile_addr);
    post_config.friend_grpc_endpoint = grpc_endpoint(friend_addr);
    post_config.notification_grpc_endpoint = grpc_endpoint(notification_addr);
    // Post exchanges admitted authority at the Gateway's internal listener. Bind
    // it now so Post knows the address; the issuer is served once Post exists.
    let gateway_grpc_listener = TcpListener::bind("127.0.0.1:0").await?;
    post_config.gateway_grpc_endpoint = grpc_endpoint(gateway_grpc_listener.local_addr()?);
    let post_pool = post_service::config::connect_pool(&post_config).await?;
    let post_state =
        Arc::new(post_service::config::build_state(&post_config, post_pool.clone()).await?);
    let post_listener = TcpListener::bind("127.0.0.1:0").await?;
    let post_service = guard(
        &post_service::adapters::r#in::grpc::method_policy::POST_METHOD_POLICY,
        PostServiceServer::new(PostGrpcServer::new(post_state.clone())),
        post_config.caller_registry.clone(),
        post_config.actor_delegation.clone(),
    )?;
    let post_resource_service = guard_without_delegation(
        &post_service::adapters::r#in::grpc::resource_authority::POST_RESOURCE_AUTHORITY_POLICY,
        post_service::adapters::r#in::grpc::resource_authority::proto::post_resource_authority_service_server::PostResourceAuthorityServiceServer::new(
            post_service::adapters::r#in::grpc::resource_authority::PostResourceAuthorityGrpcServer::new(
                Arc::new(
                    post_service::adapters::out::persistence::authority_witness_repository::PgAuthorityWitnessRepository::new(
                        post_pool.clone(),
                    ),
                ),
                post_config.actor_delegation.clone(),
                post_config.resource_evidence_signer.clone(),
            )?,
        ),
        post_config.caller_registry.clone(),
    )?;
    let (post_addr, post_stop, post_task) =
        spawn_grpc_server!(post_listener, post_service, post_resource_service);

    let mut auth_config = auth_service::config::AppConfig::load()?;
    auth_config.database_url = database_url(&pg_host, pg_port, "auth_e2e");
    auth_config.database_pool_max_connections = 4;
    auth_config.profile_grpc_endpoint = grpc_endpoint(profile_addr);
    auth_config.friend_grpc_endpoint = grpc_endpoint(friend_addr);
    auth_config.post_grpc_endpoint = grpc_endpoint(post_addr);
    auth_config.notification_grpc_endpoint = grpc_endpoint(notification_addr);
    let auth_pool = auth_service::config::connect_pool(&auth_config).await?;
    let auth_state =
        Arc::new(auth_service::config::build_state(&auth_config, auth_pool.clone()).await?);
    let jwks_json =
        auth_service::adapters::r#in::jwks::public_jwks(&auth_state.login.jwk_set()).to_string();
    let auth_listener = TcpListener::bind("127.0.0.1:0").await?;
    let auth_service = guard(
        &auth_service::adapters::r#in::grpc::method_policy::AUTH_METHOD_POLICY,
        AuthServiceServer::new(AuthGrpcServer::new(auth_state.clone())),
        auth_config.caller_registry.clone(),
        auth_config.actor_delegation.clone(),
    )?;
    let (auth_addr, auth_stop, auth_task) = spawn_grpc_server!(auth_listener, auth_service);

    // The mail sink is the sole provider double. Signup still writes the real
    // transactional outbox, and the production mail worker delivers to this
    // local HTTP receiver before the code is consumed by Gateway verification.
    let inbox: MailInbox = Arc::new(Mutex::new(Vec::new()));
    // The first provider request persists its effect but answers 503, modeling
    // an ambiguous acknowledgement that the outbox must safely replay.
    let mail_sink = MailSinkState {
        inbox: inbox.clone(),
        failures_remaining: Arc::new(AtomicUsize::new(1)),
        processed_keys: Arc::new(Mutex::new(HashSet::new())),
        attempted_keys: Arc::new(Mutex::new(Vec::new())),
    };
    let (mail_addr, mail_stop, mail_task) = start_mail_sink(mail_sink.clone()).await?;
    let mail_url = format!("http://{mail_addr}/verification");
    let mail_worker = EmailOutboxWorker::new(
        auth_pool.clone(),
        Some(MailDestination::new(&mail_url, "e2e-mail-token", false)?),
        None,
    )?;

    let mut gateway_config = api_gateway::config::AppConfig::load()?;
    gateway_config.database_url = database_url(&pg_host, pg_port, "gateway_e2e");
    gateway_config.database_pool_max_connections = 4;
    gateway_config.auth_grpc_endpoint = grpc_endpoint(auth_addr);
    gateway_config.profile_grpc_endpoint = grpc_endpoint(profile_addr);
    gateway_config.friend_grpc_endpoint = grpc_endpoint(friend_addr);
    gateway_config.notification_grpc_endpoint = grpc_endpoint(notification_addr);
    gateway_config.post_grpc_endpoint = grpc_endpoint(post_addr);
    gateway_config.jwt_issuer = auth_config.issuer.clone();
    gateway_config.jwt_audience = auth_config.access_audience.clone();
    gateway_config.jwt_jwks_json = jwks_json;
    gateway_config.jwt_keys_refresh_seconds = 0;
    gateway_config.validate()?;
    let gateway_pool = api_gateway::config::connect_pool(&gateway_config).await?;
    let gateway_state =
        Arc::new(api_gateway::config::build_state(&gateway_config, gateway_pool.clone()).await?);
    let gateway_exchange_service =
        api_gateway::config::authority_exchange_service(&gateway_config, &gateway_state.metrics)?;
    let (_, gateway_grpc_stop, gateway_grpc_task) =
        spawn_grpc_server!(gateway_grpc_listener, gateway_exchange_service);
    let gateway_router = with_transport_layers(
        http::router(gateway_state.clone()),
        &gateway_config,
        TrustedProxies::default(),
    );
    let gateway_listener = TcpListener::bind("127.0.0.1:0").await?;
    let gateway_addr = gateway_listener.local_addr()?;
    let (gateway_stop, gateway_stopped) = oneshot::channel();
    let gateway_task = tokio::spawn(async move {
        axum::serve(gateway_listener, gateway_router)
            .with_graceful_shutdown(async {
                let _ = gateway_stopped.await;
            })
            .await?;
        Ok::<(), anyhow::Error>(())
    });
    let moderation_worker_service = gateway_state.moderation_service.clone();
    let moderation_worker_metrics = gateway_state.metrics.clone();
    let mut moderation_worker_task = tokio::spawn(async move {
        api_gateway::adapters::r#in::http::moderation_decision_worker::run(
            moderation_worker_service,
            moderation_worker_metrics,
        )
        .await
    });

    // Service tasks are retained and drained at the end of the happy path.
    let stop_signals: Vec<Shutdown> = vec![
        profile_stop,
        notification_stop,
        friend_stop,
        post_stop,
        auth_stop,
        gateway_stop,
        gateway_grpc_stop,
        mail_stop,
    ];
    let tasks: Vec<ServerTask> = vec![
        profile_task,
        notification_task,
        friend_task,
        post_task,
        auth_task,
        gateway_task,
        gateway_grpc_task,
        mail_task,
    ];

    let client = reqwest::Client::new();
    let base = format!("http://{gateway_addr}");
    let suffix = Uuid::new_v4().simple().to_string();
    let owner_email = format!("journey-owner-{suffix}@example.invalid");
    let target_email = format!("journey-target-{suffix}@example.invalid");

    let mut owner = signup_verify_login(
        &client,
        &base,
        &auth_pool,
        &mail_worker,
        &mail_sink,
        &inbox,
        &owner_email,
        true,
    )
    .await?;
    let mut target = signup_verify_login(
        &client,
        &base,
        &auth_pool,
        &mail_worker,
        &mail_sink,
        &inbox,
        &target_email,
        false,
    )
    .await?;

    let owner_username = format!("owner{}", &suffix[..12]);
    let (_, owner_profile) = json_request(
        &client,
        Method::PUT,
        &base,
        &format!("/api/profile/{}", owner.user_id),
        Some(&owner.access_token),
        Some(json!({
            "username": owner_username,
            "nickname": "Journey Owner",
            "avatarUrl": "https://example.invalid/owner.png",
            "bio": "Full-stack test owner"
        })),
    )
    .await?;
    anyhow::ensure!(
        owner_profile["username"] == owner_username,
        "owner profile write failed: {owner_profile}"
    );

    let target_username = format!("target{}", &suffix[..12]);
    let (_, profile) = json_request(
        &client,
        Method::PUT,
        &base,
        &format!("/api/profile/{}", target.user_id),
        Some(&target.access_token),
        Some(json!({
            "username": target_username,
            "nickname": "Journey Target",
            "avatarUrl": "https://example.invalid/target.png",
            "bio": "Full-stack test profile"
        })),
    )
    .await?;
    anyhow::ensure!(
        profile["username"] == target_username,
        "profile write was not visible: {profile}"
    );

    let (_, owner_profile_read) = json_request(
        &client,
        Method::GET,
        &base,
        &format!("/api/profile/{}", owner.user_id),
        Some(&owner.access_token),
        None,
    )
    .await?;
    anyhow::ensure!(
        owner_profile_read["username"] == owner_username,
        "owner profile read was not visible: {owner_profile_read}"
    );

    // Keep the normal workspace acceptance test focused on the cross-service
    // user journey. The repeated high-concurrency benchmark is opt-in and should
    // be run from an optimized profile on an otherwise stable local host.
    if matches!(std::env::var("E17_5_LOAD_SWEEP").as_deref(), Ok("1")) {
        let warmup_requests = 64;
        let measured_requests = 512;
        let repetitions_per_level = 3;
        let concurrency_levels = [1, 4, 16, 64];
        let rss_before_kib = current_process_rss_kib()?;
        let mut concurrency_runs =
            Vec::with_capacity(concurrency_levels.len() * repetitions_per_level);
        let mut representative_results = Value::Null;
        for concurrency in concurrency_levels {
            for repetition in 1..=repetitions_per_level {
                let (warmup_samples, _) = profile_read_wave(
                    client.clone(),
                    base.clone(),
                    owner.access_token.clone(),
                    owner.user_id,
                    warmup_requests,
                    concurrency,
                )
                .await
                .with_context(|| {
                    format!("profile-read warmup failed at concurrency {concurrency}, repetition {repetition}")
                })?;
                let (latencies, elapsed) = profile_read_wave(
                    client.clone(),
                    base.clone(),
                    owner.access_token.clone(),
                    owner.user_id,
                    measured_requests,
                    concurrency,
                )
                .await
                .with_context(|| {
                    format!("profile-read measurement failed at concurrency {concurrency}, repetition {repetition}")
                })?;
                let results = json!({
                    "http_status": 200,
                    "errors": 0,
                    "elapsed_ms": elapsed.as_secs_f64() * 1_000.0,
                    "throughput_requests_per_second": latencies.len() as f64 / elapsed.as_secs_f64(),
                    "p95_latency_ms": percentile_ms(&latencies, 95),
                    "p99_latency_ms": percentile_ms(&latencies, 99)
                });
                if concurrency == 16 && repetition == 1 {
                    representative_results = results.clone();
                }
                concurrency_runs.push(json!({
                    "concurrency": concurrency,
                    "repetition": repetition,
                    "warmup_requests": warmup_samples.len(),
                    "measured_requests": latencies.len(),
                    "results": results
                }));
            }
        }
        let rss_after_kib = current_process_rss_kib()?;
        let revision =
            command_text("git", &["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into());
        println!(
            "E17_5_LOAD_BASELINE={}",
            json!({
                "kind": "authenticated-profile-read-baseline",
                "timestamp_utc": Utc::now().to_rfc3339(),
                "run_id": Uuid::new_v4(),
                "dataset_id": format!("signup-to-delete-{suffix}"),
                "source_revision": revision,
                "workload": {
                    "route": "GET /api/profile/{user_id}",
                    "identity": "authenticated owner reading own profile",
                    "gateway_network_read_budget_per_minute": E17_5_GATEWAY_READ_BUDGET_PER_MINUTE,
                    "warmup_requests_per_run": warmup_requests,
                    "measured_requests_per_run": measured_requests,
                    "concurrency_levels": concurrency_levels,
                    "repetitions_per_level": repetitions_per_level,
                    "arrival_model": "closed-loop fixed workers",
                    "percentile_method": "nearest rank",
                    "representative_run": "concurrency 16, repetition 1"
                },
                "results": representative_results,
                "concurrency_runs": concurrency_runs,
                "memory": {
                    "scope": "full-stack-e2e test process RSS; excludes Docker and database/cache containers",
                    "rss_before_kib": rss_before_kib,
                    "rss_after_kib": rss_after_kib
                },
                "machine": machine_profile(),
                "topology": {
                    "services": "real HTTP/gRPC listeners hosted in one test process",
                    "internal_grpc_transport": "plaintext parity mode",
                    "postgres": if native_dependencies { "real owned native PostgreSQL cluster" } else { "real PostgreSQL test container" },
                    "redis": if native_dependencies { "real owned native Redis server" } else { "real Redis test container" },
                    "mail": "local idempotent HTTP fixture, outside measured route",
                    "kafka": "not used"
                }
            })
        );
    }

    let avatar_upload = upload_test_avatar(&client, &base, &target.access_token).await?;
    anyhow::ensure!(
        avatar_upload["avatarUrl"].as_str().is_some(),
        "avatar upload returned no URL: {avatar_upload}"
    );

    // Refresh rotates both credentials; all later target actions use the new
    // access token and refresh cookie.
    let refreshed = refresh_session(&client, &base, &target.refresh_cookie).await?;
    target.access_token = refreshed.access_token;
    target.refresh_cookie = refreshed.refresh_cookie;

    let (_, owner_follows_target) = json_request(
        &client,
        Method::POST,
        &base,
        "/api/follows",
        Some(&owner.access_token),
        Some(json!({"targetUserId": target.user_id.to_string()})),
    )
    .await?;
    anyhow::ensure!(
        owner_follows_target["following"] == true,
        "owner follow did not become active: {owner_follows_target}"
    );

    let (_, target_follows_owner) = json_request(
        &client,
        Method::POST,
        &base,
        "/api/follows",
        Some(&target.access_token),
        Some(json!({"targetUserId": owner.user_id.to_string()})),
    )
    .await?;
    anyhow::ensure!(
        target_follows_owner["following"] == true,
        "target follow did not become active: {target_follows_owner}"
    );

    let (avatar_content_type, avatar_bytes) =
        fetch_avatar(&client, &base, &owner.access_token, target.user_id).await?;
    anyhow::ensure!(
        avatar_content_type == "image/png" && !avatar_bytes.is_empty(),
        "uploaded avatar did not round-trip through the service: content-type={avatar_content_type}, bytes={}",
        avatar_bytes.len()
    );

    let (_, private_profile) = json_request(
        &client,
        Method::PUT,
        &base,
        "/api/profile/privacy",
        Some(&target.access_token),
        Some(json!({"accountVisibility": "PRIVATE"})),
    )
    .await?;
    anyhow::ensure!(
        private_profile["accountVisibility"] == "PRIVATE",
        "privacy update did not persist: {private_profile}"
    );
    let (anonymous_profile_status, _, _) = send_json_request(
        &client,
        Method::GET,
        &base,
        &format!("/api/profile/{}", target.user_id),
        None,
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        anonymous_profile_status == StatusCode::NOT_FOUND,
        "private profile should be hidden from anonymous viewers, got {anonymous_profile_status}"
    );
    let (_, follower_profile) = json_request(
        &client,
        Method::GET,
        &base,
        &format!("/api/profile/{}", target.user_id),
        Some(&owner.access_token),
        None,
    )
    .await?;
    anyhow::ensure!(
        follower_profile["accountVisibility"] == "PRIVATE",
        "existing follower should retain access to the private profile: {follower_profile}"
    );

    let post_text = format!("full-stack-journey-{suffix}");
    let (_, post) = json_request(
        &client,
        Method::POST,
        &base,
        "/api/posts",
        Some(&owner.access_token),
        Some(json!({"content": post_text, "visibility": "PUBLIC"})),
    )
    .await?;
    anyhow::ensure!(
        post["id"].as_str().is_some(),
        "post creation returned no id: {post}"
    );
    let post_id: Uuid = post["id"]
        .as_str()
        .context("post response omitted id")?
        .parse()?;

    // The target follows the owner before the post is created so the real
    // Post outbox can deliver a notification through the Notification gRPC API.
    let post_events = post_state
        .outbox_worker
        .tick(Utc::now() + chrono::Duration::minutes(5), 100)
        .await?;
    let post_outbox_state: Vec<(String, bool, i32, Option<String>)> = sqlx::query_as(
        "select event_type, published, attempt_count, last_error from post_event_outbox where aggregate_id = $1 order by created_at",
    )
    .bind(post_id)
    .fetch_all(&post_pool)
    .await?;
    anyhow::ensure!(
        post_events >= 1,
        "post-created event was not forwarded: forwarded={post_events}, outbox={post_outbox_state:?}"
    );
    let (_, target_notifications) = json_request(
        &client,
        Method::GET,
        &base,
        "/api/notifications?limit=50",
        Some(&target.access_token),
        None,
    )
    .await?;
    anyhow::ensure!(
        target_notifications["items"]
            .as_array()
            .is_some_and(|items| {
                items.iter().any(|notification| {
                    notification["type"] == "POST_CREATED"
                        && notification["payloadJson"]
                            .as_str()
                            .is_some_and(|payload| payload.contains(&post_id.to_string()))
                })
            }),
        "post-created event did not reach the target notification feed: {target_notifications}"
    );

    let comment_text = format!("real-comment-{suffix}");
    let (_, comment) = json_request(
        &client,
        Method::POST,
        &base,
        &format!("/api/posts/{post_id}/comments"),
        Some(&target.access_token),
        Some(json!({"content": comment_text})),
    )
    .await?;
    anyhow::ensure!(
        comment["id"].as_str().is_some(),
        "comment creation returned no id: {comment}"
    );
    let (_, like) = json_request(
        &client,
        Method::POST,
        &base,
        &format!("/api/posts/{post_id}/likes"),
        Some(&target.access_token),
        None,
    )
    .await?;
    anyhow::ensure!(like["liked"] == true, "post like did not activate: {like}");

    let comment_persisted: bool = sqlx::query_scalar(
        "select exists(select 1 from comments where post_id = $1 and author_id = $2 and content = $3)",
    )
    .bind(post_id)
    .bind(target.user_id)
    .bind(&comment_text)
    .fetch_one(&post_pool)
    .await?;
    let like_persisted: bool = sqlx::query_scalar(
        "select exists(select 1 from post_likes where post_id = $1 and user_id = $2)",
    )
    .bind(post_id)
    .bind(target.user_id)
    .fetch_one(&post_pool)
    .await?;
    anyhow::ensure!(
        comment_persisted,
        "comment was not persisted in Post PostgreSQL"
    );
    anyhow::ensure!(like_persisted, "like was not persisted in Post PostgreSQL");

    let engagement_events = post_state
        .outbox_worker
        .tick(Utc::now() + chrono::Duration::minutes(5), 100)
        .await?;
    let engagement_outbox_state: Vec<(Uuid, String, bool, i32, Option<String>)> =
        sqlx::query_as(
            "select event_id, event_type, published, attempt_count, last_error from post_event_outbox where aggregate_id = $1 order by created_at",
        )
        .bind(post_id)
        .fetch_all(&post_pool)
        .await?;
    anyhow::ensure!(
        engagement_events >= 2,
        "comment/like events were not forwarded: forwarded={engagement_events}, outbox={engagement_outbox_state:?}"
    );
    let (_, owner_notifications) = json_request(
        &client,
        Method::GET,
        &base,
        "/api/notifications?limit=50",
        Some(&owner.access_token),
        None,
    )
    .await?;
    let owner_notification_types = owner_notifications["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|notification| notification["type"].as_str())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    anyhow::ensure!(
        owner_notification_types.contains(&"POST_COMMENTED")
            && owner_notification_types.contains(&"POST_LIKED"),
        "comment/like notifications did not reach the owner: {owner_notifications}"
    );
    let (_, marked_read) = json_request(
        &client,
        Method::PATCH,
        &base,
        "/api/notifications/read-all",
        Some(&target.access_token),
        None,
    )
    .await?;
    anyhow::ensure!(
        marked_read["updatedCount"]
            .as_i64()
            .is_some_and(|count| count >= 1),
        "notification read-all did not update the target's feed: {marked_read}"
    );
    let read_notifications: i64 = sqlx::query_scalar(
        "select count(*) from notifications where user_id = $1 and read_at is not null",
    )
    .bind(target.user_id)
    .fetch_one(&notification_pool)
    .await?;
    anyhow::ensure!(
        read_notifications >= 1,
        "notification read state was not persisted"
    );

    let (_, profile_search) = json_request(
        &client,
        Method::GET,
        &base,
        &format!("/api/profile/search?q={target_username}"),
        Some(&owner.access_token),
        None,
    )
    .await?;
    anyhow::ensure!(
        profile_search["profiles"]
            .as_array()
            .is_some_and(|profiles| {
                profiles
                    .iter()
                    .any(|candidate| candidate["userId"] == target.user_id.to_string())
            }),
        "profile search omitted the real target profile: {profile_search}"
    );

    let (_, post_search) = json_request(
        &client,
        Method::GET,
        &base,
        &format!("/api/posts/search?q={post_text}"),
        Some(&owner.access_token),
        None,
    )
    .await?;
    anyhow::ensure!(
        post_search["posts"].as_array().is_some_and(|posts| {
            posts
                .iter()
                .any(|candidate| candidate["content"] == post_text)
        }),
        "post search omitted the real post: {post_search}"
    );

    let (report_status, report) = json_request(
        &client,
        Method::POST,
        &base,
        "/api/moderation/reports",
        Some(&owner.access_token),
        Some(json!({
            "targetType": "POST",
            "targetId": post_id.to_string(),
            "category": "SPAM",
            "reason": "Automated full-stack moderation journey report"
        })),
    )
    .await?;
    anyhow::ensure!(
        report_status == StatusCode::CREATED && report["status"] == "OPEN",
        "moderation report was not accepted and opened: {report_status} {report}"
    );
    let (_, my_reports) = json_request(
        &client,
        Method::GET,
        &base,
        "/api/moderation/reports/me",
        Some(&owner.access_token),
        None,
    )
    .await?;
    anyhow::ensure!(
        my_reports["reports"]
            .as_array()
            .is_some_and(|reports| { reports.iter().any(|saved| saved["id"] == report["id"]) }),
        "moderation report was not persisted for its reporter: {my_reports}"
    );

    // Exercise the cross-database partial-success window against real Gateway
    // and PostService databases: force only Gateway decision finalization to
    // fail after PostService commits its idempotent visibility action.
    sqlx::query(
        "insert into auth_user_roles (user_id, role, granted_by) values ($1, 'ROLE_ADMIN', $1) on conflict do nothing",
    )
    .bind(owner.user_id)
    .execute(&auth_pool)
    .await?;
    let (admin_login_status, admin_login_headers, admin_login) = send_json_request(
        &client,
        Method::POST,
        &base,
        "/auth/login",
        None,
        None,
        Some(json!({"email": owner_email, "password": "Valid-passphrase-42"})),
    )
    .await?;
    anyhow::ensure!(
        admin_login_status == StatusCode::OK,
        "admin re-login failed: {admin_login_status} {admin_login}"
    );
    owner.access_token = admin_login["accessToken"]
        .as_str()
        .context("admin login response omitted accessToken")?
        .to_owned();
    owner.refresh_cookie = refresh_cookie(&admin_login_headers)?;

    let moderation_report_id: Uuid = report["id"]
        .as_str()
        .context("moderation report response omitted id")?
        .parse()?;
    let fail_note = "E2E_INJECT_GATEWAY_COMMIT_FAILURE";
    sqlx::query(
        "alter table moderation_decisions add constraint e2e_reject_moderation_decision check (note <> 'E2E_INJECT_GATEWAY_COMMIT_FAILURE')",
    )
    .execute(&gateway_pool)
    .await?;
    let (failed_decision_status, _, _) = send_json_request(
        &client,
        Method::POST,
        &base,
        &format!("/admin/moderation/reports/{moderation_report_id}/decisions"),
        Some(&owner.access_token),
        None,
        Some(json!({"decisionType": "HIDE_CONTENT", "note": fail_note})),
    )
    .await?;
    anyhow::ensure!(
        failed_decision_status == StatusCode::INTERNAL_SERVER_ERROR,
        "injected Gateway finalization failure returned {failed_decision_status}"
    );
    let post_effect: Option<Uuid> = sqlx::query_scalar(
        "select moderation_report_id from posts where id = $1 and moderation_hidden_at is not null",
    )
    .bind(post_id)
    .fetch_optional(&post_pool)
    .await?;
    anyhow::ensure!(
        post_effect == Some(moderation_report_id),
        "PostService effect was not committed before the Gateway failure: {post_effect:?}"
    );
    let gateway_decision_count: i64 =
        sqlx::query_scalar("select count(*) from moderation_decisions where report_id = $1")
            .bind(moderation_report_id)
            .fetch_one(&gateway_pool)
            .await?;
    anyhow::ensure!(
        gateway_decision_count == 0,
        "injected Gateway failure unexpectedly persisted a decision"
    );
    let (outbox_state, outbox_lease): (String, Option<Uuid>) = sqlx::query_as(
        "select state, lease_owner from moderation_content_decision_outbox where report_id = $1",
    )
    .bind(moderation_report_id)
    .fetch_one(&gateway_pool)
    .await?;
    anyhow::ensure!(
        outbox_state == "PENDING" && outbox_lease.is_none(),
        "Gateway did not retain retryable intent after finalization failure: {outbox_state}/{outbox_lease:?}"
    );
    moderation_worker_task.abort();
    let worker_exit = moderation_worker_task
        .await
        .expect_err("stopped moderation worker should be cancelled");
    anyhow::ensure!(
        worker_exit.is_cancelled(),
        "moderation worker did not stop at the simulated crash boundary: {worker_exit}"
    );

    sqlx::query("alter table moderation_decisions drop constraint e2e_reject_moderation_decision")
        .execute(&gateway_pool)
        .await?;
    sqlx::query(
        r#"
        update moderation_content_decision_outbox
        set lease_owner = coalesce(lease_owner, $2),
            lease_expires_at = now() - interval '1 second',
            next_attempt_at = now() - interval '1 second'
        where report_id = $1 and state = 'PENDING'
        "#,
    )
    .bind(moderation_report_id)
    .bind(Uuid::new_v4())
    .execute(&gateway_pool)
    .await?;
    let restarted_worker_service = gateway_state.moderation_service.clone();
    let restarted_worker_metrics = gateway_state.metrics.clone();
    moderation_worker_task = tokio::spawn(async move {
        api_gateway::adapters::r#in::http::moderation_decision_worker::run(
            restarted_worker_service,
            restarted_worker_metrics,
        )
        .await
    });

    let mut reconciled = false;
    for _ in 0..100 {
        let outbox_state: String = sqlx::query_scalar(
            "select state from moderation_content_decision_outbox where report_id = $1",
        )
        .bind(moderation_report_id)
        .fetch_one(&gateway_pool)
        .await?;
        let gateway_decision_count: i64 =
            sqlx::query_scalar("select count(*) from moderation_decisions where report_id = $1")
                .bind(moderation_report_id)
                .fetch_one(&gateway_pool)
                .await?;
        if outbox_state == "COMPLETED" && gateway_decision_count == 1 {
            reconciled = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    anyhow::ensure!(
        reconciled,
        "the supervised worker did not reconcile the real cross-database moderation decision"
    );
    let post_effect_count: i64 = sqlx::query_scalar(
        "select count(*) from post_moderation_actions where report_id = $1 and action = 'HIDE'",
    )
    .bind(moderation_report_id)
    .fetch_one(&post_pool)
    .await?;
    anyhow::ensure!(
        post_effect_count == 1,
        "PostService retry duplicated its durable moderation action: {post_effect_count}"
    );
    let resolved_status: String =
        sqlx::query_scalar("select status from moderation_reports where id = $1")
            .bind(moderation_report_id)
            .fetch_one(&gateway_pool)
            .await?;
    anyhow::ensure!(
        resolved_status == "RESOLVED",
        "Gateway worker did not resolve the report: {resolved_status}"
    );

    let (hide_status, hidden_profile) = json_request(
        &client,
        Method::DELETE,
        &base,
        &format!("/api/profile/{}", target.user_id),
        Some(&target.access_token),
        None,
    )
    .await?;
    anyhow::ensure!(
        hide_status == StatusCode::OK
            && hidden_profile["deleted"] == true
            && hidden_profile["accountDeleted"] == false,
        "profile hide was not distinct from account deletion: {hidden_profile}"
    );
    assert_soft_deleted(&profile_pool, "profiles", target.user_id).await?;
    let (hidden_status, _, _) = send_json_request(
        &client,
        Method::GET,
        &base,
        &format!("/api/profile/{}", target.user_id),
        Some(&owner.access_token),
        None,
        None,
    )
    .await?;
    anyhow::ensure!(
        hidden_status == StatusCode::NOT_FOUND,
        "soft-deleted profile remained visible to a follower: {hidden_status}"
    );
    let (_, hidden_search) = json_request(
        &client,
        Method::GET,
        &base,
        &format!("/api/profile/search?q={target_username}"),
        Some(&owner.access_token),
        None,
    )
    .await?;
    anyhow::ensure!(
        hidden_search["profiles"]
            .as_array()
            .is_some_and(|profiles| {
                profiles
                    .iter()
                    .all(|candidate| candidate["userId"] != target.user_id.to_string())
            }),
        "soft-deleted profile remained in search: {hidden_search}"
    );

    logout_and_reject_refresh(&client, &base, &target.refresh_cookie).await?;

    let (delete_status, deletion) = json_request(
        &client,
        Method::DELETE,
        &base,
        "/auth/account",
        Some(&owner.access_token),
        None,
    )
    .await?;
    anyhow::ensure!(
        delete_status == StatusCode::ACCEPTED,
        "account deletion was not accepted: {deletion}"
    );
    anyhow::ensure!(
        deletion["deleted"] == true,
        "account deletion request did not identify the account: {deletion}"
    );

    let deletion_run = auth_state.user_deletion_worker.run_once(Utc::now()).await?;
    anyhow::ensure!(
        deletion_run.succeeded == 4,
        "expected all four real deletion RPCs to succeed, got {deletion_run:?}"
    );
    let saga = auth_state
        .get_user_deletion
        .execute(owner.user_id)
        .await?
        .context("deletion saga was not persisted")?;
    anyhow::ensure!(
        saga.status == "COMPLETED",
        "cross-service deletion saga did not complete: {saga:?}"
    );
    anyhow::ensure!(
        saga.steps.iter().all(|step| step.status == "SUCCEEDED"),
        "one or more service deletion steps failed: {saga:?}"
    );

    assert_soft_deleted(&profile_pool, "profiles", owner.user_id).await?;
    assert_soft_deleted(&friend_pool, "follows", owner.user_id).await?;
    assert_soft_deleted(&post_pool, "posts", owner.user_id).await?;
    let auth_deleted: bool =
        sqlx::query_scalar("select deleted_at is not null from users where id = $1")
            .bind(owner.user_id)
            .fetch_one(&auth_pool)
            .await?;
    anyhow::ensure!(auth_deleted, "Auth did not soft-delete the owner record");

    platform::shutdown::global().request();
    moderation_worker_task.await??;
    for stop in stop_signals {
        let _ = stop.send(());
    }
    for task in tasks {
        task.await??;
    }
    for pool in [
        profile_pool,
        friend_pool,
        post_pool,
        notification_pool,
        auth_pool,
        gateway_pool,
    ] {
        pool.close().await;
    }

    // Keep any containers alive through assertions and explicit application
    // shutdown. Their Drop implementations stop and remove only these test-owned
    // ephemeral containers; a native cluster is left to its owner.
    drop(_containers);
    Ok(())
}

async fn profile_read_wave(
    client: reqwest::Client,
    base: String,
    access_token: String,
    user_id: Uuid,
    request_count: usize,
    concurrency: usize,
) -> Result<(Vec<Duration>, Duration)> {
    anyhow::ensure!(
        request_count > 0 && concurrency > 0 && request_count.is_multiple_of(concurrency),
        "profile-read workload must divide into equal, non-empty workers"
    );
    let requests_per_worker = request_count / concurrency;
    let path = format!("/api/profile/{user_id}");
    let start_barrier = Arc::new(Barrier::new(concurrency));
    let started = Instant::now();
    let mut workers = JoinSet::new();
    for worker_index in 0..concurrency {
        let client = client.clone();
        let base = base.clone();
        let path = path.clone();
        let access_token = access_token.clone();
        let start_barrier = start_barrier.clone();
        workers.spawn(async move {
            start_barrier.wait().await;
            let mut samples = Vec::with_capacity(requests_per_worker);
            for request_index in 0..requests_per_worker {
                let request_started = Instant::now();
                let (status, headers, body) = send_json_request(
                    &client,
                    Method::GET,
                    &base,
                    &path,
                    Some(&access_token),
                    None,
                    None,
                )
                .await?;
                let elapsed = request_started.elapsed();
                let correlation_id = headers
                    .get("x-correlation-id")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("missing");
                anyhow::ensure!(
                    status == StatusCode::OK,
                    "profile-read load request failed: concurrency={concurrency}, worker={worker_index}, request={request_index}, status={status}, elapsed_ms={}, x-correlation-id={correlation_id}, body={body}",
                    elapsed.as_millis()
                );
                samples.push(elapsed);
            }
            Ok::<_, anyhow::Error>(samples)
        });
    }

    let mut samples = Vec::with_capacity(request_count);
    while let Some(worker) = workers.join_next().await {
        let mut worker_samples = worker.context("profile-read load worker failed")??;
        samples.append(&mut worker_samples);
    }
    anyhow::ensure!(
        samples.len() == request_count,
        "profile-read load recorded {} samples for {request_count} requests",
        samples.len()
    );
    Ok((samples, started.elapsed()))
}

fn percentile_ms(samples: &[Duration], percentile: usize) -> f64 {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let rank = (sorted.len() * percentile).div_ceil(100).max(1);
    sorted[rank - 1].as_secs_f64() * 1_000.0
}

fn current_process_rss_kib() -> Result<u64> {
    let pid = std::process::id().to_string();
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
        .context("failed to measure test-process RSS with ps")?;
    anyhow::ensure!(output.status.success(), "ps did not return process RSS");
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u64>()
        .context("ps returned an invalid RSS value")
}

fn command_text(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!value.is_empty()).then_some(value)
}

fn machine_profile() -> Value {
    json!({
        "os": std::env::consts::OS,
        "architecture": std::env::consts::ARCH,
        "cpu_model": cpu_model(),
        "logical_cpu_count": std::thread::available_parallelism().ok().map(|value| value.get()),
        "physical_memory_bytes": physical_memory_bytes()
    })
}

#[cfg(target_os = "macos")]
fn cpu_model() -> Option<String> {
    command_text("sysctl", &["-n", "machdep.cpu.brand_string"])
}

#[cfg(target_os = "linux")]
fn cpu_model() -> Option<String> {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix("model name\t:")
                .or_else(|| line.strip_prefix("Hardware\t:"))
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        })
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn cpu_model() -> Option<String> {
    None
}

#[cfg(target_os = "macos")]
fn physical_memory_bytes() -> Option<u64> {
    command_text("sysctl", &["-n", "hw.memsize"])?.parse().ok()
}

#[cfg(target_os = "linux")]
fn physical_memory_bytes() -> Option<u64> {
    let memory = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kibibytes = memory.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        (name == "MemTotal")
            .then(|| value.split_whitespace().next()?.parse::<u64>().ok())
            .flatten()
    })?;
    kibibytes.checked_mul(1024)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn physical_memory_bytes() -> Option<u64> {
    None
}

async fn create_service_databases(admin_url: &str) -> Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(admin_url)
        .await?;
    for database in [
        "auth_e2e",
        "profile_e2e",
        "friend_e2e",
        "post_e2e",
        "notification_e2e",
        "gateway_e2e",
    ] {
        sqlx::query(&format!("create database {database}"))
            .execute(&pool)
            .await?;
    }
    pool.close().await;
    Ok(())
}

fn database_url(host: &impl std::fmt::Display, port: u16, database: &str) -> String {
    format!("postgres://postgres:postgres@{host}:{port}/{database}")
}

fn grpc_endpoint(address: SocketAddr) -> String {
    format!("http://{address}")
}

async fn start_mail_sink(state: MailSinkState) -> Result<(SocketAddr, Shutdown, ServerTask)> {
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
    if headers.get(axum::http::header::AUTHORIZATION)
        != Some(&"Bearer e2e-mail-token".parse().expect("static header"))
    {
        return StatusCode::UNAUTHORIZED;
    }
    let Some(idempotency_key) = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
    else {
        return StatusCode::BAD_REQUEST;
    };
    let Some(email) = payload.get("email").and_then(Value::as_str) else {
        return StatusCode::BAD_REQUEST;
    };
    state
        .attempted_keys
        .lock()
        .await
        .push((email.to_owned(), idempotency_key.clone()));
    let first_delivery = state.processed_keys.lock().await.insert(idempotency_key);
    if first_delivery {
        state.inbox.lock().await.push(payload);
    }
    if state
        .failures_remaining
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |remaining| {
            remaining.checked_sub(1)
        })
        .is_ok()
    {
        return StatusCode::SERVICE_UNAVAILABLE;
    }
    StatusCode::ACCEPTED
}

async fn signup_verify_login(
    client: &reqwest::Client,
    base: &str,
    auth_pool: &PgPool,
    mail_worker: &EmailOutboxWorker,
    mail_sink: &MailSinkState,
    inbox: &MailInbox,
    email: &str,
    recover_mail_delivery: bool,
) -> Result<SignedIn> {
    let (signup_status, signup) = json_request(
        client,
        Method::POST,
        base,
        "/auth/email/signup",
        None,
        Some(json!({"email": email, "password": "Valid-passphrase-42"})),
    )
    .await?;
    anyhow::ensure!(
        signup_status == StatusCode::OK,
        "signup failed for {email}: {signup}"
    );
    anyhow::ensure!(
        signup["status"] == "PENDING_VERIFICATION",
        "signup was not pending: {signup}"
    );
    let user_id: Uuid = signup["userId"]
        .as_str()
        .context("signup response omitted userId")?
        .parse()?;

    let delivered = mail_worker.run_once().await?;
    if recover_mail_delivery {
        anyhow::ensure!(
            delivered == 0,
            "worker recorded delivery despite the provider's failed acknowledgement for {email}"
        );
        let (attempts, last_error_code, retryable): (i32, Option<String>, bool) = sqlx::query_as(
            r#"
                select attempts, last_error_code,
                       delivered_at is null and dead_lettered_at is null
                           and payload_json is not null and lease_id is null
                from auth_mail_outbox
                where user_id = $1 and kind = 'VERIFY_EMAIL'
                order by created_at desc limit 1
                "#,
        )
        .bind(user_id)
        .fetch_one(auth_pool)
        .await?;
        anyhow::ensure!(
            attempts == 1 && last_error_code.as_deref() == Some("DELIVERY_HTTP_ERROR") && retryable,
            "provider failure did not leave one retryable durable outbox row: attempts={attempts}, error={last_error_code:?}, retryable={retryable}"
        );
        let accepted_before_ack = inbox
            .lock()
            .await
            .iter()
            .filter(|message| message["email"] == email)
            .count();
        anyhow::ensure!(
            accepted_before_ack == 1,
            "the test provider did not persist exactly one effect before its failed acknowledgement"
        );

        let made_due = sqlx::query(
            r#"
            update auth_mail_outbox set next_attempt_at = now()
            where user_id = $1 and kind = 'VERIFY_EMAIL'
              and delivered_at is null and dead_lettered_at is null
            "#,
        )
        .bind(user_id)
        .execute(auth_pool)
        .await?
        .rows_affected();
        anyhow::ensure!(made_due == 1, "expected one retryable verification event");
        let recovered = mail_worker.run_once().await?;
        anyhow::ensure!(
            recovered == 1,
            "recovered provider did not deliver exactly one verification mail for {email}: {recovered}"
        );
        let follow_up = mail_worker.run_once().await?;
        anyhow::ensure!(
            follow_up == 0,
            "a follow-up drain duplicated {email}'s mail"
        );
        let delivery_count = inbox
            .lock()
            .await
            .iter()
            .filter(|message| message["email"] == email)
            .count();
        anyhow::ensure!(
            delivery_count == 1,
            "expected one external mail effect for {email}, got {delivery_count}"
        );
        let attempted_keys: Vec<String> = mail_sink
            .attempted_keys
            .lock()
            .await
            .iter()
            .filter(|(attempted_email, _)| attempted_email == email)
            .map(|(_, key)| key.clone())
            .collect();
        anyhow::ensure!(
            attempted_keys.len() == 2
                && !attempted_keys[0].is_empty()
                && attempted_keys[0] == attempted_keys[1],
            "outbox replay did not reuse one stable provider idempotency key: {attempted_keys:?}"
        );
    } else {
        anyhow::ensure!(
            delivered == 1,
            "expected one verification mail delivery for {email}, got {delivered}"
        );
    }
    let message = inbox
        .lock()
        .await
        .iter()
        .rev()
        .find(|message| message["email"] == email)
        .cloned()
        .context("test mail sink did not receive the verification message")?;
    let code = message["verification_code"]
        .as_str()
        .context("test mail sink payload omitted verification_code")?;

    let (verify_status, verification) = json_request(
        client,
        Method::POST,
        base,
        "/auth/email/verify",
        None,
        Some(json!({"email": email, "code": code})),
    )
    .await?;
    anyhow::ensure!(
        verify_status == StatusCode::OK,
        "email verification failed for {email}: {verification}"
    );
    anyhow::ensure!(
        verification["status"] == "VERIFIED",
        "email verification did not activate user: {verification}"
    );

    let (login_status, login_headers, login) = send_json_request(
        client,
        Method::POST,
        base,
        "/auth/login",
        None,
        None,
        Some(json!({"email": email, "password": "Valid-passphrase-42"})),
    )
    .await?;
    anyhow::ensure!(
        login_status == StatusCode::OK,
        "login failed for {email}: {login}"
    );
    let access_token = login["accessToken"]
        .as_str()
        .context("login response omitted accessToken")?
        .to_owned();
    Ok(SignedIn {
        user_id,
        access_token,
        refresh_cookie: refresh_cookie(&login_headers)?,
    })
}

async fn refresh_session(
    client: &reqwest::Client,
    base: &str,
    cookie: &str,
) -> Result<RefreshedSession> {
    let (status, headers, body) = send_json_request(
        client,
        Method::POST,
        base,
        "/auth/refresh",
        None,
        Some(cookie),
        None,
    )
    .await?;
    anyhow::ensure!(
        status == StatusCode::OK,
        "session refresh failed: {status} {body}"
    );
    let access_token = body["accessToken"]
        .as_str()
        .context("refresh response omitted accessToken")?
        .to_owned();
    Ok(RefreshedSession {
        access_token,
        refresh_cookie: refresh_cookie(&headers)?,
    })
}

async fn logout_and_reject_refresh(
    client: &reqwest::Client,
    base: &str,
    cookie: &str,
) -> Result<()> {
    let (logout_status, _, logout_body) = send_json_request(
        client,
        Method::POST,
        base,
        "/auth/logout",
        None,
        Some(cookie),
        None,
    )
    .await?;
    anyhow::ensure!(
        logout_status == StatusCode::NO_CONTENT,
        "logout failed: {logout_status} {logout_body}"
    );

    let (refresh_status, _, refresh_body) = send_json_request(
        client,
        Method::POST,
        base,
        "/auth/refresh",
        None,
        Some(cookie),
        None,
    )
    .await?;
    anyhow::ensure!(
        refresh_status == StatusCode::UNAUTHORIZED,
        "logged-out refresh credential remained usable: {refresh_status} {refresh_body}"
    );
    Ok(())
}

fn refresh_cookie(headers: &reqwest::header::HeaderMap) -> Result<String> {
    let raw_cookie = headers
        .get(reqwest::header::SET_COOKIE)
        .context("auth response omitted the refresh Set-Cookie header")?
        .to_str()?;
    let cookie = raw_cookie
        .split(';')
        .next()
        .context("Set-Cookie header had no cookie pair")?
        .trim();
    anyhow::ensure!(
        cookie.starts_with("refresh_token="),
        "auth response returned an unexpected cookie: {cookie}"
    );
    Ok(cookie.to_owned())
}

async fn upload_test_avatar(
    client: &reqwest::Client,
    base: &str,
    access_token: &str,
) -> Result<Value> {
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        2,
        2,
        image::Rgba([0x28, 0x71, 0x9c, 0xff]),
    ));
    let mut encoded = Cursor::new(Vec::new());
    image.write_to(&mut encoded, image::ImageFormat::Png)?;
    let part = reqwest::multipart::Part::bytes(encoded.into_inner())
        .file_name("journey-avatar.png")
        .mime_str("image/png")?;
    let form = reqwest::multipart::Form::new().part("avatar", part);
    let response = client
        .post(format!("{base}/api/profile/avatar"))
        .bearer_auth(access_token)
        .multipart(form)
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;
    let value = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&body).unwrap_or_else(|_| json!({"raw": body}))
    };
    anyhow::ensure!(
        status.is_success(),
        "avatar upload returned {status}: {value}"
    );
    Ok(value)
}

async fn fetch_avatar(
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
        "avatar fetch returned {status}: {} bytes",
        bytes.len()
    );
    Ok((content_type, bytes))
}

async fn json_request(
    client: &reqwest::Client,
    method: Method,
    base: &str,
    path: &str,
    bearer: Option<&str>,
    body: Option<Value>,
) -> Result<(StatusCode, Value)> {
    let (status, _, value) =
        send_json_request(client, method.clone(), base, path, bearer, None, body).await?;
    anyhow::ensure!(
        status.is_success(),
        "{method} {path} returned {status}: {value}"
    );
    Ok((status, value))
}

async fn send_json_request(
    client: &reqwest::Client,
    method: Method,
    base: &str,
    path: &str,
    bearer: Option<&str>,
    cookie: Option<&str>,
    body: Option<Value>,
) -> Result<(StatusCode, reqwest::header::HeaderMap, Value)> {
    let mut request = client.request(method.clone(), format!("{base}{path}"));
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
    let headers = response.headers().clone();
    let body = response.text().await?;
    let value = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&body).unwrap_or_else(|_| json!({"raw": body}))
    };
    Ok((status, headers, value))
}

async fn assert_soft_deleted(pool: &PgPool, table: &str, user_id: Uuid) -> Result<()> {
    let query = match table {
        "profiles" => {
            "select exists(select 1 from profiles where user_id = $1 and deleted_at is not null)"
        }
        "follows" => {
            "select exists(select 1 from follows where follower_id = $1 and deleted_at is not null)"
        }
        "posts" => {
            "select exists(select 1 from posts where author_id = $1 and deleted_at is not null)"
        }
        _ => anyhow::bail!("unsupported deletion evidence table"),
    };
    let deleted: bool = sqlx::query_scalar(query)
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    anyhow::ensure!(
        deleted,
        "{table} did not record the expected soft deletion for {user_id}"
    );
    Ok(())
}

#[cfg(test)]
mod profile_load_diagnostics_tests {
    use super::*;
    use axum::{body::Body, http::Response, routing::get, Router};

    #[tokio::test]
    async fn failed_profile_read_reports_response_context_without_request_credentials() {
        let app = Router::new().route("/api/profile/:user_id", get(fail_profile_read));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local response fixture");
        let address = listener.local_addr().expect("read fixture address");
        let server = tokio::spawn(async move { axum::serve(listener, app).await });
        let access_token = "synthetic-profile-load-token";

        let error = profile_read_wave(
            reqwest::Client::new(),
            format!("http://{address}"),
            access_token.to_owned(),
            Uuid::new_v4(),
            1,
            1,
        )
        .await
        .expect_err("non-200 response must fail the load wave");
        server.abort();
        let _ = server.await;

        let message = format!("{error:#}");
        assert!(message.contains("concurrency=1"), "{message}");
        assert!(message.contains("worker=0"), "{message}");
        assert!(message.contains("request=0"), "{message}");
        assert!(message.contains("status=500"), "{message}");
        assert!(message.contains("elapsed_ms="), "{message}");
        assert!(
            message.contains("x-correlation-id=load-probe-correlation"),
            "{message}"
        );
        assert!(
            message.contains("synthetic downstream failure"),
            "{message}"
        );
        assert!(!message.contains(access_token), "{message}");
    }

    async fn fail_profile_read() -> Response<Body> {
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header("x-correlation-id", "load-probe-correlation")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "code": "GATEWAY_500",
                    "detail": "synthetic downstream failure"
                })
                .to_string(),
            ))
            .expect("build synthetic error response")
    }
}
