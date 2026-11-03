//! Incumbent process controller. All room mutation, capture, policy checks and transport
//! handoff occur on one owner task. No public listener or permissive production adapter.
use super::{
    ingress::SafeClose,
    interest::{Facing, RecipientPolicy, RoomFrame},
    membership::{CanonicalAuthorityPort, ClockAnchor, MembershipAdapter},
    recovery::Recovery,
    replication::{Delivery, QueueLimits, SnapshotCause},
    session::RevokeReason,
    simulation::{AuthorityError, Binding, MembershipPort, ResourcePin, Simulation},
};
use concert_domain::{
    world::{Input, Limits, Position, WorldMap},
    EventId,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

pub const MAX_ROOMS: usize = 16;
pub const OWNER_QUEUE: usize = 64;
pub const COMMANDS_PER_TICK: usize = 64;
pub const STEP_MS: u64 = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    Authority,
    Binding,
    Capacity,
    Owner,
    Stale,
    Motion,
    Projection,
    Transport,
    Clock,
    Closed,
    QueueFull,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteError {
    NotReady,
    Failed,
}
/// `send` means immediate handoff to an already-ready bounded transport under the
/// same owner/privacy fence. It MUST NOT enqueue into a second application queue.
/// A failed/partial send closes the stream; bytes must never be replayed.
pub trait TransportPort: Send + 'static {
    fn installed(&mut self, binding: &Binding, now_ms: u64) -> Result<(), WriteError>;
    fn ready(&self, binding: &Binding) -> bool;
    fn send(&mut self, binding: &Binding, delivery: Delivery) -> Result<(), WriteError>;
    fn close(&mut self, binding: &Binding, reason: SafeClose);
}
/// No actual socket is fabricated by the uncomposed runtime.
pub struct UnavailableTransport;
impl TransportPort for UnavailableTransport {
    fn installed(&mut self, _: &Binding, _: u64) -> Result<(), WriteError> {
        Err(WriteError::Failed)
    }
    fn ready(&self, _: &Binding) -> bool {
        false
    }
    fn send(&mut self, _: &Binding, _: Delivery) -> Result<(), WriteError> {
        Err(WriteError::Failed)
    }
    fn close(&mut self, _: &Binding, _: SafeClose) {}
}
// A second task/controller in this process cannot independently own an incumbent room.
// This is not a distributed lock: the qualified admission adapter must fence other processes.
static CLAIMS: OnceLock<Mutex<BTreeSet<(Uuid, Uuid)>>> = OnceLock::new();
struct RoomClaim {
    key: (Uuid, Uuid),
}
impl RoomClaim {
    fn acquire(event: EventId, room: Uuid) -> Result<Self, RuntimeError> {
        let key = (event.uuid(), room);
        let mut claims = CLAIMS
            .get_or_init(Mutex::default)
            .lock()
            .map_err(|_| RuntimeError::Owner)?;
        if claims.len() >= 256 || !claims.insert(key) {
            return Err(RuntimeError::Owner);
        }
        Ok(Self { key })
    }
}
impl Drop for RoomClaim {
    fn drop(&mut self) {
        if let Ok(mut claims) = CLAIMS.get_or_init(Mutex::default).lock() {
            claims.remove(&self.key);
        }
    }
}
pub struct RoomSpec {
    pub room_id: Uuid,
    pub room_epoch: Uuid,
    pub resources: ResourcePin,
    pub map: WorldMap,
    pub limits: Limits,
    pub capacity: usize,
    pub spawn_id: String,
}
struct Room {
    _claim: RoomClaim,
    epoch: Uuid,
    resources: ResourcePin,
    capacity: usize,
    spawn: String,
    simulation: Simulation,
    motion: BTreeMap<Uuid, (Position, Facing, bool)>,
}
fn member_key(b: &Binding) -> (Uuid, Uuid) {
    (b.key.event_id.uuid(), b.key.user_id.uuid())
}
fn cleanup_key(b: &Binding) -> (Uuid, Uuid, Uuid) {
    (b.room_id, b.connection_id, b.stream_id)
}
#[derive(Default)]
pub struct Maintenance {
    pub simulated_steps: u64,
    pub dropped_steps: u64,
    pub closed: usize,
    pub pending_cleanup: usize,
}
/// On shutdown the supervisor MUST retain/retry remaining owner cleanup obligations.
/// A nonempty report is not a successful persistent membership cleanup.
pub struct ShutdownReport {
    pub remaining_cleanup: Vec<Binding>,
}

