use chrono::{TimeZone, Utc};
use concert_domain::{
    place::{Geometry, Interaction, PlaceId, Spawn},
    world::{Input, Limits, WorldMap},
    AssetId, AuthUserId, AvatarSelection, EventId, ParticipantKey, Revision,
};
use concert_service::world::{interest::*, replication::*, session::*, simulation::*};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    io::Write,
    process::{Command, Stdio},
};
use uuid::Uuid;
fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}
fn asset(s: &str) -> AssetId {
    AssetId::new(s.into()).unwrap()
}
fn binding(n: u128) -> Binding {
    Binding {
        key: ParticipantKey {
            event_id: EventId::from_uuid(id(1)),
            user_id: AuthUserId::from_uuid(id(n)),
        },
        session_id: id(n + 100),
        connection_id: id(n + 200),
        owner_id: id(2),
        room_id: id(3),
        room_epoch: id(4),
        stream_id: id(n + 300),
        resources: ResourcePin {
            map_id: asset("map_test"),
            map_revision: Revision::initial(),
            avatar_manifest_id: asset("avatar_test"),
            avatar_manifest_version: Revision::initial(),
        },
    }
}
fn identity(n: u128) -> PublicIdentity {
    PublicIdentity {
        display_name: format!("Player {n}"),
        avatar_revision: Revision::initial(),
        avatar: AvatarSelection {
            manifest_id: asset("avatar_test"),
            manifest_version: Revision::initial(),
            appearance: concert_domain::AvatarAppearance::Layered {
                body: asset("body_test"),
                eyes: asset("eyes_test"),
                hair: asset("hair_test"),
                outfit: asset("outfit_test"),
                accessory: None,
                skin_tone: asset("tone_test"),
            },
        },
        facing: Facing::Down,
        moving: false,
    }
}
fn simulation(b: &Binding) -> Simulation {
    let g = Geometry::new(
        8,
        8,
        32,
        32,
        "collision_test".into(),
        vec![vec![0; 8]; 8],
        vec![vec![1; 8]; 8],
        vec![vec![0; 8]; 8],
        vec![Spawn {
            id: "spawn_test".into(),
            x: 1,
            y: 1,
        }],
        vec![Interaction {
            id: "inspect_test".into(),
            place_id: PlaceId::from_str("00000000-0000-4000-8000-000000000090").unwrap(),
            x: 2,
            y: 1,
        }],
        "spawn_test".into(),
    )
    .unwrap();
    Simulation::new(
        b.key.event_id,
        b.room_id,
        b.room_epoch,
        b.owner_id,
        b.resources.clone(),
        WorldMap::from_geometry(&g).unwrap(),
        Limits::new(120.0, 48.0, 250).unwrap(),
        50,
        0,
    )
    .unwrap()
}
use std::str::FromStr;
#[derive(Default)]
struct Authority {
    leases: RefCell<BTreeMap<Uuid, MembershipLease>>,
    failed: Cell<bool>,
}
impl Authority {
    fn grant(&self, b: &Binding, until: u64, access: u64) {
        self.leases.borrow_mut().insert(
            b.key.user_id.uuid(),
            MembershipLease {
                binding: b.clone(),
                lease_until_ms: until,
                access_expires_at_ms: access,
            },
        );
    }
}
impl MembershipPort for Authority {
    fn current(&self, b: &Binding, _: u64) -> Result<MembershipLease, AuthorityError> {
        if self.failed.get() {
            return Err(AuthorityError::Unavailable);
        }
        self.leases
            .borrow()
            .get(&b.key.user_id.uuid())
            .cloned()
            .ok_or(AuthorityError::Revoked)
    }
}
struct Policy {
    blocked: RefCell<BTreeSet<(Uuid, Uuid)>>,
    generation: Cell<Uuid>,
    failed: Cell<bool>,
    stale: Cell<bool>,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            blocked: RefCell::default(),
            generation: Cell::new(id(999)),
            failed: Cell::new(false),
            stale: Cell::new(false),
        }
    }
}
impl RecipientPolicy for Policy {
    fn current(
        &self,
        b: &Binding,
        subjects: &[Binding],
        now: u64,
    ) -> Result<RecipientView, ProjectionError> {
        if self.failed.get() {
            return Err(ProjectionError::PolicyUnavailable);
        }
        let blocked = self.blocked.borrow();
        let viewer = b.key.user_id.uuid();
        Ok(RecipientView {
            binding: b.clone(),
            invalidation_id: self.generation.get(),
            checked_at_ms: now,
            valid_until_ms: if self.stale.get() { now } else { now + 1000 },
            visible_users: subjects
                .iter()
                .map(|s| s.key.user_id.uuid())
                .filter(|s| !blocked.contains(&(viewer, *s)) && !blocked.contains(&(*s, viewer)))
                .collect(),
        })
    }
}
fn deadline() -> AccessDeadline {
    AccessDeadline {
        monotonic_ms: 10000,
        utc: Utc.with_ymd_and_hms(2026, 11, 4, 0, 0, 10).unwrap(),
    }
}
fn decode(delivery: Delivery) -> Value {
    assert!(delivery.bytes.len() <= SERVER_FRAME_BYTES);
    let v: Value = serde_json::from_slice(&delivery.bytes).unwrap();
    oracle(&v);
    v
}
/// Uses the repository's pinned C05 schema+semantic oracle, never a duplicate test schema.
fn oracle(v: &Value) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let script="import importlib.util,json,sys; s=importlib.util.spec_from_file_location('c05','scripts/mcr/validate-realtime.py'); m=importlib.util.module_from_spec(s); s.loader.exec_module(m); v=json.load(sys.stdin); m.Contract().validate(v,{'snapshot':'Snapshot','delta':'Delta','revoke':'Revoke','resyncRequired':'ResyncRequired'}[v['type']])";
    let mut child = Command::new("python3")
        .args(["-c", script])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::to_string(v).unwrap().as_bytes())
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "C05 oracle: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

