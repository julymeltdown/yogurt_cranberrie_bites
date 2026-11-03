//! PostgreSQL intent, replay, completion and bounded retention writes.
use crate::application::dto::journal_write_dto::*;
use crate::application::journal_write_error::{JournalWriteError, JournalWriteResult};
use crate::application::ports::*;
use crate::config::{
    DatabasePoolCaller, DatabasePoolRole, GatewayMetrics, ObservedDatabaseConnection,
};
use crate::domain::operation::{
    ManualResolutionDisposition, OperationOutcome, OperationOutcomeDecision,
};
use async_trait::async_trait;
use sqlx::{pool::PoolConnection, Connection, PgConnection, PgPool, Postgres, Row};
use uuid::Uuid;

/// Preserve the concrete retry causes used by bounded completion attempts.
pub(super) fn classify_write_error(error: anyhow::Error) -> JournalWriteError {
    match error.downcast_ref::<sqlx::Error>() {
        Some(sqlx::Error::PoolTimedOut | sqlx::Error::Io(_)) => JournalWriteError::Transient,
        Some(sqlx::Error::Database(database))
            if database.code().is_some_and(|code| {
                matches!(
                    code.as_ref(),
                    "55P03" | "57014" | "40001" | "40P01" | "53300"
                ) || code.starts_with("08")
            }) =>
        {
            JournalWriteError::Transient
        }
        _ => JournalWriteError::Permanent,
    }
}

#[derive(Clone)]
pub struct PgJournalDispatchRepository {
    pool: PgPool,
    metrics: Option<GatewayMetrics>,
}
impl PgJournalDispatchRepository {
    pub fn new(pool: PgPool, metrics: Option<GatewayMetrics>) -> Self {
        Self { pool, metrics }
    }
    async fn acquire(&self, caller: DatabasePoolCaller) -> JournalWriteResult<PgJournalConnection> {
        let held = match &self.metrics {
            Some(metrics) => JournalConnection::Observed(
                metrics
                    .acquire_database_connection(DatabasePoolRole::Journal, caller, &self.pool)
                    .await
                    .map_err(|e| classify_write_error(e.into()))?,
            ),
            None => JournalConnection::Plain(
                self.pool
                    .acquire()
                    .await
                    .map_err(|e| classify_write_error(e.into()))?,
            ),
        };
        Ok(PgJournalConnection { held })
    }
}
enum JournalConnection {
    Observed(ObservedDatabaseConnection),
    Plain(PoolConnection<Postgres>),
}
struct PgJournalConnection {
    held: JournalConnection,
}
impl PgJournalConnection {
    fn connection(&mut self) -> &mut PgConnection {
        match &mut self.held {
            JournalConnection::Observed(c) => c,
            JournalConnection::Plain(c) => c,
        }
    }
}
#[async_trait]
impl JournalDispatchRepositoryPort for PgJournalDispatchRepository {
    async fn acquire_intent_connection(
        &self,
    ) -> JournalWriteResult<Box<dyn JournalIntentConnectionPort>> {
        Ok(Box::new(self.acquire(DatabasePoolCaller::Intent).await?))
    }
    async fn acquire_completion_connection(
        &self,
        caller: JournalCompletionCallerDto,
    ) -> JournalWriteResult<Box<dyn JournalCompletionConnectionPort>> {
        let caller = match caller {
            JournalCompletionCallerDto::Completion => DatabasePoolCaller::Completion,
            JournalCompletionCallerDto::OwnerReplay => DatabasePoolCaller::OwnerReplay,
        };
        Ok(Box::new(self.acquire(caller).await?))
    }
    async fn owner_replay_identity(
        &self,
        query: JournalReplayQueryDto<'_>,
    ) -> JournalWriteResult<Option<JournalReplayIdentityDto>> {
        owner_replay_identity(&self.pool, query)
            .await
            .map_err(classify_write_error)
    }
    async fn response_replay(
        &self,
        query: JournalReplayQueryDto<'_>,
    ) -> JournalWriteResult<Option<JournalReplayResponseDto>> {
        response_replay(&self.pool, query)
            .await
            .map_err(classify_write_error)
    }
}
#[async_trait]
impl JournalIntentConnectionPort for PgJournalConnection {
    async fn insert_intent(
        &mut self,
        command: JournalIntentDto<'_>,
    ) -> JournalWriteResult<Option<Uuid>> {
        insert_intent(self.connection(), command)
            .await
            .map_err(classify_write_error)
    }
}
#[async_trait]
impl JournalCompletionConnectionPort for PgJournalConnection {
    async fn record_completion(
        &mut self,
        command: JournalCompletionCommandDto<'_>,
    ) -> JournalWriteResult<JournalCompletionWriteDto> {
        record_completion(self.connection(), command)
            .await
            .map_err(classify_write_error)
    }
    async fn record_owner_replay(
        &mut self,
        command: JournalOwnerReplayCommandDto<'_>,
    ) -> JournalWriteResult<JournalCompletionWriteDto> {
        finish_owner_replay_on(
            self.connection(),
            command.id,
            command.owner,
            command.status,
            command.content_type,
            command.body,
        )
        .await
        .map_err(classify_write_error)
    }
}
fn replay_identity(row: sqlx::postgres::PgRow) -> JournalReplayIdentityDto {
    JournalReplayIdentityDto {
        id: row.get("id"),
        state: row.get("state"),
        same_request: row.get("same_request"),
        unexpired: row.get("unexpired"),
    }
}

