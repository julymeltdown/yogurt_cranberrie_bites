//! Bounded latest-state queue. Sequence is assigned after filtering and coalescing.
use super::simulation::MembershipPort;
use super::motion_profile::{MotionAnchor, MotionCapture};
use super::{
    interest::{project, ProjectedPlayer, ProjectionError, RecipientPolicy, RoomFrame},
    session::{RevokeReason, Session, SessionError},
};
use chrono::SecondsFormat;
use concert_domain::world::MAX_SEQUENCE;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use uuid::Uuid;

pub const SERVER_FRAME_BYTES: usize = 65536;
pub const QUEUE_FRAMES: usize = 64;
pub const QUEUE_BYTES: usize = 262144;
pub const DELTA_INTERVAL_MS: u64 = 100;
pub const SLOW_CONSUMER_MS: u64 = 5000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotCause {
    Initial,
    Resync(Uuid),
    RoomSwitch(Uuid),
    EpochChanged,
}
impl SnapshotCause {
    fn wire(self) -> (&'static str, Option<Uuid>) {
        match self {
            Self::Initial => ("INITIAL", None),
            Self::Resync(r) => ("RESYNC", Some(r)),
            Self::RoomSwitch(r) => ("ROOM_SWITCH", Some(r)),
            Self::EpochChanged => ("EPOCH_CHANGED", None),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamError {
    Session(SessionError),
    Projection(ProjectionError),
    Baseline,
    Backpressure,
    Closed,
}
/// Server-owned budgets may tighten frozen C05 caps, never loosen them. One coalesced slot
/// intentionally uses fewer frames than the cap; no queue of historical motion is retained.
#[derive(Clone, Copy, Debug)]
pub struct QueueLimits {
    frame_bytes: usize,
    bytes: usize,
    frames: usize,
    slow_ms: u64,
}
impl Default for QueueLimits {
    fn default() -> Self {
        Self {
            frame_bytes: SERVER_FRAME_BYTES,
            bytes: QUEUE_BYTES,
            frames: QUEUE_FRAMES,
            slow_ms: SLOW_CONSUMER_MS,
        }
    }
}
impl QueueLimits {
    pub fn new(frame_bytes: usize, bytes: usize, frames: usize, slow_ms: u64) -> Option<Self> {
        (frame_bytes >= 256
            && frame_bytes <= SERVER_FRAME_BYTES
            && bytes >= 256
            && bytes <= QUEUE_BYTES
            && frames > 0
            && frames <= QUEUE_FRAMES
            && slow_ms > 0
            && slow_ms <= SLOW_CONSUMER_MS)
            .then_some(Self {
                frame_bytes,
                bytes,
                frames,
                slow_ms,
            })
    }
}
#[derive(Debug)]
pub struct Delivery {
    pub bytes: Vec<u8>,
    /// Close immediately after best-effort control delivery. Never retain a terminal subscription.
    pub close_code: Option<u16>,
}
struct Pending {
    bytes: Vec<u8>,
    players: BTreeMap<Uuid, ProjectedPlayer>,
    tick: u64,
    ack: u64,
    since: u64,
    motion: Option<MotionAnchor>,
}
pub struct Replication {
    session: Session,
    cause: SnapshotCause,
    limits: QueueLimits,
    baseline: Option<BTreeMap<Uuid, ProjectedPlayer>>,
    pending: Option<Pending>,
    terminal: Option<Delivery>,
    terminal_close: Option<(&'static str, u16)>,
    closed: bool,
    generation: Option<Uuid>,
    seq: u64,
    tick: u64,
    ack: u64,
    last_delta: Option<u64>,
    motion_enabled: bool,
    motion_capture: Option<MotionCapture>,
    delivered_motion: Option<MotionAnchor>,
}
impl Replication {
    pub fn new(
        session: Session,
        cause: SnapshotCause,
        limits: QueueLimits,
    ) -> Result<Self, StreamError> {
        if cause.wire().1.is_some_and(|r| r.is_nil()) {
            return Err(StreamError::Baseline);
        }
        Ok(Self {
            session,
            cause,
            limits,
            baseline: None,
            pending: None,
            terminal: None,
            terminal_close: None,
            closed: false,
            generation: None,
            seq: 0,
            tick: 0,
            ack: 0,
            last_delta: None,
            motion_enabled: false,
            motion_capture: None,
            delivered_motion: None,
        })
    }
    pub fn binding(&self) -> &super::simulation::Binding {
        self.session.binding()
    }
    pub(crate) fn enable_motion(&mut self) {
        self.motion_enabled = true;
    }
    pub(crate) fn capture_motion(&mut self, capture: Option<MotionCapture>) {
        self.motion_capture = capture;
    }
    pub fn deadline(&self) -> &super::session::AccessDeadline {
        self.session.deadline()
    }
    pub fn accepts_input(&self) -> bool {
        !self.closed && self.baseline.is_some()
    }
    pub fn is_closed(&self) -> bool {
        self.closed
    }
    pub fn queued_frames(&self) -> usize {
        usize::from(self.pending.is_some()) + usize::from(self.terminal.is_some())
    }
    pub fn queued_bytes(&self) -> usize {
        self.pending.as_ref().map_or(0, |p| p.bytes.len())
            + self.terminal.as_ref().map_or(0, |p| p.bytes.len())
    }
    pub fn delivered_seq(&self) -> u64 {
        self.seq
    }
    fn stop(&mut self, kind: &'static str, reason: &'static str, code: u16) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.pending = None;
        self.baseline = None;
        self.generation = None;
        self.terminal_close = Some((reason, code));
        self.terminal = Some(Delivery {
            bytes: serde_json::to_vec(&json!({"protocolVersion":"1.0.0",
            "type":kind,"streamId":self.binding().stream_id,"reason":reason}))
            .expect("fixed control"),
            close_code: Some(code),
        });
    }
    pub fn revoke(&mut self, reason: RevokeReason) {
        self.session.revoke(reason);
        self.stop("revoke", reason.wire(), reason.close_code());
    }
    pub fn require_resync(&mut self, reason: &'static str) {
        // Restrict control reasons to frozen C05. Unknown internal reason fails closed.
        let reason = match reason {
            "GAP" | "EPOCH_CHANGED" | "MAP_VERSION_MISMATCH" | "BACKPRESSURE" => reason,
            _ => "GAP",
        };
        self.stop("resyncRequired", reason, 4409);
    }
    pub fn check_authority(
        &mut self,
        now: u64,
        membership: &impl MembershipPort,
    ) -> Result<(), StreamError> {
        if self.closed {
            return Err(StreamError::Closed);
        }
        if let Err(e) = self.session.check(now, membership) {
            let reason = match e {
                SessionError::Revoked(r) => r,
                _ => RevokeReason::PolicyUnavailable,
            };
            self.revoke(reason);
            return Err(StreamError::Session(e));
        }
        Ok(())
    }
    /// Preserve the original terminal disposition even when the socket cannot accept bytes.
    pub(crate) fn terminal_close(&self) -> Option<(&'static str, u16)> {
        self.terminal_close
    }
    /// Terminal controls contain no private state and can be drained without a room frame.
    pub fn take_terminal(&mut self) -> Option<Delivery> {
        self.terminal.take()
    }
    /// Adapter must call under owner/policy invalidation fence on every publish attempt and
    /// deadline sweep. Bytes returned later are ALWAYS refreshed against current authority.
    pub fn refresh(
        &mut self,
        frame: &RoomFrame,
        now: u64,
        membership: &impl MembershipPort,
        policy: &impl RecipientPolicy,
    ) -> Result<(), StreamError> {
        if self.closed {
            return Err(StreamError::Closed);
        }
        self.check_authority(now, membership)?;
        if !super::interest::same_room(&frame.scope, self.binding()) {
            let reason = if frame.scope.resources != self.binding().resources {
                "MAP_VERSION_MISMATCH"
            } else {
                "EPOCH_CHANGED"
            };
            self.require_resync(reason);
            return Err(StreamError::Baseline);
        }
        let (generation, players) = match project(frame, self.binding(), now, membership, policy) {
            Ok(p) => p,
            Err(e) => {
                self.revoke(RevokeReason::PolicyUnavailable);
                return Err(StreamError::Projection(e));
            }
        };
        if self.generation.is_some_and(|g| g != generation) {
            self.require_resync("GAP");
            return Err(StreamError::Baseline);
        }
        self.generation = Some(generation);
        let ack = players[&self.binding().key.user_id.uuid()]
            .state
            .ack_input_seq;
        let motion = if self.motion_enabled {
            let capture = self.motion_capture.as_ref().filter(|c| {
                c.binding == *self.binding() && c.anchor.server_tick == frame.tick
                    && c.anchor.position.x == players[&self.binding().key.user_id.uuid()].state.position.x
                    && c.anchor.position.y == players[&self.binding().key.user_id.uuid()].state.position.y
            });
            let Some(capture) = capture else {
                self.require_resync("GAP");
                return Err(StreamError::Baseline);
            };
            Some(capture.anchor.clone())
        } else { None };
        if frame.tick < self.tick || ack < self.ack || (self.baseline.is_none() && ack != 0) {
            self.require_resync("GAP");
            return Err(StreamError::Baseline);
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|p| now.saturating_sub(p.since) >= self.limits.slow_ms)
        {
            self.require_resync("BACKPRESSURE");
            return Err(StreamError::Backpressure);
        }
        let mut wire: Value = if let Some(base) = &self.baseline {
            let upserts: Vec<_> = players
                .iter()
                .filter(|(id, p)| base.get(id) != Some(p))
                .map(|(id, p)| p.wire(*id))
                .collect();
            let removes: Vec<_> = base
                .keys()
                .filter(|id| !players.contains_key(id))
                .copied()
                .collect();
            if upserts.is_empty() && removes.is_empty() && ack == self.ack && motion == self.delivered_motion {
                self.pending = None;
                return Ok(());
            }
            if self.seq >= MAX_SEQUENCE {
                self.require_resync("GAP");
                return Err(StreamError::Baseline);
            }
            json!({"protocolVersion":"1.0.0","type":"delta","streamId":self.binding().stream_id,
                "streamSeq":self.seq+1,"roomEpoch":self.binding().room_epoch,"serverTick":frame.tick,
                "ackInputSeq":ack,"upserts":upserts,"removes":removes})
        } else {
            let b = self.binding();
            let r = &b.resources;
            let (cause, request) = self.cause.wire();
            json!({"protocolVersion":"1.0.0","type":"snapshot","streamId":b.stream_id,"streamSeq":0,
                "roomEpoch":b.room_epoch,"serverTick":frame.tick,"eventId":b.key.event_id.uuid(),"roomId":b.room_id,
                "mapId":r.map_id.as_str(),"mapRevision":r.map_revision.get(),"avatarManifestId":r.avatar_manifest_id.as_str(),
                "avatarManifestVersion":r.avatar_manifest_version.get(),"cause":cause,"requestId":request,
                "selfUserId":b.key.user_id.uuid(),"ackInputSeq":0,
                "accessExpiresAt":self.deadline().utc.to_rfc3339_opts(SecondsFormat::Millis,true),
                "players":players.iter().map(|(id,p)|p.wire(*id)).collect::<Vec<_>>()})
        };
        if let Some(anchor) = &motion {
            wire["selfMotion"] = serde_json::to_value(anchor).expect("finite simulation anchor");
            if self.baseline.is_none() {
                wire["motionProfile"] = serde_json::to_value(&self.motion_capture.as_ref().expect("capture checked").profile).expect("validated profile");
            }
        }
        let bytes = serde_json::to_vec(&wire).expect("validated public projection");
        if bytes.len() > self.limits.frame_bytes
            || bytes.len() > self.limits.bytes
            || self.limits.frames < 1
        {
            self.require_resync("BACKPRESSURE");
            return Err(StreamError::Backpressure);
        }
        let since = self.pending.as_ref().map_or(now, |p| p.since);
        self.pending = Some(Pending {
            bytes,
            players,
            tick: frame.tick,
            ack,
            since,
            motion,
        });
        Ok(())
    }
    /// Handoff directly to the transport under the same fence. No second private-data queue.
    /// A failed/partial write must close and reconnect; it cannot replay this stream's bytes.
    pub fn next(
        &mut self,
        frame: &RoomFrame,
        now: u64,
        membership: &impl MembershipPort,
        policy: &impl RecipientPolicy,
    ) -> Option<Delivery> {
        let _ = self.refresh(frame, now, membership, policy);
        if self.terminal.is_some() {
            return self.terminal.take();
        }
        if self.closed
            || (self.baseline.is_some()
                && self
                    .last_delta
                    .is_some_and(|at| now.saturating_sub(at) < DELTA_INTERVAL_MS))
        {
            return None;
        }
        let p = self.pending.take()?;
        if self.baseline.is_some() {
            self.seq += 1;
        }
        self.tick = p.tick;
        self.ack = p.ack;
        self.delivered_motion = p.motion;
        self.baseline = Some(p.players);
        self.last_delta = Some(now);
        Some(Delivery {
            bytes: p.bytes,
            close_code: None,
        })
    }
}
