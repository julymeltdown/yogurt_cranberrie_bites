//! Supervised BE15 receipt retirement and terminal outbox pruning.
//!
//! Bounded maintenance only: every round uses the retained repository limits
//! (1000 rows) and DB clock. It never claims, acknowledges or redelivers a
//! command invalidation; family delivery workers keep their own sealed claims.
use crate::adapters::out::repository::{outbox::PgOutbox, receipt::PgCommandRepository};
use concert_application::commands::CommandError;
use sqlx::PgPool;
use std::time::Duration;

const RETIRE_LIMIT: i64 = 1000;
const PRUNE_LIMIT: i64 = 1000;
const TICK: Duration = Duration::from_secs(30);

#[derive(Default, Debug, PartialEq, Eq)]
pub struct MaintenanceRound {
    pub retired: u64,
    pub pruned: u64,
}

pub struct ReceiptMaintenance {
    pool: PgPool,
}
impl ReceiptMaintenance {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    /// Expired results become RETIRED tombstones before terminal outbox
    /// diagnostics are pruned. Pruning retains every receipt tombstone.
    pub async fn run_round(&self) -> Result<MaintenanceRound, CommandError> {
        let retired = PgCommandRepository::new(self.pool.clone())
            .retire_expired(RETIRE_LIMIT)
            .await?;
        let pruned = PgOutbox::new(self.pool.clone()).prune(PRUNE_LIMIT).await?;
        Ok(MaintenanceRound { retired, pruned })
    }
    /// Production supervision entry point. A storage failure stops the worker
    /// rather than being reported as successful maintenance.
    pub async fn run_until(
        self,
        stop: impl std::future::Future<Output = ()>,
    ) -> Result<(), CommandError> {
        tokio::pin!(stop);
        let mut tick = tokio::time::interval(TICK);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                biased;
                _ = &mut stop => return Ok(()),
                _ = tick.tick() => {
                    self.run_round().await?;
                }
            }
        }
    }
}
