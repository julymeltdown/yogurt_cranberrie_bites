use crate::application::dto::post_dto::PurgeExpiredPostsDto;
use crate::application::use_cases::purge_expired_posts::PurgeExpiredPostsUseCase;
use crate::config::PostMetrics;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct PostRetentionPurgeWorker {
    purge_expired_posts: PurgeExpiredPostsUseCase,
    metrics: Arc<PostMetrics>,
    actor_id: String,
}

impl PostRetentionPurgeWorker {
    pub fn new(
        purge_expired_posts: PurgeExpiredPostsUseCase,
        metrics: Arc<PostMetrics>,
        actor_id: String,
    ) -> Self {
        Self {
            purge_expired_posts,
            metrics,
            actor_id,
        }
    }

    pub async fn tick(&self) -> Result<i64> {
        match self
            .purge_expired_posts
            .execute(PurgeExpiredPostsDto {
                actor_id: self.actor_id.clone(),
                request_id: Some("post-retention-worker".into()),
                ip_address: None,
                user_agent: Some("post-retention-worker".into()),
                dry_run: false,
            })
            .await
        {
            Ok(response) => {
                self.metrics.record_retention_purge_run("success");
                self.metrics
                    .record_retention_purged_posts("success", response.purged_post_count);
                self.metrics
                    .record_retention_purged_comments("success", response.purged_comment_count);
                Ok(response.purged_count)
            }
            Err(error) => {
                self.metrics.record_retention_purge_run("failure");
                self.metrics.record_retention_purged_posts("failure", 0);
                self.metrics.record_retention_purged_comments("failure", 0);
                Err(error)
            }
        }
    }

    pub async fn run(self, interval: Duration) -> Result<()> {
        loop {
            if let Err(error) = self.tick().await {
                tracing::error!(error = ?error, "post retention purge worker tick failed");
            }
            tokio::time::sleep(interval).await;
        }
    }
}
