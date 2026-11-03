//! PostgreSQL operation lookup and append-only manual reconciliation.
//! Private actor correlation is returned only in internal repository records.
use crate::application::dto::journal_repository_dto::*;
use crate::application::journal_repository_error::{
    JournalRepositoryError, JournalRepositoryResult,
};
use crate::application::ports::OperationJournalRepositoryPort;
use crate::domain::operation::{
    ManualOperationResolution, ManualResolutionDisposition, ManualResolutionEvidenceKind,
    OperationOutcome,
};
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct PgOperationJournalRepository {
    pool: PgPool,
}
impl PgOperationJournalRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn classify_journal_error(error: anyhow::Error) -> JournalRepositoryError {
    if error.chain().any(|cause| cause.is::<sqlx::Error>()) {
        JournalRepositoryError::Storage
    } else {
        JournalRepositoryError::Unexpected
    }
}

#[async_trait]
impl OperationJournalRepositoryPort for PgOperationJournalRepository {
    async fn operation_status(
        &self,
        query: OperationJournalLookupDto,
    ) -> JournalRepositoryResult<Option<OperationJournalRecordDto>> {
        operation_status(&self.pool, query.operation_id)
            .await
            .map_err(classify_journal_error)
    }
    async fn caller_operation_status(
        &self,
        query: CallerOperationJournalLookupDto,
    ) -> JournalRepositoryResult<Option<CallerOperationJournalRecordDto>> {
        caller_operation_status(&self.pool, query.operation_id, &query.actor_id)
            .await
            .map_err(classify_journal_error)
    }
    async fn reconciliation_backlog(&self) -> JournalRepositoryResult<ReconciliationBacklog> {
        reconciliation_backlog(&self.pool)
            .await
            .map_err(classify_journal_error)
    }
    async fn record_manual_resolution(
        &self,
        command: ManualResolutionJournalCommandDto,
    ) -> JournalRepositoryResult<ManualResolutionJournalWriteDto> {
        record_manual_resolution(&self.pool, &command)
            .await
            .map_err(classify_journal_error)
    }
}

