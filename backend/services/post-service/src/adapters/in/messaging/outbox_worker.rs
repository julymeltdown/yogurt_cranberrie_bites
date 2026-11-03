use crate::adapters::r#in::messaging::external_event_forwarder::ExternalEventForwarder;
use crate::config::PostMetrics;
use crate::domain::post::DeliveryTarget;
use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct PostOutboxWorker {
    forwarder: ExternalEventForwarder,
    metrics: Arc<PostMetrics>,
}

impl PostOutboxWorker {
    pub fn new(forwarder: ExternalEventForwarder, metrics: Arc<PostMetrics>) -> Self {
        Self { forwarder, metrics }
    }

    pub async fn tick_target(
        &self,
        target: DeliveryTarget,
        older_than: chrono::DateTime<Utc>,
        limit: i32,
    ) -> Result<i64> {
        match self
            .forwarder
            .republish_target(target, older_than, limit)
            .await
        {
            Ok(published) => {
                self.metrics.record_outbox_worker_run("success");
                self.metrics
                    .record_outbox_worker_forwarded("success", published);
                Ok(published)
            }
            Err(error) => {
                self.metrics.record_outbox_worker_run("failure");
                self.metrics.record_outbox_worker_forwarded("failure", 0);
                Err(error)
            }
        }
    }

    pub async fn tick(&self, older_than: chrono::DateTime<Utc>, limit: i32) -> Result<i64> {
        match self
            .forwarder
            .republish_unpublished(older_than, limit)
            .await
        {
            Ok(forwarded) => {
                self.metrics.record_outbox_worker_run("success");
                self.metrics
                    .record_outbox_worker_forwarded("success", forwarded);
                Ok(forwarded)
            }
            Err(error) => {
                self.metrics.record_outbox_worker_run("failure");
                self.metrics.record_outbox_worker_forwarded("failure", 0);
                Err(error)
            }
        }
    }

    pub async fn run(
        self,
        target: DeliveryTarget,
        interval: Duration,
        min_event_age: Duration,
        limit: i32,
    ) -> Result<()> {
        loop {
            let older_than = Utc::now() - chrono::Duration::from_std(min_event_age)?;
            if let Err(error) = self.tick_target(target, older_than, limit).await {
                tracing::error!(error = ?error, "post outbox worker tick failed");
            }
            tokio::time::sleep(interval).await;
        }
    }
}
