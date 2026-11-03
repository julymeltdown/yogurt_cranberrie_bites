use crate::dto::moderation_dto::{
    CreateModerationDecisionRequest, CreateReportRequest, ModerationDecisionResponse,
    ModerationMutationContextDto, ModerationReportListResponse, ModerationReportResponse,
    ReplayContentDecisionRequest,
};
use crate::moderation_input::ModerationInputError as Input;
use crate::moderation_repository_error::ModerationRepositoryError;
use crate::ports::{ModerationRepositoryPort, ModerationRuntimePort, PostClientPort};
use chrono::Utc;
use gateway_domain::moderation::{
    ContentModerationDecisionCursor, ContentModerationDecisionStage, ModerationDecisionType,
    ModerationReport, ModerationTargetUnavailable, ReportCategory, ReportStatus, ReportTargetType,
    MAX_CONTENT_DECISION_ATTEMPTS,
};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

const CONTENT_DECISION_LEASE: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct ModerationService {
    repository: Arc<dyn ModerationRepositoryPort>,
    post_client: Arc<dyn PostClientPort>,
    runtime: Arc<dyn ModerationRuntimePort>,
}

impl ModerationService {
    pub fn new(
        repository: Arc<dyn ModerationRepositoryPort>,
        post_client: Arc<dyn PostClientPort>,
        runtime: Arc<dyn ModerationRuntimePort>,
    ) -> Self {
        Self {
            repository,
            post_client,
            runtime,
        }
    }

    pub async fn content_decision_backlog(
        &self,
    ) -> crate::service_error::GatewayServiceResult<
        crate::dto::moderation_repository_dto::ContentModerationDecisionBacklogDto,
    > {
        self.repository
            .content_decision_backlog()
            .await
            .map_err(Into::into)
    }

    pub async fn list_content_decisions(
        &self,
        limit: i32,
        after_created_at: Option<chrono::DateTime<Utc>>,
        after_report_id: Option<Uuid>,
    ) -> crate::service_error::GatewayServiceResult<
        crate::dto::moderation_dto::ModerationContentDecisionListResponse,
    > {
        if after_created_at.is_some() != after_report_id.is_some() {
            return Err(gateway_domain::moderation::InvalidContentModerationDecisionCursor.into());
        }
        let limit = normalize_limit(limit);
        let after = after_created_at
            .zip(after_report_id)
            .map(|(created_at, report_id)| ContentModerationDecisionCursor {
                created_at,
                report_id,
            });
        let mut entries = self
            .repository
            .list_content_decisions(limit + 1, after.map(Into::into))
            .await?;
        let has_more = entries.len() > limit as usize;
        if has_more {
            entries.truncate(limit as usize);
        }
        let next_cursor = has_more
            .then(|| {
                entries.last().map(|entry| ContentModerationDecisionCursor {
                    created_at: entry.created_at,
                    report_id: entry.report_id,
                })
            })
            .flatten();
        Ok(
            crate::dto::moderation_dto::ModerationContentDecisionListResponse {
                decisions: entries.into_iter().map(Into::into).collect(),
                next_cursor: next_cursor.map(Into::into),
            },
        )
    }

    pub async fn replay_dead_lettered_content_decision(
        &self,
        report_id: String,
        actor_id: String,
        request: ReplayContentDecisionRequest,
    ) -> crate::service_error::GatewayServiceResult<bool> {
        let report_id = parse_uuid(&report_id, Input::ReportId)?;
        let actor_id = parse_uuid(&actor_id, Input::ReviewerId)?;
        if request.resolution_id.is_nil() {
            return Err(Input::ReplayResolutionId.into());
        }
        request.validate_input()?;
        self.repository
            .replay_dead_lettered_content_decision(
                report_id,
                request.resolution_id,
                actor_id,
                request.reason.trim(),
                request.evidence_reference.trim(),
            )
            .await
            .map_err(Into::into)
    }

    pub async fn create_report(
        &self,
        reporter_id: String,
        request: CreateReportRequest,
    ) -> crate::service_error::GatewayServiceResult<ModerationReportResponse> {
        self.create_report_with_mutation(reporter_id, request, None)
            .await
    }