async fn record_manual_resolution(
    pool: &PgPool,
    request: &ManualResolutionJournalCommandDto,
) -> anyhow::Result<ManualResolutionJournalWriteDto> {
    let operation_id = request.operation_id;
    let reviewer_id = request.reviewer_id.as_str();
    let request_id = request.request_id.as_deref();
    let ip_address = request.ip_address.as_deref();
    let user_agent = request.user_agent.as_deref();

    let mut tx = pool.begin().await?;
    sqlx::query("select set_config('lock_timeout', '500ms', true), set_config('statement_timeout', '1s', true)")
        .execute(&mut *tx)
        .await?;

    let journal: Option<(String, String, String)> = sqlx::query_as(
        "select state, outcome, outcome_evidence from gateway_request_journal where id = $1 for update",
    )
    .bind(operation_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((state, stored_outcome, stored_evidence)) = journal else {
        tx.rollback().await?;
        return Ok(ManualResolutionJournalWriteDto::OperationNotFound);
    };
    if state != "UNKNOWN" {
        tx.rollback().await?;
        return Ok(ManualResolutionJournalWriteDto::OperationNotUnknown);
    }

    let current_row = sqlx::query(
        r#"
        select resolution_id, operation_id, revision, supersedes_resolution_id,
               disposition, evidence_kind, evidence_reference, reason, reviewer_id, created_at
        from gateway_operation_manual_reconciliations
        where operation_id = $1
        order by revision desc
        limit 1
        "#,
    )
    .bind(operation_id)
    .fetch_optional(&mut *tx)
    .await?;
    let current = current_row
        .as_ref()
        .map(manual_resolution_from_row)
        .transpose()?;

    if let Some(existing) = current.as_ref() {
        if manual_resolution_matches(existing, reviewer_id, request) {
            tx.commit().await?;
            return Ok(ManualResolutionJournalWriteDto::Existing(
                existing.clone().into(),
            ));
        }
    }

    // The route's owner lookup can fail, or a reconciler can prove the commit
    // after that lookup. Check durable evidence under the same row lock as the
    // decision: losing the owner receipt later never invalidates this proof.
    let owner_proven_commit = stored_outcome == "COMMITTED"
        && (matches!(
            stored_evidence.as_str(),
            "OWNER_RECEIPT" | "OWNER_REPLAY_RECEIPT" | "SUCCESS_RESPONSE"
        ) || current.as_ref().is_some_and(|resolution| {
            resolution.disposition == ManualResolutionDisposition::Committed
                && resolution.evidence_kind == ManualResolutionEvidenceKind::OwnerConfirmation
                && resolution.evidence_reference == operation_id.to_string()
        }));
    if owner_proven_commit
        && (gateway_domain::operation::ManualResolutionDisposition::from(request.disposition)
            != ManualResolutionDisposition::Committed
            || gateway_domain::operation::ManualResolutionEvidenceKind::from(request.evidence_kind)
                != ManualResolutionEvidenceKind::OwnerConfirmation
            || request.evidence_reference != operation_id.to_string())
    {
        tx.rollback().await?;
        return Ok(ManualResolutionJournalWriteDto::Conflict);
    }

    if request.supersedes_resolution_id != current.as_ref().map(|value| value.resolution_id) {
        tx.rollback().await?;
        return Ok(ManualResolutionJournalWriteDto::Conflict);
    }

    let mut resolution = ManualOperationResolution {
        resolution_id: Uuid::new_v4(),
        operation_id,
        revision: current.as_ref().map_or(1, |value| value.revision + 1),
        supersedes_resolution_id: request.supersedes_resolution_id,
        disposition: gateway_domain::operation::ManualResolutionDisposition::from(
            request.disposition,
        ),
        evidence_kind: gateway_domain::operation::ManualResolutionEvidenceKind::from(
            request.evidence_kind,
        ),
        evidence_reference: request.evidence_reference.clone(),
        reason: request.reason.trim().to_owned(),
        reviewer_id: reviewer_id.to_owned(),
        created_at: chrono::Utc::now(),
    };

    resolution.created_at = sqlx::query_scalar(
        r#"
        insert into gateway_operation_manual_reconciliations (
            resolution_id, operation_id, revision, supersedes_resolution_id,
            disposition, evidence_kind, evidence_reference, reason, reviewer_id, created_at
        ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, now())
        returning created_at
        "#,
    )
    .bind(resolution.resolution_id)
    .bind(resolution.operation_id)
    .bind(resolution.revision)
    .bind(resolution.supersedes_resolution_id)
    .bind(resolution.disposition.as_str())
    .bind(resolution.evidence_kind.as_str())
    .bind(&resolution.evidence_reference)
    .bind(&resolution.reason)
    .bind(&resolution.reviewer_id)
    .fetch_one(&mut *tx)
    .await?;

    let outcome = match resolution.disposition {
        ManualResolutionDisposition::Committed => OperationOutcome::Committed,
        ManualResolutionDisposition::NotCommitted => OperationOutcome::RejectedBeforeCommit,
        ManualResolutionDisposition::Unresolved => OperationOutcome::Unknown,
    };
    sqlx::query(
        "update gateway_request_journal set outcome = $2, outcome_evidence = case when $3 then outcome_evidence else 'MANUAL_RESOLUTION' end where id = $1 and state = 'UNKNOWN'",
    )
    .bind(operation_id)
    .bind(outcome.as_str())
    .bind(owner_proven_commit)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        insert into admin_audit_events (
            id, actor_id, action, resource_type, resource_id, outcome,
            request_id, ip_address, user_agent, metadata_json, occurred_at
        ) values (
            $1, $2, 'OPERATION_MANUALLY_RECONCILED', 'gateway_request_journal', $3,
            $4, $5, $6, $7,
            jsonb_build_object(
                'resolutionId', $1::text,
                'revision', $8,
                'disposition', $4,
                'evidenceKind', $9,
                'evidenceReference', $10,
                'reason', $11,
                'supersedesResolutionId', $12::text
            )::text,
            now()
        )
        "#,
    )
    .bind(resolution.resolution_id)
    .bind(reviewer_id)
    .bind(operation_id.to_string())
    .bind(resolution.disposition.as_str())
    .bind(request_id)
    .bind(ip_address)
    .bind(user_agent)
    .bind(resolution.revision)
    .bind(resolution.evidence_kind.as_str())
    .bind(&resolution.evidence_reference)
    .bind(&resolution.reason)
    .bind(resolution.supersedes_resolution_id.map(|id| id.to_string()))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(ManualResolutionJournalWriteDto::Created(resolution.into()))
}

fn manual_resolution_matches(
    existing: &ManualOperationResolution,
    reviewer_id: &str,
    request: &ManualResolutionJournalCommandDto,
) -> bool {
    existing.reviewer_id == reviewer_id
        && existing.disposition
            == gateway_domain::operation::ManualResolutionDisposition::from(request.disposition)
        && existing.evidence_kind
            == gateway_domain::operation::ManualResolutionEvidenceKind::from(request.evidence_kind)
        && existing.evidence_reference == request.evidence_reference
        && existing.reason == request.reason.trim()
        && existing.supersedes_resolution_id == request.supersedes_resolution_id
}

fn manual_resolution_from_row(
    row: &sqlx::postgres::PgRow,
) -> anyhow::Result<ManualOperationResolution> {
    Ok(ManualOperationResolution {
        resolution_id: row.try_get("resolution_id")?,
        operation_id: row.try_get("operation_id")?,
        revision: row.try_get("revision")?,
        supersedes_resolution_id: row.try_get("supersedes_resolution_id")?,
        disposition: ManualResolutionDisposition::parse(row.try_get("disposition")?)?,
        evidence_kind: ManualResolutionEvidenceKind::parse(row.try_get("evidence_kind")?)?,
        evidence_reference: row.try_get("evidence_reference")?,
        reason: row.try_get("reason")?,
        reviewer_id: row.try_get("reviewer_id")?,
        created_at: row.try_get("created_at")?,
    })
}

