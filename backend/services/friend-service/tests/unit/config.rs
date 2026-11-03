use friend_service::adapters::out::grpc::profile_privacy_client::GrpcProfilePrivacyClient;
use friend_service::application::ports::{ProfileDiscoveryPort, ProfilePrivacyPort};
use friend_service::config::{build_state, build_state_with_dependencies, AppConfig};
use friend_service::domain::recommendation::ProfileDiscoverySnapshot;
use platform::config::LOCAL_PARITY_INTERNAL_GRPC_SERVICE_TOKEN;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

static ENV_LOCK: Mutex<()> = Mutex::new(());

const FRIEND_ENV_KEYS: [&str; 28] = [
    "FRIEND_RUNTIME_MODE",
    "ACTOR_DELEGATION_VERIFYING_KEYS",
    "GATEWAY_SERVICE_CREDENTIAL",
    "POST_SERVICE_CREDENTIAL",
    "AUTH_SERVICE_CREDENTIAL",
    "FRIEND_GRPC_ADDR",
    "FRIEND_HTTP_ADDR",
    "FRIEND_METRICS_ADDR",
    "FRIEND_DATABASE_URL",
    "FRIEND_DB_POOL_MAX_CONNECTIONS",
    "FRIEND_DB_POOL_ACQUIRE_TIMEOUT_MS",
    "FRIEND_PROFILE_GRPC_ENDPOINT",
    "FRIEND_SEED_ENABLED",
    "FRIEND_SERVICE_CREDENTIAL",
    "FRIEND_PEOPLE_EMBEDDING_SOURCE_ENABLED",
    "FRIEND_PEOPLE_RANKER_ADAPTER",
    "FRIEND_PEOPLE_RECENT_SEQUENCE_ENABLED",
    "FRIEND_PEOPLE_RECENT_SEQUENCE_LIMIT",
    "FRIEND_RECOMMENDATION_RETENTION_WORKER_ENABLED",
    "FRIEND_RECOMMENDATION_RETENTION_WORKER_INTERVAL_MS",
    "FRIEND_RECOMMENDATION_RETENTION_BATCH_SIZE",
    "FRIEND_RECOMMENDATION_EVENT_RETENTION_DAYS",
    "FRIEND_MUTATION_RECEIPT_RETENTION_WORKER_ENABLED",
    "FRIEND_MUTATION_RECEIPT_RETENTION_WORKER_INTERVAL_MS",
    "FRIEND_MUTATION_RECEIPT_RETENTION_BATCH_SIZE",
    "FRIEND_GRPC_TLS_CERT_PATH",
    "FRIEND_GRPC_TLS_KEY_PATH",
    "FRIEND_GRPC_TLS_CA_PATH",
];

#[test]
fn load_uses_parity_defaults_when_runtime_is_parity() {
    with_clean_friend_env(|| {
        env::set_var("FRIEND_RUNTIME_MODE", "parity");

        let config = AppConfig::load().expect("parity config");

        assert_eq!(config.metrics_addr.to_string(), "127.0.0.1:9083");
        assert_eq!(config.grpc_addr.to_string(), "127.0.0.1:50053");
        assert_eq!(config.database_pool_max_connections, 5);
        assert_eq!(
            config.database_url,
            "postgres://postgres:postgres@localhost:5432/friend_db"
        );
        assert_eq!(config.database_pool_acquire_timeout_ms, 30_000);
        assert_eq!(config.profile_grpc_endpoint, "http://127.0.0.1:50052");
        // Seed data is opt-in, so an unconfigured parity start stays empty.
        assert!(!config.seed_enabled);
        assert!(config.mutation_receipt_retention_worker_enabled);
        assert_eq!(
            config.mutation_receipt_retention_worker_interval_ms,
            3_600_000
        );
        assert_eq!(config.mutation_receipt_retention_batch_size, 1_000);
        assert!(!config.caller_registry.is_empty());
    });
}

