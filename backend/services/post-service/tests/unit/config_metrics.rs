use platform::edge::Edge;
use post_service::config::{PostMetrics, RuntimeMode};
use post_service::domain::post::OutboxMetricsSnapshot;
use std::ffi::OsString;
use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn runtime_mode_aliases_control_admin_http_route_availability() {
    assert_eq!(
        RuntimeMode::parse(" production ").expect("production"),
        RuntimeMode::Production
    );
    assert_eq!(RuntimeMode::parse("DEV").expect("dev"), RuntimeMode::Parity);
    assert!(!RuntimeMode::Production.allows_admin_http_routes());
    assert!(RuntimeMode::Parity.allows_admin_http_routes());
    assert!(RuntimeMode::parse("staging").is_err());
}

#[test]
fn app_config_uses_dedicated_metrics_default_in_parity_mode() {
    let _env = EnvGuard::parity();

    let config = post_service::config::AppConfig::load().expect("parity config");

    assert_eq!(config.metrics_addr.to_string(), "127.0.0.1:9085");
}

struct EnvGuard {
    _lock: MutexGuard<'static, ()>,
    saved: Vec<(&'static str, Option<OsString>)>,
}

impl EnvGuard {
    fn parity() -> Self {
        Self::parity_with(&[])
    }

    fn parity_with(additional: &[(&'static str, Option<&str>)]) -> Self {
        let lock = ENV_LOCK.lock().expect("env lock");
        let mut settings = vec![
            ("POST_RUNTIME_MODE", Some("parity")),
            ("POST_METRICS_ADDR", None),
        ];
        settings.extend_from_slice(additional);
        let saved = settings
            .iter()
            .map(|(name, _)| (*name, std::env::var_os(name)))
            .collect();
        for (name, value) in settings {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
        Self { _lock: lock, saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (name, value) in self.saved.drain(..) {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}

#[test]
fn post_metrics_render_outbox_snapshot_and_escape_label_values() {
    let metrics = PostMetrics::default();
    metrics.record_outbox_worker_run("ok\"line\nnext");
    metrics.record_outbox_worker_forwarded("ok\"line\nnext", 3);
    metrics.record_retention_purge_run("success");
    metrics.record_retention_purged_posts("success", 2);
    metrics.record_retention_purged_comments("success", 1);

    let rendered = metrics.render_prometheus(&OutboxMetricsSnapshot {
        unpublished_total: 7,
        oldest_unpublished_age_seconds: 11,
        dead_lettered_total: 13,
    });

    assert!(rendered.contains("post_outbox_unpublished_total 7\n"));
    assert!(rendered.contains("post_outbox_oldest_unpublished_age_seconds 11\n"));
    assert!(rendered.contains("post_outbox_dead_lettered_total 13\n"));
    assert!(rendered.contains("post_outbox_worker_runs_total{outcome=\"ok\\\"line\\nnext\"} 1\n"));
    assert!(rendered
        .contains("post_outbox_worker_forwarded_events_total{outcome=\"ok\\\"line\\nnext\"} 3\n"));
    assert!(rendered.contains("post_retention_purged_posts_total{outcome=\"success\"} 2\n"));
    assert!(rendered.contains("post_retention_purged_comments_total{outcome=\"success\"} 1\n"));
}

#[test]
fn post_metrics_export_shared_internal_edge_deadline_counts() {
    let metrics = PostMetrics::default();
    metrics
        .edge_deadline_metrics()
        .record_deadline_exhausted(Edge::PostFriend);

    let rendered = metrics.render_prometheus(&OutboxMetricsSnapshot {
        unpublished_total: 0,
        oldest_unpublished_age_seconds: 0,
        dead_lettered_total: 0,
    });

    assert!(rendered.contains("grpc_edge_deadline_exhaustions_total{edge=\"post->friend\"} 1\n"));
}

#[test]
fn database_pool_size_can_be_sized_per_service_instead_of_being_fixed() {
    let _env = EnvGuard::parity_with(&[("POST_DB_POOL_MAX_CONNECTIONS", Some("37"))]);

    let config = post_service::config::AppConfig::load().expect("parity config");

    // A single hardcoded value forced every service to share one ceiling, which is
    // wrong in both directions: it throttles a fan-out service to the limit of an
    // edge, and raising it everywhere to suit one multiplies the load on PostgreSQL.
    assert_eq!(config.database_pool_max_connections, 37);
}

#[test]
fn database_pool_size_defaults_to_the_previous_fixed_value() {
    let _env = EnvGuard::parity_with(&[("POST_DB_POOL_MAX_CONNECTIONS", None)]);

    let config = post_service::config::AppConfig::load().expect("parity config");

    // The default keeps existing deployments unchanged, so this is a new knob
    // rather than a new requirement.
    assert_eq!(config.database_pool_max_connections, 5);
}

#[test]
fn database_pool_size_rejects_a_zero_ceiling() {
    let _env = EnvGuard::parity_with(&[("POST_DB_POOL_MAX_CONNECTIONS", Some("0"))]);

    // A pool that may hold no connections cannot serve a request, so a zero is a
    // misconfiguration rather than a way to disable the pool.
    let error = match post_service::config::AppConfig::load() {
        Ok(_) => panic!("a zero pool must be rejected"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("POST_DB_POOL_MAX_CONNECTIONS"));
}

#[test]
fn database_pool_acquire_timeout_can_be_configured() {
    let _env = EnvGuard::parity_with(&[("POST_DB_POOL_ACQUIRE_TIMEOUT_MS", Some("150"))]);

    let config = post_service::config::AppConfig::load().expect("parity config");

    assert_eq!(config.database_pool_acquire_timeout_ms, 150);
}

#[test]
fn database_pool_acquire_timeout_defaults_to_thirty_seconds() {
    let _env = EnvGuard::parity_with(&[("POST_DB_POOL_ACQUIRE_TIMEOUT_MS", None)]);

    let config = post_service::config::AppConfig::load().expect("parity config");

    assert_eq!(config.database_pool_acquire_timeout_ms, 30_000);
}

#[test]
fn database_pool_acquire_timeout_rejects_zero() {
    let _env = EnvGuard::parity_with(&[("POST_DB_POOL_ACQUIRE_TIMEOUT_MS", Some("0"))]);

    let error = match post_service::config::AppConfig::load() {
        Ok(_) => panic!("zero acquire timeout must be rejected"),
        Err(error) => error,
    };

    assert!(error
        .to_string()
        .contains("POST_DB_POOL_ACQUIRE_TIMEOUT_MS"));
}
