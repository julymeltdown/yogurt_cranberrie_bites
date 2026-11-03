use anyhow::Result;
use api_gateway::adapters::out::repository::moderation_repository::classify_moderation_repository_error;
use api_gateway::application::dto::moderation_repository_dto::{
    ContentModerationDecisionCursorDto as ContentModerationDecisionCursor,
    ContentModerationDecisionOutboxEntryDto as ContentModerationDecisionOutboxEntry,
    ContentModerationDecisionStageDto as ContentModerationDecisionStage,
    ModerationDecisionDto as ModerationDecision,
    ModerationDecisionTypeDto as ModerationDecisionType, ModerationReportDto as ModerationReport,
    PendingContentModerationDecisionDto as PendingContentModerationDecision,
    ReportCategoryDto as ReportCategory, ReportStatusDto as ReportStatus,
    ReportTargetTypeDto as ReportTargetType,
};
use api_gateway::application::ports::ModerationRepositoryPort;
use api_gateway::domain::moderation::{
    ModerationDecisionConflict, ModerationDecisionInProgress, ModerationTargetUnavailable,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentDecisionReplayCall {
    pub report_id: Uuid,
    pub resolution_id: Uuid,
    pub actor_id: Uuid,
    pub reason: String,
    pub evidence_reference: String,
}

#[derive(Default)]
pub struct TestModerationRepository {
    reports: Mutex<HashMap<Uuid, ModerationReport>>,
    report_keys: Mutex<HashMap<(Uuid, ReportTargetType, Uuid, ReportCategory), Uuid>>,
    restrictions: Mutex<HashSet<Uuid>>,
    decisions: Mutex<HashMap<(Uuid, ModerationDecisionType), ModerationDecision>>,
    pending_content_decisions: Mutex<HashMap<Uuid, PendingContentModerationDecision>>,
    unavailable_content_decisions: Mutex<HashSet<Uuid>>,
    content_decisions: Mutex<Vec<ContentModerationDecisionOutboxEntry>>,
    pub replay_calls: Mutex<Vec<ContentDecisionReplayCall>>,
}

impl TestModerationRepository {
    pub fn insert_report(
        &self,
        reporter_id: Uuid,
        target_type: ReportTargetType,
        target_id: Uuid,
    ) -> ModerationReport {
        let report = ModerationReport {
            id: Uuid::new_v4(),
            reporter_id,
            target_type,
            target_id,
            category: ReportCategory::Spam,
            reason: "seed".into(),
            status: ReportStatus::Open,
            created_at: Utc::now(),
            resolved_at: None,
        };
        self.reports
            .lock()
            .unwrap()
            .insert(report.id, report.clone());
        report
    }

    pub fn restrict(&self, user_id: Uuid) {
        self.restrictions.lock().unwrap().insert(user_id);
    }

    pub fn set_content_decisions(&self, decisions: Vec<ContentModerationDecisionOutboxEntry>) {
        *self.content_decisions.lock().unwrap() = decisions;
    }

    pub fn mark_target_unavailable(&self, report_id: Uuid) {
        self.unavailable_content_decisions
            .lock()
            .unwrap()
            .insert(report_id);
    }
}

impl TestModerationRepository {
    async fn create_report_inner(&self, report: ModerationReport) -> Result<ModerationReport> {
        let key = (
            report.reporter_id,
            report.target_type,
            report.target_id,
            report.category,
        );
        if let Some(id) = self.report_keys.lock().unwrap().get(&key).copied() {
            return Ok(self.reports.lock().unwrap().get(&id).unwrap().clone());
        }
        self.report_keys.lock().unwrap().insert(key, report.id);
        self.reports
            .lock()
            .unwrap()
            .insert(report.id, report.clone());
        Ok(report)
    }

    async fn find_report_inner(&self, report_id: Uuid) -> Result<Option<ModerationReport>> {
        Ok(self.reports.lock().unwrap().get(&report_id).cloned())
    }

    async fn list_by_reporter_inner(
        &self,
        reporter_id: Uuid,
        limit: i32,
    ) -> Result<Vec<ModerationReport>> {
        Ok(self
            .reports
            .lock()
            .unwrap()
            .values()
            .filter(|report| report.reporter_id == reporter_id)
            .take(limit as usize)
            .cloned()
            .collect())
    }

    async fn list_open_inner(&self, limit: i32) -> Result<Vec<ModerationReport>> {
        Ok(self
            .reports
            .lock()
            .unwrap()
            .values()
            .filter(|report| report.status == ReportStatus::Open)
            .take(limit as usize)
            .cloned()
            .collect())
    }

    async fn list_content_decisions_inner(
        &self,
        limit: i32,
        after: Option<ContentModerationDecisionCursor>,
    ) -> Result<Vec<ContentModerationDecisionOutboxEntry>> {
        let mut decisions = self.content_decisions.lock().unwrap().clone();
        decisions.sort_by_key(|decision| (decision.created_at, decision.report_id));
        if let Some(after) = after {
            decisions.retain(|decision| {
                (decision.created_at, decision.report_id) > (after.created_at, after.report_id)
            });
        }
        decisions.truncate(limit.max(0) as usize);
        Ok(decisions)
    }

    async fn replay_dead_lettered_content_decision_inner(
        &self,
        report_id: Uuid,
        resolution_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        evidence_reference: &str,
    ) -> Result<bool> {
        self.replay_calls
            .lock()
            .unwrap()
            .push(ContentDecisionReplayCall {
                report_id,
                resolution_id,
                actor_id,
                reason: reason.to_owned(),
                evidence_reference: evidence_reference.to_owned(),
            });
        Ok(true)
    }

    async fn record_decision_inner(
        &self,
        report_id: Uuid,
        actor_id: Uuid,
        decision_type: ModerationDecisionType,
        note: &str,
        _restriction_expires_at: Option<DateTime<Utc>>,
    ) -> Result<(ModerationReport, ModerationDecision)> {
        if self
            .unavailable_content_decisions
            .lock()
            .unwrap()
            .contains(&report_id)
        {
            if decision_type != ModerationDecisionType::Dismiss {
                return Err(ModerationDecisionConflict.into());
            }
            if note.trim().is_empty() {
                return Err(
                    api_gateway::domain::moderation::ModerationResolutionNoteRequired.into(),
                );
            }
        }
        if let Some(decision) = self
            .decisions
            .lock()
            .unwrap()
            .get(&(report_id, decision_type))
            .cloned()
        {
            let report = self
                .reports
                .lock()
                .unwrap()
                .get(&report_id)
                .cloned()
                .ok_or(api_gateway::application::moderation_repository_error::ModerationRepositoryError::ReportMissing)?;
            return Ok((report, decision));
        }
        let mut reports = self.reports.lock().unwrap();
        let report = reports
            .get_mut(&report_id)
            .ok_or(api_gateway::application::moderation_repository_error::ModerationRepositoryError::ReportMissing)?;
        report.status = if decision_type == ModerationDecisionType::Dismiss {
            ReportStatus::Dismissed
        } else {
            ReportStatus::Resolved
        };
        report.resolved_at = Some(Utc::now());
        if decision_type == ModerationDecisionType::RestrictUser {
            self.restrictions.lock().unwrap().insert(report.target_id);
        } else if decision_type == ModerationDecisionType::LiftRestriction {
            self.restrictions.lock().unwrap().remove(&report.target_id);
        }
        let decision = ModerationDecision {
            id: Uuid::new_v4(),
            report_id,
            actor_id,
            decision_type,
            note: note.to_owned(),
            created_at: Utc::now(),
        };
        self.decisions
            .lock()
            .unwrap()
            .insert((report_id, decision_type), decision.clone());
        if decision_type == ModerationDecisionType::Dismiss {
            self.unavailable_content_decisions
                .lock()
                .unwrap()
                .remove(&report_id);
        }
        Ok((report.clone(), decision))
    }

    async fn begin_content_decision_inner(
        &self,
        report_id: Uuid,
        actor_id: Uuid,
        decision_type: ModerationDecisionType,
        note: &str,
        lease_owner: Uuid,
        _lease_for: Duration,
    ) -> Result<ContentModerationDecisionStage> {
        if self
            .unavailable_content_decisions
            .lock()
            .unwrap()
            .contains(&report_id)
        {
            return Err(ModerationTargetUnavailable.into());
        }
        if let Some(decision) = self
            .decisions
            .lock()
            .unwrap()
            .get(&(report_id, decision_type))
            .cloned()
        {
            let report = self
                .reports
                .lock()
                .unwrap()
                .get(&report_id)
                .cloned()
                .ok_or(api_gateway::application::moderation_repository_error::ModerationRepositoryError::ReportMissing)?;
            return Ok(ContentModerationDecisionStage::AlreadyRecorded { report, decision });
        }
        if let Some(staged) = self
            .pending_content_decisions
            .lock()
            .unwrap()
            .get(&report_id)
            .cloned()
        {
            if staged.actor_id != actor_id
                || staged.decision_type != decision_type
                || staged.note != note
            {
                return Err(ModerationDecisionConflict.into());
            }
            if !staged.lease_owner.is_nil() && staged.lease_owner != lease_owner {
                return Err(ModerationDecisionInProgress.into());
            }
        }
        let report = self
            .reports
            .lock()
            .unwrap()
            .get(&report_id)
            .cloned()
            .ok_or(api_gateway::application::moderation_repository_error::ModerationRepositoryError::ReportMissing)?;
        let pending = PendingContentModerationDecision {
            report,
            actor_id,
            decision_type,
            note: note.to_owned(),
            attempt_count: 1,
            lease_owner,
        };
        self.pending_content_decisions
            .lock()
            .unwrap()
            .insert(report_id, pending.clone());
        Ok(ContentModerationDecisionStage::Pending(pending))
    }

    async fn claim_next_content_decision_inner(
        &self,
        _lease_owner: Uuid,
        _lease_for: Duration,
    ) -> Result<Option<PendingContentModerationDecision>> {
        Ok(None)
    }

    async fn complete_content_decision_inner(
        &self,
        pending: &PendingContentModerationDecision,
    ) -> Result<(ModerationReport, ModerationDecision)> {
        let result = self
            .record_decision(
                pending.report.id,
                pending.actor_id,
                pending.decision_type,
                &pending.note,
                None,
            )
            .await?;
        self.pending_content_decisions
            .lock()
            .unwrap()
            .remove(&pending.report.id);
        Ok(result)
    }

    async fn retry_content_decision_inner(
        &self,
        pending: &PendingContentModerationDecision,
        _retry_after: Duration,
        _error_code: &str,
    ) -> Result<bool> {
        if let Some(staged) = self
            .pending_content_decisions
            .lock()
            .unwrap()
            .get_mut(&pending.report.id)
        {
            staged.lease_owner = Uuid::nil();
            return Ok(true);
        }
        Ok(false)
    }

    async fn mark_content_target_unavailable_inner(
        &self,
        pending: &PendingContentModerationDecision,
    ) -> Result<bool> {
        let removed = self
            .pending_content_decisions
            .lock()
            .unwrap()
            .remove(&pending.report.id)
            .is_some();
        if removed {
            self.unavailable_content_decisions
                .lock()
                .unwrap()
                .insert(pending.report.id);
        }
        Ok(removed)
    }

    async fn is_user_restricted_inner(&self, user_id: Uuid, _now: DateTime<Utc>) -> Result<bool> {
        Ok(self.restrictions.lock().unwrap().contains(&user_id))
    }

    // Explicit recording behavior formerly inherited from the port.
    async fn create_report_with_mutation_inner(
        &self,
        report: api_gateway::application::dto::moderation_repository_dto::ModerationReportDto,
        context: Option<
            api_gateway::application::dto::moderation_dto::ModerationMutationContextDto,
        >,
    ) -> anyhow::Result<api_gateway::application::dto::moderation_repository_dto::ModerationReportDto>
    {
        if context.is_some() {
            anyhow::bail!("moderation report mutation receipts are unavailable");
        }
        self.create_report_inner(report).await
    }

    async fn content_decision_backlog_inner(
        &self,
    ) -> anyhow::Result<api_gateway::application::dto::moderation_repository_dto::ContentModerationDecisionBacklogDto>{
        anyhow::bail!("durable content moderation decisions are unavailable")
    }

    async fn dead_letter_content_decision_inner(
        &self,
        _pending: &api_gateway::application::dto::moderation_repository_dto::PendingContentModerationDecisionDto,
        _error_code: &str,
    ) -> anyhow::Result<bool> {
        anyhow::bail!("durable content moderation decisions are unavailable")
    }
}

#[async_trait]
impl ModerationRepositoryPort for TestModerationRepository {
    async fn create_report(
        &self,
        report: ModerationReport,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<
        ModerationReport,
    > {
        self.create_report_inner(report)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn find_report(
        &self,
        report_id: Uuid,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<
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
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<
        Vec<ModerationReport>,
    > {
        self.list_by_reporter_inner(reporter_id, limit)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn list_open(
        &self,
        limit: i32,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<
        Vec<ModerationReport>,
    > {
        self.list_open_inner(limit)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn list_content_decisions(
        &self,
        limit: i32,
        after: Option<ContentModerationDecisionCursor>,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<
        Vec<ContentModerationDecisionOutboxEntry>,
    > {
        self.list_content_decisions_inner(limit, after)
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
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<bool>
    {
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
    async fn record_decision(
        &self,
        report_id: Uuid,
        actor_id: Uuid,
        decision_type: ModerationDecisionType,
        note: &str,
        _restriction_expires_at: Option<DateTime<Utc>>,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<(
        ModerationReport,
        ModerationDecision,
    )> {
        self.record_decision_inner(
            report_id,
            actor_id,
            decision_type,
            note,
            _restriction_expires_at,
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
        _lease_for: Duration,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<
        ContentModerationDecisionStage,
    > {
        self.begin_content_decision_inner(
            report_id,
            actor_id,
            decision_type,
            note,
            lease_owner,
            _lease_for,
        )
        .await
        .map_err(classify_moderation_repository_error)
    }
    async fn claim_next_content_decision(
        &self,
        _lease_owner: Uuid,
        _lease_for: Duration,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<
        Option<PendingContentModerationDecision>,
    > {
        self.claim_next_content_decision_inner(_lease_owner, _lease_for)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn complete_content_decision(
        &self,
        pending: &PendingContentModerationDecision,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<(
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
        _retry_after: Duration,
        _error_code: &str,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<bool>
    {
        self.retry_content_decision_inner(pending, _retry_after, _error_code)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn mark_content_target_unavailable(
        &self,
        pending: &PendingContentModerationDecision,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<bool>
    {
        self.mark_content_target_unavailable_inner(pending)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn is_user_restricted(
        &self,
        user_id: Uuid,
        _now: DateTime<Utc>,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<bool>
    {
        self.is_user_restricted_inner(user_id, _now)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn create_report_with_mutation(
        &self,
        report: api_gateway::application::dto::moderation_repository_dto::ModerationReportDto,
        context: Option<
            api_gateway::application::dto::moderation_dto::ModerationMutationContextDto,
        >,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<
        api_gateway::application::dto::moderation_repository_dto::ModerationReportDto,
    > {
        self.create_report_with_mutation_inner(report, context)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn content_decision_backlog(
        &self,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<api_gateway::application::dto::moderation_repository_dto::ContentModerationDecisionBacklogDto>{
        self.content_decision_backlog_inner()
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn dead_letter_content_decision(
        &self,
        _pending: &api_gateway::application::dto::moderation_repository_dto::PendingContentModerationDecisionDto,
        _error_code: &str,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<bool>
    {
        self.dead_letter_content_decision_inner(_pending, _error_code)
            .await
            .map_err(classify_moderation_repository_error)
    }
}
