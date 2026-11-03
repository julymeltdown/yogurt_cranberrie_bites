use anyhow::Result;
use api_gateway::adapters::out::moderation_runtime::SystemModerationRuntime;
use api_gateway::adapters::out::post_client::classify_post_client_error;
use api_gateway::adapters::out::repository::moderation_repository::classify_moderation_repository_error;
use api_gateway::application::dto::moderation_dto::{
    CreateModerationDecisionRequest, CreateReportRequest,
};
use api_gateway::application::dto::moderation_repository_dto::{
    ContentModerationDecisionStageDto as ContentModerationDecisionStage,
    ModerationDecisionDto as ModerationDecision,
    ModerationDecisionTypeDto as ModerationDecisionType, ModerationReportDto as ModerationReport,
    PendingContentModerationDecisionDto as PendingContentModerationDecision,
    ReportCategoryDto as ReportCategory, ReportStatusDto as ReportStatus,
    ReportTargetTypeDto as ReportTargetType,
};
use api_gateway::application::dto::post_dto::{
    CommentCreateRequest, CommentResponse, DeletePostResponse, FeedResponse, PostCreateRequest,
    PostDetailResponse, PostLikeResponse, PostResponse,
};
use api_gateway::application::ports::{ModerationRepositoryPort, PostClientPort};
use api_gateway::application::service::ModerationService;
use api_gateway::domain::moderation::{
    ModerationDecisionConflict, ModerationDecisionInProgress, ModerationTargetUnavailable,
    MAX_CONTENT_DECISION_ATTEMPTS,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

const REPORTER_ID: &str = "11111111-1111-1111-1111-111111111111";
const OTHER_ID: &str = "22222222-2222-2222-2222-222222222222";
const ADMIN_ID: &str = "33333333-3333-3333-3333-333333333333";

#[tokio::test]
async fn duplicate_report_returns_same_report_without_duplicate_storage() {
    let repository = Arc::new(FakeModerationRepository::default());
    let service = ModerationService::new(
        repository.clone(),
        Arc::new(RecordingPostClient::default()),
        Arc::new(SystemModerationRuntime),
    );
    let request = CreateReportRequest {
        target_type: "POST".into(),
        target_id: OTHER_ID.into(),
        category: "SPAM".into(),
        reason: "Repeated unsolicited advertising".into(),
    };

    let first = service
        .create_report(REPORTER_ID.into(), request.clone())
        .await
        .expect("first report");
    let second = service
        .create_report(REPORTER_ID.into(), request)
        .await
        .expect("duplicate report");

    assert_eq!(first.id, second.id);
    assert_eq!(repository.reports.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn self_user_report_is_rejected_before_repository_access() {
    let repository = Arc::new(FakeModerationRepository::default());
    let service = ModerationService::new(
        repository.clone(),
        Arc::new(RecordingPostClient::default()),
        Arc::new(SystemModerationRuntime),
    );

    let error = service
        .create_report(
            REPORTER_ID.into(),
            CreateReportRequest {
                target_type: "USER".into(),
                target_id: REPORTER_ID.into(),
                category: "HARASSMENT".into(),
                reason: "Invalid self report".into(),
            },
        )
        .await
        .expect_err("self report must fail");

    assert_eq!(error.to_string(), "Cannot report yourself");
    assert!(repository.create_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn hide_content_dispatches_to_post_service_before_immutable_decision_is_recorded() {
    let repository = Arc::new(FakeModerationRepository::default());
    let post_client = Arc::new(RecordingPostClient::default());
    let service = ModerationService::new(
        repository.clone(),
        post_client.clone(),
        Arc::new(SystemModerationRuntime),
    );
    let report =
        repository.insert_report(ReportTargetType::Post, Uuid::parse_str(OTHER_ID).unwrap());

    let response = service
        .decide(
            report.id.to_string(),
            ADMIN_ID.into(),
            CreateModerationDecisionRequest {
                decision_type: "HIDE_CONTENT".into(),
                note: "Confirmed spam".into(),
                restriction_expires_at: None,
            },
        )
        .await
        .expect("hide decision");

    assert_eq!(response.report.status, "RESOLVED");
    assert_eq!(response.decision.decision_type, "HIDE_CONTENT");
    assert_eq!(post_client.moderation_calls.lock().unwrap().len(), 1);
    let call = &post_client.moderation_calls.lock().unwrap()[0];
    assert_eq!(call.report_id, report.id.to_string());
    assert_eq!(call.target_type, "POST");
    assert_eq!(call.target_id, OTHER_ID);
    assert_eq!(call.action, "HIDE");
    assert_eq!(repository.decision_calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn missing_content_target_keeps_report_open_and_does_not_record_decision() {
    let repository = Arc::new(FakeModerationRepository::default());
    let post_client = Arc::new(RecordingPostClient::default());
    *post_client.moderation_result.lock().unwrap() = Some(false);
    let service = ModerationService::new(
        repository.clone(),
        post_client.clone(),
        Arc::new(SystemModerationRuntime),
    );
    let report =
        repository.insert_report(ReportTargetType::Post, Uuid::parse_str(OTHER_ID).unwrap());

    let error = service
        .decide(
            report.id.to_string(),
            ADMIN_ID.into(),
            CreateModerationDecisionRequest {
                decision_type: "HIDE_CONTENT".into(),
                note: "Confirmed spam".into(),
                restriction_expires_at: None,
            },
        )
        .await
        .expect_err("a missing target must not be recorded as moderated");

    assert!(std::error::Error::source(&error).is_some_and(|cause| cause
        .downcast_ref::<api_gateway::domain::moderation::ModerationTargetUnavailable>()
        .is_some()));
    assert!(repository.decision_calls.lock().unwrap().is_empty());
    assert_eq!(
        repository.reports.lock().unwrap()[&report.id].status,
        ReportStatus::Open
    );
    let retry_error = service
        .decide(
            report.id.to_string(),
            ADMIN_ID.into(),
            CreateModerationDecisionRequest {
                decision_type: "HIDE_CONTENT".into(),
                note: "Confirmed spam".into(),
                restriction_expires_at: None,
            },
        )
        .await
        .expect_err("the unavailable target stays terminal for this decision");
    assert_eq!(std::error::Error::source(&retry_error).and_then(|cause| cause.downcast_ref::<api_gateway::application::moderation_repository_error::ModerationRepositoryError>()),
        Some(&api_gateway::application::moderation_repository_error::ModerationRepositoryError::TargetUnavailable));
    assert_eq!(post_client.moderation_calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn retry_after_gateway_decision_store_failure_replays_the_same_content_action() {
    let repository = Arc::new(FakeModerationRepository::default());
    let post_client = Arc::new(RecordingPostClient::default());
    let service = ModerationService::new(
        repository.clone(),
        post_client.clone(),
        Arc::new(SystemModerationRuntime),
    );
    let report =
        repository.insert_report(ReportTargetType::Post, Uuid::parse_str(OTHER_ID).unwrap());
    repository.fail_next_decision_store();
    let request = CreateModerationDecisionRequest {
        decision_type: "HIDE_CONTENT".into(),
        note: "Confirmed spam".into(),
        restriction_expires_at: None,
    };

    let first_error = service
        .decide(report.id.to_string(), ADMIN_ID.into(), request.clone())
        .await
        .expect_err("the Gateway decision store fails after PostService succeeds");
    assert_eq!(std::error::Error::source(&first_error).and_then(|cause| cause.downcast_ref::<api_gateway::application::moderation_repository_error::ModerationRepositoryError>()),
        Some(&api_gateway::application::moderation_repository_error::ModerationRepositoryError::Unexpected));
    assert_eq!(first_error.to_string(), "Internal error");
    assert!(!first_error
        .to_string()
        .contains("simulated moderation decision store failure"));
    assert_eq!(
        repository.reports.lock().unwrap()[&report.id].status,
        ReportStatus::Open
    );
    assert!(repository.decision_calls.lock().unwrap().is_empty());

    let retry = service
        .decide(report.id.to_string(), ADMIN_ID.into(), request)
        .await
        .expect("retry records the Gateway decision after the idempotent PostService replay");

    assert_eq!(retry.report.status, "RESOLVED");
    assert_eq!(repository.decision_calls.lock().unwrap().len(), 1);
    let calls = post_client.moderation_calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0], calls[1]);
}

#[tokio::test]
async fn failed_content_decision_is_dead_lettered_at_the_eighth_attempt() {
    let repository = Arc::new(FakeModerationRepository::default());
    repository.set_next_attempt_count(MAX_CONTENT_DECISION_ATTEMPTS);
    let post_client = Arc::new(RecordingPostClient::default());
    post_client.fail_moderation();
    let service = ModerationService::new(
        repository.clone(),
        post_client,
        Arc::new(SystemModerationRuntime),
    );
    let report =
        repository.insert_report(ReportTargetType::Post, Uuid::parse_str(OTHER_ID).unwrap());

    let error = service
        .decide(
            report.id.to_string(),
            ADMIN_ID.into(),
            CreateModerationDecisionRequest {
                decision_type: "HIDE_CONTENT".into(),
                note: "Confirmed spam".into(),
                restriction_expires_at: None,
            },
        )
        .await
        .expect_err("downstream failure remains visible to the caller");

    assert_eq!(
        std::error::Error::source(&error)
            .and_then(|cause| cause
                .downcast_ref::<api_gateway::application::post_client_error::PostClientError>()),
        Some(&api_gateway::application::post_client_error::PostClientError::Unexpected)
    );
    assert_eq!(error.to_string(), "Internal error");
    assert!(!format!("{error:?}").contains("simulated PostService failure"));
    assert!(repository.retry_calls.lock().unwrap().is_empty());
    assert_eq!(
        repository.dead_letter_calls.lock().unwrap().as_slice(),
        &[(
            report.id,
            MAX_CONTENT_DECISION_ATTEMPTS,
            "POST_SERVICE_CALL_FAILED".into()
        )]
    );
}

#[tokio::test]
async fn earlier_content_decision_failures_use_backoff_instead_of_dead_lettering() {
    let repository = Arc::new(FakeModerationRepository::default());
    let post_client = Arc::new(RecordingPostClient::default());
    post_client.fail_moderation();
    let service = ModerationService::new(
        repository.clone(),
        post_client,
        Arc::new(SystemModerationRuntime),
    );
    let report =
        repository.insert_report(ReportTargetType::Post, Uuid::parse_str(OTHER_ID).unwrap());

    let _ = service
        .decide(
            report.id.to_string(),
            ADMIN_ID.into(),
            CreateModerationDecisionRequest {
                decision_type: "HIDE_CONTENT".into(),
                note: "Confirmed spam".into(),
                restriction_expires_at: None,
            },
        )
        .await
        .expect_err("first downstream failure is retryable");

    assert_eq!(repository.retry_calls.lock().unwrap().len(), 1);
    assert!(repository.dead_letter_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn restricting_user_is_queryable_from_mutation_gate() {
    let repository = Arc::new(FakeModerationRepository::default());
    let service = ModerationService::new(
        repository.clone(),
        Arc::new(RecordingPostClient::default()),
        Arc::new(SystemModerationRuntime),
    );
    let report =
        repository.insert_report(ReportTargetType::User, Uuid::parse_str(OTHER_ID).unwrap());

    service
        .decide(
            report.id.to_string(),
            ADMIN_ID.into(),
            CreateModerationDecisionRequest {
                decision_type: "RESTRICT_USER".into(),
                note: "Repeated abusive behavior".into(),
                restriction_expires_at: None,
            },
        )
        .await
        .expect("restrict user");

    assert!(service
        .is_user_restricted(OTHER_ID)
        .await
        .expect("restriction query"));
    assert!(!service
        .is_user_restricted(REPORTER_ID)
        .await
        .expect("unrestricted query"));
}

#[derive(Default)]
struct FakeModerationRepository {
    reports: Mutex<HashMap<Uuid, ModerationReport>>,
    report_keys: Mutex<HashMap<(Uuid, ReportTargetType, Uuid, ReportCategory), Uuid>>,
    restrictions: Mutex<HashSet<Uuid>>,
    create_calls: Mutex<Vec<ModerationReport>>,
    decision_calls: Mutex<Vec<(Uuid, Uuid, ModerationDecisionType)>>,
    decisions: Mutex<HashMap<(Uuid, ModerationDecisionType), ModerationDecision>>,
    pending_content_decisions: Mutex<HashMap<Uuid, PendingContentModerationDecision>>,
    unavailable_content_decisions: Mutex<HashSet<Uuid>>,
    fail_next_decision_store: Mutex<bool>,
    next_attempt_count: Mutex<Option<u32>>,
    retry_calls: Mutex<Vec<(Uuid, String)>>,
    dead_letter_calls: Mutex<Vec<(Uuid, u32, String)>>,
}

impl FakeModerationRepository {
    fn fail_next_decision_store(&self) {
        *self.fail_next_decision_store.lock().unwrap() = true;
    }

    fn set_next_attempt_count(&self, attempt_count: u32) {
        *self.next_attempt_count.lock().unwrap() = Some(attempt_count);
    }

    fn insert_report(&self, target_type: ReportTargetType, target_id: Uuid) -> ModerationReport {
        let report = ModerationReport {
            id: Uuid::new_v4(),
            reporter_id: Uuid::parse_str(REPORTER_ID).unwrap(),
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
}

impl FakeModerationRepository {
    async fn create_report_inner(&self, report: ModerationReport) -> Result<ModerationReport> {
        self.create_calls.lock().unwrap().push(report.clone());
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
        _limit: i32,
    ) -> Result<Vec<ModerationReport>> {
        Ok(self
            .reports
            .lock()
            .unwrap()
            .values()
            .filter(|report| report.reporter_id == reporter_id)
            .cloned()
            .collect())
    }

    async fn list_open_inner(&self, _limit: i32) -> Result<Vec<ModerationReport>> {
        Ok(self
            .reports
            .lock()
            .unwrap()
            .values()
            .filter(|report| report.status == ReportStatus::Open)
            .cloned()
            .collect())
    }

    async fn record_decision_inner(
        &self,
        report_id: Uuid,
        actor_id: Uuid,
        decision_type: ModerationDecisionType,
        note: &str,
        _restriction_expires_at: Option<DateTime<Utc>>,
    ) -> Result<(ModerationReport, ModerationDecision)> {
        if std::mem::take(&mut *self.fail_next_decision_store.lock().unwrap()) {
            anyhow::bail!("simulated moderation decision store failure");
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
        self.decision_calls
            .lock()
            .unwrap()
            .push((report_id, actor_id, decision_type));
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
            attempt_count: self.next_attempt_count.lock().unwrap().take().unwrap_or(1),
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
        error_code: &str,
    ) -> Result<bool> {
        self.retry_calls
            .lock()
            .unwrap()
            .push((pending.report.id, error_code.to_owned()));
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

    async fn dead_letter_content_decision_inner(
        &self,
        pending: &PendingContentModerationDecision,
        error_code: &str,
    ) -> Result<bool> {
        self.dead_letter_calls.lock().unwrap().push((
            pending.report.id,
            pending.attempt_count,
            error_code.to_owned(),
        ));
        Ok(self
            .pending_content_decisions
            .lock()
            .unwrap()
            .remove(&pending.report.id)
            .is_some())
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

    async fn list_content_decisions_inner(
        &self,
        _limit: i32,
        _after: Option<api_gateway::application::dto::moderation_repository_dto::ContentModerationDecisionCursorDto>,
    ) -> anyhow::Result<Vec<api_gateway::application::dto::moderation_repository_dto::ContentModerationDecisionOutboxEntryDto>>
    {
        anyhow::bail!("durable content moderation decisions are unavailable")
    }

    async fn replay_dead_lettered_content_decision_inner(
        &self,
        _report_id: uuid::Uuid,
        _resolution_id: uuid::Uuid,
        _actor_id: uuid::Uuid,
        _reason: &str,
        _evidence_reference: &str,
    ) -> anyhow::Result<bool> {
        anyhow::bail!("durable content moderation decisions are unavailable")
    }
}

#[async_trait]
impl ModerationRepositoryPort for FakeModerationRepository {
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
        _limit: i32,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<
        Vec<ModerationReport>,
    > {
        self.list_by_reporter_inner(reporter_id, _limit)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn list_open(
        &self,
        _limit: i32,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<
        Vec<ModerationReport>,
    > {
        self.list_open_inner(_limit)
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
        error_code: &str,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<bool>
    {
        self.retry_content_decision_inner(pending, _retry_after, error_code)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn dead_letter_content_decision(
        &self,
        pending: &PendingContentModerationDecision,
        error_code: &str,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<bool>
    {
        self.dead_letter_content_decision_inner(pending, error_code)
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
    async fn list_content_decisions(
        &self,
        _limit: i32,
        _after: Option<api_gateway::application::dto::moderation_repository_dto::ContentModerationDecisionCursorDto>,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<Vec<api_gateway::application::dto::moderation_repository_dto::ContentModerationDecisionOutboxEntryDto>>
    {
        self.list_content_decisions_inner(_limit, _after)
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn replay_dead_lettered_content_decision(
        &self,
        _report_id: uuid::Uuid,
        _resolution_id: uuid::Uuid,
        _actor_id: uuid::Uuid,
        _reason: &str,
        _evidence_reference: &str,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<bool>
    {
        self.replay_dead_lettered_content_decision_inner(
            _report_id,
            _resolution_id,
            _actor_id,
            _reason,
            _evidence_reference,
        )
        .await
        .map_err(classify_moderation_repository_error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModerationCall {
    report_id: String,
    target_type: String,
    target_id: String,
    actor_id: String,
    action: String,
}

#[derive(Default)]
struct RecordingPostClient {
    moderation_calls: Mutex<Vec<ModerationCall>>,
    moderation_result: Mutex<Option<bool>>,
    fail_moderation: Mutex<bool>,
}

impl RecordingPostClient {
    fn fail_moderation(&self) {
        *self.fail_moderation.lock().unwrap() = true;
    }
}

impl RecordingPostClient {
    async fn get_author_post_counts_inner(
        &self,
        _: Vec<String>,
    ) -> Result<std::collections::HashMap<String, i64>> {
        unreachable!()
    }

    async fn get_feed_inner(&self, _: String, _: Option<String>, _: i32) -> Result<FeedResponse> {
        unreachable!()
    }
    async fn get_author_posts_inner(
        &self,
        _: String,
        _: String,
        _: Option<String>,
        _: i32,
    ) -> Result<FeedResponse> {
        unreachable!()
    }
    async fn search_posts_inner(
        &self,
        _: String,
        _: String,
        _: Option<String>,
        _: i32,
    ) -> Result<FeedResponse> {
        unreachable!()
    }
    async fn get_trending_inner(
        &self,
        _: String,
        _: Option<String>,
        _: i32,
    ) -> Result<FeedResponse> {
        unreachable!()
    }
    async fn get_post_inner(&self, _: String, _: String) -> Result<PostDetailResponse> {
        unreachable!()
    }
    async fn create_post_inner(&self, _: String, _: PostCreateRequest) -> Result<PostResponse> {
        unreachable!()
    }
    async fn delete_post_inner(
        &self,
        _: String,
        _: String,
        _: Option<String>,
        _: Option<String>,
        _: Option<String>,
    ) -> Result<DeletePostResponse> {
        unreachable!()
    }
    async fn create_comment_inner(
        &self,
        _: String,
        _: String,
        _: CommentCreateRequest,
    ) -> Result<CommentResponse> {
        unreachable!()
    }
    async fn like_post_inner(&self, _: String, _: String) -> Result<PostLikeResponse> {
        unreachable!()
    }
    async fn unlike_post_inner(&self, _: String, _: String) -> Result<PostLikeResponse> {
        unreachable!()
    }

    async fn apply_content_moderation_inner(
        &self,
        report_id: String,
        target_type: String,
        target_id: String,
        actor_id: String,
        action: String,
    ) -> Result<bool> {
        self.moderation_calls.lock().unwrap().push(ModerationCall {
            report_id,
            target_type,
            target_id,
            actor_id,
            action,
        });
        if std::mem::take(&mut *self.fail_moderation.lock().unwrap()) {
            anyhow::bail!("simulated PostService failure");
        }
        Ok(self.moderation_result.lock().unwrap().unwrap_or(true))
    }

    // Explicit recording behavior formerly inherited from the port.
    async fn notification_posts_visible_inner(
        &self,
        _viewer_id: String,
        _post_ids: Vec<String>,
        _max_duration: std::time::Duration,
    ) -> anyhow::Result<std::collections::HashMap<String, bool>> {
        anyhow::bail!("Notification batch authorization is unavailable")
    }

    async fn get_mutation_receipt_status_inner(
        &self,
        _operation_id: uuid::Uuid,
    ) -> anyhow::Result<Option<api_gateway::application::dto::post_dto::MutationReceiptStatusDto>>
    {
        anyhow::bail!("Owner mutation status lookup is unavailable")
    }

    async fn get_ranked_feed_inner(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
        request_id: String,
        _session_id: Option<String>,
    ) -> anyhow::Result<api_gateway::application::dto::post_dto::RankedFeedResponse> {
        let feed = self.get_feed_inner(user_id, cursor, limit).await?;
        Ok(
            api_gateway::application::dto::post_dto::RankedFeedResponse::chronological_fallback(
                feed, request_id,
            ),
        )
    }

    async fn record_feed_recommendation_event_inner(
        &self,
        _viewer_id: String,
        _request_id: String,
        _event: api_gateway::application::dto::post_dto::FeedRecommendationEventRequest,
    ) -> anyhow::Result<api_gateway::application::dto::post_dto::FeedRecommendationEventResponse>
    {
        anyhow::bail!("Feed recommendation events are not implemented")
    }

    async fn resolve_dead_letter_replay_inner(
        &self,
        _request: api_gateway::application::dto::post_dto::ResolveDeadLetterReplayRequestDto,
    ) -> anyhow::Result<api_gateway::application::dto::post_dto::ResolveDeadLetterReplayResponseDto>
    {
        anyhow::bail!("Audited dead-letter replay is not implemented")
    }
}

#[async_trait]
impl PostClientPort for RecordingPostClient {
    async fn get_author_post_counts(
        &self,
        _post_arg_0: Vec<String>,
    ) -> api_gateway::application::post_client_error::PostClientResult<
        std::collections::HashMap<String, i64>,
    > {
        self.get_author_post_counts_inner(_post_arg_0)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_feed(
        &self,
        _post_arg_0: String,
        _post_arg_1: Option<String>,
        _post_arg_2: i32,
    ) -> api_gateway::application::post_client_error::PostClientResult<FeedResponse> {
        self.get_feed_inner(_post_arg_0, _post_arg_1, _post_arg_2)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_author_posts(
        &self,
        _post_arg_0: String,
        _post_arg_1: String,
        _post_arg_2: Option<String>,
        _post_arg_3: i32,
    ) -> api_gateway::application::post_client_error::PostClientResult<FeedResponse> {
        self.get_author_posts_inner(_post_arg_0, _post_arg_1, _post_arg_2, _post_arg_3)
            .await
            .map_err(classify_post_client_error)
    }
    async fn search_posts(
        &self,
        _post_arg_0: String,
        _post_arg_1: String,
        _post_arg_2: Option<String>,
        _post_arg_3: i32,
    ) -> api_gateway::application::post_client_error::PostClientResult<FeedResponse> {
        self.search_posts_inner(_post_arg_0, _post_arg_1, _post_arg_2, _post_arg_3)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_trending(
        &self,
        _post_arg_0: String,
        _post_arg_1: Option<String>,
        _post_arg_2: i32,
    ) -> api_gateway::application::post_client_error::PostClientResult<FeedResponse> {
        self.get_trending_inner(_post_arg_0, _post_arg_1, _post_arg_2)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_post(
        &self,
        _post_arg_0: String,
        _post_arg_1: String,
    ) -> api_gateway::application::post_client_error::PostClientResult<PostDetailResponse> {
        self.get_post_inner(_post_arg_0, _post_arg_1)
            .await
            .map_err(classify_post_client_error)
    }
    async fn create_post(
        &self,
        _post_arg_0: String,
        _post_arg_1: PostCreateRequest,
    ) -> api_gateway::application::post_client_error::PostClientResult<PostResponse> {
        self.create_post_inner(_post_arg_0, _post_arg_1)
            .await
            .map_err(classify_post_client_error)
    }
    async fn delete_post(
        &self,
        _post_arg_0: String,
        _post_arg_1: String,
        _post_arg_2: Option<String>,
        _post_arg_3: Option<String>,
        _post_arg_4: Option<String>,
    ) -> api_gateway::application::post_client_error::PostClientResult<DeletePostResponse> {
        self.delete_post_inner(
            _post_arg_0,
            _post_arg_1,
            _post_arg_2,
            _post_arg_3,
            _post_arg_4,
        )
        .await
        .map_err(classify_post_client_error)
    }
    async fn create_comment(
        &self,
        _post_arg_0: String,
        _post_arg_1: String,
        _post_arg_2: CommentCreateRequest,
    ) -> api_gateway::application::post_client_error::PostClientResult<CommentResponse> {
        self.create_comment_inner(_post_arg_0, _post_arg_1, _post_arg_2)
            .await
            .map_err(classify_post_client_error)
    }
    async fn like_post(
        &self,
        _post_arg_0: String,
        _post_arg_1: String,
    ) -> api_gateway::application::post_client_error::PostClientResult<PostLikeResponse> {
        self.like_post_inner(_post_arg_0, _post_arg_1)
            .await
            .map_err(classify_post_client_error)
    }
    async fn unlike_post(
        &self,
        _post_arg_0: String,
        _post_arg_1: String,
    ) -> api_gateway::application::post_client_error::PostClientResult<PostLikeResponse> {
        self.unlike_post_inner(_post_arg_0, _post_arg_1)
            .await
            .map_err(classify_post_client_error)
    }
    async fn apply_content_moderation(
        &self,
        report_id: String,
        target_type: String,
        target_id: String,
        actor_id: String,
        action: String,
    ) -> api_gateway::application::post_client_error::PostClientResult<bool> {
        self.apply_content_moderation_inner(report_id, target_type, target_id, actor_id, action)
            .await
            .map_err(classify_post_client_error)
    }
    async fn notification_posts_visible(
        &self,
        _viewer_id: String,
        _post_ids: Vec<String>,
        _max_duration: std::time::Duration,
    ) -> api_gateway::application::post_client_error::PostClientResult<
        std::collections::HashMap<String, bool>,
    > {
        self.notification_posts_visible_inner(_viewer_id, _post_ids, _max_duration)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_mutation_receipt_status(
        &self,
        _operation_id: uuid::Uuid,
    ) -> api_gateway::application::post_client_error::PostClientResult<
        Option<api_gateway::application::dto::post_dto::MutationReceiptStatusDto>,
    > {
        self.get_mutation_receipt_status_inner(_operation_id)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_ranked_feed(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
        request_id: String,
        _session_id: Option<String>,
    ) -> api_gateway::application::post_client_error::PostClientResult<
        api_gateway::application::dto::post_dto::RankedFeedResponse,
    > {
        self.get_ranked_feed_inner(user_id, cursor, limit, request_id, _session_id)
            .await
            .map_err(classify_post_client_error)
    }
    async fn record_feed_recommendation_event(
        &self,
        _viewer_id: String,
        _request_id: String,
        _event: api_gateway::application::dto::post_dto::FeedRecommendationEventRequest,
    ) -> api_gateway::application::post_client_error::PostClientResult<
        api_gateway::application::dto::post_dto::FeedRecommendationEventResponse,
    > {
        self.record_feed_recommendation_event_inner(_viewer_id, _request_id, _event)
            .await
            .map_err(classify_post_client_error)
    }
    async fn resolve_dead_letter_replay(
        &self,
        _request: api_gateway::application::dto::post_dto::ResolveDeadLetterReplayRequestDto,
    ) -> api_gateway::application::post_client_error::PostClientResult<
        api_gateway::application::dto::post_dto::ResolveDeadLetterReplayResponseDto,
    > {
        self.resolve_dead_letter_replay_inner(_request)
            .await
            .map_err(classify_post_client_error)
    }
}

#[path = "moderation_runtime.rs"]
mod runtime_capability_tests;
