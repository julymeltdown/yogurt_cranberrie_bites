use super::*;
use crate::adapters::r#in::http::client_identity::TrustedProxies;
use crate::adapters::r#in::http::server::with_transport_layers;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Router;
use tower::ServiceExt;

fn config(origins: Vec<String>) -> AppConfig {
    AppConfig {
        database_migration_mode: migration_runtime::MigrationMode::Startup,
        http_addr: "127.0.0.1:8080".parse().unwrap(),
        metrics_addr: "127.0.0.1:9080".parse().unwrap(),
        grpc_addr: "127.0.0.1:0".parse().expect("grpc addr"),
        database_pool_max_connections: 5,
        journal_pool_max_connections: 4,
        background_pool_max_connections: 3,
        database_url: "postgres://postgres:postgres@localhost/gateway_db".into(),
        seed_enabled: false,
        auth_grpc_endpoint: "http://127.0.0.1:50051".into(),
        concert_grpc_endpoint: None,
        concert_release_digest: None,
        profile_grpc_endpoint: "http://127.0.0.1:50052".into(),
        friend_grpc_endpoint: "http://127.0.0.1:50053".into(),
        notification_grpc_endpoint: "http://127.0.0.1:50054".into(),
        post_grpc_endpoint: "http://127.0.0.1:50055".into(),
        refresh_cookie_domain: String::new(),
        oauth_exposure: crate::config::OAuthExposurePolicy::default(),
        refresh_cookie_secure: false,
        refresh_cookie_max_age_seconds: 1209600,
        aggregation_cache_ttl_seconds: 30,
        cors_allowed_origins: origins,
        jwt_issuer: "test".into(),
        jwt_audience: "test".into(),
        jwt_jwks_json: "{\"keys\":[]}".into(),
        jwt_keys_refresh_seconds: 0,
        jwt_keys_min_refresh_seconds: 10,
        high_risk_session_freshness_seconds: 5,
        runtime_mode: RuntimeMode::Parity,
        developer_portal_links: DeveloperPortalLinks::defaults(RuntimeMode::Parity),
        rate_limit_per_minute: 600,
        admission: crate::domain::admission::AdmissionPolicies::default(),
        admission_hmac_key: "test-admission-hmac-key-with-at-least-32-bytes".into(),
        search_cursor_hmac_key: "test-search-cursor-key-has-32-bytes-minimum".into(),
        search_cursor_ttl_seconds: 900,
        search_cursor_legacy_accept_until: None,
        request_timeout_ms: 1000,
        journal_client_timeout_ms: 900,
        async_operation_keyring: None,
        telemetry_retention_days: 30,
        telemetry_queue_capacity: 4096,
        audit_retention_days: 365,
        // The edge accepts no internal callers; it only sends them.
        actor_delegation_signer: platform::actor_delegation::ActorDelegationSigner::local_parity(
            platform::grpc_auth::Workload::Gateway,
        ),
        caller_registry: platform::grpc_auth::CallerRegistry::new(),
        authority_exchange: crate::config::authority_exchange(
            false,
            &platform::actor_delegation::ActorDelegationSigner::local_parity(
                platform::grpc_auth::Workload::Gateway,
            ),
            false,
            platform::actor_delegation::exchange::ResourceEvidenceVerifier::local_parity(),
        )
        .expect("parity authority exchange"),
        outbound_identity: "test-token".into(),
        // These cases exercise HTTP hardening, not the internal transport.
        tls_material: None,
    }
}

#[test]
fn participant_release_requires_an_explicit_endpoint_and_canonical_digest() {
    let mut candidate = config(Vec::new());
    candidate.concert_release_digest = Some("a".repeat(64));
    assert!(candidate.validate().is_err());
    candidate.concert_grpc_endpoint = Some("http://127.0.0.1:55099".into());
    for invalid in [
        "0".repeat(64),
        "A".repeat(64),
        "a".repeat(63),
        "g".repeat(64),
    ] {
        candidate.concert_release_digest = Some(invalid);
        assert!(candidate.validate().is_err());
    }
    candidate.concert_release_digest = Some("a".repeat(64));
    candidate
        .validate()
        .expect("explicit trusted release input");
}

