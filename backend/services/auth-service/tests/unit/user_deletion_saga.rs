use async_trait::async_trait;
use auth_service::adapters::r#in::messaging::user_deletion_worker::{
    UserDeletionWorker, UserDeletionWorkerPolicy,
};
use auth_service::application::dto::auth_dto::DeleteUserDto;
use auth_service::application::ports::{UserDeletionDispatcherPort, UserDeletionSagaPort};
use auth_service::application::use_cases::delete_user::DeleteUserUseCase;
use auth_service::application::use_cases::get_user_deletion::GetUserDeletionUseCase;
use auth_service::application::use_cases::list_dead_lettered_user_deletions::ListDeadLetteredUserDeletionsUseCase;
use auth_service::application::use_cases::retry_user_deletion::RetryUserDeletionUseCase;
use auth_service::domain::user_deletion::{
    BeginUserDeletion, RetryUserDeletion, UserDeletionSaga, UserDeletionSagaStatus,
    UserDeletionStep, UserDeletionStepStatus, UserDeletionTarget,
};
use chrono::{Duration, TimeZone, Utc};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

type FailedStep = (Uuid, UserDeletionTarget, String, bool);
type DispatchCall = (UserDeletionTarget, Uuid, Uuid, chrono::DateTime<Utc>, Uuid);

#[derive(Clone, Default)]
struct RecordingSagaRepository {
    begin_requests: Arc<Mutex<Vec<BeginUserDeletion>>>,
    retry_requests: Arc<Mutex<Vec<RetryUserDeletion>>>,
    sagas: Arc<Mutex<HashMap<Uuid, UserDeletionSaga>>>,
    due: Arc<Mutex<VecDeque<UserDeletionStep>>>,
    succeeded: Arc<Mutex<Vec<(Uuid, UserDeletionTarget)>>>,
    failed: Arc<Mutex<Vec<FailedStep>>>,
}

#[async_trait]
impl UserDeletionSagaPort for RecordingSagaRepository {
    async fn begin(
        &self,
        request: auth_service::application::dto::deletion_dto::AuthBeginUserDeletionDto,
    ) -> auth_service::application::errors::AuthResult<
        Option<auth_service::application::dto::deletion_dto::AuthUserDeletionSagaRecordDto>,
    > {
        let request = auth_service::domain::user_deletion::BeginUserDeletion::from(request);

        let result: anyhow::Result<_> = async {
            self.begin_requests.lock().unwrap().push(request.clone());
            let saga = saga(
                request.saga_id,
                request.user_id,
                request.actor_id,
                request.retention_until,
            );
            self.sagas
                .lock()
                .unwrap()
                .insert(request.user_id, saga.clone());
            Ok(Some(saga))
        }
        .await;
        result
            .map_err(auth_service::adapters::out::errors::boundary_error)
            .map(|value| value.map(Into::into))
    }

    async fn find_by_user_id(
        &self,
        user_id: Uuid,
    ) -> auth_service::application::errors::AuthResult<
        Option<auth_service::application::dto::deletion_dto::AuthUserDeletionSagaRecordDto>,
    > {
        let result: anyhow::Result<_> =
            async { Ok(self.sagas.lock().unwrap().get(&user_id).cloned()) }.await;
        result
            .map_err(auth_service::adapters::out::errors::boundary_error)
            .map(|value| value.map(Into::into))
    }

    async fn find_by_saga_id(
        &self,
        saga_id: Uuid,
    ) -> auth_service::application::errors::AuthResult<
        Option<auth_service::application::dto::deletion_dto::AuthUserDeletionSagaRecordDto>,
    > {
        let result: anyhow::Result<_> = async {
            Ok(self
                .sagas
                .lock()
                .unwrap()
                .values()
                .find(|saga| saga.id == saga_id)
                .cloned())
        }
        .await;
        result
            .map_err(auth_service::adapters::out::errors::boundary_error)
            .map(|value| value.map(Into::into))
    }

    async fn list_dead_lettered(
        &self,
        after: Option<(chrono::DateTime<Utc>, Uuid)>,
        limit: i32,
    ) -> auth_service::application::errors::AuthResult<
        Vec<auth_service::application::dto::deletion_dto::AuthUserDeletionSagaRecordDto>,
    > {
        let result: anyhow::Result<_> = async {
            let mut sagas = self
                .sagas
                .lock()
                .unwrap()
                .values()
                .filter(|saga| saga.status == UserDeletionSagaStatus::DeadLettered)
                .filter(|saga| after.is_none_or(|cursor| (saga.updated_at, saga.id) > cursor))
                .cloned()
                .collect::<Vec<_>>();
            sagas.sort_by_key(|saga| (saga.created_at, saga.id));
            sagas.truncate(limit.max(0) as usize);
            Ok(sagas)
        }
        .await;
        result
            .map_err(auth_service::adapters::out::errors::boundary_error)
            .map(|value| value.into_iter().map(Into::into).collect())
    }

