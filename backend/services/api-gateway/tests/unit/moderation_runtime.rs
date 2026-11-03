use super::*;
use api_gateway::adapters::out::repository::moderation_repository::classify_moderation_repository_error;
use api_gateway::application::dto::moderation_dto::ModerationMutationContextDto;
use api_gateway::application::dto::moderation_repository_dto::{
    ContentModerationDecisionBacklogDto, ContentModerationDecisionCursorDto,
    ContentModerationDecisionOutboxEntryDto,
};
use api_gateway::application::ports::ModerationRuntimePort;
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Trace {
    Id(Uuid),
    Now(DateTime<Utc>),
    Create(Uuid, DateTime<Utc>),
    Find(Uuid),
    Begin(Uuid, Duration),
    Claim(Uuid, Duration),
    Complete(Uuid, Uuid),
    Restriction(Uuid, DateTime<Utc>),
}

struct ScriptedRuntime {
    values: Mutex<VecDeque<Trace>>,
    trace: Arc<Mutex<Vec<Trace>>>,
}

impl ScriptedRuntime {
    fn new(values: Vec<Trace>, trace: Arc<Mutex<Vec<Trace>>>) -> Self {
        Self {
            values: Mutex::new(values.into()),
            trace,
        }
    }

    fn take(&self) -> Trace {
        let value = self
            .values
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected runtime sampling");
        self.trace.lock().unwrap().push(value.clone());
        value
    }

    fn exhausted(&self) {
        assert!(
            self.values.lock().unwrap().is_empty(),
            "expected runtime value was not consumed"
        );
    }
}

impl ModerationRuntimePort for ScriptedRuntime {
    fn now(&self) -> DateTime<Utc> {
        match self.take() {
            Trace::Now(value) => value,
            other => panic!("clock sampled instead of {other:?}"),
        }
    }

    fn new_id(&self) -> Uuid {
        match self.take() {
            Trace::Id(value) => value,
            other => panic!("identity allocated instead of {other:?}"),
        }
    }
}

struct RuntimeRecordingRepository {
    inner: Arc<FakeModerationRepository>,
    trace: Arc<Mutex<Vec<Trace>>>,
    claimed: Mutex<Option<PendingContentModerationDecision>>,
}

impl RuntimeRecordingRepository {
    async fn create_report_inner(&self, report: ModerationReport) -> Result<ModerationReport> {
        self.trace
            .lock()
            .unwrap()
            .push(Trace::Create(report.id, report.created_at));
        self.inner.create_report(report).await.map_err(Into::into)
    }

    async fn create_report_with_mutation_inner(
        &self,
        report: ModerationReport,
        context: Option<ModerationMutationContextDto>,
    ) -> Result<ModerationReport> {
        self.trace
            .lock()
            .unwrap()
            .push(Trace::Create(report.id, report.created_at));
        self.inner
            .create_report_with_mutation(report, context)
            .await
            .map_err(Into::into)
    }

    async fn find_report_inner(&self, report_id: Uuid) -> Result<Option<ModerationReport>> {
        self.trace.lock().unwrap().push(Trace::Find(report_id));
        self.inner.find_report(report_id).await.map_err(Into::into)
    }

    async fn list_by_reporter_inner(
        &self,
        reporter_id: Uuid,
        limit: i32,
    ) -> Result<Vec<ModerationReport>> {
        self.inner
            .list_by_reporter(reporter_id, limit)
            .await
            .map_err(Into::into)
    }

    async fn list_open_inner(&self, limit: i32) -> Result<Vec<ModerationReport>> {
        self.inner.list_open(limit).await.map_err(Into::into)
    }

    async fn content_decision_backlog_inner(&self) -> Result<ContentModerationDecisionBacklogDto> {
        self.inner
            .content_decision_backlog()
            .await
            .map_err(Into::into)
    }

    async fn list_content_decisions_inner(
        &self,
        limit: i32,
        after: Option<ContentModerationDecisionCursorDto>,
    ) -> Result<Vec<ContentModerationDecisionOutboxEntryDto>> {
        self.inner
            .list_content_decisions(limit, after)
            .await
            .map_err(Into::into)
    }

