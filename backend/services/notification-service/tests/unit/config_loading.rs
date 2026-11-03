use notification_service::config::AppConfig;
use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

const LOCAL_RETENTION_PURGE_ACTOR_ID: &str = "99999999-9999-9999-9999-999999999999";
const REMOTE_RETENTION_PURGE_ACTOR_ID: &str = "88888888-8888-8888-8888-888888888888";
const ENV_KEYS: &[&str] = &[
    "NOTIFICATION_RUNTIME_MODE",
    "ACTOR_DELEGATION_VERIFYING_KEYS",
    "GATEWAY_SERVICE_CREDENTIAL",
    "POST_SERVICE_CREDENTIAL",
    "AUTH_SERVICE_CREDENTIAL",
    "NOTIFICATION_SERVICE_CREDENTIAL",
    "GATEWAY_SERVICE_CREDENTIAL",
    "NOTIFICATION_RETENTION_PURGE_ACTOR_ID",
    "NOTIFICATION_HTTP_ADDR",
    "NOTIFICATION_METRICS_ADDR",
    "NOTIFICATION_GRPC_ADDR",
    "NOTIFICATION_DATABASE_URL",
    "NOTIFICATION_RETENTION_PURGE_WORKER_ENABLED",
    "NOTIFICATION_RETENTION_PURGE_WORKER_INTERVAL_MS",
    "NOTIFICATION_GRPC_TLS_CERT_PATH",
    "NOTIFICATION_GRPC_TLS_KEY_PATH",
    "NOTIFICATION_GRPC_TLS_CA_PATH",
];

#[test]
fn app_config_loads_parity_defaults_when_runtime_is_local() {
    let _env = EnvGuard::new();
    set_env("NOTIFICATION_RUNTIME_MODE", "parity");

    let config = AppConfig::load().expect("parity config");

    assert_eq!(config.http_addr.to_string(), "127.0.0.1:8084");
    assert_eq!(config.metrics_addr.to_string(), "127.0.0.1:9084");
    assert_eq!(config.grpc_addr.to_string(), "127.0.0.1:50054");
    assert_eq!(
        config.database_url,
        "postgres://postgres:postgres@localhost:5432/notification_db"
    );
    assert!(!config.caller_registry.is_empty());
    assert_eq!(
        config.retention_purge_actor_id,
        LOCAL_RETENTION_PURGE_ACTOR_ID
    );
    assert!(config.retention_purge_worker_enabled);
    assert_eq!(config.retention_purge_worker_interval_ms, 3_600_000);
}

#[test]
fn app_config_loads_trimmed_explicit_parity_settings() {
    let _env = EnvGuard::new();
    set_env("NOTIFICATION_RUNTIME_MODE", " dev ");
    set_env("NOTIFICATION_SERVICE_CREDENTIAL", " custom-token ");
    set_env(
        "NOTIFICATION_RETENTION_PURGE_ACTOR_ID",
        &format!(" {REMOTE_RETENTION_PURGE_ACTOR_ID} "),
    );
    set_env("NOTIFICATION_HTTP_ADDR", "0.0.0.0:18084");
    set_env("NOTIFICATION_METRICS_ADDR", "0.0.0.0:19084");
    set_env("NOTIFICATION_GRPC_ADDR", "0.0.0.0:15054");
    set_env(
        "NOTIFICATION_DATABASE_URL",
        "postgres://notification:secret@db.internal:5432/notification_db",
    );
    set_env("NOTIFICATION_RETENTION_PURGE_WORKER_ENABLED", "false");
    set_env("NOTIFICATION_RETENTION_PURGE_WORKER_INTERVAL_MS", "1000");

    let config = AppConfig::load().expect("explicit parity config");

    assert_eq!(config.http_addr.to_string(), "0.0.0.0:18084");
    assert_eq!(config.metrics_addr.to_string(), "0.0.0.0:19084");
    assert_eq!(config.grpc_addr.to_string(), "0.0.0.0:15054");
    assert!(!config.caller_registry.is_empty());
    assert_eq!(
        config.retention_purge_actor_id,
        REMOTE_RETENTION_PURGE_ACTOR_ID
    );
    assert!(!config.retention_purge_worker_enabled);
    assert_eq!(config.retention_purge_worker_interval_ms, 1_000);
}

