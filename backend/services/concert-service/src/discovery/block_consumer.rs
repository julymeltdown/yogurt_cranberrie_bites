//! The block consumer: friend invalidation signals become projection rows.
//!
//! `discovery_blocks` is the projection's block filter. Blocks are written by the
//! **friend** service, which publishes `FRIEND_RELATIONSHIP_CHANGED` signals with
//! a `Pair { actor, subject }` scope on its invalidation journal. This module
//! reads that journal, resolves each pair to its current block state through the
//! relationship authority, and feeds the result to the projection.
//!
//! ## Why a signal is not a fact
//!
//! A `FRIEND_RELATIONSHIP_CHANGED` signal says *something about this pair
//! changed*. It does **not** say the pair is now blocked: the same signal is
//! emitted for a block, an unblock, a follow, an unfollow and a mute. Treating
//! the signal itself as "blocked" would mirror every follow as a block.
//!
//! So the consumer resolves the pair through a [`PairAuthority`] port, which
//! returns the pair's current state, and only then writes the projection. The
//! production implementation reads the friend service's relationship authority;
//! the tests use an in-memory authority so the consumer's behaviour can be
//! exercised without a second service.
//!
//! ## Cursor discipline
//!
//! The journal is replayable. The consumer advances its durable cursor **only
//! after** the projection write for that page commits, so a crash re-reads the
//! same signals. Re-applying a page is harmless because every projection write
//! is an upsert keyed by the account pair.
//!
//! ## Why the pair is resolved per signal rather than batched
//!
//! A page can name the same pair twice (a block then an unblock). Resolving each
//! signal in order and writing after each one means the projection ends at the
//! *last* state the journal describes, which is what a replay should produce. A
//! batched resolve-and-write would need to dedupe by pair and could apply an
//! older state last.
use super::blocks::BlockState;
use super::projection::{ProjectionError, ProjectionWriter};
use crate::adapters::out::relationship_authority::invalidation::{
    InvalidationOwner, InvalidationSource, OwnerCursor, ReplayBudget, SignalReason, SignalScope,
};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// The most signals one poll applies. The journal's own page cap is 256; this is
/// the consumer's budget for one tick, so a large backlog is worked through over
/// several ticks rather than in one unbounded transaction.
pub const MAX_SIGNALS_PER_POLL: u16 = 128;

/// The per-operation deadline for one journal read.
pub const POLL_DEADLINE: Duration = Duration::from_secs(2);

/// The pair's current block state, as the authority reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairState {
    /// A block is active in at least one direction.
    Blocked,
    /// No block is active.
    Unblocked,
    /// The authority could not answer. The consumer must **not** treat this as
    /// "unblocked": that would silently restore a blocked profile.
    Unknown,
}

/// Resolves a pair to its current block state.
///
/// The production implementation asks the friend service's relationship
/// authority. The port exists so the consumer's ordering, cursor and failure
/// behaviour can be tested without a second service.
#[async_trait::async_trait]
pub trait PairAuthority: Send + Sync {
    async fn block_state(&self, actor: Uuid, subject: Uuid) -> Result<PairState, PairFailure>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairFailure {
    /// The authority is unreachable.
    Unavailable,
    /// The authority refused the read.
    Denied,
}

impl std::fmt::Display for PairFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "pair authority failure: {self:?}")
    }
}

impl std::error::Error for PairFailure {}

/// The journal read port, so the consumer can be tested without the transport.
#[async_trait::async_trait]
pub trait BlockJournal: Send + Sync {
    /// Read up to `limit` signals after `after`.
    async fn read_after(
        &self,
        after: OwnerCursor,
        limit: u16,
    ) -> Result<JournalPage, PairFailure>;
}

/// One page of the journal, reduced to what the block consumer needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalPage {
    pub next: OwnerCursor,
    pub has_more: bool,
    /// `(reason, scope)` pairs, in journal order.
    pub signals: Vec<(SignalReason, SignalScope)>,
}

