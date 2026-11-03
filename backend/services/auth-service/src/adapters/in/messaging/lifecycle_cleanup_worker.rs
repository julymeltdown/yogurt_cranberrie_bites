use crate::application::dto::auth_dto::AuthLifecycleCleanupResult;
use crate::application::ports::AuthLifecycleCleanupPort;
use crate::application::use_cases::lifecycle_cleanup::AuthLifecycleCleanupUseCase;
use crate::config::AuthMetrics;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct AuthLifecycleCleanupWorker<R> {
    cleanup: AuthLifecycleCleanupUseCase<R>,
    metrics: Arc<AuthMetrics>,
}

impl<R> AuthLifecycleCleanupWorker<R>
where
    R: AuthLifecycleCleanupPort,
{
    pub fn new(cleanup: AuthLifecycleCleanupUseCase<R>, metrics: Arc<AuthMetrics>) -> Self {
        Self { cleanup, metrics }
    }

    pub async fn tick(&self) -> Result<AuthLifecycleCleanupResult> {
        match self.cleanup.execute().await {
            Ok(result) => {
                self.metrics.record_lifecycle_cleanup_run("success");
                self.metrics.record_lifecycle_cleanup_deleted_records(
                    "email_verification",
                    "success",
                    result.deleted_email_verifications,
                );
                self.metrics.record_lifecycle_cleanup_deleted_records(
                    "password_reset",
                    "success",
                    result.deleted_password_reset_tokens,
                );
                self.metrics.record_lifecycle_cleanup_deleted_records(
                    "refresh_token",
                    "success",
                    result.deleted_refresh_tokens,
                );
                self.metrics.record_lifecycle_cleanup_deleted_records(
                    "oauth_authorization",
                    "success",
                    result.deleted_oauth_authorizations,
                );
                self.metrics.record_lifecycle_cleanup_deleted_records(
                    "user",
                    "success",
                    result.deleted_users,
                );
                Ok(result)
            }
            Err(error) => {
                self.metrics.record_lifecycle_cleanup_run("failure");
                Err(error.into())
            }
        }
    }

    pub async fn run(self, interval: Duration) -> Result<()> {
        platform::shutdown::periodic("auth-lifecycle-cleanup", interval, || self.tick()).await
    }
}
