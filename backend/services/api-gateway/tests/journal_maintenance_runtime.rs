//! Real-PostgreSQL maintenance acceptance with restrictive journal children.
#[path = "support/mod.rs"]
mod support;

use api_gateway::adapters::r#in::http::request_journal;
use sqlx::PgPool;
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

async fn journal_row(pool: &PgPool, state: &str, expired: bool) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    let (outcome, evidence) = match state {
        "COMPLETED" => ("COMMITTED", "SUCCESS_RESPONSE"),
        "IN_PROGRESS" => ("PENDING", "INTENT_RECORDED"),
        _ => ("UNKNOWN", "STALE_OPERATION"),
    };
    sqlx::query(
        r#"
        insert into gateway_request_journal (
            id, actor_id, action, resource_path, state, response_status,
            response_body, created_at, expires_at, outcome, outcome_evidence
        ) values ($1, $2, 'POST /api/test-write', '/api/test-write', $3, 200,
            $4, now() - interval '25 hours',
            case when $5 then now() - interval '1 hour' else now() + interval '1 hour' end,
            $6, $7)
        "#,
    )
    .bind(id)
    .bind(USER)
    .bind(state)
    .bind(b"sensitive-replay-body".to_vec())
    .bind(expired)
    .bind(outcome)
    .bind(evidence)
    .execute(pool)
    .await?;
    Ok(id)
}

async fn queue_row(pool: &PgPool, id: Uuid, state: &str) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        insert into gateway_async_operation_queue (
            operation_id, request_uri, encryption_key_id, state, available_at,
            payload_nonce, payload_ciphertext, lease_owner, lease_expires_at
        ) values ($1, '/api/test-write', 1, $2, now(),
            case when $2 in ('READY', 'LEASED') then $3 else null end,
            case when $2 in ('READY', 'LEASED') then $4 else null end,
            case when $2 = 'LEASED' then $5 else null end,
            case when $2 = 'LEASED' then now() + interval '1 hour' else null end)
        "#,
    )
    .bind(id)
    .bind(state)
    .bind(vec![0_u8; 12])
    .bind(vec![0_u8; 16])
    .bind(Uuid::new_v4())
    .execute(pool)
    .await?;
    Ok(())
}

async fn exists(pool: &PgPool, id: Uuid) -> anyhow::Result<bool> {
    Ok(
        sqlx::query_scalar("select exists(select 1 from gateway_request_journal where id = $1)")
            .bind(id)
            .fetch_one(pool)
            .await?,
    )
}