async fn insert_intent(
    connection: &mut PgConnection,
    command: JournalIntentDto<'_>,
) -> anyhow::Result<Option<Uuid>> {
    let inserted = sqlx::query_scalar(
        r#"
        insert into gateway_request_journal (
            id, actor_id, action, resource_path, request_id, ip_address, user_agent,
            idempotency_key, request_fingerprint
        ) values ($1, $2, $3, $4, $5, $6, $7, $8,
            encode(sha256(convert_to($9, 'UTF8') || $10::bytea), 'hex'))
        on conflict (actor_id, idempotency_key) where idempotency_key is not null do nothing
        returning id
        "#,
    )
    .bind(command.id)
    .bind(command.actor)
    .bind(command.action)
    .bind(command.path)
    .bind(command.request_id)
    .bind(command.ip)
    .bind(command.agent)
    .bind(command.key)
    .bind(command.context)
    .bind(command.fingerprint_body)
    .fetch_optional(connection)
    .await?;
    Ok(inserted)
}

async fn owner_replay_identity(
    pool: &PgPool,
    query: JournalReplayQueryDto<'_>,
) -> anyhow::Result<Option<JournalReplayIdentityDto>> {
    let JournalReplayQueryDto {
        actor,
        key,
        context,
        body,
    } = query;

    let row = sqlx::query("select id, state, request_fingerprint = encode(sha256(convert_to($3,'UTF8') || $4::bytea),'hex') as same_request, expires_at > now() as unexpired from gateway_request_journal where actor_id=$1 and idempotency_key=$2")
        .bind(actor).bind(key).bind(context).bind(body).fetch_optional(pool).await
?;
    Ok(row.map(replay_identity))
}

async fn response_replay(
    pool: &PgPool,
    query: JournalReplayQueryDto<'_>,
) -> anyhow::Result<Option<JournalReplayResponseDto>> {
    let JournalReplayQueryDto {
        actor,
        key,
        context,
        body,
    } = query;

    let row = sqlx::query(
        "select id, state, response_status, response_content_type, response_body, request_fingerprint = encode(sha256(convert_to($3, 'UTF8') || $4::bytea), 'hex') as same_request, expires_at > now() as unexpired from gateway_request_journal where actor_id = $1 and idempotency_key = $2"
    ).bind(actor).bind(key).bind(context).bind(body).fetch_optional(pool).await
?;
    Ok(row.map(|row| JournalReplayResponseDto {
        response_status: row.get("response_status"),
        response_content_type: row.get("response_content_type"),
        response_body: row.get("response_body"),
        identity: replay_identity(row),
    }))
}

