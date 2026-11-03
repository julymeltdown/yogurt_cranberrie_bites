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
fn replica(b: &Binding, a: &Authority, limits: QueueLimits) -> Replication {
    Replication::new(
        Session::new(b.clone(), deadline(), 0, a).unwrap(),
        SnapshotCause::Initial,
        limits,
    )
    .unwrap()
}
struct Fixture {
    sim: Simulation,
    bindings: Vec<Binding>,
    identities: Vec<PublicIdentity>,
    authority: Authority,
    policy: Policy,
}
impl Fixture {
    fn new() -> Self {
        let bindings: Vec<_> = (10..13).map(binding).collect();
        let authority = Authority::default();
        let mut sim = simulation(&bindings[0]);
        for b in &bindings {
            authority.grant(b, 5000, 10000);
            sim.admit(b.clone(), "spawn_test", 0, &authority).unwrap();
        }
        Self {
            sim,
            bindings,
            identities: (10..13).map(identity).collect(),
            authority,
            policy: Policy::default(),
        }
    }
    fn frame(&self) -> RoomFrame {
        RoomFrame::capture(
            &self.sim,
            self.bindings[0].clone(),
            self.bindings.iter().cloned().zip(self.identities.clone()),
        )
        .unwrap()
    }
    fn motion(&mut self, index: usize, seq: u64, now: u64) {
        let b = &self.bindings[index];
        self.sim
            .submit(
                b,
                Input::new(b.stream_id, b.room_epoch, seq, 1.0, 0.0, false).unwrap(),
                now,
                &self.authority,
            )
            .unwrap();
        self.sim.advance(now + 50, &self.authority).unwrap();
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
/// Synthetic metadata checks the wire variant, not catalog or product-art admission.
#[test]
fn whole_character_snapshot_and_delta_preserve_canonical_config() {
    let mut f = Fixture::new();
    f.identities[1].avatar.appearance = concert_domain::AvatarAppearance::WholeCharacter {
        preset_id: asset("preset_fixture"),
    };
    let viewer = &f.bindings[0];
    let mut r = replica(viewer, &f.authority, QueueLimits::default());
    let snapshot = decode(r.next(&f.frame(), 0, &f.authority, &f.policy).unwrap());
    let subject = f.bindings[1].key.user_id.uuid().to_string();
    let player = snapshot["players"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["userId"] == subject)
        .unwrap();
    let config = &player["avatar"]["config"];
    assert_eq!(config["contract_version"], "1.1.0");
    assert_eq!(config["representation"], "whole_character");
    assert_eq!(config["preset_id"], "preset_fixture");
    assert!(config.get("item_ids").is_none());
    assert!(config.get("skin_tone_id").is_none());
    let layered = &snapshot["players"][0]["avatar"]["config"];
    assert_eq!(layered["contract_version"], "1.0.0");
    assert_eq!(layered["item_ids"]["body"], "body_test");
    f.identities[1].avatar_revision = Revision::new(2).unwrap();
    f.identities[1].avatar.appearance = concert_domain::AvatarAppearance::WholeCharacter {
        preset_id: asset("preset_changed"),
    };
    let delta = decode(r.next(&f.frame(), 100, &f.authority, &f.policy).unwrap());
    assert_eq!(delta["upserts"].as_array().unwrap().len(), 1);
    assert_eq!(delta["upserts"][0]["userId"], subject);
    assert_eq!(delta["upserts"][0]["avatar"]["revision"], 2);
    assert_eq!(
        delta["upserts"][0]["avatar"]["config"]["preset_id"],
        "preset_changed"
    );
    assert!(delta["upserts"][0]["avatar"]["config"]
        .get("item_ids")
        .is_none());
}
#[test]
fn two_filtered_viewers_have_no_false_gaps_and_avatar_versions_upsert() {
    let mut f = Fixture::new();
    let a = f.bindings[0].clone();
    let b = f.bindings[1].clone();
    f.policy
        .blocked
        .borrow_mut()
        .insert((a.key.user_id.uuid(), id(12)));
    let mut ra = replica(&a, &f.authority, QueueLimits::default());
    let mut rb = replica(&b, &f.authority, QueueLimits::default());
    let sa = decode(ra.next(&f.frame(), 0, &f.authority, &f.policy).unwrap());
    let sb = decode(rb.next(&f.frame(), 0, &f.authority, &f.policy).unwrap());
    assert_eq!(sa["players"].as_array().unwrap().len(), 2);
    assert_eq!(sb["players"].as_array().unwrap().len(), 3);
    f.motion(2, 1, 0);
    assert!(ra.next(&f.frame(), 100, &f.authority, &f.policy).is_none());
    let db = decode(rb.next(&f.frame(), 100, &f.authority, &f.policy).unwrap());
    assert_eq!(db["streamSeq"], 1);
    f.identities[1].avatar_revision = Revision::new(2).unwrap();
    let concert_domain::AvatarAppearance::Layered { hair, .. } =
        &mut f.identities[1].avatar.appearance
    else {
        panic!("layered fixture required");
    };
    *hair = asset("hair_changed");
    let da = decode(ra.next(&f.frame(), 200, &f.authority, &f.policy).unwrap());
    let db = decode(rb.next(&f.frame(), 200, &f.authority, &f.policy).unwrap());
    assert_eq!(da["streamSeq"], 1);
    assert_eq!(db["streamSeq"], 2);
    assert_eq!(da["upserts"][0]["avatar"]["revision"], 2);
    assert!(!serde_json::to_string(&da).unwrap().contains("session"));
}
#[test]
fn coalescing_uses_delivered_baseline_and_fresh_snapshot_baseline() {
    let mut f = Fixture::new();
    let mut r = replica(&f.bindings[0], &f.authority, QueueLimits::default());
    r.refresh(&f.frame(), 0, &f.authority, &f.policy).unwrap();
    f.motion(1, 1, 0);
    let snapshot = decode(r.next(&f.frame(), 50, &f.authority, &f.policy).unwrap());
    assert_eq!(snapshot["streamSeq"], 0);
    assert_eq!(snapshot["players"][1]["position"]["x"], 54.0);
    f.motion(1, 2, 50);
    r.refresh(&f.frame(), 100, &f.authority, &f.policy).unwrap();
    f.motion(1, 3, 100);
    r.refresh(&f.frame(), 150, &f.authority, &f.policy).unwrap();
    assert_eq!(r.queued_frames(), 1);
    let delta = decode(r.next(&f.frame(), 150, &f.authority, &f.policy).unwrap());
    assert_eq!(delta["streamSeq"], 1);
    assert_eq!(delta["upserts"][0]["position"]["x"], 66.0);
}
#[test]
fn policy_change_purges_queued_identity_before_publication_and_requires_new_baseline() {
    let f = Fixture::new();
    let mut r = replica(&f.bindings[0], &f.authority, QueueLimits::default());
    decode(r.next(&f.frame(), 0, &f.authority, &f.policy).unwrap());
    r.refresh(&f.frame(), 10, &f.authority, &f.policy).unwrap();
    f.policy.blocked.borrow_mut().insert((id(12), id(10)));
    f.policy.generation.set(id(1000));
    let d = r.next(&f.frame(), 100, &f.authority, &f.policy).unwrap();
    assert_eq!(d.close_code, Some(4409));
    let wire = decode(d);
    assert_eq!(wire["type"], "resyncRequired");
    assert!(wire.get("players").is_none());
    assert!(!r.accepts_input());
    assert_eq!(r.queued_bytes(), 0);
    assert!(r.next(&f.frame(), 200, &f.authority, &f.policy).is_none());
}
#[test]
fn stale_unavailable_and_missing_self_policy_fail_closed_even_with_pending_snapshot() {
    for mode in 0..4 {
        let f = Fixture::new();
        let mut r = replica(&f.bindings[0], &f.authority, QueueLimits::default());
        r.refresh(&f.frame(), 0, &f.authority, &f.policy).unwrap();
        match mode {
            0 => f.policy.failed.set(true),
            1 => f.policy.stale.set(true),
            2 => {
                f.policy.blocked.borrow_mut().insert((id(10), id(10)));
            }
            _ => f.authority.failed.set(true),
        };
        let d = r.next(&f.frame(), 1, &f.authority, &f.policy).unwrap();
        assert_eq!(d.close_code, Some(4500));
        assert_eq!(decode(d)["reason"], "POLICY_UNAVAILABLE");
        assert!(!r.accepts_input());
    }
}
#[test]
fn queued_subject_revocation_removes_private_data_before_send() {
    let f = Fixture::new();
    let mut r = replica(&f.bindings[0], &f.authority, QueueLimits::default());
    r.refresh(&f.frame(), 0, &f.authority, &f.policy).unwrap();
    f.authority.leases.borrow_mut().remove(&id(12));
    let d = decode(r.next(&f.frame(), 1, &f.authority, &f.policy).unwrap());
    assert_eq!(d["type"], "revoke");
}
#[test]
fn frame_and_byte_bounds_and_slow_consumer_close_truthfully() {
    let f = Fixture::new();
    for limits in [
        QueueLimits::new(256, 262144, 64, 5000).unwrap(),
        QueueLimits::new(65536, 256, 1, 5000).unwrap(),
    ] {
        let mut r = replica(&f.bindings[0], &f.authority, limits);
        assert_eq!(
            r.refresh(&f.frame(), 0, &f.authority, &f.policy),
            Err(StreamError::Backpressure)
        );
        assert!(r.queued_frames() <= 1);
        assert!(r.queued_bytes() <= 256);
        let d = r.next(&f.frame(), 1, &f.authority, &f.policy).unwrap();
        assert_eq!(d.close_code, Some(4409));
        assert_eq!(decode(d)["reason"], "BACKPRESSURE");
    }
    let mut r = replica(
        &f.bindings[0],
        &f.authority,
        QueueLimits::new(65536, 262144, 1, 1000).unwrap(),
    );
    for now in 0..1000 {
        r.refresh(&f.frame(), now, &f.authority, &f.policy).unwrap();
        assert_eq!(r.queued_frames(), 1);
        assert!(r.queued_bytes() <= 65536);
    }
    assert_eq!(
        decode(r.next(&f.frame(), 1000, &f.authority, &f.policy).unwrap())["reason"],
        "BACKPRESSURE"
    );
    assert!(QueueLimits::new(65537, 262144, 64, 1000).is_none());
    assert!(QueueLimits::new(65536, 262145, 64, 1000).is_none());
    assert!(QueueLimits::new(65536, 262144, 65, 1000).is_none());
}
#[test]
fn delta_rate_ack_only_changes_and_removed_players_follow_baseline() {
    let mut f = Fixture::new();
    let mut r = replica(&f.bindings[0], &f.authority, QueueLimits::default());
    decode(r.next(&f.frame(), 0, &f.authority, &f.policy).unwrap());
    f.motion(0, 1, 0);
    assert!(r.next(&f.frame(), 50, &f.authority, &f.policy).is_none());
    let d = decode(r.next(&f.frame(), 100, &f.authority, &f.policy).unwrap());
    assert_eq!(d["ackInputSeq"], 1);
    let removed = f.bindings.pop().unwrap();
    f.identities.pop();
    f.sim.remove(&removed);
    let d = decode(r.next(&f.frame(), 200, &f.authority, &f.policy).unwrap());
    assert_eq!(d["streamSeq"], 2);
    assert_eq!(d["removes"][0], id(12).to_string());
}
#[test]
fn stale_clock_tick_epoch_and_resource_changes_are_terminal() {
    let mut f = Fixture::new();
    let stale = f.frame();
    f.sim.advance(250, &f.authority).unwrap();
    let mut r = replica(&f.bindings[0], &f.authority, QueueLimits::default());
    decode(r.next(&f.frame(), 300, &f.authority, &f.policy).unwrap());
    assert_eq!(
        decode(r.next(&stale, 400, &f.authority, &f.policy).unwrap())["reason"],
        "GAP"
    );
    let mut r = replica(&f.bindings[0], &f.authority, QueueLimits::default());
    r.refresh(&f.frame(), 100, &f.authority, &f.policy).unwrap();
    assert_eq!(
        decode(r.next(&f.frame(), 99, &f.authority, &f.policy).unwrap())["reason"],
        "POLICY_UNAVAILABLE"
    );
    for resource in [false, true] {
        let mut r = replica(&f.bindings[0], &f.authority, QueueLimits::default());
        // Scope is private: construct an actual new owner/room frame instead of mutating it.
        let mut bs = f.bindings.clone();
        for b in &mut bs {
            if resource {
                b.resources.map_revision = Revision::new(2).unwrap()
            } else {
                b.room_epoch = id(400)
            }
        }
        let mut sim = simulation(&bs[0]);
        for b in &bs {
            f.authority.grant(b, 5000, 10000);
            sim.admit(b.clone(), "spawn_test", 0, &f.authority).unwrap();
        }
        let frame = RoomFrame::capture(
            &sim,
            bs[0].clone(),
            bs.into_iter().zip(f.identities.clone()),
        )
        .unwrap();
        // Retain the old recipient authority so the scope mismatch is the first failure.
        f.authority.grant(&f.bindings[0], 5000, 10000);
        assert_eq!(
            decode(r.next(&frame, 0, &f.authority, &f.policy).unwrap())["reason"],
            if resource {
                "MAP_VERSION_MISMATCH"
            } else {
                "EPOCH_CHANGED"
            }
        );
    }
}
#[test]
fn public_frame_rejects_duplicate_roster_invalid_name_and_wrong_avatar_pin() {
    let f = Fixture::new();
    let b = f.bindings[0].clone();
    assert!(RoomFrame::capture(
        &f.sim,
        b.clone(),
        vec![(b.clone(), identity(10)), (b.clone(), identity(10))]
    )
    .is_err());
    for name in ["".into(), "x".repeat(41), "bad\nname".into()] {
        let mut roster = f.identities.clone();
        roster[0].display_name = name;
        assert!(RoomFrame::capture(
            &f.sim,
            b.clone(),
            f.bindings.clone().into_iter().zip(roster)
        )
        .is_err());
    }
    let mut roster = f.identities.clone();
    roster[0].avatar.manifest_version = Revision::new(2).unwrap();
    assert!(RoomFrame::capture(
        &f.sim,
        b.clone(),
        f.bindings.clone().into_iter().zip(roster)
    )
    .is_err());
}
#[test]
fn default_policy_is_unavailable_and_auth_expiry_cannot_be_renewed_by_activity() {
    let f = Fixture::new();
    let mut r = replica(&f.bindings[0], &f.authority, QueueLimits::default());
    assert_eq!(
        decode(
            r.next(&f.frame(), 0, &f.authority, &UnavailableRecipientPolicy)
                .unwrap()
        )["reason"],
        "POLICY_UNAVAILABLE"
    );
    let mut r = replica(&f.bindings[0], &f.authority, QueueLimits::default());
    f.authority.grant(&f.bindings[0], 5000, 20000);
    assert_eq!(
        decode(r.next(&f.frame(), 1, &f.authority, &f.policy).unwrap())["reason"],
        "POLICY_UNAVAILABLE"
    );
}
#[test]
fn capture_rejects_old_simulation_relabeled_as_current_room_epoch_or_connection() {
    let f = Fixture::new();
    for component in 0..5 {
        let mut roster = f.bindings.clone();
        for b in &mut roster {
            match component {
                0 => b.room_epoch = id(900),
                1 => b.room_id = id(901),
                2 => b.owner_id = id(902),
                3 => b.connection_id = id(903),
                _ => b.stream_id = id(904),
            }
        }
        assert!(RoomFrame::capture(
            &f.sim,
            roster[0].clone(),
            roster.into_iter().zip(f.identities.clone())
        )
        .is_err());
    }
}