    async fn record_decision_inner(
        &self,
        report_id: Uuid,
        actor_id: Uuid,
        decision_type: ModerationDecisionType,
        note: &str,
        restriction_expires_at: Option<DateTime<Utc>>,
    ) -> Result<(ModerationReport, ModerationDecision)> {
        self.inner
            .record_decision(
                report_id,
                actor_id,
                decision_type,
                note,
                restriction_expires_at,
            )
            .await
            .map_err(Into::into)
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
        self.trace
            .lock()
            .unwrap()
            .push(Trace::Begin(lease_owner, lease_for));
        self.inner
            .begin_content_decision(
                report_id,
                actor_id,
                decision_type,
                note,
                lease_owner,
                lease_for,
            )
            .await
            .map_err(Into::into)
    }

    async fn claim_next_content_decision_inner(
        &self,
        lease_owner: Uuid,
        lease_for: Duration,
    ) -> Result<Option<PendingContentModerationDecision>> {
        self.trace
            .lock()
            .unwrap()
            .push(Trace::Claim(lease_owner, lease_for));
        Ok(self.claimed.lock().unwrap().take())
    }

    async fn complete_content_decision_inner(
        &self,
        pending: &PendingContentModerationDecision,
    ) -> Result<(ModerationReport, ModerationDecision)> {
        self.trace
            .lock()
            .unwrap()
            .push(Trace::Complete(pending.report.id, pending.lease_owner));
        self.inner
            .complete_content_decision(pending)
            .await
            .map_err(Into::into)
    }

    async fn retry_content_decision_inner(
        &self,
        pending: &PendingContentModerationDecision,
        retry_after: Duration,
        error_code: &str,
    ) -> Result<bool> {
        self.inner
            .retry_content_decision(pending, retry_after, error_code)
            .await
            .map_err(Into::into)
    }

    async fn dead_letter_content_decision_inner(
        &self,
        pending: &PendingContentModerationDecision,
        error_code: &str,
    ) -> Result<bool> {
        self.inner
            .dead_letter_content_decision(pending, error_code)
            .await
            .map_err(Into::into)
    }

    async fn replay_dead_lettered_content_decision_inner(
        &self,
        report_id: Uuid,
        resolution_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        evidence_reference: &str,
    ) -> Result<bool> {
        self.inner
            .replay_dead_lettered_content_decision(
                report_id,
                resolution_id,
                actor_id,
                reason,
                evidence_reference,
            )
            .await
            .map_err(Into::into)
    }

    async fn mark_content_target_unavailable_inner(
        &self,
        pending: &PendingContentModerationDecision,
    ) -> Result<bool> {
        self.inner
            .mark_content_target_unavailable(pending)
            .await
            .map_err(Into::into)
    }

    async fn is_user_restricted_inner(&self, user_id: Uuid, now: DateTime<Utc>) -> Result<bool> {
        self.trace
            .lock()
            .unwrap()
            .push(Trace::Restriction(user_id, now));
        self.inner
            .is_user_restricted(user_id, now)
            .await
            .map_err(Into::into)
    }
}

