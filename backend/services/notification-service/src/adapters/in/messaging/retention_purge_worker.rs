use crate::application::dto::notification_dto::PurgeExpiredNotificationsDto;
use crate::application::notification_purge::NotificationPurgePort;
use crate::application::use_cases::purge_expired_notifications::PurgeExpiredNotificationsUseCase;
use crate::config::NotificationMetrics;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct NotificationRetentionPurgeWorker<R> {
    purge_expired_notifications: PurgeExpiredNotificationsUseCase<R>,
    actor_id: String,
    metrics: Arc<NotificationMetrics>,
}

impl<R> NotificationRetentionPurgeWorker<R>
where
    R: NotificationPurgePort,
{
    pub fn new(
        purge_expired_notifications: PurgeExpiredNotificationsUseCase<R>,
        actor_id: String,
        metrics: Arc<NotificationMetrics>,
    ) -> Self {
        Self {
            purge_expired_notifications,
            actor_id,
            metrics,
        }
    }

    pub async fn tick(&self) -> Result<i64> {
        let response = self
            .purge_expired_notifications
            .execute(PurgeExpiredNotificationsDto {
                actor_id: self.actor_id.clone(),
                request_id: Some("notification-retention-worker".into()),
                ip_address: None,
                user_agent: Some("notification-retention-worker".into()),
                dry_run: false,
            })
            .await;

        match response {
            Ok(response) => {
                self.metrics.record_retention_purge_run("success");
                self.metrics
                    .record_retention_purged_notifications("success", response.purged_count);
                Ok(response.purged_count)
            }
            Err(error) => {
                self.metrics.record_retention_purge_run("failure");
                self.metrics
                    .record_retention_purged_notifications("failure", 0);
                Err(error.into())
            }
        }
    }

    pub async fn run(self, interval: Duration) -> Result<()> {
        loop {
            if let Err(error) = self.tick().await {
                tracing::error!(error = ?error, "notification retention purge worker tick failed");
            }
            tokio::time::sleep(interval).await;
        }
    }
}
