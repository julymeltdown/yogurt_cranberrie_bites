#![allow(dead_code)]
use api_gateway::adapters::out::post_client::classify_post_client_error;

pub mod http;

use anyhow::Result;
use api_gateway::adapters::out::telemetry_queue::QueuedTelemetry;
use api_gateway::application::dto::auth_dto::{
    AuthResponse, EmailAvailabilityResponse, EmailCheckRequest, EmailSignupRequest,
    EmailVerifyRequest, LinkedProvidersResponse, LoginRequest, OAuthLinkRequest,
    PasswordResetConfirmRequest, PasswordResetRequest, PasswordResetResponse, SignupResponse,
};
use api_gateway::application::dto::friend_dto::{
    FollowActionResponse, FollowCountsResponse, FollowEdgeResponse, FollowListResponse,
    FollowStatusResponse, RelationshipPolicyResponse,
};
use api_gateway::application::dto::notification_dto::{
    NotificationFeedResponse, NotificationMarkAllReadResponse, NotificationResponse,
};
use api_gateway::application::dto::post_dto::{
    CommentCreateRequest, CommentResponse, DeletePostResponse, FeedResponse,
    MutationReceiptStatusDto, PostCreateRequest, PostDetailResponse, PostLikeResponse,
    PostResponse,
};
use api_gateway::application::dto::profile_dto::{
    AvatarContent, DeleteProfileResponse, ProfileBatchResponse, ProfileResponse,
    ProfileSearchResponse, ProfileUpdateRequest,
};
use api_gateway::application::ports::{
    AuthClientPort, FriendClientPort, NotificationClientPort, PostClientPort, ProfileClientPort,
    TelemetryRecord, TelemetryRepositoryPort,
};
use api_gateway::config::{
    build_state_with_clients, connect_pool, AppConfig, AppState, RuntimeMode,
};
use api_gateway::domain::telemetry::TelemetrySummary;
use api_gateway::domain::IdentityProvider;
use async_trait::async_trait;
use axum::Router;
use base64ct::{Base64UrlUnpadded, Encoding};
use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use rand::rngs::OsRng;
use rsa::pkcs8::EncodePrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use tokio::sync::watch;
use uuid::Uuid;

const TEST_JWT_ISSUER: &str = "auth-service-test";
const TEST_JWT_AUDIENCE: &str = "port-rust-api-test";
const TEST_JWT_KID: &str = "api-gateway-test-key";
/// Session family named by every token `bearer_token` mints.
pub const TEST_SESSION_ID: &str = "44444444-4444-4444-4444-444444444444";
const MAX_CONCURRENT_POSTGRES_TEST_APPS: usize = 1;
type CapturedTraceContexts = Arc<Mutex<Vec<Option<platform::trace_context::TraceContext>>>>;

pub(crate) struct TestAppLimiter {
    state: Mutex<TestAppLimiterState>,
    changes: watch::Sender<u64>,
}

enum TestAppLimiterState {
    Available,
    InUse,
    Cleaning,
    Failed(String),
}

impl TestAppLimiter {
    fn new() -> Self {
        let (changes, _) = watch::channel(0);
        Self {
            state: Mutex::new(TestAppLimiterState::Available),
            changes,
        }
    }

    pub(crate) async fn acquire(self: &Arc<Self>) -> Result<TestAppPermit> {
        let mut changes = self.changes.subscribe();
        loop {
            {
                let mut state = self.state.lock().expect("test app limiter state");
                match &*state {
                    TestAppLimiterState::Available => {
                        *state = TestAppLimiterState::InUse;
                        return Ok(TestAppPermit {
                            limiter: self.clone(),
                            state: TestAppPermitState::Startup,
                        });
                    }
                    TestAppLimiterState::Failed(error) => {
                        return Err(anyhow::anyhow!(
                            "previous Gateway test app cleanup failed: {error}"
                        ));
                    }
                    TestAppLimiterState::InUse | TestAppLimiterState::Cleaning => {}
                }
            }
            changes.changed().await?;
        }
    }

    pub(crate) fn available_permits(&self) -> usize {
        usize::from(matches!(
            *self.state.lock().expect("test app limiter state"),
            TestAppLimiterState::Available
        ))
    }

    pub(crate) async fn drain(&self) -> Result<()> {
        let mut changes = self.changes.subscribe();
        loop {
            {
                let state = self.state.lock().expect("test app limiter state");
                match &*state {
                    TestAppLimiterState::Available => return Ok(()),
                    TestAppLimiterState::Failed(error) => {
                        return Err(anyhow::anyhow!("Gateway test app cleanup failed: {error}"));
                    }
                    TestAppLimiterState::InUse | TestAppLimiterState::Cleaning => {}
                }
            }
            changes.changed().await?;
        }
    }

    fn begin_cleanup(&self) {
        let mut state = self.state.lock().expect("test app limiter state");
        assert!(matches!(*state, TestAppLimiterState::InUse));
        *state = TestAppLimiterState::Cleaning;
        self.changes.send_modify(|version| *version += 1);
    }

    fn finish_cleanup(&self, result: Result<()>) {
        let mut state = self.state.lock().expect("test app limiter state");
        assert!(matches!(*state, TestAppLimiterState::Cleaning));
        *state = match result {
            Ok(()) => TestAppLimiterState::Available,
            Err(error) => TestAppLimiterState::Failed(error.to_string()),
        };
        self.changes.send_modify(|version| *version += 1);
    }
}

pub(crate) fn postgres_test_app_limiter() -> Arc<TestAppLimiter> {
    static LIMITER: OnceLock<Arc<TestAppLimiter>> = OnceLock::new();
    LIMITER
        .get_or_init(|| {
            assert_eq!(MAX_CONCURRENT_POSTGRES_TEST_APPS, 1);
            Arc::new(TestAppLimiter::new())
        })
        .clone()
}

pub(crate) fn isolated_postgres_test_app_limiter() -> Arc<TestAppLimiter> {
    Arc::new(TestAppLimiter::new())
}

pub(crate) struct TestAppPermit {
    limiter: Arc<TestAppLimiter>,
    state: TestAppPermitState,
}

#[derive(Clone, Copy)]
enum TestAppPermitState {
    Startup,
    Cleanup,
    Completed,
}

impl TestAppPermit {
    fn begin_cleanup(&mut self) {
        self.limiter.begin_cleanup();
        self.state = TestAppPermitState::Cleanup;
    }

    fn finish(mut self, result: Result<()>) {
        self.state = TestAppPermitState::Completed;
        self.limiter.finish_cleanup(result);
    }
}

impl Drop for TestAppPermit {
    fn drop(&mut self) {
        match self.state {
            TestAppPermitState::Startup => {
                let mut state = self.limiter.state.lock().expect("test app limiter state");
                *state = TestAppLimiterState::Available;
                self.limiter.changes.send_modify(|version| *version += 1);
            }
            TestAppPermitState::Cleanup => self.limiter.finish_cleanup(Err(anyhow::anyhow!(
                "cleanup worker ended before reporting completion"
            ))),
            TestAppPermitState::Completed => {}
        }
    }
}

#[must_use = "TestApp must be explicitly shut down with TestApp::shutdown().await"]
pub struct TestApp {
    pub config: AppConfig,
    pub pool: sqlx::PgPool,
    pub state: Arc<AppState>,
    pub router: Router,
    pub friend_client: Arc<FakeFriendClient>,
    pub post_client: Arc<FakePostClient>,
    profile_avatar_upload_calls: Arc<AtomicUsize>,
    profile_trace_contexts: CapturedTraceContexts,
    owner_receipt_statuses: Arc<Mutex<HashMap<Uuid, MutationReceiptStatusDto>>>,
    owner_receipt_lookup_available: Arc<AtomicBool>,
    _cleanup: TestAppCleanup,
}

impl TestApp {
    pub fn profile_avatar_upload_calls(&self) -> usize {
        self.profile_avatar_upload_calls.load(Ordering::SeqCst)
    }

    pub fn profile_trace_contexts(&self) -> Vec<Option<platform::trace_context::TraceContext>> {
        self.profile_trace_contexts
            .lock()
            .expect("fake profile trace contexts")
            .clone()
    }

    pub fn set_owner_receipt_status(&self, receipt: MutationReceiptStatusDto) {
        self.owner_receipt_statuses
            .lock()
            .expect("fake owner receipt status")
            .insert(receipt.operation_id, receipt);
    }

    pub fn set_owner_receipt_lookup_available(&self, available: bool) {
        self.owner_receipt_lookup_available
            .store(available, Ordering::SeqCst);
    }

    pub fn remove_owner_receipt(&self, operation_id: Uuid) {
        self.owner_receipt_statuses
            .lock()
            .expect("fake owner receipt status")
            .remove(&operation_id);
    }

    pub async fn shutdown(mut self) -> Result<()> {
        self._cleanup.shutdown().await
    }
}

struct TestAppCleanup {
    postgres: Option<ContainerAsync<Postgres>>,
    postgres_permit: Option<TestAppPermit>,
    pool: sqlx::PgPool,
    inject_removal_failure: bool,
}

impl Drop for TestAppCleanup {
    fn drop(&mut self) {
        if let Some(job) = self.take_job() {
            if let Err(error) = test_app_cleanup_sender().send(job) {
                drop(error.0);
            }
            if !std::thread::panicking() {
                panic!("TestApp dropped without awaiting TestApp::shutdown()");
            }
        }
    }
}

impl TestAppCleanup {
    async fn shutdown(&mut self) -> Result<()> {
        match self.take_job() {
            Some(job) => job.run().await,
            None => Ok(()),
        }
    }