#[test]
fn database_pool_acquire_timeout_can_be_configured() {
    with_clean_friend_env(|| {
        env::set_var("FRIEND_RUNTIME_MODE", "parity");
        env::set_var("FRIEND_DB_POOL_ACQUIRE_TIMEOUT_MS", "150");

        let config = AppConfig::load().expect("parity config");

        assert_eq!(config.database_pool_acquire_timeout_ms, 150);
    });
}

#[test]
fn database_pool_acquire_timeout_rejects_values_outside_the_supported_range() {
    for invalid in ["0", "300001", "not-a-number"] {
        with_clean_friend_env(|| {
            env::set_var("FRIEND_RUNTIME_MODE", "parity");
            env::set_var("FRIEND_DB_POOL_ACQUIRE_TIMEOUT_MS", invalid);

            let error = expect_load_error("invalid acquire timeout must fail");

            assert!(error
                .to_string()
                .contains("FRIEND_DB_POOL_ACQUIRE_TIMEOUT_MS"));
        });
    }
}

#[test]
fn load_accepts_explicit_safe_production_settings() {
    with_clean_friend_env(|| {
        set_safe_production_env();

        let config = AppConfig::load().expect("production config");

        assert_eq!(config.grpc_addr.to_string(), "0.0.0.0:50053");
        assert_eq!(
            config.database_url,
            "postgres://friend:secret@postgres.internal:5432/friend_db"
        );
        assert_eq!(
            config.profile_grpc_endpoint,
            "http://profile.internal:50052"
        );
        assert!(!config.seed_enabled);
        assert!(!config.caller_registry.is_empty());
    });
}

#[test]
fn load_rejects_unknown_runtime_mode() {
    with_clean_friend_env(|| {
        env::set_var("FRIEND_RUNTIME_MODE", "staging");

        let error = expect_load_error("unknown mode must fail");

        assert!(error.to_string().contains("FRIEND_RUNTIME_MODE"));
    });
}

#[test]
fn load_rejects_invalid_seed_flag() {
    with_clean_friend_env(|| {
        env::set_var("FRIEND_RUNTIME_MODE", "parity");
        env::set_var("FRIEND_SEED_ENABLED", "sometimes");

        let error = expect_load_error("invalid seed flag must fail");

        assert!(error.to_string().contains("FRIEND_SEED_ENABLED"));
    });
}

#[test]
fn load_rejects_invalid_grpc_addr() {
    with_clean_friend_env(|| {
        env::set_var("FRIEND_RUNTIME_MODE", "parity");
        env::set_var("FRIEND_GRPC_ADDR", "not an address");

        let error = expect_load_error("invalid gRPC address must fail");

        assert!(error.to_string().contains("invalid socket address"));
    });
}

#[test]
fn load_rejects_missing_production_service_token() {
    with_clean_friend_env(|| {
        set_safe_production_env();
        env::remove_var("FRIEND_SERVICE_CREDENTIAL");

        let error = expect_load_error("missing token must fail");

        // The service's own outbound credential is required in production.
        assert!(error.to_string().contains("FRIEND_SERVICE_CREDENTIAL"));
    });
}

#[test]
fn load_rejects_missing_required_caller_credential() {
    with_clean_friend_env(|| {
        set_safe_production_env();
        // The edge is a required caller; without its credential the service
        // would accept no trusted caller at all.
        env::remove_var("GATEWAY_SERVICE_CREDENTIAL");

        let error = expect_load_error("missing caller credential must fail");

        assert!(error.to_string().contains("GATEWAY_SERVICE_CREDENTIAL"));
    });
}

#[test]
fn load_rejects_placeholder_production_service_token() {
    with_clean_friend_env(|| {
        set_safe_production_env();
        env::set_var(
            "FRIEND_SERVICE_CREDENTIAL",
            LOCAL_PARITY_INTERNAL_GRPC_SERVICE_TOKEN,
        );

        let error = expect_load_error("placeholder token must fail");

        assert!(error.to_string().contains("FRIEND_SERVICE_CREDENTIAL"));
    });
}

