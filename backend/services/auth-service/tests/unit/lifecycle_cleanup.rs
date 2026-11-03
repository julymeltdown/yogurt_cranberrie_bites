use async_trait::async_trait;
use auth_service::adapters::r#in::messaging::lifecycle_cleanup_worker::AuthLifecycleCleanupWorker;
use auth_service::application::dto::auth_dto::AuthLifecycleCleanupResult;
use auth_service::application::ports::AuthLifecycleCleanupPort;
use auth_service::application::use_cases::lifecycle_cleanup::AuthLifecycleCleanupUseCase;
use auth_service::config::AuthMetrics;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn execute_delegates_cleanup_with_injected_time() {
    let repository = RecordingCleanupRepository::with_result(cleanup_result());
    let use_case = AuthLifecycleCleanupUseCase::new(
        repository.clone(),
        Arc::new(crate::support::controlled_runtime::ControlledRuntime::new(
            "2070-02-03T04:05:06Z".parse().unwrap(),
        )),
    );

    let result = use_case.execute().await.expect("cleanup succeeds");

    assert_eq!(result, cleanup_result());
    let calls = repository.calls.lock().await;
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0],
        "2070-02-03T04:05:06Z".parse::<DateTime<Utc>>().unwrap()
    );
}

#[tokio::test]
async fn execute_returns_repository_error() {
    let repository = RecordingCleanupRepository::with_error("cleanup storage unavailable");
    let use_case = AuthLifecycleCleanupUseCase::new(
        repository,
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );

    let error = use_case.execute().await.expect_err("cleanup fails");

    assert_eq!(
        error,
        auth_service::application::errors::AuthError::Unexpected
    );
}

#[tokio::test]
async fn tick_records_success_metrics_for_each_deleted_record_type() {
    let repository = RecordingCleanupRepository::with_result(cleanup_result());
    let metrics = Arc::new(AuthMetrics::default());
    let worker = AuthLifecycleCleanupWorker::new(
        AuthLifecycleCleanupUseCase::new(
            repository,
            std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
        ),
        metrics.clone(),
    );

    let result = worker.tick().await.expect("cleanup tick succeeds");

    assert_eq!(result, cleanup_result());
    let rendered = metrics.render_prometheus();
    assert!(rendered.contains("auth_lifecycle_cleanup_runs_total{outcome=\"success\"} 1\n"));
    assert!(rendered.contains(
        "auth_lifecycle_cleanup_deleted_records_total{record_type=\"email_verification\",outcome=\"success\"} 2\n"
    ));
    assert!(rendered.contains(
        "auth_lifecycle_cleanup_deleted_records_total{record_type=\"password_reset\",outcome=\"success\"} 3\n"
    ));
    assert!(rendered.contains(
        "auth_lifecycle_cleanup_deleted_records_total{record_type=\"refresh_token\",outcome=\"success\"} 5\n"
    ));
    assert!(rendered.contains(
        "auth_lifecycle_cleanup_deleted_records_total{record_type=\"oauth_authorization\",outcome=\"success\"} 7\n"
    ));
    assert!(rendered.contains(
        "auth_lifecycle_cleanup_deleted_records_total{record_type=\"user\",outcome=\"success\"} 11\n"
    ));
}

#[tokio::test]
async fn tick_records_failure_without_deleted_record_metrics() {
    let repository = RecordingCleanupRepository::with_error("database write failed");
    let metrics = Arc::new(AuthMetrics::default());
    let worker = AuthLifecycleCleanupWorker::new(
        AuthLifecycleCleanupUseCase::new(
            repository,
            std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
        ),
        metrics.clone(),
    );

    let error = worker.tick().await.expect_err("cleanup tick fails");

    assert!(matches!(
        error.downcast_ref::<auth_service::application::errors::AuthError>(),
        Some(auth_service::application::errors::AuthError::Unexpected)
    ));
    let rendered = metrics.render_prometheus();
    assert!(rendered.contains("auth_lifecycle_cleanup_runs_total{outcome=\"failure\"} 1\n"));
    assert!(!rendered.contains("auth_lifecycle_cleanup_deleted_records_total"));
}

#[derive(Clone)]
struct RecordingCleanupRepository {
    calls: Arc<Mutex<Vec<DateTime<Utc>>>>,
    outcome: CleanupOutcome,
}

impl RecordingCleanupRepository {
    fn with_result(result: AuthLifecycleCleanupResult) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            outcome: CleanupOutcome::Result(result),
        }
    }

    fn with_error(message: &str) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            outcome: CleanupOutcome::Error(message.to_owned()),
        }
    }
}

#[async_trait]
impl AuthLifecycleCleanupPort for RecordingCleanupRepository {
    async fn cleanup_expired(
        &self,
        now: DateTime<Utc>,
    ) -> auth_service::application::errors::AuthResult<AuthLifecycleCleanupResult> {
        let result: anyhow::Result<_> = async {
            self.calls.lock().await.push(now);
            match &self.outcome {
                CleanupOutcome::Result(result) => Ok(result.clone()),
                CleanupOutcome::Error(message) => anyhow::bail!(message.clone()),
            }
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
}

#[derive(Clone)]
enum CleanupOutcome {
    Result(AuthLifecycleCleanupResult),
    Error(String),
}

fn cleanup_result() -> AuthLifecycleCleanupResult {
    AuthLifecycleCleanupResult {
        deleted_email_verifications: 2,
        deleted_password_reset_tokens: 3,
        deleted_refresh_tokens: 5,
        deleted_oauth_authorizations: 7,
        deleted_users: 11,
    }
}
