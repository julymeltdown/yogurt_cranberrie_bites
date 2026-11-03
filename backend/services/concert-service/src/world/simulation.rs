//! BE12 single-room owner. Calls must be serialized with admission/takeover/switch commits.
//! The membership port is a synchronous, fail-closed view of owner-fenced authority,
//! not a database/network query on every tick and never a heartbeat-based renewal.
use concert_domain::{
    world::{Input, Limits, Position, WorldMap, STEP_MS},
    AssetId, ParticipantKey, Revision,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use uuid::Uuid;

pub const ROOM_CAPACITY: usize = 50;
pub const MAX_CATCH_UP_STEPS: u64 = 4;
pub const INPUT_RATE: u64 = 30;
pub const INPUT_BURST: u64 = 60;
pub const AUTH_LEASE_MAX_MS: u64 = 5000;
pub const MAX_STREAMS_PER_CONNECTION: usize = 1024;
pub const MAX_PENDING_INPUTS: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourcePin {
    pub map_id: AssetId,
    pub map_revision: Revision,
    pub avatar_manifest_id: AssetId,
    pub avatar_manifest_version: Revision,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub key: ParticipantKey,
    pub session_id: Uuid,
    pub connection_id: Uuid,
    pub owner_id: Uuid,
    pub room_id: Uuid,
    pub room_epoch: Uuid,
    pub stream_id: Uuid,
    pub resources: ResourcePin,
}
impl Binding {
    fn non_nil(&self) -> bool {
        [
            self.key.event_id.uuid(),
            self.key.user_id.uuid(),
            self.session_id,
            self.connection_id,
            self.owner_id,
            self.room_id,
            self.room_epoch,
            self.stream_id,
        ]
        .iter()
        .all(|id| !id.is_nil())
    }
}
#[derive(Clone, Debug)]
pub struct MembershipLease {
    pub binding: Binding,
    /// Server monotonic milliseconds, mapped conservatively from authoritative deadlines.
    pub lease_until_ms: u64,
    /// Captured at admission from the verified access credential, immutable on this connection.
    pub access_expires_at_ms: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorityError {
    Unavailable,
    Revoked,
}
pub trait MembershipPort {
    /// Must compare live canonical owner/session/connection/membership/pins, not just row presence.
    /// Return an error on unknown authority; serialize this read with owner state changes.
    fn current(&self, expected: &Binding, now_ms: u64) -> Result<MembershipLease, AuthorityError>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimulationError {
    Authority,
    Binding,
    Capacity,
    AlreadyAdmitted,
    MissingPlayer,
    StaleStream,
    Replay,
    RateLimited,
    StreamLimit,
    InputQueueFull,
    InteractionPending,
    Clock,
    Spawn,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerState {
    pub position: Position,
    pub ack_input_seq: u64,
}
struct Player {
    binding: Binding,
    access_expires_at_ms: u64,
    state: PlayerState,
    last_received_seq: u64,
    pending: VecDeque<(Input, u64)>,
    pending_interaction: Option<u64>,
    held: Option<(Input, u64)>,
    credits: u64, // Milli-tokens: 1000 per message, 30 per ms.
    rate_updated_ms: u64,
    used_streams: BTreeSet<Uuid>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InteractionIntent {
    pub user_id: Uuid,
    pub interaction_id: String,
}
#[derive(Clone, Debug, Default)]
pub struct Advance {
    pub steps: u64,
    pub server_tick: u64,
    pub dropped_steps: u64,
    pub revoked_users: Vec<Uuid>,
    pub interactions: Vec<InteractionIntent>,
}
pub struct Simulation {
    event_id: concert_domain::EventId,
    room_id: Uuid,
    room_epoch: Uuid,
    owner_id: Uuid,
    resources: ResourcePin,
    map: WorldMap,
    limits: Limits,
    capacity: usize,
    players: BTreeMap<Uuid, Player>,
    last_now_ms: u64,
    last_step_ms: u64,
    server_tick: u64,
}
impl Simulation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        event_id: concert_domain::EventId,
        room_id: Uuid,
        room_epoch: Uuid,
        owner_id: Uuid,
        resources: ResourcePin,
        map: WorldMap,
        limits: Limits,
        capacity: usize,
        now_ms: u64,
    ) -> Result<Self, SimulationError> {
        if event_id.uuid().is_nil()
            || room_id.is_nil()
            || room_epoch.is_nil()
            || owner_id.is_nil()
            || capacity == 0
            || capacity > ROOM_CAPACITY
        {
            return Err(SimulationError::Binding);
        }
        Ok(Self {
            event_id,
            room_id,
            room_epoch,
            owner_id,
            resources,
            map,
            limits,
            capacity,
            players: BTreeMap::new(),
            last_now_ms: now_ms,
            last_step_ms: now_ms,
            server_tick: 0,
        })
    }
    fn clock(&mut self, now_ms: u64) -> Result<(), SimulationError> {
        if now_ms < self.last_now_ms {
            return Err(SimulationError::Clock);
        }
        self.last_now_ms = now_ms;
        Ok(())
    }
    fn binding_valid(&self, b: &Binding) -> bool {
        b.non_nil()
            && b.key.event_id == self.event_id
            && b.room_id == self.room_id
            && b.room_epoch == self.room_epoch
            && b.owner_id == self.owner_id
            && b.resources == self.resources
    }
    fn authorize(
        port: &impl MembershipPort,
        binding: &Binding,
        captured_expiry: Option<u64>,
        now: u64,
    ) -> Result<u64, SimulationError> {
        let lease = port
            .current(binding, now)
            .map_err(|_| SimulationError::Authority)?;
        if lease.binding != *binding
            || lease.lease_until_ms <= now
            || lease.access_expires_at_ms <= now
            || lease.lease_until_ms - now > AUTH_LEASE_MAX_MS
            || captured_expiry.is_some_and(|expiry| expiry != lease.access_expires_at_ms)
        {
            return Err(SimulationError::Authority);
        }
        Ok(lease.access_expires_at_ms)
    }
    /// Trusted admission adapter supplies binding and a server-selected named spawn. No client position API.
    pub fn admit(
        &mut self,
        binding: Binding,
        spawn_id: &str,
        now: u64,
        port: &impl MembershipPort,
    ) -> Result<PlayerState, SimulationError> {
        self.clock(now)?;
        if !self.binding_valid(&binding) {
            return Err(SimulationError::Binding);
        }
        let access_expires_at_ms = Self::authorize(port, &binding, None, now)?;
        let id = binding.key.user_id.uuid();
        if self.players.contains_key(&id) {
            return Err(SimulationError::AlreadyAdmitted);
        }
        if self.players.len() >= self.capacity {
            return Err(SimulationError::Capacity);
        }
        let state = PlayerState {
            position: self
                .map
                .spawn(spawn_id)
                .map_err(|_| SimulationError::Spawn)?,
            ack_input_seq: 0,
        };
        self.players.insert(
            id,
            Player {
                used_streams: BTreeSet::from([binding.stream_id]),
                binding,
                access_expires_at_ms,
                state,
                last_received_seq: 0,
                pending: VecDeque::new(),
                pending_interaction: None,
                held: None,
                credits: INPUT_BURST * 1000,
                rate_updated_ms: now,
            },
        );
        Ok(state)
    }
    /// Invoke only after authoritative resync/takeover commits. Preserves position, clears inputs/ack.
    /// Room/epoch/pin changes require a separately constructed owner; stale bindings cannot translate coordinates.
    pub fn replace_stream(
        &mut self,
        binding: Binding,
        now: u64,
        port: &impl MembershipPort,
    ) -> Result<(), SimulationError> {
        self.clock(now)?;
        if !self.binding_valid(&binding) {
            return Err(SimulationError::Binding);
        }
        let id = binding.key.user_id.uuid();
        let player = self
            .players
            .get_mut(&id)
            .ok_or(SimulationError::MissingPlayer)?;
        if binding.stream_id == player.binding.stream_id {
            return Err(SimulationError::StaleStream);
        }
        let same_connection = binding.connection_id == player.binding.connection_id;
        if same_connection {
            if binding.session_id != player.binding.session_id {
                return Err(SimulationError::Binding);
            }
            if player.used_streams.contains(&binding.stream_id) {
                return Err(SimulationError::StaleStream);
            }
            if player.used_streams.len() >= MAX_STREAMS_PER_CONNECTION {
                return Err(SimulationError::StreamLimit);
            }
        }
        let expiry = Self::authorize(
            port,
            &binding,
            same_connection.then_some(player.access_expires_at_ms),
            now,
        )?;
        if !same_connection {
            player.used_streams.clear();
        }
        player.used_streams.insert(binding.stream_id);
        player.binding = binding;
        player.access_expires_at_ms = expiry;
        player.state.ack_input_seq = 0;
        player.last_received_seq = 0;
        player.pending.clear();
        player.pending_interaction = None;
        player.held = None;
        // Retain the rate bucket across stream resets; resync does not grant a new input burst.
        Ok(())
    }
    /// Binding comes from the admitted connection, never from client JSON identity.
    pub fn submit(
        &mut self,
        binding: &Binding,
        input: Input,
        now: u64,
        port: &impl MembershipPort,
    ) -> Result<(), SimulationError> {
        self.clock(now)?;
        let id = binding.key.user_id.uuid();
        let player = self
            .players
            .get_mut(&id)
            .ok_or(SimulationError::MissingPlayer)?;
        if player.binding != *binding
            || input.stream_id() != binding.stream_id
            || input.room_epoch() != binding.room_epoch
        {
            return Err(SimulationError::StaleStream);
        }
        if Self::authorize(port, binding, Some(player.access_expires_at_ms), now).is_err() {
            self.players.remove(&id);
            return Err(SimulationError::Authority);
        }
        player.credits = player
            .credits
            .saturating_add(
                now.saturating_sub(player.rate_updated_ms)
                    .saturating_mul(INPUT_RATE),
            )
            .min(INPUT_BURST * 1000);
        player.rate_updated_ms = now;
        if player.credits < 1000 {
            return Err(SimulationError::RateLimited);
        }
        player.credits -= 1000;
        if input.seq() <= player.last_received_seq {
            return Err(SimulationError::Replay);
        }
        if player.pending.len() >= MAX_PENDING_INPUTS {
            return Err(SimulationError::InputQueueFull);
        }
        if player
            .pending_interaction
            .is_some_and(|at| now - at > self.limits.input_hold_ms())
        {
            player.pending_interaction = None;
        }
        if input.interact() {
            if player.pending_interaction.is_some() {
                return Err(SimulationError::InteractionPending);
            }
            player.pending_interaction = Some(now);
        }
        player.last_received_seq = input.seq();
        player.pending.push_back((input, now));
        Ok(())
    }
    /// At most four 50ms steps; discard excess wall time instead of unbounded catch-up/teleport.
    /// Recheck authority even when no step is due, promptly removing expired/revoked state.
    pub fn advance(
        &mut self,
        now: u64,
        port: &impl MembershipPort,
    ) -> Result<Advance, SimulationError> {
        self.clock(now)?;
        let due = (now - self.last_step_ms) / STEP_MS;
        let steps = due.min(MAX_CATCH_UP_STEPS);
        if self
            .server_tick
            .checked_add(due)
            .is_none_or(|tick| tick > concert_domain::world::MAX_SEQUENCE)
        {
            return Err(SimulationError::Clock);
        }
        let mut result = Advance {
            steps,
            dropped_steps: due - steps,
            ..Advance::default()
        };
        self.players.retain(|id, player| {
            let keep = Self::authorize(
                port,
                &player.binding,
                Some(player.access_expires_at_ms),
                now,
            )
            .is_ok();
            if !keep {
                result.revoked_users.push(*id);
            }
            keep
        });
        for step in 0..steps {
            let step_at = self.last_step_ms + (due - steps + step + 1) * STEP_MS;
            for (id, player) in &mut self.players {
                // Retain each timestamp until its due step, then coalesce only eligible axes.
                // A later packet must not erase a stop that applies to an earlier catch-up step.
                while player.pending.front().is_some_and(|(_, at)| *at <= step_at) {
                    let (input, at) = player.pending.pop_front().expect("front checked above");
                    if now - at <= self.limits.input_hold_ms() {
                        player.state.ack_input_seq = input.seq();
                        player.held = Some((input, at));
                    } else {
                        player.held = None;
                    }
                }
                let interact = player
                    .pending_interaction
                    .is_some_and(|at| at <= step_at && now - at <= self.limits.input_hold_ms());
                if player.pending_interaction.is_some_and(|at| at <= step_at) {
                    player.pending_interaction = None;
                }
                if let Some((input, at)) = &player.held {
                    if *at <= step_at && now - at <= self.limits.input_hold_ms() {
                        player.state.position =
                            self.map.step(player.state.position, input, self.limits);
                    } else {
                        player.held = None;
                    }
                }
                if interact {
                    if let Some(interaction_id) = self
                        .map
                        .nearby_interaction(player.state.position, self.limits)
                    {
                        result.interactions.push(InteractionIntent {
                            user_id: *id,
                            interaction_id: interaction_id.to_owned(),
                        });
                    }
                }
            }
        }
        self.last_step_ms += due * STEP_MS;
        self.server_tick += due;
        result.server_tick = self.server_tick;
        Ok(result)
    }
    /// Owner-only state for authorized projection, never a public unfiltered roster endpoint.
    pub fn state(&self, user_id: Uuid) -> Option<PlayerState> {
        self.players.get(&user_id).map(|p| p.state)
    }
    pub fn player_count(&self) -> usize {
        self.players.len()
    }
    pub fn pending_input_count(&self) -> usize {
        self.players.values().map(|p| p.pending.len()).sum()
    }
    /// Explicit disconnect/removal; obsolete connection cannot remove a newer stream.
    pub fn remove(&mut self, binding: &Binding) -> bool {
        let id = binding.key.user_id.uuid();
        if self.players.get(&id).is_some_and(|p| p.binding == *binding) {
            self.players.remove(&id);
            true
        } else {
            false
        }
    }
}