    async fn retry_dead_lettered(
        &self,
        request: auth_service::application::dto::deletion_dto::AuthRetryUserDeletionDto,
    ) -> auth_service::application::errors::AuthResult<
        Option<auth_service::application::dto::deletion_dto::AuthUserDeletionSagaRecordDto>,
    > {
        let request = auth_service::domain::user_deletion::RetryUserDeletion::from(request);

        let result: anyhow::Result<_> = async {
            let mut sagas = self.sagas.lock().unwrap();
            let Some(saga) = sagas.values_mut().find(|saga| saga.id == request.saga_id) else {
                return Ok(None);
            };
            if saga.status != UserDeletionSagaStatus::DeadLettered {
                return Err(auth_service::application::errors::AuthError::Conflict(
                    "Only a dead-lettered account deletion can be retried",
                )
                .into());
            }
            saga.status = UserDeletionSagaStatus::InProgress;
            for step in &mut saga.steps {
                if step.status != UserDeletionStepStatus::Succeeded {
                    step.status = UserDeletionStepStatus::Pending;
                    step.attempt_count = 0;
                    step.retry_generation += 1;
                }
            }
            self.retry_requests.lock().unwrap().push(request);
            Ok(Some(saga.clone()))
        }
        .await;
        result
            .map_err(auth_service::adapters::out::errors::boundary_error)
            .map(|value| value.map(Into::into))
    }

    async fn claim_due(
        &self,
        _now: chrono::DateTime<Utc>,
        _lease_timeout: Duration,
        limit: i32,
    ) -> auth_service::application::errors::AuthResult<
        Vec<auth_service::application::dto::deletion_dto::AuthUserDeletionStepRecordDto>,
    > {
        let result: anyhow::Result<Vec<UserDeletionStep>> = async {
            let mut due = self.due.lock().unwrap();
            Ok((0..limit.max(0)).filter_map(|_| due.pop_front()).collect())
        }
        .await;
        result
            .map_err(auth_service::adapters::out::errors::boundary_error)
            .map(|value| value.into_iter().map(Into::into).collect())
    }

    async fn mark_step_succeeded(
        &self,
        step_claim: &auth_service::application::dto::deletion_dto::AuthUserDeletionStepRecordDto,
        _now: chrono::DateTime<Utc>,
    ) -> auth_service::application::errors::AuthResult<
        auth_service::application::dto::deletion_dto::AuthUserDeletionSagaRecordDto,
    > {
        let step_claim =
            &auth_service::domain::user_deletion::UserDeletionStep::from(step_claim.clone());

        let result: anyhow::Result<_> = async {
            self.succeeded
                .lock()
                .unwrap()
                .push((step_claim.saga_id, step_claim.target));
            let mut sagas = self.sagas.lock().unwrap();
            let saga = sagas
                .values_mut()
                .find(|saga| saga.id == step_claim.saga_id)
                .unwrap();
            let step = saga
                .steps
                .iter_mut()
                .find(|step| step.target == step_claim.target)
                .unwrap();
            step.status = UserDeletionStepStatus::Succeeded;
            if saga
                .steps
                .iter()
                .all(|step| step.status == UserDeletionStepStatus::Succeeded)
            {
                saga.status = UserDeletionSagaStatus::Completed;
            }
            Ok(saga.clone())
        }
        .await;
        result
            .map_err(auth_service::adapters::out::errors::boundary_error)
            .map(|value| value.into())
    }

