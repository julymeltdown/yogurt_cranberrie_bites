use platform::config::LOCAL_PARITY_INTERNAL_GRPC_SERVICE_TOKEN;
use profile_service::config::{AppConfig, ProfileMetrics};
use std::sync::{Mutex, OnceLock};

const LOCAL_RETENTION_PURGE_ACTOR_ID: &str = "99999999-9999-9999-9999-999999999999";
const PRODUCTION_RETENTION_PURGE_ACTOR_ID: &str = "11111111-1111-1111-1111-111111111111";
const PROFILE_ENV_KEYS: &[&str] = &[
    "PROFILE_RUNTIME_MODE",
    "ACTOR_DELEGATION_VERIFYING_KEYS",
    "GATEWAY_SERVICE_CREDENTIAL",
    "POST_SERVICE_CREDENTIAL",
    "FRIEND_SERVICE_CREDENTIAL",
    "AUTH_SERVICE_CREDENTIAL",
    "PROFILE_SERVICE_CREDENTIAL",
    "PROFILE_RETENTION_PURGE_ACTOR_ID",
    "PROFILE_HTTP_ADDR",
    "PROFILE_METRICS_ADDR",
    "PROFILE_GRPC_ADDR",
    "PROFILE_DATABASE_URL",
    "PROFILE_SEED_ENABLED",
    "PROFILE_RETENTION_PURGE_WORKER_ENABLED",
    "PROFILE_RETENTION_PURGE_WORKER_INTERVAL_MS",
    "PROFILE_MUTATION_RECEIPT_RETENTION_WORKER_ENABLED",
    "PROFILE_MUTATION_RECEIPT_RETENTION_WORKER_INTERVAL_MS",
    "PROFILE_MUTATION_RECEIPT_RETENTION_BATCH_SIZE",
    "PROFILE_GRPC_TLS_CERT_PATH",
    "PROFILE_GRPC_TLS_KEY_PATH",
    "PROFILE_GRPC_TLS_CA_PATH",
];

#[test]
fn app_config_load_uses_parity_defaults_when_runtime_is_not_production() {
    let _guard = env_lock().lock().expect("env lock");
    let _snapshot = EnvSnapshot::capture_and_clear();
    std::env::set_var("PROFILE_RUNTIME_MODE", "parity");

    let config = AppConfig::load().expect("parity config");

    assert_eq!(config.metrics_addr.to_string(), "127.0.0.1:9082");
    // Parity admits at least one caller without configuration; production
    // requires an explicit credential for every required caller.
    assert!(!config.caller_registry.is_empty());
    assert_eq!(
        config.retention_purge_actor_id,
        LOCAL_RETENTION_PURGE_ACTOR_ID
    );
    // Seed data is opt-in, so an unconfigured parity start stays empty.
    assert!(!config.seed_enabled);
    assert!(config.retention_purge_worker_enabled);
    assert_eq!(config.retention_purge_worker_interval_ms, 3_600_000);
    assert!(config.mutation_receipt_retention_worker_enabled);
    assert_eq!(
        config.mutation_receipt_retention_worker_interval_ms,
        3_600_000
    );
    assert_eq!(config.mutation_receipt_retention_batch_size, 1_000);
}

#[test]
fn app_config_load_rejects_missing_required_caller_credential() {
    let _guard = env_lock().lock().expect("env lock");
    let _snapshot = EnvSnapshot::capture_and_clear();
    std::env::set_var("PROFILE_RUNTIME_MODE", "production");

    let error = load_config_error();

    // The edge is a required caller. Without its credential the service would
    // accept no trusted caller at all, so production must refuse to start.
    assert!(error.contains("GATEWAY_SERVICE_CREDENTIAL"));
}

#[test]
fn app_config_load_rejects_unknown_runtime_mode() {
    let _guard = env_lock().lock().expect("env lock");
    let _snapshot = EnvSnapshot::capture_and_clear();
    std::env::set_var("PROFILE_RUNTIME_MODE", "staging");

    let error = load_config_error();

    assert!(error.contains("PROFILE_RUNTIME_MODE"));
    assert!(error.contains("production or parity"));
}

#[test]
fn app_config_load_rejects_placeholder_caller_credential() {
    let _guard = env_lock().lock().expect("env lock");
    let _snapshot = EnvSnapshot::capture_and_clear();
    std::env::set_var("PROFILE_RUNTIME_MODE", "production");
    std::env::set_var(
        "GATEWAY_SERVICE_CREDENTIAL",
        LOCAL_PARITY_INTERNAL_GRPC_SERVICE_TOKEN,
    );

    let error = load_config_error();

    // A shared placeholder would let any caller impersonate any other.
    assert!(error.contains("GATEWAY_SERVICE_CREDENTIAL"));
    assert!(error.contains("placeholder"));
}

