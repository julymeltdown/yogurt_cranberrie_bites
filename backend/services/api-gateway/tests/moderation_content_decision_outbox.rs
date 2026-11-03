use api_gateway::application::moderation_repository_error::ModerationRepositoryError;
#[path = "support/mod.rs"]
mod support;

use api_gateway::adapters::out::repository::moderation_repository::PgModerationRepository;
use api_gateway::application::dto::moderation_repository_dto::{
    ContentModerationDecisionCursorDto as ContentModerationDecisionCursor,
    ContentModerationDecisionOutboxStateDto as ContentModerationDecisionOutboxState,
    ContentModerationDecisionStageDto as ContentModerationDecisionStage,
    ModerationDecisionTypeDto as ModerationDecisionType, ModerationReportDto as ModerationReport,
    PendingContentModerationDecisionDto as PendingContentModerationDecision,
    ReportCategoryDto as ReportCategory, ReportStatusDto as ReportStatus,
    ReportTargetTypeDto as ReportTargetType,
};
use api_gateway::application::ports::ModerationRepositoryPort;
use api_gateway::domain::moderation::MAX_CONTENT_DECISION_ATTEMPTS;
use chrono::Utc;
use std::time::Duration;
use uuid::Uuid;

#[tokio::test]
async fn content_decision_outbox_reclaims_retries_and_finalizes_once() -> anyhow::Result<()> {
    let app = support::start_gateway_app(false).await?;
    let result = async {
        let repository = PgModerationRepository::new(app.pool.clone());
        let report = ModerationReport {
            id: Uuid::new_v4(),
            reporter_id: Uuid::new_v4(),
            target_type: ReportTargetType::Post,
            target_id: Uuid::new_v4(),
            category: ReportCategory::Spam,
            reason: "Durable moderation outbox acceptance".into(),
            status: ReportStatus::Open,
            created_at: Utc::now(),
            resolved_at: None,
        };
        repository.create_report(report.clone()).await?;

        let actor_id = Uuid::new_v4();
        let decision_type = ModerationDecisionType::HideContent;
        let lease_owner = Uuid::new_v4();
        let pending = match repository
            .begin_content_decision(
                report.id,
                actor_id,
                decision_type,
                "Confirmed spam",
                lease_owner,
                Duration::from_secs(30),
            )
            .await?
        {
            ContentModerationDecisionStage::Pending(pending) => pending,
            ContentModerationDecisionStage::AlreadyRecorded { .. } => {
                anyhow::bail!("new report already had a moderation decision")
            }
        };
        assert_eq!(pending.attempt_count, 1);
        sqlx::query(
            "update moderation_content_decision_outbox set created_at = now() - interval '5 minutes' where report_id = $1",
        )
        .bind(report.id)
        .execute(&app.pool)
        .await?;
        let backlog = repository.content_decision_backlog().await?;
        assert_eq!(backlog.pending, 1);
        assert!(backlog.oldest_pending_age_seconds >= 299);
        assert_eq!(backlog.target_unavailable, 0);
        let page = repository.list_content_decisions(10, None).await?;
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].report_id, report.id);
        assert_eq!(page[0].state, ContentModerationDecisionOutboxState::Pending);

        let duplicate = repository
            .begin_content_decision(
                report.id,
                actor_id,
                decision_type,
                "Confirmed spam",
                Uuid::new_v4(),
                Duration::from_secs(30),
            )
            .await
            .expect_err("another request cannot steal an active lease");
        assert_eq!(duplicate, ModerationRepositoryError::DecisionInProgress);
        assert!(repository
            .claim_next_content_decision(Uuid::new_v4(), Duration::from_secs(30))
            .await?
            .is_none());

        assert!(repository
            .retry_content_decision(&pending, Duration::from_secs(1), "POST_SERVICE_UNAVAILABLE")
            .await?);
        sqlx::query(
            "update moderation_content_decision_outbox set next_attempt_at = now() - interval '1 second' where report_id = $1",
        )
        .bind(report.id)
        .execute(&app.pool)
        .await?;

        let owner_a = Uuid::new_v4();
        let owner_b = Uuid::new_v4();
        let (claim_a, claim_b) = tokio::join!(
            repository.claim_next_content_decision(owner_a, Duration::from_secs(30)),
            repository.claim_next_content_decision(owner_b, Duration::from_secs(30)),
        );
        let claim_a = claim_a?;
        let claim_b = claim_b?;
        assert_ne!(claim_a.is_some(), claim_b.is_some(), "exactly one worker claims the row");
        let claimed: PendingContentModerationDecision = claim_a.or(claim_b).unwrap();
        assert_eq!(claimed.report.id, report.id);
        assert_eq!(claimed.attempt_count, 2);

        let (resolved_report, decision) = repository.complete_content_decision(&claimed).await?;
        assert_eq!(resolved_report.status, ReportStatus::Resolved);
        assert_eq!(decision.report_id, report.id);
        assert_eq!(decision.decision_type, decision_type);

        let repeated = repository.complete_content_decision(&claimed).await?;
        assert_eq!(repeated.1.id, decision.id, "completion replay is idempotent");
        let decision_count: i64 = sqlx::query_scalar(
            "select count(*) from moderation_decisions where report_id = $1",
        )
        .bind(report.id)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(decision_count, 1);
        let backlog = repository.content_decision_backlog().await?;
        assert_eq!(backlog.pending, 0, "completed work leaves the pending gauge");
        assert_eq!(backlog.target_unavailable, 0);
        assert!(repository.list_content_decisions(10, None).await?.is_empty());
        let state: String = sqlx::query_scalar(
            "select state from moderation_content_decision_outbox where report_id = $1",
        )
        .bind(report.id)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(state, "COMPLETED");

        let replay = repository
            .begin_content_decision(
                report.id,
                actor_id,
                decision_type,
                "Confirmed spam",
                Uuid::new_v4(),
                Duration::from_secs(30),
            )
            .await?;
        assert!(matches!(
            replay,
            ContentModerationDecisionStage::AlreadyRecorded { .. }
        ));

        // Expired leases recover work after a worker process exits without
        // explicitly releasing its claim.
        let crashed_report = ModerationReport {
            id: Uuid::new_v4(),
            target_id: Uuid::new_v4(),
            ..report.clone()
        };
        repository.create_report(crashed_report.clone()).await?;
        repository
            .begin_content_decision(
                crashed_report.id,
                actor_id,
                decision_type,
                "Worker restart recovery",
                Uuid::new_v4(),
                Duration::from_secs(30),
            )
            .await?;
        sqlx::query(
            "update moderation_content_decision_outbox set lease_expires_at = now() - interval '1 second', next_attempt_at = now() - interval '1 second' where report_id = $1",
        )
        .bind(crashed_report.id)
        .execute(&app.pool)
        .await?;
        let recovered = repository
            .claim_next_content_decision(Uuid::new_v4(), Duration::from_secs(30))
            .await?
            .expect("expired worker lease is reclaimed");
        assert_eq!(recovered.report.id, crashed_report.id);
        assert_eq!(recovered.attempt_count, 2);
        repository.complete_content_decision(&recovered).await?;

        let unavailable_report = ModerationReport {
            id: Uuid::new_v4(),
            target_id: Uuid::new_v4(),
            ..report
        };
        repository.create_report(unavailable_report.clone()).await?;
        let unavailable = match repository
            .begin_content_decision(
                unavailable_report.id,
                actor_id,
                decision_type,
                "Target was deleted",
                Uuid::new_v4(),
                Duration::from_secs(30),
            )
            .await?
        {
            ContentModerationDecisionStage::Pending(pending) => pending,
            ContentModerationDecisionStage::AlreadyRecorded { .. } => {
                anyhow::bail!("new report already had a moderation decision")
            }
        };
        assert!(repository
            .mark_content_target_unavailable(&unavailable)
            .await?);
        let backlog = repository.content_decision_backlog().await?;
        assert_eq!(backlog.pending, 0);
        assert_eq!(backlog.target_unavailable, 1);
        let page = repository.list_content_decisions(10, None).await?;
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].report_id, unavailable_report.id);
        assert_eq!(
            page[0].state,
            ContentModerationDecisionOutboxState::TargetUnavailable
        );
        let unavailable_error = repository
            .begin_content_decision(
                unavailable_report.id,
                actor_id,
                decision_type,
                "Target was deleted",
                Uuid::new_v4(),
                Duration::from_secs(30),
            )
            .await
            .expect_err("unavailable target is terminal for this decision");
        assert_eq!(unavailable_error, ModerationRepositoryError::TargetUnavailable);

        let second_unavailable_report = ModerationReport {
            id: Uuid::new_v4(),
            target_id: Uuid::new_v4(),
            ..unavailable_report
        };
        repository
            .create_report(second_unavailable_report.clone())
            .await?;
        let second_unavailable = match repository
            .begin_content_decision(
                second_unavailable_report.id,
                actor_id,
                decision_type,
                "Target was deleted",
                Uuid::new_v4(),
                Duration::from_secs(30),
            )
            .await?
        {
            ContentModerationDecisionStage::Pending(pending) => pending,
            ContentModerationDecisionStage::AlreadyRecorded { .. } => {
                anyhow::bail!("new report already had a moderation decision")
            }
        };
        repository
            .mark_content_target_unavailable(&second_unavailable)
            .await?;
        sqlx::query(
            "update moderation_content_decision_outbox set created_at = case when report_id = $1 then now() - interval '2 minutes' else now() - interval '1 minute' end where report_id in ($1, $2)",
        )
        .bind(unavailable_report.id)
        .bind(second_unavailable_report.id)
        .execute(&app.pool)
        .await?;
        let first_page = repository.list_content_decisions(1, None).await?;
        assert_eq!(first_page.len(), 1);
        assert_eq!(first_page[0].report_id, unavailable_report.id);
        let second_page = repository
            .list_content_decisions(
                1,
                Some(ContentModerationDecisionCursor {
                    created_at: first_page[0].created_at,
                    report_id: first_page[0].report_id,
                }),
            )
            .await?;
        assert_eq!(second_page.len(), 1);
        assert_eq!(second_page[0].report_id, second_unavailable_report.id);

        let missing_resolution_note = repository
            .record_decision(
                unavailable_report.id,
                actor_id,
                ModerationDecisionType::Dismiss,
                "  ",
                None,
            )
            .await
            .expect_err("unavailable targets require an audit note");
        assert_eq!(missing_resolution_note, ModerationRepositoryError::ResolutionNoteRequired);
        let state: String = sqlx::query_scalar(
            "select state from moderation_content_decision_outbox where report_id = $1",
        )
        .bind(unavailable_report.id)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(state, "TARGET_UNAVAILABLE", "rejected repair is atomic");

        let (dismissed_report, dismissal) = repository
            .record_decision(
                unavailable_report.id,
                actor_id,
                ModerationDecisionType::Dismiss,
                "Target no longer exists; closing the report",
                None,
            )
            .await?;
        assert_eq!(dismissed_report.status, ReportStatus::Dismissed);
        assert_eq!(dismissal.actor_id, actor_id);
        assert_eq!(dismissal.note, "Target no longer exists; closing the report");
        let state: String = sqlx::query_scalar(
            "select state from moderation_content_decision_outbox where report_id = $1",
        )
        .bind(unavailable_report.id)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(state, "RESOLVED_UNAVAILABLE");
        let replayed = repository
            .record_decision(
                unavailable_report.id,
                actor_id,
                ModerationDecisionType::Dismiss,
                "Target no longer exists; closing the report",
                None,
            )
            .await?;
        assert_eq!(replayed.1.id, dismissal.id);
        let backlog = repository.content_decision_backlog().await?;
        assert_eq!(backlog.target_unavailable, 1);
        let remaining = repository.list_content_decisions(10, None).await?;
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].report_id, second_unavailable_report.id);
        Ok::<(), anyhow::Error>(())
    }
    .await;
    app.shutdown().await?;
    result
}