#[test]
fn production_search_cursors_reject_the_parity_key_and_weak_secrets() {
    let mut config = config(Vec::new());
    config.runtime_mode = RuntimeMode::Production;
    config.search_cursor_hmac_key = DEFAULT_PARITY_SEARCH_CURSOR_HMAC_KEY.to_owned();
    assert!(config
        .validate()
        .expect_err("production must not use the parity key")
        .to_string()
        .contains("must be set to a shared secret in production"));

    config.search_cursor_hmac_key = "short-key".to_owned();
    assert!(config
        .validate()
        .expect_err("weak cursor key must fail")
        .to_string()
        .contains("between 32 and 4096 bytes"));
}

#[test]
fn search_cursor_ttl_and_legacy_deadline_are_bounded() {
    let mut config = config(Vec::new());
    for invalid_ttl in [59, 3_601] {
        config.search_cursor_ttl_seconds = invalid_ttl;
        assert!(config
            .validate()
            .expect_err("out-of-range cursor TTL must fail")
            .to_string()
            .contains("GATEWAY_SEARCH_CURSOR_TTL_SECONDS"));
    }

    config.search_cursor_ttl_seconds = 60;
    config.validate().expect("minimum cursor TTL is supported");
    config.search_cursor_ttl_seconds = 3_600;
    config.validate().expect("maximum cursor TTL is supported");

    config.search_cursor_legacy_accept_until =
        Some(Utc::now() + ChronoDuration::days(MAX_SEARCH_CURSOR_LEGACY_COMPATIBILITY_DAYS + 1));
    assert!(config
        .validate()
        .expect_err("legacy acceptance beyond the migration window must fail")
        .to_string()
        .contains("GATEWAY_SEARCH_CURSOR_LEGACY_ACCEPT_UNTIL"));
}