    fn take_job(&mut self) -> Option<TestAppCleanupJob> {
        let mut permit = self.postgres_permit.take()?;
        permit.begin_cleanup();
        Some(TestAppCleanupJob {
            pool: self.pool.clone(),
            postgres: self.postgres.take(),
            permit,
            inject_removal_failure: self.inject_removal_failure,
        })
    }
}

struct TestAppCleanupJob {
    pool: sqlx::PgPool,
    postgres: Option<ContainerAsync<Postgres>>,
    permit: TestAppPermit,
    inject_removal_failure: bool,
}

impl TestAppCleanupJob {
    async fn run(self) -> Result<()> {
        let Self {
            pool,
            postgres,
            permit,
            inject_removal_failure,
        } = self;
        pool.close().await;
        let result = match (inject_removal_failure, postgres) {
            (true, postgres) => {
                drop(postgres);
                Err(anyhow::anyhow!("injected testcontainer removal failure"))
            }
            (false, Some(postgres)) => postgres.rm().await.map_err(|error| {
                anyhow::anyhow!("failed to remove Gateway test Postgres: {error}")
            }),
            (false, None) => Ok(()),
        };
        match result {
            Ok(()) => {
                permit.finish(Ok(()));
                Ok(())
            }
            Err(error) => {
                let message = error.to_string();
                permit.finish(Err(anyhow::anyhow!(message.clone())));
                Err(anyhow::anyhow!(message))
            }
        }
    }
}

fn test_app_cleanup_sender() -> &'static tokio::sync::mpsc::UnboundedSender<TestAppCleanupJob> {
    static CLEANUP_SENDER: OnceLock<tokio::sync::mpsc::UnboundedSender<TestAppCleanupJob>> =
        OnceLock::new();
    CLEANUP_SENDER.get_or_init(|| {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<TestAppCleanupJob>();
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .thread_name("gateway-test-cleanup")
                .enable_all()
                .build()
                .expect("Gateway test cleanup runtime")
                .block_on(async move {
                    while let Some(job) = receiver.recv().await {
                        let _ = job.run().await;
                    }
                });
        });
        sender
    })
}

pub async fn start_gateway_app(seed_enabled: bool) -> Result<TestApp> {
    start_gateway_app_with_runtime_mode(seed_enabled, RuntimeMode::Production).await
}

pub async fn assembled_gateway_router() -> Result<(Router, Arc<FakeFriendClient>)> {
    let config = AppConfig {
        database_migration_mode: migration_runtime::MigrationMode::Startup,
        http_addr: "127.0.0.1:0".parse()?,
        metrics_addr: "127.0.0.1:0".parse()?,
        grpc_addr: "127.0.0.1:0".parse().expect("grpc addr"),
        database_pool_max_connections: 5,
        journal_pool_max_connections: 4,
        background_pool_max_connections: 3,
        database_url: "postgres://postgres:postgres@localhost:5432/gateway_smoke".into(),
        seed_enabled: false,
        auth_grpc_endpoint: "http://127.0.0.1:50051".into(),
        concert_grpc_endpoint: None,
        profile_grpc_endpoint: "http://127.0.0.1:50052".into(),
        friend_grpc_endpoint: "http://127.0.0.1:50053".into(),
        notification_grpc_endpoint: "http://127.0.0.1:50054".into(),
        post_grpc_endpoint: "http://127.0.0.1:50055".into(),
        refresh_cookie_domain: String::new(),
        refresh_cookie_secure: false,
        refresh_cookie_max_age_seconds: 1_209_600,
        aggregation_cache_ttl_seconds: 30,
        cors_allowed_origins: vec!["http://localhost:3000".into()],
        jwt_issuer: TEST_JWT_ISSUER.into(),
        jwt_audience: TEST_JWT_AUDIENCE.into(),
        jwt_jwks_json: test_jwt_keys().jwks_json.clone(),
        jwt_keys_refresh_seconds: 0,
        jwt_keys_min_refresh_seconds: 10,
        high_risk_session_freshness_seconds: 5,
        runtime_mode: RuntimeMode::Production,
        developer_portal_links: api_gateway::config::DeveloperPortalLinks::defaults(
            RuntimeMode::Production,
        ),
        rate_limit_per_minute: 600,
        admission: api_gateway::domain::admission::AdmissionPolicies::default(),
        admission_hmac_key: "test-admission-hmac-key-with-at-least-32-bytes".into(),
        search_cursor_hmac_key: "test-search-cursor-key-has-32-bytes-minimum".into(),
        search_cursor_ttl_seconds: 900,
        search_cursor_legacy_accept_until: None,
        request_timeout_ms: 1_000,
        journal_client_timeout_ms: 900,
        async_operation_keyring: None,
        telemetry_retention_days: 30,
        telemetry_queue_capacity: 4096,
        audit_retention_days: 365,
        actor_delegation_signer: platform::actor_delegation::ActorDelegationSigner::local_parity(
            platform::grpc_auth::Workload::Gateway,
        ),
        caller_registry: platform::grpc_auth::CallerRegistry::new(),
        authority_exchange: api_gateway::config::authority_exchange(
            false,
            &platform::actor_delegation::ActorDelegationSigner::local_parity(
                platform::grpc_auth::Workload::Gateway,
            ),
            false,
            platform::actor_delegation::exchange::ResourceEvidenceVerifier::local_parity(),
        )
        .expect("parity authority exchange"),
        outbound_identity: "test-internal-service-token".into(),
        tls_material: None,
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_millis(20))
        .connect_lazy(&config.database_url)?;
    let friend_client = Arc::new(FakeFriendClient::default());
    let mut state = build_state_with_clients(
        &config,
        pool,
        Arc::new(FakeAuthClient) as Arc<dyn AuthClientPort>,
        Arc::new(FakeProfileClient::default()) as Arc<dyn ProfileClientPort>,
        friend_client.clone() as Arc<dyn FriendClientPort>,
        Arc::new(FakeNotificationClient) as Arc<dyn NotificationClientPort>,
        Arc::new(FakePostClient::default()) as Arc<dyn PostClientPort>,
    )
    .await?;
    state.telemetry_service = QueuedTelemetry::with_capacity(
        Arc::new(NoopTelemetryRepository),
        api_gateway::adapters::out::telemetry_queue::DEFAULT_QUEUE_CAPACITY,
    );
    let state = Arc::new(state);
    Ok((
        api_gateway::adapters::r#in::http::router(state),
        friend_client,
    ))
}

struct NoopTelemetryRepository;

impl NoopTelemetryRepository {
    async fn record_infrastructure_inner(&self, _record: TelemetryRecord) -> anyhow::Result<()> {
        Ok(())
    }

    async fn summary_infrastructure_inner(&self) -> anyhow::Result<TelemetrySummary> {
        Ok(TelemetrySummary {
            total_requests: 0,
            error_rate: 0.0,
            p95_latency_ms: 0,
        })
    }
}

