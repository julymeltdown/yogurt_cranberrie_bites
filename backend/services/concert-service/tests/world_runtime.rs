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
fn auth_wire() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"protocolVersion":"1.0.0","type":"authenticate","ticket":"A".repeat(43),"eventId":id(10),"roomId":id(11),"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1})).unwrap()
}
fn input_wire(seq: u64) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"protocolVersion":"1.0.0","type":"input","streamId":id(20),"roomEpoch":id(30),"seq":seq,"axisX":1,"axisY":0,"buttons":[]})).unwrap()
}
fn active_ingress() -> ConnectionIngress {
    let mut i = ConnectionIngress::new(0).unwrap();
    i.receive(FragmentKind::Text, true, &auth_wire(), 0)
        .unwrap();
    i.activate(id(20), id(30), 0).unwrap();
    i
}

#[test]
fn default_canonical_authority_is_unavailable_and_never_admits() {
    let f = fixture(2, 1);
    let b = binding(&f, 1, 0);
    let mut adapter = MembershipAdapter::new(
        UnavailableCanonicalAuthority,
        ClockAnchor {
            monotonic_ms: 0,
            utc: utc(),
        },
        2,
    )
    .unwrap();
    assert!(matches!(
        adapter.install(&b, 0),
        Err(AuthorityError::Unavailable)
    ));
    assert_eq!(adapter.captured_count(), 0);
}
#[test]
fn fixed_clock_anchor_floors_deadlines_and_same_connection_cannot_extend_expiry() {
    let f = fixture(2, 1);
    let b = binding(&f, 1, 0);
    f.authority.grant(&b);
    let anchor = ClockAnchor {
        monotonic_ms: 0,
        utc: utc(),
    };
    assert_eq!(
        anchor
            .deadline(utc() + TimeDelta::microseconds(1000999))
            .unwrap(),
        1000
    );
    assert!(anchor.deadline(utc()).is_err());
    let mut adapter = MembershipAdapter::new(f.authority.clone(), anchor, 2).unwrap();
    adapter.install(&b, 0).unwrap();
    f.authority
        .0
        .lock()
        .unwrap()
        .records
        .get_mut(&(b.key.event_id.uuid(), b.key.user_id.uuid()))
        .unwrap()
        .access_expires_at += TimeDelta::milliseconds(1);
    assert!(adapter.current(&b, 1).is_err());
    assert!(adapter.install(&b, 1).is_err());
}
#[test]
fn canonical_adapter_rechecks_exact_binding_lease_and_release_fences_newer_connection() {
    let f = fixture(2, 1);
    let b = binding(&f, 1, 0);
    f.authority.grant(&b);
    let mut adapter = MembershipAdapter::new(
        f.authority.clone(),
        ClockAnchor {
            monotonic_ms: 0,
            utc: utc(),
        },
        2,
    )
    .unwrap();
    adapter.install(&b, 0).unwrap();
    assert!(adapter.current(&b, 4999).is_ok());
    assert!(adapter.current(&b, 5000).is_err());
    let mut newer = b.clone();
    newer.connection_id = id(900);
    newer.stream_id = id(901);
    f.authority.grant(&newer);
    adapter.install(&newer, 1).unwrap();
    assert!(adapter.current(&b, 1).is_err());
    adapter.release(&b).unwrap();
    assert!(adapter.current(&newer, 1).is_ok());
}
#[test]
fn one_process_cannot_start_two_independent_owners_for_the_same_room() {
    let f = fixture(2, 1);
    let duplicate = Owner::new(
        f.event,
        id(999),
        ClockAnchor {
            monotonic_ms: 0,
            utc: utc(),
        },
        vec![spec(f.rooms[0], 2)],
        f.authority.clone(),
        f.policy.clone(),
        f.socket.clone(),
    );
    assert!(matches!(duplicate, Err(RuntimeError::Owner)));
}
#[test]
fn runtime_cannot_publish_or_move_without_qualified_authority() {
    let mut f = fixture(2, 1);
    let b = binding(&f, 1, 0);
    assert_eq!(
        f.owner.install(b.clone(), SnapshotCause::Initial, 0),
        Err(RuntimeError::Authority)
    );
    assert_eq!(f.owner.active_count(), 0);
    assert!(f.socket.0.lock().unwrap().frames.is_empty());
}
#[test]
fn initial_snapshot_precedes_input_and_real_simulation_tick_drives_delta() {
    let mut f = fixture(2, 1);
    let b = binding(&f, 1, 0);
    enter(&mut f, &b);
    assert_eq!(
        f.owner.input(&b, input(&b, 1), 0),
        Err(RuntimeError::Motion)
    );
    f.owner.publish(&b, 0, true).unwrap();
    f.owner.input(&b, input(&b, 1), 0).unwrap();
    f.owner.maintain(50).unwrap();
    f.owner.publish(&b, 50, true).unwrap();
    assert_eq!(f.socket.0.lock().unwrap().frames.len(), 1);
    f.owner.maintain(100).unwrap();
    f.owner.publish(&b, 100, true).unwrap();
    let socket = f.socket.0.lock().unwrap();
    assert_eq!(socket.frames[0].1["type"], "snapshot");
    assert_eq!(socket.frames[0].1["streamSeq"], 0);
    assert_eq!(socket.frames[1].1["type"], "delta");
    assert_eq!(socket.frames[1].1["streamSeq"], 1);
    assert_eq!(socket.frames[1].1["serverTick"], 2);
    assert_eq!(socket.frames[1].1["ackInputSeq"], 1);
    assert_eq!(socket.frames[1].1["upserts"][0]["position"]["x"], 60.0);
}
#[test]
fn takeover_fences_old_input_disconnect_and_transport_before_new_snapshot() {
    let mut f = fixture(2, 1);
    let old = binding(&f, 1, 0);
    enter(&mut f, &old);
    f.owner.publish(&old, 0, true).unwrap();
    let mut new = old.clone();
    new.connection_id = id(901);
    new.stream_id = id(902);
    f.authority.grant(&new);
    f.owner
        .install(new.clone(), SnapshotCause::Initial, 1)
        .unwrap();
    assert_eq!(
        f.owner.input(&old, input(&old, 1), 1),
        Err(RuntimeError::Stale)
    );
    assert!(!f.owner.disconnect(&old, 1).unwrap());
    f.owner.publish(&new, 1, true).unwrap();
    assert_eq!(f.owner.active_count(), 1);
    let s = f.socket.0.lock().unwrap();
    assert!(s
        .closes
        .iter()
        .any(|(b, c)| b == &old && c.label == "CONNECTION_REPLACED"));
    assert_eq!(s.frames.last().unwrap().1["streamSeq"], 0);
    assert_eq!(s.frames.last().unwrap().0, new);
}
#[test]
fn target_capacity_failure_keeps_old_membership_and_stream_authoritative() {
    let mut f = fixture(1, 2);
    let old = binding(&f, 1, 0);
    let occupant = binding(&f, 2, 1);
    enter(&mut f, &old);
    enter(&mut f, &occupant);
    let mut target = old.clone();
    target.room_id = f.rooms[1];
    target.stream_id = id(909);
    assert_eq!(
        f.owner
            .install(target, SnapshotCause::RoomSwitch(id(910)), 1),
        Err(RuntimeError::Capacity)
    );
    f.owner.publish(&old, 1, true).unwrap();
    assert_eq!(f.socket.0.lock().unwrap().frames.last().unwrap().0, old);
}
#[test]
fn committed_room_switch_resets_stream_and_old_simulation_without_duplicate_membership() {
    let mut f = fixture(2, 2);
    let old = binding(&f, 1, 0);
    enter(&mut f, &old);
    f.owner.publish(&old, 0, true).unwrap();
    let mut target = old.clone();
    target.room_id = f.rooms[1];
    target.stream_id = id(909);
    f.authority.grant(&target);
    f.owner
        .install(target.clone(), SnapshotCause::RoomSwitch(id(910)), 1)
        .unwrap();
    f.owner.publish(&target, 1, true).unwrap();
    assert_eq!(f.owner.active_count(), 1);
    assert_eq!(
        f.owner.input(&old, input(&old, 1), 1),
        Err(RuntimeError::Stale)
    );
    let s = f.socket.0.lock().unwrap();
    let wire = &s.frames.last().unwrap().1;
    assert_eq!(wire["cause"], "ROOM_SWITCH");
    assert_eq!(wire["roomId"], target.room_id.to_string());
    assert_eq!(wire["ackInputSeq"], 0);
    assert_eq!(wire["streamSeq"], 0);
}
#[test]
fn current_policy_invalidation_clears_queued_private_projection_before_handoff() {
    let mut f = fixture(2, 1);
    let a = binding(&f, 1, 0);
    let b = binding(&f, 2, 0);
    enter(&mut f, &a);
    enter(&mut f, &b);
    f.owner.publish(&a, 0, true).unwrap();
    f.owner.publish(&b, 0, true).unwrap();
    f.owner.input(&b, input(&b, 1), 0).unwrap();
    f.owner.maintain(50).unwrap();
    {
        let mut p = f.policy.0.lock().unwrap();
        p.blocked
            .insert((a.key.user_id.uuid(), b.key.user_id.uuid()));
        p.generation += 1;
    }
    f.owner.publish(&a, 100, true).unwrap();
    let s = f.socket.0.lock().unwrap();
    let last = s.frames.iter().rev().find(|(who, _, _)| who == &a).unwrap();
    assert_eq!(last.1["type"], "resyncRequired");
    assert!(last.1.get("players").is_none());
    assert!(last.1.get("upserts").is_none());
    assert_eq!(last.2, Some(4409));
}
#[test]
fn transport_not_ready_does_not_allocate_or_queue_numbered_frames() {
    let mut f = fixture(2, 1);
    let b = binding(&f, 1, 0);
    enter(&mut f, &b);
    f.socket.0.lock().unwrap().ready = false;
    for now in [0, 50, 100] {
        f.owner.publish(&b, now, true).unwrap();
    }
    assert!(f.socket.0.lock().unwrap().frames.is_empty());
    f.socket.0.lock().unwrap().ready = true;
    f.owner.publish(&b, 150, true).unwrap();
    assert_eq!(f.socket.0.lock().unwrap().frames[0].1["streamSeq"], 0);
}
#[test]
fn failed_transport_write_closes_instead_of_replaying_bytes() {
    let mut f = fixture(2, 1);
    let b = binding(&f, 1, 0);
    enter(&mut f, &b);
    f.socket.0.lock().unwrap().failed = true;
    assert_eq!(f.owner.publish(&b, 0, true), Err(RuntimeError::Transport));
    f.owner.maintain(1).unwrap();
    assert!(f.socket.0.lock().unwrap().frames.is_empty());
    assert!(!f.socket.0.lock().unwrap().closes.is_empty());
}
#[test]
fn revocation_cleanup_runs_without_a_due_tick_or_socket_readiness() {
    let mut f = fixture(2, 1);
    let b = binding(&f, 1, 0);
    enter(&mut f, &b);
    f.socket.0.lock().unwrap().ready = false;
    f.authority.0.lock().unwrap().records.clear();
    f.owner.maintain(1).unwrap();
    assert_eq!(f.owner.active_count(), 0);
    assert!(f.socket.0.lock().unwrap().frames.is_empty());
    assert!(f
        .socket
        .0
        .lock()
        .unwrap()
        .closes
        .iter()
        .any(|(closed, _)| closed == &b));
}
#[test]
fn failed_cleanup_survives_disconnect_takeover_and_retries_after_active_slot_is_gone() {
    let mut f = fixture(2, 1);
    let b = binding(&f, 1, 0);
    enter(&mut f, &b);
    f.authority.0.lock().unwrap().fail_release = true;
    f.owner.disconnect(&b, 1).unwrap();
    assert!(f.owner.pending_cleanup_count() > 0);
    let mut new = b.clone();
    new.connection_id = id(990);
    new.stream_id = id(991);
    f.authority.grant(&new);
    f.owner
        .install(new.clone(), SnapshotCause::Initial, 2)
        .unwrap();
    f.owner.disconnect(&new, 3).unwrap();
    assert!(f.owner.pending_cleanup_count() > 0);
    f.authority.0.lock().unwrap().fail_release = false;
    f.owner.maintain(50).unwrap();
    assert_eq!(f.owner.pending_cleanup_count(), 0);
    assert_eq!(f.owner.active_count(), 0);
}
#[test]
fn shutdown_reports_active_cleanup_even_when_pending_budget_is_full() {
    let mut f = fixture(2, 1);
    let a = binding(&f, 1, 0);
    let b = binding(&f, 2, 0);
    enter(&mut f, &a);
    enter(&mut f, &b);
    f.authority.0.lock().unwrap().fail_release = true;
    f.owner.disconnect(&a, 1).unwrap();
    let mut replacement = b.clone();
    replacement.connection_id = id(992);
    replacement.stream_id = id(993);
    f.authority.grant(&replacement);
    f.owner
        .install(replacement.clone(), SnapshotCause::Initial, 2)
        .unwrap();
    let report = f.owner.shutdown();
    for expected in [&a, &b, &replacement] {
        assert!(report.remaining_cleanup.iter().any(|got| got == expected));
    }
    assert_eq!(f.owner.active_count(), 0);
    assert_eq!(
        f.owner.publish(&replacement, 3, true),
        Err(RuntimeError::Closed)
    );
}
#[tokio::test]
async fn process_queue_is_bounded_and_stop_does_not_wait_for_socket_work() {
    let f = fixture(2, 1);
    let b = binding(&f, 1, 0);
    let (mut handle, join) = f.owner.spawn();
    for _ in 0..OWNER_QUEUE {
        let (reply, _) = tokio::sync::oneshot::channel();
        handle
            .try_submit(Command::Input {
                binding: b.clone(),
                input: input(&b, 1),
                reply,
            })
            .unwrap();
    }
    let (reply, _) = tokio::sync::oneshot::channel();
    assert_eq!(
        handle.try_submit(Command::Input {
            binding: b.clone(),
            input: input(&b, 1),
            reply
        }),
        Err(RuntimeError::QueueFull)
    );
    assert_eq!(handle.queue_capacity(), 0);
    handle.stop();
    assert!(join.await.unwrap().remaining_cleanup.is_empty());
}