pub struct WorldRuntime<P, R, T>
where
    P: CanonicalAuthorityPort,
    R: RecipientPolicy + Send + 'static,
    T: TransportPort,
{
    event: EventId,
    owner: Uuid,
    rooms: BTreeMap<Uuid, Room>,
    members: BTreeMap<(Uuid, Uuid), Binding>,
    cleanup: BTreeMap<(Uuid, Uuid, Uuid), Binding>,
    membership: MembershipAdapter<P>,
    recovery: Recovery,
    recipient: R,
    transport: T,
    capacity: usize,
    last_now: u64,
    origin_ms: u64,
    started: Instant,
    closed: bool,
}
impl<P, R, T> WorldRuntime<P, R, T>
where
    P: CanonicalAuthorityPort,
    R: RecipientPolicy + Send + 'static,
    T: TransportPort,
{
    pub fn new(
        event: EventId,
        owner: Uuid,
        anchor: ClockAnchor,
        room_specs: Vec<RoomSpec>,
        authority: P,
        recipient: R,
        transport: T,
    ) -> Result<Self, RuntimeError> {
        if event.uuid().is_nil()
            || owner.is_nil()
            || room_specs.is_empty()
            || room_specs.len() > MAX_ROOMS
        {
            return Err(RuntimeError::Binding);
        }
        let mut rooms = BTreeMap::new();
        let mut capacity = 0usize;
        for spec in room_specs {
            if spec.room_id.is_nil()
                || spec.room_epoch.is_nil()
                || rooms.contains_key(&spec.room_id)
                || spec.capacity == 0
                || spec.capacity > 50
                || spec.map.spawn(&spec.spawn_id).is_err()
            {
                return Err(RuntimeError::Binding);
            }
            let claim = RoomClaim::acquire(event, spec.room_id)?;
            let simulation = Simulation::new(
                event,
                spec.room_id,
                spec.room_epoch,
                owner,
                spec.resources.clone(),
                spec.map,
                spec.limits,
                spec.capacity,
                anchor.monotonic_ms,
            )
            .map_err(|_| RuntimeError::Binding)?;
            capacity += spec.capacity;
            rooms.insert(
                spec.room_id,
                Room {
                    _claim: claim,
                    epoch: spec.room_epoch,
                    resources: spec.resources,
                    capacity: spec.capacity,
                    spawn: spec.spawn_id,
                    simulation,
                    motion: BTreeMap::new(),
                },
            );
        }
        Ok(Self {
            event,
            owner,
            rooms,
            members: BTreeMap::new(),
            cleanup: BTreeMap::new(),
            membership: MembershipAdapter::new(authority, anchor.clone(), capacity)
                .map_err(|_| RuntimeError::Authority)?,
            recovery: Recovery::new(capacity, QueueLimits::default())
                .map_err(|_| RuntimeError::Capacity)?,
            recipient,
            transport,
            capacity,
            last_now: anchor.monotonic_ms,
            origin_ms: anchor.monotonic_ms,
            started: Instant::now(),
            closed: false,
        })
    }
    fn clock(&mut self, now: u64) -> Result<(), RuntimeError> {
        if self.closed {
            return Err(RuntimeError::Closed);
        }
        if now < self.last_now {
            return Err(RuntimeError::Clock);
        }
        self.last_now = now;
        Ok(())
    }
    fn scope(&self, b: &Binding) -> Result<(), RuntimeError> {
        let room = self.rooms.get(&b.room_id).ok_or(RuntimeError::Binding)?;
        if b.key.event_id != self.event
            || b.owner_id != self.owner
            || b.room_epoch != room.epoch
            || b.resources != room.resources
        {
            return Err(RuntimeError::Binding);
        }
        Ok(())
    }
    fn remember_cleanup(&mut self, b: Binding) -> Result<(), RuntimeError> {
        let key = cleanup_key(&b);
        if !self.cleanup.contains_key(&key) && self.cleanup.len() >= self.capacity {
            return Err(RuntimeError::Capacity);
        }
        self.cleanup.insert(key, b);
        Ok(())
    }
    fn close_member(&mut self, b: &Binding, reason: RevokeReason) {
        if let Some(room) = self.rooms.get_mut(&b.room_id) {
            let _ = self.recovery.revoke(b, reason, &mut room.simulation);
            room.motion.remove(&b.key.user_id.uuid());
        }
        self.transport.close(
            b,
            SafeClose {
                code: reason.close_code(),
                label: reason.wire(),
            },
        );
        // Active closed slots and captured bindings retain failed cleanup for retry.
        let _ = self.remember_cleanup(b.clone());
    }
    /// Binding is supplied by trusted committed admission/recovery, never by frame identity.
    /// The qualified adapter must preflight target capacity before committing a switch.
    pub fn install(
        &mut self,
        b: Binding,
        cause: SnapshotCause,
        now: u64,
    ) -> Result<(), RuntimeError> {
        let fence = self.membership.publication_fence();
        let _guard = fence.lock().map_err(|_| RuntimeError::Authority)?;
        self.clock(now)?;
        self.scope(&b)?;
        let old = self.members.get(&member_key(&b)).cloned();
        let target = &self.rooms[&b.room_id];
        let already_here = old.as_ref().is_some_and(|old| old.room_id == b.room_id);
        if self.cleanup.len() >= self.capacity
            || (!already_here && target.simulation.player_count() >= target.capacity)
        {
            return Err(RuntimeError::Capacity);
        }
        let installed = self
            .membership
            .install(&b, now)
            .map_err(|_| RuntimeError::Authority)?;
        let activated = if let Some(previous) = old
            .as_ref()
            .filter(|old| old.room_id != b.room_id && old.connection_id == b.connection_id)
        {
            let SnapshotCause::RoomSwitch(request) = cause else {
                self.close_member(previous, RevokeReason::PolicyUnavailable);
                let _ = self.remember_cleanup(b.clone());
                return Err(RuntimeError::Binding);
            };
            // Temporarily extract the old room to obtain two exclusive mutable simulations.
            // Its process claim remains live in `old_room` throughout this synchronous operation.
            let mut old_room = self
                .rooms
                .remove(&previous.room_id)
                .ok_or(RuntimeError::Binding)?;
            let new_room = self
                .rooms
                .get_mut(&b.room_id)
                .ok_or(RuntimeError::Binding)?;
            let result = self.recovery.switch_room(
                previous,
                b.clone(),
                installed.deadline,
                request,
                &new_room.spawn,
                now,
                &self.membership,
                &mut old_room.simulation,
                &mut new_room.simulation,
            );
            self.rooms.insert(previous.room_id, old_room);
            result.map(|_| old.clone())
        } else {
            let room = self
                .rooms
                .get_mut(&b.room_id)
                .ok_or(RuntimeError::Binding)?;
            self.recovery.activate(
                b.clone(),
                installed.deadline,
                cause,
                &room.spawn,
                now,
                &self.membership,
                &mut room.simulation,
            )
        };
        if activated.is_err() {
            if let Some(old) = &old {
                self.close_member(old, RevokeReason::PolicyUnavailable);
            }
            let _ = self.remember_cleanup(b);
            return Err(RuntimeError::Authority);
        }
        if let Some(previous) = old {
            if previous != b {
                if let Some(room) = self.rooms.get_mut(&previous.room_id) {
                    room.simulation.remove(&previous);
                    room.motion.remove(&previous.key.user_id.uuid());
                }
                if previous.connection_id != b.connection_id {
                    self.transport.close(
                        &previous,
                        SafeClose {
                            code: 4401,
                            label: "CONNECTION_REPLACED",
                        },
                    );
                }
                self.remember_cleanup(previous)?;
            }
        }
        self.members.insert(member_key(&b), b.clone());
        if let Some(room) = self.rooms.get_mut(&b.room_id) {
            if let Some((_, state)) = room.simulation.replication_state(&b) {
                room.motion.insert(
                    b.key.user_id.uuid(),
                    (state.position, installed.identity.facing, false),
                );
            }
        }
        if self.transport.installed(&b, now).is_err() {
            self.close_member(&b, RevokeReason::PolicyUnavailable);
            return Err(RuntimeError::Transport);
        }
        Ok(())
    }
    pub fn input(&mut self, b: &Binding, input: Input, now: u64) -> Result<(), RuntimeError> {
        let fence = self.membership.publication_fence();
        let _guard = fence.lock().map_err(|_| RuntimeError::Authority)?;
        self.clock(now)?;
        self.scope(b)?;
        if self.members.get(&member_key(b)) != Some(b) {
            return Err(RuntimeError::Stale);
        }
        let room = self
            .rooms
            .get_mut(&b.room_id)
            .ok_or(RuntimeError::Binding)?;
        self.recovery
            .submit(b, input, now, &self.membership, &mut room.simulation)
            .map_err(|_| RuntimeError::Motion)
    }
    pub fn disconnect(&mut self, b: &Binding, now: u64) -> Result<bool, RuntimeError> {
        let fence = self.membership.publication_fence();
        let _guard = fence.lock().map_err(|_| RuntimeError::Authority)?;
        self.disconnect_inner(b, now)
    }
    fn disconnect_inner(&mut self, b: &Binding, now: u64) -> Result<bool, RuntimeError> {
        self.clock(now)?;
        if self.members.get(&member_key(b)) != Some(b) {
            return Ok(false);
        }
        if self.remember_cleanup(b.clone()).is_err() {
            self.close_member(b, RevokeReason::PolicyUnavailable);
            return Err(RuntimeError::Capacity);
        }
        let room = self
            .rooms
            .get_mut(&b.room_id)
            .ok_or(RuntimeError::Binding)?;
        self.recovery.disconnect(b, &mut room.simulation);
        room.motion.remove(&b.key.user_id.uuid());
        self.members.remove(&member_key(b));
        self.remember_cleanup(b.clone())?;
        self.retry_cleanup();
        Ok(true)
    }
    pub fn revoke(
        &mut self,
        b: &Binding,
        reason: RevokeReason,
        now: u64,
    ) -> Result<(), RuntimeError> {
        let fence = self.membership.publication_fence();
        let _guard = fence.lock().map_err(|_| RuntimeError::Authority)?;
        self.clock(now)?;
        if self.members.get(&member_key(b)) != Some(b) {
            return Err(RuntimeError::Stale);
        }
        self.close_member(b, reason);
        Ok(())
    }
    fn retry_cleanup(&mut self) {
        let membership = &mut self.membership;
        let _ = self
            .recovery
            .retry_cleanup(|b| membership.release(b).is_ok());
        let bindings: Vec<_> = self.cleanup.values().cloned().collect();
        for b in bindings {
            if self.membership.release(&b).is_ok() {
                if self.members.get(&member_key(&b)) == Some(&b) {
                    if let Some(room) = self.rooms.get_mut(&b.room_id) {
                        self.recovery.disconnect(&b, &mut room.simulation);
                    }
                    self.members.remove(&member_key(&b));
                }
                self.cleanup.remove(&cleanup_key(&b));
            }
        }
        // Disconnect can register a new recovery cleanup obligation after the
        // first pass. Confirm that exact idempotent release before reporting it.
        let membership = &mut self.membership;
        let _ = self
            .recovery
            .retry_cleanup(|b| membership.release(b).is_ok());
    }
    fn authority_reason(&mut self, b: &Binding, now: u64) -> RevokeReason {
        if self
            .recovery
            .stream_mut(b)
            .is_ok_and(|s| s.deadline().monotonic_ms <= now)
        {
            return RevokeReason::SessionExpired;
        }
        match self.membership.current(b, now) {
            Err(AuthorityError::Revoked) => RevokeReason::SessionRevoked,
            _ => RevokeReason::PolicyUnavailable,
        }
    }
    /// Called independently of socket writes, on the one 20Hz process scheduler.
    pub fn maintain(&mut self, now: u64) -> Result<Maintenance, RuntimeError> {
        let fence = self.membership.publication_fence();
        let _guard = fence.lock().map_err(|_| RuntimeError::Authority)?;
        self.clock(now)?;
        let mut report = Maintenance::default();
        let revoked = self.recovery.sweep(now, &self.membership);
        for b in revoked {
            let reason = self.authority_reason(&b, now);
            self.close_member(&b, reason);
            report.closed += 1;
        }
        for room in self.rooms.values_mut() {
            let result = room
                .simulation
                .advance(now, &self.membership)
                .map_err(|_| RuntimeError::Clock)?;
            report.simulated_steps += result.steps;
            report.dropped_steps += result.dropped_steps;
            // Interaction intents deliberately have no physical-arrival or durable-command effect.
            room.motion
                .retain(|user, _| room.simulation.state(*user).is_some());
        }
        let members: Vec<_> = self.members.values().cloned().collect();
        for b in &members {
            let room = self
                .rooms
                .get_mut(&b.room_id)
                .ok_or(RuntimeError::Binding)?;
            if let Some((_, state)) = room.simulation.replication_state(b) {
                let previous = room.motion.get(&b.key.user_id.uuid()).copied();
                let (facing, moving) = previous.map_or((Facing::Down, false), |(p, f, _)| {
                    let dx = state.position.x - p.x;
                    let dy = state.position.y - p.y;
                    let moving = dx != 0.0 || dy != 0.0;
                    let facing = if !moving {
                        f
                    } else if dx.abs() > dy.abs() {
                        if dx < 0.0 {
                            Facing::Left
                        } else {
                            Facing::Right
                        }
                    } else if dy < 0.0 {
                        Facing::Up
                    } else {
                        Facing::Down
                    };
                    (facing, moving)
                });
                room.motion
                    .insert(b.key.user_id.uuid(), (state.position, facing, moving));
            }
        }
        // Refresh without dequeuing/numbering to enforce current privacy and slow-consumer limits.
        for b in members {
            self.publish_inner(&b, now, false)?;
        }
        self.retry_cleanup();
        report.pending_cleanup = self.cleanup.len() + self.recovery.pending_cleanup_count();
        Ok(report)
    }
    fn frame(&self, scope: &Binding, now: u64) -> Result<RoomFrame, RuntimeError> {
        let room = self
            .rooms
            .get(&scope.room_id)
            .ok_or(RuntimeError::Binding)?;
        let mut roster = Vec::new();
        for b in self.members.values().filter(|b| b.room_id == scope.room_id) {
            if room.simulation.replication_state(b).is_none() {
                continue;
            }
            let mut identity = self
                .membership
                .identity(b, now)
                .map_err(|_| RuntimeError::Authority)?;
            if let Some((_, facing, moving)) = room.motion.get(&b.key.user_id.uuid()) {
                identity.facing = *facing;
                identity.moving = *moving;
            }
            roster.push((b.clone(), identity));
        }
        RoomFrame::capture(&room.simulation, scope.clone(), roster)
            .map_err(|_| RuntimeError::Projection)
    }
    /// Transport must be ready before sequence allocation, then accept bytes immediately.
    /// Every call reprojects current authority/privacy even if no simulation tick was due.
    pub fn publish(&mut self, b: &Binding, now: u64, handoff: bool) -> Result<(), RuntimeError> {
        let fence = self.membership.publication_fence();
        let _guard = fence.lock().map_err(|_| RuntimeError::Authority)?;
        self.publish_inner(b, now, handoff)
    }
    fn publish_inner(&mut self, b: &Binding, now: u64, handoff: bool) -> Result<(), RuntimeError> {
        self.clock(now)?;
        if self.members.get(&member_key(b)) != Some(b) {
            return Err(RuntimeError::Stale);
        }
        if self
            .recovery
            .stream_mut(b)
            .map_err(|_| RuntimeError::Stale)?
            .check_authority(now, &self.membership)
            .is_err()
        {
            let reason = self.authority_reason(b, now);
            self.close_member(b, reason);
            return Ok(());
        }
        let frame = match self.frame(b, now) {
            Ok(frame) => frame,
            Err(_) => {
                self.close_member(b, RevokeReason::PolicyUnavailable);
                return Ok(());
            }
        };
        let stream = self
            .recovery
            .stream_mut(b)
            .map_err(|_| RuntimeError::Stale)?;
        if !handoff || !self.transport.ready(b) {
            if stream
                .refresh(&frame, now, &self.membership, &self.recipient)
                .is_err()
            {
                self.close_member(b, RevokeReason::PolicyUnavailable);
            }
            return Ok(());
        }
        if let Some(delivery) = stream.next(&frame, now, &self.membership, &self.recipient) {
            let terminal = delivery.close_code.is_some();
            if self.transport.send(b, delivery).is_err() {
                self.close_member(b, RevokeReason::PolicyUnavailable);
                return Err(RuntimeError::Transport);
            }
            if terminal {
                // The transport already receives the exact terminal close code above.
                let _ = self.disconnect_inner(b, now);
            }
        }
        Ok(())
    }
    pub fn clock_origin(&self) -> (Instant, u64) {
        (self.started, self.origin_ms)
    }
    pub fn active_count(&self) -> usize {
        self.members.len()
    }
    pub fn pending_cleanup_count(&self) -> usize {
        self.cleanup.len() + self.recovery.pending_cleanup_count()
    }
    pub fn shutdown(&mut self) -> ShutdownReport {
        let fence = self.membership.publication_fence();
        let guard = fence.lock().ok();
        let bindings: Vec<_> = self.members.values().cloned().collect();
        for b in &bindings {
            self.close_member(b, RevokeReason::PolicyUnavailable);
        }
        if guard.is_some() {
            self.retry_cleanup();
        }
        self.closed = true;
        let mut remaining: BTreeMap<_, _> = self
            .cleanup
            .values()
            .chain(self.members.values())
            .map(|b| (cleanup_key(b), b.clone()))
            .collect();
        let _ = self.recovery.retry_cleanup(|b| {
            remaining.insert(cleanup_key(b), b.clone());
            false
        });
        // Local state/sockets stop even when persistent cleanup must be retried by the supervisor.
        for b in &bindings {
            if let Some(room) = self.rooms.get_mut(&b.room_id) {
                room.simulation.remove(b);
            }
        }
        self.members.clear();
        self.rooms.clear();
        ShutdownReport {
            remaining_cleanup: remaining.into_values().collect(),
        }
    }
    /// Moves the sole controller into one Tokio task; bounded sender has no blocking enqueue.
    pub fn spawn(self) -> (RuntimeHandle, tokio::task::JoinHandle<ShutdownReport>) {
        let (sender, receiver) = mpsc::channel(OWNER_QUEUE);
        let (stop, stop_rx) = oneshot::channel();
        let handle = RuntimeHandle {
            sender,
            stop: Mutex::new(Some(stop)),
        };
        (handle, tokio::spawn(self.run(receiver, stop_rx)))
    }
    async fn run(
        mut self,
        mut receiver: mpsc::Receiver<Command>,
        mut stop: oneshot::Receiver<()>,
    ) -> ShutdownReport {
        let mut interval = tokio::time::interval(Duration::from_millis(STEP_MS));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut commands = 0usize;
        loop {
            tokio::select! { biased;
                _ = &mut stop => break,
                _ = interval.tick() => {
                    let Some(now) = self.origin_ms.checked_add(self.started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)) else { break; };
                    if self.maintain(now).is_err() { break; }
                    let bindings: Vec<_> = self.members.values().cloned().collect();
                    for b in bindings { let _ = self.publish(&b,now,true); }
                    commands = 0;
                },
                command = receiver.recv(), if commands < COMMANDS_PER_TICK => {
                    let Some(command) = command else { break; }; commands += 1;
                    let Some(now) = self.origin_ms.checked_add(self.started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)) else { break; };
                    match command {
                        Command::Install { binding,cause,reply } => { let _ = reply.send(self.install(binding,cause,now)); },
                        Command::Input { binding,input,reply } => { let _ = reply.send(self.input(&binding,input,now)); },
                        Command::Disconnect { binding,reply } => { let _ = reply.send(self.disconnect(&binding,now)); },
                        Command::Revoke { binding,reason,reply } => { let _ = reply.send(self.revoke(&binding,reason,now)); },
                    }
                }
            }
        }
        receiver.close();
        self.shutdown()
    }
}
impl<P, R, T> Drop for WorldRuntime<P, R, T>
where
    P: CanonicalAuthorityPort,
    R: RecipientPolicy + Send + 'static,
    T: TransportPort,
{
    fn drop(&mut self) {
        if !self.closed {
            let _ = self.shutdown();
        }
    }
}
pub enum Command {
    Install {
        binding: Binding,
        cause: SnapshotCause,
        reply: oneshot::Sender<Result<(), RuntimeError>>,
    },
    Input {
        binding: Binding,
        input: Input,
        reply: oneshot::Sender<Result<(), RuntimeError>>,
    },
    Disconnect {
        binding: Binding,
        reply: oneshot::Sender<Result<bool, RuntimeError>>,
    },
    Revoke {
        binding: Binding,
        reason: RevokeReason,
        reply: oneshot::Sender<Result<(), RuntimeError>>,
    },
}
pub struct RuntimeHandle {
    sender: mpsc::Sender<Command>,
    stop: Mutex<Option<oneshot::Sender<()>>>,
}
impl RuntimeHandle {
    pub fn try_submit(&self, command: Command) -> Result<(), RuntimeError> {
        self.sender.try_send(command).map_err(|e| match e {
            mpsc::error::TrySendError::Full(_) => RuntimeError::QueueFull,
            mpsc::error::TrySendError::Closed(_) => RuntimeError::Closed,
        })
    }
    pub fn request_stop(&self) {
        if let Ok(mut slot) = self.stop.lock() {
            if let Some(stop) = slot.take() {
                let _ = stop.send(());
            }
        }
    }
    pub fn stop(&mut self) {
        self.request_stop();
    }
    pub fn queue_capacity(&self) -> usize {
        self.sender.capacity()
    }
}
impl Drop for RuntimeHandle {
    fn drop(&mut self) {
        self.stop();
    }
}
