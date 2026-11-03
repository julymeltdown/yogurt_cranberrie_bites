//! PostgreSQL owner receipt claims, lease fences and atomic commit evidence.
use super::journal_write_repository::classify_write_error;
use crate::application::dto::journal_repository_dto::OperationJournalLookupDto;
use crate::application::dto::journal_write_dto::*;
use crate::application::journal_write_error::JournalWriteResult;
use crate::application::ports::OwnerReceiptJournalRepositoryPort;
use async_trait::async_trait;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;
#[derive(Clone)]
pub struct PgOwnerReceiptJournalRepository {
    pool: PgPool,
}
impl PgOwnerReceiptJournalRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
#[async_trait]
impl OwnerReceiptJournalRepositoryPort for PgOwnerReceiptJournalRepository {
    async fn claim_next(
        &self,
        command: OwnerReceiptClaimRequestDto,
    ) -> JournalWriteResult<Option<OwnerReceiptClaimDto>> {
        claim_next(&self.pool, command)
            .await
            .map_err(classify_write_error)
    }
    async fn finish_lookup(
        &self,
        command: OwnerReceiptFinishLookupDto<'_>,
    ) -> JournalWriteResult<bool> {
        finish_lookup(&self.pool, command)
            .await
            .map_err(classify_write_error)
    }
    async fn record_commit(&self, command: OwnerReceiptCommitDto<'_>) -> JournalWriteResult<bool> {
        record_commit(&self.pool, command)
            .await
            .map_err(classify_write_error)
    }
    async fn local_receipt(
        &self,
        query: OperationJournalLookupDto,
    ) -> JournalWriteResult<Option<LocalOwnerReceiptRecordDto>> {
        local_receipt(&self.pool, query)
            .await
            .map_err(classify_write_error)
    }
}
async fn bounded_transaction(pool: &PgPool) -> anyhow::Result<Transaction<'_, Postgres>> {
    let mut tx = pool.begin().await?;
    sqlx::query("select set_config('lock_timeout', '200ms', true), set_config('statement_timeout', '1s', true)")
        .execute(&mut *tx).await?;
    Ok(tx)
}
async fn claim_next(
    pool: &PgPool,
    command: OwnerReceiptClaimRequestDto,
) -> anyhow::Result<Option<OwnerReceiptClaimDto>> {
    let mut tx = bounded_transaction(pool).await?;
    let token = Uuid::new_v4();
    let candidate = sqlx::query_as::<_, (Uuid, i32)>(
        r#"
        with candidate as (
            select id from gateway_request_journal
            where state = 'UNKNOWN' and outcome = 'UNKNOWN'
              and outcome_evidence <> 'MANUAL_RESOLUTION'
              and owner_receipt_suspended_reason is null
              and coalesce(owner_receipt_next_check_at, created_at) <= now()
            order by coalesce(owner_receipt_next_check_at, created_at), id
            limit 1 for update skip locked
        )
        update gateway_request_journal journal
        set owner_receipt_checked_at = now(), owner_receipt_claim_id = $2,
            owner_receipt_next_check_at = now() + make_interval(secs => $1),
            owner_receipt_attempt_count = least(owner_receipt_attempt_count::bigint + 1, 2147483647)::integer
        from candidate where journal.id = candidate.id
        returning journal.id, journal.owner_receipt_attempt_count
        "#,
    )
    .bind(command.lease_seconds)
    .bind(token)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(
        candidate.map(|(operation_id, attempt)| OwnerReceiptClaimDto {
            operation_id,
            token,
            attempt,
        }),
    )
}
async fn finish_lookup(
    pool: &PgPool,
    command: OwnerReceiptFinishLookupDto<'_>,
) -> anyhow::Result<bool> {
    let claim = command.claim;
    let suspended_reason = command.suspended_reason;

    let mut tx = bounded_transaction(pool).await?;
    let changed = sqlx::query(
        r#"
        update gateway_request_journal
        set owner_receipt_claim_id = null, owner_receipt_suspended_reason = $3,
            owner_receipt_next_check_at = case when $3::text is null
                then now() + make_interval(secs => $4) else null end
        where id = $1 and owner_receipt_claim_id = $2
          and state = 'UNKNOWN' and outcome = 'UNKNOWN' and outcome_evidence <> 'MANUAL_RESOLUTION'
          and actor_id is not distinct from $5 and action = $6 and resource_path = $7
        "#,
    )
    .bind(claim.operation_id)
    .bind(claim.token)
    .bind(suspended_reason)
    .bind(command.retry_seconds)
    .bind(command.identity.actor)
    .bind(command.identity.action)
    .bind(command.identity.resource_path)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(changed.rows_affected() == 1)
}
async fn record_commit(pool: &PgPool, command: OwnerReceiptCommitDto<'_>) -> anyhow::Result<bool> {
    let mut tx = bounded_transaction(pool).await?;
    let recorded = sqlx::query(
        r#"
                    with updated as (
                        update gateway_request_journal
                        set outcome = 'COMMITTED', outcome_evidence = 'OWNER_RECEIPT',
                            owner_receipt_checked_at = now(), owner_receipt_claim_id = null,
                            owner_receipt_next_check_at = null
                        where id = $1 and state = 'UNKNOWN' and outcome = 'UNKNOWN'
                          and outcome_evidence <> 'MANUAL_RESOLUTION'
                          and actor_id is not distinct from $6
                          and action = $7 and resource_path = $8
                          and owner_receipt_claim_id = $9
                        returning id, actor_id
                    )
                    insert into admin_audit_events (
                        id, actor_id, action, resource_type, resource_id, outcome,
                        metadata_json, occurred_at
                    )
                    select $2, null, 'OPERATION_RECONCILED_FROM_OWNER_RECEIPT',
                           'gateway_request_journal', id::text, 'COMMITTED',
                           jsonb_build_object(
                               'scope', $3::text,
                               'receiptState', $4::text,
                               'committedAt', $5::text,
                               'operationActorId', actor_id
                           )::text,
                           now()
                    from updated
                    "#,
    )
    .bind(command.claim.operation_id)
    .bind(Uuid::new_v4())
    .bind(command.scope)
    .bind(command.receipt_state)
    .bind(command.committed_at.map(|at| at.to_rfc3339()))
    .bind(command.identity.actor)
    .bind(command.identity.action)
    .bind(command.identity.resource_path)
    .bind(command.claim.token)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    tx.commit().await?;
    Ok(recorded == 1)
}
async fn local_receipt(
    pool: &PgPool,
    query: OperationJournalLookupDto,
) -> anyhow::Result<Option<LocalOwnerReceiptRecordDto>> {
    let row = sqlx::query(
        r#"
            select actor_id, scope, state, committed_at, replay_until,
                   (outcome_json is not null and replay_until > now()) as replayable
            from moderation_report_operation_receipts
            where operation_id = $1
            "#,
    )
    .bind(query.operation_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| LocalOwnerReceiptRecordDto {
        actor_id: row.get("actor_id"),
        scope: row.get("scope"),
        state: row.get("state"),
        committed_at: row.get("committed_at"),
        replay_until: row.get("replay_until"),
        replayable: row.get("replayable"),
    }))
}