async fn preflight(origins: Vec<String>, origin: &str) -> axum::response::Response {
    let router = with_transport_layers(
        Router::new().route("/", get(|| async { StatusCode::NO_CONTENT })),
        &config(origins),
        TrustedProxies::default(),
    );
    router
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/")
                .header("origin", origin)
                .header("access-control-request-method", "POST")
                .header(
                    "access-control-request-headers",
                    "authorization,content-type,x-correlation-id",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn credentialed_cors_builds_and_permits_explicit_headers() {
    let response = preflight(vec!["https://client.test".into()], "https://client.test").await;
    assert_eq!(response.status(), StatusCode::OK);
    let headers = response.headers();
    assert_eq!(
        headers["access-control-allow-origin"],
        "https://client.test"
    );
    assert_eq!(headers["access-control-allow-credentials"], "true");
    let allowed = headers["access-control-allow-headers"].to_str().unwrap();
    for expected in ["authorization", "content-type", "x-correlation-id"] {
        assert!(allowed.split(',').any(|header| header.trim() == expected));
    }
    assert!(!allowed.contains('*'));
}

#[tokio::test]
async fn unlisted_origin_is_not_allowed() {
    let response = preflight(vec!["https://client.test".into()], "https://other.test").await;
    assert!(!response
        .headers()
        .contains_key("access-control-allow-origin"));
}

#[tokio::test]
async fn empty_origin_list_does_not_panic_or_allow_wildcards() {
    let response = preflight(Vec::new(), "https://client.test").await;
    assert!(!response
        .headers()
        .contains_key("access-control-allow-origin"));
}

#[test]
fn wildcard_and_non_origin_cors_settings_are_rejected() {
    for origin in [
        "*",
        "https://client.test/path",
        "https://client.test?secret=1",
    ] {
        assert!(config(vec![origin.into()]).validate().is_err());
    }
    assert!(config(vec!["https://client.test".into()])
        .validate()
        .is_ok());
}

#[test]
fn metric_cardinality_has_a_hard_upper_bound() {
    let metrics = GatewayMetrics::default();
    for index in 0..(GatewayMetrics::MAX_SERIES * 2) {
        metrics.inc_rate_limit_rejection(&format!("/untrusted/{index}"));
    }
    let counters = metrics.counters.lock().unwrap();
    assert_eq!(counters.len(), GatewayMetrics::MAX_SERIES);
    assert!(counters["gateway_metrics_dropped_series_total{}"] > 0);
}

#[test]
fn gauge_series_overflow_is_counted_once_as_a_counter() {
    let metrics = GatewayMetrics::default();
    for index in 0..(GatewayMetrics::MAX_SERIES * 2) {
        let label = format!("untrusted-{index}");
        metrics.set_gauge("test_gauge", &[("key", &label)], index as i64);
    }

    let gauges = metrics.gauges.lock().unwrap();
    assert_eq!(gauges.len(), GatewayMetrics::MAX_SERIES);
    assert!(!gauges.contains_key("gateway_metrics_dropped_series_total{}"));
    drop(gauges);

    let counters = metrics.counters.lock().unwrap();
    assert!(counters["gateway_metrics_dropped_series_total{}"] > 0);
    drop(counters);

    let output = metrics.render_prometheus();
    assert_eq!(
        output
            .lines()
            .filter(|line| line.starts_with("gateway_metrics_dropped_series_total"))
            .count(),
        1
    );
}

#[test]
fn retention_purge_failures_have_a_bounded_counter() {
    let metrics = GatewayMetrics::default();
    metrics.inc_retention_purge_failure();
    metrics.inc_retention_purge_failure();

    assert_eq!(
        metrics.render_prometheus(),
        "gateway_retention_purge_failures_total{} 2\n"
    );
}

mod external_pool_acceptance {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/migration-runtime/tests/support/service_pool.rs"
    ));
}

#[tokio::test]
async fn external_migration_mode_rejects_schema_damage_without_owner_privileges(
) -> anyhow::Result<()> {
    external_pool_acceptance::verify(
        |url, mode| async move {
            let mut config = config(vec![]);
            config.database_url = url;
            config.database_migration_mode = mode;
            connect_pool(&config).await
        },
        "gateway_request_journal",
        "id",
        "gateway_reset_owner_receipt_schedule",
    )
    .await
}

