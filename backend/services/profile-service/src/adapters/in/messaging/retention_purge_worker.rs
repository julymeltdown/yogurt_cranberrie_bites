use crate::application::dto::profile_dto::PurgeExpiredProfilesDto;
use crate::application::ports::ProfileRepositoryPort;
use crate::application::use_cases::purge_expired_profiles::PurgeExpiredProfilesUseCase;
use crate::config::ProfileMetrics;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct ProfileRetentionPurgeWorker<R> {
    purge_expired_profiles: PurgeExpiredProfilesUseCase<R>,
    actor_id: String,
    metrics: Arc<ProfileMetrics>,
}

impl<R> ProfileRetentionPurgeWorker<R>
where
    R: ProfileRepositoryPort,
{
    pub fn new(
        purge_expired_profiles: PurgeExpiredProfilesUseCase<R>,
        actor_id: String,
        metrics: Arc<ProfileMetrics>,
    ) -> Self {
        Self {
            purge_expired_profiles,
            actor_id,
            metrics,
        }
    }

    pub async fn tick(&self) -> Result<i64> {
        let response = self
            .purge_expired_profiles
            .execute(PurgeExpiredProfilesDto {
                actor_id: self.actor_id.clone(),
                request_id: Some("profile-retention-worker".into()),
                ip_address: None,
                user_agent: Some("profile-retention-worker".into()),
                dry_run: false,
            })
            .await;

        match response {
            Ok(response) => {
                self.metrics.record_retention_purge_run("success");
                self.metrics
                    .record_retention_purged_profiles("success", response.purged_count);
                Ok(response.purged_count)
            }
            Err(error) => {
                self.metrics.record_retention_purge_run("failure");
                self.metrics.record_retention_purged_profiles("failure", 0);
                Err(error.into())
            }
        }
    }

    pub async fn run(self, interval: Duration) -> Result<()> {
        loop {
            if let Err(error) = self.tick().await {
                tracing::error!(error = ?error, "profile retention purge worker tick failed");
            }
            tokio::time::sleep(interval).await;
        }
    }
}
