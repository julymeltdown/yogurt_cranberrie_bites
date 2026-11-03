//! Real-PostgreSQL acceptance coverage for shared limits and durable dispatch.

#[path = "support/mod.rs"]
mod support;

use api_gateway::adapters::r#in::http::request_journal;
use api_gateway::config::{AppState, InMemoryRateLimiter};
use api_gateway::domain::admission::{AdmissionOutcome, AdmissionPolicies, RouteClass};
use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use tower::ServiceExt;
use uuid::Uuid;

fn journal_ports(
    pool: &sqlx::PgPool,
) -> api_gateway::application::journal_runtime_ports::JournalRuntimePorts {
    api_gateway::config::journal_runtime_ports(
        pool,
        &api_gateway::config::GatewayMetrics::default(),
    )
}

const USER: &str = "11111111-1111-1111-1111-111111111111";

/// The lock bound migration 0006 sets for the gateway admission function
/// (set_config lock_timeout 150ms). The product fails closed when it is
/// exceeded, so a test that wants to observe a lock wait must keep its own
/// contribution inside this budget.
const PRODUCT_LOCK_TIMEOUT: Duration = Duration::from_millis(150);

/// The controlled hold the test injects while the admission call waits on the
/// bucket lock. It must sit strictly inside PRODUCT_LOCK_TIMEOUT so the call
/// is admitted rather than failed closed, and it must exceed the 30 ms floor
/// the assertion checks on the sampled lock wait.
const CONTROLLED_HOLD: Duration = Duration::from_millis(60);

struct WriteState {
    calls: AtomicUsize,
    started: Notify,
    delay: Duration,
}

impl WriteState {
    fn new(delay: Duration) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            started: Notify::new(),
            delay,
        })
    }
}

async fn write(State(state): State<Arc<WriteState>>) -> Json<Value> {
    let value = state.calls.fetch_add(1, Ordering::SeqCst) + 1;
    state.started.notify_one();
    tokio::time::sleep(state.delay).await;
    Json(json!({"value": value}))
}

async fn journal(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    request_journal::dispatch(state, request, next).await
}

/// A handler standing in for an auth route: it answers the same way sign-in does,
/// without touching the journal.
async fn auth_route() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"code": "INVALID_CREDENTIALS"})),
    )
        .into_response()
}

fn auth_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/auth/login", post(auth_route))
        .layer(from_fn_with_state(state, journal))
}

#[tokio::test]
async fn authentication_does_not_enter_the_operation_journal() -> anyhow::Result<()> {
    let app = support::start_gateway_app(true).await?;
    let auth_requests = auth_router(app.state.clone());

    // A failed sign-in. Auth keeps its own durable record of this, and a journal
    // row would be a second account of the same fact that disagrees with the
    // first exactly when it matters: an ordinary rejection would become an
    // UNKNOWN outcome an operator has to reconcile.
    let response = auth_requests
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"email":"a@b.test","password":"wrong"}"#))
                .expect("request"),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let journal_rows: i64 = sqlx::query_scalar(
        "select count(*) from gateway_request_journal where resource_path = '/auth/login'",
    )
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(
        journal_rows, 0,
        "an auth request must not leave a journal row to reconcile"
    );

    // The mutation path still journals, so this is an exemption for auth rather
    // than the journal having stopped recording.
    let writes = WriteState::new(Duration::ZERO);
    let mutation_requests = router(app.state.clone(), writes.clone());
    let idempotency_key = format!("journal-still-records-{}", Uuid::new_v4());
    let response = mutation_requests
        .oneshot(request(&idempotency_key, "{}"))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let recorded: i64 = sqlx::query_scalar(
        "select count(*) from gateway_request_journal where idempotency_key = $1",
    )
    .bind(&idempotency_key)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(recorded, 1);

    app.shutdown().await?;
    Ok(())
}

fn router(state: Arc<AppState>, writes: Arc<WriteState>) -> Router {
    Router::new()
        .route("/api/test-write", post(write))
        .with_state(writes)
        .layer(from_fn_with_state(state, journal))
}

fn request(key: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/test-write")
        .header(
            "authorization",
            format!("Bearer {}", support::bearer_token(USER, &["ROLE_USER"])),
        )
        .header("content-type", "application/json")
        .header("idempotency-key", key)
        .body(Body::from(body.to_owned()))
        .expect("request")
}

async fn json_body(response: Response) -> Value {
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    serde_json::from_slice(&body).expect("JSON")
}