/// The production journal: the friend service's invalidation source.
pub struct FriendJournal {
    source: InvalidationSource,
    consumer: Uuid,
}

impl FriendJournal {
    pub fn new(source: InvalidationSource, consumer: Uuid) -> Result<Self, PairFailure> {
        if source.owner() != InvalidationOwner::Friend || consumer.is_nil() {
            return Err(PairFailure::Denied);
        }
        Ok(Self { source, consumer })
    }
}

#[async_trait::async_trait]
impl BlockJournal for FriendJournal {
    async fn read_after(
        &self,
        after: OwnerCursor,
        limit: u16,
    ) -> Result<JournalPage, PairFailure> {
        let budget = ReplayBudget::for_duration(POLL_DEADLINE, Default::default())
            .map_err(|_| PairFailure::Unavailable)?;
        let page = self
            .source
            .read_after(self.consumer, after, limit, &budget)
            .await
            .map_err(|_| PairFailure::Unavailable)?;
        Ok(JournalPage {
            next: page.next(),
            has_more: page.has_more(),
            signals: page
                .signals()
                .iter()
                .map(|signal| (signal.reason(), signal.scope()))
                .collect(),
        })
    }
}

/// What one poll did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BlockPollReport {
    /// Signals read from the journal.
    pub signals: usize,
    /// Pair signals that resolved to a state and were applied.
    pub applied: usize,
    /// Signals skipped because they were not pair-scoped relationship changes.
    pub skipped: usize,
    /// Pairs whose authority could not answer. The cursor does **not** advance
    /// past these.
    pub unresolved: usize,
    /// Whether the consumer caught up with the journal head.
    pub caught_up: bool,
}

/// The consumer.
pub struct BlockConsumer {
    pool: PgPool,
    journal: Arc<dyn BlockJournal>,
    authority: Arc<dyn PairAuthority>,
    writer: Arc<ProjectionWriter>,
    consumer: Uuid,
}

impl BlockConsumer {
    pub fn new(
        pool: PgPool,
        journal: Arc<dyn BlockJournal>,
        authority: Arc<dyn PairAuthority>,
        writer: Arc<ProjectionWriter>,
        consumer: Uuid,
    ) -> Result<Self, ProjectionError> {
        if consumer.is_nil() {
            return Err(ProjectionError::SourceInvalid);
        }
        Ok(Self {
            pool,
            journal,
            authority,
            writer,
            consumer,
        })
    }

    /// The durable cursor, or `None` before the first read.
    pub async fn cursor(&self) -> Result<Option<OwnerCursor>, ProjectionError> {
        let row = sqlx::query(
            "select epoch, sequence from discovery_block_cursor where consumer_id=$1",
        )
        .bind(self.consumer)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        use sqlx::Row;
        let epoch: Option<Uuid> = row.try_get("epoch")?;
        let sequence: Option<i64> = row.try_get("sequence")?;
        match (epoch, sequence) {
            (Some(epoch), Some(sequence)) => Ok(Some(
                OwnerCursor::new(InvalidationOwner::Friend, epoch, sequence)
                    .map_err(|_| ProjectionError::SourceInvalid)?,
            )),
            _ => Ok(None),
        }
    }

