//! Bounded journal recovery/body expiry, separated from parent retirement.
//!
//! Immutable decisions and non-DONE queue work retain their parent identity.
//! Retirement is best effort after recovery commits, so a restrictive foreign
//! key or a concurrent lock cannot undo recovery and sensitive-body expiry.

use crate::application::dto::operation_dto::ReconciliationOutcome;
use sqlx::PgPool;
use uuid::Uuid;

const MAINTENANCE_BATCH_SIZE: i64 = 500;
// The scheduler has a five-second envelope. Preserve time to return committed
// phase-one counts even if retirement cannot acquire a connection or finish.
const RECOVERY_BUDGET: std::time::Duration = std::time::Duration::from_secs(3);
const RETIREMENT_BUDGET: std::time::Duration = std::time::Duration::from_millis(1500);

pub async fn reconcile_stale(
    pool: &PgPool,
    stale_operation_age_seconds: u64,
) -> anyhow::Result<ReconciliationOutcome> {
    let mut outcome = tokio::time::timeout(
        RECOVERY_BUDGET,
        recover_and_prune(pool, stale_operation_age_seconds),
    )
    .await
    .map_err(|_| anyhow::anyhow!("journal recovery exceeded its time budget"))??;
    match tokio::time::timeout(RETIREMENT_BUDGET, retire_completed(pool)).await {
        Ok(Ok((journal_rows, queue_rows))) => {
            outcome.journal_rows_retired = journal_rows;
            outcome.completed_queue_rows_retired = queue_rows;
        }
        other => {
            outcome.retirement_deferred = true;
            let error = match other {
                Ok(Err(error)) => error,
                Err(_) => anyhow::anyhow!("journal retirement exceeded its time budget"),
                Ok(Ok(_)) => unreachable!(),
            };
            tracing::warn!(
                error = ?error,
                code = "JOURNAL_RETIREMENT_DEFERRED",
                "journal recovery and body expiry committed; parent retirement deferred"
            );
        }
    }
    Ok(outcome)
}

async fn recover_and_prune(
    pool: &PgPool,
    stale_operation_age_seconds: u64,
) -> anyhow::Result<ReconciliationOutcome> {
    let mut tx = pool.begin().await?;
    sqlx::query("select set_config('lock_timeout', '500ms', true), set_config('statement_timeout', '1s', true)")
        .execute(&mut *tx)
        .await?;
    let abandoned_operations = sqlx::query(
        r#"
        with abandoned as (
            update gateway_request_journal set state = 'UNKNOWN', outcome = 'UNKNOWN',
                outcome_evidence = 'STALE_OPERATION', finished_at = now()
            where id in (select id from gateway_request_journal
                where state = 'IN_PROGRESS' and created_at < now() - make_interval(secs => $1)
                order by created_at limit 500 for update skip locked)
            returning *
        )
        insert into admin_audit_events (id, actor_id, action, resource_type, outcome,
            request_id, ip_address, user_agent, metadata_json, occurred_at)
          select id, actor_id, action, resource_path, 'UNKNOWN', request_id,
              ip_address, user_agent, '{"reconciliation":"required"}', now() from abandoned
          on conflict (id) do nothing
          "#,
    )
    .bind(stale_operation_age_seconds as f64)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    // Unresolved intents and protected identities survive body expiry. Replay
    // bodies use the existing explicit 24-hour retention window.
    let journal_bodies_pruned = sqlx::query("update gateway_request_journal set response_body = null where id in (select id from gateway_request_journal where expires_at < now() and response_body is not null order by expires_at limit 500 for update skip locked)")
        .execute(&mut *tx).await?.rows_affected();
    let report_receipt_bodies_pruned = sqlx::query(
        r#"
        update moderation_report_operation_receipts
        set state = 'EXPIRED', outcome_json = null
        where operation_id in (
            select operation_id
            from moderation_report_operation_receipts
            where state = 'REPLAYABLE' and replay_until <= now()
            order by replay_until, operation_id
            limit 500
            for update skip locked
        )
        "#,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    tx.commit().await?;
    Ok(ReconciliationOutcome {
        abandoned_operations,
        journal_bodies_pruned,
        report_receipt_bodies_pruned,
        ..Default::default()
    })
}

async fn retire_completed(pool: &PgPool) -> anyhow::Result<(u64, u64)> {
    let mut tx = pool.begin().await?;
    sqlx::query("set transaction isolation level read committed")
        .execute(&mut *tx)
        .await?;
    sqlx::query("select set_config('lock_timeout', '500ms', true), set_config('statement_timeout', '1s', true)")
        .execute(&mut *tx)
        .await?;
    // Exclude retained work before the limit, so it cannot starve later eligible
    // rows. Parent locks also serialize application manual decisions/enqueue and
    // conflict with foreign-key checks on new child insertions.
    let candidates: Vec<Uuid> = sqlx::query_scalar(
        r#"
        select journal.id from gateway_request_journal journal
        where journal.state = 'COMPLETED' and journal.expires_at < now()
          and not exists (
              select 1 from gateway_operation_manual_reconciliations decision
              where decision.operation_id = journal.id
          )
          and not exists (
              select 1 from gateway_async_operation_queue queue
              where queue.operation_id = journal.id and queue.state <> 'DONE'
          )
        order by journal.expires_at, journal.id
        limit $1 for update of journal skip locked
        "#,
    )
    .bind(MAINTENANCE_BATCH_SIZE)
    .fetch_all(&mut *tx)
    .await?;
    // A child may have committed after the selection statement's snapshot but
    // before it acquired the parent lock. Recheck in a fresh statement snapshot
    // while retaining all parent locks; no new FK child can then appear.
    let eligible: Vec<Uuid> = sqlx::query_scalar(
        r#"
        select journal.id from gateway_request_journal journal
        where journal.id = any($1)
          and journal.state = 'COMPLETED' and journal.expires_at < now()
          and not exists (
              select 1 from gateway_operation_manual_reconciliations decision
              where decision.operation_id = journal.id
          )
          and not exists (
              select 1 from gateway_async_operation_queue queue
              where queue.operation_id = journal.id and queue.state <> 'DONE'
          )
        "#,
    )
    .bind(&candidates)
    .fetch_all(&mut *tx)
    .await?;
    let queue_rows = sqlx::query(
        "delete from gateway_async_operation_queue where operation_id = any($1) and state = 'DONE'",
    )
    .bind(&eligible)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let journal_rows = sqlx::query("delete from gateway_request_journal where id = any($1)")
        .bind(&eligible)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    tx.commit().await?;
    Ok((journal_rows, queue_rows))
}

#[derive(Clone)]
pub struct PgJournalMaintenanceRepository {
    pool: PgPool,
}
impl PgJournalMaintenanceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
#[async_trait::async_trait]
impl crate::application::ports::JournalMaintenanceRepositoryPort
    for PgJournalMaintenanceRepository
{
    async fn reconcile_stale(
        &self,
        command: crate::application::dto::queue_repository_dto::JournalMaintenanceCommandDto,
    ) -> crate::application::journal_write_error::JournalWriteResult<ReconciliationOutcome> {
        reconcile_stale(&self.pool, command.stale_operation_age_seconds)
            .await
            .map_err(super::journal_write_repository::classify_write_error)
    }
}
