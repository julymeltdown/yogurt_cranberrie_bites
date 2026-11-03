//! Single-writer recovery book. Admission commits/fences precede calls; this is not ticket ingress.
use super::{
    interest::same_room,
    replication::{QueueLimits, Replication, SnapshotCause},
    session::{AccessDeadline, RevokeReason, Session},
    simulation::{Binding, MembershipPort, Simulation},
};
use concert_domain::world::Input;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub const MAX_STREAMS_PER_CONNECTION: usize = 1024;
pub const RESYNC_MIN_INTERVAL_MS: u64 = 1000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryError {
    Authority,
    Binding,
    Stale,
    Capacity,
    Motion,
    RateLimited,
    ReconnectRequired,
}
struct Slot {
    stream: Replication,
    used_streams: BTreeSet<Uuid>,
    used_requests: BTreeSet<Uuid>,
    installed_at: u64,
}
/// One subscription per canonical (event,user), stricter than one per active shell.
/// Capacity includes closed slots; clean them on disconnect/expiry. No unbounded tombstone map.
pub struct Recovery {
    slots: BTreeMap<(Uuid, Uuid), Slot>,
    pending_cleanup: BTreeMap<(Uuid, Uuid, Uuid, Uuid), Binding>,
    capacity: usize,
    limits: QueueLimits,
}
fn cleanup_key(b: &Binding) -> (Uuid, Uuid, Uuid, Uuid) {
    (
        b.key.event_id.uuid(),
        b.key.user_id.uuid(),
        b.connection_id,
        b.stream_id,
    )
}
fn key(b: &Binding) -> (Uuid, Uuid) {
    (b.key.event_id.uuid(), b.key.user_id.uuid())
}
impl Recovery {
    pub fn new(capacity: usize, limits: QueueLimits) -> Result<Self, RecoveryError> {
        if capacity == 0 || capacity > 10000 {
            return Err(RecoveryError::Capacity);
        }
        Ok(Self {
            slots: BTreeMap::new(),
            pending_cleanup: BTreeMap::new(),
            capacity,
            limits,
        })
    }
    pub fn pending_cleanup_count(&self) -> usize {
        self.pending_cleanup.len()
    }
    pub fn len(&self) -> usize {
        self.slots.len()
    }
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
    pub fn stream_mut(&mut self, expected: &Binding) -> Result<&mut Replication, RecoveryError> {
        self.slots
            .get_mut(&key(expected))
            .filter(|s| s.stream.binding() == expected)
            .map(|s| &mut s.stream)
            .ok_or(RecoveryError::Stale)
    }
    /// A trusted BE10/11 adapter has already consumed/fenced a fresh ticket or committed a
    /// correlated resync. New stream UUIDs are SERVER-generated. Never restore an old stream.
    /// For same-room reconnect this method clears BE12 held/pending input via replace_stream.
    /// New owner/epoch uses a new Simulation and a validated server-selected spawn.
    #[allow(clippy::too_many_arguments)]
    pub fn activate(
        &mut self,
        binding: Binding,
        deadline: AccessDeadline,
        cause: SnapshotCause,
        spawn: &str,
        now: u64,
        membership: &impl MembershipPort,
        simulation: &mut Simulation,
    ) -> Result<Option<Binding>, RecoveryError> {
        let session = Session::new(binding.clone(), deadline, now, membership)
            .map_err(|_| RecoveryError::Authority)?;
        let candidate =
            Replication::new(session, cause, self.limits).map_err(|_| RecoveryError::Binding)?;
        if self.slots.values().any(|s| {
            s.stream.binding().connection_id == binding.connection_id
                && key(s.stream.binding()) != key(&binding)
        }) {
            return Err(RecoveryError::Binding);
        }
        let previous = self.slots.get(&key(&binding));
        let replaced = previous.map(|p| p.stream.binding().clone());
        if previous.is_none() && self.slots.len() >= self.capacity {
            return Err(RecoveryError::Capacity);
        }
        let same_connection =
            previous.is_some_and(|p| p.stream.binding().connection_id == binding.connection_id);
        let request = match cause {
            SnapshotCause::Resync(r) | SnapshotCause::RoomSwitch(r) => Some(r),
            _ => None,
        };
        if let Some(old) = previous {
            if old.stream.binding().stream_id == binding.stream_id {
                return Err(RecoveryError::Stale);
            }
            if same_connection {
                if old.stream.is_closed() {
                    return Err(RecoveryError::ReconnectRequired);
                }
                if old.stream.binding().session_id != binding.session_id
                    || old.stream.deadline() != candidate.deadline()
                {
                    return Err(RecoveryError::Binding);
                }
                if old.used_streams.contains(&binding.stream_id)
                    || request.is_some_and(|r| old.used_requests.contains(&r))
                {
                    return Err(RecoveryError::Stale);
                }
                if old.used_streams.len() >= MAX_STREAMS_PER_CONNECTION {
                    return Err(RecoveryError::ReconnectRequired);
                }
                if now < old.installed_at
                    || (matches!(cause, SnapshotCause::Resync(_))
                        && now - old.installed_at < RESYNC_MIN_INTERVAL_MS)
                {
                    return Err(RecoveryError::RateLimited);
                }
                match cause {
                    SnapshotCause::Resync(_) if same_room(old.stream.binding(), &binding) => {}
                    SnapshotCause::RoomSwitch(_)
                        if old.stream.binding().room_id != binding.room_id
                            && old.stream.binding().resources == binding.resources => {}
                    SnapshotCause::EpochChanged
                        if old.stream.binding().room_epoch != binding.room_epoch
                            && old.stream.binding().room_id == binding.room_id
                            && old.stream.binding().resources == binding.resources => {}
                    _ => return Err(RecoveryError::Binding),
                }
            } else if cause != SnapshotCause::Initial {
                return Err(RecoveryError::Binding);
            }
        } else if cause != SnapshotCause::Initial {
            return Err(RecoveryError::Binding);
        }
        let same_owner_room = previous.is_some_and(|p| same_room(p.stream.binding(), &binding));
        let cleanup_obligation = previous
            .filter(|_| !same_owner_room)
            .map(|s| s.stream.binding().clone());
        if let Some(old) = &cleanup_obligation {
            if self.pending_cleanup.len() >= self.capacity
                && !self.pending_cleanup.contains_key(&cleanup_key(old))
            {
                return Err(RecoveryError::Capacity);
            }
        }
        if same_owner_room && simulation.state(binding.key.user_id.uuid()).is_some() {
            simulation
                .replace_stream(binding.clone(), now, membership)
                .map_err(|_| RecoveryError::Motion)?;
        } else {
            if same_connection && same_owner_room {
                return Err(RecoveryError::ReconnectRequired);
            }
            simulation
                .admit(binding.clone(), spawn, now, membership)
                .map_err(|_| RecoveryError::Motion)?;
        }
        let mut used_streams = if same_connection {
            previous.expect("checked").used_streams.clone()
        } else {
            BTreeSet::new()
        };
        let mut used_requests = if same_connection {
            previous.expect("checked").used_requests.clone()
        } else {
            BTreeSet::new()
        };
        if let Some(r) = request {
            used_requests.insert(r);
        }
        used_streams.insert(binding.stream_id);
        if let Some(old) = self.slots.get_mut(&key(&binding)) {
            old.stream.revoke(RevokeReason::ConnectionReplaced);
        }
        if let Some(old) = cleanup_obligation {
            self.pending_cleanup.insert(cleanup_key(&old), old);
        }
        self.slots.insert(
            key(&binding),
            Slot {
                stream: candidate,
                used_streams,
                used_requests,
                installed_at: now,
            },
        );
        Ok(replaced)
    }
    /// Switch admission must commit atomically at BE11 before this serialized local handoff.
    /// The old simulation is explicitly emptied; delayed old disconnect/input cannot touch new.
    #[allow(clippy::too_many_arguments)]
    pub fn switch_room(
        &mut self,
        old: &Binding,
        new: Binding,
        deadline: AccessDeadline,
        request: Uuid,
        spawn: &str,
        now: u64,
        membership: &impl MembershipPort,
        old_sim: &mut Simulation,
        new_sim: &mut Simulation,
    ) -> Result<(), RecoveryError> {
        self.stream_mut(old)?;
        if key(old) != key(&new)
            || old.room_id == new.room_id
            || old.connection_id != new.connection_id
        {
            return Err(RecoveryError::Binding);
        }
        self.activate(
            new,
            deadline,
            SnapshotCause::RoomSwitch(request),
            spawn,
            now,
            membership,
            new_sim,
        )?;
        if old_sim.remove(old) {
            self.pending_cleanup.remove(&cleanup_key(old));
        }
        Ok(())
    }
    /// All input passes here after a delivered snapshot; no queued pre-snapshot/old-stream motion.
    pub fn submit(
        &mut self,
        expected: &Binding,
        input: Input,
        now: u64,
        membership: &impl MembershipPort,
        simulation: &mut Simulation,
    ) -> Result<(), RecoveryError> {
        let stream = self.stream_mut(expected)?;
        if stream.check_authority(now, membership).is_err() {
            simulation.remove(expected);
            return Err(RecoveryError::Authority);
        }
        if !stream.accepts_input() {
            return Err(RecoveryError::Stale);
        }
        if simulation.submit(expected, input, now, membership).is_err() {
            // BE12 owns precise input/rate errors. Authority is independently swept before output.
            return Err(RecoveryError::Motion);
        }
        Ok(())
    }
    pub fn revoke(
        &mut self,
        expected: &Binding,
        reason: RevokeReason,
        simulation: &mut Simulation,
    ) -> Result<(), RecoveryError> {
        self.stream_mut(expected)?.revoke(reason);
        simulation.remove(expected);
        Ok(())
    }
    pub fn disconnect(&mut self, expected: &Binding, simulation: &mut Simulation) -> bool {
        if !self
            .slots
            .get(&key(expected))
            .is_some_and(|s| s.stream.binding() == expected)
        {
            return false;
        }
        self.slots
            .get_mut(&key(expected))
            .expect("matched")
            .stream
            .revoke(RevokeReason::SessionRevoked);
        if !simulation.remove(expected) {
            // A wrong room or an already-swept simulation cannot erase the last cleanup
            // identity. The owner-routed callback may acknowledge idempotent absence.
            let k = cleanup_key(expected);
            if self.pending_cleanup.len() >= self.capacity && !self.pending_cleanup.contains_key(&k)
            {
                return false;
            }
            self.pending_cleanup.insert(k, expected.clone());
        }
        self.slots.remove(&key(expected));
        true
    }
    /// Sweep independently of frames/ticks, including suspension/revocation/outage. Caller
    /// routes removed bindings to their owning simulations and sends terminal controls/closes.
    pub fn sweep(&mut self, now: u64, membership: &impl MembershipPort) -> Vec<Binding> {
        self.slots
            .values_mut()
            .filter_map(|s| {
                s.stream
                    .check_authority(now, membership)
                    .err()
                    .map(|_| s.stream.binding().clone())
            })
            .collect()
    }
    /// Retry displaced owner cleanup independently of the current subscription. A failed
    /// expiry cleanup cannot be forgotten when a fresh connection moves to another owner.
    pub fn retry_cleanup(&mut self, mut cleanup: impl FnMut(&Binding) -> bool) -> Vec<Binding> {
        let mut cleaned = Vec::new();
        self.pending_cleanup.retain(|_, b| {
            if cleanup(b) {
                cleaned.push(b.clone());
                false
            } else {
                true
            }
        });
        cleaned
    }
    /// Deterministic expiry cleanup even when no socket writes or simulation ticks occur.
    /// Adapter must also sweep live authority/policy via Replication::refresh before publication.
    pub fn expire(&mut self, now: u64, mut cleanup: impl FnMut(&Binding) -> bool) -> Vec<Binding> {
        let expired: Vec<_> = self
            .slots
            .values()
            .filter(|s| s.stream.deadline().monotonic_ms <= now)
            .map(|s| s.stream.binding().clone())
            .collect();
        let mut cleaned = self.retry_cleanup(&mut cleanup);
        for b in expired {
            self.slots
                .get_mut(&key(&b))
                .expect("collected")
                .stream
                .revoke(RevokeReason::SessionExpired);
            // Retain a closed, discoverable slot when owner cleanup fails, for a later retry.
            if cleanup(&b) {
                self.slots.remove(&key(&b));
                cleaned.push(b);
            }
        }
        cleaned
    }
}