#[tokio::test]
async fn retention_purge_removes_expired_records_and_keeps_reconciliation_evidence(
) -> anyhow::Result<()> {
    let app = support::start_gateway_app(true).await?;
    let expired_telemetry_id = format!("old-telemetry-{}", Uuid::new_v4());
    let fresh_telemetry_id = format!("fresh-telemetry-{}", Uuid::new_v4());
    let expired_key = format!("expired-key-{}", Uuid::new_v4());
    let live_key = format!("live-key-{}", Uuid::new_v4());
    let old_audit_id = Uuid::new_v4();
    let unresolved_audit_id = Uuid::new_v4();
    let fresh_audit_id = Uuid::new_v4();

    // Expired telemetry and rate-limit rows, plus audit rows on both sides of the
    // retention window.
    sqlx::query(
        "insert into telemetry_records (id, correlation_id, route_key, outcome, latency_ms, downstream_latencies, recorded_at) values ($1, $2, 'POST /api/x', 'SUCCESS', 1, '{}', now() - interval '400 days'), ($3, $4, 'POST /api/x', 'SUCCESS', 1, '{}', now())",
    )
    .bind(&expired_telemetry_id)
    .bind(format!("correlation-{}", Uuid::new_v4()))
    .bind(&fresh_telemetry_id)
    .bind(format!("correlation-{}", Uuid::new_v4()))
    .execute(&app.pool)
    .await?;
    sqlx::query(
        "insert into gateway_rate_limit_keys (client_key, bucket, request_count, expires_at) values ($1, 1, 1, now() - interval '1 hour'), ($2, 1, 1, now() + interval '1 hour')",
    )
    .bind(&expired_key)
    .bind(&live_key)
    .execute(&app.pool)
    .await?;
    sqlx::query(
        r#"
        insert into admin_audit_events (id, actor_id, action, resource_type, outcome, metadata_json, occurred_at) values
            ($1, 'actor', 'OLD', 'x', 'SUCCESS', '{}', now() - interval '400 days'),
            ($2, 'actor', 'OLD', 'x', 'SUCCESS', '{}', now() - interval '400 days'),
            ($3, 'actor', 'FRESH', 'x', 'SUCCESS', '{}', now())
        "#,
    )
    .bind(old_audit_id)
    .bind(unresolved_audit_id)
    .bind(fresh_audit_id)
    .execute(&app.pool)
    .await?;
    // The middle row is an unresolved outcome, which is the evidence an operator
    // needs. It is past the window and must survive anyway.
    sqlx::query(
        r#"
update admin_audit_events set outcome = 'UNKNOWN', metadata_json = '{"reconciliation":"required"}'
where id = $1
"#,
    )
    .bind(unresolved_audit_id)
    .execute(&app.pool)
    .await?;

    let outcome = match api_gateway::adapters::r#in::http::request_journal::purge_expired_records(
        &journal_ports(&app.pool),
        30,
        365,
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(error) => {
            app.shutdown().await?;
            panic!("purge failed: {error:?}");
        }
    };

    // The purge reports what it removed, so a purge that stops making progress is
    // visible rather than indistinguishable from one with nothing to do.
    assert!(outcome.telemetry_records >= 1);
    assert!(outcome.audit_events >= 1);
    assert!(outcome.rate_limit_keys >= 1);
    assert!(outcome.total() >= 3);

    let old_telemetry: i64 =
        sqlx::query_scalar("select count(*) from telemetry_records where id = $1")
            .bind(&expired_telemetry_id)
            .fetch_one(&app.pool)
            .await?;
    let fresh_telemetry: i64 =
        sqlx::query_scalar("select count(*) from telemetry_records where id = $1")
            .bind(&fresh_telemetry_id)
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(old_telemetry, 0, "expired telemetry must be purged");
    assert_eq!(fresh_telemetry, 1, "fresh telemetry must survive");

    let expired_keys: i64 =
        sqlx::query_scalar("select count(*) from gateway_rate_limit_keys where client_key = $1")
            .bind(&expired_key)
            .fetch_one(&app.pool)
            .await?;
    let live_keys: i64 =
        sqlx::query_scalar("select count(*) from gateway_rate_limit_keys where client_key = $1")
            .bind(&live_key)
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(expired_keys, 0, "expired rate-limit keys must be purged");
    assert_eq!(live_keys, 1, "live rate-limit keys must survive");

    let expired_audit: i64 =
        sqlx::query_scalar("select count(*) from admin_audit_events where id = $1")
            .bind(old_audit_id)
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(expired_audit, 0, "expired resolved audit must be purged");
    let unresolved: i64 = sqlx::query_scalar(
        "select count(*) from admin_audit_events where id = $1 and outcome = 'UNKNOWN'",
    )
    .bind(unresolved_audit_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(
        unresolved, 1,
        "a reconciliation-required audit row must not be deleted by age"
    );
    let remaining_audit: i64 =
        sqlx::query_scalar("select count(*) from admin_audit_events where id in ($1, $2)")
            .bind(unresolved_audit_id)
            .bind(fresh_audit_id)
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(remaining_audit, 2, "the fresh audit row must survive");

    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn reconciliation_backlog_reports_outstanding_unknown_operations_and_their_age(
) -> anyhow::Result<()> {
    let app = support::start_gateway_app(true).await?;
    let baseline = api_gateway::adapters::r#in::http::request_journal::reconciliation_backlog(
        &journal_ports(&app.pool),
    )
    .await?;
    let oldest_operation_id = Uuid::new_v4();
    let newer_operation_id = Uuid::new_v4();
    let completed_operation_id = Uuid::new_v4();
    let actor_id = format!("durable-runtime-{}", Uuid::new_v4());

    // A fresh UNKNOWN row is a dependency blip; an aged one means reconciliation
    // is not happening at all. The age is what an operator alerts on, so it has to
    // be reported alongside the count rather than derived from the row set.
    sqlx::query(
        r#"
        insert into gateway_request_journal (id, actor_id, action, resource_path, state, outcome, outcome_evidence, created_at)
        values
            ($1, $4, 'POST /api/x', '/api/x', 'UNKNOWN', 'UNKNOWN', 'LEGACY_UNRESOLVED', now() - interval '2 hours'),
            ($2, $4, 'POST /api/x', '/api/x', 'UNKNOWN', 'UNKNOWN', 'LEGACY_UNRESOLVED', now() - interval '1 hour'),
            ($3, $4, 'POST /api/y', '/api/y', 'COMPLETED', 'COMMITTED', 'SUCCESS_RESPONSE', now() - interval '3 hours')
        "#,
    )
    .bind(oldest_operation_id)
    .bind(newer_operation_id)
    .bind(completed_operation_id)
    .bind(actor_id)
    .execute(&app.pool)
    .await?;

    let backlog = api_gateway::adapters::r#in::http::request_journal::reconciliation_backlog(
        &journal_ports(&app.pool),
    )
    .await?;

    // Only UNKNOWN rows are outstanding: a completed operation has an outcome and
    // needs nothing from an operator.
    assert_eq!(backlog.outstanding, baseline.outstanding + 2);
    // The oldest of the two, not the newest and not an average.
    let expected_oldest_age = baseline.oldest_age_seconds.max(7_200);
    assert!(
        (expected_oldest_age..=expected_oldest_age + 10).contains(&backlog.oldest_age_seconds),
        "oldest age was {} seconds; baseline was {} seconds",
        backlog.oldest_age_seconds,
        baseline.oldest_age_seconds,
    );

    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn reconciliation_backlog_is_stable_and_reports_zero_when_empty() -> anyhow::Result<()> {
    let app = support::start_gateway_app(true).await?;

    let baseline = api_gateway::adapters::r#in::http::request_journal::reconciliation_backlog(
        &journal_ports(&app.pool),
    )
    .await?;
    let backlog = api_gateway::adapters::r#in::http::request_journal::reconciliation_backlog(
        &journal_ports(&app.pool),
    )
    .await?;

    // Zero, not a null that a dashboard would render as a gap. A healthy system
    // must be distinguishable from a missing reading.
    assert_eq!(backlog.outstanding, baseline.outstanding);
    if baseline.outstanding == 0 {
        assert_eq!(backlog.oldest_age_seconds, 0);
    } else {
        assert!(backlog.oldest_age_seconds >= baseline.oldest_age_seconds);
    }

    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn a_second_purge_pass_is_bounded_and_makes_no_further_progress() -> anyhow::Result<()> {
    let app = support::start_gateway_app(true).await?;
    let telemetry_id = format!("expired-telemetry-{}", Uuid::new_v4());
    let correlation_id = format!("correlation-{}", Uuid::new_v4());
    sqlx::query(
        "insert into telemetry_records (id, correlation_id, route_key, outcome, latency_ms, downstream_latencies, recorded_at) values ($1, $2, 'POST /api/x', 'SUCCESS', 1, '{}', now() - interval '400 days')",
    )
    .bind(&telemetry_id)
    .bind(&correlation_id)
    .execute(&app.pool)
    .await?;

    let first = api_gateway::adapters::r#in::http::request_journal::purge_expired_records(
        &journal_ports(&app.pool),
        30,
        365,
    )
    .await?;
    let second = api_gateway::adapters::r#in::http::request_journal::purge_expired_records(
        &journal_ports(&app.pool),
        30,
        365,
    )
    .await?;

    assert_eq!(first.telemetry_records, 1);
    // Running again is a no-op rather than an error, so the schedule can call it
    // every minute without special-casing an empty backlog.
    assert_eq!(second.telemetry_records, 0);
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn independent_gateway_instances_share_one_atomic_quota_per_class() -> anyhow::Result<()> {
    let app = support::start_gateway_app(false).await?;
    let first = shared_limiter(&app, AdmissionPolicies::uniform(7));
    let second = shared_limiter(&app, AdmissionPolicies::uniform(7));
    let network = format!("203.0.113.10-{}", Uuid::new_v4());
    let mut jobs = tokio::task::JoinSet::new();
    for index in 0..24 {
        let limiter = if index % 2 == 0 {
            first.clone()
        } else {
            second.clone()
        };
        let network = network.clone();
        jobs.spawn(async move { limiter.admit(RouteClass::Write, &network).await });
    }
    let mut admitted = 0;
    while let Some(result) = jobs.join_next().await {
        admitted += usize::from(result? == AdmissionOutcome::Admitted);
    }
    assert_eq!(admitted, 7);
    assert_eq!(
        second.admit(RouteClass::Write, &network).await,
        AdmissionOutcome::RejectedQuota
    );
    // Another class from the same network keeps its own budget on both.
    assert_eq!(
        second.admit(RouteClass::Login, &network).await,
        AdmissionOutcome::Admitted
    );
    sqlx::query(
        "update gateway_rate_limit_keys set expires_at = now() - interval '1 second' where client_key in ($1, $2)",
    )
        .bind(format!("write:{network}"))
        .bind(format!("login:{network}"))
        .execute(&app.pool)
        .await?;
    assert_eq!(
        first.admit(RouteClass::Write, &network).await,
        AdmissionOutcome::Admitted
    );
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn admission_quota_is_shared_across_processes() -> anyhow::Result<()> {
    const HELPER_FLAG: &str = "GATEWAY_ADMISSION_SUBPROCESS_HELPER";
    const DATABASE_URL: &str = "GATEWAY_ADMISSION_SUBPROCESS_DATABASE_URL";
    const IDENTITY: &str = "GATEWAY_ADMISSION_SUBPROCESS_IDENTITY";

    let app = support::start_gateway_app(false).await?;
    let limiter = shared_limiter(&app, AdmissionPolicies::uniform(1));
    let network = format!("203.0.113.11-{}", Uuid::new_v4());
    assert_eq!(
        limiter.admit(RouteClass::Write, &network).await,
        AdmissionOutcome::Admitted
    );

    let executable = std::env::current_exe()?;
    let database_url = app.config.database_url.clone();
    let helper_network = network.clone();
    let child_result = tokio::task::spawn_blocking(move || {
        Command::new(executable)
            .args(["--exact", "admission_subprocess_helper", "--nocapture"])
            .env(HELPER_FLAG, "1")
            .env(DATABASE_URL, database_url)
            .env(IDENTITY, helper_network)
            .output()
    })
    .await;
    app.shutdown().await?;
    let child = child_result??;
    if !child.status.success() {
        anyhow::bail!(
            "separate Gateway test process did not observe the spent quota\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
    }

    Ok(())
}

/// Re-entered by `admission_quota_is_shared_across_processes` as a second OS
/// process. The normal test run invokes this without the private flag, so it
/// remains a cheap no-op outside the parent regression.
#[tokio::test]
async fn admission_subprocess_helper() -> anyhow::Result<()> {
    const HELPER_FLAG: &str = "GATEWAY_ADMISSION_SUBPROCESS_HELPER";
    const DATABASE_URL: &str = "GATEWAY_ADMISSION_SUBPROCESS_DATABASE_URL";
    const IDENTITY: &str = "GATEWAY_ADMISSION_SUBPROCESS_IDENTITY";

    if std::env::var(HELPER_FLAG).as_deref() != Ok("1") {
        return Ok(());
    }
    let database_url = std::env::var(DATABASE_URL)?;
    let network = std::env::var(IDENTITY)?;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await?;
    let limiter =
        InMemoryRateLimiter::with_policies(AdmissionPolicies::uniform(1), Duration::from_secs(60))
            .with_database(pool.clone());

    let outcome = limiter.admit(RouteClass::Write, &network).await;
    pool.close().await;
    assert_eq!(outcome, AdmissionOutcome::RejectedQuota);
    Ok(())
}

#[tokio::test]
async fn independent_gateway_instances_share_keyed_identity_budgets() -> anyhow::Result<()> {
    let app = support::start_gateway_app(false).await?;
    let policies = AdmissionPolicies::default().with_identity_budget(RouteClass::Login, 1);
    let first = shared_limiter(&app, policies.clone());
    let second = shared_limiter(&app, policies);
    let same_identity = Uuid::new_v4().simple().to_string().repeat(2);
    let other_identity = Uuid::new_v4().simple().to_string().repeat(2);

    assert_eq!(
        first
            .admit_identity(RouteClass::Login, &same_identity)
            .await,
        AdmissionOutcome::Admitted
    );
    assert_eq!(
        second
            .admit_identity(RouteClass::Login, &same_identity)
            .await,
        AdmissionOutcome::RejectedQuota
    );
    assert_eq!(
        second
            .admit_identity(RouteClass::Login, &other_identity)
            .await,
        AdmissionOutcome::Admitted
    );

    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn independent_gateway_instances_share_verified_principal_budgets() -> anyhow::Result<()> {
    let app = support::start_gateway_app(false).await?;
    let policies = AdmissionPolicies::default().with_principal_budget(RouteClass::Search, 1);
    let first = shared_limiter(&app, policies.clone());
    let second = shared_limiter(&app, policies);
    let secret = b"test-admission-hmac-key-with-at-least-32-bytes";
    let same_principal =
        api_gateway::adapters::r#in::http::client_identity::keyed_admission_principal(
            secret,
            &format!("user-actor-{}", Uuid::new_v4()),
        );
    let other_principal =
        api_gateway::adapters::r#in::http::client_identity::keyed_admission_principal(
            secret,
            &format!("user-actor-{}", Uuid::new_v4()),
        );

    assert_eq!(
        first
            .admit_principal(RouteClass::Search, &same_principal)
            .await,
        AdmissionOutcome::Admitted
    );
    assert_eq!(
        second
            .admit_principal(RouteClass::Search, &same_principal)
            .await,
        AdmissionOutcome::RejectedQuota
    );
    assert_eq!(
        second
            .admit_principal(RouteClass::Search, &other_principal)
            .await,
        AdmissionOutcome::Admitted
    );

    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn admission_reports_real_postgres_advisory_lock_wait() -> anyhow::Result<()> {
    let app = support::start_gateway_app(false).await?;
    let policies = AdmissionPolicies::default().with_budget(RouteClass::Login, 10);
    let limiter = shared_limiter(&app, policies);
    let network = format!("203.0.113.91-{}", Uuid::new_v4());
    let key = format!("login:{network}");
    let bucket: i32 = sqlx::query_scalar("select (hashtextextended($1, 0) & 63)::integer")
        .bind(&key)
        .fetch_one(&app.pool)
        .await?;

    let mut blocker = app.pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock(724006, $1)")
        .bind(bucket)
        .execute(&mut *blocker)
        .await?;

    let waiting_limiter = limiter.clone();
    let waiting_network = network.to_owned();
    let waiting = tokio::spawn(async move {
        waiting_limiter
            .admit_observed(RouteClass::Login, &waiting_network)
            .await
    });

    // Hold the bucket lock and release it on PostgreSQL's own clock. Observing
    // the waiter and enforcing the controlled hold happen inside one server-side
    // statement, so client round-trip latency on a loaded host cannot stretch the
    // wait past the product's deployed lock_timeout. Migration 0006 fails the
    // admission closed with RejectedStoreUnavailable once that bound is exceeded,
    // and that is correct product behaviour rather than something to relax here.
    assert!(
        CONTROLLED_HOLD < PRODUCT_LOCK_TIMEOUT,
        "the controlled hold must stay inside the product lock bound"
    );
    // The hold is a deadline measured from when PostgreSQL first saw the
    // waiter, not a sleep appended after the test noticed it. Polling from the
    // client adds its own latency before the hold even starts, which on a loaded
    // host pushes the total wait past the product bound and the admission then
    // correctly fails closed. Waiting to the deadline inside the server keeps
    // the injected wait at CONTROLLED_HOLD however slow the client is.
    let hold_seconds = CONTROLLED_HOLD.as_secs_f64();
    let hold = format!(
        "do $do$
         declare seen_at timestamptz;
         begin
           for _ in 1..2000 loop
             select activity.query_start into seen_at
             from pg_locks waiting
             join pg_stat_activity activity on activity.pid = waiting.pid
             where waiting.locktype = 'advisory' and waiting.classid = 724006::oid
               and waiting.objid = {bucket}::oid and waiting.objsubid = 2
               and not waiting.granted
             limit 1;
             exit when seen_at is not null;
             perform pg_sleep(0.002);
           end loop;
           if seen_at is null then
             raise exception 'admission query never appeared as waiting on its bucket lock';
           end if;
           perform pg_sleep(greatest(0.0, {hold_seconds} - extract(epoch from clock_timestamp() - seen_at)));
         end $do$"
    );
    sqlx::query(&hold).execute(&mut *blocker).await?;
    blocker.commit().await?;
    let (outcome, lock_wait) = waiting.await?;
    assert_eq!(outcome, AdmissionOutcome::Admitted);
    assert!(
        lock_wait.expect("PostgreSQL lock-wait sample") >= Duration::from_millis(30),
        "direct database lock wait should include the controlled hold"
    );

    // Keep the old function signature operational for a previously deployed
    // Gateway during an application rollback after migration 0006.
    let legacy: String = sqlx::query_scalar("select gateway_admit($1, 'login', 1, 60, 256, 10)")
        .bind(format!("login:legacy-wrapper-check:{}", Uuid::new_v4()))
        .fetch_one(&app.pool)
        .await?;
    assert_eq!(legacy, "admitted");

    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn identity_churn_against_a_full_store_cannot_reset_any_quota() -> anyhow::Result<()> {
    // Given: two instances sharing the store, and a client that has spent its
    // write budget through them.
    let app = support::start_gateway_app(false).await?;
    let mut policies = AdmissionPolicies::uniform(3);
    policies.overflow_factor = 2;
    let first = shared_limiter(&app, policies.clone());
    let second = shared_limiter(&app, policies);
    let client_network = format!("203.0.113.30-{}", Uuid::new_v4());
    let churn_prefix = format!("198.51.100-{}", Uuid::new_v4());
    for limiter in [&first, &second, &first] {
        assert_eq!(
            limiter.admit(RouteClass::Write, &client_network).await,
            AdmissionOutcome::Admitted
        );
    }

    // When: detailed capacity is full on both instances and fifty identities
    // the store has never seen arrive through them.
    let full_first = first.clone().with_bucket_capacity(0);
    let full_second = second.clone().with_bucket_capacity(0);
    let mut outcomes = Vec::new();
    for index in 0..50 {
        let limiter = if index % 2 == 0 {
            &full_first
        } else {
            &full_second
        };
        outcomes.push(
            limiter
                .admit(RouteClass::Write, &format!("{churn_prefix}-{index}"))
                .await,
        );
    }

    // Then: they shared one overflow budget of 2 × 3 for the class and were
    // refused beyond it, none of them received a budget of its own, and the
    // client that spent its budget is still refused everywhere.
    let count = |wanted: AdmissionOutcome| outcomes.iter().filter(|o| **o == wanted).count();
    assert_eq!(count(AdmissionOutcome::AdmittedOverflow), 6);
    assert_eq!(count(AdmissionOutcome::RejectedCardinality), 44);
    let churned: i64 =
        sqlx::query_scalar("select count(*) from gateway_rate_limit_keys where client_key like $1")
            .bind(format!("write:{churn_prefix}-%"))
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(churned, 0);
    for limiter in [&first, &second, &full_first, &full_second] {
        assert_eq!(
            limiter.admit(RouteClass::Write, &client_network).await,
            AdmissionOutcome::RejectedQuota
        );
    }
    let write_overflow_rows: i64 = sqlx::query_scalar(
        "select count(*) from gateway_rate_limit_overflow where route_class = 'write'",
    )
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(
        write_overflow_rows, 1,
        "write overflow state is one row per class"
    );

    // Once the window passes, both budgets are available again.
    sqlx::query(
        "update gateway_rate_limit_keys set expires_at = now() - interval '1 second' where client_key = $1 or client_key like $2",
    )
        .bind(format!("write:{client_network}"))
        .bind(format!("write:{churn_prefix}-%"))
        .execute(&app.pool)
        .await?;
    sqlx::query(
        "update gateway_rate_limit_overflow set expires_at = now() - interval '1 second' where route_class = 'write'",
    )
        .execute(&app.pool)
        .await?;
    assert_eq!(
        first.admit(RouteClass::Write, &client_network).await,
        AdmissionOutcome::Admitted
    );
    assert_eq!(
        full_second
            .admit(RouteClass::Write, &format!("{churn_prefix}-overflow"))
            .await,
        AdmissionOutcome::AdmittedOverflow
    );
    sqlx::query(
        "update gateway_rate_limit_overflow set expires_at = now() - interval '1 second' where route_class = 'write'",
    )
    .execute(&app.pool)
    .await?;
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn a_stalled_store_fails_sensitive_classes_closed_and_admission_recovers(
) -> anyhow::Result<()> {
    // Given: another session holds the store's bucket locks for this client,
    // as a stalled or overloaded store would.
    let app = support::start_gateway_app(false).await?;
    let mut policies = AdmissionPolicies::uniform(100);
    policies.fallback_per_second = 2;
    let limiter = shared_limiter(&app, policies);
    let mut holder = app.pool.acquire().await?;
    for key in [
        "write:203.0.113.40",
        "login:203.0.113.40",
        "read:203.0.113.40",
    ] {
        sqlx::query("select pg_advisory_lock(724006, (hashtextextended($1, 0) & 63)::integer)")
            .bind(key)
            .execute(&mut *holder)
            .await?;
    }

    // When/Then: sensitive classes are refused as a store failure, and reads
    // are admitted only from the small per-instance allowance.
    assert_eq!(
        limiter.admit(RouteClass::Write, "203.0.113.40").await,
        AdmissionOutcome::RejectedStoreUnavailable
    );
    assert_eq!(
        limiter.admit(RouteClass::Login, "203.0.113.40").await,
        AdmissionOutcome::RejectedStoreUnavailable
    );
    let reads = [
        limiter.admit(RouteClass::Read, "203.0.113.40").await,
        limiter.admit(RouteClass::Read, "203.0.113.40").await,
        limiter.admit(RouteClass::Read, "203.0.113.40").await,
    ];
    assert_eq!(
        reads,
        [
            AdmissionOutcome::AdmittedFallback,
            AdmissionOutcome::AdmittedFallback,
            AdmissionOutcome::RejectedFallbackExhausted
        ]
    );

    // Recovery: once the store answers again, the same limiter uses it; there
    // is no fallback mode to leave.
    sqlx::query("select pg_advisory_unlock_all()")
        .execute(&mut *holder)
        .await?;
    drop(holder);
    assert_eq!(
        limiter.admit(RouteClass::Write, "203.0.113.40").await,
        AdmissionOutcome::Admitted
    );
    assert_eq!(
        limiter.admit(RouteClass::Read, "203.0.113.40").await,
        AdmissionOutcome::Admitted
    );

    // A store that is gone entirely behaves the same way.
    app.pool.close().await;
    assert_eq!(
        limiter.admit(RouteClass::Refresh, "203.0.113.41").await,
        AdmissionOutcome::RejectedStoreUnavailable
    );
    app.shutdown().await?;
    Ok(())
}

fn shared_limiter(app: &support::TestApp, policies: AdmissionPolicies) -> InMemoryRateLimiter {
    InMemoryRateLimiter::with_policies(policies, Duration::from_secs(60))
        .with_database(app.pool.clone())
}

#[tokio::test]
async fn duplicate_requests_replay_without_repeating_the_mutation() -> anyhow::Result<()> {
    let app = support::start_gateway_app(true).await?;
    let writes = WriteState::new(Duration::ZERO);
    let router = router(app.state.clone(), writes.clone());
    let idempotency_key = format!("same-operation-{}", Uuid::new_v4());
    let first = router
        .clone()
        .oneshot(request(&idempotency_key, "{\"a\":1}"))
        .await?;
    assert_eq!(first.status(), StatusCode::OK);
    let operation_id_header = first
        .headers()
        .get("x-operation-id")
        .expect("operation ID")
        .clone();
    let operation_id = Uuid::parse_str(operation_id_header.to_str()?)?;
    let first_body = json_body(first).await;
    let replay = router
        .clone()
        .oneshot(request(&idempotency_key, "{\"a\":1}"))
        .await?;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(
        replay.headers().get("x-operation-id"),
        Some(&operation_id_header)
    );
    assert_eq!(
        replay
            .headers()
            .get("idempotency-replayed")
            .expect("replay header"),
        "true"
    );
    assert_eq!(json_body(replay).await, first_body);
    let conflict = router
        .oneshot(request(&idempotency_key, "{\"a\":2}"))
        .await?;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    assert_eq!(writes.calls.load(Ordering::SeqCst), 1);
    let count: i64 = sqlx::query_scalar(
        "select count(*) from admin_audit_events where id = $1 and action = 'POST /api/test-write'",
    )
    .bind(operation_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(count, 1);
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn concurrent_duplicate_keys_admit_at_most_one_operation() -> anyhow::Result<()> {
    let app = support::start_gateway_app(true).await?;
    let writes = WriteState::new(Duration::from_millis(50));
    let router = router(app.state.clone(), writes.clone());
    let idempotency_key = format!("concurrent-operation-{}", Uuid::new_v4());
    let mut jobs = tokio::task::JoinSet::new();
    for _ in 0..12 {
        let router = router.clone();
        let idempotency_key = idempotency_key.clone();
        jobs.spawn(async move { router.oneshot(request(&idempotency_key, "{}")).await });
    }
    while let Some(result) = jobs.join_next().await {
        let response = result??;
        assert!(matches!(
            response.status(),
            StatusCode::OK | StatusCode::CONFLICT
        ));
    }
    assert_eq!(writes.calls.load(Ordering::SeqCst), 1);
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn cancellation_after_dispatch_still_records_completion_and_supports_replay(
) -> anyhow::Result<()> {
    let app = support::start_gateway_app(true).await?;
    let writes = WriteState::new(Duration::from_millis(150));
    let router = router(app.state.clone(), writes.clone());
    let idempotency_key = format!("cancelled-request-{}", Uuid::new_v4());
    let first_key = idempotency_key.clone();
    let first_router = router.clone();
    let first = tokio::spawn(async move { first_router.oneshot(request(&first_key, "{}")).await });
    tokio::time::timeout(Duration::from_secs(5), writes.started.notified()).await?;
    first.abort();
    let _ = first.await;
    tokio::time::timeout(Duration::from_secs(5), request_journal::drain()).await??;
    let replay = router.oneshot(request(&idempotency_key, "{}")).await?;
    assert_eq!(replay.status(), StatusCode::OK);
    let operation_id = Uuid::parse_str(
        replay
            .headers()
            .get("x-operation-id")
            .and_then(|value| value.to_str().ok())
            .expect("operation id"),
    )?;
    assert_eq!(json_body(replay).await["value"], 1);
    assert_eq!(writes.calls.load(Ordering::SeqCst), 1);
    let count: i64 = sqlx::query_scalar("select count(*) from admin_audit_events where id = $1 and action = 'POST /api/test-write' and outcome = 'SUCCESS'")
        .bind(operation_id)
        .fetch_one(&app.pool).await?;
    assert_eq!(count, 1);
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn abandoned_dispatch_is_audited_once_without_automatic_reexecution() -> anyhow::Result<()> {
    let app = support::start_gateway_app(true).await?;
    let id = Uuid::new_v4();
    sqlx::query("insert into gateway_request_journal (id, actor_id, action, resource_path, created_at) values ($1, $2, 'POST /api/test-write', '/api/test-write', now() - interval '6 minutes')")
        .bind(id).bind(USER).execute(&app.pool).await?;
    request_journal::reconcile_stale(&journal_ports(&app.pool)).await?;
    request_journal::reconcile_stale(&journal_ports(&app.pool)).await?;
    let recorded: (String, String, String) = sqlx::query_as(
        "select state, outcome, outcome_evidence from gateway_request_journal where id = $1",
    )
    .bind(id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(
        recorded,
        ("UNKNOWN".into(), "UNKNOWN".into(), "STALE_OPERATION".into())
    );
    let count: i64 = sqlx::query_scalar(
        "select count(*) from admin_audit_events where id = $1 and outcome = 'UNKNOWN'",
    )
    .bind(id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(count, 1);
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn a_timed_out_operation_still_reports_its_operation_id() -> anyhow::Result<()> {
    let app = support::start_gateway_app(true).await?;
    // The write never finishes, so the journal's client window expires and the
    // caller must still learn which operation it gave up on. Without the id the
    // caller has a 504 and no way to check the outcome, so its only safe action is
    // not to retry -- and the row becomes operator work the caller could have
    // identified itself.
    let writes = WriteState::new(Duration::from_secs(30));
    let router = router(app.state.clone(), writes.clone());
    let idempotency_key = format!("timed-out-operation-{}", Uuid::new_v4());
    let response = tokio::time::timeout(
        Duration::from_secs(10),
        router.oneshot(request(&idempotency_key, "{}")),
    )
    .await
    .expect("the journal must answer within its own budget")?;

    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    let operation_id = response
        .headers()
        .get("x-operation-id")
        .expect("a timed-out operation must still name its operation");
    // The id must resolve to the committed intent, which is what makes it useful.
    let known: bool = sqlx::query_scalar(
    "select exists(select 1 from gateway_request_journal where id = $1 and state = 'IN_PROGRESS')",
)
.bind(Uuid::parse_str(operation_id.to_str().expect("header")).expect("uuid"))
.fetch_one(&app.pool)
.await?;
    assert!(known, "the operation id must name the committed intent");
    assert_eq!(
        response.headers().get("cache-control"),
        Some(&axum::http::HeaderValue::from_static("no-store")),
        "a timeout is not the operation's outcome and must not be cached"
    );

    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn an_operation_id_resolves_to_its_recorded_state() -> anyhow::Result<()> {
    let app = support::start_gateway_app(true).await?;

    // A completed operation, an unresolved one, and an id that was never issued.
    // The three answers have to be distinguishable: a caller checking whether a
    // timed-out write took effect must not read an unknown id as "nothing
    // happened", and must not read an unresolved row as a success either.
    let completed = Uuid::new_v4();
    let unresolved = Uuid::new_v4();
    sqlx::query(
        r#"
        insert into gateway_request_journal
            (id, actor_id, action, resource_path, state, outcome, outcome_evidence, response_status, response_body, created_at, finished_at)
        values
            ($1, 'actor', 'POST /api/posts', '/api/posts', 'COMPLETED', 'COMMITTED', 'SUCCESS_RESPONSE', 201, '\x00'::bytea, now(), now()),
            ($2, 'actor', 'POST /api/posts', '/api/posts', 'UNKNOWN', 'UNKNOWN', 'LEGACY_UNRESOLVED', null, null, now(), now())
        "#,
    )
    .bind(completed)
    .bind(unresolved)
    .execute(&app.pool)
    .await?;

    let finished = api_gateway::adapters::r#in::http::request_journal::operation_status(
        &journal_ports(&app.pool),
        completed,
    )
    .await?
    .expect("the completed operation must resolve");
    assert_eq!(finished.state, "COMPLETED");
    assert_eq!(
        finished.outcome,
        api_gateway::domain::operation::OperationOutcome::Committed
    );
    assert_eq!(finished.response_status, Some(201));
    assert!(
        finished.replayable,
        "a stored response must report as replayable"
    );
    assert_eq!(finished.operation_id, completed);
    assert!(finished.finished_at.is_some());

    let pending = api_gateway::adapters::r#in::http::request_journal::operation_status(
        &journal_ports(&app.pool),
        unresolved,
    )
    .await?
    .expect("the unresolved operation must resolve");
    assert_eq!(pending.state, "UNKNOWN");
    assert_eq!(
        pending.outcome,
        api_gateway::domain::operation::OperationOutcome::Unknown
    );
    // No response was recorded, and that is not the same as a successful one.
    assert_eq!(pending.response_status, None);
    assert!(!pending.replayable);

    let missing = api_gateway::adapters::r#in::http::request_journal::operation_status(
        &journal_ports(&app.pool),
        Uuid::new_v4(),
    )
    .await?;
    assert!(missing.is_none(), "an unknown id must not resolve");

    app.shutdown().await?;
    Ok(())
}