    /// Persist the cursor. Called only after the projection write committed.
    async fn store_cursor(&self, cursor: OwnerCursor) -> Result<(), ProjectionError> {
        sqlx::query(
            "insert into discovery_block_cursor(consumer_id,epoch,sequence,last_failure) \
             values($1,$2,$3,null) \
             on conflict (consumer_id) do update set epoch=excluded.epoch, \
               sequence=excluded.sequence, last_failure=null",
        )
        .bind(self.consumer)
        .bind(cursor.epoch())
        .bind(cursor.sequence())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Record a failure without advancing the cursor.
    pub async fn record_failure(&self, message: &str) -> Result<(), ProjectionError> {
        let bounded: String = message
            .chars()
            .filter(|character| !character.is_control())
            .take(400)
            .collect();
        let bounded = if bounded.is_empty() {
            "block journal unavailable".to_owned()
        } else {
            bounded
        };
        sqlx::query(
            "insert into discovery_block_cursor(consumer_id,last_failure) values($1,$2) \
             on conflict (consumer_id) do update set last_failure=excluded.last_failure",
        )
        .bind(self.consumer)
        .bind(bounded)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Read one page and apply it.
    ///
    /// The cursor advances only past signals that were applied. If any pair's
    /// authority cannot answer, the cursor stops **before** that signal so the
    /// next poll retries it, and the report says so.
    pub async fn poll(&self) -> Result<BlockPollReport, ProjectionError> {
        let Some(mut after) = self.cursor().await? else {
            // The first read needs a cursor. The journal's head is that cursor:
            // starting from the beginning of a retained journal would replay
            // history the projection does not need, and starting from "now"
            // would skip signals. Reading the head and then reading after it is
            // the only position that is both complete and bounded.
            return Err(ProjectionError::SourceInvalid);
        };
        let mut report = BlockPollReport::default();
        let page = self
            .journal
            .read_after(after, MAX_SIGNALS_PER_POLL)
            .await
            .map_err(|_| ProjectionError::SourceUnavailable)?;
        report.signals = page.signals.len();
        for (reason, scope) in &page.signals {
            // Only a pair-scoped relationship change can affect a block.
            let (actor, subject) = match (reason, scope) {
                (SignalReason::FriendRelationshipChanged, SignalScope::Pair { actor, subject }) => {
                    (*actor, *subject)
                }
                _ => {
                    report.skipped += 1;
                    continue;
                }
            };
            let state = match self.authority.block_state(actor, subject).await {
                Ok(PairState::Blocked) => true,
                Ok(PairState::Unblocked) => false,
                // An authority that cannot answer must not be read as
                // "unblocked": that would silently restore a blocked profile.
                Ok(PairState::Unknown) | Err(_) => {
                    report.unresolved += 1;
                    // Stop here so the cursor does not advance past a signal
                    // whose effect was not applied.
                    report.caught_up = false;
                    return Ok(report);
                }
            };
            self.writer
                .mirror_block(actor, subject, state)
                .await?;
            report.applied += 1;
            // Advance only after the write succeeded.
            after = OwnerCursor::new(
                InvalidationOwner::Friend,
                after.epoch(),
                after.sequence() + 1,
            )
            .map_err(|_| ProjectionError::SourceInvalid)?;
        }
        // The page's own `next` is authoritative for the position; use it when
        // every signal in the page was consumed.
        if report.unresolved == 0 {
            after = page.next;
            self.store_cursor(after).await?;
            report.caught_up = !page.has_more;
        }
        Ok(report)
    }
}

/// The initial cursor for a fresh consumer: the journal's current head.
///
/// Exposed so composition can seed the cursor at startup rather than having the
/// consumer replay a retained journal from its beginning.
pub async fn seed_cursor(
    pool: &PgPool,
    consumer: Uuid,
    head: OwnerCursor,
) -> Result<(), ProjectionError> {
    if consumer.is_nil() || head.owner() != InvalidationOwner::Friend {
        return Err(ProjectionError::SourceInvalid);
    }
    sqlx::query(
        "insert into discovery_block_cursor(consumer_id,epoch,sequence) values($1,$2,$3) \
         on conflict (consumer_id) do nothing",
    )
    .bind(consumer)
    .bind(head.epoch())
    .bind(head.sequence())
    .execute(pool)
    .await?;
    Ok(())
}

/// A [`BlockState`] for one resolved pair, for callers that already have the
/// authority's answer.
pub fn block_state(actor: Uuid, subject: Uuid, blocked: bool) -> BlockState {
    BlockState {
        blocker_account_id: actor,
        blocked_account_id: subject,
        active: blocked,
    }
}
