//! Narrow motion owner regressions. Fixture helpers copied from world_runtime.rs at cf1ca629; authority/transport are explicit component fixtures.
//! Runtime unit fixtures do not qualify canonical production authority or IN01.
use chrono::{DateTime, TimeDelta, Utc};
use concert_domain::{
    place::{Geometry, Interaction, PlaceId, Spawn},
    world::{Input, Limits, WorldMap},
    AssetId, AuthUserId, AvatarSelection, EventId, ParticipantKey, Revision,
};
use concert_service::world::{ingress, interest, membership, replication, runtime, simulation};
use ingress::*;
use interest::{Facing, ProjectionError, PublicIdentity, RecipientPolicy, RecipientView};
use membership::*;
use replication::{Delivery, SnapshotCause};
use runtime::*;
use simulation::{AuthorityError, Binding, MembershipPort, ResourcePin};
use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use uuid::Uuid;

fn id(v: u128) -> Uuid {
    Uuid::from_u128(v)
}
fn utc() -> DateTime<Utc> {
    "2026-11-03T16:00:00Z".parse().unwrap()
}
fn asset(s: &str) -> AssetId {
    AssetId::new(s.into()).unwrap()
}
fn pins() -> ResourcePin {
    ResourcePin {
        map_id: asset("map_fixture"),
        map_revision: Revision::initial(),
        avatar_manifest_id: asset("avatar_fixture"),
        avatar_manifest_version: Revision::initial(),
    }
}
fn identity() -> PublicIdentity {
    PublicIdentity {
        display_name: "Fixture user".into(),
        avatar_revision: Revision::initial(),
        avatar: AvatarSelection {
            manifest_id: asset("avatar_fixture"),
            manifest_version: Revision::initial(),
            appearance: concert_domain::AvatarAppearance::Layered {
                body: asset("body_fixture"),
                eyes: asset("eyes_fixture"),
                hair: asset("hair_fixture"),
                outfit: asset("outfit_fixture"),
                accessory: None,
                skin_tone: asset("tone_fixture"),
            },
        },
        facing: Facing::Down,
        moving: false,
    }
}
fn spec(room: Uuid, capacity: usize) -> RoomSpec {
    let g = Geometry::new(
        12,
        8,
        32,
        32,
        "collision_fixture".into(),
        vec![vec![0; 12]; 8],
        vec![vec![1; 12]; 8],
        vec![vec![0; 12]; 8],
        vec![Spawn {
            id: "spawn_fixture".into(),
            x: 1,
            y: 2,
        }],
        vec![Interaction {
            id: "inspect_fixture".into(),
            place_id: PlaceId::from_str("00000000-0000-4000-8000-000000000090").unwrap(),
            x: 2,
            y: 2,
        }],
        "spawn_fixture".into(),
    )
    .unwrap();
    RoomSpec {
        room_id: room,
        room_epoch: id(6),
        resources: pins(),
        map: WorldMap::from_geometry(&g).unwrap(),
        limits: Limits::new(120.0, 48.0, 250).unwrap(),
        capacity,
        spawn_id: "spawn_fixture".into(),
    }
}
#[derive(Default)]
struct AuthorityState {
    records: BTreeMap<(Uuid, Uuid), CanonicalMembership>,
    fail_release: bool,
    unavailable: bool,
    publication_paused: bool,
}
#[derive(Clone, Default)]
struct Authority(Arc<Mutex<AuthorityState>>, Arc<Mutex<()>>);
impl Authority {
    fn grant(&self, b: &Binding) {
        self.0.lock().unwrap().records.insert(
            (b.key.event_id.uuid(), b.key.user_id.uuid()),
            CanonicalMembership {
                binding: b.clone(),
                checked_at_ms: 0,
                lease_expires_at: utc() + TimeDelta::milliseconds(5000),
                access_expires_at: utc() + TimeDelta::milliseconds(10000),
                identity: identity(),
            },
        );
    }
}
impl CanonicalAuthorityPort for Authority {
    fn publication_ready(&self, _: &Binding) -> bool {
        !self.0.lock().unwrap().publication_paused
    }
    fn publication_fence(&self) -> Arc<Mutex<()>> {
        self.1.clone()
    }
    fn current(&self, b: &Binding, now: u64) -> Result<CanonicalMembership, AuthorityError> {
        let state = self.0.lock().unwrap();
        if state.unavailable {
            return Err(AuthorityError::Unavailable);
        }
        let mut observed = state
            .records
            .get(&(b.key.event_id.uuid(), b.key.user_id.uuid()))
            .filter(|r| r.binding == *b)
            .cloned()
            .ok_or(AuthorityError::Revoked)?;
        observed.checked_at_ms = now;
        Ok(observed)
    }
    fn release(&self, b: &Binding) -> Result<(), AuthorityError> {
        let mut state = self.0.lock().unwrap();
        if state.fail_release {
            return Err(AuthorityError::Unavailable);
        }
        let key = (b.key.event_id.uuid(), b.key.user_id.uuid());
        if state.records.get(&key).is_some_and(|r| r.binding == *b) {
            state.records.remove(&key);
        }
        Ok(())
    }
}
#[derive(Default)]
struct PolicyState {
    blocked: BTreeSet<(Uuid, Uuid)>,
    unavailable: bool,
    generation: u128,
}
#[derive(Clone, Default)]
struct Policy(Arc<Mutex<PolicyState>>);
impl RecipientPolicy for Policy {
    fn current(
        &self,
        b: &Binding,
        subjects: &[Binding],
        now: u64,
    ) -> Result<RecipientView, ProjectionError> {
        let p = self.0.lock().unwrap();
        if p.unavailable {
            return Err(ProjectionError::PolicyUnavailable);
        }
        let actor = b.key.user_id.uuid();
        let visible_users = subjects
            .iter()
            .filter(|s| {
                let subject = s.key.user_id.uuid();
                !p.blocked.contains(&(actor, subject)) && !p.blocked.contains(&(subject, actor))
            })
            .map(|s| s.key.user_id.uuid())
            .collect();
        Ok(RecipientView {
            binding: b.clone(),
            invalidation_id: id(100 + p.generation),
            checked_at_ms: now,
            valid_until_ms: now + 1000,
            visible_users,
        })
    }
}
#[derive(Default)]
struct SocketState {
    ready: bool,
    failed: bool,
    installed_failed: bool,
    frames: Vec<(Binding, serde_json::Value, Option<u16>)>,
    closes: Vec<(Binding, SafeClose)>,
}
#[derive(Clone, Default)]
struct Socket(Arc<Mutex<SocketState>>);
impl TransportPort for Socket {
    fn installed(&mut self, _: &Binding, _: u64) -> Result<(), WriteError> {
        if self.0.lock().unwrap().installed_failed {
            Err(WriteError::Failed)
        } else {
            Ok(())
        }
    }
    fn ready(&self, _: &Binding) -> bool {
        self.0.lock().unwrap().ready
    }
    fn send(&mut self, b: &Binding, d: Delivery) -> Result<(), WriteError> {
        let mut s = self.0.lock().unwrap();
        if s.failed || s.frames.len() >= 128 {
            return Err(WriteError::Failed);
        }
        assert!(d.bytes.len() <= 65536);
        s.frames.push((
            b.clone(),
            serde_json::from_slice(&d.bytes).unwrap(),
            d.close_code,
        ));
        Ok(())
    }
    fn close(&mut self, b: &Binding, c: SafeClose) {
        let mut s = self.0.lock().unwrap();
        if s.closes.len() < 128 {
            s.closes.push((b.clone(), c));
        }
    }
}
type Owner = WorldRuntime<Authority, Policy, Socket>;
struct Fixture {
    owner: Owner,
    authority: Authority,
    policy: Policy,
    socket: Socket,
    event: EventId,
    rooms: Vec<Uuid>,
}
static FIXTURE_ID: AtomicU64 = AtomicU64::new(10000);
fn fixture(capacity: usize, rooms: usize) -> Fixture {
    let event = EventId::from_uuid(id(FIXTURE_ID.fetch_add(1, Ordering::Relaxed) as u128));
    let room_ids: Vec<_> = (0..rooms).map(|n| id(50 + n as u128)).collect();
    let authority = Authority::default();
    let policy = Policy::default();
    let socket = Socket::default();
    socket.0.lock().unwrap().ready = true;
    let owner = Owner::new(
        event,
        id(4),
        ClockAnchor {
            monotonic_ms: 0,
            utc: utc(),
        },
        room_ids.iter().map(|r| spec(*r, capacity)).collect(),
        authority.clone(),
        policy.clone(),
        socket.clone(),
    )
    .unwrap();
    Fixture {
        owner,
        authority,
        policy,
        socket,
        event,
        rooms: room_ids,
    }
}
fn binding(f: &Fixture, user: u128, room: usize) -> Binding {
    Binding {
        key: ParticipantKey {
            event_id: f.event,
            user_id: AuthUserId::from_uuid(id(user)),
        },
        session_id: id(2 + user),
        connection_id: id(200 + user),
        owner_id: id(4),
        room_id: f.rooms[room],
        room_epoch: id(6),
        stream_id: id(700 + user),
        resources: pins(),
    }
}
fn enter(f: &mut Fixture, b: &Binding) {
    f.authority.grant(b);
    f.owner
        .install(b.clone(), SnapshotCause::Initial, 0)
        .unwrap();
}
fn input(b: &Binding, seq: u64) -> Input {
    Input::new(b.stream_id, b.room_epoch, seq, 1.0, 0.0, false).unwrap()
}