#[tokio::test]
async fn journal_maintenance_retires_done_work_and_preserves_restricted_evidence(
) -> anyhow::Result<()> {
    let app = support::start_gateway_app(true).await?;
    let result = async {
        let plain = journal_row(&app.pool, "COMPLETED", true).await?;
        let done = journal_row(&app.pool, "COMPLETED", true).await?;
        queue_row(&app.pool, done, "DONE").await?;
        let manual = journal_row(&app.pool, "COMPLETED", true).await?;
        queue_row(&app.pool, manual, "DONE").await?;
        let resolution_id = Uuid::new_v4();
        sqlx::query(
            r#"
            insert into gateway_operation_manual_reconciliations (
                resolution_id, operation_id, revision, disposition, evidence_kind,
                evidence_reference, reason, reviewer_id
            ) values ($1, $2, 1, 'COMMITTED', 'OWNER_CONFIRMATION',
                'incident:maintenance-regression', 'Owner receipt proves the completed operation.', 'operator')
            "#,
        )
        .bind(resolution_id)
        .bind(manual)
        .execute(&app.pool)
        .await?;
        let mut retained = vec![manual];
        for state in ["READY", "LEASED", "DEAD"] {
            let id = journal_row(&app.pool, "COMPLETED", true).await?;
            queue_row(&app.pool, id, state).await?;
            retained.push(id);
        }
        let unknown = journal_row(&app.pool, "UNKNOWN", true).await?;
        queue_row(&app.pool, unknown, "DONE").await?;
        retained.push(unknown);
        let proven_unknown = journal_row(&app.pool, "UNKNOWN", true).await?;
        queue_row(&app.pool, proven_unknown, "DONE").await?;
        sqlx::query("update gateway_request_journal set outcome = 'COMMITTED', outcome_evidence = 'OWNER_RECEIPT' where id = $1")
            .bind(proven_unknown)
            .execute(&app.pool)
            .await?;
        retained.push(proven_unknown);
        let live = journal_row(&app.pool, "COMPLETED", false).await?;
        queue_row(&app.pool, live, "DONE").await?;
        let stale = journal_row(&app.pool, "IN_PROGRESS", true).await?;
        retained.push(stale);
        let receipt = Uuid::new_v4();
        sqlx::query(
            r#"
            insert into moderation_report_operation_receipts (
                operation_id, actor_id, scope, outcome_json, committed_at, replay_until
            ) values ($1, $2, 'gateway.v1.CreateModerationReport', '{}',
                now() - interval '2 days', now() - interval '1 day')
            "#,
        )
        .bind(receipt)
        .bind(Uuid::parse_str(USER)?)
        .execute(&app.pool)
        .await?;

        let outcome = request_journal::reconcile_stale(&journal_ports(&app.pool)).await?;
        assert_eq!(outcome.abandoned_operations, 1);
        assert_eq!(outcome.journal_bodies_pruned, 9);
        assert_eq!(outcome.report_receipt_bodies_pruned, 1);
        assert_eq!(outcome.journal_rows_retired, 2);
        assert_eq!(outcome.completed_queue_rows_retired, 1);
        assert!(!outcome.retirement_deferred);
        assert!(!exists(&app.pool, plain).await?);
        assert!(!exists(&app.pool, done).await?);
        for id in &retained {
            let body: Option<Vec<u8>> = sqlx::query_scalar(
                "select response_body from gateway_request_journal where id = $1",
            )
            .bind(id)
            .fetch_one(&app.pool)
            .await?;
            assert!(body.is_none(), "retained identities must still lose expired bodies");
        }
        let preserved_queue: i64 = sqlx::query_scalar(
            "select count(*) from gateway_async_operation_queue where operation_id = any($1)",
        )
        .bind(&retained)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(preserved_queue, 6);
        let history: i64 = sqlx::query_scalar(
            "select count(*) from gateway_operation_manual_reconciliations where resolution_id = $1",
        )
        .bind(resolution_id)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(history, 1);
        let fresh_body: Option<Vec<u8>> = sqlx::query_scalar(
            "select response_body from gateway_request_journal where id = $1",
        )
        .bind(live)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(fresh_body, Some(b"sensitive-replay-body".to_vec()));
        let state: (String, String, String) = sqlx::query_as(
            "select state, outcome, outcome_evidence from gateway_request_journal where id = $1",
        )
        .bind(stale)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(state, ("UNKNOWN".into(), "UNKNOWN".into(), "STALE_OPERATION".into()));
        let audits: i64 = sqlx::query_scalar(
            "select count(*) from admin_audit_events where id = $1 and outcome = 'UNKNOWN'",
        )
        .bind(stale)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(audits, 1);
        let receipt_state: (String, bool) = sqlx::query_as(
            "select state, outcome_json is null from moderation_report_operation_receipts where operation_id = $1",
        )
        .bind(receipt)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(receipt_state, ("EXPIRED".into(), true));
        let next = request_journal::reconcile_stale(&journal_ports(&app.pool)).await?;
        assert_eq!(next.abandoned_operations, 0);
        assert_eq!(next.journal_bodies_pruned, 0);
        assert_eq!(next.report_receipt_bodies_pruned, 0);
        Ok(())
    }
    .await;
    app.shutdown().await?;
    result
}

