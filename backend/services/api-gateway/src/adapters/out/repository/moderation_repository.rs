use crate::application::dto::moderation_repository_dto::{
    ContentModerationDecisionBacklogDto as ContentModerationDecisionBacklog,
    ContentModerationDecisionCursorDto as ContentModerationDecisionCursor,
    ContentModerationDecisionOutboxEntryDto as ContentModerationDecisionOutboxEntry,
    ContentModerationDecisionOutboxStateDto as ContentModerationDecisionOutboxState,
    ContentModerationDecisionStageDto as ContentModerationDecisionStage,
    ModerationDecisionDto as ModerationDecision,
    ModerationDecisionTypeDto as ModerationDecisionType, ModerationReportDto as ModerationReport,
    PendingContentModerationDecisionDto as PendingContentModerationDecision,
    ReportCategoryDto as ReportCategory, ReportStatusDto as ReportStatus,
    ReportTargetTypeDto as ReportTargetType,
};
use crate::application::moderation_repository_error::ModerationRepositoryError;
use crate::application::ports::ModerationRepositoryPort;
use crate::domain::moderation::{
    ContentDecisionReplayConflict, ModerationDecisionConflict, ModerationDecisionInProgress,
    ModerationTargetUnavailable, ReportMutationReceiptError, CREATE_REPORT_OWNER_SCOPE,
    MAX_CONTENT_DECISION_ATTEMPTS,
};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
struct ReportReceiptOutcome {
    version: u32,
    report: crate::domain::moderation::ModerationReport,
}

/// Classify concrete causes only. No diagnostic string or SQL driver escapes.
pub fn classify_moderation_repository_error(error: anyhow::Error) -> ModerationRepositoryError {
    use ModerationRepositoryError as Failure;
    for cause in error.chain() {
        if let Some(failure) = cause.downcast_ref::<Failure>() {
            return *failure;
        }
        if let Some(receipt) = cause.downcast_ref::<ReportMutationReceiptError>() {
            return match receipt {
                ReportMutationReceiptError::Missing => Failure::ReceiptMissing,
                ReportMutationReceiptError::Expired => Failure::ReceiptExpired,
                ReportMutationReceiptError::Conflict => Failure::ReceiptConflict,
            };
        }
        if cause.is::<ModerationTargetUnavailable>() {
            return Failure::TargetUnavailable;
        }
        if cause.is::<ModerationDecisionInProgress>() {
            return Failure::DecisionInProgress;
        }
        if cause.is::<ModerationDecisionConflict>() {
            return Failure::DecisionConflict;
        }
        if cause.is::<crate::domain::moderation::ModerationResolutionNoteRequired>() {
            return Failure::ResolutionNoteRequired;
        }
        if cause.is::<ContentDecisionReplayConflict>() {
            return Failure::ReplayConflict;
        }
        if cause.is::<sqlx::Error>() {
            return Failure::Storage;
        }
    }
    Failure::Unexpected
}

#[derive(Clone)]
pub struct PgModerationRepository {
    pool: PgPool,
}

impl PgModerationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl PgModerationRepository {
    async fn create_report_inner(&self, report: ModerationReport) -> Result<ModerationReport> {
        self.create_report_with_mutation_inner(report, None).await
    }