fn configure(f: &mut Fixture) {
    f.owner
        .configure_motion_profile(&format!("sha256:{}", "0".repeat(64)))
        .unwrap();
}
#[test]
fn opt_in_adds_complete_anchor_without_changing_legacy_frames() {
    let mut legacy = fixture(2, 1);
    configure(&mut legacy);
    let old = binding(&legacy, 1, 0);
    enter(&mut legacy, &old);
    legacy.owner.publish(&old, 0, true).unwrap();
    assert!(legacy.socket.0.lock().unwrap().frames[0]
        .1
        .get("motionProfile")
        .is_none());
    let mut f = fixture(2, 1);
    configure(&mut f);
    let b = binding(&f, 1, 0);
    f.authority.grant(&b);
    f.owner
        .install_motion(b.clone(), SnapshotCause::Initial, 0)
        .unwrap();
    assert!(f.owner.input(&b, input(&b, 1), 0).is_err());
    f.owner.publish(&b, 0, true).unwrap();
    f.owner
        .input(
            &b,
            Input::new(b.stream_id, b.room_epoch, 1, 0.0, 0.0, false).unwrap(),
            0,
        )
        .unwrap();
    f.owner.maintain(100).unwrap();
    f.owner.publish(&b, 100, true).unwrap();
    f.owner.maintain(200).unwrap();
    f.owner.publish(&b, 200, true).unwrap();
    let socket = f.socket.0.lock().unwrap();
    let baseline = &socket.frames[0].1;
    let delta = &socket.frames[1].1;
    assert_eq!(baseline["motionProfile"]["speedPxPerSecond"], 120.0);
    assert_eq!(delta["selfMotion"]["inputAppliedTicks"], 2);
    assert_eq!(delta["selfMotion"]["serverTick"], delta["serverTick"]);
    assert_eq!(
        delta["selfMotion"]["position"],
        baseline["players"][0]["position"]
    );
    assert_eq!(delta["selfMotion"]["consumedInputSeq"], 1);
    let stationary = &socket.frames[2].1;
    assert!(stationary["upserts"].as_array().unwrap().is_empty());
    assert_eq!(stationary["selfMotion"]["inputAppliedTicks"], 4);
    assert_eq!(
        stationary["selfMotion"]["position"],
        baseline["players"][0]["position"]
    );
}
#[test]
fn unconfigured_negotiation_falls_back_without_inventing_geometry() {
    let mut f = fixture(2, 1);
    let b = binding(&f, 1, 0);
    f.authority.grant(&b);
    f.owner
        .install_motion(b.clone(), SnapshotCause::Initial, 0)
        .unwrap();
    f.owner.publish(&b, 0, true).unwrap();
    let socket = f.socket.0.lock().unwrap();
    assert!(socket.frames[0].1.get("motionProfile").is_none());
    assert!(socket.frames[0].1.get("selfMotion").is_none());
}
#[test]
fn replacement_stream_has_fresh_duration_while_epoch_counter_survives() {
    let mut f = fixture(2, 1);
    configure(&mut f);
    let b = binding(&f, 1, 0);
    f.authority.grant(&b);
    f.owner
        .install_motion(b.clone(), SnapshotCause::Initial, 0)
        .unwrap();
    f.owner.publish(&b, 0, true).unwrap();
    f.owner.input(&b, input(&b, 1), 0).unwrap();
    f.owner.maintain(1000).unwrap();
    let mut next = b.clone();
    next.connection_id = id(9000);
    next.stream_id = id(9001);
    f.authority.grant(&next);
    f.owner
        .install_motion(next.clone(), SnapshotCause::Initial, 1000)
        .unwrap();
    f.owner.publish(&next, 1000, true).unwrap();
    assert!(f.owner.input(&b, input(&b, 2), 1000).is_err());
    let socket = f.socket.0.lock().unwrap();
    let frame = &socket.frames.last().unwrap().1;
    assert_eq!(frame["ackInputSeq"], 0);
    assert_eq!(frame["selfMotion"]["consumedInputSeq"], 0);
    assert_eq!(frame["selfMotion"]["inputAppliedTicks"], 0);
    assert!(frame["selfMotion"]["heldInput"].is_null());
    assert_eq!(frame["selfMotion"]["droppedTicks"], 16);
}
#[test]
fn denied_admission_does_not_install_or_publish_motion() {
    let mut f = fixture(2, 1);
    configure(&mut f);
    let b = binding(&f, 1, 0);
    assert!(f
        .owner
        .install_motion(b.clone(), SnapshotCause::Initial, 0)
        .is_err());
    assert!(f.owner.publish(&b, 0, true).is_err());
    assert!(f.socket.0.lock().unwrap().frames.is_empty());
}
#[test]
fn different_geometry_revision_cannot_receive_the_configured_profile() {
    let mut f = fixture(2, 1);
    configure(&mut f);
    let mut b = binding(&f, 1, 0);
    b.resources.map_revision = Revision::new(2).unwrap();
    f.authority.grant(&b);
    assert!(f
        .owner
        .install_motion(b, SnapshotCause::Initial, 0)
        .is_err());
    assert!(f.socket.0.lock().unwrap().frames.is_empty());
}
