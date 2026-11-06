#[path = "support/mod.rs"]
mod support;

use api_gateway::adapters::out::repository::async_operation_queue::{
    self, EnqueueRequest, PayloadCipher,
};
use api_gateway::adapters::r#in::http::operation_worker;
use api_gateway::adapters::r#in::http::{self, client_identity::TrustedProxies};
use api_gateway::application::dto::moderation_dto::{
    CreateReportRequest, ModerationMutationContextDto,
};
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use sqlx::Row;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};
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

fn test_cipher() -> PayloadCipher {
    PayloadCipher::from_hex_keyring(&format!("1:{}", "ab".repeat(32))).expect("keyring")
}

async fn insert_journal(
    pool: &sqlx::PgPool,
    operation_id: Uuid,
    actor_id: &str,
    idempotency_key: &str,
    fingerprint: &str,
    action: &str,
    resource_path: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        insert into gateway_request_journal (
            id, actor_id, action, resource_path, idempotency_key,
            request_fingerprint, expires_at
        ) values ($1, $2, $3, $4, $5, $6, now() + interval '1 day')
        "#,
    )
    .bind(operation_id)
    .bind(actor_id)
    .bind(action)
    .bind(resource_path)
    .bind(idempotency_key)
    .bind(fingerprint)
    .execute(pool)
    .await?;
    Ok(())
}

fn enqueue_request<'a>(
    operation_id: Uuid,
    actor_id: &'a str,
    fingerprint: &'a str,
    body: &'a [u8],
    request_uri: &'a str,
) -> EnqueueRequest<'a> {
    EnqueueRequest {
        operation_id,
        actor_id,
        request_fingerprint: fingerprint,
        request_uri,
        content_type: Some("application/json"),
        body,
    }
}