    async fn mark_step_failed(
        &self,
        step_claim: &auth_service::application::dto::deletion_dto::AuthUserDeletionStepRecordDto,
        error: &str,
        _next_attempt_at: chrono::DateTime<Utc>,
        dead_lettered: bool,
        _now: chrono::DateTime<Utc>,
    ) -> auth_service::application::errors::AuthResult<
        auth_service::application::dto::deletion_dto::AuthUserDeletionSagaRecordDto,
    > {
        let step_claim =
            &auth_service::domain::user_deletion::UserDeletionStep::from(step_claim.clone());

        let result: anyhow::Result<_> = async {
            self.failed.lock().unwrap().push((
                step_claim.saga_id,
                step_claim.target,
                error.to_owned(),
                dead_lettered,
            ));
            let mut sagas = self.sagas.lock().unwrap();
            let saga = sagas
                .values_mut()
                .find(|saga| saga.id == step_claim.saga_id)
                .unwrap();
            let step = saga
                .steps
                .iter_mut()
                .find(|step| step.target == step_claim.target)
                .unwrap();
            step.status = if dead_lettered {
                saga.status = UserDeletionSagaStatus::DeadLettered;
                UserDeletionStepStatus::DeadLettered
            } else {
                UserDeletionStepStatus::Retry
            };
            Ok(saga.clone())
        }
        .await;
        result
            .map_err(auth_service::adapters::out::errors::boundary_error)
            .map(|value| value.into())
    }
}

#[derive(Clone, Default)]
struct RecordingDispatcher {
    calls: Arc<Mutex<Vec<DispatchCall>>>,
    failures: Arc<Mutex<HashMap<UserDeletionTarget, usize>>>,
    observed_contexts: Arc<Mutex<Vec<Option<platform::trace_context::TraceContext>>>>,
}

#[async_trait]
impl UserDeletionDispatcherPort for RecordingDispatcher {
    async fn apply(
        &self,
        target: auth_service::application::dto::deletion_dto::AuthUserDeletionTargetDto,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: chrono::DateTime<Utc>,
    ) -> auth_service::application::errors::AuthResult<()> {
        let target = auth_service::domain::user_deletion::UserDeletionTarget::from(target);

        let result: anyhow::Result<_> = async {
            self.observed_contexts
                .lock()
                .unwrap()
                .push(platform::trace_context::current());
            self.calls
                .lock()
                .unwrap()
                .push((target, user_id, actor_id, retention_until, saga_id));
            let mut failures = self.failures.lock().unwrap();
            if let Some(remaining) = failures.get_mut(&target) {
                if *remaining > 0 {
                    *remaining -= 1;
                    anyhow::bail!("downstream unavailable");
                }
            }
            Ok(())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
}

fn saga(
    saga_id: Uuid,
    user_id: Uuid,
    actor_id: Uuid,
    retention_until: chrono::DateTime<Utc>,
) -> UserDeletionSaga {
    let now = Utc.with_ymd_and_hms(2026, 8, 3, 12, 0, 0).unwrap();
    UserDeletionSaga {
        id: saga_id,
        user_id,
        actor_id,
        status: UserDeletionSagaStatus::InProgress,
        retention_until,
        created_at: now,
        updated_at: now,
        completed_at: None,
        steps: UserDeletionTarget::ALL
            .into_iter()
            .map(|target| UserDeletionStep {
                saga_id,
                user_id,
                actor_id,
                target,
                status: UserDeletionStepStatus::Pending,
                attempt_count: 0,
                retry_generation: 0,
                next_attempt_at: now,
                locked_at: None,
                last_error: None,
                completed_at: None,
                retention_until,
                origin_traceparent: None,
            })
            .collect(),
    }
}

#[tokio::test]
async fn delete_user_starts_one_atomic_saga_with_eight_targets() {
    let users = crate::support::InMemoryUserRepository::default();
    let audits = crate::support::InMemoryAuthAuditRepository::default();
    let sagas = RecordingSagaRepository::default();
    let user_id = crate::support::seed_active_user(&users, "hana@example.com")
        .await
        .expect("seed");
    let use_case = DeleteUserUseCase::new(
        users,
        audits.clone(),
        sagas.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );

    let response = use_case
        .execute(DeleteUserDto {
            user_id: user_id.to_string(),
            actor_id: user_id.to_string(),
            request_id: Some("delete-request".into()),
            ip_address: Some("127.0.0.1".into()),
            user_agent: Some("phase2-test".into()),
        })
        .await
        .expect("start deletion");

    assert!(response.deleted);
    assert_eq!(response.saga_id, response.saga.id.to_string());
    assert_eq!(response.saga.steps.len(), UserDeletionTarget::ALL.len());
    {
        let requests = sagas.begin_requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].user_id, user_id);
        assert_eq!(requests[0].actor_id, user_id);
        assert_eq!(requests[0].targets, UserDeletionTarget::ALL.to_vec());
        assert_eq!(requests[0].audit.action, "USER_DELETION_REQUESTED");
        assert_eq!(
            requests[0].audit.request_id.as_deref(),
            Some("delete-request")
        );
    }
    assert!(audits.audits.lock().await.is_empty());
}