#[test]
fn load_rejects_loopback_production_grpc_bind() {
    with_clean_friend_env(|| {
        set_safe_production_env();
        env::set_var("FRIEND_GRPC_ADDR", "127.0.0.1:50053");

        let error = expect_load_error("loopback bind must fail");

        assert!(error.to_string().contains("FRIEND_GRPC_ADDR"));
    });
}

#[test]
fn load_rejects_local_production_database_url() {
    with_clean_friend_env(|| {
        set_safe_production_env();
        env::set_var(
            "FRIEND_DATABASE_URL",
            "postgres://postgres:postgres@localhost:5432/friend_db",
        );

        let error = expect_load_error("local database must fail");

        assert!(error.to_string().contains("FRIEND_DATABASE_URL"));
    });
}

#[test]
fn load_rejects_enabled_production_seed_data() {
    with_clean_friend_env(|| {
        set_safe_production_env();
        env::set_var("FRIEND_SEED_ENABLED", "true");

        let error = expect_load_error("production seed must fail");

        assert!(error.to_string().contains("FRIEND_SEED_ENABLED"));
    });
}

#[test]
fn load_rejects_learned_ranker_without_a_configured_adapter() {
    with_clean_friend_env(|| {
        env::set_var("FRIEND_RUNTIME_MODE", "parity");
        env::set_var("FRIEND_PEOPLE_RANKER_ADAPTER", "TWO_TOWER");

        let error = expect_load_error("learned ranker without adapter must fail");

        assert!(error.to_string().contains("FRIEND_PEOPLE_RANKER_ADAPTER"));
    });
}

#[test]
fn load_rejects_embedding_source_without_a_configured_adapter() {
    with_clean_friend_env(|| {
        env::set_var("FRIEND_RUNTIME_MODE", "parity");
        env::set_var("FRIEND_PEOPLE_EMBEDDING_SOURCE_ENABLED", "true");

        let error = expect_load_error("embedding source without adapter must fail");

        assert!(error
            .to_string()
            .contains("FRIEND_PEOPLE_EMBEDDING_SOURCE_ENABLED"));
    });
}

#[test]
fn load_rejects_unbounded_recommendation_retention_settings() {
    with_clean_friend_env(|| {
        env::set_var("FRIEND_RUNTIME_MODE", "parity");
        env::set_var("FRIEND_RECOMMENDATION_RETENTION_BATCH_SIZE", "5001");

        let error = expect_load_error("oversized retention batch must fail");

        assert!(error
            .to_string()
            .contains("FRIEND_RECOMMENDATION_RETENTION_BATCH_SIZE"));
    });
    with_clean_friend_env(|| {
        env::set_var("FRIEND_RUNTIME_MODE", "parity");
        env::set_var("FRIEND_RECOMMENDATION_EVENT_RETENTION_DAYS", "0");

        let error = expect_load_error("zero event retention must fail");

        assert!(error
            .to_string()
            .contains("FRIEND_RECOMMENDATION_EVENT_RETENTION_DAYS"));
    });
}

#[test]
fn load_rejects_unbounded_mutation_receipt_retention_settings() {
    with_clean_friend_env(|| {
        env::set_var("FRIEND_RUNTIME_MODE", "parity");
        env::set_var("FRIEND_MUTATION_RECEIPT_RETENTION_BATCH_SIZE", "5001");

        let error = expect_load_error("oversized receipt retention batch must fail");

        assert!(error
            .to_string()
            .contains("FRIEND_MUTATION_RECEIPT_RETENTION_BATCH_SIZE"));
    });
    with_clean_friend_env(|| {
        env::set_var("FRIEND_RUNTIME_MODE", "parity");
        env::set_var(
            "FRIEND_MUTATION_RECEIPT_RETENTION_WORKER_INTERVAL_MS",
            "999",
        );

        let error = expect_load_error("too-short receipt retention interval must fail");

        assert!(error
            .to_string()
            .contains("FRIEND_MUTATION_RECEIPT_RETENTION_WORKER_INTERVAL_MS"));
    });
    with_clean_friend_env(|| {
        env::set_var("FRIEND_RUNTIME_MODE", "parity");
        env::set_var(
            "FRIEND_MUTATION_RECEIPT_RETENTION_WORKER_ENABLED",
            "sometimes",
        );

        let error = expect_load_error("invalid receipt retention toggle must fail");

        assert!(error
            .to_string()
            .contains("FRIEND_MUTATION_RECEIPT_RETENTION_WORKER_ENABLED"));
    });
}