async fn finish_owner_replay_on(
    connection: &mut PgConnection,
    id: Uuid,
    owner: &str,
    status: u16,
    content_type: Option<&str>,
    body: Option<&[u8]>,
) -> anyhow::Result<JournalCompletionWriteDto> {
    let mut tx = connection.begin().await?;
    sqlx::query("select set_config('lock_timeout', '500ms', true), set_config('statement_timeout', '1s', true)").execute(&mut *tx).await?;
    // A successful lookup is proof of COMMIT, not another mutation attempt.
    // The original audit stays immutable; reconciliation adds a separate event.
    let owner_audit = sqlx::query(
        "with resolved as (
            update gateway_request_journal set state='COMPLETED', outcome='COMMITTED',
                outcome_evidence='OWNER_REPLAY_RECEIPT', response_status=$2,
                response_content_type=$3, response_body=$4, finished_at=now()
            where id=$1 and (state <> 'COMPLETED' or response_status is null
                or response_status < 200 or response_status >= 300)
            returning actor_id, resource_path, request_id, ip_address, user_agent
        ) insert into admin_audit_events (id, actor_id, action, resource_type, resource_id,
            outcome, request_id, ip_address, user_agent, metadata_json, occurred_at)
          select $5, actor_id, 'OPERATION_OWNER_RECONCILED', resource_path, $1::text,
            'SUCCESS', request_id, ip_address, user_agent,
            jsonb_build_object('owner', $6::text, 'evidence', 'committed-receipt')::text,
            now() from resolved",
    )
    .bind(id)
    .bind(i32::from(status))
    .bind(content_type)
    .bind(body)
    .bind(Uuid::new_v4())
    .bind(owner)
    .execute(&mut *tx)
    .await?;

    // If an operator previously attested that the write did not commit (or left
    // it unresolved), a later verified owner receipt is stronger evidence. Keep
    // the operator event immutable and append a system-authored correction.
    if owner_audit.rows_affected() > 0 {
        let prior = sqlx::query(
            r#"
            select resolution_id, revision, disposition
            from gateway_operation_manual_reconciliations
            where operation_id = $1
            order by revision desc
            limit 1
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(prior) = prior {
            let prior_disposition: String = prior.try_get("disposition")?;
            let prior_disposition = ManualResolutionDisposition::parse(&prior_disposition)?;
            if prior_disposition != ManualResolutionDisposition::Committed {
                let prior_id: Uuid = prior.try_get("resolution_id")?;
                let revision: i32 = prior.try_get("revision")?;
                let resolution_id = Uuid::new_v4();
                let reviewer = "system:gateway-owner-replay";
                let evidence_reference = id.to_string();
                let reason =
                    "A verified committed owner receipt superseded the prior manual disposition.";
                sqlx::query(
                    r#"
                    insert into gateway_operation_manual_reconciliations (
                        resolution_id, operation_id, revision, supersedes_resolution_id,
                        disposition, evidence_kind, evidence_reference, reason, reviewer_id, created_at
                    ) values ($1, $2, $3, $4, 'COMMITTED', 'OWNER_CONFIRMATION', $5, $6, $7, now())
                    "#,
                )
                .bind(resolution_id)
                .bind(id)
                .bind(revision + 1)
                .bind(prior_id)
                .bind(&evidence_reference)
                .bind(reason)
                .bind(reviewer)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    r#"
                    insert into admin_audit_events (
                        id, actor_id, action, resource_type, resource_id, outcome,
                        metadata_json, occurred_at
                    ) values (
                        $1, $2, 'OPERATION_RECONCILIATION_SUPERSEDED_BY_OWNER',
                        'gateway_request_journal', $3, 'SUCCESS',
                        jsonb_build_object(
                            'resolutionId', $1::text,
                            'supersedesResolutionId', $4::text,
                            'evidence', 'committed-owner-receipt'
                        )::text,
                        now()
                    )
                    "#,
                )
                .bind(resolution_id)
                .bind(reviewer)
                .bind(id.to_string())
                .bind(prior_id)
                .execute(&mut *tx)
                .await?;
            }
        }
    }
    tx.commit().await?;
    Ok(if owner_audit.rows_affected() > 0 {
        JournalCompletionWriteDto::Recorded
    } else {
        JournalCompletionWriteDto::AlreadyResolved
    })
}