#[tokio::test]
async fn forbidden_delete_records_audit_without_starting_saga() {
    let users = crate::support::InMemoryUserRepository::default();
    let audits = crate::support::InMemoryAuthAuditRepository::default();
    let sagas = RecordingSagaRepository::default();
    let user_id = crate::support::seed_active_user(&users, "hana@example.com")
        .await
        .expect("seed");
    let actor_id = Uuid::new_v4();
    let use_case = DeleteUserUseCase::new(
        users,
        audits.clone(),
        sagas.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );

    let error = use_case
        .execute(DeleteUserDto {
            user_id: user_id.to_string(),
            actor_id: actor_id.to_string(),
            request_id: None,
            ip_address: None,
            user_agent: None,
        })
        .await
        .expect_err("forbidden");

    assert_eq!(error.to_string(), "Forbidden");
    assert!(sagas.begin_requests.lock().unwrap().is_empty());
    assert_eq!(audits.audits.lock().await.len(), 1);
}

#[tokio::test]
async fn get_user_deletion_returns_the_current_saga() {
    let repository = RecordingSagaRepository::default();
    let user_id = Uuid::new_v4();
    let actor_id = user_id;
    let saga_id = Uuid::new_v4();
    let retention = Utc::now() + Duration::days(30);
    repository
        .sagas
        .lock()
        .unwrap()
        .insert(user_id, saga(saga_id, user_id, actor_id, retention));

    let result = GetUserDeletionUseCase::new(repository)
        .execute(user_id)
        .await
        .expect("query")
        .expect("saga");

    assert_eq!(result.id, saga_id.to_string());
    assert_eq!(result.steps.len(), UserDeletionTarget::ALL.len());
}

#[tokio::test]
async fn dead_lettered_saga_pages_are_bounded_and_resume_after_updated_at_and_id() {
    let repository = RecordingSagaRepository::default();
    let actor_id = Uuid::new_v4();
    let retention = Utc::now() + Duration::days(30);
    let first_id = Uuid::from_u128(1);
    let second_id = Uuid::from_u128(2);
    let first_user = Uuid::new_v4();
    let second_user = Uuid::new_v4();
    let base_time = Utc.with_ymd_and_hms(2026, 9, 30, 12, 0, 0).unwrap();
    let mut first = saga(first_id, first_user, actor_id, retention);
    first.status = UserDeletionSagaStatus::DeadLettered;
    first.updated_at = base_time;
    let mut second = saga(second_id, second_user, actor_id, retention);
    second.status = UserDeletionSagaStatus::DeadLettered;
    second.updated_at = base_time + Duration::seconds(1);
    repository
        .sagas
        .lock()
        .unwrap()
        .extend([(first_user, first), (second_user, second)]);
    let use_case = ListDeadLetteredUserDeletionsUseCase::new(repository);

    let first_page = use_case.execute(1, None).await.expect("first page");
    assert_eq!(first_page.sagas.len(), 1);
    assert_eq!(first_page.sagas[0].id, first_id.to_string());
    let cursor = first_page.next_cursor.expect("next cursor");
    assert_eq!(cursor.updated_at, base_time);
    assert_eq!(cursor.saga_id, first_id.to_string());

    let second_page = use_case
        .execute(
            1,
            Some((cursor.updated_at, cursor.saga_id.parse().unwrap())),
        )
        .await
        .expect("second page");
    assert_eq!(second_page.sagas.len(), 1);
    assert_eq!(second_page.sagas[0].id, second_id.to_string());
    assert!(second_page.next_cursor.is_none());
}