#[tokio::test]
async fn journal_retirement_failure_cannot_roll_back_committed_recovery() -> anyhow::Result<()> {
    let app = support::start_gateway_app(true).await?;
    let result = async {
        let done = journal_row(&app.pool, "COMPLETED", true).await?;
        queue_row(&app.pool, done, "DONE").await?;
        let stale = journal_row(&app.pool, "IN_PROGRESS", true).await?;
        // A database-side fault after the DONE child delete proves that both
        // retirement deletes roll back while the earlier recovery stays durable.
        sqlx::raw_sql(
            r#"
            create function test_reject_journal_retirement() returns trigger language plpgsql as $$
            begin raise exception 'injected retirement foreign-key failure' using errcode = '23503'; end;
            $$;
            create trigger test_reject_journal_retirement before delete on gateway_request_journal
            for each row execute function test_reject_journal_retirement();
            "#,
        )
        .execute(&app.pool)
        .await?;
        let outcome = request_journal::reconcile_stale(&journal_ports(&app.pool)).await?;
        assert_eq!(outcome.abandoned_operations, 1);
        assert_eq!(outcome.journal_bodies_pruned, 2);
        assert!(outcome.retirement_deferred);
        assert_eq!(outcome.journal_rows_retired, 0);
        assert_eq!(outcome.completed_queue_rows_retired, 0);
        let state: (String, bool) = sqlx::query_as(
            "select state, response_body is null from gateway_request_journal where id = $1",
        )
        .bind(stale)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(state, ("UNKNOWN".into(), true));
        assert!(exists(&app.pool, done).await?);
        let queue: String = sqlx::query_scalar(
            "select state from gateway_async_operation_queue where operation_id = $1",
        )
        .bind(done)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(queue, "DONE", "the child delete must roll back with its parent delete");
        let audit_count: i64 = sqlx::query_scalar(
            "select count(*) from admin_audit_events where id = $1 and outcome = 'UNKNOWN'",
        )
        .bind(stale)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(audit_count, 1);
        sqlx::raw_sql(
            "drop trigger test_reject_journal_retirement on gateway_request_journal; drop function test_reject_journal_retirement();",
        )
        .execute(&app.pool)
        .await?;
        let retry = request_journal::reconcile_stale(&journal_ports(&app.pool)).await?;
        assert!(!retry.retirement_deferred);
        assert_eq!(retry.journal_rows_retired, 1);
        assert_eq!(retry.completed_queue_rows_retired, 1);
        assert_eq!(retry.abandoned_operations, 0);
        assert!(!exists(&app.pool, done).await?);
        assert!(exists(&app.pool, stale).await?);
        Ok(())
    }
    .await;
    app.shutdown().await?;
    result
}