#[test]
fn app_config_load_rejects_missing_production_retention_actor() {
    let _guard = env_lock().lock().expect("env lock");
    let _snapshot = EnvSnapshot::capture_and_clear();
    std::env::set_var("PROFILE_RUNTIME_MODE", "production");
    std::env::set_var("PROFILE_SERVICE_CREDENTIAL", "production-token");
    // The edge is a required caller, so its credential must also be explicit.
    std::env::set_var(
        "GATEWAY_SERVICE_CREDENTIAL",
        "gateway-production-credential",
    );

    let error = load_config_error();

    assert!(error.contains("PROFILE_RETENTION_PURGE_ACTOR_ID"));
}

#[test]
fn app_config_load_rejects_invalid_retention_actor_uuid() {
    let _guard = env_lock().lock().expect("env lock");
    let _snapshot = EnvSnapshot::capture_and_clear();
    std::env::set_var("PROFILE_RUNTIME_MODE", "production");
    std::env::set_var("PROFILE_SERVICE_CREDENTIAL", "production-token");
    // The edge is a required caller, so its credential must also be explicit.
    std::env::set_var(
        "GATEWAY_SERVICE_CREDENTIAL",
        "gateway-production-credential",
    );
    std::env::set_var("PROFILE_RETENTION_PURGE_ACTOR_ID", "not-a-uuid");

    let error = load_config_error();

    assert!(error.contains("PROFILE_RETENTION_PURGE_ACTOR_ID must be a UUID"));
}

#[test]
fn app_config_load_rejects_invalid_worker_interval() {
    let _guard = env_lock().lock().expect("env lock");
    let _snapshot = EnvSnapshot::capture_and_clear();
    std::env::set_var("PROFILE_RUNTIME_MODE", "parity");
    std::env::set_var("PROFILE_RETENTION_PURGE_WORKER_INTERVAL_MS", "999");

    let error = load_config_error();

    assert!(error.contains("PROFILE_RETENTION_PURGE_WORKER_INTERVAL_MS"));
    assert!(error.contains("between 1000 and 86400000"));
}

#[test]
fn app_config_load_rejects_invalid_worker_enabled_boolean() {
    let _guard = env_lock().lock().expect("env lock");
    let _snapshot = EnvSnapshot::capture_and_clear();
    std::env::set_var("PROFILE_RUNTIME_MODE", "parity");
    std::env::set_var("PROFILE_RETENTION_PURGE_WORKER_ENABLED", "sometimes");

    let error = load_config_error();

    assert!(error.contains("PROFILE_RETENTION_PURGE_WORKER_ENABLED"));
    assert!(error.contains("invalid"));
}

#[test]
fn app_config_load_rejects_unbounded_mutation_receipt_retention_batch_size() {
    let _guard = env_lock().lock().expect("env lock");
    let _snapshot = EnvSnapshot::capture_and_clear();
    std::env::set_var("PROFILE_RUNTIME_MODE", "parity");
    std::env::set_var("PROFILE_MUTATION_RECEIPT_RETENTION_BATCH_SIZE", "5001");

    let error = load_config_error();

    assert!(error.contains("PROFILE_MUTATION_RECEIPT_RETENTION_BATCH_SIZE"));
    assert!(error.contains("between 1 and 5000"));
}

#[test]
fn app_config_load_rejects_invalid_mutation_receipt_retention_interval() {
    let _guard = env_lock().lock().expect("env lock");
    let _snapshot = EnvSnapshot::capture_and_clear();
    std::env::set_var("PROFILE_RUNTIME_MODE", "parity");
    std::env::set_var(
        "PROFILE_MUTATION_RECEIPT_RETENTION_WORKER_INTERVAL_MS",
        "999",
    );

    let error = load_config_error();

    assert!(error.contains("PROFILE_MUTATION_RECEIPT_RETENTION_WORKER_INTERVAL_MS"));
    assert!(error.contains("between 1000 and 86400000"));
}

#[test]
fn app_config_load_rejects_unsafe_production_http_bind() {
    let _guard = env_lock().lock().expect("env lock");
    let _snapshot = EnvSnapshot::capture_and_clear();
    set_safe_production_env();
    std::env::set_var("PROFILE_HTTP_ADDR", "127.0.0.1:8082");

    let error = load_config_error();

    assert!(error.contains("PROFILE_HTTP_ADDR"));
    assert!(error.contains("loopback"));
}