#[async_trait]
impl ModerationRepositoryPort for RuntimeRecordingRepository {
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
    async fn create_report_with_mutation(
        &self,
        report: ModerationReport,
        context: Option<ModerationMutationContextDto>,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<
        ModerationReport,
    > {
        self.create_report_with_mutation_inner(report, context)
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
    async fn content_decision_backlog(
        &self,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<
        ContentModerationDecisionBacklogDto,
    > {
        self.content_decision_backlog_inner()
            .await
            .map_err(classify_moderation_repository_error)
    }
    async fn list_content_decisions(
        &self,
        limit: i32,
        after: Option<ContentModerationDecisionCursorDto>,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<
        Vec<ContentModerationDecisionOutboxEntryDto>,
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
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<(
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
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<
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
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<
        Option<PendingContentModerationDecision>,
    > {
        self.claim_next_content_decision_inner(lease_owner, lease_for)
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
        retry_after: Duration,
        error_code: &str,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<bool>
    {
        self.retry_content_decision_inner(pending, retry_after, error_code)
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
        now: DateTime<Utc>,
    ) -> api_gateway::application::moderation_repository_error::ModerationRepositoryResult<bool>
    {
        self.is_user_restricted_inner(user_id, now)
            .await
            .map_err(classify_moderation_repository_error)
    }
}

fn fixture(
    values: Vec<Trace>,
) -> (
    ModerationService,
    Arc<RuntimeRecordingRepository>,
    Arc<RecordingPostClient>,
    Arc<ScriptedRuntime>,
) {
    let trace = Arc::new(Mutex::new(Vec::new()));
    let runtime = Arc::new(ScriptedRuntime::new(values, trace.clone()));
    let repository = Arc::new(RuntimeRecordingRepository {
        inner: Arc::new(FakeModerationRepository::default()),
        trace,
        claimed: Mutex::new(None),
    });
    let post = Arc::new(RecordingPostClient::default());
    let service = ModerationService::new(repository.clone(), post.clone(), runtime.clone());
    (service, repository, post, runtime)
}

fn instant() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2024-02-29T12:34:56.123456Z")
        .unwrap()
        .with_timezone(&Utc)
}

fn report_request() -> CreateReportRequest {
    CreateReportRequest {
        target_type: "POST".into(),
        target_id: OTHER_ID.into(),
        category: "SPAM".into(),
        reason: "  valid report reason  ".into(),
    }
}

fn decision_request() -> CreateModerationDecisionRequest {
    CreateModerationDecisionRequest {
        decision_type: "HIDE_CONTENT".into(),
        note: "  reviewed  ".into(),
        restriction_expires_at: None,
    }
}

#[tokio::test]
async fn application_validation_returns_typed_inputs_without_runtime_samples_or_dispatch() {
    use api_gateway::application::dto::moderation_dto::ReplayContentDecisionRequest;
    use api_gateway::application::moderation_input::ModerationInputError as Input;
    let (service, repository, post, runtime) = fixture(vec![]);
    let error = service
        .create_report("bad-actor".into(), report_request())
        .await
        .unwrap_err();
    assert_eq!(
        std::error::Error::source(&error).and_then(|cause| cause.downcast_ref::<Input>()),
        Some(&Input::ReporterId)
    );
    let mut request = report_request();
    request.target_id = "bad-target".into();
    let error = service
        .create_report(REPORTER_ID.into(), request)
        .await
        .unwrap_err();
    assert_eq!(
        std::error::Error::source(&error).and_then(|cause| cause.downcast_ref::<Input>()),
        Some(&Input::TargetId)
    );
    let error = service
        .list_my_reports("bad-actor".into(), 20)
        .await
        .unwrap_err();
    assert_eq!(
        std::error::Error::source(&error).and_then(|cause| cause.downcast_ref::<Input>()),
        Some(&Input::ReporterId)
    );
    let error = service.is_user_restricted("bad-user").await.unwrap_err();
    assert_eq!(
        std::error::Error::source(&error).and_then(|cause| cause.downcast_ref::<Input>()),
        Some(&Input::UserId)
    );
    let error = service
        .decide(OTHER_ID.into(), "bad-actor".into(), decision_request())
        .await
        .unwrap_err();
    assert_eq!(
        std::error::Error::source(&error).and_then(|cause| cause.downcast_ref::<Input>()),
        Some(&Input::ActorId)
    );
    let report = repository
        .inner
        .insert_report(ReportTargetType::User, Uuid::parse_str(OTHER_ID).unwrap());
    let error = service
        .decide(report.id.to_string(), ADMIN_ID.into(), decision_request())
        .await
        .unwrap_err();
    assert_eq!(
        std::error::Error::source(&error).and_then(|cause| cause.downcast_ref::<Input>()),
        Some(&Input::ContentTarget)
    );
    let valid = ReplayContentDecisionRequest {
        resolution_id: Uuid::from_u128(99),
        reason: "Reviewed protected evidence for replay".into(),
        evidence_reference: "incident/42".into(),
    };
    for (report, actor, request, input) in [
        ("bad-report", ADMIN_ID, valid.clone(), Input::ReportId),
        (OTHER_ID, "bad-reviewer", valid.clone(), Input::ReviewerId),
        (
            OTHER_ID,
            ADMIN_ID,
            ReplayContentDecisionRequest {
                resolution_id: Uuid::nil(),
                ..valid.clone()
            },
            Input::ReplayResolutionId,
        ),
        (
            OTHER_ID,
            ADMIN_ID,
            ReplayContentDecisionRequest {
                reason: "short".into(),
                ..valid.clone()
            },
            Input::ReplayReason,
        ),
        (
            OTHER_ID,
            ADMIN_ID,
            ReplayContentDecisionRequest {
                evidence_reference: "private invalid reference".into(),
                ..valid
            },
            Input::ReplayEvidence,
        ),
    ] {
        let error = service
            .replay_dead_lettered_content_decision(report.into(), actor.into(), request)
            .await
            .unwrap_err();
        assert_eq!(
            std::error::Error::source(&error).and_then(|cause| cause.downcast_ref::<Input>()),
            Some(&input)
        );
    }
    assert_eq!(
        *repository.trace.lock().unwrap(),
        vec![Trace::Find(report.id)]
    );
    assert!(repository.inner.create_calls.lock().unwrap().is_empty());
    assert!(post.moderation_calls.lock().unwrap().is_empty());
    runtime.exhausted();
}

#[tokio::test]
async fn report_samples_id_then_time_before_persistence() {
    let id = Uuid::from_u128(41);
    let (service, repository, _, runtime) = fixture(vec![Trace::Id(id), Trace::Now(instant())]);
    let response = service
        .create_report(REPORTER_ID.into(), report_request())
        .await
        .unwrap();
    assert_eq!(response.id, id.to_string());
    assert_eq!(response.created_at, instant());
    assert_eq!(response.reason, "valid report reason");
    assert_eq!(
        *repository.trace.lock().unwrap(),
        vec![
            Trace::Id(id),
            Trace::Now(instant()),
            Trace::Create(id, instant())
        ]
    );
    let calls = repository.inner.create_calls.lock().unwrap();
    assert_eq!(calls[0].reporter_id, Uuid::parse_str(REPORTER_ID).unwrap());
    assert_eq!(calls[0].target_id, Uuid::parse_str(OTHER_ID).unwrap());
    assert_eq!(calls[0].id, id);
    runtime.exhausted();
}

#[tokio::test]
async fn rejected_report_validation_consumes_no_clock_or_identity() {
    let (service, repository, _, runtime) = fixture(vec![]);
    let mut self_report = report_request();
    self_report.target_type = "USER".into();
    self_report.target_id = REPORTER_ID.into();
    let mut invalid_target = report_request();
    invalid_target.target_type = "unsupported".into();
    let mut invalid_category = report_request();
    invalid_category.category = "unsupported".into();
    let mut short_reason = report_request();
    short_reason.reason = " x ".into();
    let mut long_reason = report_request();
    long_reason.reason = "a".repeat(1001);
    for (actor, request) in [
        ("invalid", report_request()),
        (REPORTER_ID, self_report),
        (REPORTER_ID, invalid_target),
        (REPORTER_ID, invalid_category),
        (REPORTER_ID, short_reason),
        (REPORTER_ID, long_reason),
    ] {
        assert!(service.create_report(actor.into(), request).await.is_err());
    }
    assert!(repository.trace.lock().unwrap().is_empty());
    assert!(repository.inner.create_calls.lock().unwrap().is_empty());
    runtime.exhausted();
}

#[tokio::test]
async fn rejected_decision_validation_and_missing_report_allocate_no_lease() {
    let (service, repository, post, runtime) = fixture(vec![]);
    let missing = Uuid::from_u128(42);
    assert!(service
        .decide(missing.to_string(), ADMIN_ID.into(), decision_request())
        .await
        .is_err());
    assert_eq!(
        *repository.trace.lock().unwrap(),
        vec![Trace::Find(missing)]
    );
    repository.trace.lock().unwrap().clear();
    assert!(service
        .decide("invalid".into(), ADMIN_ID.into(), decision_request())
        .await
        .is_err());
    let report = repository
        .inner
        .insert_report(ReportTargetType::User, Uuid::parse_str(OTHER_ID).unwrap());
    assert!(service
        .decide(report.id.to_string(), ADMIN_ID.into(), decision_request())
        .await
        .is_err());
    assert_eq!(
        *repository.trace.lock().unwrap(),
        vec![Trace::Find(report.id)]
    );
    assert!(post.moderation_calls.lock().unwrap().is_empty());
    runtime.exhausted();
}

#[tokio::test]
async fn synchronous_content_decision_uses_the_sampled_lease_owner_once() {
    let owner = Uuid::from_u128(43);
    let (service, repository, post, runtime) = fixture(vec![Trace::Id(owner)]);
    let report = repository
        .inner
        .insert_report(ReportTargetType::Post, Uuid::parse_str(OTHER_ID).unwrap());
    let response = service
        .decide(report.id.to_string(), ADMIN_ID.into(), decision_request())
        .await
        .unwrap();
    assert_eq!(response.report.status, "RESOLVED");
    assert_eq!(
        *repository.trace.lock().unwrap(),
        vec![
            Trace::Find(report.id),
            Trace::Id(owner),
            Trace::Begin(owner, Duration::from_secs(30)),
            Trace::Complete(report.id, owner)
        ]
    );
    assert_eq!(
        post.moderation_calls.lock().unwrap()[0].report_id,
        report.id.to_string()
    );
    runtime.exhausted();
}

#[tokio::test]
async fn already_recorded_content_decision_retains_its_existing_owner_sampling() {
    let first = Uuid::from_u128(44);
    let second = Uuid::from_u128(45);
    let (service, repository, post, runtime) = fixture(vec![Trace::Id(first), Trace::Id(second)]);
    let report = repository
        .inner
        .insert_report(ReportTargetType::Post, Uuid::parse_str(OTHER_ID).unwrap());
    let initial = service
        .decide(report.id.to_string(), ADMIN_ID.into(), decision_request())
        .await
        .unwrap();
    repository.trace.lock().unwrap().clear();
    let replay = service
        .decide(report.id.to_string(), ADMIN_ID.into(), decision_request())
        .await
        .unwrap();
    assert_eq!(replay, initial);
    assert_eq!(
        *repository.trace.lock().unwrap(),
        vec![
            Trace::Find(report.id),
            Trace::Id(second),
            Trace::Begin(second, Duration::from_secs(30))
        ]
    );
    assert_eq!(post.moderation_calls.lock().unwrap().len(), 1);
    runtime.exhausted();
}

#[tokio::test]
async fn restriction_query_samples_exactly_once_after_uuid_validation() {
    let (service, repository, _, runtime) = fixture(vec![Trace::Now(instant())]);
    assert!(service.is_user_restricted("invalid").await.is_err());
    assert!(repository.trace.lock().unwrap().is_empty());
    assert!(!service.is_user_restricted(OTHER_ID).await.unwrap());
    assert_eq!(
        *repository.trace.lock().unwrap(),
        vec![
            Trace::Now(instant()),
            Trace::Restriction(Uuid::parse_str(OTHER_ID).unwrap(), instant())
        ]
    );
    runtime.exhausted();
}

#[tokio::test]
async fn empty_worker_claim_still_samples_one_owner_before_claiming() {
    let owner = Uuid::from_u128(46);
    let (service, repository, post, runtime) = fixture(vec![Trace::Id(owner)]);
    assert!(!service.reconcile_one_content_decision().await.unwrap());
    assert_eq!(
        *repository.trace.lock().unwrap(),
        vec![
            Trace::Id(owner),
            Trace::Claim(owner, Duration::from_secs(30))
        ]
    );
    assert!(post.moderation_calls.lock().unwrap().is_empty());
    runtime.exhausted();
}

#[tokio::test]
async fn worker_completion_or_retry_never_resamples_the_claim_owner() {
    for fail_post in [false, true] {
        let owner = Uuid::from_u128(47);
        let (service, repository, post, runtime) = fixture(vec![Trace::Id(owner)]);
        let report = repository
            .inner
            .insert_report(ReportTargetType::Post, Uuid::parse_str(OTHER_ID).unwrap());
        let pending = PendingContentModerationDecision {
            report: report.clone(),
            actor_id: Uuid::parse_str(ADMIN_ID).unwrap(),
            decision_type: ModerationDecisionType::HideContent,
            note: "reviewed".into(),
            attempt_count: 1,
            lease_owner: owner,
        };
        repository
            .inner
            .pending_content_decisions
            .lock()
            .unwrap()
            .insert(report.id, pending.clone());
        *repository.claimed.lock().unwrap() = Some(pending);
        if fail_post {
            post.fail_moderation();
        }
        assert!(service.reconcile_one_content_decision().await.unwrap());
        let mut expected = vec![
            Trace::Id(owner),
            Trace::Claim(owner, Duration::from_secs(30)),
        ];
        if fail_post {
            assert_eq!(
                repository.inner.retry_calls.lock().unwrap().as_slice(),
                &[(report.id, "POST_SERVICE_CALL_FAILED".into())]
            );
        } else {
            expected.push(Trace::Complete(report.id, owner));
            assert_eq!(repository.inner.decision_calls.lock().unwrap().len(), 1);
        }
        assert_eq!(*repository.trace.lock().unwrap(), expected);
        assert_eq!(post.moderation_calls.lock().unwrap()[0].actor_id, ADMIN_ID);
        runtime.exhausted();
    }
}
