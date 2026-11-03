//! Explicit one-batch runner; composition does not start this automatically.
use crate::adapters::out::repository::outbox::{Delivery, DeliveryFailure, PgOutbox};
use async_trait::async_trait;
use concert_application::commands::CommandError;
/// Port accepts only safe invalidation metadata. Implementations must durably
/// deduplicate delivery.id atomically with their own local effect, reauthorize
/// before any sensitive projection, and bound network deadlines below the lease.
/// Never log credentials or downstream raw errors. A crash after external success
/// before acknowledgement WILL redeliver; this is not exactly-once delivery.
#[async_trait]
pub trait IdempotentInvalidationSink: Send + Sync {
    async fn deliver(&self, delivery: &Delivery) -> Result<(), DeliveryFailure>;
}
#[derive(Default, Debug, PartialEq, Eq)]
pub struct BatchResult {
    pub claimed: usize,
    pub acknowledged: usize,
    pub failed: usize,
    pub stale: usize,
}
pub async fn run_batch<S: IdempotentInvalidationSink + 'static>(
    outbox: &PgOutbox,
    sink: std::sync::Arc<S>,
    limit: i64,
) -> Result<BatchResult, CommandError> {
    let deliveries = outbox.claim(limit).await?;
    let mut result = BatchResult {
        claimed: deliveries.len(),
        ..BatchResult::default()
    };
    // Concurrent, bounded tasks keep the ten-second delivery deadline below the
    // fixed lease. JoinSet aborts outstanding work if this batch is cancelled.
    let mut tasks = tokio::task::JoinSet::new();
    for delivery in deliveries {
        let sink = sink.clone();
        let outbox = outbox.clone();
        tasks.spawn(async move {
            let outcome =
                tokio::time::timeout(std::time::Duration::from_secs(10), sink.deliver(&delivery))
                    .await
                    .unwrap_or(Err(DeliveryFailure::Retryable));
            match outcome {
                Ok(()) => outbox.acknowledge(&delivery).await.map(|ok| (ok, true)),
                Err(failure) => outbox.fail(&delivery, failure).await.map(|ok| (ok, false)),
            }
        });
    }
    while let Some(outcome) = tasks.join_next().await {
        let (accepted, success) = outcome.map_err(|_| CommandError::Unavailable)??;
        if !accepted {
            result.stale += 1;
        } else if success {
            result.acknowledged += 1;
        } else {
            result.failed += 1;
        }
    }
    Ok(result)
}

/// Opt-in only: World application is one fanout leg, never a global queue ack.
/// `remaining` must apply every other required feature/owner leg durably. An
/// unavailable/unbound leg must return Retryable rather than claim completion.
/// No constructor here starts a worker or qualifies production startup.
pub struct WorldCompositeSink<S> {
    world: std::sync::Arc<crate::world::invalidation::WorldInvalidationConsumer>,
    remaining: std::sync::Arc<S>,
}
impl<S: IdempotentInvalidationSink> WorldCompositeSink<S> {
    pub fn new(
        world: std::sync::Arc<crate::world::invalidation::WorldInvalidationConsumer>,
        remaining: std::sync::Arc<S>,
    ) -> Self {
        Self { world, remaining }
    }
}
#[async_trait]
impl<S: IdempotentInvalidationSink> IdempotentInvalidationSink for WorldCompositeSink<S> {
    async fn deliver(&self, delivery: &Delivery) -> Result<(), DeliveryFailure> {
        self.world
            .apply(delivery)
            .await
            .map_err(DeliveryFailure::from)?;
        self.remaining.deliver(delivery).await
    }
}