#[tokio::test]
async fn worker_retries_only_failed_steps_and_completes_after_all_targets_succeed() {
    let repository = RecordingSagaRepository::default();
    let dispatcher = RecordingDispatcher::default();
    let user_id = Uuid::new_v4();
    let saga_id = Uuid::new_v4();
    let retention = Utc::now() + Duration::days(30);
    let model = saga(saga_id, user_id, user_id, retention);
    repository
        .sagas
        .lock()
        .unwrap()
        .insert(user_id, model.clone());
    repository.due.lock().unwrap().extend(model.steps.clone());
    dispatcher
        .failures
        .lock()
        .unwrap()
        .insert(UserDeletionTarget::Post, 1);
    let worker = UserDeletionWorker::new(
        repository.clone(),
        dispatcher.clone(),
        UserDeletionWorkerPolicy {
            batch_size: 10,
            max_attempts: 3,
            lease_timeout: Duration::minutes(5),
            base_retry: Duration::seconds(10),
        },
    );

    let first = worker.run_once(Utc::now()).await.expect("first run");
    assert_eq!(first.succeeded, 3);
    assert_eq!(first.retried, 1);
    assert_eq!(repository.succeeded.lock().unwrap().len(), 3);
    assert_eq!(repository.failed.lock().unwrap().len(), 1);

    let mut retry = model
        .steps
        .iter()
        .find(|step| step.target == UserDeletionTarget::Post)
        .unwrap()
        .clone();
    retry.attempt_count = 1;
    retry.status = UserDeletionStepStatus::Retry;
    repository.due.lock().unwrap().push_back(retry);

    let second = worker
        .run_once(Utc::now() + Duration::seconds(11))
        .await
        .expect("second run");
    assert_eq!(second.succeeded, 1);
    assert_eq!(second.retried, 0);
    assert_eq!(
        repository
            .sagas
            .lock()
            .unwrap()
            .get(&user_id)
            .unwrap()
            .status,
        UserDeletionSagaStatus::Completed
    );
}

#[tokio::test]
async fn worker_dead_letters_a_step_on_the_maximum_attempt() {
    let repository = RecordingSagaRepository::default();
    let dispatcher = RecordingDispatcher::default();
    let user_id = Uuid::new_v4();
    let saga_id = Uuid::new_v4();
    let retention = Utc::now() + Duration::days(30);
    let model = saga(saga_id, user_id, user_id, retention);
    repository
        .sagas
        .lock()
        .unwrap()
        .insert(user_id, model.clone());
    let mut step = model.steps[0].clone();
    step.attempt_count = 3;
    repository.due.lock().unwrap().push_back(step.clone());
    dispatcher.failures.lock().unwrap().insert(step.target, 1);
    let worker = UserDeletionWorker::new(
        repository.clone(),
        dispatcher,
        UserDeletionWorkerPolicy {
            batch_size: 10,
            max_attempts: 3,
            lease_timeout: Duration::minutes(5),
            base_retry: Duration::seconds(10),
        },
    );

    let result = worker.run_once(Utc::now()).await.expect("run");

    assert_eq!(result.dead_lettered, 1);
    assert!(repository.failed.lock().unwrap()[0].3);
    assert_eq!(
        repository
            .sagas
            .lock()
            .unwrap()
            .get(&user_id)
            .unwrap()
            .status,
        UserDeletionSagaStatus::DeadLettered
    );
}

#[tokio::test]
async fn worker_does_not_exhaust_the_limit_one_dispatch_early() {
    let repository = RecordingSagaRepository::default();
    let dispatcher = RecordingDispatcher::default();
    let user_id = Uuid::new_v4();
    let saga_id = Uuid::new_v4();
    let retention = Utc::now() + Duration::days(30);
    let model = saga(saga_id, user_id, user_id, retention);
    repository
        .sagas
        .lock()
        .unwrap()
        .insert(user_id, model.clone());
    let mut step = model.steps[0].clone();
    step.attempt_count = 2;
    step.status = UserDeletionStepStatus::InProgress;
    repository.due.lock().unwrap().push_back(step.clone());
    dispatcher.failures.lock().unwrap().insert(step.target, 1);
    let worker = UserDeletionWorker::new(
        repository.clone(),
        dispatcher,
        UserDeletionWorkerPolicy {
            batch_size: 10,
            max_attempts: 3,
            lease_timeout: Duration::minutes(5),
            base_retry: Duration::seconds(10),
        },
    );

    let result = worker.run_once(Utc::now()).await.expect("run");

    assert_eq!(result.retried, 1);
    assert_eq!(result.dead_lettered, 0);
    assert!(!repository.failed.lock().unwrap()[0].3);
}