    pub async fn create_report_with_mutation(
        &self,
        reporter_id: String,
        request: CreateReportRequest,
        context: Option<ModerationMutationContextDto>,
    ) -> crate::service_error::GatewayServiceResult<ModerationReportResponse> {
        let reporter_id = parse_uuid(&reporter_id, Input::ReporterId)?;
        let target_type =
            ReportTargetType::parse(&request.target_type).map_err(|_| Input::TargetType)?;
        let target_id = parse_uuid(&request.target_id, Input::TargetId)?;
        if target_type == ReportTargetType::User && reporter_id == target_id {
            return Err(Input::SelfReport.into());
        }
        let category = ReportCategory::parse(&request.category).map_err(|_| Input::Category)?;
        let reason = request.reason.trim();
        if !(3..=1000).contains(&reason.chars().count()) {
            return Err(Input::Reason.into());
        }
        let report = ModerationReport {
            id: self.runtime.new_id(),
            reporter_id,
            target_type,
            target_id,
            category,
            reason: reason.to_owned(),
            status: ReportStatus::Open,
            created_at: self.runtime.now(),
            resolved_at: None,
        };
        Ok(self
            .repository
            .create_report_with_mutation(report.into(), context)
            .await?
            .into())
    }

    pub async fn list_my_reports(
        &self,
        reporter_id: String,
        limit: i32,
    ) -> crate::service_error::GatewayServiceResult<ModerationReportListResponse> {
        let reporter_id = parse_uuid(&reporter_id, Input::ReporterId)?;
        let reports = self
            .repository
            .list_by_reporter(reporter_id, normalize_limit(limit))
            .await?;
        Ok(ModerationReportListResponse {
            reports: reports.into_iter().map(Into::into).collect(),
        })
    }

    pub async fn list_open_reports(
        &self,
        limit: i32,
    ) -> crate::service_error::GatewayServiceResult<ModerationReportListResponse> {
        let reports = self.repository.list_open(normalize_limit(limit)).await?;
        Ok(ModerationReportListResponse {
            reports: reports.into_iter().map(Into::into).collect(),
        })
    }

    pub async fn decide(
        &self,
        report_id: String,
        actor_id: String,
        request: CreateModerationDecisionRequest,
    ) -> crate::service_error::GatewayServiceResult<ModerationDecisionResponse> {
        let report_id = parse_uuid(&report_id, Input::ReportId)?;
        let actor_id = parse_uuid(&actor_id, Input::ActorId)?;
        let decision_type = ModerationDecisionType::parse(&request.decision_type)
            .map_err(|_| Input::DecisionType)?;
        let note = request.note.trim();
        if note.chars().count() > 2000 {
            return Err(Input::Note.into());
        }
        let report = self
            .repository
            .find_report(report_id)
            .await?
            .map(ModerationReport::from)
            .ok_or(ModerationRepositoryError::ReportMissing)?;

        validate_decision_target(decision_type, report.target_type)?;
        if decision_type.content_action().is_some() {
            let lease_owner = self.runtime.new_id();
            let stage = self
                .repository
                .begin_content_decision(
                    report_id,
                    actor_id,
                    decision_type.into(),
                    note,
                    lease_owner,
                    CONTENT_DECISION_LEASE,
                )
                .await?;
            let pending = match ContentModerationDecisionStage::from(stage) {
                ContentModerationDecisionStage::AlreadyRecorded { report, decision } => {
                    return Ok(ModerationDecisionResponse {
                        report: report.into(),
                        decision: decision.into(),
                    });
                }
                ContentModerationDecisionStage::Pending(pending) => pending,
            };
            self.apply_pending_content_decision(&pending).await?;
            return match self
                .repository
                .complete_content_decision(&(&pending).into())
                .await
            {
                Ok((report, decision)) => Ok(ModerationDecisionResponse {
                    report: report.into(),
                    decision: decision.into(),
                }),
                Err(error) => {
                    self.schedule_content_decision_retry(&pending, "GATEWAY_DECISION_STORE_FAILED")
                        .await?;
                    Err(error.into())
                }
            };
        }
        let (report, decision) = self
            .repository
            .record_decision(
                report_id,
                actor_id,
                decision_type.into(),
                note,
                request.restriction_expires_at,
            )
            .await?;
        Ok(ModerationDecisionResponse {
            report: report.into(),
            decision: decision.into(),
        })
    }