async fn record_completion(
    connection: &mut PgConnection,
    request: JournalCompletionCommandDto<'_>,
) -> anyhow::Result<JournalCompletionWriteDto> {
    let classified = OperationOutcomeDecision {
        outcome: request.outcome.into(),
        evidence: request.evidence.into(),
    };
    let mut tx = connection.begin().await?;
    sqlx::query("select set_config('lock_timeout', '500ms', true), set_config('statement_timeout', '1s', true)")
        .execute(&mut *tx).await?;
    // The outcome transition and its audit event are one statement: the audit
    // row is written only for the call that actually moved the row out of
    // IN_PROGRESS, and the caller learns whether that call was this one.
    let transitioned: i64 = sqlx::query_scalar(
        r#"
        with updated as (
            update gateway_request_journal
            set state = $2, outcome = $3, outcome_evidence = $4, response_status = $5,
                response_content_type = $6, response_body = $7, finished_at = now()
            where id = $1 and state = 'IN_PROGRESS'
            returning id, actor_id, action, resource_path, request_id, ip_address, user_agent
        ), audited as (
            insert into admin_audit_events (
                id, actor_id, action, resource_type, resource_id, outcome,
                request_id, ip_address, user_agent, metadata_json, occurred_at
            ) select id, actor_id, action, resource_path, null, $8,
                request_id, ip_address, user_agent, '{}', now()
              from updated
            on conflict (id) do nothing
            returning id
        )
        select count(*) from updated
        "#,
    )
    .bind(request.id)
    .bind(if classified.outcome == OperationOutcome::Unknown {
        "UNKNOWN"
    } else {
        "COMPLETED"
    })
    .bind(classified.outcome.as_str())
    .bind(classified.evidence.as_str())
    .bind(i32::from(request.status))
    .bind(request.content_type)
    .bind(request.body)
    .bind(classified.outcome.audit_result())
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(if transitioned > 0 {
        JournalCompletionWriteDto::Recorded
    } else {
        JournalCompletionWriteDto::AlreadyResolved
    })
}

#[derive(Clone)]
pub struct PgJournalRetentionRepository {
    pool: PgPool,
}
impl PgJournalRetentionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
#[async_trait]
impl JournalRetentionRepositoryPort for PgJournalRetentionRepository {
    async fn purge(&self, command: JournalPurgeCommandDto) -> JournalWriteResult<PurgeOutcome> {
        purge_expired_records(
            &self.pool,
            command.telemetry_retention_days,
            command.audit_retention_days,
        )
        .await
        .map_err(classify_write_error)
    }
}
const PURGE_BATCH_SIZE: i64 = 500;
async fn purge_expired_records(
    pool: &PgPool,
    telemetry_retention_days: i64,
    audit_retention_days: i64,
) -> anyhow::Result<PurgeOutcome> {
    let mut tx = pool.begin().await?;
    // The same short bounds the rest of this module uses. A purge competing with
    // request-path audit writes must not be allowed to hold a lock indefinitely.
    sqlx::query("select set_config('lock_timeout', '500ms', true), set_config('statement_timeout', '1s', true)")
    .execute(&mut *tx)
    .await?;

    let telemetry = sqlx::query(
        r#"
    delete from telemetry_records where id in (
        select id from telemetry_records
            -- The cast is required: the driver binds an integer as int8, and
            -- make_interval's `days` field is int4, so the call would not resolve.
            where recorded_at < now() - make_interval(days => $1::int)
        order by recorded_at
        limit $2
    )
    "#,
    )
    .bind(telemetry_retention_days)
    .bind(PURGE_BATCH_SIZE)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    // Reconciliation-required rows are retained. They exist so an operator can
    // resolve an ambiguous outcome, and an age-based delete would remove that
    // obligation along with the evidence for it.
    let audit = sqlx::query(
        r#"
    delete from admin_audit_events where id in (
        select id from admin_audit_events
            where occurred_at < now() - make_interval(days => $1::int)
              and outcome <> 'UNKNOWN'
              and metadata_json not like '%"reconciliation":"required"%'
        order by occurred_at
        limit $2
    )
    "#,
    )
    .bind(audit_retention_days)
    .bind(PURGE_BATCH_SIZE)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    let rate_limit_keys = sqlx::query(
        r#"
    delete from gateway_rate_limit_keys where client_key in (
        select client_key from gateway_rate_limit_keys
        where expires_at < now()
        limit $1
    )
    "#,
    )
    .bind(PURGE_BATCH_SIZE)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    tx.commit().await?;
    Ok(PurgeOutcome {
        telemetry_records: telemetry,
        audit_events: audit,
        rate_limit_keys,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concrete_driver_causes_retain_retry_classification_through_context() {
        for error in [
            sqlx::Error::PoolTimedOut,
            sqlx::Error::Io(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                "private driver diagnostic",
            )),
        ] {
            assert_eq!(
                classify_write_error(anyhow::Error::new(error).context("physical adapter")),
                JournalWriteError::Transient
            );
        }
        for error in [
            sqlx::Error::PoolClosed,
            sqlx::Error::RowNotFound,
            sqlx::Error::ColumnNotFound("private column".into()),
        ] {
            assert_eq!(
                classify_write_error(error.into()),
                JournalWriteError::Permanent
            );
        }
        assert_eq!(
            classify_write_error(anyhow::anyhow!("invalid private projection")),
            JournalWriteError::Permanent
        );
    }
}