#[test]
fn production_config_requires_explicit_retention_actor() {
    let _env = EnvGuard::new();
    set_production_safe_env();
    std::env::remove_var("NOTIFICATION_RETENTION_PURGE_ACTOR_ID");

    assert_load_error_contains("NOTIFICATION_RETENTION_PURGE_ACTOR_ID is required in production");
}

#[test]
fn production_config_rejects_malformed_retention_actor() {
    let _env = EnvGuard::new();
    set_production_safe_env();
    set_env("NOTIFICATION_RETENTION_PURGE_ACTOR_ID", "not-a-uuid");

    assert_load_error_contains("NOTIFICATION_RETENTION_PURGE_ACTOR_ID must be a UUID");
}

#[test]
fn production_config_rejects_loopback_grpc_bind() {
    let _env = EnvGuard::new();
    set_production_safe_env();
    set_env("NOTIFICATION_GRPC_ADDR", "127.0.0.1:50054");

    assert_load_error_contains("NOTIFICATION_GRPC_ADDR");
}

#[test]
fn app_config_rejects_invalid_worker_enabled_flag() {
    let _env = EnvGuard::new();
    set_env("NOTIFICATION_RUNTIME_MODE", "parity");
    set_env("NOTIFICATION_RETENTION_PURGE_WORKER_ENABLED", "sometimes");

    assert_load_error_contains("NOTIFICATION_RETENTION_PURGE_WORKER_ENABLED");
}

#[test]
fn app_config_rejects_worker_interval_outside_bounds() {
    let _env = EnvGuard::new();
    set_env("NOTIFICATION_RUNTIME_MODE", "parity");
    set_env("NOTIFICATION_RETENTION_PURGE_WORKER_INTERVAL_MS", "999");

    assert_load_error_contains("NOTIFICATION_RETENTION_PURGE_WORKER_INTERVAL_MS");
}

struct EnvGuard {
    _lock: MutexGuard<'static, ()>,
}

impl EnvGuard {
    fn new() -> Self {
        let lock = ENV_LOCK.lock().expect("env lock");
        clear_env();
        Self { _lock: lock }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        clear_env();
    }
}

fn clear_env() {
    for key in ENV_KEYS {
        std::env::remove_var(key);
    }
}

fn set_env(key: &str, value: &str) {
    std::env::set_var(key, value);
}

fn set_production_safe_env() {
    set_env("NOTIFICATION_RUNTIME_MODE", "production");
    set_env(
        "ACTOR_DELEGATION_VERIFYING_KEYS",
        "AwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwM=",
    );
    set_env("NOTIFICATION_SERVICE_CREDENTIAL", "production-token");
    // The edge is a required caller, so its credential must also be explicit.
    set_env(
        "GATEWAY_SERVICE_CREDENTIAL",
        "gateway-production-credential",
    );
    set_env(
        "NOTIFICATION_RETENTION_PURGE_ACTOR_ID",
        REMOTE_RETENTION_PURGE_ACTOR_ID,
    );
    set_env("NOTIFICATION_HTTP_ADDR", "0.0.0.0:8084");
    set_env("NOTIFICATION_METRICS_ADDR", "0.0.0.0:9084");
    set_env("NOTIFICATION_GRPC_ADDR", "0.0.0.0:50054");
    set_env(
        "NOTIFICATION_DATABASE_URL",
        "postgres://notification:secret@postgres.internal:5432/notification_db",
    );
    // Production refuses to load without internal TLS material, so point the
    // service at generated certificates. Leaving them unset would make every
    // rejection test below pass for the wrong reason.
    test_support::tls::apply_env("notification", "notification")
        .expect("generated internal TLS material");
}

fn assert_load_error_contains(expected: &str) {
    let error = match AppConfig::load() {
        Ok(_) => panic!("config should fail"),
        Err(error) => error,
    };
    let message = error.to_string();
    assert!(
        message.contains(expected),
        "expected `{message}` to contain `{expected}`"
    );
}