    pub async fn is_user_restricted(
        &self,
        user_id: &str,
    ) -> crate::service_error::GatewayServiceResult<bool> {
        let user_id = parse_uuid(user_id, Input::UserId)?;
        self.repository
            .is_user_restricted(user_id, self.runtime.now())
            .await
            .map_err(Into::into)
    }

    /// Process one durable content decision. The worker uses the same PostService
    /// idempotency key and Gateway finalization path as the synchronous request.
    pub async fn reconcile_one_content_decision(
        &self,
    ) -> crate::service_error::GatewayServiceResult<bool> {
        let lease_owner = self.runtime.new_id();
        let Some(pending) = self
            .repository
            .claim_next_content_decision(lease_owner, CONTENT_DECISION_LEASE)
            .await?
            .map(gateway_domain::moderation::PendingContentModerationDecision::from)
        else {
            return Ok(false);
        };
        if let Err(error) = self.apply_pending_content_decision(&pending).await {
            tracing::warn!(
                report_id = %pending.report.id,
                attempt_count = pending.attempt_count,
                error = ?error,
                "failed to reconcile staged content moderation decision"
            );
            return Ok(true);
        }
        if let Err(error) = self
            .repository
            .complete_content_decision(&(&pending).into())
            .await
        {
            tracing::warn!(
                report_id = %pending.report.id,
                attempt_count = pending.attempt_count,
                error = ?error,
                "failed to persist reconciled content moderation decision"
            );
            self.schedule_content_decision_retry(&pending, "GATEWAY_DECISION_STORE_FAILED")
                .await?;
        }
        Ok(true)
    }

    async fn apply_pending_content_decision(
        &self,
        pending: &gateway_domain::moderation::PendingContentModerationDecision,
    ) -> crate::service_error::GatewayServiceResult<()> {
        let action = pending
            .decision_type
            .content_action()
            .ok_or(crate::service_error::GatewayServiceError::InvalidModerationAction)?;
        match self
            .post_client
            .apply_content_moderation(
                pending.report.id.to_string(),
                pending.report.target_type.as_str().to_owned(),
                pending.report.target_id.to_string(),
                pending.actor_id.to_string(),
                action.to_owned(),
            )
            .await
        {
            Ok(true) => Ok(()),
            Ok(false) => {
                self.repository
                    .mark_content_target_unavailable(&pending.into())
                    .await?;
                Err(ModerationTargetUnavailable.into())
            }
            Err(error) => {
                self.schedule_content_decision_retry(pending, "POST_SERVICE_CALL_FAILED")
                    .await?;
                Err(error.into())
            }
        }
    }

    async fn schedule_content_decision_retry(
        &self,
        pending: &gateway_domain::moderation::PendingContentModerationDecision,
        error_code: &str,
    ) -> crate::service_error::GatewayServiceResult<()> {
        if pending.attempt_count >= MAX_CONTENT_DECISION_ATTEMPTS {
            self.repository
                .dead_letter_content_decision(&pending.into(), error_code)
                .await?;
        } else {
            self.repository
                .retry_content_decision(
                    &pending.into(),
                    content_decision_retry_after(pending.attempt_count),
                    error_code,
                )
                .await?;
        }
        Ok(())
    }
}

fn content_decision_retry_after(attempt_count: u32) -> Duration {
    let exponent = attempt_count.saturating_sub(1).min(6);
    Duration::from_secs(2_u64.saturating_pow(exponent).min(60))
}

fn validate_decision_target(
    decision_type: ModerationDecisionType,
    target_type: ReportTargetType,
) -> Result<(), Input> {
    match decision_type {
        ModerationDecisionType::HideContent | ModerationDecisionType::RestoreContent => {
            if !matches!(
                target_type,
                ReportTargetType::Post | ReportTargetType::Comment
            ) {
                return Err(Input::ContentTarget);
            }
        }
        ModerationDecisionType::RestrictUser | ModerationDecisionType::LiftRestriction => {
            if target_type != ReportTargetType::User {
                return Err(Input::UserTarget);
            }
        }
        ModerationDecisionType::Dismiss => {}
    }
    Ok(())
}

fn normalize_limit(limit: i32) -> i32 {
    if limit <= 0 {
        20
    } else {
        limit.min(100)
    }
}

fn parse_uuid(value: &str, input: Input) -> Result<Uuid, Input> {
    Uuid::parse_str(value.trim()).map_err(|_| input)
}