#[async_trait]
impl TelemetryRepositoryPort for NoopTelemetryRepository {
    async fn record(
        &self,
        _record: TelemetryRecord,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<()> {
        self.record_infrastructure_inner(_record).await.map_err(
            api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
        )
    }
    async fn summary(
        &self,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<
        api_gateway::application::dto::infrastructure_dto::TelemetrySummaryDto,
    > {
        self.summary_infrastructure_inner()
            .await
            .map(Into::into)
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
}

pub async fn start_gateway_app_with_runtime_mode(
    seed_enabled: bool,
    runtime_mode: RuntimeMode,
) -> Result<TestApp> {
    start_gateway_app_with_options(
        seed_enabled,
        runtime_mode,
        600,
        1000,
        None,
        postgres_test_app_limiter(),
        false,
    )
    .await
}

pub async fn start_gateway_app_with_rate_limit(
    seed_enabled: bool,
    rate_limit_per_minute: u32,
) -> Result<TestApp> {
    start_gateway_app_with_options(
        seed_enabled,
        RuntimeMode::Production,
        rate_limit_per_minute,
        1000,
        None,
        postgres_test_app_limiter(),
        false,
    )
    .await
}

pub async fn start_gateway_app_with_profile_delay(
    seed_enabled: bool,
    profile_delay: Duration,
    request_timeout_ms: u64,
) -> Result<TestApp> {
    start_gateway_app_with_options(
        seed_enabled,
        RuntimeMode::Production,
        600,
        request_timeout_ms,
        Some(profile_delay),
        postgres_test_app_limiter(),
        false,
    )
    .await
}

pub(crate) async fn start_gateway_app_with_injected_cleanup_failure(
    limiter: Arc<TestAppLimiter>,
) -> Result<TestApp> {
    start_gateway_app_with_options(
        false,
        RuntimeMode::Production,
        600,
        1000,
        None,
        limiter,
        true,
    )
    .await
}

async fn start_gateway_app_with_options(
    seed_enabled: bool,
    runtime_mode: RuntimeMode,
    rate_limit_per_minute: u32,
    request_timeout_ms: u64,
    profile_delay: Option<Duration>,
    limiter: Arc<TestAppLimiter>,
    inject_removal_failure: bool,
) -> Result<TestApp> {
    let postgres_permit = limiter.acquire().await?;
    // Keep disposable Testcontainers as the default, while allowing an explicit
    // isolated local PostgreSQL URL for environments without a container runtime.
    let external_database_url = std::env::var("GATEWAY_TEST_DATABASE_URL")
        .ok()
        .filter(|url| !url.trim().is_empty());
    let (postgres, database_url) = match external_database_url {
        Some(database_url) => (None, database_url),
        None => {
            let (postgres, database_url) =
                test_support::postgres::start_postgres_with_url().await?;
            (Some(postgres), database_url)
        }
    };
    let database_url = if postgres.is_none()
        && std::env::var("GATEWAY_TEST_CREATE_ISOLATED_DATABASES").as_deref() == Ok("1")
    {
        use sqlx::postgres::PgConnectOptions;
        use std::net::IpAddr;
        let options: PgConnectOptions = database_url.parse()?;
        let host = options
            .get_host()
            .trim_start_matches('[')
            .trim_end_matches(']');
        anyhow::ensure!(
            host == "localhost" || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()),
            "isolated Gateway fixtures require loopback"
        );
        anyhow::ensure!(
            options.get_socket().is_none()
                && options.get_options().is_none()
                && !database_url.contains('?')
                && !database_url.contains('#'),
            "fixture overrides forbidden"
        );
        anyhow::ensure!(
            options
                .get_database()
                .is_some_and(|db| db.starts_with("port_rust_gateway_test_")),
            "owned Gateway parent database required"
        );
        let admin = sqlx::PgPool::connect(&database_url).await?;
        let observed: String =
            sqlx::query_scalar("select system_identifier::text from pg_control_system()")
                .fetch_one(&admin)
                .await?;
        anyhow::ensure!(
            std::env::var("GATEWAY_TEST_INITDB_SYSTEM_IDENTIFIER").as_deref()
                == Ok(observed.as_str()),
            "observed initdb lineage differs"
        );
        let database = format!("port_rust_gateway_fixture_{}", Uuid::new_v4().simple());
        sqlx::query(&format!("create database {database} template template0"))
            .execute(&admin)
            .await?;
        admin.close().await;
        let isolated = options.database(&database);
        let probe = sqlx::PgPool::connect_with(isolated.clone()).await?;
        let empty: bool = sqlx::query_scalar(
            "select not exists(select 1 from pg_tables where schemaname='public')",
        )
        .fetch_one(&probe)
        .await?;
        anyhow::ensure!(empty, "new fixture contains tables before migrations");
        probe.close().await;
        println!("GATEWAY_FRESH_FIXTURE {{\"database\":\"{database}\",\"systemIdentifier\":\"{observed}\",\"template\":\"template0\",\"emptyBeforeMigrations\":true}}");
        // Preserve the validated parent URL's credential and authority, changing
        // only its controlled database path. These native URLs have no overrides.
        let mut url = database_url
            .rsplit_once('/')
            .ok_or_else(|| anyhow::anyhow!("invalid fixture URL"))?
            .0
            .to_string();
        url.push('/');
        url.push_str(&database);
        url
    } else {
        database_url
    };
    let config = AppConfig {
        database_migration_mode: migration_runtime::MigrationMode::Startup,
        http_addr: "127.0.0.1:18080".parse()?,
        metrics_addr: "127.0.0.1:0".parse()?,
        grpc_addr: "127.0.0.1:0".parse().expect("grpc addr"),
        database_pool_max_connections: 5,
        journal_pool_max_connections: 4,
        background_pool_max_connections: 3,
        database_url,
        seed_enabled,
        auth_grpc_endpoint: "http://127.0.0.1:50051".into(),
        concert_grpc_endpoint: None,
        profile_grpc_endpoint: "http://127.0.0.1:50052".into(),
        friend_grpc_endpoint: "http://127.0.0.1:50053".into(),
        notification_grpc_endpoint: "http://127.0.0.1:50054".into(),
        post_grpc_endpoint: "http://127.0.0.1:50055".into(),
        refresh_cookie_domain: String::new(),
        refresh_cookie_secure: false,
        refresh_cookie_max_age_seconds: 1209600,
        aggregation_cache_ttl_seconds: 30,
        cors_allowed_origins: vec!["http://localhost:3000".into()],
        jwt_issuer: TEST_JWT_ISSUER.into(),
        jwt_audience: TEST_JWT_AUDIENCE.into(),
        jwt_jwks_json: test_jwt_keys().jwks_json.clone(),
        jwt_keys_refresh_seconds: 0,
        jwt_keys_min_refresh_seconds: 10,
        high_risk_session_freshness_seconds: 5,
        runtime_mode,
        developer_portal_links: api_gateway::config::DeveloperPortalLinks::defaults(runtime_mode),
        rate_limit_per_minute,
        admission: api_gateway::domain::admission::AdmissionPolicies::default(),
        admission_hmac_key: "test-admission-hmac-key-with-at-least-32-bytes".into(),
        search_cursor_hmac_key: "test-search-cursor-key-has-32-bytes-minimum".into(),
        search_cursor_ttl_seconds: 900,
        search_cursor_legacy_accept_until: None,
        request_timeout_ms,
        journal_client_timeout_ms: 900,
        async_operation_keyring: None,
        telemetry_queue_capacity: 4096,
        telemetry_retention_days: 30,
        audit_retention_days: 365,
        actor_delegation_signer: platform::actor_delegation::ActorDelegationSigner::local_parity(
            platform::grpc_auth::Workload::Gateway,
        ),
        caller_registry: platform::grpc_auth::CallerRegistry::new(),
        authority_exchange: api_gateway::config::authority_exchange(
            false,
            &platform::actor_delegation::ActorDelegationSigner::local_parity(
                platform::grpc_auth::Workload::Gateway,
            ),
            false,
            platform::actor_delegation::exchange::ResourceEvidenceVerifier::local_parity(),
        )
        .expect("parity authority exchange"),
        outbound_identity: "test-internal-service-token".into(),
        tls_material: None,
    };
    let pool = test_support::postgres::with_startup_diagnostics(
        postgres.as_ref(),
        "Gateway fixture",
        connect_pool(&config),
    )
    .await?;
    let friend_client = Arc::new(FakeFriendClient::default());
    let post_client = Arc::new(FakePostClient::default());
    let profile_trace_contexts = Arc::new(Mutex::new(Vec::new()));
    let profile_client = Arc::new(FakeProfileClient {
        delay: profile_delay,
        trace_contexts: profile_trace_contexts.clone(),
        ..Default::default()
    });
    let profile_avatar_upload_calls = profile_client.avatar_upload_calls.clone();
    let state = Arc::new(
        build_state_with_clients(
            &config,
            pool.clone(),
            Arc::new(FakeAuthClient) as Arc<dyn AuthClientPort>,
            profile_client as Arc<dyn ProfileClientPort>,
            friend_client.clone() as Arc<dyn FriendClientPort>,
            Arc::new(FakeNotificationClient) as Arc<dyn NotificationClientPort>,
            post_client.clone() as Arc<dyn PostClientPort>,
        )
        .await?,
    );
    let router = api_gateway::adapters::r#in::http::router(state.clone());
    let cleanup = TestAppCleanup {
        postgres,
        postgres_permit: Some(postgres_permit),
        pool: pool.clone(),
        inject_removal_failure,
    };
    Ok(TestApp {
        config,
        pool,
        state,
        router,
        friend_client,
        post_client: post_client.clone(),
        profile_avatar_upload_calls,
        profile_trace_contexts,
        owner_receipt_statuses: post_client.owner_receipt_statuses.clone(),
        owner_receipt_lookup_available: post_client.owner_receipt_lookup_available.clone(),
        _cleanup: cleanup,
    })
}

pub fn unsigned_bearer_token(user_id: &str, roles: &[&str]) -> String {
    let header = Base64UrlUnpadded::encode_string(br#"{"alg":"none","typ":"JWT"}"#);
    let payload = serde_json::to_vec(&serde_json::json!({
        "sub": user_id,
        "roles": roles,
    }))
    .expect("payload");
    let payload = Base64UrlUnpadded::encode_string(&payload);
    format!("{header}.{payload}.signature")
}

pub fn bearer_token(user_id: &str, roles: &[&str]) -> String {
    let now = Utc::now().timestamp();
    let claims = TestAccessClaims {
        sub: user_id.to_owned(),
        iss: TEST_JWT_ISSUER,
        aud: TEST_JWT_AUDIENCE,
        iat: now,
        exp: now + 900,
        jti: Uuid::new_v4().to_string(),
        token_type: "access",
        roles: roles.iter().map(|role| (*role).to_owned()).collect(),
        sid: TEST_SESSION_ID,
    };
    let header = Header {
        alg: Algorithm::RS256,
        kid: Some(TEST_JWT_KID.to_owned()),
        ..Header::default()
    };
    jsonwebtoken::encode(&header, &claims, &test_jwt_keys().encoding).expect("signed token")
}

#[derive(Serialize)]
struct TestAccessClaims<'a> {
    sub: String,
    iss: &'a str,
    aud: &'a str,
    iat: i64,
    exp: i64,
    jti: String,
    token_type: &'a str,
    roles: Vec<String>,
    sid: &'a str,
}

struct TestJwtKeys {
    encoding: EncodingKey,
    jwks_json: String,
}

fn test_jwt_keys() -> &'static TestJwtKeys {
    static KEYS: OnceLock<TestJwtKeys> = OnceLock::new();
    KEYS.get_or_init(|| {
        let private_key = RsaPrivateKey::new(&mut OsRng, 2048).expect("test RSA key");
        let public_key = RsaPublicKey::from(&private_key);
        let private_pem = private_key
            .to_pkcs8_pem(Default::default())
            .expect("private pem");
        let jwks_json = serde_json::json!({
            "keys": [{
                "kty": "RSA",
                "use": "sig",
                "alg": "RS256",
                "kid": TEST_JWT_KID,
                "n": Base64UrlUnpadded::encode_string(&public_key.n().to_bytes_be()),
                "e": Base64UrlUnpadded::encode_string(&public_key.e().to_bytes_be())
            }]
        })
        .to_string();
        TestJwtKeys {
            encoding: EncodingKey::from_rsa_pem(private_pem.as_bytes()).expect("encoding key"),
            jwks_json,
        }
    })
}

#[derive(Clone, Default)]
struct FakeAuthClient;

impl FakeAuthClient {
    async fn signup_auth_client_port_inner(
        &self,
        _request: EmailSignupRequest,
    ) -> anyhow::Result<SignupResponse> {
        Ok(SignupResponse {
            user_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111")?,
            status: "PENDING_VERIFICATION".into(),
        })
    }

