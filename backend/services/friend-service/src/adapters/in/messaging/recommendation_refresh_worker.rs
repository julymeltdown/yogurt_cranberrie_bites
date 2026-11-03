use crate::application::ports::PeopleRecommendationRepositoryPort;
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use platform::recommendation_metrics::{
    RecommendationMetricOutcome, RecommendationMetricStage, RecommendationMetricSurface,
    RecommendationMetrics,
};
use std::sync::Arc;
use std::time::{Duration as StdDuration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RecommendationRefreshTickResult {
    pub claimed: i64,
    pub succeeded: i64,
    pub failed: i64,
}

#[derive(Clone)]
pub struct RecommendationRefreshWorker<R> {
    repository: R,
    model_version: String,
    lease_timeout: Duration,
    batch_size: i32,
    max_attempts: i32,
    base_retry: Duration,
    metrics: Arc<RecommendationMetrics>,
}

impl<R> RecommendationRefreshWorker<R>
where
    R: PeopleRecommendationRepositoryPort,
{
    pub fn new(
        repository: R,
        model_version: String,
        lease_timeout: Duration,
        batch_size: i32,
        max_attempts: i32,
        base_retry: Duration,
    ) -> Self {
        Self {
            repository,
            model_version,
            lease_timeout,
            batch_size: batch_size.clamp(1, 500),
            max_attempts: max_attempts.clamp(1, 20),
            base_retry: base_retry.max(Duration::milliseconds(100)),
            metrics: Arc::new(RecommendationMetrics::default()),
        }
    }

    pub fn with_metrics(mut self, metrics: Arc<RecommendationMetrics>) -> Self {
        self.metrics = metrics;
        self
    }

    pub async fn tick(&self, now: DateTime<Utc>) -> Result<RecommendationRefreshTickResult> {
        let started = Instant::now();
        let result = self.tick_inner(now).await;
        self.metrics.record_refresh_run(
            RecommendationMetricSurface::People,
            if result.is_ok() {
                RecommendationMetricOutcome::Success
            } else {
                RecommendationMetricOutcome::Failure
            },
        );
        if let Ok(result) = &result {
            self.metrics.record_refresh_viewers(
                RecommendationMetricSurface::People,
                RecommendationMetricOutcome::Success,
                result.succeeded,
            );
            self.metrics.record_refresh_viewers(
                RecommendationMetricSurface::People,
                RecommendationMetricOutcome::Failure,
                result.failed,
            );
        }
        self.metrics.observe_stage_latency(
            RecommendationMetricSurface::People,
            RecommendationMetricStage::CandidateCollection,
            started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        );
        result
    }

    async fn tick_inner(&self, now: DateTime<Utc>) -> Result<RecommendationRefreshTickResult> {
        let claims = self
            .repository
            .claim_refresh_batch(now, self.lease_timeout, self.batch_size)
            .await?;
        let mut result = RecommendationRefreshTickResult {
            claimed: claims.len() as i64,
            ..RecommendationRefreshTickResult::default()
        };
        for claim in claims {
            let refreshed = self
                .repository
                .refresh_candidates(
                    claim.viewer_id,
                    now,
                    now + Duration::hours(24),
                    &self.model_version,
                    2_000,
                )
                .await;
            match refreshed {
                Ok(_) => {
                    self.repository.complete_refresh(claim).await?;
                    result.succeeded += 1;
                }
                Err(error) => {
                    self.repository
                        .fail_refresh(
                            claim,
                            &error.to_string(),
                            now + self.base_retry,
                            self.max_attempts,
                        )
                        .await?;
                    result.failed += 1;
                }
            }
        }
        Ok(result)
    }

    pub async fn run(&self, interval: StdDuration) -> Result<()> {
        let mut ticker = tokio::time::interval(interval.max(StdDuration::from_millis(100)));
        loop {
            ticker.tick().await;
            if let Err(error) = self.tick(Utc::now()).await {
                tracing::error!(error = ?error, "people recommendation refresh tick failed");
            }
        }
    }
}
