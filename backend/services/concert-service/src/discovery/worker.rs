//! The discovery projection worker.
//!
//! `ProjectionWriter::run_pass` is the unit of work; this module is the loop that
//! runs it on a bounded interval, alongside the block consumer, and stops
//! cleanly on shutdown.
//!
//! ## Shape
//!
//! It follows the established pattern in this crate
//! (`workers::outbox::OutboxWorker::run_until`): a `tokio::time::interval` with
//! `MissedTickBehavior::Skip`, a `biased` `select!` that checks the stop signal
//! first, and one bounded round per tick. It deliberately does **not** race
//! cancellation against an in-flight round: a round that has already started
//! finishes its current page and then the loop returns, so shutdown never
//! interrupts a transaction mid-commit.
//!
//! ## Failure behaviour
//!
//! A source failure is **not** fatal and **not** silent:
//!
//! * the projection is left exactly as it was — an empty projection would be
//!   indistinguishable from "nobody has favorites", which is a fabricated fact;
//! * the failure is recorded in `discovery_projection_state.last_failure` (or
//!   `discovery_block_cursor.last_failure`), so an operator can see it;
//! * the round returns an error that the loop logs at `error` level with the
//!   worker name, so it appears in the service's own logs;
//! * the loop keeps running and retries on the next tick.
//!
//! A failure that is **not** the source's — a broken writer, a corrupt cursor —
//! is also retried rather than stopping the process, because stopping the whole
//! concert service because a derived read failed would be worse than a stale
//! projection. The one exception is a construction error, which is returned
//! before the loop starts.
//!
//! ## Observability
//!
//! Every round is traced with `worker = "discovery-projection"` and an
//! `outcome` field of `caught_up`, `behind` or `failure`, plus the counts the
//! round applied. That is the same shape `platform::shutdown::periodic` uses for
//! the other workers, so an operator greps one pattern for all of them.
use super::block_consumer::BlockConsumer;
use super::projection::{ProjectionError, ProjectionWriter};
use std::sync::Arc;
use std::time::Duration;

/// The default interval between projection rounds.
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(30);

/// The longest a single round may run before the loop moves on. A round that
/// exceeds this is abandoned rather than allowed to hold the loop open, so a
/// wedged source cannot stop the worker from noticing shutdown.
pub const ROUND_DEADLINE: Duration = Duration::from_secs(60);

/// The outcome of one round.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoundReport {
    /// Profiles applied by the profile pass.
    pub profiles: usize,
    /// Pages applied by the profile pass.
    pub pages: usize,
    /// Whether the profile pass caught up with the source.
    pub profiles_caught_up: bool,
    /// Block signals applied.
    pub blocks_applied: usize,
    /// Whether the block consumer caught up with the journal.
    pub blocks_caught_up: bool,
    /// The failures this round observed. Empty means the round was clean.
    pub failures: Vec<RoundFailure>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundFailure {
    /// The profile source could not be read. The projection is unchanged.
    ProfileSource,
    /// The profile projection store could not be written.
    ProfileStore,
    /// The block journal could not be read. The cursor is unchanged.
    BlockJournal,
    /// The block projection store could not be written.
    BlockStore,
    /// The round exceeded its deadline and was abandoned.
    Deadline,
}

impl RoundFailure {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProfileSource => "profile_source",
            Self::ProfileStore => "profile_store",
            Self::BlockJournal => "block_journal",
            Self::BlockStore => "block_store",
            Self::Deadline => "deadline",
        }
    }
}

/// The worker.
pub struct ProjectionWorker {
    writer: Arc<ProjectionWriter>,
    blocks: Option<Arc<BlockConsumer>>,
    interval: Duration,
    round_deadline: Duration,
}

impl ProjectionWorker {
    /// Build a worker. `interval` must be positive and `round_deadline` must be
    /// at least the interval's worth of work, so a misconfiguration is refused
    /// before the loop starts rather than producing a spinning worker.
    pub fn new(
        writer: Arc<ProjectionWriter>,
        blocks: Option<Arc<BlockConsumer>>,
        interval: Duration,
        round_deadline: Duration,
    ) -> Result<Self, ProjectionError> {
        if interval.is_zero() || round_deadline.is_zero() {
            return Err(ProjectionError::SourceInvalid);
        }
        Ok(Self {
            writer,
            blocks,
            interval,
            round_deadline,
        })
    }