#[test]
fn app_config_load_accepts_safe_production_settings() {
    let _guard = env_lock().lock().expect("env lock");
    let _snapshot = EnvSnapshot::capture_and_clear();
    set_safe_production_env();

    let config = AppConfig::load().expect("production config");

    assert_eq!(config.http_addr.to_string(), "0.0.0.0:8082");
    assert_eq!(config.grpc_addr.to_string(), "0.0.0.0:50052");
    assert_eq!(
        config.database_url,
        "postgres://profile:secret@postgres.internal:5432/profile_db"
    );
    assert!(!config.seed_enabled);
    assert!(!config.retention_purge_worker_enabled);
    assert_eq!(config.retention_purge_worker_interval_ms, 1_000);
    assert_eq!(
        config.retention_purge_actor_id,
        PRODUCTION_RETENTION_PURGE_ACTOR_ID
    );
}

#[test]
fn profile_metrics_accumulate_and_escape_prometheus_labels() {
    let metrics = ProfileMetrics::default();

    metrics.record_retention_purge_run("line\nquote\"slash\\");
    metrics.record_retention_purge_run("line\nquote\"slash\\");
    metrics.record_retention_purged_profiles("success", 3);
    metrics.record_mutation_receipt_retention_run("success");
    metrics.record_mutation_receipt_retention_run("failure");
    metrics.record_mutation_receipt_outcome_bodies_purged(2);

    let rendered = metrics.render_prometheus();

    assert!(rendered
        .contains("profile_retention_purge_runs_total{outcome=\"line\\nquote\\\"slash\\\\\"} 2"));
    assert!(rendered.contains("profile_retention_purged_profiles_total{outcome=\"success\"} 3"));
    assert!(
        rendered.contains("profile_mutation_receipt_retention_runs_total{outcome=\"success\"} 1")
    );
    assert!(
        rendered.contains("profile_mutation_receipt_retention_runs_total{outcome=\"failure\"} 1")
    );
    assert!(rendered.contains("profile_mutation_receipt_outcome_bodies_purged_total{} 2"));
}

fn set_safe_production_env() {
    std::env::set_var("PROFILE_RUNTIME_MODE", "production");
    std::env::set_var(
        "ACTOR_DELEGATION_VERIFYING_KEYS",
        "AwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwM=",
    );
    std::env::set_var("PROFILE_SERVICE_CREDENTIAL", "production-token");
    // The edge is a required caller, so its credential must also be explicit.
    std::env::set_var(
        "GATEWAY_SERVICE_CREDENTIAL",
        "gateway-production-credential",
    );
    std::env::set_var(
        "PROFILE_RETENTION_PURGE_ACTOR_ID",
        PRODUCTION_RETENTION_PURGE_ACTOR_ID,
    );
    std::env::set_var("PROFILE_HTTP_ADDR", "0.0.0.0:8082");
    std::env::set_var("PROFILE_METRICS_ADDR", "0.0.0.0:9082");
    std::env::set_var("PROFILE_GRPC_ADDR", "0.0.0.0:50052");
    std::env::set_var(
        "PROFILE_DATABASE_URL",
        "postgres://profile:secret@postgres.internal:5432/profile_db",
    );
    std::env::set_var("PROFILE_SEED_ENABLED", "false");
    std::env::set_var("PROFILE_RETENTION_PURGE_WORKER_ENABLED", "false");
    std::env::set_var("PROFILE_RETENTION_PURGE_WORKER_INTERVAL_MS", "1000");
    std::env::set_var("PROFILE_MUTATION_RECEIPT_RETENTION_WORKER_ENABLED", "false");
    std::env::set_var(
        "PROFILE_MUTATION_RECEIPT_RETENTION_WORKER_INTERVAL_MS",
        "1000",
    );
    std::env::set_var("PROFILE_MUTATION_RECEIPT_RETENTION_BATCH_SIZE", "1");
    // Production refuses to load without internal TLS material, so point the
    // service at generated certificates. Leaving them unset would make every
    // rejection test below pass for the wrong reason.
    test_support::tls::apply_env("profile", "profile").expect("generated internal TLS material");
}

fn load_config_error() -> String {
    match AppConfig::load() {
        Ok(_) => panic!("config should be rejected"),
        Err(error) => error.to_string(),
    }
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct EnvSnapshot {
    values: Vec<(&'static str, Option<String>)>,
}

impl EnvSnapshot {
    fn capture_and_clear() -> Self {
        let values = PROFILE_ENV_KEYS
            .iter()
            .map(|key| (*key, std::env::var(key).ok()))
            .collect::<Vec<_>>();
        for key in PROFILE_ENV_KEYS {
            std::env::remove_var(key);
        }
        // Every case gets generated internal TLS material. Production refuses to
        // load without it, and leaving it unset would make a test that asserts
        // "this retention actor is invalid" fail on the missing-certificate error
        // instead, which would hide the behaviour under test.
        test_support::tls::apply_env("profile", "profile")
            .expect("generated internal TLS material");
        Self { values }
    }
}

impl Drop for EnvSnapshot {
    fn drop(&mut self) {
        for (key, value) in &self.values {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
