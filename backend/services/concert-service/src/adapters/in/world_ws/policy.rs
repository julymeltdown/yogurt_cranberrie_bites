//! Private Social withdrawal metadata on the incumbent World transport.
//! The baseline port must be supplied by the canonical Social owner. The default
//! World transport has no baseline and grants no private Social availability.
use super::{handoff, Binding, Message, SafeClose, Slot, SocketTransport, WriteError};
use async_trait::async_trait;
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

const MAX_REVISION: i64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug)]
pub struct PolicyUnavailable;

/// A held, canonical generation observation. Commit consumes the database guard;
/// it does not authorize a private read or imply that a frame was transmitted.
#[async_trait]
pub trait HeldPolicyBaseline: Send {
    fn revision(&self) -> i64;
    async fn commit(self: Box<Self>) -> Result<(), PolicyUnavailable>;
}

#[async_trait]
pub trait PolicyBaselinePort: Send + Sync {
    async fn held_baseline(
        &self,
        event: Uuid,
        actor: Uuid,
    ) -> Result<Box<dyn HeldPolicyBaseline>, PolicyUnavailable>;
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PolicyReason {
    SourceChanged,
    ConsentChanged,
    BlockChanged,
    Expired,
    CanonicalPolicyChanged,
}

/// This handle shares only the incumbent socket registry. It does not retain
/// the baseline owner, so composing owner -> publisher cannot create an Arc cycle.
#[derive(Clone)]
pub struct WorldPolicyPublisher {
    transport: SocketTransport,
}

#[derive(Default)]
pub(super) struct PolicyState {
    stream: Option<Uuid>,
    pub(super) snapshot_sent: bool,
    floor: i64,
    pending: Option<i64>,
    committed: bool,
    attempted: bool,
    baseline_sent: bool,
}

impl PolicyState {
    pub(super) fn installed(&mut self, b: &Binding) {
        // A duplicate local install must never reopen a withdrawn generation.
        if self.stream != Some(b.stream_id) {
            *self = Self {
                stream: Some(b.stream_id),
                ..Self::default()
            };
        }
    }

    fn prepare(&mut self, revision: i64) -> Result<(), WriteError> {
        if self.attempted || !(1..=MAX_REVISION).contains(&revision) || revision <= self.floor {
            return Err(WriteError::Failed);
        }
        self.attempted = true;
        self.floor = revision;
        self.pending = Some(revision);
        self.committed = false;
        Ok(())
    }

    fn finish(&mut self, revision: i64, committed: bool) {
        // A publisher crossing SQL commit and this local arm wins permanently.
        if self.pending == Some(revision) && self.floor == revision {
            self.committed = committed;
            if !committed {
                self.pending = None;
            }
        }
    }

    fn invalidate(&mut self, revision: i64) -> Result<bool, WriteError> {
        if !(1..=MAX_REVISION).contains(&revision) {
            return Err(WriteError::Failed);
        }
        if revision <= self.floor {
            return Ok(false);
        }
        self.floor = revision;
        self.pending = None;
        self.committed = false;
        // Before the first baseline there is no private view to withdraw. The
        // pending view is fenced locally and cannot later be armed by a stale commit.
        Ok(self.baseline_sent)
    }

    fn baseline(&self) -> Option<i64> {
        (self.snapshot_sent && self.committed && !self.baseline_sent)
            .then_some(self.pending)
            .flatten()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Frame {
    protocol_version: &'static str,
    #[serde(rename = "type")]
    kind: &'static str,
    stream_id: Uuid,
    event_id: Uuid,
    scope: &'static str,
    policy_revision: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<PolicyReason>,
}

fn frame(b: &Binding, revision: i64, reason: Option<PolicyReason>) -> Result<Message, WriteError> {
    let value = Frame {
        protocol_version: "1.0.0",
        kind: if reason.is_some() {
            "policy.invalidate"
        } else {
            "policy.baseline"
        },
        stream_id: b.stream_id,
        event_id: b.key.event_id.uuid(),
        scope: "SOCIAL",
        policy_revision: revision,
        reason,
    };
    serde_json::to_string(&value)
        .map(|bytes| Message::Text(bytes.into()))
        .map_err(|_| WriteError::Failed)
}

pub(super) fn snapshot_matches(b: &Binding, bytes: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return false;
    };
    value.get("type").and_then(|v| v.as_str()) == Some("snapshot")
        && value.get("streamId").and_then(|v| v.as_str()) == Some(b.stream_id.to_string().as_str())
        && value.get("streamSeq").and_then(|v| v.as_u64()) == Some(0)
}

pub(super) fn flush_baseline(slot: &mut Slot, b: &Binding) -> Result<(), WriteError> {
    let Some(revision) = slot.policy.baseline() else {
        return Ok(());
    };
    let result = handoff(slot, frame(b, revision, None)?);
    if result.is_ok() {
        slot.policy.baseline_sent = true;
    } else {
        slot.policy.pending = None;
        slot.policy.committed = false;
        // An uncertain native write cannot be acknowledged as private withdrawal.
        slot.terminal.get_or_insert(SafeClose::UNAVAILABLE);
    }
    result
}

impl SocketTransport {
    /// Configure before cloning the transport into the single World owner.
    pub fn with_policy_baseline(mut self, owner: Arc<dyn PolicyBaselinePort>) -> Self {
        self.policy_baseline = Some(owner);
        self
    }