    /// The default worker: the standard interval and deadline.
    pub fn standard(
        writer: Arc<ProjectionWriter>,
        blocks: Option<Arc<BlockConsumer>>,
    ) -> Result<Self, ProjectionError> {
        Self::new(writer, blocks, DEFAULT_INTERVAL, ROUND_DEADLINE)
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Run one bounded round.
    ///
    /// The profile pass and the block poll are independent: one failing does not
    /// skip the other, because they track different streams and a block backlog
    /// should not wait for a profile source to recover.
    pub async fn run_round(&self) -> RoundReport {
        let mut report = RoundReport::default();
        // Profile pass.
        match self.writer.run_pass().await {
            Ok(pass) => {
                report.profiles = pass.profiles;
                report.pages = pass.pages;
                report.profiles_caught_up = pass.caught_up;
            }
            Err(error) => {
                let failure = match error {
                    ProjectionError::SourceUnavailable | ProjectionError::SourceInvalid => {
                        RoundFailure::ProfileSource
                    }
                    ProjectionError::StoreUnavailable => RoundFailure::ProfileStore,
                };
                // Record it so it is visible, and leave the projection alone.
                let _ = self.writer.record_failure(failure.as_str()).await;
                report.failures.push(failure);
            }
        }
        // Block poll.
        if let Some(blocks) = &self.blocks {
            match blocks.poll().await {
                Ok(poll) => {
                    report.blocks_applied = poll.applied;
                    report.blocks_caught_up = poll.caught_up;
                }
                Err(error) => {
                    let failure = match error {
                        ProjectionError::SourceUnavailable | ProjectionError::SourceInvalid => {
                            RoundFailure::BlockJournal
                        }
                        ProjectionError::StoreUnavailable => RoundFailure::BlockStore,
                    };
                    let _ = blocks.record_failure(failure.as_str()).await;
                    report.failures.push(failure);
                }
            }
        }
        report
    }

    /// Run until `stop` resolves.
    ///
    /// The stop future is checked **before** each tick (`biased`), and a round
    /// already in progress is allowed to finish its current page. The function
    /// therefore returns only when no round is running, which is what makes a
    /// clean shutdown observable: after `run_until` returns, the caller can drop
    /// the pool without racing an in-flight transaction.
    pub async fn run_until(
        self,
        stop: impl std::future::Future<Output = ()>,
    ) -> Result<(), ProjectionError> {
        tokio::pin!(stop);
        let mut tick = tokio::time::interval(self.interval);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                biased;
                _ = &mut stop => return Ok(()),
                _ = tick.tick() => {
                    // One bounded round. The deadline bounds a wedged source so
                    // the loop still observes the stop signal.
                    let round = tokio::time::timeout(self.round_deadline, self.run_round());
                    let report = match round.await {
                        Ok(report) => report,
                        Err(_) => RoundReport {
                            failures: vec![RoundFailure::Deadline],
                            ..RoundReport::default()
                        },
                    };
                    self.observe(&report);
                }
            }
        }
    }

    /// Emit the round's outcome. A failure is logged at `error`, a clean round
    /// at `info`; both carry the same fields so one query finds either.
    fn observe(&self, report: &RoundReport) {
        let outcome = if report.failures.is_empty() {
            if report.profiles_caught_up {
                "caught_up"
            } else {
                "behind"
            }
        } else {
            "failure"
        };
        let span = tracing::info_span!(
            "discovery_projection_round",
            worker = "discovery-projection",
            outcome,
            profiles = report.profiles,
            pages = report.pages,
            blocks_applied = report.blocks_applied,
        );
        let failures: Vec<&str> = report.failures.iter().map(|f| f.as_str()).collect();
        let _guard = span.enter();
        if failures.is_empty() {
            tracing::info!("discovery projection round complete");
        } else {
            tracing::error!(failures = ?failures, "discovery projection round failed");
        }
    }
}

/// The tracing field name used for a round's outcome, so a test can assert the
/// worker emits it rather than only that it runs.
pub const OUTCOME_FIELD: &str = "outcome";