    async fn verify_auth_client_port_inner(
        &self,
        _request: EmailVerifyRequest,
    ) -> anyhow::Result<SignupResponse> {
        Ok(SignupResponse {
            user_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111")?,
            status: "ACTIVE".into(),
        })
    }

    async fn check_email_auth_client_port_inner(
        &self,
        request: EmailCheckRequest,
    ) -> anyhow::Result<EmailAvailabilityResponse> {
        Ok(EmailAvailabilityResponse {
            email: request.email,
            available: true,
            status: "AVAILABLE".into(),
        })
    }

    async fn request_password_reset_auth_client_port_inner(
        &self,
        _request: PasswordResetRequest,
    ) -> anyhow::Result<PasswordResetResponse> {
        Ok(PasswordResetResponse {
            status: "RESET_REQUESTED".into(),
        })
    }

    async fn confirm_password_reset_auth_client_port_inner(
        &self,
        _request: PasswordResetConfirmRequest,
    ) -> anyhow::Result<PasswordResetResponse> {
        Ok(PasswordResetResponse {
            status: "RESET_CONFIRMED".into(),
        })
    }

    async fn login_auth_client_port_inner(
        &self,
        _request: LoginRequest,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> anyhow::Result<AuthResponse> {
        Ok(base_auth_response(vec![IdentityProvider::Email]))
    }

    async fn refresh_auth_client_port_inner(
        &self,
        _refresh_token: String,
    ) -> anyhow::Result<AuthResponse> {
        Ok(base_auth_response(vec![IdentityProvider::Email]))
    }

    async fn logout_auth_client_port_inner(&self, _refresh_token: String) -> anyhow::Result<()> {
        Ok(())
    }

    async fn authorize_auth_client_port_inner(
        &self,
        provider: IdentityProvider,
    ) -> anyhow::Result<api_gateway::application::dto::auth_dto::OAuthAuthorizationDto> {
        let state = match provider {
            IdentityProvider::Google => "google-state",
            IdentityProvider::Apple => "apple-state",
            IdentityProvider::X => "x-state",
            IdentityProvider::Email => "email-state",
        };
        Ok(
            api_gateway::application::dto::auth_dto::OAuthAuthorizationDto {
                authorization_uri: format!("https://accounts.example.test/oauth?state={state}")
                    .parse::<::http::Uri>()?
                    .to_string(),
            },
        )
    }

    async fn oauth_callback_auth_client_port_inner(
        &self,
        provider: IdentityProvider,
        _code: String,
        _state: String,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> anyhow::Result<AuthResponse> {
        Ok(base_auth_response(vec![IdentityProvider::Email, provider]))
    }

    async fn link_provider_auth_client_port_inner(
        &self,
        provider: IdentityProvider,
        _request: OAuthLinkRequest,
        user_id: String,
    ) -> anyhow::Result<LinkedProvidersResponse> {
        Ok(LinkedProvidersResponse {
            user_id: user_id.parse()?,
            providers: vec![IdentityProvider::Email, provider],
        })
    }

    /// Every session is live and holds both roles, so the token's claims
    /// decide, as they did before sessions were checked.
    async fn session_state_auth_client_port_inner(
        &self,
        _request: &api_gateway::application::dto::issuer_dto::SessionAuthorityRequestDto,
    ) -> anyhow::Result<api_gateway::application::dto::issuer_dto::SessionStateDto> {
        Ok(
            api_gateway::application::dto::issuer_dto::SessionStateDto::Active {
                roles: vec!["ROLE_USER".into(), "ROLE_ADMIN".into()],
            },
        )
    }

    // Explicit recording behavior formerly inherited from the port.
    async fn delete_account_auth_client_port_inner(
        &self,
        _user_id: String,
        _request_id: Option<String>,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> anyhow::Result<api_gateway::application::dto::auth_dto::DeleteAccountResponse> {
        anyhow::bail!("Account deletion is not implemented")
    }

    async fn get_user_deletion_auth_client_port_inner(
        &self,
        _user_id: String,
    ) -> anyhow::Result<Option<api_gateway::application::dto::auth_dto::UserDeletionSagaResponse>>
    {
        anyhow::bail!("Account deletion status is not implemented")
    }

    async fn get_user_deletion_by_saga_id_auth_client_port_inner(
        &self,
        _saga_id: String,
    ) -> anyhow::Result<Option<api_gateway::application::dto::auth_dto::UserDeletionSagaResponse>>
    {
        anyhow::bail!("Account deletion saga lookup is not implemented")
    }

    async fn list_dead_lettered_user_deletions_auth_client_port_inner(
        &self,
        _limit: i32,
        _after_updated_at: Option<chrono::DateTime<chrono::Utc>>,
        _after_saga_id: Option<uuid::Uuid>,
    ) -> anyhow::Result<api_gateway::application::dto::auth_dto::DeadLetteredUserDeletionListResponse>
    {
        anyhow::bail!("Dead-lettered account deletion listing is not implemented")
    }

    async fn retry_user_deletion_auth_client_port_inner(
        &self,
        _request: api_gateway::application::dto::auth_dto::RetryUserDeletionRequestDto,
    ) -> anyhow::Result<api_gateway::application::dto::auth_dto::UserDeletionSagaResponse> {
        anyhow::bail!("Account deletion recovery is not implemented")
    }
}

#[async_trait]
impl AuthClientPort for FakeAuthClient {
    async fn signup(
        &self,
        _request: EmailSignupRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<SignupResponse> {
        self.signup_auth_client_port_inner(_request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn verify(
        &self,
        _request: EmailVerifyRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<SignupResponse> {
        self.verify_auth_client_port_inner(_request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn check_email(
        &self,
        request: EmailCheckRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<EmailAvailabilityResponse>
    {
        self.check_email_auth_client_port_inner(request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn request_password_reset(
        &self,
        _request: PasswordResetRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<PasswordResetResponse> {
        self.request_password_reset_auth_client_port_inner(_request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn confirm_password_reset(
        &self,
        _request: PasswordResetConfirmRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<PasswordResetResponse> {
        self.confirm_password_reset_auth_client_port_inner(_request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn login(
        &self,
        _request: LoginRequest,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<AuthResponse> {
        self.login_auth_client_port_inner(_request, _ip_address, _user_agent)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn refresh(
        &self,
        _refresh_token: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<AuthResponse> {
        self.refresh_auth_client_port_inner(_refresh_token)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn logout(
        &self,
        _refresh_token: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<()> {
        self.logout_auth_client_port_inner(_refresh_token)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn authorize(
        &self,
        provider: api_gateway::application::dto::facade_dto::IdentityProviderDto,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::auth_dto::OAuthAuthorizationDto,
    > {
        self.authorize_auth_client_port_inner(provider.into())
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn oauth_callback(
        &self,
        provider: api_gateway::application::dto::facade_dto::IdentityProviderDto,
        _code: String,
        _state: String,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<AuthResponse> {
        self.oauth_callback_auth_client_port_inner(
            provider.into(),
            _code,
            _state,
            _ip_address,
            _user_agent,
        )
        .await
        .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn link_provider(
        &self,
        provider: api_gateway::application::dto::facade_dto::IdentityProviderDto,
        _request: OAuthLinkRequest,
        user_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<LinkedProvidersResponse> {
        self.link_provider_auth_client_port_inner(provider.into(), _request, user_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn session_state(
        &self,
        _request: &api_gateway::application::dto::issuer_dto::SessionAuthorityRequestDto,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::issuer_dto::SessionStateDto,
    > {
        self.session_state_auth_client_port_inner(_request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn delete_account(
        &self,
        _user_id: String,
        _request_id: Option<String>,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::auth_dto::DeleteAccountResponse,
    > {
        self.delete_account_auth_client_port_inner(_user_id, _request_id, _ip_address, _user_agent)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_user_deletion(
        &self,
        _user_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        Option<api_gateway::application::dto::auth_dto::UserDeletionSagaResponse>,
    > {
        self.get_user_deletion_auth_client_port_inner(_user_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_user_deletion_by_saga_id(
        &self,
        _saga_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        Option<api_gateway::application::dto::auth_dto::UserDeletionSagaResponse>,
    > {
        self.get_user_deletion_by_saga_id_auth_client_port_inner(_saga_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn list_dead_lettered_user_deletions(
        &self,
        _limit: i32,
        _after_updated_at: Option<chrono::DateTime<chrono::Utc>>,
        _after_saga_id: Option<uuid::Uuid>,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::auth_dto::DeadLetteredUserDeletionListResponse,
    > {
        self.list_dead_lettered_user_deletions_auth_client_port_inner(
            _limit,
            _after_updated_at,
            _after_saga_id,
        )
        .await
        .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn retry_user_deletion(
        &self,
        _request: api_gateway::application::dto::auth_dto::RetryUserDeletionRequestDto,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::auth_dto::UserDeletionSagaResponse,
    > {
        self.retry_user_deletion_auth_client_port_inner(_request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
}

#[derive(Clone, Default)]
struct FakeProfileClient {
    delay: Option<Duration>,
    avatar_upload_calls: Arc<AtomicUsize>,
    trace_contexts: CapturedTraceContexts,
}

impl FakeProfileClient {
    async fn maybe_delay(&self) {
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
    }
}

impl FakeProfileClient {
    async fn get_profile_profile_client_port_inner(
        &self,
        user_id: String,
    ) -> anyhow::Result<ProfileResponse> {
        self.maybe_delay().await;
        Ok(base_profile(&user_id))
    }

    async fn get_profile_by_username_profile_client_port_inner(
        &self,
        username: String,
    ) -> anyhow::Result<ProfileResponse> {
        let user_id = if username == "hana_ani" {
            "11111111-1111-1111-1111-111111111111"
        } else {
            "22222222-2222-2222-2222-222222222222"
        };
        Ok(base_profile(user_id))
    }

    async fn get_profiles_profile_client_port_inner(
        &self,
        user_ids: Vec<String>,
    ) -> anyhow::Result<ProfileBatchResponse> {
        Ok(ProfileBatchResponse {
            profiles: user_ids
                .iter()
                .map(|user_id| base_profile(user_id))
                .collect(),
        })
    }

    async fn search_profiles_profile_client_port_inner(
        &self,
        query: String,
        cursor: Option<String>,
        limit: i32,
    ) -> anyhow::Result<ProfileSearchResponse> {
        let profiles = if query.to_ascii_lowercase().contains("muted") {
            vec![base_profile("44444444-4444-4444-4444-444444444444")]
        } else if query.to_ascii_lowercase().contains("hana") {
            vec![base_profile("11111111-1111-1111-1111-111111111111")]
        } else {
            vec![base_profile("22222222-2222-2222-2222-222222222222")]
        };
        // A real repository offers a cursor only when rows remain beyond the batch
        // it returned. Echoing the request cursor back would claim an endless result
        // set, and the gateway now treats an unmoved cursor as the end of the scan,
        // so an echoing fixture would silently stop exercising the search path.
        let more_remain = cursor.is_none() && profiles.len() == 1 && limit == 1;
        Ok(ProfileSearchResponse {
            profiles,
            next_cursor: more_remain.then(|| "profiles_next".to_owned()),
        })
    }

    async fn upload_avatar_profile_client_port_inner(
        &self,
        user_id: String,
        _content: Vec<u8>,
        _content_type: String,
        _filename: String,
    ) -> anyhow::Result<String> {
        self.avatar_upload_calls.fetch_add(1, Ordering::SeqCst);
        Ok(format!("/api/profile/avatar/{user_id}"))
    }

    async fn get_avatar_profile_client_port_inner(
        &self,
        _user_id: String,
    ) -> anyhow::Result<AvatarContent> {
        Ok(AvatarContent {
            content: b"avatar-bytes".to_vec(),
            content_type: "image/png".into(),
        })
    }

    async fn update_profile_profile_client_port_inner(
        &self,
        user_id: String,
        request: ProfileUpdateRequest,
    ) -> anyhow::Result<ProfileResponse> {
        self.trace_contexts
            .lock()
            .expect("fake profile trace contexts")
            .push(platform::trace_context::current());
        self.maybe_delay().await;
        Ok(ProfileResponse {
            user_id,
            username: request.username,
            nickname: request.nickname,
            avatar_url: request.avatar_url,
            bio: request.bio,
            follower_count: 0,
            following_count: 0,
            post_count: 9,
            account_visibility: "PUBLIC".into(),
            lifecycle_status: "ACTIVE".into(),
        })
    }

    async fn delete_profile_profile_client_port_inner(
        &self,
        user_id: String,
        _request_id: Option<String>,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> anyhow::Result<DeleteProfileResponse> {
        Ok(DeleteProfileResponse {
            user_id,
            deleted: true,
            account_deleted: false,
        })
    }

    // Explicit recording behavior formerly inherited from the port.
    async fn get_mutation_receipt_status_profile_client_port_inner(
        &self,
        _operation_id: uuid::Uuid,
    ) -> anyhow::Result<Option<api_gateway::application::dto::post_dto::MutationReceiptStatusDto>>
    {
        anyhow::bail!("Profile mutation status lookup is unavailable")
    }

    async fn update_profile_privacy_profile_client_port_inner(
        &self,
        _user_id: String,
        _request: api_gateway::application::dto::profile_dto::ProfilePrivacyUpdateRequest,
    ) -> anyhow::Result<api_gateway::application::dto::profile_dto::ProfileResponse> {
        anyhow::bail!("Profile privacy operation is not implemented")
    }
}

#[async_trait]
impl ProfileClientPort for FakeProfileClient {
    async fn get_profile(
        &self,
        user_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<ProfileResponse> {
        self.get_profile_profile_client_port_inner(user_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_profile_by_username(
        &self,
        username: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<ProfileResponse> {
        self.get_profile_by_username_profile_client_port_inner(username)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_profiles(
        &self,
        user_ids: Vec<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<ProfileBatchResponse> {
        self.get_profiles_profile_client_port_inner(user_ids)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn search_profiles(
        &self,
        query: String,
        cursor: Option<String>,
        limit: i32,
    ) -> api_gateway::application::client_error::GatewayClientResult<ProfileSearchResponse> {
        self.search_profiles_profile_client_port_inner(query, cursor, limit)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn upload_avatar(
        &self,
        user_id: String,
        _content: Vec<u8>,
        _content_type: String,
        _filename: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<String> {
        self.upload_avatar_profile_client_port_inner(user_id, _content, _content_type, _filename)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_avatar(
        &self,
        _user_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<AvatarContent> {
        self.get_avatar_profile_client_port_inner(_user_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn update_profile(
        &self,
        user_id: String,
        request: ProfileUpdateRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<ProfileResponse> {
        self.update_profile_profile_client_port_inner(user_id, request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn delete_profile(
        &self,
        user_id: String,
        _request_id: Option<String>,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<DeleteProfileResponse> {
        self.delete_profile_profile_client_port_inner(
            user_id,
            _request_id,
            _ip_address,
            _user_agent,
        )
        .await
        .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_mutation_receipt_status(
        &self,
        _operation_id: uuid::Uuid,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        Option<api_gateway::application::dto::post_dto::MutationReceiptStatusDto>,
    > {
        self.get_mutation_receipt_status_profile_client_port_inner(_operation_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn update_profile_privacy(
        &self,
        _user_id: String,
        _request: api_gateway::application::dto::profile_dto::ProfilePrivacyUpdateRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::profile_dto::ProfileResponse,
    > {
        self.update_profile_privacy_profile_client_port_inner(_user_id, _request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
}

#[derive(Default)]
pub struct FakeFriendClient {
    count_calls: Mutex<Vec<String>>,
    default_relationship_policy: Mutex<RelationshipPolicyResponse>,
    relationship_policies: Mutex<HashMap<(String, String), RelationshipPolicyResponse>>,
    unavailable_relationships: Mutex<HashSet<(String, String)>>,
    relationship_calls: Mutex<Vec<(String, String)>>,
}

impl FakeFriendClient {
    pub fn count_calls(&self) -> Vec<String> {
        self.count_calls.lock().unwrap().clone()
    }

    pub fn set_default_relationship_policy(&self, policy: RelationshipPolicyResponse) {
        *self.default_relationship_policy.lock().unwrap() = policy;
    }

    pub fn set_relationship_policy(
        &self,
        viewer_id: &str,
        target_id: &str,
        policy: RelationshipPolicyResponse,
    ) {
        self.relationship_policies
            .lock()
            .unwrap()
            .insert((viewer_id.to_owned(), target_id.to_owned()), policy);
    }

    pub fn set_relationship_unavailable(&self, viewer_id: &str, target_id: &str) {
        self.unavailable_relationships
            .lock()
            .unwrap()
            .insert((viewer_id.to_owned(), target_id.to_owned()));
    }

    pub fn relationship_calls(&self) -> Vec<(String, String)> {
        self.relationship_calls.lock().unwrap().clone()
    }
}

impl FakeFriendClient {
    async fn follow_friend_client_port_inner(
        &self,
        _follower_id: String,
        _followee_id: String,
    ) -> anyhow::Result<FollowActionResponse> {
        Ok(FollowActionResponse {
            following: true,
            followed_at: Some("2026-03-12T00:00:00Z".into()),
            status: "ACTIVE".into(),
            request_id: None,
        })
    }

    async fn unfollow_friend_client_port_inner(
        &self,
        _follower_id: String,
        _followee_id: String,
    ) -> anyhow::Result<FollowActionResponse> {
        Ok(FollowActionResponse {
            following: false,
            followed_at: None,
            status: "INACTIVE".into(),
            request_id: None,
        })
    }

    async fn list_followers_friend_client_port_inner(
        &self,
        _user_id: String,
        cursor: Option<String>,
        _limit: Option<i32>,
    ) -> anyhow::Result<FollowListResponse> {
        Ok(FollowListResponse {
            edges: vec![FollowEdgeResponse {
                user_id: "22222222-2222-2222-2222-222222222222".into(),
                followed_at: "2026-03-12T00:00:00Z".into(),
            }],
            next_cursor: cursor,
            total_count: 1,
        })
    }

    async fn list_following_friend_client_port_inner(
        &self,
        _user_id: String,
        cursor: Option<String>,
        _limit: Option<i32>,
    ) -> anyhow::Result<FollowListResponse> {
        Ok(FollowListResponse {
            edges: vec![FollowEdgeResponse {
                user_id: "33333333-3333-3333-3333-333333333333".into(),
                followed_at: "2026-03-12T00:00:00Z".into(),
            }],
            next_cursor: cursor,
            total_count: 1,
        })
    }

    async fn get_counts_friend_client_port_inner(
        &self,
        user_id: String,
    ) -> anyhow::Result<FollowCountsResponse> {
        self.count_calls.lock().unwrap().push(user_id.clone());
        Ok(counts_for_user(&user_id))
    }

    async fn get_counts_batch_friend_client_port_inner(
        &self,
        user_ids: Vec<String>,
    ) -> anyhow::Result<HashMap<String, FollowCountsResponse>> {
        Ok(user_ids
            .into_iter()
            .map(|user_id| {
                let counts = counts_for_user(&user_id);
                (user_id, counts)
            })
            .collect())
    }

    async fn check_following_friend_client_port_inner(
        &self,
        _follower_id: String,
        _followee_id: String,
    ) -> anyhow::Result<FollowStatusResponse> {
        Ok(FollowStatusResponse { following: true })
    }

    async fn get_relationship_friend_client_port_inner(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> anyhow::Result<RelationshipPolicyResponse> {
        self.relationship_calls
            .lock()
            .unwrap()
            .push((viewer_id.clone(), target_id.clone()));
        if self
            .unavailable_relationships
            .lock()
            .unwrap()
            .contains(&(viewer_id.clone(), target_id.clone()))
        {
            anyhow::bail!("Friend relationship service unavailable");
        }
        Ok(self
            .relationship_policies
            .lock()
            .unwrap()
            .get(&(viewer_id, target_id))
            .copied()
            .unwrap_or_else(|| *self.default_relationship_policy.lock().unwrap()))
    }

    // Explicit recording behavior formerly inherited from the port.
    async fn get_mutation_receipt_status_friend_client_port_inner(
        &self,
        _operation_id: uuid::Uuid,
    ) -> anyhow::Result<Option<api_gateway::application::dto::post_dto::MutationReceiptStatusDto>>
    {
        anyhow::bail!("Friend mutation status lookup is unavailable")
    }

    async fn accept_follow_request_friend_client_port_inner(
        &self,
        _request_id: String,
        _actor_id: String,
    ) -> anyhow::Result<api_gateway::application::dto::friend_dto::FollowActionResponse> {
        anyhow::bail!("Follow request operation is not implemented")
    }

    async fn reject_follow_request_friend_client_port_inner(
        &self,
        _request_id: String,
        _actor_id: String,
    ) -> anyhow::Result<api_gateway::application::dto::friend_dto::FollowRequestResponse> {
        anyhow::bail!("Follow request operation is not implemented")
    }

    async fn cancel_follow_request_friend_client_port_inner(
        &self,
        _request_id: String,
        _actor_id: String,
    ) -> anyhow::Result<api_gateway::application::dto::friend_dto::FollowRequestResponse> {
        anyhow::bail!("Follow request operation is not implemented")
    }

    async fn list_incoming_follow_requests_friend_client_port_inner(
        &self,
        _user_id: String,
        _cursor: Option<String>,
        _limit: i32,
    ) -> anyhow::Result<api_gateway::application::dto::friend_dto::FollowRequestPageResponse> {
        anyhow::bail!("Follow request operation is not implemented")
    }

    async fn list_outgoing_follow_requests_friend_client_port_inner(
        &self,
        _user_id: String,
        _cursor: Option<String>,
        _limit: i32,
    ) -> anyhow::Result<api_gateway::application::dto::friend_dto::FollowRequestPageResponse> {
        anyhow::bail!("Follow request operation is not implemented")
    }

    async fn block_user_friend_client_port_inner(
        &self,
        _viewer_id: String,
        _target_id: String,
    ) -> anyhow::Result<api_gateway::application::dto::friend_dto::RelationshipPolicyResponse> {
        anyhow::bail!("Relationship operation is not implemented")
    }

    async fn unblock_user_friend_client_port_inner(
        &self,
        _viewer_id: String,
        _target_id: String,
    ) -> anyhow::Result<api_gateway::application::dto::friend_dto::RelationshipPolicyResponse> {
        anyhow::bail!("Relationship operation is not implemented")
    }

    async fn mute_user_friend_client_port_inner(
        &self,
        _viewer_id: String,
        _target_id: String,
    ) -> anyhow::Result<api_gateway::application::dto::friend_dto::RelationshipPolicyResponse> {
        anyhow::bail!("Relationship operation is not implemented")
    }

    async fn unmute_user_friend_client_port_inner(
        &self,
        _viewer_id: String,
        _target_id: String,
    ) -> anyhow::Result<api_gateway::application::dto::friend_dto::RelationshipPolicyResponse> {
        anyhow::bail!("Relationship operation is not implemented")
    }

    async fn get_people_recommendations_friend_client_port_inner(
        &self,
        _viewer_id: String,
        _cursor: Option<String>,
        _limit: i32,
        _request_id: String,
    ) -> anyhow::Result<api_gateway::application::dto::friend_dto::PeopleRecommendationPageResponse>
    {
        anyhow::bail!("People recommendations are not implemented")
    }

    async fn record_people_recommendation_event_friend_client_port_inner(
        &self,
        _viewer_id: String,
        _request_id: String,
        _event: api_gateway::application::dto::friend_dto::PeopleRecommendationEventRequest,
    ) -> anyhow::Result<api_gateway::application::dto::friend_dto::RecommendationEventResponse>
    {
        anyhow::bail!("People recommendation events are not implemented")
    }

    async fn get_relationships_batch_friend_client_port_inner(
        &self,
        viewer_id: String,
        target_ids: Vec<String>,
    ) -> anyhow::Result<
        std::collections::HashMap<
            String,
            api_gateway::application::dto::friend_dto::RelationshipPolicyResponse,
        >,
    > {
        if target_ids.len() > 100 {
            anyhow::bail!("Relationship batch supports at most 100 user IDs");
        }
        let mut policies = std::collections::HashMap::new();
        for target_id in target_ids {
            let policy = self
                .get_relationship(viewer_id.clone(), target_id.clone())
                .await?;
            policies.insert(target_id, policy);
        }
        Ok(policies)
    }
}

#[async_trait]
impl FriendClientPort for FakeFriendClient {
    async fn follow(
        &self,
        _follower_id: String,
        _followee_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowActionResponse> {
        self.follow_friend_client_port_inner(_follower_id, _followee_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn unfollow(
        &self,
        _follower_id: String,
        _followee_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowActionResponse> {
        self.unfollow_friend_client_port_inner(_follower_id, _followee_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn list_followers(
        &self,
        _user_id: String,
        cursor: Option<String>,
        _limit: Option<i32>,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowListResponse> {
        self.list_followers_friend_client_port_inner(_user_id, cursor, _limit)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn list_following(
        &self,
        _user_id: String,
        cursor: Option<String>,
        _limit: Option<i32>,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowListResponse> {
        self.list_following_friend_client_port_inner(_user_id, cursor, _limit)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_counts(
        &self,
        user_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowCountsResponse> {
        self.get_counts_friend_client_port_inner(user_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_counts_batch(
        &self,
        user_ids: Vec<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        HashMap<String, FollowCountsResponse>,
    > {
        self.get_counts_batch_friend_client_port_inner(user_ids)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn check_following(
        &self,
        _follower_id: String,
        _followee_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowStatusResponse> {
        self.check_following_friend_client_port_inner(_follower_id, _followee_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_relationship(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<RelationshipPolicyResponse>
    {
        self.get_relationship_friend_client_port_inner(viewer_id, target_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_mutation_receipt_status(
        &self,
        _operation_id: uuid::Uuid,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        Option<api_gateway::application::dto::post_dto::MutationReceiptStatusDto>,
    > {
        self.get_mutation_receipt_status_friend_client_port_inner(_operation_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn accept_follow_request(
        &self,
        _request_id: String,
        _actor_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::friend_dto::FollowActionResponse,
    > {
        self.accept_follow_request_friend_client_port_inner(_request_id, _actor_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn reject_follow_request(
        &self,
        _request_id: String,
        _actor_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::friend_dto::FollowRequestResponse,
    > {
        self.reject_follow_request_friend_client_port_inner(_request_id, _actor_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn cancel_follow_request(
        &self,
        _request_id: String,
        _actor_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::friend_dto::FollowRequestResponse,
    > {
        self.cancel_follow_request_friend_client_port_inner(_request_id, _actor_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn list_incoming_follow_requests(
        &self,
        _user_id: String,
        _cursor: Option<String>,
        _limit: i32,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::friend_dto::FollowRequestPageResponse,
    > {
        self.list_incoming_follow_requests_friend_client_port_inner(_user_id, _cursor, _limit)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn list_outgoing_follow_requests(
        &self,
        _user_id: String,
        _cursor: Option<String>,
        _limit: i32,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::friend_dto::FollowRequestPageResponse,
    > {
        self.list_outgoing_follow_requests_friend_client_port_inner(_user_id, _cursor, _limit)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn block_user(
        &self,
        _viewer_id: String,
        _target_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::friend_dto::RelationshipPolicyResponse,
    > {
        self.block_user_friend_client_port_inner(_viewer_id, _target_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn unblock_user(
        &self,
        _viewer_id: String,
        _target_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::friend_dto::RelationshipPolicyResponse,
    > {
        self.unblock_user_friend_client_port_inner(_viewer_id, _target_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn mute_user(
        &self,
        _viewer_id: String,
        _target_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::friend_dto::RelationshipPolicyResponse,
    > {
        self.mute_user_friend_client_port_inner(_viewer_id, _target_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn unmute_user(
        &self,
        _viewer_id: String,
        _target_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::friend_dto::RelationshipPolicyResponse,
    > {
        self.unmute_user_friend_client_port_inner(_viewer_id, _target_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_people_recommendations(
        &self,
        _viewer_id: String,
        _cursor: Option<String>,
        _limit: i32,
        _request_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::friend_dto::PeopleRecommendationPageResponse,
    > {
        self.get_people_recommendations_friend_client_port_inner(
            _viewer_id,
            _cursor,
            _limit,
            _request_id,
        )
        .await
        .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn record_people_recommendation_event(
        &self,
        _viewer_id: String,
        _request_id: String,
        _event: api_gateway::application::dto::friend_dto::PeopleRecommendationEventRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::friend_dto::RecommendationEventResponse,
    > {
        self.record_people_recommendation_event_friend_client_port_inner(
            _viewer_id,
            _request_id,
            _event,
        )
        .await
        .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_relationships_batch(
        &self,
        viewer_id: String,
        target_ids: Vec<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        std::collections::HashMap<
            String,
            api_gateway::application::dto::friend_dto::RelationshipPolicyResponse,
        >,
    > {
        self.get_relationships_batch_friend_client_port_inner(viewer_id, target_ids)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
}

#[derive(Clone, Default)]
struct FakeNotificationClient;

impl FakeNotificationClient {
    async fn list_notifications_notification_client_port_inner(
        &self,
        user_id: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> anyhow::Result<NotificationFeedResponse> {
        Ok(NotificationFeedResponse {
            items: vec![NotificationResponse {
                id: "notif-1".into(),
                user_id,
                notification_type: "POST_CREATED".into(),
                payload_json: r#"{"postId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"}"#.into(),
                created_at: "2026-03-12T00:00:00Z".into(),
                read_at: String::new(),
            }],
            next_cursor: cursor,
        })
    }

    async fn mark_read_notification_client_port_inner(
        &self,
        user_id: String,
        notification_id: String,
    ) -> anyhow::Result<NotificationResponse> {
        Ok(NotificationResponse {
            id: notification_id,
            user_id,
            notification_type: "POST_CREATED".into(),
            payload_json: r#"{"postId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"}"#.into(),
            created_at: "2026-03-12T00:00:00Z".into(),
            read_at: "2026-03-12T00:01:00Z".into(),
        })
    }

    async fn mark_all_read_notification_client_port_inner(
        &self,
        _user_id: String,
    ) -> anyhow::Result<NotificationMarkAllReadResponse> {
        Ok(NotificationMarkAllReadResponse { updated_count: 2 })
    }
}

#[async_trait]
impl NotificationClientPort for FakeNotificationClient {
    async fn list_notifications(
        &self,
        user_id: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> api_gateway::application::client_error::GatewayClientResult<NotificationFeedResponse> {
        self.list_notifications_notification_client_port_inner(user_id, cursor, _limit)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn mark_read(
        &self,
        user_id: String,
        notification_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<NotificationResponse> {
        self.mark_read_notification_client_port_inner(user_id, notification_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn mark_all_read(
        &self,
        _user_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<NotificationMarkAllReadResponse>
    {
        self.mark_all_read_notification_client_port_inner(_user_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
}

#[derive(Clone)]
pub struct FakePostClient {
    owner_receipt_statuses: Arc<Mutex<HashMap<Uuid, MutationReceiptStatusDto>>>,
    owner_receipt_lookup_available: Arc<AtomicBool>,
    owner_receipt_lookup_gate: Arc<Mutex<Option<Arc<OwnerReceiptLookupGate>>>>,
    owner_receipt_lookup_calls: Arc<AtomicUsize>,
    create_post_error: Arc<Mutex<Option<String>>>,
}

/// Holds a receipt lookup after the reconciler claims its journal row. The test
/// can make an operator decision while the owner answer is still in flight.
pub struct OwnerReceiptLookupGate {
    pub entered: tokio::sync::Semaphore,
    pub released: tokio::sync::Semaphore,
}

impl Default for FakePostClient {
    fn default() -> Self {
        Self {
            owner_receipt_statuses: Arc::new(Mutex::new(HashMap::new())),
            owner_receipt_lookup_available: Arc::new(AtomicBool::new(true)),
            owner_receipt_lookup_gate: Arc::new(Mutex::new(None)),
            owner_receipt_lookup_calls: Arc::new(AtomicUsize::new(0)),
            create_post_error: Arc::new(Mutex::new(None)),
        }
    }
}

impl FakePostClient {
    pub fn owner_receipt_lookup_calls(&self) -> usize {
        self.owner_receipt_lookup_calls.load(Ordering::SeqCst)
    }

    pub fn hold_owner_receipt_lookup(&self) -> Arc<OwnerReceiptLookupGate> {
        let gate = Arc::new(OwnerReceiptLookupGate {
            entered: tokio::sync::Semaphore::new(0),
            released: tokio::sync::Semaphore::new(0),
        });
        *self
            .owner_receipt_lookup_gate
            .lock()
            .expect("receipt lookup gate") = Some(gate.clone());
        gate
    }

    pub fn clear_owner_receipt_lookup_gate(&self) {
        *self
            .owner_receipt_lookup_gate
            .lock()
            .expect("receipt lookup gate") = None;
    }

    pub fn set_create_post_error(&self, message: impl Into<String>) {
        *self
            .create_post_error
            .lock()
            .expect("fake PostService error") = Some(message.into());
    }
}

impl FakePostClient {
    async fn notification_posts_visible_inner(
        &self,
        viewer_id: String,
        post_ids: Vec<String>,
        _max_duration: Duration,
    ) -> anyhow::Result<HashMap<String, bool>> {
        let viewer_valid = Uuid::parse_str(&viewer_id).is_ok();
        Ok(post_ids
            .into_iter()
            .map(|id| {
                let visible = viewer_valid && id == "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
                (id, visible)
            })
            .collect())
    }

    async fn get_mutation_receipt_status_inner(
        &self,
        operation_id: Uuid,
    ) -> anyhow::Result<Option<MutationReceiptStatusDto>> {
        self.owner_receipt_lookup_calls
            .fetch_add(1, Ordering::SeqCst);
        let gate = self
            .owner_receipt_lookup_gate
            .lock()
            .expect("receipt lookup gate")
            .clone();
        if let Some(gate) = gate {
            gate.entered.add_permits(1);
            gate.released.acquire().await?.forget();
        }
        if !self.owner_receipt_lookup_available.load(Ordering::SeqCst) {
            anyhow::bail!("fake owner receipt lookup unavailable")
        }
        Ok(self
            .owner_receipt_statuses
            .lock()
            .expect("fake owner receipt status")
            .get(&operation_id)
            .cloned())
    }

    async fn get_author_post_counts_inner(
        &self,
        _author_ids: Vec<String>,
    ) -> anyhow::Result<std::collections::HashMap<String, i64>> {
        Ok(std::collections::HashMap::new())
    }

    async fn get_feed_inner(
        &self,
        _user_id: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> anyhow::Result<FeedResponse> {
        Ok(FeedResponse {
            posts: vec![base_post("post-1")],
            next_cursor: cursor,
        })
    }

    async fn get_author_posts_inner(
        &self,
        author_id: String,
        _viewer_id: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> anyhow::Result<FeedResponse> {
        Ok(FeedResponse {
            posts: vec![PostResponse {
                author_id,
                ..base_post("post-author")
            }],
            next_cursor: cursor,
        })
    }

    async fn search_posts_inner(
        &self,
        _viewer_id: String,
        _query: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> anyhow::Result<FeedResponse> {
        Ok(FeedResponse {
            posts: vec![base_post("post-search")],
            next_cursor: cursor,
        })
    }

    async fn get_trending_inner(
        &self,
        _viewer_id: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> anyhow::Result<FeedResponse> {
        Ok(FeedResponse {
            posts: vec![base_post("post-trending")],
            next_cursor: cursor,
        })
    }

    async fn get_post_inner(
        &self,
        _viewer_id: String,
        post_id: String,
    ) -> anyhow::Result<PostDetailResponse> {
        Ok(PostDetailResponse {
            post: base_post(&post_id),
            comments: vec![CommentResponse {
                id: "comment-1".into(),
                post_id,
                author_id: "22222222-2222-2222-2222-222222222222".into(),
                content: "Nice post".into(),
                created_at: "2026-03-12T00:00:00Z".into(),
            }],
        })
    }

    async fn create_post_inner(
        &self,
        user_id: String,
        request: PostCreateRequest,
    ) -> anyhow::Result<PostResponse> {
        // A fake owner call models the same conservative boundary as the real
        // gRPC adapter's `mutation_context::attach` before it dispatches.
        if let Some(context) = platform::mutation_context::current() {
            context.mark_dispatch_may_have_occurred();
        }
        if let Some(message) = self
            .create_post_error
            .lock()
            .expect("fake PostService error")
            .clone()
        {
            anyhow::bail!(message);
        }
        Ok(PostResponse {
            id: "post-new".into(),
            author_id: user_id,
            content: request.content,
            visibility: request.visibility,
            created_at: "2026-03-12T00:00:00Z".into(),
            comment_count: 0,
            like_count: 0,
            liked_by_me: false,
        })
    }

    async fn delete_post_inner(
        &self,
        _user_id: String,
        post_id: String,
        _request_id: Option<String>,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> anyhow::Result<DeletePostResponse> {
        Ok(DeletePostResponse {
            post_id,
            deleted: true,
        })
    }

    async fn create_comment_inner(
        &self,
        user_id: String,
        post_id: String,
        request: CommentCreateRequest,
    ) -> anyhow::Result<CommentResponse> {
        Ok(CommentResponse {
            id: "comment-new".into(),
            post_id,
            author_id: user_id,
            content: request.content,
            created_at: "2026-03-12T00:00:00Z".into(),
        })
    }

    async fn like_post_inner(
        &self,
        _user_id: String,
        post_id: String,
    ) -> anyhow::Result<PostLikeResponse> {
        Ok(PostLikeResponse {
            post_id,
            like_count: 11,
            liked: true,
        })
    }

    async fn unlike_post_inner(
        &self,
        _user_id: String,
        post_id: String,
    ) -> anyhow::Result<PostLikeResponse> {
        Ok(PostLikeResponse {
            post_id,
            like_count: 10,
            liked: false,
        })
    }

    // Explicit recording behavior formerly inherited from the port.
    async fn get_ranked_feed_inner(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
        request_id: String,
        _session_id: Option<String>,
    ) -> anyhow::Result<api_gateway::application::dto::post_dto::RankedFeedResponse> {
        let feed = self.get_feed_inner(user_id, cursor, limit).await?;
        Ok(
            api_gateway::application::dto::post_dto::RankedFeedResponse::chronological_fallback(
                feed, request_id,
            ),
        )
    }

    async fn record_feed_recommendation_event_inner(
        &self,
        _viewer_id: String,
        _request_id: String,
        _event: api_gateway::application::dto::post_dto::FeedRecommendationEventRequest,
    ) -> anyhow::Result<api_gateway::application::dto::post_dto::FeedRecommendationEventResponse>
    {
        anyhow::bail!("Feed recommendation events are not implemented")
    }

    async fn apply_content_moderation_inner(
        &self,
        _report_id: String,
        _target_type: String,
        _target_id: String,
        _actor_id: String,
        _action: String,
    ) -> anyhow::Result<bool> {
        anyhow::bail!("Content moderation is not implemented")
    }

    async fn resolve_dead_letter_replay_inner(
        &self,
        _request: api_gateway::application::dto::post_dto::ResolveDeadLetterReplayRequestDto,
    ) -> anyhow::Result<api_gateway::application::dto::post_dto::ResolveDeadLetterReplayResponseDto>
    {
        anyhow::bail!("Audited dead-letter replay is not implemented")
    }
}

#[async_trait]
impl PostClientPort for FakePostClient {
    async fn notification_posts_visible(
        &self,
        viewer_id: String,
        post_ids: Vec<String>,
        _max_duration: Duration,
    ) -> api_gateway::application::post_client_error::PostClientResult<HashMap<String, bool>> {
        self.notification_posts_visible_inner(viewer_id, post_ids, _max_duration)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_mutation_receipt_status(
        &self,
        operation_id: Uuid,
    ) -> api_gateway::application::post_client_error::PostClientResult<
        Option<MutationReceiptStatusDto>,
    > {
        self.get_mutation_receipt_status_inner(operation_id)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_author_post_counts(
        &self,
        _author_ids: Vec<String>,
    ) -> api_gateway::application::post_client_error::PostClientResult<
        std::collections::HashMap<String, i64>,
    > {
        self.get_author_post_counts_inner(_author_ids)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_feed(
        &self,
        _user_id: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> api_gateway::application::post_client_error::PostClientResult<FeedResponse> {
        self.get_feed_inner(_user_id, cursor, _limit)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_author_posts(
        &self,
        author_id: String,
        _viewer_id: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> api_gateway::application::post_client_error::PostClientResult<FeedResponse> {
        self.get_author_posts_inner(author_id, _viewer_id, cursor, _limit)
            .await
            .map_err(classify_post_client_error)
    }
    async fn search_posts(
        &self,
        _viewer_id: String,
        _query: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> api_gateway::application::post_client_error::PostClientResult<FeedResponse> {
        self.search_posts_inner(_viewer_id, _query, cursor, _limit)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_trending(
        &self,
        _viewer_id: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> api_gateway::application::post_client_error::PostClientResult<FeedResponse> {
        self.get_trending_inner(_viewer_id, cursor, _limit)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_post(
        &self,
        _viewer_id: String,
        post_id: String,
    ) -> api_gateway::application::post_client_error::PostClientResult<PostDetailResponse> {
        self.get_post_inner(_viewer_id, post_id)
            .await
            .map_err(classify_post_client_error)
    }
    async fn create_post(
        &self,
        user_id: String,
        request: PostCreateRequest,
    ) -> api_gateway::application::post_client_error::PostClientResult<PostResponse> {
        self.create_post_inner(user_id, request)
            .await
            .map_err(classify_post_client_error)
    }
    async fn delete_post(
        &self,
        _user_id: String,
        post_id: String,
        _request_id: Option<String>,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> api_gateway::application::post_client_error::PostClientResult<DeletePostResponse> {
        self.delete_post_inner(_user_id, post_id, _request_id, _ip_address, _user_agent)
            .await
            .map_err(classify_post_client_error)
    }
    async fn create_comment(
        &self,
        user_id: String,
        post_id: String,
        request: CommentCreateRequest,
    ) -> api_gateway::application::post_client_error::PostClientResult<CommentResponse> {
        self.create_comment_inner(user_id, post_id, request)
            .await
            .map_err(classify_post_client_error)
    }
    async fn like_post(
        &self,
        _user_id: String,
        post_id: String,
    ) -> api_gateway::application::post_client_error::PostClientResult<PostLikeResponse> {
        self.like_post_inner(_user_id, post_id)
            .await
            .map_err(classify_post_client_error)
    }
    async fn unlike_post(
        &self,
        _user_id: String,
        post_id: String,
    ) -> api_gateway::application::post_client_error::PostClientResult<PostLikeResponse> {
        self.unlike_post_inner(_user_id, post_id)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_ranked_feed(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
        request_id: String,
        _session_id: Option<String>,
    ) -> api_gateway::application::post_client_error::PostClientResult<
        api_gateway::application::dto::post_dto::RankedFeedResponse,
    > {
        self.get_ranked_feed_inner(user_id, cursor, limit, request_id, _session_id)
            .await
            .map_err(classify_post_client_error)
    }
    async fn record_feed_recommendation_event(
        &self,
        _viewer_id: String,
        _request_id: String,
        _event: api_gateway::application::dto::post_dto::FeedRecommendationEventRequest,
    ) -> api_gateway::application::post_client_error::PostClientResult<
        api_gateway::application::dto::post_dto::FeedRecommendationEventResponse,
    > {
        self.record_feed_recommendation_event_inner(_viewer_id, _request_id, _event)
            .await
            .map_err(classify_post_client_error)
    }
    async fn apply_content_moderation(
        &self,
        _report_id: String,
        _target_type: String,
        _target_id: String,
        _actor_id: String,
        _action: String,
    ) -> api_gateway::application::post_client_error::PostClientResult<bool> {
        self.apply_content_moderation_inner(
            _report_id,
            _target_type,
            _target_id,
            _actor_id,
            _action,
        )
        .await
        .map_err(classify_post_client_error)
    }
    async fn resolve_dead_letter_replay(
        &self,
        _request: api_gateway::application::dto::post_dto::ResolveDeadLetterReplayRequestDto,
    ) -> api_gateway::application::post_client_error::PostClientResult<
        api_gateway::application::dto::post_dto::ResolveDeadLetterReplayResponseDto,
    > {
        self.resolve_dead_letter_replay_inner(_request)
            .await
            .map_err(classify_post_client_error)
    }
}

fn base_auth_response(linked_providers: Vec<IdentityProvider>) -> AuthResponse {
    AuthResponse {
        user_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid"),
        access_token: "access.token".into(),
        refresh_token: "refresh-token".into(),
        token_type: "Bearer".into(),
        expires_in: 900,
        linked_providers,
    }
}

fn base_profile(user_id: &str) -> ProfileResponse {
    ProfileResponse {
        user_id: user_id.into(),
        username: if user_id == "11111111-1111-1111-1111-111111111111" {
            "hana_ani".into()
        } else {
            "mira_ghibli".into()
        },
        nickname: "Hana Akiyama".into(),
        avatar_url: "/avatars/cat.svg".into(),
        bio: "Rust gateway profile".into(),
        follower_count: 0,
        following_count: 0,
        post_count: 9,
        account_visibility: "PUBLIC".into(),
        lifecycle_status: "ACTIVE".into(),
    }
}

fn counts_for_user(user_id: &str) -> FollowCountsResponse {
    if user_id == "11111111-1111-1111-1111-111111111111" {
        FollowCountsResponse {
            follower_count: 12,
            following_count: 7,
        }
    } else {
        FollowCountsResponse {
            follower_count: 3,
            following_count: 2,
        }
    }
}

fn base_post(post_id: &str) -> PostResponse {
    PostResponse {
        id: post_id.into(),
        author_id: "11111111-1111-1111-1111-111111111111".into(),
        content: "Rust gateway post".into(),
        visibility: "PUBLIC".into(),
        created_at: "2026-03-12T00:00:00Z".into(),
        comment_count: 1,
        like_count: 10,
        liked_by_me: true,
    }
}
