use crate::application::use_cases::process_timeline_fanout::ProcessTimelineFanoutUseCase;
use crate::config::PostMetrics;
use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct TimelineFanoutWorker {
    use_case: ProcessTimelineFanoutUseCase,
    metrics: Arc<PostMetrics>,
}

impl TimelineFanoutWorker {
    pub fn new(use_case: ProcessTimelineFanoutUseCase, metrics: Arc<PostMetrics>) -> Self {
        Self { use_case, metrics }
    }

    pub async fn tick(&self) -> Result<i64> {
        match self.use_case.execute(Utc::now()).await {
            Ok(processed) => {
                self.metrics.record_timeline_fanout_run("success");
                self.metrics
                    .record_timeline_fanout_pages("success", processed);
                Ok(processed)
            }
            Err(error) => {
                self.metrics.record_timeline_fanout_run("failure");
                self.metrics.record_timeline_fanout_pages("failure", 0);
                Err(error)
            }
        }
    }

    pub async fn run(self, interval: Duration) -> Result<()> {
        loop {
            if let Err(error) = self.tick().await {
                tracing::error!(error = ?error, "timeline fanout worker tick failed");
            }
            tokio::time::sleep(interval).await;
        }
    }
}