#[tokio::test]
async fn public_privacy_fixture_and_production_grpc_client_satisfy_profile_ports() {
    let config = test_config(false);

    let visibility = crate::public_profile_privacy::PublicProfilePrivacy
        .account_visibility(uuid::Uuid::nil())
        .await
        .expect("public visibility");
    assert_eq!(
        visibility,
        friend_service::domain::follow_request::AccountVisibility::Public
    );

    let client = GrpcProfilePrivacyClient::connect_lazy(
        config.profile_grpc_endpoint,
        platform::grpc_auth::Workload::Friend,
        &config.outbound_identity,
        None,
    )
    .expect("production gRPC client");
    assert_profile_ports(&client);
}

#[tokio::test]
async fn production_build_state_constructs_the_configured_grpc_profile_client() {
    let mut config = test_config(false);
    config.profile_grpc_endpoint = "not a profile endpoint".into();

    let error = match build_state(&config, lazy_pool_without_database()).await {
        Ok(_) => panic!("production state must validate its gRPC client endpoint"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("invalid URI"));
}

#[tokio::test]
async fn build_state_skips_seed_data_when_seed_is_disabled() {
    let config = test_config(false);
    let pool = lazy_pool_without_database();

    let state = build_state(&config, pool).await.expect("state");
    let stats = state
        .get_stats_batch
        .execute(&[])
        .await
        .expect("empty stats");

    assert!(stats.is_empty());
}

#[tokio::test]
async fn injected_dependencies_do_not_construct_the_configured_profile_endpoint() {
    let mut config = test_config(false);
    config.profile_grpc_endpoint = "not a profile endpoint".into();

    let state = build_state_with_dependencies(
        &config,
        lazy_pool_without_database(),
        Arc::new(crate::public_profile_privacy::PublicProfilePrivacy),
        Arc::new(EmptyProfileDiscovery),
    )
    .await
    .expect("injected state");
    let stats = state
        .get_stats_batch
        .execute(&[])
        .await
        .expect("empty stats");

    assert!(stats.is_empty());
}

#[tokio::test]
async fn build_state_propagates_seed_database_error_when_seed_is_enabled() {
    let config = test_config(true);
    let pool = lazy_pool_without_database();

    let error = match build_state(&config, pool).await {
        Ok(_) => panic!("seeded build_state must fail without a database"),
        Err(error) => error,
    };

    assert!(!error.to_string().is_empty());
}

fn with_clean_friend_env<T>(test: impl FnOnce() -> T) -> T {
    let _lock = ENV_LOCK.lock().expect("environment lock");
    let _guard = FriendEnvGuard::capture();
    test()
}

fn test_config(seed_enabled: bool) -> AppConfig {
    AppConfig {
        database_migration_mode: migration_runtime::MigrationMode::Startup,
        http_addr: "127.0.0.1:8083".parse().expect("addr"),
        metrics_addr: "127.0.0.1:0".parse().expect("addr"),
        grpc_addr: "127.0.0.1:50053".parse().expect("addr"),
        database_pool_max_connections: 5,
        database_pool_acquire_timeout_ms: 30_000,
        database_url: "postgres://postgres:postgres@127.0.0.1:1/friend_unit".into(),
        profile_grpc_endpoint: "http://127.0.0.1:50052".into(),
        seed_enabled,
        actor_delegation: platform::actor_delegation::ActorDelegationVerifier::local_parity(
            platform::grpc_auth::Workload::Friend,
        ),
        caller_registry: platform::grpc_auth::CallerRegistry::new()
            .register(platform::grpc_auth::Workload::Gateway, "friend-unit-token")
            .expect("gateway caller"),
        outbound_identity: "friend-unit-service-token".into(),
        tls_material: None,
        recommendation_worker_enabled: false,
        recommendation_worker_interval_ms: 5_000,
        recommendation_worker_batch_size: 100,
        recommendation_worker_lease_timeout_ms: 300_000,
        recommendation_worker_max_attempts: 5,
        recommendation_worker_base_retry_ms: 1_000,
        recommendation_retention_worker_enabled: false,
        recommendation_retention_worker_interval_ms: 3_600_000,
        recommendation_retention_batch_size: 500,
        recommendation_event_retention_days: 90,
        mutation_receipt_retention_worker_enabled: false,
        mutation_receipt_retention_worker_interval_ms: 3_600_000,
        mutation_receipt_retention_batch_size: 1_000,
        people_rank_version: "people-linear-v1".into(),
        people_ranker_adapter: "LINEAR".into(),
        people_embedding_source_enabled: false,
        people_recent_sequence_enabled: false,
        people_recent_sequence_limit: 20,
    }
}

fn lazy_pool_without_database() -> sqlx::PgPool {
    PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(5))
        .connect_lazy("postgres://postgres:postgres@127.0.0.1:1/friend_unit")
        .expect("lazy pool")
}