async fn reconciliation_backlog(pool: &PgPool) -> anyhow::Result<ReconciliationBacklog> {
    let row = sqlx::query(
        r#"
        select count(*) as outstanding,
               coalesce(extract(epoch from now() - min(created_at)), 0)::bigint as oldest_age_seconds
        from gateway_request_journal
        where state = 'UNKNOWN'
          and outcome_evidence <> 'OWNER_RECEIPT'
          and not exists (
              select 1 from gateway_operation_manual_reconciliations resolution
              where resolution.operation_id = gateway_request_journal.id
                and resolution.revision = (
                    select max(latest.revision)
                    from gateway_operation_manual_reconciliations latest
                    where latest.operation_id = gateway_request_journal.id
                )
                and resolution.disposition in ('COMMITTED', 'NOT_COMMITTED')
          )
        "#,
    )
    .fetch_one(pool)
    .await?;
    Ok(ReconciliationBacklog {
        outstanding: row.get("outstanding"),
        oldest_age_seconds: row.get("oldest_age_seconds"),
    })
}

async fn operation_status(
    pool: &PgPool,
    operation_id: Uuid,
) -> anyhow::Result<Option<OperationJournalRecordDto>> {
    let row = sqlx::query(
        r#"
        select journal.id, journal.actor_id, journal.state, journal.outcome,
               journal.action, journal.resource_path,
               journal.response_status, journal.created_at, journal.finished_at,
               (journal.response_body is not null) as replayable,
               reconciliation.resolution_id as reconciliation_id,
               reconciliation.operation_id as reconciliation_operation_id,
               reconciliation.revision as reconciliation_revision,
               reconciliation.supersedes_resolution_id as reconciliation_supersedes_id,
               reconciliation.disposition as reconciliation_disposition,
               reconciliation.evidence_kind as reconciliation_evidence_kind,
               reconciliation.evidence_reference as reconciliation_evidence_reference,
               reconciliation.reason as reconciliation_reason,
               reconciliation.reviewer_id as reconciliation_reviewer_id,
               reconciliation.created_at as reconciliation_created_at
        from gateway_request_journal journal
        left join lateral (
            select * from gateway_operation_manual_reconciliations
            where operation_id = journal.id
            order by revision desc
            limit 1
        ) reconciliation on true
        where journal.id = $1
        "#,
    )
    .bind(operation_id)
    .fetch_optional(pool)
    .await?;
    row.map(|row| {
        let manual_resolution = if row
            .try_get::<Option<Uuid>, _>("reconciliation_id")?
            .is_some()
        {
            Some(manual_resolution_from_status_row(&row)?)
        } else {
            None
        };
        Ok(OperationJournalRecordDto {
            operation_id: row.get("id"),
            actor_id: row.get("actor_id"),
            state: row.get("state"),
            outcome: OperationOutcome::parse(row.try_get::<String, _>("outcome")?.as_str())?.into(),
            action: row.get("action"),
            resource_path: row.get("resource_path"),
            response_status: row.get("response_status"),
            created_at: row.get("created_at"),
            finished_at: row.get("finished_at"),
            replayable: row.get("replayable"),
            manual_resolution: manual_resolution.map(Into::into),
        })
    })
    .transpose()
}

async fn caller_operation_status(
    pool: &PgPool,
    operation_id: Uuid,
    actor_id: &str,
) -> anyhow::Result<Option<CallerOperationJournalRecordDto>> {
    let row = sqlx::query(
        r#"
        select id, state, outcome, response_status, created_at, finished_at,
               (response_body is not null) as replayable
        from gateway_request_journal
        where id = $1 and actor_id = $2
        "#,
    )
    .bind(operation_id)
    .bind(actor_id)
    .fetch_optional(pool)
    .await?;
    row.map(|row| {
        Ok(CallerOperationJournalRecordDto {
            operation_id: row.try_get("id")?,
            state: row.try_get("state")?,
            outcome: OperationOutcome::parse(row.try_get::<String, _>("outcome")?.as_str())?.into(),
            response_status: row.try_get("response_status")?,
            created_at: row.try_get("created_at")?,
            finished_at: row.try_get("finished_at")?,
            replayable: row.try_get("replayable")?,
        })
    })
    .transpose()
}

fn manual_resolution_from_status_row(
    row: &sqlx::postgres::PgRow,
) -> anyhow::Result<ManualOperationResolution> {
    Ok(ManualOperationResolution {
        resolution_id: row.try_get("reconciliation_id")?,
        operation_id: row.try_get("reconciliation_operation_id")?,
        revision: row.try_get("reconciliation_revision")?,
        supersedes_resolution_id: row.try_get("reconciliation_supersedes_id")?,
        disposition: ManualResolutionDisposition::parse(
            row.try_get("reconciliation_disposition")?,
        )?,
        evidence_kind: ManualResolutionEvidenceKind::parse(
            row.try_get("reconciliation_evidence_kind")?,
        )?,
        evidence_reference: row.try_get("reconciliation_evidence_reference")?,
        reason: row.try_get("reconciliation_reason")?,
        reviewer_id: row.try_get("reconciliation_reviewer_id")?,
        created_at: row.try_get("reconciliation_created_at")?,
    })
}