#[tokio::test]
async fn operator_retry_restarts_only_incomplete_steps_in_a_new_generation() {
    let repository = RecordingSagaRepository::default();
    let user_id = Uuid::new_v4();
    let saga_id = Uuid::new_v4();
    let retention = Utc::now() + Duration::days(30);
    let mut model = saga(saga_id, user_id, user_id, retention);
    model.status = UserDeletionSagaStatus::DeadLettered;
    model.steps[0].status = UserDeletionStepStatus::Succeeded;
    model.steps[0].attempt_count = 2;
    model.steps[0].completed_at = Some(Utc::now());
    model.steps[1].status = UserDeletionStepStatus::DeadLettered;
    model.steps[1].attempt_count = 8;
    model.steps[1].retry_generation = 0;
    model.steps[1].last_error = Some("provider unavailable".into());
    repository
        .sagas
        .lock()
        .unwrap()
        .insert(user_id, model.clone());
    let use_case = RetryUserDeletionUseCase::new(
        repository.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    let resolution_id = Uuid::new_v4();

    let response = use_case
        .execute(
            auth_service::application::dto::auth_dto::RetryUserDeletionDto {
                saga_id: saga_id.to_string(),
                resolution_id: resolution_id.to_string(),
                actor_id: Uuid::new_v4().to_string(),
                reason: "Reviewed the outage and approved a fresh retry".into(),
                evidence_reference: "INC-2026-0912".into(),
            },
        )
        .await
        .expect("retry saga");

    assert!(response.accepted);
    assert_eq!(response.resolution_id, resolution_id.to_string());
    assert_eq!(response.saga.status, "IN_PROGRESS");
    assert_eq!(response.saga.steps[0].status, "SUCCEEDED");
    assert_eq!(response.saga.steps[0].attempt_count, 2);
    assert_eq!(response.saga.steps[1].status, "PENDING");
    assert_eq!(response.saga.steps[1].attempt_count, 0);
    let retried = repository.sagas.lock().unwrap();
    let retried = retried.get(&user_id).expect("saga");
    assert_eq!(retried.steps[1].retry_generation, 1);
    assert_eq!(repository.retry_requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn operator_retry_rejects_nil_saga_resolution_and_actor_ids() {
    let repository = RecordingSagaRepository::default();
    let user_id = Uuid::new_v4();
    let saga_id = Uuid::new_v4();
    let mut model = saga(saga_id, user_id, user_id, Utc::now() + Duration::days(30));
    model.status = UserDeletionSagaStatus::DeadLettered;
    model.steps[1].status = UserDeletionStepStatus::DeadLettered;
    repository.sagas.lock().unwrap().insert(user_id, model);
    let use_case = RetryUserDeletionUseCase::new(
        repository.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    let valid_resolution_id = Uuid::new_v4();
    let valid_actor_id = Uuid::new_v4();
    let request = |saga_id: Uuid, resolution_id: Uuid, actor_id: Uuid| {
        auth_service::application::dto::auth_dto::RetryUserDeletionDto {
            saga_id: saga_id.to_string(),
            resolution_id: resolution_id.to_string(),
            actor_id: actor_id.to_string(),
            reason: "Reviewed the outage and approved a fresh retry".into(),
            evidence_reference: "INC-2026-0912".into(),
        }
    };

    for request in [
        request(Uuid::nil(), valid_resolution_id, valid_actor_id),
        request(saga_id, Uuid::nil(), valid_actor_id),
        request(saga_id, valid_resolution_id, Uuid::nil()),
    ] {
        assert!(use_case.execute(request).await.is_err());
    }
    assert!(repository.retry_requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn deletion_worker_reestablishes_a_child_trace_context_for_dispatch() {
    const PARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

    let repository = RecordingSagaRepository::default();
    let user_id = Uuid::new_v4();
    let saga_id = Uuid::new_v4();
    let retention_until = Utc::now() + Duration::days(30);
    let request_context = platform::trace_context::TraceContext::child_from_traceparent(PARENT)
        .expect("valid request trace context");
    let model = saga(saga_id, user_id, user_id, retention_until);
    let mut step = model.steps[0].clone();
    step.origin_traceparent = Some(request_context.traceparent());
    repository.due.lock().unwrap().push_back(step);
    repository
        .sagas
        .lock()
        .unwrap()
        .insert(user_id, model.clone());

    let dispatcher = RecordingDispatcher::default();
    let worker = UserDeletionWorker::new(
        repository,
        dispatcher.clone(),
        UserDeletionWorkerPolicy {
            batch_size: 10,
            max_attempts: 3,
            lease_timeout: Duration::minutes(5),
            base_retry: Duration::seconds(1),
        },
    );

    let result = worker
        .run_once(Utc::now())
        .await
        .expect("dispatch deletion step");
    assert_eq!(result.succeeded, 1);
    let observed = dispatcher
        .observed_contexts
        .lock()
        .unwrap()
        .first()
        .cloned()
        .flatten()
        .expect("worker restored trace context for dispatcher");
    assert_eq!(observed.trace_id(), request_context.trace_id());
    assert_eq!(observed.parent_span_id(), Some(request_context.span_id()));
    assert_ne!(observed.span_id(), request_context.span_id());
}