#[test]
fn origin_allowlist_is_exact_and_rejects_null_missing_duplicates_and_untrusted() {
    let policy = OriginPolicy::new(vec!["https://mcr.example".into()], false).unwrap();
    assert!(policy.check(&["https://mcr.example"]).is_ok());
    for origins in [
        vec![],
        vec!["null"],
        vec!["https://evil.example"],
        vec!["https://mcr.example/"],
        vec!["https://mcr.example", "https://mcr.example"],
    ] {
        assert!(policy.check(&origins).is_err());
    }
    for bad in [
        "https://mcr.example/",
        "https://user@mcr.example",
        "https://*.example",
        "http://mcr.example",
    ] {
        assert!(OriginPolicy::new(vec![bad.into()], false).is_err());
    }
    assert!(OriginPolicy::new(vec!["http://localhost:9000".into()], true).is_ok());
}
#[test]
fn frozen_negative_wire_fixtures_reject_duplicates_nonfinite_and_unknown_fields() {
    for raw in [
        include_bytes!("../../../../contracts/fixtures/realtime/duplicate-key.json").as_slice(),
        include_bytes!("../../../../contracts/fixtures/realtime/non-finite.json").as_slice(),
    ] {
        let fixture: serde_json::Value = serde_json::from_slice(raw).unwrap();
        assert_eq!(
            decode(fixture["payload"].as_str().unwrap().as_bytes())
                .err()
                .unwrap()
                .code,
            4400
        );
    }
    let mut wire: serde_json::Value = serde_json::from_slice(&auth_wire()).unwrap();
    wire["position"] = serde_json::json!({"x":999});
    assert!(decode(&serde_json::to_vec(&wire).unwrap()).is_err());
    assert!(decode(br#"{"protocolVersion":"1.0.0","type":"pong","heartbeatId":"00000000-0000-0000-0000-000000000020","x":{"k":1,"k":2}}"#).is_err());
}
#[test]
fn auth_is_first_and_three_second_deadline_cannot_be_extended_by_fragments() {
    let mut i = ConnectionIngress::new(0).unwrap();
    assert_eq!(
        i.receive(FragmentKind::Text, true, &input_wire(1), 0)
            .err()
            .unwrap(),
        SafeClose::AUTH_REQUIRED
    );
    let mut i = ConnectionIngress::new(0).unwrap();
    assert!(i
        .receive(FragmentKind::Text, false, b"{", 2999)
        .unwrap()
        .is_none());
    assert_eq!(
        i.receive(FragmentKind::Continuation, true, b"}", 3000)
            .err()
            .unwrap(),
        SafeClose::AUTH_TIMEOUT
    );
    let mut i = ConnectionIngress::new(0).unwrap();
    i.receive(FragmentKind::Text, true, &auth_wire(), 0)
        .unwrap();
    assert_eq!(
        i.receive(FragmentKind::Text, true, &input_wire(1), 1)
            .err()
            .unwrap(),
        SafeClose::AUTH_REQUIRED
    );
}
#[test]
fn fragments_are_aggregate_bounded_binary_utf8_and_depth_fail_with_safe_labels() {
    let mut i = ConnectionIngress::new(0).unwrap();
    assert!(i
        .receive(FragmentKind::Text, false, &vec![b' '; 4096], 0)
        .unwrap()
        .is_none());
    assert_eq!(
        i.receive(FragmentKind::Continuation, true, b" ", 0)
            .err()
            .unwrap(),
        SafeClose::TOO_BIG
    );
    assert_eq!(i.buffered_bytes(), 0);
    let mut i = ConnectionIngress::new(0).unwrap();
    assert_eq!(
        i.receive(FragmentKind::Binary, true, b"", 0)
            .err()
            .unwrap()
            .code,
        1003
    );
    assert_eq!(decode(&[0xff]).err().unwrap().code, 1007);
    let deep = format!("{}0{}", "[".repeat(17), "]".repeat(17));
    assert_eq!(decode(deep.as_bytes()).err().unwrap(), SafeClose::INVALID);
}
#[test]
fn valid_fragmented_auth_has_redacted_ticket_and_does_not_admit_itself() {
    let bytes = auth_wire();
    let mut i = ConnectionIngress::new(0).unwrap();
    assert!(i
        .receive(FragmentKind::Text, false, &bytes[..20], 0)
        .unwrap()
        .is_none());
    let message = i
        .receive(FragmentKind::Continuation, true, &bytes[20..], 1)
        .unwrap()
        .unwrap();
    assert!(!format!("{message:?}").contains(&"A".repeat(43)));
    let ClientMessage::Authenticate(intent) = message else {
        panic!("expected auth")
    };
    assert_eq!(intent.ticket.expose_for_consumption().len(), 43);
    assert_eq!(
        i.check_deadline(3000).err().unwrap(),
        SafeClose::AUTH_TIMEOUT
    );
}
#[test]
fn ingress_enforces_distinct_message_and_input_buckets() {
    let mut i = active_ingress();
    for seq in 1..=60 {
        i.receive(FragmentKind::Text, true, &input_wire(seq), 0)
            .unwrap();
    }
    assert_eq!(
        i.receive(FragmentKind::Text, true, &input_wire(61), 0)
            .err()
            .unwrap(),
        SafeClose::THROTTLED
    );
    let mut i = active_ingress();
    let resync=serde_json::to_vec(&serde_json::json!({"protocolVersion":"1.0.0","type":"resync","requestId":id(40),"streamId":id(20),"roomEpoch":id(30),"lastStreamSeq":0,"reason":"GAP"})).unwrap();
    for _ in 0..79 {
        i.receive(FragmentKind::Text, true, &resync, 0).unwrap();
    }
    assert_eq!(
        i.receive(FragmentKind::Text, true, &resync, 0)
            .err()
            .unwrap(),
        SafeClose::THROTTLED
    );
}
#[test]
fn heartbeat_requires_matching_pong_and_does_not_allow_input_to_delay_timeout() {
    let mut i = active_ingress();
    assert!(i.ping(9999, id(50)).unwrap().is_none());
    assert!(i.ping(10000, id(50)).unwrap().is_some());
    i.receive(FragmentKind::Text, true, &input_wire(1), 19999)
        .unwrap();
    assert_eq!(
        i.check_deadline(20000).err().unwrap().label,
        "HEARTBEAT_TIMEOUT"
    );
    let mut i = active_ingress();
    i.ping(10000, id(50)).unwrap();
    let pong = serde_json::to_vec(
        &serde_json::json!({"protocolVersion":"1.0.0","type":"pong","heartbeatId":id(50)}),
    )
    .unwrap();
    i.receive(FragmentKind::Text, true, &pong, 10001).unwrap();
    assert!(i.check_deadline(20000).is_ok());
    let mut i = active_ingress();
    assert!(i.receive(FragmentKind::Text, true, &pong, 1).is_err());
}
#[test]
fn ingress_rejects_stale_streams_noncanonical_ids_and_clock_regression() {
    let mut i = active_ingress();
    let mut wire: serde_json::Value = serde_json::from_slice(&input_wire(1)).unwrap();
    wire["streamId"] = serde_json::json!(id(99));
    assert_eq!(
        i.receive(
            FragmentKind::Text,
            true,
            &serde_json::to_vec(&wire).unwrap(),
            1
        )
        .err()
        .unwrap()
        .code,
        4409
    );
    let mut i = active_ingress();
    assert!(i.check_deadline(5).is_ok());
    assert_eq!(i.check_deadline(4).err().unwrap(), SafeClose::UNAVAILABLE);
    let mut wire: serde_json::Value = serde_json::from_slice(&auth_wire()).unwrap();
    wire["eventId"] = serde_json::json!("AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA");
    assert!(decode(&serde_json::to_vec(&wire).unwrap()).is_err());
}

#[test]
fn bounded_switch_transition_survives_a_committed_scope_change_between_owner_ticks() {
    let mut f = fixture(3, 2);
    let old = binding(&f, 1, 0);
    let observer = binding(&f, 2, 0);
    let elsewhere = binding(&f, 3, 1);
    for b in [&old, &observer, &elsewhere] {
        enter(&mut f, b);
        f.owner.publish(b, 0, true).unwrap();
    }
    let before = f.socket.0.lock().unwrap().frames.len();
    f.owner.input(&elsewhere, input(&elsewhere, 1), 0).unwrap();
    let request = id(2000);
    f.owner.begin_switch(&old, request, 0).unwrap();
    let mut next = old.clone();
    next.room_id = f.rooms[1];
    next.stream_id = id(3000);
    // The durable bridge has committed: the old actual canonical grant is gone.
    f.authority.grant(&next);
    f.owner.maintain(50).unwrap();
    f.owner.maintain(100).unwrap();
    f.owner.publish(&old, 100, true).unwrap();
    f.owner.publish(&observer, 100, true).unwrap();
    f.owner.publish(&elsewhere, 100, true).unwrap();
    let other_room_progress = {
        let socket = f.socket.0.lock().unwrap();
        assert!(!socket.closes.iter().any(|(b, _)| b == &old));
        assert!(!socket.frames[before..].iter().any(|(b, _, _)| b == &old));
        socket.frames[before..]
            .iter()
            .any(|(b, frame, _)| b == &elsewhere && frame["ackInputSeq"] == 1)
    };
    assert!(other_room_progress);
    f.owner
        .install(next.clone(), SnapshotCause::RoomSwitch(request), 100)
        .unwrap();
    f.owner.publish(&next, 100, true).unwrap();
    let socket = f.socket.0.lock().unwrap();
    let frame = &socket.frames.last().unwrap().1;
    assert_eq!(frame["type"], "snapshot");
    assert_eq!(frame["streamSeq"], 0);
    assert_eq!(frame["requestId"], request.to_string());
    assert_eq!(frame["roomId"], next.room_id.to_string());
    drop(socket);
    assert_eq!(
        f.owner.input(&old, input(&old, 1), 101),
        Err(RuntimeError::Stale)
    );
}
#[test]
fn rejected_switch_resumes_only_the_matching_still_current_subscription() {
    let mut f = fixture(2, 1);
    let b = binding(&f, 1, 0);
    enter(&mut f, &b);
    f.owner.publish(&b, 0, true).unwrap();
    let request = id(2001);
    f.owner.begin_switch(&b, request, 0).unwrap();
    f.owner.maintain(50).unwrap();
    assert_eq!(
        f.owner
            .reject_switch(&b, id(2002), SwitchRejection::RoomFull, 50),
        Err(RuntimeError::Stale)
    );
    f.owner
        .reject_switch(&b, request, SwitchRejection::RoomFull, 50)
        .unwrap();
    f.owner.input(&b, input(&b, 1), 50).unwrap();
    f.owner.maintain(100).unwrap();
    f.owner.publish(&b, 100, true).unwrap();
    let socket = f.socket.0.lock().unwrap();
    assert_eq!(socket.frames[1].1["type"], "switchRejected");
    assert_eq!(socket.frames[1].1["streamId"], b.stream_id.to_string());
    assert_eq!(socket.frames.last().unwrap().1["ackInputSeq"], 1);
    assert!(socket.closes.is_empty());
}
#[test]
fn abandoned_switch_expires_with_a_bounded_deadline_without_other_room_stalling() {
    let mut f = fixture(2, 2);
    let b = binding(&f, 1, 0);
    let other = binding(&f, 2, 1);
    enter(&mut f, &b);
    enter(&mut f, &other);
    f.owner.publish(&b, 0, true).unwrap();
    f.owner.publish(&other, 0, true).unwrap();
    f.owner.begin_switch(&b, id(2003), 0).unwrap();
    assert_eq!(
        f.owner.input(&b, input(&b, 1), 50),
        Err(RuntimeError::Stale)
    );
    f.owner
        .input(&other, input(&other, 1), SWITCH_TRANSITION_MS - 100)
        .unwrap();
    f.owner.maintain(SWITCH_TRANSITION_MS).unwrap();
    f.owner.publish(&other, SWITCH_TRANSITION_MS, true).unwrap();
    assert_eq!(f.owner.active_count(), 1);
    assert!(f
        .socket
        .0
        .lock()
        .unwrap()
        .closes
        .iter()
        .any(|(who, _)| who == &b));
    assert!(f
        .socket
        .0
        .lock()
        .unwrap()
        .frames
        .iter()
        .any(|(who, frame, _)| who == &other
            && frame["serverTick"].as_u64().is_some_and(|tick| tick > 0)));
}

#[test]
fn publication_pause_defers_native_send_and_preserves_authority_expiry() {
    let mut f = fixture(2, 1);
    let b = binding(&f, 1, 0);
    enter(&mut f, &b);
    f.authority.0.lock().unwrap().publication_paused = true;
    f.owner.publish(&b, 0, true).unwrap();
    assert!(f.socket.0.lock().unwrap().frames.is_empty());
    assert_eq!(f.owner.active_count(), 1);
    f.authority.0.lock().unwrap().publication_paused = false;
    f.owner.publish(&b, 1, true).unwrap();
    assert_eq!(f.socket.0.lock().unwrap().frames.len(), 1);
    f.authority.0.lock().unwrap().publication_paused = true;
    f.owner.maintain(5000).unwrap();
    assert_eq!(f.owner.active_count(), 0);
    assert_eq!(f.socket.0.lock().unwrap().frames.len(), 1);
    assert!(!f.socket.0.lock().unwrap().closes.is_empty());
}

#[test]
fn install_drains_confirmed_cleanup_without_waiting_for_another_tick() {
    let mut f = fixture(2, 1);
    let a = binding(&f, 1, 0);
    let b = binding(&f, 2, 0);
    enter(&mut f, &a);
    enter(&mut f, &b);
    f.authority.0.lock().unwrap().fail_release = true;
    f.owner.disconnect(&a, 1).unwrap();
    f.owner.disconnect(&b, 2).unwrap();
    let c = binding(&f, 3, 0);
    f.authority.grant(&c);
    assert_eq!(
        f.owner.install(c.clone(), SnapshotCause::Initial, 3),
        Err(RuntimeError::Capacity),
        "unconfirmed cleanup must still reserve bounded capacity"
    );
    f.authority.0.lock().unwrap().fail_release = false;
    f.owner.install(c, SnapshotCause::Initial, 4).unwrap();
    assert_eq!(f.owner.pending_cleanup_count(), 0);
    assert_eq!(f.owner.active_count(), 1);
}

#[tokio::test]
async fn readiness_requires_actual_owner_task_and_closes_after_shutdown() {
    let f = fixture(2, 1);
    let (handle, task) = f.owner.spawn();
    handle.ready().await.unwrap();
    handle.request_stop();
    assert!(task.await.unwrap().remaining_cleanup.is_empty());
    assert_eq!(handle.ready().await, Err(RuntimeError::Closed));
}