async fn queue_state_snapshot(pool: &sqlx::PgPool, operation_id: Uuid) -> anyhow::Result<String> {
    let snapshot = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            Option<f64>,
            Option<i32>,
            Option<String>,
            bool,
        ),
    >(
        r#"
        select journal.state, queue.state,
               extract(epoch from (queue.available_at - now()))::double precision,
               queue.attempt_count, queue.lease_owner::text,
               exists (
                   select 1 from gateway_operation_manual_reconciliations resolution
                   where resolution.operation_id = journal.id
               )
        from gateway_request_journal journal
        left join gateway_async_operation_queue queue on queue.operation_id = journal.id
        where journal.id = $1
        "#,
    )
    .bind(operation_id)
    .fetch_optional(pool)
    .await?;
    Ok(format!("{snapshot:?}"))
}

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
async fn encrypted_queue_is_journal_bound_leased_and_reconciliation_aware() -> anyhow::Result<()> {
    let app = support::start_gateway_app(false).await?;
    let result = async {
    let cipher = test_cipher();
    let operation_id = Uuid::new_v4();
    let actor_id = format!("queue-actor-{}", Uuid::new_v4());
    let fingerprint = format!("fingerprint-{}", Uuid::new_v4());
    let idempotency_key = format!("queue-key-{}", Uuid::new_v4());
    let body = br#"{"content":"durable private payload"}"#;
    insert_journal(
        &app.pool,
        operation_id,
        &actor_id,
        &idempotency_key,
        &fingerprint,
        "POST /api/posts",
        "/api/posts",
    )
    .await?;

    assert!(
        !async_operation_queue::enqueue(
            &app.pool,
            &cipher,
            enqueue_request(
                operation_id,
                "different-actor",
                &fingerprint,
                body,
                "/api/posts",
            ),
        )
        .await?
    );
    assert!(
        !async_operation_queue::enqueue(
            &app.pool,
            &cipher,
            enqueue_request(
                operation_id,
                &actor_id,
                "different-fingerprint",
                body,
                "/api/posts",
            ),
        )
        .await?
    );
    assert!(
        async_operation_queue::enqueue(
            &app.pool,
            &cipher,
            enqueue_request(operation_id, &actor_id, &fingerprint, body, "/api/posts"),
        )
        .await?
    );
    let journal_state = sqlx::query_as::<_, (String, String)>(
        "select state, outcome from gateway_request_journal where id = $1",
    )
    .bind(operation_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(journal_state, ("QUEUED".into(), "PENDING".into()));
    assert!(
        !async_operation_queue::enqueue(
            &app.pool,
            &cipher,
            enqueue_request(operation_id, &actor_id, &fingerprint, body, "/api/posts"),
        )
        .await?
    );

    let stored = sqlx::query(
        "select payload_nonce, payload_ciphertext from gateway_async_operation_queue where operation_id = $1",
    )
    .bind(operation_id)
    .fetch_one(&app.pool)
    .await?;
    let stored_body: Vec<u8> = stored.try_get("payload_ciphertext")?;
    assert!(!stored_body.windows(body.len()).any(|window| window == body));
    let stored_nonce: Vec<u8> = stored.try_get("payload_nonce")?;
    assert_eq!(stored_nonce.len(), 12);

    assert!(async_operation_queue::claim_next(
        &app.pool,
        Uuid::new_v4(),
        Duration::from_millis(500),
    )
    .await
    .is_err());

    let first_lease = Uuid::new_v4();
    let second_lease = Uuid::new_v4();
    let (first_claim, second_claim) = tokio::join!(
        async_operation_queue::claim_next(&app.pool, first_lease, Duration::from_secs(30)),
        async_operation_queue::claim_next(&app.pool, second_lease, Duration::from_secs(30)),
    );
    let claimed = first_claim?.or(second_claim?).expect("one claim succeeds");
    assert_eq!(claimed.operation_id, operation_id);
    assert_eq!(claimed.attempt_count, 1);
    assert_eq!(claimed.decrypt_body(&cipher)?, body);

    sqlx::query(
        "update gateway_async_operation_queue set lease_expires_at = now() - interval '1 second' where operation_id = $1",
    )
    .bind(operation_id)
    .execute(&app.pool)
    .await?;
    let recovered_lease = Uuid::new_v4();
    let recovered =
        async_operation_queue::claim_next(&app.pool, recovered_lease, Duration::from_secs(30))
            .await?
            .expect("expired lease can be recovered");
    assert_eq!(recovered.attempt_count, 2);
    assert_ne!(recovered.lease_owner, claimed.lease_owner);
    assert!(
        !async_operation_queue::mark_done(&app.pool, operation_id, claimed.lease_owner,).await?
    );
    assert!(
        async_operation_queue::mark_done(&app.pool, operation_id, recovered.lease_owner,).await?
    );
    let erased = sqlx::query_scalar::<_, bool>(
        "select payload_nonce is null and payload_ciphertext is null from gateway_async_operation_queue where operation_id = $1",
    )
    .bind(operation_id)
    .fetch_one(&app.pool)
    .await?;
    assert!(erased, "completed jobs must not retain request bytes");

    for disposition in ["COMMITTED", "NOT_COMMITTED", "UNRESOLVED"] {
        let resolved_operation_id = Uuid::new_v4();
        let resolved_actor = format!("queue-actor-{}", Uuid::new_v4());
        let resolved_fingerprint = format!("fingerprint-{}", Uuid::new_v4());
        let resolved_idempotency_key = format!("queue-key-{}", Uuid::new_v4());
        insert_journal(
            &app.pool,
            resolved_operation_id,
            &resolved_actor,
            &resolved_idempotency_key,
            &resolved_fingerprint,
            "POST /api/posts",
            "/api/posts",
        )
        .await?;
        assert!(
            async_operation_queue::enqueue(
                &app.pool,
                &cipher,
                enqueue_request(
                    resolved_operation_id,
                    &resolved_actor,
                    &resolved_fingerprint,
                    body,
                    "/api/posts",
                ),
            )
            .await?
        );
        sqlx::query(
            r#"
            insert into gateway_operation_manual_reconciliations (
                resolution_id, operation_id, revision, disposition, evidence_kind,
                evidence_reference, reason, reviewer_id
            ) values ($1, $2, 1, $3, 'DATABASE_AUDIT', $4,
                      'Verified using the durable owner record.', 'queue-test-reviewer')
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(resolved_operation_id)
        .bind(disposition)
        .bind(format!("incident:queue-{}", Uuid::new_v4()))
        .execute(&app.pool)
        .await?;
    }
    for _ in 0..3 {
        assert!(
            async_operation_queue::claim_next(&app.pool, Uuid::new_v4(), Duration::from_secs(30),)
                .await?
                .is_none(),
            "any manual disposition pauses automatic queue execution"
        );
    }

    Ok(())
    }
    .await;
    app.shutdown().await?;
    result
}

#[tokio::test]
async fn queued_operation_survives_worker_process_termination_and_restart() -> anyhow::Result<()> {
    const HELPER_FLAG: &str = "GATEWAY_QUEUE_RESTART_SUBPROCESS_HELPER";
    const HELPER_DATABASE_URL: &str = "GATEWAY_QUEUE_RESTART_SUBPROCESS_DATABASE_URL";
    const HELPER_OPERATION_ID: &str = "GATEWAY_QUEUE_RESTART_SUBPROCESS_OPERATION_ID";

    let app = support::start_gateway_app(false).await?;
    let result = async {
        let cipher = test_cipher();
        let operation_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4().to_string();
        let resource_path = format!("/api/profile/{actor_id}");
        let action = format!("PUT {resource_path}");
        let fingerprint = format!("fingerprint-{}", Uuid::new_v4());
        let idempotency_key = format!("process-restart-{}", Uuid::new_v4());
        let body = br#"{"username":"restart-user","nickname":"Restart User","avatarUrl":"/avatars/restart.png","bio":"Recovered after worker loss"}"#;

        insert_journal(
            &app.pool,
            operation_id,
            &actor_id,
            &idempotency_key,
            &fingerprint,
            &action,
            &resource_path,
        )
        .await?;
        assert!(async_operation_queue::enqueue(
            &app.pool,
            &cipher,
            enqueue_request(
                operation_id,
                &actor_id,
                &fingerprint,
                body,
                &resource_path,
            ),
        )
        .await?);

        let database_url = app.config.database_url.clone();
        let executable = std::env::current_exe()?;
        let mut crashed_worker = ChildGuard(
            Command::new(executable)
                .args(["--exact", "queue_worker_process_helper", "--nocapture"])
                .env(HELPER_FLAG, "claim-and-wait")
                .env(HELPER_DATABASE_URL, &database_url)
                .env(HELPER_OPERATION_ID, operation_id.to_string())
                .env("GATEWAY_TEST_DATABASE_URL", &database_url)
                // This child must recover the parent's queue in the same fixture.
                // Only the parent creates the isolated database.
                .env_remove("GATEWAY_TEST_CREATE_ISOLATED_DATABASES")
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
                .spawn()?,
        );

        let claim_deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let leased: Option<(String, Option<Uuid>, i32)> = sqlx::query_as(
                "select state, lease_owner, attempt_count from gateway_async_operation_queue where operation_id = $1",
            )
            .bind(operation_id)
            .fetch_optional(&app.pool)
            .await?;
            if let Some((state, lease_owner, attempts)) = leased {
                if state == "LEASED" {
                    assert!(lease_owner.is_some());
                    assert_eq!(attempts, 1, "the first worker must own the first attempt");
                    break;
                }
            }
            if let Some(status) = crashed_worker.0.try_wait()? {
                anyhow::bail!("claiming worker exited before acquiring its lease: {status}");
            }
            if Instant::now() >= claim_deadline {
                anyhow::bail!("worker process did not acquire the queued operation lease");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        crashed_worker.0.kill()?;
        let killed_status = crashed_worker.0.wait()?;
        assert!(
            !killed_status.success(),
            "the first worker must be terminated before it can acknowledge the operation"
        );

        let lease_expiry_deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let lease_expired: bool = sqlx::query_scalar(
                "select state = 'LEASED' and lease_expires_at <= now() from gateway_async_operation_queue where operation_id = $1",
            )
            .bind(operation_id)
            .fetch_one(&app.pool)
            .await?;
            if lease_expired {
                break;
            }
            if Instant::now() >= lease_expiry_deadline {
                anyhow::bail!("terminated worker lease did not expire in the expected window");
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        let executable = std::env::current_exe()?;
        let database_url = app.config.database_url.clone();
        let operation_id_text = operation_id.to_string();
        let restarted_worker = tokio::task::spawn_blocking(move || {
            Command::new(executable)
                .args(["--exact", "queue_worker_process_helper", "--nocapture"])
                .env(HELPER_FLAG, "restart-and-run")
                .env(HELPER_DATABASE_URL, &database_url)
                .env(HELPER_OPERATION_ID, operation_id_text)
                .env("GATEWAY_TEST_DATABASE_URL", database_url)
                .env_remove("GATEWAY_TEST_CREATE_ISOLATED_DATABASES")
                .output()
        })
        .await??;
        if !restarted_worker.status.success() {
            anyhow::bail!(
                "replacement worker failed\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&restarted_worker.stdout),
                String::from_utf8_lossy(&restarted_worker.stderr)
            );
        }

        let recovered: (String, String, i32, bool, Option<i32>) = sqlx::query_as(
            r#"
            select queue.state, journal.state, queue.attempt_count,
                   queue.payload_ciphertext is null, journal.response_status
            from gateway_async_operation_queue queue
            join gateway_request_journal journal on journal.id = queue.operation_id
            where queue.operation_id = $1
            "#,
        )
        .bind(operation_id)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(recovered, ("DONE".into(), "COMPLETED".into(), 2, true, Some(200)));
        Ok::<(), anyhow::Error>(())
    }
    .await;
    app.shutdown().await?;
    result
}

/// Child-process entry point for the crash/restart acceptance journey above.
/// The ordinary test run leaves this as a fast no-op.
#[tokio::test]
async fn queue_worker_process_helper() -> anyhow::Result<()> {
    const HELPER_FLAG: &str = "GATEWAY_QUEUE_RESTART_SUBPROCESS_HELPER";
    const HELPER_DATABASE_URL: &str = "GATEWAY_QUEUE_RESTART_SUBPROCESS_DATABASE_URL";
    const HELPER_OPERATION_ID: &str = "GATEWAY_QUEUE_RESTART_SUBPROCESS_OPERATION_ID";

    let Ok(mode) = std::env::var(HELPER_FLAG) else {
        return Ok(());
    };
    let database_url = std::env::var(HELPER_DATABASE_URL)?;
    let operation_id = Uuid::parse_str(&std::env::var(HELPER_OPERATION_ID)?)?;
    let app = support::start_gateway_app(false).await?;
    anyhow::ensure!(
        app.config.database_url == database_url,
        "queue worker must reuse the parent's existing database"
    );
    let result = match mode.as_str() {
        "claim-and-wait" => {
            let claim = async_operation_queue::claim_next(
                &app.pool,
                Uuid::new_v4(),
                Duration::from_secs(2),
            )
            .await?
            .ok_or_else(|| anyhow::anyhow!("replacement-test operation was not claimable"))?;
            anyhow::ensure!(
                claim.operation_id == operation_id,
                "worker claimed a different operation"
            );
            // The parent kills this process while it holds the lease, emulating
            // loss before downstream dispatch/acknowledgement.
            std::future::pending::<()>().await;
            Ok(())
        }
        "restart-and-run" => {
            let cipher = test_cipher();
            let mut worker_state = app.state.as_ref().clone();
            worker_state.async_operation_cipher = Some(Arc::new(cipher.clone()));
            anyhow::ensure!(
                operation_worker::run_once(
                    &journal_ports(&app.pool),
                    &cipher,
                    Arc::new(worker_state)
                )
                .await?,
                "replacement worker found no expired operation to recover"
            );
            Ok(())
        }
        other => anyhow::bail!("unknown queue worker subprocess mode {other}"),
    };
    app.shutdown().await?;
    result
}

#[tokio::test]
async fn timed_out_owner_mutation_is_accepted_then_completed_by_the_worker() -> anyhow::Result<()> {
    let app =
        support::start_gateway_app_with_profile_delay(false, Duration::from_millis(1_000), 2_000)
            .await?;
    let result = async {
        let cipher = test_cipher();
        let mut worker_state = app.state.as_ref().clone();
        worker_state.async_operation_cipher = Some(Arc::new(cipher.clone()));
        worker_state.journal_client_timeout = Duration::from_millis(250);
        let worker_state = Arc::new(worker_state);
        let router = http::server::with_transport_layers(
            http::router(worker_state.clone()),
            &app.config,
            TrustedProxies::default(),
        );
        let actor = "11111111-1111-1111-1111-111111111111";
        let token = support::bearer_token(actor, &["ROLE_USER"]);
        let idempotency_key = format!("async-queue-acceptance-{}", Uuid::new_v4());
        let incoming_traceparent =
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let body = r#"{"username":"queued-user","nickname":"Queued User","avatarUrl":"/avatars/user.png","bio":"Accepted durably"}"#;
        let path = format!("/api/profile/{actor}");
        let make_request = || {
            Request::builder()
                .method("PUT")
                .uri(&path)
                .header("authorization", format!("Bearer {token}"))
                .header("idempotency-key", &idempotency_key)
                .header("content-type", "application/json")
                .header("origin", "http://localhost:3000")
                .header("x-mcr-intent", "browser")
                .header("traceparent", incoming_traceparent)
                .header("tracestate", "vendor=value")
                .body(Body::from(body))
                .expect("request")
        };

        let accepted = router.clone().oneshot(make_request()).await?;
        assert_eq!(accepted.status(), StatusCode::ACCEPTED);
        assert_eq!(accepted.headers()["cache-control"], "no-store");
        assert_eq!(accepted.headers()["retry-after"], "2");
        let operation_id = accepted.headers()["x-operation-id"].to_str()?.to_owned();
        assert_eq!(
            accepted.headers()["location"],
            format!("/api/operations/{operation_id}")
        );
        let response_body = to_bytes(accepted.into_body(), 16 * 1024).await?;
        let accepted_json: serde_json::Value = serde_json::from_slice(&response_body)?;
        assert_eq!(accepted_json["status"], "ACCEPTED");
        assert_eq!(accepted_json["operationId"], operation_id);

        let duplicate = router.clone().oneshot(make_request()).await?;
        assert_eq!(duplicate.status(), StatusCode::ACCEPTED);
        assert_eq!(duplicate.headers()["x-operation-id"], operation_id);
        assert_eq!(duplicate.headers()["idempotency-replayed"], "true");
        let queued_state: (String, String) = sqlx::query_as(
            "select state, outcome from gateway_request_journal where id = $1",
        )
        .bind(Uuid::parse_str(&operation_id)?)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(queued_state, ("QUEUED".into(), "PENDING".into()));
        let queued_contexts = app.profile_trace_contexts();
        assert_eq!(queued_contexts.len(), 1);
        let request_context = queued_contexts[0]
            .clone()
            .ok_or_else(|| anyhow::anyhow!("detached request lost its trace context"))?;
        assert_eq!(request_context.trace_id(), "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(request_context.parent_span_id(), Some("00f067aa0ba902b7"));
        assert_eq!(request_context.tracestate(), Some("vendor=value"));
        let origin_traceparent: String = sqlx::query_scalar(
            "select origin_traceparent from gateway_async_operation_queue where operation_id = $1",
        )
        .bind(Uuid::parse_str(&operation_id)?)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(origin_traceparent, request_context.traceparent());

        let claimed = operation_worker::run_once(&journal_ports(&app.pool), &cipher, worker_state).await?;
        anyhow::ensure!(
            claimed,
            "worker found no operation; queue state={}",
            queue_state_snapshot(&app.pool, Uuid::parse_str(&operation_id)?).await?
        );
        let completed: (String, Option<i32>, bool, bool) = sqlx::query_as(
            "select journal.state, journal.response_status, queue.payload_ciphertext is null, queue.origin_traceparent is null from gateway_request_journal journal join gateway_async_operation_queue queue on queue.operation_id = journal.id where journal.id = $1",
        )
        .bind(Uuid::parse_str(&operation_id)?)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(completed, ("COMPLETED".into(), Some(200), true, true));
        let processed_contexts = app.profile_trace_contexts();
        assert_eq!(processed_contexts.len(), 2);
        let worker_context = processed_contexts[1]
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("queue worker lost its trace context"))?;
        assert_eq!(worker_context.trace_id(), request_context.trace_id());
        assert_eq!(worker_context.parent_span_id(), Some(request_context.span_id()));
        assert_ne!(worker_context.span_id(), request_context.span_id());
        assert_eq!(worker_context.tracestate(), None);

        let status = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/operations/{operation_id}"))
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(status.status(), StatusCode::OK);
        assert_eq!(status.headers()["cache-control"], "no-store");
        let status_body = to_bytes(status.into_body(), 16 * 1024).await?;
        let status_json: serde_json::Value = serde_json::from_slice(&status_body)?;
        assert_eq!(status_json["state"], "COMPLETED");
        assert_eq!(status_json["outcome"], "COMMITTED");
        assert_eq!(status_json["responseStatus"], 200);
        assert_eq!(status_json["replayable"], true);
        Ok::<(), anyhow::Error>(())
    }
    .await;
    app.shutdown().await?;
    result
}

#[tokio::test]
async fn timed_out_moderation_report_is_accepted_and_completed_by_the_worker() -> anyhow::Result<()>
{
    let app = support::start_gateway_app(false).await?;
    let result = async {
        let cipher = test_cipher();
        let mut worker_state = app.state.as_ref().clone();
        worker_state.async_operation_cipher = Some(Arc::new(cipher.clone()));
        worker_state.journal_client_timeout = Duration::from_millis(100);
        let worker_state = Arc::new(worker_state);
        let router = http::server::with_transport_layers(
            http::router(worker_state.clone()),
            &app.config,
            TrustedProxies::default(),
        );
        let actor_id = Uuid::new_v4();
        let actor_id_text = actor_id.to_string();
        let target_id = Uuid::new_v4();
        let token = support::bearer_token(&actor_id_text, &["ROLE_USER"]);
        let idempotency_key = format!("moderation-report-queue-{}", Uuid::new_v4());
        let body = serde_json::json!({
            "targetType": "POST",
            "targetId": target_id,
            "category": "SPAM",
            "reason": "queued spam report",
        })
        .to_string();
        let make_request = || {
            Request::builder()
                .method("POST")
                .uri("/api/moderation/reports")
                .header("authorization", format!("Bearer {token}"))
                .header("idempotency-key", &idempotency_key)
                .header("content-type", "application/json")
                .header("origin", "http://localhost:3000")
                .header("x-mcr-intent", "browser")
                .body(Body::from(body.clone()))
                .expect("request")
        };

        let mut report_lock = app.pool.begin().await?;
        sqlx::query("lock table moderation_reports in share mode")
            .execute(&mut *report_lock)
            .await?;

        let accepted = router.clone().oneshot(make_request()).await?;
        assert_eq!(accepted.status(), StatusCode::ACCEPTED);
        assert_eq!(accepted.headers()["cache-control"], "no-store");
        assert_eq!(accepted.headers()["retry-after"], "2");
        let operation_id = accepted.headers()["x-operation-id"].to_str()?.to_owned();
        assert_eq!(
            accepted.headers()["location"],
            format!("/api/operations/{operation_id}")
        );
        let accepted_body = to_bytes(accepted.into_body(), 16 * 1024).await?;
        let accepted_json: serde_json::Value = serde_json::from_slice(&accepted_body)?;
        assert_eq!(accepted_json["status"], "ACCEPTED");
        assert_eq!(accepted_json["operationId"], operation_id);

        let duplicate = router.clone().oneshot(make_request()).await?;
        assert_eq!(duplicate.status(), StatusCode::ACCEPTED);
        assert_eq!(duplicate.headers()["x-operation-id"], operation_id);
        assert_eq!(duplicate.headers()["idempotency-replayed"], "true");
        let queued_state: String = sqlx::query_scalar(
            "select state from gateway_request_journal where id = $1",
        )
        .bind(Uuid::parse_str(&operation_id)?)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(queued_state, "QUEUED");

        report_lock.rollback().await?;
        let claimed =
            operation_worker::run_once(&journal_ports(&app.pool), &cipher, worker_state.clone()).await?;
        anyhow::ensure!(
            claimed,
            "worker found no operation; queue state={}",
            queue_state_snapshot(&app.pool, Uuid::parse_str(&operation_id)?).await?
        );

        let completed: (String, Option<i32>, bool) = sqlx::query_as(
            "select journal.state, journal.response_status, queue.payload_ciphertext is null from gateway_request_journal journal join gateway_async_operation_queue queue on queue.operation_id = journal.id where journal.id = $1",
        )
        .bind(Uuid::parse_str(&operation_id)?)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(completed, ("COMPLETED".into(), Some(201), true));

        let report_count: i64 = sqlx::query_scalar(
            "select count(*) from moderation_reports where reporter_id = $1 and target_id = $2",
        )
            .bind(actor_id)
            .bind(target_id)
            .fetch_one(&app.pool)
            .await?;
        let receipt_count: i64 = sqlx::query_scalar(
            "select count(*) from moderation_report_operation_receipts where operation_id = $1",
        )
        .bind(Uuid::parse_str(&operation_id)?)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(report_count, 1);
        assert_eq!(receipt_count, 1);

        let replayed = worker_state
            .moderation_service
            .create_report_with_mutation(
                actor_id_text.clone(),
                serde_json::from_str::<CreateReportRequest>(&body)?,
                Some(ModerationMutationContextDto {
                    operation_id: Uuid::parse_str(&operation_id)?,
                    actor_id,
                    idempotency_key: Some(idempotency_key.clone()),
                    lookup_only: false,
                }),
            )
            .await?;
        let persisted_report_id: Uuid = sqlx::query_scalar(
            "select id from moderation_reports where reporter_id = $1 and target_id = $2",
        )
        .bind(actor_id)
        .bind(target_id)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(
            serde_json::to_value(replayed)?["id"],
            persisted_report_id.to_string()
        );

        let status = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/operations/{operation_id}"))
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(status.status(), StatusCode::OK);
        let status_body = to_bytes(status.into_body(), 16 * 1024).await?;
        let status_json: serde_json::Value = serde_json::from_slice(&status_body)?;
        assert_eq!(status_json["state"], "COMPLETED");
        assert_eq!(status_json["outcome"], "COMMITTED");
        assert_eq!(status_json["responseStatus"], 201);

        let completed_replay = router.clone().oneshot(make_request()).await?;
        assert_eq!(completed_replay.status(), StatusCode::CREATED);
        assert_eq!(
            completed_replay.headers()["idempotency-replayed"],
            "true"
        );
        assert_eq!(report_count, 1);
        Ok::<(), anyhow::Error>(())
    }
    .await;
    app.shutdown().await?;
    result
}

#[tokio::test]
async fn concurrent_moderation_report_reexecution_resolves_the_committed_receipt(
) -> anyhow::Result<()> {
    let app = support::start_gateway_app(false).await?;
    let operation_id = Uuid::new_v4();
    let trigger_suffix = Uuid::new_v4().simple().to_string();
    let function_name = format!("test_delay_report_receipt_insert_{trigger_suffix}");
    let trigger_name = format!("test_delay_report_receipt_insert_{trigger_suffix}");
    let result = async {
        sqlx::query(&format!(
            r#"
            create function {function_name}() returns trigger
            language plpgsql as $$
            begin
                if new.operation_id = '{operation_id}'::uuid then
                    perform pg_sleep(0.25);
                end if;
                return new;
            end
            $$
            "#,
        ))
        .execute(&app.pool)
        .await?;
        sqlx::query(&format!(
            "create trigger {trigger_name} before insert on moderation_report_operation_receipts for each row execute function {function_name}()"
        ))
        .execute(&app.pool)
        .await?;

        let actor_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111")?;
        let target_id = Uuid::new_v4();
        let key = format!("moderation-report-concurrent-{}", Uuid::new_v4());
        let request = CreateReportRequest {
            target_type: "POST".into(),
            target_id: target_id.to_string(),
            category: "SPAM".into(),
            reason: "concurrent worker retry".into(),
        };
        let context = ModerationMutationContextDto {
            operation_id,
            actor_id,
            idempotency_key: Some(key),
            lookup_only: false,
        };

        let service = app.state.moderation_service.clone();
        let (first, second) = tokio::join!(
            service.create_report_with_mutation(
                actor_id.to_string(),
                request.clone(),
                Some(context.clone()),
            ),
            service.create_report_with_mutation(
                actor_id.to_string(),
                request,
                Some(context),
            ),
        );
        let first = first?;
        let second = second?;
        assert_eq!(first.id, second.id);

        let report_count: i64 = sqlx::query_scalar(
            "select count(*) from moderation_reports where reporter_id = $1 and target_id = $2",
        )
        .bind(actor_id)
        .bind(target_id)
        .fetch_one(&app.pool)
        .await?;
        let receipt_count: i64 = sqlx::query_scalar(
            "select count(*) from moderation_report_operation_receipts where operation_id = $1",
        )
        .bind(operation_id)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(report_count, 1);
        assert_eq!(receipt_count, 1);
        Ok::<(), anyhow::Error>(())
    }
    .await;
    let cleanup = async {
        sqlx::query(&format!(
            "drop trigger if exists {trigger_name} on moderation_report_operation_receipts"
        ))
        .execute(&app.pool)
        .await?;
        sqlx::query(&format!("drop function if exists {function_name}()"))
            .execute(&app.pool)
            .await?;
        Ok::<(), anyhow::Error>(())
    }
    .await;
    app.shutdown().await?;
    result?;
    cleanup
}

#[tokio::test]
async fn typed_queue_ports_preserve_crypto_claim_metrics_and_stale_completion_fences(
) -> anyhow::Result<()> {
    use api_gateway::application::dto::journal_write_dto::JournalOutcomeEvidenceDto;
    use api_gateway::application::dto::operation_view_dto::OperationOutcomeDto;
    use api_gateway::application::dto::queue_repository_dto::*;
    use api_gateway::application::ports::QueuePayloadPort;
    use api_gateway::application::queue_operation_error::QueueOperationError;
    let app = support::start_gateway_app(false).await?;
    let cipher = test_cipher();
    let payload_port: &dyn QueuePayloadPort = &cipher;
    let id = Uuid::new_v4();
    let actor = Uuid::new_v4().to_string();
    let body = b"private queue command";
    let fingerprint = "typed-queue-private-fingerprint";
    insert_journal(
        &app.pool,
        id,
        &actor,
        "typed-queue-key",
        fingerprint,
        "POST /api/posts",
        "/api/posts",
    )
    .await?;
    let payload = payload_port.encrypt(QueuePayloadEncryptDto {
        operation_id: id,
        actor_id: &actor,
        request_fingerprint: fingerprint,
        plaintext: body,
    })?;
    assert!(matches!(
        payload_port.decrypt(QueuePayloadDecryptDto {
            operation_id: id,
            actor_id: "wrong-actor",
            request_fingerprint: fingerprint,
            payload: &payload
        }),
        Err(QueueOperationError::PayloadUnavailable)
    ));
    let mut tampered = payload.clone();
    tampered.ciphertext[0] ^= 1;
    assert!(matches!(
        payload_port.decrypt(QueuePayloadDecryptDto {
            operation_id: id,
            actor_id: &actor,
            request_fingerprint: fingerprint,
            payload: &tampered
        }),
        Err(QueueOperationError::PayloadUnavailable)
    ));
    let metrics = api_gateway::config::GatewayMetrics::default();
    let repository =
        api_gateway::config::async_operation_queue_repository(&app.pool, Some(metrics.clone()));
    assert!(
        repository
            .enqueue(QueueEnqueueCommandDto {
                operation_id: id,
                actor_id: &actor,
                request_fingerprint: fingerprint,
                request_uri: "/api/posts",
                content_type: None,
                payload,
                origin_traceparent: Some(
                    "00-11111111111111111111111111111111-2222222222222222-01".into()
                )
            })
            .await?
    );
    let mut held = repository.acquire_claim_connection().await?;
    assert!(metrics.render_prometheus().contains(
        "gateway_db_pool_observed_holders{pool=\"background\",caller=\"worker_claim\"} 1"
    ));
    assert!(matches!(
        held.claim_next(QueueClaimQueryDto {
            worker_id: Uuid::new_v4(),
            lease_duration: Duration::ZERO
        })
        .await,
        Err(QueueOperationError::Rejected)
    ));
    let claim = held
        .claim_next(QueueClaimQueryDto {
            worker_id: Uuid::new_v4(),
            lease_duration: Duration::from_secs(60),
        })
        .await?
        .expect("claim");
    drop(held);
    assert!(metrics.render_prometheus().contains(
        "gateway_db_pool_observed_holders{pool=\"background\",caller=\"worker_claim\"} 0"
    ));
    assert_eq!(claim.decrypt_body(payload_port)?, body);
    assert!(!format!("{claim:?}").contains(&actor));
    assert!(!format!("{claim:?}").contains(fingerprint));
    let complete = |lease, response_status| QueueCompleteCommandDto {
        lease,
        response_status,
        outcome: OperationOutcomeDto::Committed,
        evidence: JournalOutcomeEvidenceDto::SuccessResponse,
        content_type: None,
        response_body: Some(b"created"),
    };
    assert_eq!(
        repository
            .complete_claim(complete(claim.lease_identity(), 202))
            .await,
        Err(QueueOperationError::Rejected)
    );
    sqlx::query("update gateway_async_operation_queue set lease_expires_at=now()-interval '1 second' where operation_id=$1").bind(id).execute(&app.pool).await?;
    let fresh = repository
        .claim_next(QueueClaimQueryDto {
            worker_id: Uuid::new_v4(),
            lease_duration: Duration::from_secs(60),
        })
        .await?
        .expect("reclaimed");
    assert_eq!(fresh.attempt_count, 2);
    assert_ne!(claim.lease_owner, fresh.lease_owner);
    assert!(
        !repository
            .complete_claim(complete(claim.lease_identity(), 201))
            .await?
    );
    let state: String = sqlx::query_scalar("select state from gateway_request_journal where id=$1")
        .bind(id)
        .fetch_one(&app.pool)
        .await?;
    assert_eq!(
        state, "QUEUED",
        "stale completion must roll back its journal projection"
    );
    assert!(
        repository
            .complete_claim(complete(fresh.lease_identity(), 201))
            .await?
    );
    assert!(
        !repository
            .complete_claim(complete(fresh.lease_identity(), 201))
            .await?
    );
    let stored = sqlx::query("select payload_nonce is null and payload_ciphertext is null and origin_traceparent is null as erased from gateway_async_operation_queue where operation_id=$1").bind(id).fetch_one(&app.pool).await?;
    assert!(stored.get::<bool, _>("erased"));
    let audits: i64 = sqlx::query_scalar("select count(*) from admin_audit_events where resource_id=$1 and action='GATEWAY_ASYNC_OPERATION_COMPLETED'").bind(id.to_string()).fetch_one(&app.pool).await?;
    assert_eq!(audits, 1);
    app.shutdown().await?;
    assert!(matches!(
        repository.acquire_claim_connection().await,
        Err(QueueOperationError::Storage)
    ));
    Ok(())
}