    pub fn policy_publisher(&self) -> WorldPolicyPublisher {
        WorldPolicyPublisher {
            transport: Self {
                slots: self.slots.clone(),
                policy_baseline: None,
            },
        }
    }

    pub(super) fn prepare_policy(&self, b: &Binding, revision: i64) -> Result<(), WriteError> {
        let slot = self.slot(b.connection_id)?;
        let mut slot = slot.lock().map_err(|_| WriteError::Failed)?;
        if slot.binding.as_ref() != Some(b) || slot.terminal.is_some() {
            return Err(WriteError::Failed);
        }
        slot.policy.prepare(revision)
    }

    pub(super) fn finish_policy(&self, b: &Binding, revision: i64, committed: bool) {
        if let Ok(slot) = self.slot(b.connection_id) {
            if let Ok(mut slot) = slot.lock() {
                if slot.binding.as_ref() == Some(b) && slot.terminal.is_none() {
                    slot.policy.finish(revision, committed);
                    let _ = flush_baseline(&mut slot, b);
                }
            }
        }
    }
}

impl WorldPolicyPublisher {
    /// Source-owner adapter checks durable active memberships before treating
    /// this process's publication as complete. A foreign active connection is
    /// never covered by an empty local registry or a queue acknowledgement.
    pub(crate) fn covers_connections(
        &self,
        event: Uuid,
        actor: Uuid,
        connections: &[Uuid],
    ) -> Result<bool, PolicyUnavailable> {
        let slots = self.transport.slots.lock().map_err(|_| PolicyUnavailable)?;
        let mut local = std::collections::BTreeSet::new();
        for slot in slots.values() {
            let slot = slot.lock().map_err(|_| PolicyUnavailable)?;
            if let Some(binding) = slot.binding.as_ref() {
                if binding.key.event_id.uuid() == event && binding.key.user_id.uuid() == actor {
                    local.insert(binding.connection_id);
                }
            }
        }
        Ok(connections.iter().all(|id| local.contains(id)))
    }
    /// Called only by the canonical sealed-source adapter, never from HTTP/WS
    /// input or arbitrary invalidation rows. The adapter owns durable source proof.
    pub(crate) fn publish_metadata(
        &self,
        event: Uuid,
        actor: Uuid,
        revision: i64,
        reason: PolicyReason,
    ) -> Result<(), PolicyUnavailable> {
        if event.is_nil() || actor.is_nil() || !(1..=MAX_REVISION).contains(&revision) {
            return Err(PolicyUnavailable);
        }
        let slots = self.transport.slots.lock().map_err(|_| PolicyUnavailable)?;
        for slot in slots.values() {
            let mut slot = slot.lock().map_err(|_| PolicyUnavailable)?;
            let Some(b) = slot.binding.clone() else {
                continue;
            };
            if b.key.event_id.uuid() != event
                || b.key.user_id.uuid() != actor
                || slot.terminal.is_some()
            {
                continue;
            }
            let send = slot
                .policy
                .invalidate(revision)
                .map_err(|_| PolicyUnavailable)?;
            if send
                && handoff(
                    &mut slot,
                    frame(&b, revision, Some(reason)).map_err(|_| PolicyUnavailable)?,
                )
                .is_err()
            {
                // This is actual removal from the publication set, followed by
                // the incumbent session's terminal-close path; no extra queue.
                slot.terminal.get_or_insert(SafeClose::UNAVAILABLE);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn staged(revision: i64) -> PolicyState {
        let mut state = PolicyState::default();
        state.prepare(revision).unwrap();
        state
    }

    #[test]
    fn baseline_needs_both_commit_and_actual_snapshot() {
        let mut state = staged(4);
        state.finish(4, true);
        assert_eq!(state.baseline(), None);
        state.snapshot_sent = true;
        assert_eq!(state.baseline(), Some(4));
        state.baseline_sent = true;
        assert_eq!(state.baseline(), None);
        let mut reversed = staged(4);
        reversed.snapshot_sent = true;
        assert_eq!(reversed.baseline(), None);
        reversed.finish(4, true);
        assert_eq!(reversed.baseline(), Some(4));
    }

    #[test]
    fn invalidation_between_database_commit_and_arm_wins() {
        let mut state = staged(4);
        state.snapshot_sent = true;
        assert!(!state.invalidate(5).unwrap());
        state.finish(4, true);
        assert_eq!(state.baseline(), None);
        assert_eq!(state.floor, 5);
        assert!(state.prepare(5).is_err());
    }

    #[test]
    fn old_signals_and_duplicate_baselines_cannot_restore_a_withdrawn_view() {
        let mut state = staged(4);
        state.snapshot_sent = true;
        state.finish(4, true);
        state.baseline_sent = true;
        assert!(state.invalidate(5).unwrap());
        assert!(!state.invalidate(4).unwrap());
        assert!(!state.invalidate(5).unwrap());
        state.finish(4, true);
        assert_eq!(state.floor, 5);
        assert_eq!(state.baseline(), None);
    }

    #[test]
    fn failed_guard_and_unsafe_revisions_never_emit_baselines() {
        let mut state = staged(1);
        state.snapshot_sent = true;
        state.finish(1, false);
        state.finish(1, true);
        assert_eq!(state.baseline(), None);
        for revision in [i64::MIN, -1, 0, MAX_REVISION + 1, i64::MAX] {
            assert!(PolicyState::default().prepare(revision).is_err());
            assert!(state.invalidate(revision).is_err());
        }
    }
}
