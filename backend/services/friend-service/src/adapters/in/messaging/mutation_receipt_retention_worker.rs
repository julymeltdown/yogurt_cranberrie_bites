use crate::application::dto::mutation_dto::PurgeExpiredMutationOutcomesDto;
use crate::application::use_cases::purge_expired_mutation_outcomes::PurgeExpiredMutationOutcomesUseCase;
use crate::config::retention_metrics::MutationReceiptRetentionMetrics;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct MutationReceiptRetentionWorker<R> {
    purge_expired_outcomes: PurgeExpiredMutationOutcomesUseCase<R>,
    metrics: Arc<MutationReceiptRetentionMetrics>,
    batch_size: i32,
}

impl<R> MutationReceiptRetentionWorker<R>
where
    R: crate::application::ports::MutationReceiptMaintenancePort,
{
    pub fn new(
        purge_expired_outcomes: PurgeExpiredMutationOutcomesUseCase<R>,
        metrics: Arc<MutationReceiptRetentionMetrics>,
        batch_size: i32,
    ) -> Self {
        Self {
            purge_expired_outcomes,
            metrics,
            batch_size: batch_size.clamp(1, 5_000),
        }
    }

    pub async fn tick(&self) -> Result<i64> {
        match self
            .purge_expired_outcomes
            .execute(PurgeExpiredMutationOutcomesDto {
                batch_size: self.batch_size,
            })
            .await
        {
            Ok(purged) => {
                self.metrics.record_success(purged);
                Ok(purged)
            }
            Err(error) => {
                self.metrics.record_failure();
                Err(error)
            }
        }
    }

    pub async fn run(self, interval: Duration) -> Result<()> {
        loop {
            if let Err(error) = self.tick().await {
                tracing::error!(error = ?error, "friend mutation receipt retention tick failed");
            }
            tokio::time::sleep(interval.max(Duration::from_secs(1))).await;
        }
    }
}