    async fn create_report_with_mutation_inner(
        &self,
        report: ModerationReport,
        context: Option<crate::application::dto::moderation_dto::ModerationMutationContextDto>,
    ) -> Result<ModerationReport> {
        if let Some(context) = context.as_ref() {
            if context.operation_id.is_nil()
                || context.actor_id.is_nil()
                || context.actor_id != report.reporter_id
            {
                return Err(ReportMutationReceiptError::Conflict.into());
            }
            if context.lookup_only {
                return lookup_report_receipt(&self.pool, context).await;
            }
            match lookup_report_receipt(&self.pool, context).await {
                Ok(report) => return Ok(report),
                Err(error)
                    if matches!(
                        error.downcast_ref::<ReportMutationReceiptError>(),
                        Some(ReportMutationReceiptError::Missing)
                    ) => {}
                Err(error) => return Err(error),
            }
        }

        let mut tx = self.pool.begin().await?;
        sqlx::query("select set_config('lock_timeout', '500ms', true), set_config('statement_timeout', '2s', true)")
            .execute(&mut *tx)
            .await?;
        let row = sqlx::query(
            r#"
            insert into moderation_reports (
                id, reporter_id, target_type, target_id, category, reason,
                status, created_at, resolved_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            on conflict (reporter_id, target_type, target_id, category) do update
            set reason = moderation_reports.reason
            returning id, reporter_id, target_type, target_id, category, reason,
                status, created_at, resolved_at
            "#,
        )
        .bind(report.id)
        .bind(report.reporter_id)
        .bind(report.target_type.as_str())
        .bind(report.target_id)
        .bind(report.category.as_str())
        .bind(&report.reason)
        .bind(report.status.as_str())
        .bind(report.created_at)
        .bind(report.resolved_at)
        .fetch_one(&mut *tx)
        .await?;
        let persisted = map_report(row)?;
        if let Some(context) = context.as_ref() {
            let outcome = serde_json::to_value(ReportReceiptOutcome {
                version: 1,
                report: (&persisted).into(),
            })?;
            if let Err(error) = sqlx::query(
                r#"
                insert into moderation_report_operation_receipts (
                    operation_id, actor_id, scope, idempotency_key, outcome_json,
                    committed_at, replay_until
                ) values ($1, $2, $3, $4, $5, now(), now() + interval '24 hours')
                "#,
            )
            .bind(context.operation_id)
            .bind(context.actor_id)
            .bind(CREATE_REPORT_OWNER_SCOPE)
            .bind(context.idempotency_key.as_deref())
            .bind(outcome)
            .execute(&mut *tx)
            .await
            {
                if error
                    .as_database_error()
                    .and_then(|database| database.code())
                    .as_deref()
                    == Some("23505")
                {
                    tx.rollback().await?;
                    return match lookup_report_receipt(&self.pool, context).await {
                        Ok(report) => Ok(report),
                        Err(error)
                            if matches!(
                                error.downcast_ref::<ReportMutationReceiptError>(),
                                Some(ReportMutationReceiptError::Missing)
                            ) =>
                        {
                            Err(ReportMutationReceiptError::Conflict.into())
                        }
                        Err(error) => Err(error),
                    };
                }
                return Err(error.into());
            }
        }
        tx.commit().await?;
        Ok(persisted)
    }

    async fn find_report_inner(&self, report_id: Uuid) -> Result<Option<ModerationReport>> {
        let row = sqlx::query(
            r#"
            select id, reporter_id, target_type, target_id, category, reason,
                status, created_at, resolved_at
            from moderation_reports
            where id = $1
            "#,
        )
        .bind(report_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(map_report).transpose()
    }

    async fn list_by_reporter_inner(
        &self,
        reporter_id: Uuid,
        limit: i32,
    ) -> Result<Vec<ModerationReport>> {
        let rows = sqlx::query(
            r#"
            select id, reporter_id, target_type, target_id, category, reason,
                status, created_at, resolved_at
            from moderation_reports
            where reporter_id = $1
            order by created_at desc, id desc
            limit $2
            "#,
        )
        .bind(reporter_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(map_report).collect()
    }

    async fn list_open_inner(&self, limit: i32) -> Result<Vec<ModerationReport>> {
        let rows = sqlx::query(
            r#"
            select id, reporter_id, target_type, target_id, category, reason,
                status, created_at, resolved_at
            from moderation_reports
            where status = 'OPEN'
            order by created_at asc, id asc
            limit $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(map_report).collect()
    }

    async fn content_decision_backlog_inner(&self) -> Result<ContentModerationDecisionBacklog> {
        let row = sqlx::query(
            r#"
            select
                count(*) filter (where state = 'PENDING')::bigint as pending,
                coalesce(
                    extract(epoch from (
                        now() - min(created_at) filter (where state = 'PENDING')
                    ))::bigint,
                    0
                ) as oldest_pending_age_seconds,
                count(*) filter (where state = 'TARGET_UNAVAILABLE')::bigint
                    as target_unavailable,
                count(*) filter (where state = 'RETRY_EXHAUSTED')::bigint as retry_exhausted
            from moderation_content_decision_outbox
            "#,
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(ContentModerationDecisionBacklog {
            pending: row.get("pending"),
            oldest_pending_age_seconds: row.get("oldest_pending_age_seconds"),
            target_unavailable: row.get("target_unavailable"),
            retry_exhausted: row.get("retry_exhausted"),
        })
    }

    async fn list_content_decisions_inner(
        &self,
        limit: i32,
        after: Option<ContentModerationDecisionCursor>,
    ) -> Result<Vec<ContentModerationDecisionOutboxEntry>> {
        let rows = sqlx::query(
            r#"
            select queue.report_id, reports.target_type, reports.target_id,
                   queue.decision_type, queue.state, queue.attempt_count, queue.replay_count,
                   queue.last_error_code, queue.created_at, queue.updated_at,
                   queue.next_attempt_at
            from moderation_content_decision_outbox queue
            join moderation_reports reports on reports.id = queue.report_id
            where queue.state in ('PENDING', 'TARGET_UNAVAILABLE', 'RETRY_EXHAUSTED')
              and (
                  $1::timestamptz is null
                  or (queue.created_at, queue.report_id) > ($1::timestamptz, $2::uuid)
              )
            order by queue.created_at asc, queue.report_id asc
            limit $3
            "#,
        )
        .bind(after.map(|cursor| cursor.created_at))
        .bind(after.map(|cursor| cursor.report_id))
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                let attempt_count: i32 = row.get("attempt_count");
                anyhow::ensure!(attempt_count >= 0, "invalid moderation attempt count");
                let replay_count: i32 = row.get("replay_count");
                anyhow::ensure!(replay_count >= 0, "invalid moderation replay count");
                Ok(ContentModerationDecisionOutboxEntry {
                    report_id: row.get("report_id"),
                    target_type: ReportTargetType::parse(
                        row.get::<String, _>("target_type").as_str(),
                    )?,
                    target_id: row.get("target_id"),
                    decision_type: ModerationDecisionType::parse(
                        row.get::<String, _>("decision_type").as_str(),
                    )?,
                    state: ContentModerationDecisionOutboxState::parse(
                        row.get::<String, _>("state").as_str(),
                    )?,
                    attempt_count: attempt_count as u32,
                    replay_count: replay_count as u32,
                    last_error_code: row.get("last_error_code"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                    next_attempt_at: row.get("next_attempt_at"),
                })
            })
            .collect()
    }

    async fn record_decision_inner(
        &self,
        report_id: Uuid,
        actor_id: Uuid,
        decision_type: ModerationDecisionType,
        note: &str,
        restriction_expires_at: Option<DateTime<Utc>>,
    ) -> Result<(ModerationReport, ModerationDecision)> {
        let mut tx = self.pool.begin().await?;
        let report_row = sqlx::query(
            r#"
            select id, reporter_id, target_type, target_id, category, reason,
                status, created_at, resolved_at
            from moderation_reports
            where id = $1
            for update
            "#,
        )
        .bind(report_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(crate::application::moderation_repository_error::ModerationRepositoryError::ReportMissing)?;
        let mut report = map_report(report_row)?;

        let content_decision_state = sqlx::query_scalar::<_, String>(
            "select state from moderation_content_decision_outbox where report_id = $1 for update",
        )
        .bind(report_id)
        .fetch_optional(&mut *tx)
        .await?;
        match content_decision_state.as_deref() {
            Some("PENDING") => return Err(ModerationDecisionInProgress.into()),
            Some("TARGET_UNAVAILABLE") => {
                if decision_type != ModerationDecisionType::Dismiss {
                    return Err(ModerationDecisionConflict.into());
                }
                if note.trim().is_empty() {
                    return Err(crate::domain::moderation::ModerationResolutionNoteRequired.into());
                }
            }
            Some("RESOLVED_UNAVAILABLE") if decision_type != ModerationDecisionType::Dismiss => {
                return Err(ModerationDecisionConflict.into());
            }
            Some("RETRY_EXHAUSTED") => return Err(ModerationDecisionConflict.into()),
            Some("COMPLETED") | Some("RESOLVED_UNAVAILABLE") | None => {}
            Some(_) => anyhow::bail!("invalid moderation outbox state"),
        }

        let existing = sqlx::query(
            r#"
            select id, report_id, actor_id, decision_type, note, created_at
            from moderation_decisions
            where report_id = $1 and decision_type = $2
            "#,
        )
        .bind(report_id)
        .bind(decision_type.as_str())
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(existing) = existing {
            let decision = map_decision(existing)?;
            tx.commit().await?;
            return Ok((report, decision));
        }

        let decision_row = sqlx::query(
            r#"
            insert into moderation_decisions (
                id, report_id, actor_id, decision_type, note, created_at
            )
            values ($1, $2, $3, $4, $5, now())
            returning id, report_id, actor_id, decision_type, note, created_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(report_id)
        .bind(actor_id)
        .bind(decision_type.as_str())
        .bind(note)
        .fetch_one(&mut *tx)
        .await?;
        let decision = map_decision(decision_row)?;

        let status = if decision_type == ModerationDecisionType::Dismiss {
            ReportStatus::Dismissed
        } else {
            ReportStatus::Resolved
        };
        let resolved_at = Utc::now();
        sqlx::query("update moderation_reports set status = $2, resolved_at = $3 where id = $1")
            .bind(report_id)
            .bind(status.as_str())
            .bind(resolved_at)
            .execute(&mut *tx)
            .await?;
        report.status = status;
        report.resolved_at = Some(resolved_at);

        if content_decision_state.as_deref() == Some("TARGET_UNAVAILABLE") {
            let closed = sqlx::query(
                r#"
                update moderation_content_decision_outbox
                set state = 'RESOLVED_UNAVAILABLE', lease_owner = null,
                    lease_expires_at = null,
                    last_error_code = 'ADMIN_DISMISSED_UNAVAILABLE_TARGET',
                    updated_at = now()
                where report_id = $1 and state = 'TARGET_UNAVAILABLE'
                "#,
            )
            .bind(report_id)
            .execute(&mut *tx)
            .await?;
            anyhow::ensure!(
                closed.rows_affected() == 1,
                "unavailable moderation target changed while being resolved"
            );
        }

        match decision_type {
            ModerationDecisionType::RestrictUser => {
                sqlx::query(
                    r#"
                    insert into user_restrictions (
                        user_id, report_id, reason, restricted_by, restricted_at,
                        expires_at, lifted_at
                    )
                    values ($1, $2, $3, $4, now(), $5, null)
                    on conflict (user_id) do update
                    set report_id = excluded.report_id,
                        reason = excluded.reason,
                        restricted_by = excluded.restricted_by,
                        restricted_at = now(),
                        expires_at = excluded.expires_at,
                        lifted_at = null
                    "#,
                )
                .bind(report.target_id)
                .bind(report_id)
                .bind(if note.is_empty() {
                    &report.reason
                } else {
                    note
                })
                .bind(actor_id)
                .bind(restriction_expires_at)
                .execute(&mut *tx)
                .await?;
            }
            ModerationDecisionType::LiftRestriction => {
                sqlx::query(
                    "update user_restrictions set lifted_at = now() where user_id = $1 and lifted_at is null",
                )
                .bind(report.target_id)
                .execute(&mut *tx)
                .await?;
            }
            _ => {}
        }

        tx.commit().await?;
        Ok((report, decision))
    }

    async fn begin_content_decision_inner(
        &self,
        report_id: Uuid,
        actor_id: Uuid,
        decision_type: ModerationDecisionType,
        note: &str,
        lease_owner: Uuid,
        lease_for: Duration,
    ) -> Result<ContentModerationDecisionStage> {
        anyhow::ensure!(
            matches!(
                decision_type,
                ModerationDecisionType::HideContent | ModerationDecisionType::RestoreContent
            ),
            "durable moderation staging only accepts content decisions"
        );
        anyhow::ensure!(!lease_owner.is_nil(), "moderation lease owner is required");

        let mut tx = self.pool.begin().await?;
        sqlx::query("select set_config('lock_timeout', '500ms', true), set_config('statement_timeout', '2s', true)")
            .execute(&mut *tx)
            .await?;
        let report_row = sqlx::query(
            r#"
            select id, reporter_id, target_type, target_id, category, reason,
                status, created_at, resolved_at
            from moderation_reports
            where id = $1
            for update
            "#,
        )
        .bind(report_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(crate::application::moderation_repository_error::ModerationRepositoryError::ReportMissing)?;
        let report = map_report(report_row)?;

        let existing_decision = sqlx::query(
            r#"
            select id, report_id, actor_id, decision_type, note, created_at
            from moderation_decisions
            where report_id = $1
            order by created_at asc, id asc
            limit 1
            "#,
        )
        .bind(report_id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(existing) = existing_decision {
            let decision = map_decision(existing)?;
            if decision.decision_type == decision_type {
                tx.commit().await?;
                return Ok(ContentModerationDecisionStage::AlreadyRecorded { report, decision });
            }
            return Err(ModerationDecisionConflict.into());
        }
        if report.status != ReportStatus::Open {
            return Err(ModerationDecisionConflict.into());
        }

        let pending = sqlx::query(
            r#"
            select actor_id, decision_type, note, state, attempt_count,
                   lease_owner, lease_expires_at
            from moderation_content_decision_outbox
            where report_id = $1
            for update
            "#,
        )
        .bind(report_id)
        .fetch_optional(&mut *tx)
        .await?;
        let decision_name = decision_type.as_str();
        let attempt_count = if let Some(pending) = pending {
            let state: String = pending.get("state");
            if state == "TARGET_UNAVAILABLE" {
                return Err(ModerationTargetUnavailable.into());
            }
            if state == "RETRY_EXHAUSTED" {
                return Err(ModerationDecisionConflict.into());
            }
            anyhow::ensure!(state == "PENDING", "invalid moderation outbox state");
            if pending.get::<Uuid, _>("actor_id") != actor_id
                || pending.get::<String, _>("decision_type") != decision_name
                || pending.get::<String, _>("note") != note
            {
                return Err(ModerationDecisionConflict.into());
            }
            let current_owner: Option<Uuid> = pending.get("lease_owner");
            let lease_expires_at: Option<chrono::DateTime<Utc>> = pending.get("lease_expires_at");
            if lease_expires_at.is_some_and(|expires| expires > Utc::now())
                && current_owner != Some(lease_owner)
            {
                return Err(ModerationDecisionInProgress.into());
            }
            let attempts: i32 = pending.get("attempt_count");
            let next_attempt = attempts.saturating_add(1);
            sqlx::query(
                r#"
                update moderation_content_decision_outbox
                set lease_owner = $2,
                    lease_expires_at = now() + make_interval(secs => $3::int),
                    attempt_count = $4,
                    updated_at = now()
                where report_id = $1
                "#,
            )
            .bind(report_id)
            .bind(lease_owner)
            .bind(duration_seconds(lease_for))
            .bind(next_attempt)
            .execute(&mut *tx)
            .await?;
            next_attempt
        } else {
            sqlx::query(
                r#"
                insert into moderation_content_decision_outbox (
                    report_id, actor_id, decision_type, note, attempt_count,
                    lease_owner, lease_expires_at
                ) values ($1, $2, $3, $4, 1, $5,
                          now() + make_interval(secs => $6::int))
                "#,
            )
            .bind(report_id)
            .bind(actor_id)
            .bind(decision_name)
            .bind(note)
            .bind(lease_owner)
            .bind(duration_seconds(lease_for))
            .execute(&mut *tx)
            .await?;
            1
        };
        tx.commit().await?;

        Ok(ContentModerationDecisionStage::Pending(
            PendingContentModerationDecision {
                report,
                actor_id,
                decision_type,
                note: note.to_owned(),
                attempt_count: u32::try_from(attempt_count).unwrap_or(u32::MAX),
                lease_owner,
            },
        ))
    }

    async fn claim_next_content_decision_inner(
        &self,
        lease_owner: Uuid,
        lease_for: Duration,
    ) -> Result<Option<PendingContentModerationDecision>> {
        anyhow::ensure!(!lease_owner.is_nil(), "moderation lease owner is required");
        let mut tx = self.pool.begin().await?;
        sqlx::query("select set_config('lock_timeout', '500ms', true), set_config('statement_timeout', '2s', true)")
            .execute(&mut *tx)
            .await?;
        let row = sqlx::query(
            r#"
            with candidate as (
                select queue.report_id
                from moderation_content_decision_outbox queue
                join moderation_reports report on report.id = queue.report_id
                where queue.state = 'PENDING'
                  and queue.next_attempt_at <= now()
                  and (queue.lease_expires_at is null or queue.lease_expires_at <= now())
                  and report.status = 'OPEN'
                order by queue.next_attempt_at asc, queue.created_at asc, queue.report_id asc
                for update of queue skip locked
                limit 1
            )
            update moderation_content_decision_outbox queue
            set lease_owner = $1,
                lease_expires_at = now() + make_interval(secs => $2::int),
                attempt_count = queue.attempt_count + 1,
                updated_at = now()
            from candidate
            where queue.report_id = candidate.report_id
            returning queue.report_id, queue.actor_id, queue.decision_type,
                      queue.note, queue.attempt_count, queue.lease_owner
            "#,
        )
        .bind(lease_owner)
        .bind(duration_seconds(lease_for))
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };
        let report_id: Uuid = row.get("report_id");
        let report_row = sqlx::query(
            r#"
            select id, reporter_id, target_type, target_id, category, reason,
                status, created_at, resolved_at
            from moderation_reports
            where id = $1
            "#,
        )
        .bind(report_id)
        .fetch_one(&mut *tx)
        .await?;
        let pending = map_pending_content_decision(row, map_report(report_row)?)?;
        tx.commit().await?;
        Ok(Some(pending))
    }

    async fn complete_content_decision_inner(
        &self,
        pending: &PendingContentModerationDecision,
    ) -> Result<(ModerationReport, ModerationDecision)> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("select set_config('lock_timeout', '500ms', true), set_config('statement_timeout', '2s', true)")
            .execute(&mut *tx)
            .await?;
        // Keep the lock order consistent with begin_content_decision and
        // record_decision (report first, then outbox) to avoid a report/outbox
        // deadlock when a client retries while the worker is finalizing.
        sqlx::query("select id from moderation_reports where id = $1 for update")
            .bind(pending.report.id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(crate::application::moderation_repository_error::ModerationRepositoryError::ReportMissing)?;
        let stored = sqlx::query(
            r#"
            select actor_id, decision_type, note, state, lease_owner
            from moderation_content_decision_outbox
            where report_id = $1
            for update
            "#,
        )
        .bind(pending.report.id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(stored) = stored else {
            if let Some(result) =
                load_existing_decision(&mut tx, pending.report.id, pending.decision_type).await?
            {
                if result.1.decision_type == pending.decision_type {
                    tx.commit().await?;
                    return Ok(result);
                }
            }
            anyhow::bail!("staged moderation decision is missing")
        };
        let state: String = stored.get("state");
        anyhow::ensure!(
            stored.get::<Uuid, _>("actor_id") == pending.actor_id
                && stored.get::<String, _>("decision_type") == pending.decision_type.as_str()
                && stored.get::<String, _>("note") == pending.note,
            "moderation decision command changed"
        );
        if state == "COMPLETED" {
            let result = load_existing_decision(&mut tx, pending.report.id, pending.decision_type)
                .await?
                .ok_or_else(|| anyhow::anyhow!("completed moderation decision is missing"))?;
            tx.commit().await?;
            return Ok(result);
        }
        anyhow::ensure!(state == "PENDING", "moderation decision is not pending");
        anyhow::ensure!(
            stored.get::<Option<Uuid>, _>("lease_owner") == Some(pending.lease_owner),
            "moderation decision lease or command changed"
        );
        let result = finish_content_decision(&mut tx, pending).await?;
        sqlx::query(
            r#"
            update moderation_content_decision_outbox
            set state = 'COMPLETED', completed_at = now(), lease_owner = null,
                lease_expires_at = null, last_error_code = null, updated_at = now()
            where report_id = $1
            "#,
        )
        .bind(pending.report.id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(result)
    }

    async fn retry_content_decision_inner(
        &self,
        pending: &PendingContentModerationDecision,
        retry_after: Duration,
        error_code: &str,
    ) -> Result<bool> {
        let updated = sqlx::query(
            r#"
            update moderation_content_decision_outbox
            set lease_owner = null, lease_expires_at = null,
                next_attempt_at = now() + make_interval(secs => $3::int),
                last_error_code = $4, updated_at = now()
            where report_id = $1 and state = 'PENDING' and lease_owner = $2
            "#,
        )
        .bind(pending.report.id)
        .bind(pending.lease_owner)
        .bind(duration_seconds(retry_after))
        .bind(error_code)
        .execute(&self.pool)
        .await?;
        Ok(updated.rows_affected() == 1)
    }

    async fn dead_letter_content_decision_inner(
        &self,
        pending: &PendingContentModerationDecision,
        error_code: &str,
    ) -> Result<bool> {
        anyhow::ensure!(
            matches!(
                error_code,
                "POST_SERVICE_CALL_FAILED" | "GATEWAY_DECISION_STORE_FAILED"
            ),
            "invalid moderation retry error code"
        );
        let updated = sqlx::query(
            r#"
            update moderation_content_decision_outbox
            set state = 'RETRY_EXHAUSTED', lease_owner = null,
                lease_expires_at = null, last_error_code = $3, updated_at = now()
            where report_id = $1 and state = 'PENDING' and lease_owner = $2
              and attempt_count >= $4
            "#,
        )
        .bind(pending.report.id)
        .bind(pending.lease_owner)
        .bind(error_code)
        .bind(i32::try_from(MAX_CONTENT_DECISION_ATTEMPTS).unwrap_or(i32::MAX))
        .execute(&self.pool)
        .await?;
        Ok(updated.rows_affected() == 1)
    }

    async fn replay_dead_lettered_content_decision_inner(
        &self,
        report_id: Uuid,
        resolution_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        evidence_reference: &str,
    ) -> Result<bool> {
        anyhow::ensure!(!report_id.is_nil(), "moderation report ID is required");
        anyhow::ensure!(!resolution_id.is_nil(), "replay resolution ID is required");
        anyhow::ensure!(!actor_id.is_nil(), "replay reviewer ID is required");
        anyhow::ensure!(
            (20..=1024).contains(&reason.chars().count())
                && reason.len() <= 1024
                && !reason.chars().any(char::is_control),
            "invalid moderation replay reason"
        );
        anyhow::ensure!(
            (3..=256).contains(&evidence_reference.len())
                && evidence_reference.is_ascii()
                && evidence_reference
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._:/#-".contains(&byte))
                && evidence_reference
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric),
            "invalid moderation replay evidence reference"
        );

        let mut tx = self.pool.begin().await?;
        sqlx::query("select set_config('lock_timeout', '500ms', true), set_config('statement_timeout', '2s', true)")
            .execute(&mut *tx)
            .await?;
        sqlx::query("select pg_advisory_xact_lock(hashtextextended($1::text, 0))")
            .bind(resolution_id)
            .execute(&mut *tx)
            .await?;
        if let Some(existing) = sqlx::query(
            r#"
            select report_id, actor_id, reason, evidence_reference
            from moderation_content_decision_replays
            where resolution_id = $1
            "#,
        )
        .bind(resolution_id)
        .fetch_optional(&mut *tx)
        .await?
        {
            let matches_request = existing.get::<Uuid, _>("report_id") == report_id
                && existing.get::<Uuid, _>("actor_id") == actor_id
                && existing.get::<String, _>("reason") == reason
                && existing.get::<String, _>("evidence_reference") == evidence_reference;
            if !matches_request {
                return Err(ContentDecisionReplayConflict.into());
            }
            tx.commit().await?;
            return Ok(false);
        }

        let report_status: Option<String> =
            sqlx::query_scalar("select status from moderation_reports where id = $1 for update")
                .bind(report_id)
                .fetch_optional(&mut *tx)
                .await?;
        if report_status.as_deref() != Some("OPEN") {
            return Err(ContentDecisionReplayConflict.into());
        }
        let outbox = sqlx::query(
            r#"
            select state, attempt_count, last_error_code
            from moderation_content_decision_outbox
            where report_id = $1
            for update
            "#,
        )
        .bind(report_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| anyhow::Error::new(ContentDecisionReplayConflict))?;
        let state: String = outbox.get("state");
        let attempt_count: i32 = outbox.get("attempt_count");
        let last_error_code: Option<String> = outbox.get("last_error_code");
        if state != "RETRY_EXHAUSTED"
            || attempt_count < i32::try_from(MAX_CONTENT_DECISION_ATTEMPTS).unwrap_or(i32::MAX)
            || last_error_code.is_none()
        {
            return Err(ContentDecisionReplayConflict.into());
        }

        sqlx::query(
            r#"
            insert into moderation_content_decision_replays (
                resolution_id, report_id, actor_id, attempt_count,
                last_error_code, reason, evidence_reference
            ) values ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(resolution_id)
        .bind(report_id)
        .bind(actor_id)
        .bind(attempt_count)
        .bind(last_error_code)
        .bind(reason)
        .bind(evidence_reference)
        .execute(&mut *tx)
        .await?;
        let updated = sqlx::query(
            r#"
            update moderation_content_decision_outbox
            set state = 'PENDING', attempt_count = 0, replay_count = replay_count + 1,
                next_attempt_at = now(), lease_owner = null, lease_expires_at = null,
                last_error_code = null, updated_at = now()
            where report_id = $1 and state = 'RETRY_EXHAUSTED'
            "#,
        )
        .bind(report_id)
        .execute(&mut *tx)
        .await?;
        anyhow::ensure!(
            updated.rows_affected() == 1,
            "moderation content decision changed during replay"
        );
        tx.commit().await?;
        Ok(true)
    }

    async fn mark_content_target_unavailable_inner(
        &self,
        pending: &PendingContentModerationDecision,
    ) -> Result<bool> {
        let updated = sqlx::query(
            r#"
            update moderation_content_decision_outbox
            set state = 'TARGET_UNAVAILABLE', lease_owner = null,
                lease_expires_at = null, last_error_code = 'TARGET_UNAVAILABLE',
                updated_at = now()
            where report_id = $1 and state = 'PENDING' and lease_owner = $2
            "#,
        )
        .bind(pending.report.id)
        .bind(pending.lease_owner)
        .execute(&self.pool)
        .await?;
        Ok(updated.rows_affected() == 1)
    }

    async fn is_user_restricted_inner(&self, user_id: Uuid, now: DateTime<Utc>) -> Result<bool> {
        let restricted = sqlx::query_scalar::<_, bool>(
            r#"
            select exists (
                select 1
                from user_restrictions
                where user_id = $1
                  and lifted_at is null
                  and (expires_at is null or expires_at > $2)
            )
            "#,
        )
        .bind(user_id)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;
        Ok(restricted)
    }
}

#[async_trait]
impl ModerationRepositoryPort for PgModerationRepository {
    async fn create_report(
        &self,
        report: ModerationReport,
    ) -> crate::application::moderation_repository_error::ModerationRepositoryResult<ModerationReport>
    {
        self.create_report_inner(report)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn create_report_with_mutation(
        &self,
        report: ModerationReport,
        context: Option<crate::application::dto::moderation_dto::ModerationMutationContextDto>,
    ) -> crate::application::moderation_repository_error::ModerationRepositoryResult<ModerationReport>
    {
        self.create_report_with_mutation_inner(report, context)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn find_report(
        &self,
        report_id: Uuid,
    ) -> crate::application::moderation_repository_error::ModerationRepositoryResult<
        Option<ModerationReport>,
    > {
        self.find_report_inner(report_id)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn list_by_reporter(
        &self,
        reporter_id: Uuid,
        limit: i32,
    ) -> crate::application::moderation_repository_error::ModerationRepositoryResult<
        Vec<ModerationReport>,
    > {
        self.list_by_reporter_inner(reporter_id, limit)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn list_open(
        &self,
        limit: i32,
    ) -> crate::application::moderation_repository_error::ModerationRepositoryResult<
        Vec<ModerationReport>,
    > {
        self.list_open_inner(limit)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn content_decision_backlog(
        &self,
    ) -> crate::application::moderation_repository_error::ModerationRepositoryResult<
        ContentModerationDecisionBacklog,
    > {
        self.content_decision_backlog_inner()
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn list_content_decisions(
        &self,
        limit: i32,
        after: Option<ContentModerationDecisionCursor>,
    ) -> crate::application::moderation_repository_error::ModerationRepositoryResult<
        Vec<ContentModerationDecisionOutboxEntry>,
    > {
        self.list_content_decisions_inner(limit, after)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn record_decision(
        &self,
        report_id: Uuid,
        actor_id: Uuid,
        decision_type: ModerationDecisionType,
        note: &str,
        restriction_expires_at: Option<DateTime<Utc>>,
    ) -> crate::application::moderation_repository_error::ModerationRepositoryResult<(
        ModerationReport,
        ModerationDecision,
    )> {
        self.record_decision_inner(
            report_id,
            actor_id,
            decision_type,
            note,
            restriction_expires_at,
        )
        .await
        .map_err(classify_moderation_repository_error)
    }
    async fn begin_content_decision(
        &self,
        report_id: Uuid,
        actor_id: Uuid,
        decision_type: ModerationDecisionType,
        note: &str,
        lease_owner: Uuid,
        lease_for: Duration,
    ) -> crate::application::moderation_repository_error::ModerationRepositoryResult<
        ContentModerationDecisionStage,
    > {
        self.begin_content_decision_inner(
            report_id,
            actor_id,
            decision_type,
            note,
            lease_owner,
            lease_for,
        )
        .await
        .map_err(classify_moderation_repository_error)
    }
    async fn claim_next_content_decision(
        &self,
        lease_owner: Uuid,
        lease_for: Duration,
    ) -> crate::application::moderation_repository_error::ModerationRepositoryResult<
        Option<PendingContentModerationDecision>,
    > {
        self.claim_next_content_decision_inner(lease_owner, lease_for)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn complete_content_decision(
        &self,
        pending: &PendingContentModerationDecision,
    ) -> crate::application::moderation_repository_error::ModerationRepositoryResult<(
        ModerationReport,
        ModerationDecision,
    )> {
        self.complete_content_decision_inner(pending)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn retry_content_decision(
        &self,
        pending: &PendingContentModerationDecision,
        retry_after: Duration,
        error_code: &str,
    ) -> crate::application::moderation_repository_error::ModerationRepositoryResult<bool> {
        self.retry_content_decision_inner(pending, retry_after, error_code)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn dead_letter_content_decision(
        &self,
        pending: &PendingContentModerationDecision,
        error_code: &str,
    ) -> crate::application::moderation_repository_error::ModerationRepositoryResult<bool> {
        self.dead_letter_content_decision_inner(pending, error_code)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn replay_dead_lettered_content_decision(
        &self,
        report_id: Uuid,
        resolution_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        evidence_reference: &str,
    ) -> crate::application::moderation_repository_error::ModerationRepositoryResult<bool> {
        self.replay_dead_lettered_content_decision_inner(
            report_id,
            resolution_id,
            actor_id,
            reason,
            evidence_reference,
        )
        .await
        .map_err(classify_moderation_repository_error)
    }
    async fn mark_content_target_unavailable(
        &self,
        pending: &PendingContentModerationDecision,
    ) -> crate::application::moderation_repository_error::ModerationRepositoryResult<bool> {
        self.mark_content_target_unavailable_inner(pending)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn is_user_restricted(
        &self,
        user_id: Uuid,
        now: DateTime<Utc>,
    ) -> crate::application::moderation_repository_error::ModerationRepositoryResult<bool> {
        self.is_user_restricted_inner(user_id, now)
            .await
            .map_err(classify_moderation_repository_error)
    }
}

fn duration_seconds(duration: Duration) -> i32 {
    duration.as_secs().clamp(1, i32::MAX as u64) as i32
}

fn map_pending_content_decision(
    row: sqlx::postgres::PgRow,
    report: ModerationReport,
) -> Result<PendingContentModerationDecision> {
    let attempt_count: i32 = row.get("attempt_count");
    anyhow::ensure!(attempt_count >= 0, "invalid moderation attempt count");
    let lease_owner: Option<Uuid> = row.get("lease_owner");
    Ok(PendingContentModerationDecision {
        report,
        actor_id: row.get("actor_id"),
        decision_type: ModerationDecisionType::parse(
            row.get::<String, _>("decision_type").as_str(),
        )?,
        note: row.get("note"),
        attempt_count: attempt_count as u32,
        lease_owner: lease_owner
            .ok_or_else(|| anyhow::anyhow!("moderation decision lease owner is missing"))?,
    })
}

async fn load_existing_decision(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    report_id: Uuid,
    decision_type: ModerationDecisionType,
) -> Result<Option<(ModerationReport, ModerationDecision)>> {
    let report = sqlx::query(
        r#"
        select id, reporter_id, target_type, target_id, category, reason,
            status, created_at, resolved_at
        from moderation_reports
        where id = $1
        for update
        "#,
    )
    .bind(report_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(report) = report else {
        return Ok(None);
    };
    let decision = sqlx::query(
        r#"
        select id, report_id, actor_id, decision_type, note, created_at
        from moderation_decisions
        where report_id = $1 and decision_type = $2
        "#,
    )
    .bind(report_id)
    .bind(decision_type.as_str())
    .fetch_optional(&mut **tx)
    .await?;
    let Some(decision) = decision else {
        return Ok(None);
    };
    Ok(Some((map_report(report)?, map_decision(decision)?)))
}

async fn finish_content_decision(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pending: &PendingContentModerationDecision,
) -> Result<(ModerationReport, ModerationDecision)> {
    let report_row = sqlx::query(
        r#"
        select id, reporter_id, target_type, target_id, category, reason,
            status, created_at, resolved_at
        from moderation_reports
        where id = $1
        for update
        "#,
    )
    .bind(pending.report.id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ModerationRepositoryError::ReportMissing)?;
    let mut report = map_report(report_row)?;

    if let Some(existing) = sqlx::query(
        r#"
        select id, report_id, actor_id, decision_type, note, created_at
        from moderation_decisions
        where report_id = $1
        "#,
    )
    .bind(pending.report.id)
    .fetch_optional(&mut **tx)
    .await?
    {
        let decision = map_decision(existing)?;
        anyhow::ensure!(
            decision.decision_type == pending.decision_type,
            "moderation report already has a different decision"
        );
        return Ok((report, decision));
    }
    anyhow::ensure!(
        report.status == ReportStatus::Open,
        "moderation report is not open"
    );

    let decision_row = sqlx::query(
        r#"
        insert into moderation_decisions (
            id, report_id, actor_id, decision_type, note, created_at
        )
        values ($1, $2, $3, $4, $5, now())
        returning id, report_id, actor_id, decision_type, note, created_at
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(pending.report.id)
    .bind(pending.actor_id)
    .bind(pending.decision_type.as_str())
    .bind(&pending.note)
    .fetch_one(&mut **tx)
    .await?;
    let decision = map_decision(decision_row)?;
    let resolved_at = Utc::now();
    sqlx::query(
        "update moderation_reports set status = 'RESOLVED', resolved_at = $2 where id = $1",
    )
    .bind(pending.report.id)
    .bind(resolved_at)
    .execute(&mut **tx)
    .await?;
    report.status = ReportStatus::Resolved;
    report.resolved_at = Some(resolved_at);
    Ok((report, decision))
}

async fn lookup_report_receipt(
    pool: &PgPool,
    context: &crate::application::dto::moderation_dto::ModerationMutationContextDto,
) -> Result<ModerationReport> {
    let row = sqlx::query(
        r#"
        select actor_id, scope, idempotency_key, state, outcome_json,
               replay_until > now() as replayable
        from moderation_report_operation_receipts
        where operation_id = $1
        "#,
    )
    .bind(context.operation_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ReportMutationReceiptError::Missing)?;
    if row.get::<Uuid, _>("actor_id") != context.actor_id
        || row.get::<String, _>("scope") != CREATE_REPORT_OWNER_SCOPE
        || row.get::<Option<String>, _>("idempotency_key") != context.idempotency_key
    {
        return Err(ReportMutationReceiptError::Conflict.into());
    }
    if row.get::<String, _>("state") != "REPLAYABLE" || !row.get::<bool, _>("replayable") {
        return Err(ReportMutationReceiptError::Expired.into());
    }
    let outcome: serde_json::Value = row
        .get::<Option<serde_json::Value>, _>("outcome_json")
        .ok_or(ReportMutationReceiptError::Expired)?;
    let outcome: ReportReceiptOutcome =
        serde_json::from_value(outcome).map_err(|_| ReportMutationReceiptError::Conflict)?;
    if outcome.version != 1 || outcome.report.reporter_id != context.actor_id {
        return Err(ReportMutationReceiptError::Conflict.into());
    }
    Ok(outcome.report.into())
}

fn map_report(row: sqlx::postgres::PgRow) -> Result<ModerationReport> {
    Ok(ModerationReport {
        id: row.get("id"),
        reporter_id: row.get("reporter_id"),
        target_type: ReportTargetType::parse(row.get::<String, _>("target_type").as_str())?,
        target_id: row.get("target_id"),
        category: ReportCategory::parse(row.get::<String, _>("category").as_str())?,
        reason: row.get("reason"),
        status: ReportStatus::parse(row.get::<String, _>("status").as_str())?,
        created_at: row.get("created_at"),
        resolved_at: row.get("resolved_at"),
    })
}

fn map_decision(row: sqlx::postgres::PgRow) -> Result<ModerationDecision> {
    Ok(ModerationDecision {
        id: row.get("id"),
        report_id: row.get("report_id"),
        actor_id: row.get("actor_id"),
        decision_type: ModerationDecisionType::parse(
            row.get::<String, _>("decision_type").as_str(),
        )?,
        note: row.get("note"),
        created_at: row.get("created_at"),
    })
}
