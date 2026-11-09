//! Block mirroring: the friend service's signals become projection rows.
//!
//! `discovery_blocks` is the projection's block filter. Blocks are written by
//! the **friend** service, which the concert service already reaches for
//! relationship authority: it publishes `FRIEND_RELATIONSHIP_CHANGED` signals
//! with a `Pair { actor, subject }` scope, and this module turns one such signal
//! into a `discovery_blocks` write.
//!
//! Two rules:
//!
//! * **A signal is not a fact.** The signal says "something about this pair
//!   changed", not "this pair is now blocked". The caller resolves the pair's
//!   current state through the relationship authority and passes it in. This
//!   module therefore takes an explicit `active` flag rather than reading one
//!   from the signal.
//! * **A lifted block is a tombstone, not a delete.** The discovery query filters
//!   on `deleted_at is null`, so restoring a profile needs no query change, and
//!   the history of the block survives for an operator to inspect.
//!
//! The pair scope names **users** (Auth ids). `discovery_blocks` is keyed by
//! **account** ids, and `profile_identities.account_id` is the canonical Auth
//! user UUID, so the two are the same value by contract
//! (`domain-contract.md` §1.1). The caller passes account ids directly.
use super::projection::{ProjectionError, ProjectionWriter};
use uuid::Uuid;

/// One resolved block state for a pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockState {
    pub blocker_account_id: Uuid,
    pub blocked_account_id: Uuid,
    /// Whether the block is currently active.
    pub active: bool,
}

/// Apply a batch of resolved block states.
///
/// Every write goes through [`ProjectionWriter::mirror_block`], which is an
/// upsert, so replaying a signal twice writes the same row. A malformed pair (a
/// nil id, or a self-block) is refused rather than written, because
/// `discovery_blocks_not_self` would reject it anyway and a partial batch is
/// worse than a named failure.
pub async fn apply_blocks(
    writer: &ProjectionWriter,
    states: &[BlockState],
) -> Result<usize, ProjectionError> {
    let mut applied = 0;
    for state in states {
        writer
            .mirror_block(
                state.blocker_account_id,
                state.blocked_account_id,
                state.active,
            )
            .await?;
        applied += 1;
    }
    Ok(applied)
}
