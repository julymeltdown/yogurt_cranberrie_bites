//! Configured, bounded recovery of durable finished-root children. This worker
//! owns no content authority and cannot recreate a lost in-memory scope.
pub use super::scope_ledger::ChildRecoveryCounts;
use super::scope_ledger::{ChildRecoveryCursor, PgScopeLedger};
use std::time::Duration;
use tonic::Status;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChildRecoverySettings {
    batch_size: u16,
    per_child_timeout: Duration,
    batch_budget: Duration,
    interval: Duration,
}
impl Default for ChildRecoverySettings {
    fn default() -> Self {
        Self {
            batch_size: 16,
            per_child_timeout: Duration::from_millis(500),
            batch_budget: Duration::from_secs(2),
            interval: Duration::from_secs(1),
        }
    }
}
impl ChildRecoverySettings {
    pub fn new(
        batch_size: u16,
        per_child_timeout: Duration,
        batch_budget: Duration,
        interval: Duration,
    ) -> Result<Self, Status> {
        if !(1..=202).contains(&batch_size)
            || !(Duration::from_millis(50)..=Duration::from_secs(2)).contains(&per_child_timeout)
            || !(Duration::from_millis(50)..=Duration::from_secs(10)).contains(&batch_budget)
            || per_child_timeout > batch_budget
            || !(Duration::from_millis(100)..=Duration::from_secs(60)).contains(&interval)
        {
            return Err(settings_error());
        }
        Ok(Self {
            batch_size,
            per_child_timeout,
            batch_budget,
            interval,
        })
    }
    /// Missing values use explicit defaults; malformed/out-of-range settings
    /// fail startup. No setting selects a child kind, endpoint, instance or key.
    pub fn from_env() -> Result<Self, Status> {
        fn value(name: &str, default: u64) -> Result<u64, Status> {
            match std::env::var(name) {
                Ok(value) if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) => {
                    value.parse().map_err(|_| settings_error())
                }
                Err(std::env::VarError::NotPresent) => Ok(default),
                _ => Err(settings_error()),
            }
        }
        Self::new(
            value("CONCERT_CHILD_RECOVERY_BATCH_SIZE", 16)?
                .try_into()
                .map_err(|_| settings_error())?,
            Duration::from_millis(value("CONCERT_CHILD_RECOVERY_CHILD_TIMEOUT_MS", 500)?),
            Duration::from_millis(value("CONCERT_CHILD_RECOVERY_BATCH_BUDGET_MS", 2_000)?),
            Duration::from_millis(value("CONCERT_CHILD_RECOVERY_INTERVAL_MS", 1_000)?),
        )
    }
    pub fn interval(&self) -> Duration {
        self.interval
    }
}
fn settings_error() -> Status {
    Status::invalid_argument("Invalid child recovery worker settings")
}

pub struct ChildRecoveryWorker {
    ledger: PgScopeLedger,
    settings: ChildRecoverySettings,
    cursor: ChildRecoveryCursor,
}
impl ChildRecoveryWorker {
    /// Pass the SAME configured ledger used by the root registry. Its instance,
    /// retained keyring and real typed settlers are preserved by the clone.
    pub fn new(ledger: PgScopeLedger, settings: ChildRecoverySettings) -> Result<Self, Status> {
        let settings = ChildRecoverySettings::new(
            settings.batch_size,
            settings.per_child_timeout,
            settings.batch_budget,
            settings.interval,
        )?;
        Ok(Self {
            ledger,
            settings,
            cursor: ChildRecoveryCursor::default(),
        })
    }
    /// Each tick gets a fresh SYSTEM work budget, never renewed actor/content
    /// authority. Concurrent workers rely on actual idempotent child settlement
    /// and immutable SQL ACKs; no transaction is held across remote work.
    pub async fn tick(&mut self) -> Result<ChildRecoveryCounts, Status> {
        self.ledger
            .drain_finished_children(
                &mut self.cursor,
                self.settings.batch_size,
                self.settings.per_child_timeout,
                tokio::time::Instant::now() + self.settings.batch_budget,
            )
            .await
    }
    /// Attach to the service supervisor. Shutdown denies a new cycle and lets
    /// the current bounded cycle finish; dependency failures retry next cycle.
    pub async fn run(mut self) -> Result<(), Status> {
        let shutdown = platform::shutdown::global();
        let mut interval = tokio::time::interval(self.settings.interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        while shutdown.tick(&mut interval).await {
            match self.tick().await {
                Ok(counts) if counts.attempted > 0 => eprintln!(
                    "child_recovery attempted={} completed={} pending={} timed_out={}",
                    counts.attempted, counts.completed, counts.pending, counts.timed_out
                ),
                Ok(_) => (),
                Err(_) => eprintln!("child_recovery scan_unavailable"),
            }
        }
        Ok(())
    }
}