#[tokio::test]
async fn journal_retirement_skips_locked_parents_and_bounds_each_batch() -> anyhow::Result<()> {
    let app = support::start_gateway_app(true).await?;
    let result = async {
        let held = journal_row(&app.pool, "COMPLETED", true).await?;
        queue_row(&app.pool, held, "DONE").await?;
        let mut lock = app.pool.begin().await?;
        sqlx::query("select id from gateway_request_journal where id = $1 for update")
            .bind(held)
            .execute(&mut *lock)
            .await?;
        // A full older protected batch must not monopolize the retirement
        // limit. DEAD children require operator attention, even on a COMPLETED
        // parent; they are not inferred safe to discard.
        sqlx::query(
            r#"
            with inserted as (
                insert into gateway_request_journal (
                    id, actor_id, action, resource_path, state, response_status,
                    created_at, expires_at, outcome, outcome_evidence
                ) select gen_random_uuid(), $1, 'POST /api/test-write', '/api/test-write',
                    'COMPLETED', 200, now() - interval '25 hours', now() - interval '2 hours',
                    'COMMITTED', 'SUCCESS_RESPONSE' from generate_series(1, 501)
                returning id
            )
            insert into gateway_async_operation_queue (
                operation_id, request_uri, encryption_key_id, state, available_at
            ) select id, '/api/test-write', 1, 'DEAD', now() from inserted
            "#,
        )
        .bind(USER)
        .execute(&app.pool)
        .await?;
        // All generated rows are eligible and get a DONE child. The locked row
        // must survive, and an unlocked pass can retire at most 500 parents.
        sqlx::query(
            r#"
            with inserted as (
                insert into gateway_request_journal (
                    id, actor_id, action, resource_path, state, response_status,
                    created_at, expires_at, outcome, outcome_evidence
                ) select gen_random_uuid(), $1, 'POST /api/test-write', '/api/test-write',
                    'COMPLETED', 200, now() - interval '25 hours', now() - interval '1 hour',
                    'COMMITTED', 'SUCCESS_RESPONSE' from generate_series(1, 501)
                returning id
            )
            insert into gateway_async_operation_queue (
                operation_id, request_uri, encryption_key_id, state, available_at
            ) select id, '/api/test-write', 1, 'DONE', now() from inserted
            "#,
        )
        .bind(USER)
        .execute(&app.pool)
        .await?;
        let first = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            request_journal::reconcile_stale(&journal_ports(&app.pool)),
        )
        .await??;
        assert_eq!(
            first.journal_bodies_pruned, 0,
            "the sole body holder is locked"
        );
        assert!(!first.retirement_deferred);
        assert_eq!(first.journal_rows_retired, 500);
        assert_eq!(first.completed_queue_rows_retired, 500);
        let remaining: i64 = sqlx::query_scalar("select count(*) from gateway_request_journal")
            .fetch_one(&app.pool)
            .await?;
        assert_eq!(remaining, 503);
        assert!(exists(&app.pool, held).await?);
        let queue_count: i64 =
            sqlx::query_scalar("select count(*) from gateway_async_operation_queue")
                .fetch_one(&app.pool)
                .await?;
        assert_eq!(queue_count, 503);
        lock.rollback().await?;
        request_journal::reconcile_stale(&journal_ports(&app.pool)).await?;
        let remaining: i64 = sqlx::query_scalar("select count(*) from gateway_request_journal")
            .fetch_one(&app.pool)
            .await?;
        let queue_count: i64 =
            sqlx::query_scalar("select count(*) from gateway_async_operation_queue")
                .fetch_one(&app.pool)
                .await?;
        assert_eq!((remaining, queue_count), (501, 501));
        Ok(())
    }
    .await;
    app.shutdown().await?;
    result
}

#[tokio::test]
async fn journal_retirement_lock_timeout_returns_committed_counts() -> anyhow::Result<()> {
    let app = support::start_gateway_app(true).await?;
    let result = async {
        let done = journal_row(&app.pool, "COMPLETED", true).await?;
        queue_row(&app.pool, done, "DONE").await?;
        let stale = journal_row(&app.pool, "IN_PROGRESS", true).await?;
        let mut holder = app.pool.begin().await?;
        // Phase one does not touch this table. Retirement needs its metadata
        // under a lock, so this creates deterministic contention only in phase two.
        sqlx::query("lock table gateway_operation_manual_reconciliations in access exclusive mode")
            .execute(&mut *holder)
            .await?;
        let outcome = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            request_journal::reconcile_stale(&journal_ports(&app.pool)),
        )
        .await??;
        assert!(outcome.retirement_deferred);
        assert_eq!(outcome.abandoned_operations, 1);
        assert_eq!(outcome.journal_bodies_pruned, 2);
        assert_eq!(outcome.journal_rows_retired, 0);
        assert_eq!(outcome.completed_queue_rows_retired, 0);
        let state: (String, bool) = sqlx::query_as(
            "select state, response_body is null from gateway_request_journal where id = $1",
        )
        .bind(stale)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(state, ("UNKNOWN".into(), true));
        assert!(exists(&app.pool, done).await?);
        holder.rollback().await?;
        let next = request_journal::reconcile_stale(&journal_ports(&app.pool)).await?;
        assert!(!next.retirement_deferred);
        assert_eq!(next.abandoned_operations, 0);
        assert_eq!(next.journal_rows_retired, 1);
        assert_eq!(next.completed_queue_rows_retired, 1);
        let audits: i64 = sqlx::query_scalar(
            "select count(*) from admin_audit_events where id = $1 and outcome = 'UNKNOWN'",
        )
        .bind(stale)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(audits, 1);
        Ok(())
    }
    .await;
    app.shutdown().await?;
    result
}