/// Production quota pool construction and real store behavior, not a production
/// HTTP/mTLS deployment or service-binary acceptance test.
#[tokio::test]
async fn external_quota_pool_preserves_restricted_shared_budgets_and_timeouts() -> anyhow::Result<()>
{
    use crate::domain::admission::AdmissionOutcome;

    let (pg, owner_url) = test_support::postgres::start_postgres_with_url_or_native().await?;
    let owner = test_support::postgres::with_startup_diagnostics(
        pg.as_ref(),
        "Gateway quota acceptance owner",
        async {
            Ok(PgPoolOptions::new()
                .max_connections(2)
                .connect(&owner_url)
                .await?)
        },
    )
    .await?;
    sqlx::migrate::Migrator::new(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("migrations")
            .as_path(),
    )
    .await?
    .run(&owner)
    .await?;
    let role = format!("quota_runtime_{}", uuid::Uuid::new_v4().simple());
    let password = uuid::Uuid::new_v4().simple().to_string();
    sqlx::raw_sql(&format!("create role {role} login password '{password}' nosuperuser nocreatedb nocreaterole noinherit noreplication nobypassrls;
        revoke all on database postgres from public; grant connect on database postgres to {role};
        revoke create on schema public from public;
        create schema shadow; grant usage on schema shadow to {role};
        create function shadow.gateway_admit_with_lock_wait(text,text,integer,integer,integer,integer)
        returns table(outcome text,lock_wait_seconds double precision) language sql as $$ select 'admitted'::text,0::float8 $$;"))
        .execute(&owner).await?;
    sqlx::raw_sql(&include_str!("../../runtime-grants.sql").replace(":\"runtime_role\"", &role))
        .execute(&owner)
        .await?;
    let mut url = reqwest::Url::parse(&owner_url)?;
    url.set_username(&role)
        .map_err(|_| anyhow::anyhow!("invalid fixture role"))?;
    url.set_password(Some(&password))
        .map_err(|_| anyhow::anyhow!("invalid fixture password"))?;
    url.query_pairs_mut()
        .append_pair("options", "-csearch_path=shadow");
    let mut settings = config(Vec::new());
    settings.runtime_mode = RuntimeMode::Production;
    settings.database_migration_mode = migration_runtime::MigrationMode::External;
    settings.database_url = url.into();
    let main = connect_pool(&settings).await?;
    let quota = connect_quota_pool(&settings).await?;
    let peer = connect_quota_pool(&settings).await?;
    assert_eq!(quota.options().get_max_connections(), 4);
    assert_eq!(
        quota.options().get_acquire_timeout(),
        Duration::from_millis(200)
    );
    let observed: (String, String, String, String, String) = sqlx::query_as(
        "select current_user::text,current_schema()::text,current_setting('application_name'),current_setting('statement_timeout'),current_setting('lock_timeout')"
    ).fetch_one(&quota).await?;
    assert_eq!(
        observed,
        (
            role,
            "public".into(),
            "gateway-quota".into(),
            "200ms".into(),
            "150ms".into()
        )
    );

    let policies = AdmissionPolicies::default().with_budget(RouteClass::Read, 2);
    let first = InMemoryRateLimiter::production(policies.clone()).with_database(quota.clone());
    let second = InMemoryRateLimiter::production(policies).with_database(peer.clone());
    for (limiter, expected) in [
        (&first, AdmissionOutcome::Admitted),
        (&second, AdmissionOutcome::Admitted),
        (&first, AdmissionOutcome::RejectedQuota),
    ] {
        let (outcome, wait) = limiter
            .admit_observed(RouteClass::Read, "external-quota-fixture")
            .await;
        assert_eq!(outcome, expected);
        assert!(
            wait.is_some(),
            "local fallback must not count as shared-store acceptance"
        );
    }
    let requests: i32 = sqlx::query_scalar(
        "select request_count from public.gateway_rate_limit_keys where client_key=$1",
    )
    .bind(format!("{}:external-quota-fixture", RouteClass::Read))
    .fetch_one(&owner)
    .await?;
    assert_eq!(requests, 2);
    let error = sqlx::query("select pg_sleep(1)")
        .execute(&quota)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("57014")
    );
    let mut barrier = owner.begin().await?;
    sqlx::query("lock table public.gateway_rate_limit_keys in access exclusive mode")
        .execute(&mut *barrier)
        .await?;
    let error = sqlx::query("select count(*) from public.gateway_rate_limit_keys")
        .execute(&quota)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("55P03")
    );
    barrier.rollback().await?;
    let (outcome, wait) = first
        .admit_observed(RouteClass::Read, "external-quota-fixture")
        .await;
    assert_eq!(outcome, AdmissionOutcome::RejectedQuota);
    assert!(
        wait.is_some(),
        "store recovery must retain the spent shared budget"
    );

    // Startup compatibility keeps URL-selected schemas; external overrides only
    // that selection and does not drop statement/lock timeout options.
    settings.database_migration_mode = migration_runtime::MigrationMode::Startup;
    let compatibility = connect_quota_pool(&settings).await?;
    let schema: String = sqlx::query_scalar("show search_path")
        .fetch_one(&compatibility)
        .await?;
    assert_eq!(schema, "shadow");
    compatibility.close().await;
    main.close().await;
    quota.close().await;
    peer.close().await;
    owner.close().await;
    if let Some(container) = pg {
        container.rm().await?;
    }
    Ok(())
}