use concert_service::world::recovery::*;
fn setup() -> (Recovery, Simulation, Binding, Authority, Policy) {
    let b = binding(10);
    let sim = simulation(&b);
    let a = Authority::default();
    a.grant(&b, 5000, 10000);
    (
        Recovery::new(50, QueueLimits::default()).unwrap(),
        sim,
        b,
        a,
        Policy::default(),
    )
}
fn frame(sim: &Simulation, b: &Binding) -> RoomFrame {
    RoomFrame::capture(sim, b.clone(), vec![(b.clone(), identity(10))]).unwrap()
}
fn snapshot(
    book: &mut Recovery,
    sim: &Simulation,
    b: &Binding,
    now: u64,
    a: &Authority,
    p: &Policy,
) -> Value {
    decode(
        book.stream_mut(b)
            .unwrap()
            .next(&frame(sim, b), now, a, p)
            .unwrap(),
    )
}
#[test]
fn initial_snapshot_gates_input_and_reconnect_clears_held_motion() {
    let (mut book, mut sim, b, a, p) = setup();
    assert!(book
        .activate(
            b.clone(),
            deadline(),
            SnapshotCause::Initial,
            "spawn_test",
            0,
            &a,
            &mut sim
        )
        .unwrap()
        .is_none());
    assert_eq!(
        book.submit(
            &b,
            Input::new(b.stream_id, b.room_epoch, 1, 1.0, 0.0, false).unwrap(),
            0,
            &a,
            &mut sim
        ),
        Err(RecoveryError::Stale)
    );
    assert_eq!(snapshot(&mut book, &sim, &b, 0, &a, &p)["cause"], "INITIAL");
    book.submit(
        &b,
        Input::new(b.stream_id, b.room_epoch, 1, 1.0, 0.0, false).unwrap(),
        0,
        &a,
        &mut sim,
    )
    .unwrap();
    sim.advance(50, &a).unwrap();
    // Accepted but unsimulated motion must also disappear during takeover.
    book.submit(
        &b,
        Input::new(b.stream_id, b.room_epoch, 2, 1.0, 0.0, true).unwrap(),
        50,
        &a,
        &mut sim,
    )
    .unwrap();
    let before = sim.state(id(10)).unwrap().position;
    let mut next = b.clone();
    next.connection_id = id(900);
    next.stream_id = id(901);
    a.grant(&next, 5000, 10000);
    assert_eq!(
        book.activate(
            next.clone(),
            deadline(),
            SnapshotCause::Initial,
            "spawn_test",
            50,
            &a,
            &mut sim
        )
        .unwrap(),
        Some(b.clone())
    );
    assert_eq!(book.len(), 1);
    assert_eq!(sim.pending_input_count(), 0);
    assert_eq!(sim.state(id(10)).unwrap().ack_input_seq, 0);
    assert!(book.stream_mut(&b).is_err());
    assert!(!book.disconnect(&b, &mut sim));
    let s = snapshot(&mut book, &sim, &next, 50, &a, &p);
    assert_eq!(s["streamSeq"], 0);
    assert_eq!(s["ackInputSeq"], 0);
    sim.advance(200, &a).unwrap();
    assert_eq!(sim.state(id(10)).unwrap().position, before);
    assert_eq!(
        book.submit(
            &next,
            Input::new(b.stream_id, b.room_epoch, 3, 1.0, 0.0, false).unwrap(),
            200,
            &a,
            &mut sim
        ),
        Err(RecoveryError::Motion)
    );
}
#[test]
fn resync_is_correlated_rate_bounded_and_never_reuses_streams_or_deadlines() {
    let (mut book, mut sim, b, a, p) = setup();
    book.activate(
        b.clone(),
        deadline(),
        SnapshotCause::Initial,
        "spawn_test",
        0,
        &a,
        &mut sim,
    )
    .unwrap();
    snapshot(&mut book, &sim, &b, 0, &a, &p);
    let mut next = b.clone();
    next.stream_id = id(500);
    a.grant(&next, 5000, 10000);
    assert_eq!(
        book.activate(
            next.clone(),
            deadline(),
            SnapshotCause::Resync(id(600)),
            "spawn_test",
            999,
            &a,
            &mut sim
        ),
        Err(RecoveryError::RateLimited)
    );
    book.activate(
        next.clone(),
        deadline(),
        SnapshotCause::Resync(id(600)),
        "spawn_test",
        1000,
        &a,
        &mut sim,
    )
    .unwrap();
    let s = snapshot(&mut book, &sim, &next, 1000, &a, &p);
    assert_eq!(s["cause"], "RESYNC");
    assert_eq!(s["requestId"], id(600).to_string());
    assert_eq!(s["accessExpiresAt"], "2026-11-04T00:00:10.000Z");
    a.grant(&b, 5000, 10000);
    assert_eq!(
        book.activate(
            b.clone(),
            deadline(),
            SnapshotCause::Resync(id(601)),
            "spawn_test",
            2000,
            &a,
            &mut sim
        ),
        Err(RecoveryError::Stale)
    );
    let mut invalid = next.clone();
    invalid.stream_id = id(501);
    invalid.session_id = id(800);
    a.grant(&invalid, 5000, 10000);
    assert_eq!(
        book.activate(
            invalid,
            deadline(),
            SnapshotCause::Resync(id(602)),
            "spawn_test",
            2000,
            &a,
            &mut sim
        ),
        Err(RecoveryError::Binding)
    );
    let mut invalid = next.clone();
    invalid.stream_id = id(502);
    let mut expiry = deadline();
    expiry.monotonic_ms = 12000;
    a.grant(&invalid, 5000, 12000);
    assert_eq!(
        book.activate(
            invalid,
            expiry,
            SnapshotCause::Resync(id(603)),
            "spawn_test",
            2000,
            &a,
            &mut sim
        ),
        Err(RecoveryError::Binding)
    );
    assert!(book.stream_mut(&next).is_ok());
}
#[test]
fn expired_revoked_suspended_and_unavailable_accounts_cannot_restore_subscriptions() {
    for reason in [
        RevokeReason::SessionExpired,
        RevokeReason::SessionRevoked,
        RevokeReason::Suspended,
        RevokeReason::EventClosed,
        RevokeReason::PolicyUnavailable,
    ] {
        let (mut book, mut sim, b, a, p) = setup();
        book.activate(
            b.clone(),
            deadline(),
            SnapshotCause::Initial,
            "spawn_test",
            0,
            &a,
            &mut sim,
        )
        .unwrap();
        snapshot(&mut book, &sim, &b, 0, &a, &p);
        let frozen = frame(&sim, &b);
        book.revoke(&b, reason, &mut sim).unwrap();
        assert_eq!(sim.player_count(), 0);
        let wire = book
            .stream_mut(&b)
            .unwrap()
            .next(&frozen, 1, &a, &p)
            .unwrap();
        assert_eq!(wire.close_code, Some(reason.close_code()));
        assert_eq!(decode(wire)["reason"], reason.wire());
        let mut next = b.clone();
        next.stream_id = id(700);
        a.grant(&next, 5000, 10000);
        assert_eq!(
            book.activate(
                next,
                deadline(),
                SnapshotCause::Resync(id(701)),
                "spawn_test",
                1000,
                &a,
                &mut sim
            ),
            Err(RecoveryError::ReconnectRequired)
        );
        a.leases.borrow_mut().remove(&id(10));
        let mut fresh = b.clone();
        fresh.connection_id = id(702);
        fresh.stream_id = id(703);
        assert_eq!(
            book.activate(
                fresh,
                deadline(),
                SnapshotCause::Initial,
                "spawn_test",
                1000,
                &a,
                &mut sim
            ),
            Err(RecoveryError::Authority)
        );
    }
}
#[test]
fn access_expiry_and_owner_mismatch_revoke_without_a_due_tick() {
    let (mut book, mut sim, b, a, p) = setup();
    book.activate(
        b.clone(),
        deadline(),
        SnapshotCause::Initial,
        "spawn_test",
        0,
        &a,
        &mut sim,
    )
    .unwrap();
    let f = frame(&sim, &b);
    let d = book
        .stream_mut(&b)
        .unwrap()
        .next(&f, 10000, &a, &p)
        .unwrap();
    assert_eq!(decode(d)["reason"], "SESSION_EXPIRED");
    assert_eq!(book.expire(10000, |b| sim.remove(b)), vec![b]);
    assert!(book.is_empty());
    assert_eq!(sim.player_count(), 0);
    let (mut book, mut sim, b, a, p) = setup();
    book.activate(
        b.clone(),
        deadline(),
        SnapshotCause::Initial,
        "spawn_test",
        0,
        &a,
        &mut sim,
    )
    .unwrap();
    let f = frame(&sim, &b);
    let mut newer = b.clone();
    newer.owner_id = id(900);
    a.grant(&newer, 5000, 10000);
    assert_eq!(
        decode(book.stream_mut(&b).unwrap().next(&f, 1, &a, &p).unwrap())["reason"],
        "CONNECTION_REPLACED"
    );
}
#[test]
fn epoch_restart_uses_new_simulation_snapshot_and_rejects_old_motion() {
    let (mut book, mut old_sim, b, a, p) = setup();
    book.activate(
        b.clone(),
        deadline(),
        SnapshotCause::Initial,
        "spawn_test",
        0,
        &a,
        &mut old_sim,
    )
    .unwrap();
    snapshot(&mut book, &old_sim, &b, 0, &a, &p);
    book.submit(
        &b,
        Input::new(b.stream_id, b.room_epoch, 1, 1.0, 0.0, false).unwrap(),
        0,
        &a,
        &mut old_sim,
    )
    .unwrap();
    old_sim.advance(50, &a).unwrap();
    let mut next = b.clone();
    next.owner_id = id(800);
    next.room_epoch = id(801);
    next.stream_id = id(802);
    a.grant(&next, 5000, 10000);
    let mut new_sim = simulation(&next);
    book.activate(
        next.clone(),
        deadline(),
        SnapshotCause::EpochChanged,
        "spawn_test",
        1000,
        &a,
        &mut new_sim,
    )
    .unwrap();
    old_sim.advance(1000, &a).unwrap();
    assert_eq!(old_sim.player_count(), 0);
    let s = snapshot(&mut book, &new_sim, &next, 1000, &a, &p);
    assert_eq!(s["cause"], "EPOCH_CHANGED");
    assert_eq!(s["players"][0]["position"]["x"], 48.0);
    assert_eq!(s["ackInputSeq"], 0);
    assert!(!book.disconnect(&b, &mut new_sim));
    assert_eq!(
        book.submit(
            &next,
            Input::new(b.stream_id, b.room_epoch, 2, 1.0, 0.0, false).unwrap(),
            1000,
            &a,
            &mut new_sim
        ),
        Err(RecoveryError::Motion)
    );
    // Real process restart has no memory of an old subscription: fresh ticket -> INITIAL.
    let mut restarted = Recovery::new(50, QueueLimits::default()).unwrap();
    let mut fresh = next.clone();
    fresh.connection_id = id(803);
    fresh.stream_id = id(804);
    a.grant(&fresh, 5000, 10000);
    let mut sim = simulation(&fresh);
    restarted
        .activate(
            fresh.clone(),
            deadline(),
            SnapshotCause::Initial,
            "spawn_test",
            1000,
            &a,
            &mut sim,
        )
        .unwrap();
    assert_eq!(
        snapshot(&mut restarted, &sim, &fresh, 1000, &a, &p)["cause"],
        "INITIAL"
    );
}
#[test]
fn room_switch_removes_old_owner_and_ignores_delayed_disconnect() {
    let (mut book, mut old_sim, b, a, p) = setup();
    book.activate(
        b.clone(),
        deadline(),
        SnapshotCause::Initial,
        "spawn_test",
        0,
        &a,
        &mut old_sim,
    )
    .unwrap();
    snapshot(&mut book, &old_sim, &b, 0, &a, &p);
    let mut next = b.clone();
    next.room_id = id(900);
    next.room_epoch = id(901);
    next.stream_id = id(902);
    a.grant(&next, 5000, 10000);
    let mut new_sim = simulation(&next);
    book.switch_room(
        &b,
        next.clone(),
        deadline(),
        id(903),
        "spawn_test",
        1000,
        &a,
        &mut old_sim,
        &mut new_sim,
    )
    .unwrap();
    assert_eq!(book.len(), 1);
    assert_eq!(old_sim.player_count(), 0);
    assert_eq!(new_sim.player_count(), 1);
    let s = snapshot(&mut book, &new_sim, &next, 1000, &a, &p);
    assert_eq!(s["cause"], "ROOM_SWITCH");
    assert_eq!(s["requestId"], id(903).to_string());
    assert!(!book.disconnect(&b, &mut new_sim));
    assert!(book.disconnect(&next, &mut new_sim));
    assert!(!book.disconnect(&next, &mut new_sim));
    assert_eq!(new_sim.player_count(), 0);
}
#[test]
fn capacity_and_failed_motion_reset_do_not_replace_existing_subscription() {
    let (mut book, mut sim, b, a, p) = setup();
    book.activate(
        b.clone(),
        deadline(),
        SnapshotCause::Initial,
        "spawn_test",
        0,
        &a,
        &mut sim,
    )
    .unwrap();
    snapshot(&mut book, &sim, &b, 0, &a, &p);
    let mut next = b.clone();
    next.stream_id = id(999);
    a.grant(&next, 5000, 10000);
    let mut wrong = simulation(&binding(12));
    assert_eq!(
        book.activate(
            next.clone(),
            deadline(),
            SnapshotCause::Resync(id(800)),
            "spawn_test",
            1000,
            &a,
            &mut wrong
        ),
        Err(RecoveryError::ReconnectRequired)
    );
    assert!(book.stream_mut(&b).is_ok());
    // Distinct case: a populated foreign owner must fail the actual BE12
    // replacement fence, without replacing the original subscription.
    let mut foreign = b.clone();
    foreign.room_id = id(700);
    foreign.room_epoch = id(701);
    foreign.stream_id = id(702);
    let mut foreign_sim = simulation(&foreign);
    a.grant(&foreign, 5000, 10000);
    foreign_sim
        .admit(foreign.clone(), "spawn_test", 0, &a)
        .unwrap();
    a.grant(&next, 5000, 10000);
    assert_eq!(
        book.activate(
            next,
            deadline(),
            SnapshotCause::Resync(id(801)),
            "spawn_test",
            1000,
            &a,
            &mut foreign_sim
        ),
        Err(RecoveryError::Motion)
    );
    assert!(book.stream_mut(&b).is_ok());
    assert!(sim.replication_state(&b).is_some());
    assert!(foreign_sim.replication_state(&foreign).is_some());
    assert!(Recovery::new(0, QueueLimits::default()).is_err());
    assert!(Recovery::new(10001, QueueLimits::default()).is_err());
}
#[test]
fn sweep_closes_revoked_authority_without_room_frame_and_capacity_is_enforced() {
    let (mut book, mut sim, b, a, _) = setup();
    book.activate(
        b.clone(),
        deadline(),
        SnapshotCause::Initial,
        "spawn_test",
        0,
        &a,
        &mut sim,
    )
    .unwrap();
    a.failed.set(true);
    let removed = book.sweep(1, &a);
    assert_eq!(removed, vec![b.clone()]);
    let control = book.stream_mut(&b).unwrap().take_terminal().unwrap();
    assert_eq!(control.close_code, Some(4500));
    assert_eq!(decode(control)["reason"], "POLICY_UNAVAILABLE");
    assert!(!book.stream_mut(&b).unwrap().accepts_input());
    let mut limited = Recovery::new(1, QueueLimits::default()).unwrap();
    a.failed.set(false);
    limited
        .activate(
            b.clone(),
            deadline(),
            SnapshotCause::Initial,
            "spawn_test",
            1,
            &a,
            &mut simulation(&b),
        )
        .unwrap();
    let other = binding(11);
    a.grant(&other, 5000, 10000);
    assert_eq!(
        limited.activate(
            other.clone(),
            deadline(),
            SnapshotCause::Initial,
            "spawn_test",
            1,
            &a,
            &mut simulation(&other)
        ),
        Err(RecoveryError::Capacity)
    );
    assert_eq!(limited.len(), 1);
}
#[test]
fn repeated_request_ids_and_room_switch_resource_changes_are_rejected() {
    let (mut book, mut sim, b, a, p) = setup();
    book.activate(
        b.clone(),
        deadline(),
        SnapshotCause::Initial,
        "spawn_test",
        0,
        &a,
        &mut sim,
    )
    .unwrap();
    snapshot(&mut book, &sim, &b, 0, &a, &p);
    let mut next = b.clone();
    next.stream_id = id(500);
    a.grant(&next, 5000, 10000);
    book.activate(
        next.clone(),
        deadline(),
        SnapshotCause::Resync(id(600)),
        "spawn_test",
        1000,
        &a,
        &mut sim,
    )
    .unwrap();
    snapshot(&mut book, &sim, &next, 1000, &a, &p);
    let mut third = next.clone();
    third.stream_id = id(501);
    a.grant(&third, 5000, 10000);
    assert_eq!(
        book.activate(
            third,
            deadline(),
            SnapshotCause::Resync(id(600)),
            "spawn_test",
            2000,
            &a,
            &mut sim
        ),
        Err(RecoveryError::Stale)
    );
    let mut switched = next.clone();
    switched.stream_id = id(502);
    switched.room_id = id(900);
    switched.room_epoch = id(901);
    switched.resources.map_revision = Revision::new(2).unwrap();
    a.grant(&switched, 5000, 10000);
    let mut dest = simulation(&switched);
    assert_eq!(
        book.switch_room(
            &next,
            switched,
            deadline(),
            id(602),
            "spawn_test",
            2000,
            &a,
            &mut sim,
            &mut dest
        ),
        Err(RecoveryError::Binding)
    );
    assert_eq!(dest.player_count(), 0);
}
#[test]
fn freshly_authorized_connection_can_rejoin_after_revocation_removes_simulation_player() {
    let (mut book, mut sim, b, a, p) = setup();
    book.activate(
        b.clone(),
        deadline(),
        SnapshotCause::Initial,
        "spawn_test",
        0,
        &a,
        &mut sim,
    )
    .unwrap();
    snapshot(&mut book, &sim, &b, 0, &a, &p);
    book.revoke(&b, RevokeReason::SessionRevoked, &mut sim)
        .unwrap();
    assert_eq!(sim.player_count(), 0);
    let mut new = b.clone();
    new.connection_id = id(900);
    new.stream_id = id(901);
    a.grant(&new, 5000, 10000);
    book.activate(
        new.clone(),
        deadline(),
        SnapshotCause::Initial,
        "spawn_test",
        1000,
        &a,
        &mut sim,
    )
    .unwrap();
    assert_eq!(
        snapshot(&mut book, &sim, &new, 1000, &a, &p)["cause"],
        "INITIAL"
    );
    assert_eq!(sim.player_count(), 1);
}
#[test]
fn multiroom_expiry_routes_cleanup_and_retains_failed_owner_for_retry() {
    let (mut book, mut sim_a, a, authority, p) = setup();
    book.activate(
        a.clone(),
        deadline(),
        SnapshotCause::Initial,
        "spawn_test",
        0,
        &authority,
        &mut sim_a,
    )
    .unwrap();
    snapshot(&mut book, &sim_a, &a, 0, &authority, &p);
    let mut b = binding(11);
    b.room_id = id(800);
    b.room_epoch = id(801);
    authority.grant(&b, 5000, 10000);
    let mut sim_b = simulation(&b);
    book.activate(
        b.clone(),
        deadline(),
        SnapshotCause::Initial,
        "spawn_test",
        0,
        &authority,
        &mut sim_b,
    )
    .unwrap();
    let removed = book.expire(10000, |binding| {
        if binding.room_id == a.room_id {
            sim_a.remove(binding)
        } else {
            false
        }
    });
    assert_eq!(removed, vec![a]);
    assert_eq!(book.len(), 1);
    assert_eq!(sim_a.player_count(), 0);
    assert_eq!(sim_b.player_count(), 1);
    assert!(book.stream_mut(&b).unwrap().is_closed());
    assert_eq!(book.expire(10000, |binding| sim_b.remove(binding)), vec![b]);
    assert!(book.is_empty());
    assert_eq!(sim_b.player_count(), 0);
}
#[test]
fn failed_expiry_cleanup_survives_fresh_cross_owner_reconnect_and_retry() {
    let (mut book, mut old_sim, b, a, _) = setup();
    book.activate(
        b.clone(),
        deadline(),
        SnapshotCause::Initial,
        "spawn_test",
        0,
        &a,
        &mut old_sim,
    )
    .unwrap();
    assert!(book.expire(10000, |_| false).is_empty());
    assert_eq!(old_sim.player_count(), 1);
    let mut fresh = b.clone();
    fresh.connection_id = id(900);
    fresh.stream_id = id(901);
    fresh.owner_id = id(902);
    fresh.room_epoch = id(903);
    let mut new_deadline = deadline();
    new_deadline.monotonic_ms = 20000;
    new_deadline.utc += chrono::Duration::seconds(10);
    a.grant(&fresh, 15000, 20000);
    let mut new_sim = simulation(&fresh);
    book.activate(
        fresh.clone(),
        new_deadline,
        SnapshotCause::Initial,
        "spawn_test",
        10000,
        &a,
        &mut new_sim,
    )
    .unwrap();
    assert_eq!(book.len(), 1);
    assert_eq!(book.pending_cleanup_count(), 1);
    assert!(book.stream_mut(&fresh).is_ok());
    assert!(book.retry_cleanup(|_| false).is_empty());
    assert_eq!(book.pending_cleanup_count(), 1);
    assert_eq!(
        book.retry_cleanup(|binding| old_sim.remove(binding)),
        vec![b]
    );
    assert_eq!(book.pending_cleanup_count(), 0);
    assert_eq!(old_sim.player_count(), 0);
    assert_eq!(new_sim.player_count(), 1);
}
#[test]
fn displaced_owner_cleanup_history_is_bounded_before_destination_admission() {
    let (_, mut first, b, a, _) = setup();
    let mut book = Recovery::new(1, QueueLimits::default()).unwrap();
    book.activate(
        b.clone(),
        deadline(),
        SnapshotCause::Initial,
        "spawn_test",
        0,
        &a,
        &mut first,
    )
    .unwrap();
    let mut second = b.clone();
    second.connection_id = id(900);
    second.stream_id = id(901);
    second.owner_id = id(902);
    second.room_epoch = id(903);
    a.grant(&second, 5000, 10000);
    let mut second_sim = simulation(&second);
    book.activate(
        second.clone(),
        deadline(),
        SnapshotCause::Initial,
        "spawn_test",
        1,
        &a,
        &mut second_sim,
    )
    .unwrap();
    assert_eq!(book.pending_cleanup_count(), 1);
    let mut third = second.clone();
    third.connection_id = id(910);
    third.stream_id = id(911);
    third.owner_id = id(912);
    third.room_epoch = id(913);
    a.grant(&third, 5000, 10000);
    let mut third_sim = simulation(&third);
    assert_eq!(
        book.activate(
            third,
            deadline(),
            SnapshotCause::Initial,
            "spawn_test",
            2,
            &a,
            &mut third_sim
        ),
        Err(RecoveryError::Capacity)
    );
    assert_eq!(third_sim.player_count(), 0);
    assert_eq!(book.pending_cleanup_count(), 1);
    assert!(book.stream_mut(&second).is_ok());
    // The failed third admission did not free the cleanup budget. Wrong-owner
    // disconnect must retain the second subscription closed until its owner can remove it.
    assert!(!book.disconnect(&second, &mut third_sim));
    assert_eq!(book.len(), 1);
    assert!(book.stream_mut(&second).unwrap().is_closed());
    assert_eq!(second_sim.player_count(), 1);
    assert_eq!(book.retry_cleanup(|old| first.remove(old)), vec![b]);
    assert!(book.disconnect(&second, &mut second_sim));
    assert!(book.is_empty());
    assert_eq!(book.pending_cleanup_count(), 0);
    assert_eq!(second_sim.player_count(), 0);
}
#[test]
fn disconnect_with_wrong_owner_retains_cleanup_and_cannot_orphan_membership() {
    let (mut book, mut actual, b, a, _) = setup();
    book.activate(
        b.clone(),
        deadline(),
        SnapshotCause::Initial,
        "spawn_test",
        0,
        &a,
        &mut actual,
    )
    .unwrap();
    let mut wrong = simulation(&binding(12));
    assert!(book.disconnect(&b, &mut wrong));
    assert_eq!(book.len(), 0);
    assert_eq!(book.pending_cleanup_count(), 1);
    assert_eq!(actual.player_count(), 1);
    assert_eq!(
        book.retry_cleanup(|binding| actual.remove(binding)),
        vec![b]
    );
    assert_eq!(book.pending_cleanup_count(), 0);
    assert_eq!(actual.player_count(), 0);
}
#[test]
fn one_transport_connection_cannot_acquire_subscriptions_for_multiple_principals_or_events() {
    let (mut book, mut first, b, a, _) = setup();
    book.activate(
        b.clone(),
        deadline(),
        SnapshotCause::Initial,
        "spawn_test",
        0,
        &a,
        &mut first,
    )
    .unwrap();
    for event_change in [false, true] {
        let mut second = binding(11);
        second.connection_id = b.connection_id;
        if event_change {
            second.key.user_id = b.key.user_id;
            second.key.event_id = EventId::from_uuid(id(900));
        }
        a.grant(&second, 5000, 10000);
        let mut destination = simulation(&second);
        assert_eq!(
            book.activate(
                second,
                deadline(),
                SnapshotCause::Initial,
                "spawn_test",
                0,
                &a,
                &mut destination
            ),
            Err(RecoveryError::Binding)
        );
        assert_eq!(destination.player_count(), 0);
    }
    assert_eq!(book.len(), 1);
    assert_eq!(first.player_count(), 1);
}
