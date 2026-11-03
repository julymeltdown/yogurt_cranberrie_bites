use concert_domain::{
    place::{Geometry, Interaction, PlaceId, Spawn},
    world::{Input, Limits, Position, WorldMap, MAX_SEQUENCE},
    AssetId, AuthUserId, EventId, ParticipantKey, Revision,
};
use concert_service::world::simulation::*;
use std::{cell::RefCell, collections::BTreeMap, str::FromStr};
use uuid::Uuid;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}
fn geometry(tile: i32) -> Geometry {
    let mut grid = vec![vec![0; 12]; 8];
    grid[2][3] = 1;
    Geometry::new(
        12,
        8,
        tile,
        tile,
        "collision_test".into(),
        grid,
        vec![vec![1; 12]; 8],
        vec![vec![0; 12]; 8],
        vec![Spawn {
            id: "spawn_test".into(),
            x: 1,
            y: 2,
        }],
        vec![Interaction {
            id: "inspect_test".into(),
            place_id: PlaceId::from_str("00000000-0000-4000-8000-000000000090").unwrap(),
            x: 2,
            y: 2,
        }],
        "spawn_test".into(),
    )
    .unwrap()
}
fn pin() -> ResourcePin {
    ResourcePin {
        map_id: AssetId::new("map_test".into()).unwrap(),
        map_revision: Revision::initial(),
        avatar_manifest_id: AssetId::new("avatar_test".into()).unwrap(),
        avatar_manifest_version: Revision::initial(),
    }
}
fn binding(user: u128) -> Binding {
    Binding {
        key: ParticipantKey {
            event_id: EventId::from_uuid(id(1)),
            user_id: AuthUserId::from_uuid(id(user)),
        },
        session_id: id(2),
        connection_id: id(3 + user),
        owner_id: id(4),
        room_id: id(5),
        room_epoch: id(6),
        stream_id: id(7 + user),
        resources: pin(),
    }
}
#[derive(Default)]
struct Authority {
    leases: RefCell<BTreeMap<Uuid, MembershipLease>>,
    unavailable: bool,
}
impl Authority {
    fn grant(&self, b: &Binding, lease: u64, access: u64) {
        self.leases.borrow_mut().insert(
            b.key.user_id.uuid(),
            MembershipLease {
                binding: b.clone(),
                lease_until_ms: lease,
                access_expires_at_ms: access,
            },
        );
    }
}
impl MembershipPort for Authority {
    fn current(&self, expected: &Binding, _: u64) -> Result<MembershipLease, AuthorityError> {
        if self.unavailable {
            return Err(AuthorityError::Unavailable);
        }
        self.leases
            .borrow()
            .get(&expected.key.user_id.uuid())
            .cloned()
            .ok_or(AuthorityError::Revoked)
    }
}
fn fixture(tile: i32, speed: f64) -> (Simulation, Binding, Authority) {
    let b = binding(20);
    let a = Authority::default();
    a.grant(&b, 5000, 10000);
    let mut s = Simulation::new(
        b.key.event_id,
        b.room_id,
        b.room_epoch,
        b.owner_id,
        pin(),
        WorldMap::from_geometry(&geometry(tile)).unwrap(),
        Limits::new(speed, 48.0, 250).unwrap(),
        50,
        0,
    )
    .unwrap();
    s.admit(b.clone(), "spawn_test", 0, &a).unwrap();
    (s, b, a)
}
fn input(b: &Binding, seq: u64, x: f64, y: f64, interact: bool) -> Input {
    Input::new(b.stream_id, b.room_epoch, seq, x, y, interact).unwrap()
}
fn pos(s: &Simulation, b: &Binding) -> Position {
    s.state(b.key.user_id.uuid()).unwrap().position
}

