use crate::application::ports::FeedRecommendationMaintenancePort;
use crate::config::PostMetrics;
use crate::domain::recommendation::FeedRecommendationRetentionResult;
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use platform::recommendation_metrics::{
    RecommendationEntity, RecommendationMetricOutcome, RecommendationMetricStage,
};
use std::sync::Arc;
use std::time::{Duration as StdDuration, Instant};

#[derive(Clone)]
pub struct FeedRecommendationRetentionWorker<R> {
    repository: R,
    metrics: Arc<PostMetrics>,
    event_retention: Duration,
    batch_size: i32,
}

impl<R> FeedRecommendationRetentionWorker<R>
where
    R: FeedRecommendationMaintenancePort,
{
    pub fn new(
        repository: R,
        metrics: Arc<PostMetrics>,
        event_retention: Duration,
        batch_size: i32,
    ) -> Self {
        Self {
            repository,
            metrics,
            event_retention: event_retention.max(Duration::days(1)),
            batch_size: batch_size.clamp(1, 5_000),
        }
    }

    pub async fn tick(&self, now: DateTime<Utc>) -> Result<FeedRecommendationRetentionResult> {
        let started = Instant::now();
        match self
            .repository
            .purge_expired(now, now - self.event_retention, self.batch_size)
            .await
        {
            Ok(result) => {
                self.metrics
                    .record_recommendation_retention_run(RecommendationMetricOutcome::Success);
                for (entity, count) in result.metric_rows() {
                    self.metrics.record_recommendation_retention(
                        entity,
                        RecommendationMetricOutcome::Success,
                        count,
                    );
                }
                self.metrics.observe_recommendation_stage_latency(
                    RecommendationMetricStage::Retention,
                    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                );
                Ok(result)
            }
            Err(error) => {
                self.metrics
                    .record_recommendation_retention_run(RecommendationMetricOutcome::Failure);
                Err(error)
            }
        }
    }

    pub async fn run(self, interval: StdDuration) -> Result<()> {
        let mut ticker = tokio::time::interval(interval.max(StdDuration::from_secs(1)));
        loop {
            ticker.tick().await;
            if let Err(error) = self.tick(Utc::now()).await {
                tracing::error!(error = ?error, "feed recommendation retention tick failed");
            }
        }
    }
}

trait RetentionMetricRows {
    fn metric_rows(self) -> [(RecommendationEntity, i64); 9];
}

impl RetentionMetricRows for FeedRecommendationRetentionResult {
    fn metric_rows(self) -> [(RecommendationEntity, i64); 9] {
        [
            (RecommendationEntity::TimelineEntry, self.timeline_entries),
            (RecommendationEntity::Candidate, self.high_fanout_posts),
            (RecommendationEntity::SnapshotItem, self.snapshot_items),
            (RecommendationEntity::Impression, self.impressions),
            (RecommendationEntity::Snapshot, self.snapshots),
            (RecommendationEntity::Event, self.events),
            (
                RecommendationEntity::ExperimentAssignment,
                self.experiment_assignments,
            ),
            (RecommendationEntity::Embedding, self.embeddings),
            (RecommendationEntity::FanoutJob, self.fanout_jobs),
        ]
    }
}