#[tokio::test]
async fn journal_retirement_acquisition_timeout_preserves_recovery_result() -> anyhow::Result<()> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    let app = support::start_gateway_app(true).await?;
    let result = async {
        let done = journal_row(&app.pool, "COMPLETED", true).await?;
        queue_row(&app.pool, done, "DONE").await?;
        let stale = journal_row(&app.pool, "IN_PROGRESS", true).await?;
        let acquisitions = Arc::new(AtomicUsize::new(0));
        let counted = acquisitions.clone();
        // A real PostgreSQL pool with a deterministic acquisition hook. Phase
        // one acquires normally. The second acquisition stalls before SQL, so
        // only the complete retirement-future timeout can release this pass.
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .min_connections(1)
            .before_acquire(move |_, _| {
                let ordinal = counted.fetch_add(1, Ordering::SeqCst);
                Box::pin(async move {
                    if ordinal == 1 {
                        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                    }
                    Ok(true)
                })
            })
            .connect(&app.config.database_url)
            .await?;
        acquisitions.store(0, Ordering::SeqCst);
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            request_journal::reconcile_stale(&journal_ports(&pool)),
        )
        .await;
        pool.close().await;
        let outcome = result??;
        assert_eq!(acquisitions.load(Ordering::SeqCst), 2);
        assert!(outcome.retirement_deferred);
        assert_eq!(outcome.abandoned_operations, 1);
        assert_eq!(outcome.journal_bodies_pruned, 2);
        assert_eq!(outcome.journal_rows_retired, 0);
        assert_eq!(outcome.completed_queue_rows_retired, 0);
        let state: (String, bool) = sqlx::query_as(
            "select state, response_body is null from gateway_request_journal where id = $1",
        )
        .bind(stale)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(state, ("UNKNOWN".into(), true));
        assert!(exists(&app.pool, done).await?);
        let next = request_journal::reconcile_stale(&journal_ports(&app.pool)).await?;
        assert!(!next.retirement_deferred);
        assert_eq!(next.abandoned_operations, 0);
        assert_eq!(next.journal_rows_retired, 1);
        assert_eq!(next.completed_queue_rows_retired, 1);
        Ok(())
    }
    .await;
    app.shutdown().await?;
    result
}

#[tokio::test]
async fn typed_maintenance_port_preserves_recovery_counts_and_retained_queue_identity(
) -> anyhow::Result<()> {
    use api_gateway::application::dto::queue_repository_dto::JournalMaintenanceCommandDto;
    use api_gateway::application::journal_write_error::JournalWriteError;
    let app = support::start_gateway_app(false).await?;
    let stale = journal_row(&app.pool, "IN_PROGRESS", true).await?;
    let retained = journal_row(&app.pool, "COMPLETED", true).await?;
    queue_row(&app.pool, retained, "DEAD").await?;
    let retired = journal_row(&app.pool, "COMPLETED", true).await?;
    let repository = api_gateway::config::journal_maintenance_repository(&app.pool);
    let outcome = repository
        .reconcile_stale(JournalMaintenanceCommandDto {
            stale_operation_age_seconds: 300,
        })
        .await?;
    assert_eq!(outcome.abandoned_operations, 1);
    assert_eq!(outcome.journal_bodies_pruned, 3);
    assert_eq!(outcome.journal_rows_retired, 1);
    assert!(!outcome.retirement_deferred);
    assert!(exists(&app.pool, stale).await?);
    assert!(exists(&app.pool, retained).await?);
    assert!(!exists(&app.pool, retired).await?);
    let state: String = sqlx::query_scalar("select state from gateway_request_journal where id=$1")
        .bind(stale)
        .fetch_one(&app.pool)
        .await?;
    assert_eq!(state, "UNKNOWN");
    app.shutdown().await?;
    assert_eq!(
        repository
            .reconcile_stale(JournalMaintenanceCommandDto {
                stale_operation_age_seconds: 300
            })
            .await,
        Err(JournalWriteError::Permanent)
    );
    Ok(())
}