fn assert_profile_ports<T: ProfilePrivacyPort + ProfileDiscoveryPort>(_: &T) {}

#[derive(Clone, Copy)]
struct EmptyProfileDiscovery;

#[async_trait::async_trait]
impl ProfileDiscoveryPort for EmptyProfileDiscovery {
    async fn batch_profiles(
        &self,
        _user_ids: &[uuid::Uuid],
    ) -> anyhow::Result<Vec<ProfileDiscoverySnapshot>> {
        Ok(Vec::new())
    }
}

fn set_safe_production_env() {
    env::set_var("FRIEND_RUNTIME_MODE", "production");
    env::set_var(
        "ACTOR_DELEGATION_VERIFYING_KEYS",
        "AwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwM=",
    );
    env::set_var("FRIEND_HTTP_ADDR", "0.0.0.0:8083");
    env::set_var("FRIEND_METRICS_ADDR", "0.0.0.0:9083");
    env::set_var("FRIEND_GRPC_ADDR", "0.0.0.0:50053");
    env::set_var(
        "FRIEND_DATABASE_URL",
        "postgres://friend:secret@postgres.internal:5432/friend_db",
    );
    env::set_var(
        "FRIEND_PROFILE_GRPC_ENDPOINT",
        "http://profile.internal:50052",
    );
    env::set_var("FRIEND_SEED_ENABLED", "false");
    env::set_var(
        "FRIEND_SERVICE_CREDENTIAL",
        "friend-production-service-token",
    );
    // The edge is a required caller, so its credential must also be explicit.
    env::set_var(
        "GATEWAY_SERVICE_CREDENTIAL",
        "gateway-production-credential",
    );
    // Production refuses to load without internal TLS material, so point the
    // service at generated certificates. Leaving them unset would make every
    // rejection test below pass for the wrong reason.
    test_support::tls::apply_env("friend", "friend").expect("generated internal TLS material");
}

fn expect_load_error(context: &str) -> anyhow::Error {
    match AppConfig::load() {
        Ok(_) => panic!("{context}"),
        Err(error) => error,
    }
}

struct FriendEnvGuard {
    saved: Vec<(&'static str, Option<String>)>,
}

impl FriendEnvGuard {
    fn capture() -> Self {
        let saved = FRIEND_ENV_KEYS
            .into_iter()
            .map(|key| (key, env::var(key).ok()))
            .collect::<Vec<_>>();
        for key in FRIEND_ENV_KEYS {
            env::remove_var(key);
        }
        Self { saved }
    }
}

impl Drop for FriendEnvGuard {
    fn drop(&mut self) {
        for (key, value) in &self.saved {
            match value {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
            }
        }
    }
}