#[tokio::test]
async fn exhausted_content_decision_requires_an_audited_idempotent_replay() -> anyhow::Result<()> {
    let app = support::start_gateway_app(false).await?;
    let result = async {
        let repository = PgModerationRepository::new(app.pool.clone());
        let report = ModerationReport {
            id: Uuid::new_v4(),
            reporter_id: Uuid::new_v4(),
            target_type: ReportTargetType::Post,
            target_id: Uuid::new_v4(),
            category: ReportCategory::Spam,
            reason: "Moderation retry exhaustion acceptance".into(),
            status: ReportStatus::Open,
            created_at: Utc::now(),
            resolved_at: None,
        };
        repository.create_report(report.clone()).await?;
        let actor_id = Uuid::new_v4();
        let decision_type = ModerationDecisionType::HideContent;
        let mut pending = match repository
            .begin_content_decision(
                report.id,
                actor_id,
                decision_type,
                "Confirmed spam",
                Uuid::new_v4(),
                Duration::from_secs(30),
            )
            .await?
        {
            ContentModerationDecisionStage::Pending(pending) => pending,
            ContentModerationDecisionStage::AlreadyRecorded { .. } => {
                anyhow::bail!("new report already had a moderation decision")
            }
        };
        pending.attempt_count = MAX_CONTENT_DECISION_ATTEMPTS;
        sqlx::query(
            "update moderation_content_decision_outbox set attempt_count = $2 where report_id = $1",
        )
        .bind(report.id)
        .bind(i32::try_from(MAX_CONTENT_DECISION_ATTEMPTS)?)
        .execute(&app.pool)
        .await?;
        assert!(repository
            .dead_letter_content_decision(&pending, "POST_SERVICE_CALL_FAILED")
            .await?);

        let backlog = repository.content_decision_backlog().await?;
        assert_eq!(backlog.pending, 0);
        assert_eq!(backlog.retry_exhausted, 1);
        assert_eq!(backlog.target_unavailable, 0);
        let entries = repository.list_content_decisions(10, None).await?;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].state, ContentModerationDecisionOutboxState::RetryExhausted);
        assert_eq!(entries[0].attempt_count, MAX_CONTENT_DECISION_ATTEMPTS);
        assert_eq!(entries[0].replay_count, 0);
        assert_eq!(
            entries[0].last_error_code.as_deref(),
            Some("POST_SERVICE_CALL_FAILED")
        );

        let ordinary_retry = repository
            .begin_content_decision(
                report.id,
                actor_id,
                decision_type,
                "Confirmed spam",
                Uuid::new_v4(),
                Duration::from_secs(30),
            )
            .await
            .expect_err("a new content decision cannot bypass dead-letter review");
        assert_eq!(ordinary_retry, ModerationRepositoryError::DecisionConflict);

        let resolution_id = Uuid::new_v4();
        let reason = "The downstream incident is resolved; replay is safe";
        let evidence_reference = "OPS-INC-2026-1042";
        assert!(repository
            .replay_dead_lettered_content_decision(
                report.id,
                resolution_id,
                actor_id,
                reason,
                evidence_reference,
            )
            .await?);
        assert!(!repository
            .replay_dead_lettered_content_decision(
                report.id,
                resolution_id,
                actor_id,
                reason,
                evidence_reference,
            )
            .await?);
        let conflicting_replay = repository
            .replay_dead_lettered_content_decision(
                report.id,
                resolution_id,
                actor_id,
                "A different justification for replaying this command",
                evidence_reference,
            )
            .await
            .expect_err("a resolution ID cannot be reused for a different command");
        assert_eq!(conflicting_replay, ModerationRepositoryError::ReplayConflict);
        let second_replay = repository
            .replay_dead_lettered_content_decision(
                report.id,
                Uuid::new_v4(),
                actor_id,
                reason,
                evidence_reference,
            )
            .await
            .expect_err("a second resolution cannot race an active replay");
        assert_eq!(second_replay, ModerationRepositoryError::ReplayConflict);

        let (state, attempt_count, replay_count, lease_owner):
            (String, i32, i32, Option<Uuid>) = sqlx::query_as(
            "select state, attempt_count, replay_count, lease_owner from moderation_content_decision_outbox where report_id = $1",
        )
        .bind(report.id)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(state, "PENDING");
        assert_eq!(attempt_count, 0, "an authorized replay starts a fresh attempt cycle");
        assert_eq!(replay_count, 1);
        assert_eq!(lease_owner, None);

        let (audit_actor, audit_attempts, audit_error, audit_reason, audit_reference): (
            Uuid,
            i32,
            String,
            String,
            String,
        ) = sqlx::query_as(
            "select actor_id, attempt_count, last_error_code, reason, evidence_reference from moderation_content_decision_replays where resolution_id = $1",
        )
        .bind(resolution_id)
        .fetch_one(&app.pool)
        .await?;
        assert_eq!(audit_actor, actor_id);
        assert_eq!(audit_attempts, i32::try_from(MAX_CONTENT_DECISION_ATTEMPTS)?);
        assert_eq!(audit_error, "POST_SERVICE_CALL_FAILED");
        assert_eq!(audit_reason, reason);
        assert_eq!(audit_reference, evidence_reference);

        let trigger_error = sqlx::query(
            "update moderation_content_decision_replays set reason = 'rewritten justification' where resolution_id = $1",
        )
        .bind(resolution_id)
        .execute(&app.pool)
        .await
        .expect_err("replay audit rows are append-only");
        assert!(trigger_error.to_string().contains("append-only"));

        let replayed = repository
            .claim_next_content_decision(Uuid::new_v4(), Duration::from_secs(30))
            .await?
            .expect("authorized replay is queued immediately");
        assert_eq!(replayed.report.id, report.id);
        assert_eq!(replayed.attempt_count, 1);
        assert_eq!(replayed.actor_id, actor_id);
        assert_eq!(replayed.note, "Confirmed spam");
        let (_, decision) = repository.complete_content_decision(&replayed).await?;
        assert_eq!(decision.decision_type, decision_type);
        assert_eq!(decision.actor_id, actor_id);
        let backlog = repository.content_decision_backlog().await?;
        assert_eq!(backlog.retry_exhausted, 0);
        assert_eq!(backlog.pending, 0);
        Ok::<(), anyhow::Error>(())
    }
    .await;
    app.shutdown().await?;
    result
}