#[test]
fn intent_wire_rejects_teleport_speed_duration_identity_and_unknown_buttons() {
    let b = binding(20);
    let valid = serde_json::json!({"protocolVersion":"1.0.0","type":"input","streamId":b.stream_id,"roomEpoch":b.room_epoch,"seq":1,"axisX":1,"axisY":0,"buttons":[]});
    assert!(serde_json::from_value::<Input>(valid.clone()).is_ok());
    for field in ["x", "y", "position", "speed", "deltaTime", "userId"] {
        let mut v = valid.clone();
        v[field] = serde_json::json!(9999);
        assert!(serde_json::from_value::<Input>(v).is_err(), "{field}");
    }
    for buttons in [
        serde_json::json!(["teleport"]),
        serde_json::json!(["interact", "interact"]),
    ] {
        let mut v = valid.clone();
        v["buttons"] = buttons;
        assert!(serde_json::from_value::<Input>(v).is_err());
    }
    let raw = serde_json::to_string(&valid)
        .unwrap()
        .replace("\"seq\":1", "\"seq\":1,\"seq\":2");
    assert!(serde_json::from_str::<Input>(&raw).is_err());
}
#[test]
fn invalid_axes_and_sequences_are_rejected_before_simulation() {
    for (x, y) in [
        (1.0, 1.0),
        (1.00001, 0.0),
        (f64::NAN, 0.0),
        (0.0, f64::INFINITY),
        (-1.1, 0.0),
    ] {
        assert!(Input::new(id(1), id(2), 1, x, y, false).is_err());
    }
    for seq in [0, MAX_SEQUENCE + 1, u64::MAX] {
        assert!(Input::new(id(1), id(2), seq, 0.0, 0.0, false).is_err());
    }
    let axis = std::f64::consts::FRAC_1_SQRT_2 * (1.0 + 0.5e-9);
    let i = Input::new(id(1), id(2), 1, axis, axis, false).unwrap();
    assert!(i.axes().0.hypot(i.axes().1) <= 1.0);
}
#[test]
fn fixed_step_and_diagonal_never_exceed_configured_speed() {
    let (mut s, b, a) = fixture(32, 120.0);
    let start = pos(&s, &b);
    let axis = std::f64::consts::FRAC_1_SQRT_2;
    s.submit(&b, input(&b, 1, axis, axis, false), 0, &a)
        .unwrap();
    assert_eq!(s.advance(49, &a).unwrap().steps, 0);
    assert_eq!(pos(&s, &b), start);
    assert_eq!(s.state(b.key.user_id.uuid()).unwrap().ack_input_seq, 0);
    s.advance(50, &a).unwrap();
    let p = pos(&s, &b);
    assert!(((p.x - start.x).hypot(p.y - start.y) - 6.0).abs() < 1e-10);
    assert_eq!(s.state(b.key.user_id.uuid()).unwrap().ack_input_seq, 1);
    assert_eq!(s.advance(50, &a).unwrap().steps, 0);
}
#[test]
fn packet_flood_cannot_accelerate_and_bucket_refills_at_thirty_per_second() {
    let (mut s, b, a) = fixture(32, 120.0);
    let start = pos(&s, &b);
    for seq in 1..=60 {
        s.submit(&b, input(&b, seq, 1.0, 0.0, false), 0, &a)
            .unwrap();
    }
    assert_eq!(s.pending_input_count(), 1);
    assert_eq!(
        s.submit(&b, input(&b, 61, 1.0, 0.0, false), 0, &a),
        Err(SimulationError::RateLimited)
    );
    s.advance(50, &a).unwrap();
    assert_eq!(pos(&s, &b).x, start.x + 6.0);
    assert_eq!(s.state(b.key.user_id.uuid()).unwrap().ack_input_seq, 60);
    s.submit(&b, input(&b, 61, 0.0, 0.0, false), 50, &a)
        .unwrap();
    assert_eq!(
        s.submit(&b, input(&b, 62, 0.0, 0.0, false), 50, &a),
        Err(SimulationError::RateLimited)
    );
    s.submit(&b, input(&b, 62, 0.0, 0.0, false), 67, &a)
        .unwrap();
}
#[test]
fn gaps_are_accepted_replays_rejected_ack_only_after_consumption() {
    let (mut s, b, a) = fixture(32, 120.0);
    s.submit(&b, input(&b, 9, 1.0, 0.0, false), 0, &a).unwrap();
    for seq in [9, 8, 1] {
        assert_eq!(
            s.submit(&b, input(&b, seq, 0.0, 0.0, false), 0, &a),
            Err(SimulationError::Replay)
        );
    }
    assert_eq!(s.state(b.key.user_id.uuid()).unwrap().ack_input_seq, 0);
    s.advance(50, &a).unwrap();
    assert_eq!(s.state(b.key.user_id.uuid()).unwrap().ack_input_seq, 9);
}
#[test]
fn stale_stream_epoch_connection_owner_and_resource_pin_never_mutate_position() {
    let (mut s, b, a) = fixture(32, 120.0);
    let start = pos(&s, &b);
    for kind in 0..6 {
        let mut bad = b.clone();
        match kind {
            0 => bad.stream_id = id(900),
            1 => bad.room_epoch = id(901),
            2 => bad.connection_id = id(902),
            3 => bad.owner_id = id(903),
            4 => bad.session_id = id(904),
            _ => bad.resources.map_revision = Revision::new(2).unwrap(),
        };
        assert_eq!(
            s.submit(&bad, input(&bad, 1, 1.0, 0.0, false), 0, &a),
            Err(SimulationError::StaleStream)
        );
    }
    assert_eq!(s.pending_input_count(), 0);
    s.advance(50, &a).unwrap();
    assert_eq!(pos(&s, &b), start);
}
#[test]
fn authoritative_takeover_fences_old_connection_and_resets_held_input() {
    let (mut s, b, a) = fixture(32, 120.0);
    s.submit(&b, input(&b, 1, 1.0, 0.0, true), 0, &a).unwrap();
    s.advance(50, &a).unwrap();
    let at = pos(&s, &b);
    let mut newer = b.clone();
    newer.stream_id = id(800);
    newer.connection_id = id(801);
    a.grant(&newer, 5000, 12000);
    s.replace_stream(newer.clone(), 50, &a).unwrap();
    assert!(!s.remove(&b));
    assert_eq!(
        s.submit(&b, input(&b, 2, 1.0, 0.0, false), 50, &a),
        Err(SimulationError::StaleStream)
    );
    assert_eq!(s.state(b.key.user_id.uuid()).unwrap().ack_input_seq, 0);
    assert_eq!(s.pending_input_count(), 0);
    assert!(s.advance(100, &a).unwrap().interactions.is_empty());
    assert_eq!(pos(&s, &b), at);
    s.submit(&newer, input(&newer, 1, 0.0, 1.0, false), 100, &a)
        .unwrap();
    s.advance(150, &a).unwrap();
    assert_eq!(pos(&s, &newer).y, at.y + 6.0);
}
#[test]
fn same_connection_resync_cannot_extend_captured_access_expiry_or_reset_rate_bucket() {
    let (mut s, b, a) = fixture(32, 120.0);
    for seq in 1..=60 {
        s.submit(&b, input(&b, seq, 0.0, 0.0, false), 0, &a)
            .unwrap();
    }
    let mut new = b.clone();
    new.stream_id = id(800);
    a.grant(&new, 5000, 11000);
    assert_eq!(
        s.replace_stream(new.clone(), 0, &a),
        Err(SimulationError::Authority)
    );
    a.grant(&new, 5000, 10000);
    s.replace_stream(new.clone(), 0, &a).unwrap();
    assert_eq!(
        s.submit(&new, input(&new, 1, 1.0, 0.0, false), 0, &a),
        Err(SimulationError::RateLimited)
    );
}
#[test]
fn expired_or_unavailable_authority_removes_player_even_without_due_tick() {
    for mode in 0..5 {
        let (mut s, b, mut a) = fixture(32, 120.0);
        s.submit(&b, input(&b, 1, 1.0, 0.0, false), 0, &a).unwrap();
        match mode {
            0 => a.grant(&b, 1, 10000),
            1 => a.grant(&b, 5000, 1),
            2 => a.unavailable = true,
            3 => {
                a.leases.borrow_mut().clear();
            }
            _ => {
                let mut other = b.clone();
                other.owner_id = id(90);
                a.grant(&other, 5000, 10000);
            }
        }
        let r = s.advance(1, &a).unwrap();
        assert_eq!(r.steps, 0);
        assert_eq!(r.revoked_users, vec![b.key.user_id.uuid()]);
        assert_eq!(s.player_count(), 0);
    }
}
#[test]
fn expired_input_denied_without_extending_authority() {
    let (mut s, b, a) = fixture(32, 120.0);
    a.grant(&b, 10, 10000);
    assert_eq!(
        s.submit(&b, input(&b, 1, 1.0, 0.0, false), 10, &a),
        Err(SimulationError::Authority)
    );
    assert_eq!(s.player_count(), 0);
}
#[test]
fn stop_input_and_hold_timeout_prevent_stuck_movement() {
    let (mut s, b, a) = fixture(32, 120.0);
    s.submit(&b, input(&b, 1, 1.0, 0.0, false), 0, &a).unwrap();
    s.advance(50, &a).unwrap();
    let p = pos(&s, &b);
    s.submit(&b, input(&b, 2, 0.0, 0.0, false), 50, &a).unwrap();
    s.advance(100, &a).unwrap();
    assert_eq!(pos(&s, &b), p);
    s.submit(&b, input(&b, 3, 0.0, 1.0, false), 100, &a)
        .unwrap();
    s.advance(400, &a).unwrap();
    assert_eq!(pos(&s, &b), p);
    assert_eq!(s.state(b.key.user_id.uuid()).unwrap().ack_input_seq, 2);
}
#[test]
fn lag_has_bounded_catchup_and_new_input_is_not_backdated() {
    let (mut s, b, a) = fixture(32, 120.0);
    let p = pos(&s, &b);
    s.submit(&b, input(&b, 1, 1.0, 0.0, false), 1001, &a)
        .unwrap();
    let r = s.advance(1001, &a).unwrap();
    assert_eq!(r.steps, 4);
    assert_eq!(r.dropped_steps, 16);
    assert_eq!(r.server_tick, 20);
    assert_eq!(pos(&s, &b), p);
    s.advance(1050, &a).unwrap();
    assert_eq!(pos(&s, &b).x, p.x + 6.0);
    assert_eq!(s.advance(1049, &a).unwrap_err(), SimulationError::Clock);
}
#[test]
fn swept_collision_blocks_wall_tunneling_even_when_destination_is_walkable() {
    let (mut s, b, a) = fixture(8, 1000.0);
    let p = pos(&s, &b);
    s.submit(&b, input(&b, 1, 1.0, 0.0, false), 0, &a).unwrap();
    s.advance(50, &a).unwrap();
    assert_eq!(pos(&s, &b), p);
    let map = WorldMap::from_geometry(&geometry(8)).unwrap();
    assert!(map.walkable(Position { x: 62.0, y: 20.0 }));
    assert!(!map.clear_segment(p, Position { x: 62.0, y: 20.0 }));
}
#[test]
fn corner_cutting_grid_edge_skimming_and_out_of_bounds_are_blocked() {
    let mut g = geometry(8);
    g.collision[1][2] = 1;
    let map = WorldMap::from_geometry(&g).unwrap();
    assert!(!map.clear_segment(Position { x: 12.0, y: 12.0 }, Position { x: 20.0, y: 20.0 }));
    assert!(!map.clear_segment(Position { x: 16.0, y: 16.0 }, Position { x: 24.0, y: 16.0 }));
    assert!(!map.clear_segment(Position { x: 12.0, y: 20.0 }, Position { x: -1.0, y: 20.0 }));
    assert!(!map.walkable(Position {
        x: f64::NAN,
        y: 0.0
    }));
}
#[test]
fn spawn_is_cell_centered_and_mutated_or_malformed_geometry_is_rejected() {
    let map = WorldMap::from_geometry(&geometry(32)).unwrap();
    assert_eq!(
        map.spawn("spawn_test").unwrap(),
        Position { x: 48.0, y: 80.0 }
    );
    assert!(map.spawn("unknown").is_err());
    for mode in 0..6 {
        let mut g = geometry(32);
        match mode {
            0 => g.spawns[0].x = 3,
            1 => g.spawns[0].x = -1,
            2 => g.collision[0].clear(),
            3 => g.tile_width = 0,
            4 => g.default_spawn_id = "missing".into(),
            _ => g.interactions[0].x = 99,
        };
        assert!(WorldMap::from_geometry(&g).is_err());
    }
}
#[test]
fn range_requires_known_interaction_distance_and_clear_virtual_path() {
    let map = WorldMap::from_geometry(&geometry(32)).unwrap();
    let limits = Limits::new(120.0, 48.0, 250).unwrap();
    assert!(map.interaction_in_range(Position { x: 48.0, y: 80.0 }, "inspect_test", limits));
    assert!(!map.interaction_in_range(
        Position { x: 144.0, y: 80.0 },
        "inspect_test",
        Limits::new(120.0, 200.0, 250).unwrap()
    ));
    assert!(!map.interaction_in_range(Position { x: 16.0, y: 80.0 }, "inspect_test", limits));
    assert!(!map.interaction_in_range(Position { x: 48.0, y: 80.0 }, "missing", limits));
}
#[test]
fn one_shot_interaction_is_consumed_once_and_not_replayed_by_held_motion() {
    let (mut s, b, a) = fixture(32, 120.0);
    s.submit(&b, input(&b, 1, 0.0, 0.0, true), 0, &a).unwrap();
    let r = s.advance(200, &a).unwrap();
    assert_eq!(r.steps, 4);
    assert_eq!(r.interactions.len(), 1);
    assert_eq!(r.interactions[0].interaction_id, "inspect_test");
    assert!(s.advance(250, &a).unwrap().interactions.is_empty());
}
#[test]
fn explicit_limits_bound_configuration_and_workload_at_fifty_players() {
    for speed in [0.0, -1.0, 1000.01, f64::NAN] {
        assert!(Limits::new(speed, 48.0, 250).is_err());
    }
    assert!(Limits::new(120.0, 257.0, 250).is_err());
    assert!(Limits::new(120.0, 48.0, 1001).is_err());
    let (mut s, b, a) = fixture(32, 120.0);
    for n in 21..70 {
        let member = binding(n);
        a.grant(&member, 5000, 10000);
        s.admit(member.clone(), "spawn_test", 0, &a).unwrap();
        s.submit(&member, input(&member, 1, 0.0, 0.0, true), 0, &a)
            .unwrap();
    }
    let extra = binding(70);
    a.grant(&extra, 5000, 10000);
    assert_eq!(
        s.admit(extra, "spawn_test", 0, &a),
        Err(SimulationError::Capacity)
    );
    s.submit(&b, input(&b, 1, 0.0, 0.0, true), 0, &a).unwrap();
    assert_eq!(s.pending_input_count(), 50);
    let r = s.advance(200, &a).unwrap();
    assert_eq!(r.steps, 4);
    assert_eq!(r.interactions.len(), 50);
    assert_eq!(s.pending_input_count(), 0);
    assert_eq!(s.player_count(), 50);
}
#[test]
fn frozen_c06_synthetic_geometry_has_expected_spawn_and_collision() {
    let bytes = include_bytes!("../../../../contracts/fixtures/assets/map-manifest.json");
    let canonical = concert_domain::place::canonical_json_bytes(bytes).unwrap();
    let manifest = concert_domain::place::parse_stored_release(&canonical).unwrap();
    let map = WorldMap::from_geometry(&manifest.geometry).unwrap();
    assert_eq!(
        map.spawn("spawn_fixture").unwrap(),
        Position { x: 48.0, y: 48.0 }
    );
    assert!(!map.clear_segment(
        Position { x: 48.0, y: 80.0 },
        Position { x: 112.0, y: 80.0 }
    ));
}
