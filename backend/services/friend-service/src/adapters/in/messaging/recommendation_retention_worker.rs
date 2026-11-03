use crate::application::ports::PeopleRecommendationMaintenancePort;
use crate::domain::recommendation::PeopleRecommendationRetentionResult;
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use platform::recommendation_metrics::{
    RecommendationEntity, RecommendationMetricOutcome, RecommendationMetricStage,
    RecommendationMetricSurface, RecommendationMetrics,
};
use std::sync::Arc;
use std::time::{Duration as StdDuration, Instant};

#[derive(Clone)]
pub struct PeopleRecommendationRetentionWorker<R> {
    repository: R,
    metrics: Arc<RecommendationMetrics>,
    event_retention: Duration,
    batch_size: i32,
}

impl<R> PeopleRecommendationRetentionWorker<R>
where
    R: PeopleRecommendationMaintenancePort,
{
    pub fn new(
        repository: R,
        metrics: Arc<RecommendationMetrics>,
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

    pub async fn tick(&self, now: DateTime<Utc>) -> Result<PeopleRecommendationRetentionResult> {
        let started = Instant::now();
        match self
            .repository
            .purge_expired(now, now - self.event_retention, self.batch_size)
            .await
        {
            Ok(result) => {
                self.metrics.record_retention_run(
                    RecommendationMetricSurface::People,
                    RecommendationMetricOutcome::Success,
                );
                for (entity, count) in result.metric_rows() {
                    self.metrics.record_retention(
                        RecommendationMetricSurface::People,
                        entity,
                        RecommendationMetricOutcome::Success,
                        count,
                    );
                }
                self.metrics.observe_stage_latency(
                    RecommendationMetricSurface::People,
                    RecommendationMetricStage::Retention,
                    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                );
                Ok(result)
            }
            Err(error) => {
                self.metrics.record_retention_run(
                    RecommendationMetricSurface::People,
                    RecommendationMetricOutcome::Failure,
                );
                Err(error)
            }
        }
    }

    pub async fn run(self, interval: StdDuration) -> Result<()> {
        let mut ticker = tokio::time::interval(interval.max(StdDuration::from_secs(1)));
        loop {
            ticker.tick().await;
            if let Err(error) = self.tick(Utc::now()).await {
                tracing::error!(error = ?error, "people recommendation retention tick failed");
            }
        }
    }
}

trait RetentionMetricRows {
    fn metric_rows(self) -> [(RecommendationEntity, i64); 8];
}

impl RetentionMetricRows for PeopleRecommendationRetentionResult {
    fn metric_rows(self) -> [(RecommendationEntity, i64); 8] {
        [
            (RecommendationEntity::Candidate, self.candidates),
            (RecommendationEntity::SnapshotItem, self.snapshot_items),
            (RecommendationEntity::Impression, self.impressions),
            (RecommendationEntity::Snapshot, self.snapshots),
            (RecommendationEntity::Event, self.events),
            (
                RecommendationEntity::ExperimentAssignment,
                self.experiment_assignments,
            ),
            (RecommendationEntity::Embedding, self.embeddings),
            (RecommendationEntity::RefreshJob, self.refresh_jobs),
        ]
    }
}
