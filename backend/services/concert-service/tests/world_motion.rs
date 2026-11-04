//! Language-neutral golden vectors exercise the actual BE12 kernel, not a second simulator.
use concert_domain::{
    place::{Geometry, Interaction, Spawn},
    world::{Input, Limits, Position, WorldMap},
    AssetId, AuthUserId, EventId, ParticipantKey, Revision,
};
use concert_service::world::{
    ingress::{decode, ClientMessage},
    simulation::*,
};
use serde_json::{json, Value};
use std::cell::RefCell;
use uuid::Uuid;

const MAP: &str = include_str!("../../../../contracts/fixtures/assets/map-manifest.json");
const VECTORS: &str =
    include_str!("../../../../contracts/fixtures/realtime/motion-v1/vectors.json");
fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}
fn geometry() -> Geometry {
    let map: Value = serde_json::from_str(MAP).unwrap();
    let g = &map["geometry"];
    Geometry::new(
        g["width_cells"].as_i64().unwrap() as i32,
        g["height_cells"].as_i64().unwrap() as i32,
        g["tile_width_px"].as_i64().unwrap() as i32,
        g["tile_height_px"].as_i64().unwrap() as i32,
        g["collision_id"].as_str().unwrap().to_owned(),
        serde_json::from_value(g["collision_grid"].clone()).unwrap(),
        serde_json::from_value(g["terrain_layer"].clone()).unwrap(),
        serde_json::from_value(g["overdraw_layer"].clone()).unwrap(),
        g["spawns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| Spawn {
                id: s["spawn_id"].as_str().unwrap().into(),
                x: s["cell_x"].as_i64().unwrap() as i32,
                y: s["cell_y"].as_i64().unwrap() as i32,
            })
            .collect(),
        g["interactions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| Interaction {
                id: s["interaction_id"].as_str().unwrap().into(),
                place_id: s["place_id"].as_str().unwrap().parse().unwrap(),
                x: s["cell_x"].as_i64().unwrap() as i32,
                y: s["cell_y"].as_i64().unwrap() as i32,
            })
            .collect(),
        g["default_spawn_id"].as_str().unwrap().into(),
    )
    .unwrap()
}
struct Authority(RefCell<Option<MembershipLease>>);
impl MembershipPort for Authority {
    fn current(&self, _: &Binding, _: u64) -> Result<MembershipLease, AuthorityError> {
        self.0.borrow().clone().ok_or(AuthorityError::Unavailable)
    }
}
fn fixture() -> (Simulation, Binding, Authority) {
    fixture_epoch(id(7))
}
fn fixture_epoch(epoch: Uuid) -> (Simulation, Binding, Authority) {
    let resources = ResourcePin {
        map_id: AssetId::new("map_fixture".into()).unwrap(),
        map_revision: Revision::initial(),
        avatar_manifest_id: AssetId::new("avatar_fixture".into()).unwrap(),
        avatar_manifest_version: Revision::initial(),
    };
    let b = Binding {
        key: ParticipantKey {
            event_id: EventId::from_uuid(id(1)),
            user_id: AuthUserId::from_uuid(id(2)),
        },
        session_id: id(3),
        connection_id: id(4),
        owner_id: id(5),
        room_id: id(6),
        room_epoch: epoch,
        stream_id: id(8),
        resources: resources.clone(),
    };
    let a = Authority(RefCell::new(Some(MembershipLease {
        binding: b.clone(),
        lease_until_ms: 5000,
        access_expires_at_ms: 10000,
    })));
    let mut s = Simulation::new(
        b.key.event_id,
        b.room_id,
        b.room_epoch,
        b.owner_id,
        resources,
        WorldMap::from_geometry(&geometry()).unwrap(),
        Limits::new(160.0, 48.0, 250).unwrap(),
        50,
        0,
    )
    .unwrap();
    let map: Value = serde_json::from_str(MAP).unwrap();
    s.configure_motion_profile(map["geometry_hash"].as_str().unwrap())
        .unwrap();
    s.admit(b.clone(), "spawn_fixture", 0, &a).unwrap();
    (s, b, a)
}
fn assert_close(actual: &Value, expected: &Value, tolerance: f64) {
    match (actual, expected) {
        (Value::Object(a), Value::Object(e)) => {
            for (k, v) in e {
                assert_close(&a[k], v, tolerance);
            }
        }
        (Value::Number(a), Value::Number(e)) => assert!(
            (a.as_f64().unwrap() - e.as_f64().unwrap()).abs() <= tolerance,
            "actual {actual}, expected {expected}"
        ),
        _ => assert_eq!(actual, expected),
    }
}
#[test]
fn shared_vectors_apply_real_kernel_and_report_actual_duration_and_discard() {
    let vectors: Value = serde_json::from_str(VECTORS).unwrap();
    for case in vectors["cases"].as_array().unwrap() {
        let (mut s, mut b, mut a) = fixture();
        for action in case["actions"].as_array().unwrap() {
            if let Some(i) = action.get("input") {
                s.submit(
                    &b,
                    Input::new(
                        b.stream_id,
                        b.room_epoch,
                        i["seq"].as_u64().unwrap(),
                        i["axisX"].as_f64().unwrap(),
                        i["axisY"].as_f64().unwrap(),
                        false,
                    )
                    .unwrap(),
                    i["atMs"].as_u64().unwrap(),
                    &a,
                )
                .unwrap();
            } else {
                let at = action
                    .get("advanceMs")
                    .unwrap_or(&action["atMs"])
                    .as_u64()
                    .unwrap();
                match action["reset"].as_str() {
                    Some("STREAM") => {
                        b.stream_id = id(99);
                        a.0.borrow_mut().as_mut().unwrap().binding = b.clone();
                        s.replace_stream(b.clone(), at, &a).unwrap();
                    }
                    Some("EPOCH") => {
                        (s, b, a) = fixture_epoch(id(100));
                    }
                    _ => {
                        s.advance(at, &a).unwrap();
                    }
                }
                let (profile, anchor) = s.motion_state(&b, at).unwrap();
                let mut actual = serde_json::to_value(anchor).unwrap();
                actual["ackInputSeq"] = json!(s.state(b.key.user_id.uuid()).unwrap().ack_input_seq);
                assert_close(
                    &actual,
                    &action["expected"],
                    vectors["tolerance"].as_f64().unwrap(),
                );
                assert_eq!(
                    serde_json::to_value(profile).unwrap()["geometryHash"],
                    vectors["geometryHash"]
                );
            }
        }
    }
}
#[test]
fn shared_supercover_segments_include_corner_edge_and_bounds() {
    let vectors: Value = serde_json::from_str(VECTORS).unwrap();
    let map = WorldMap::from_geometry(&geometry()).unwrap();
    for v in vectors["segments"].as_array().unwrap() {
        let p = |name: &str| Position {
            x: v[name]["x"].as_f64().unwrap(),
            y: v[name]["y"].as_f64().unwrap(),
        };
        assert_eq!(
            map.clear_segment(p("from"), p("to")),
            v["clear"].as_bool().unwrap(),
            "{}",
            v["name"]
        );
    }
}
#[test]
fn stream_reset_discards_pending_held_and_duration_but_preserves_room_drops() {
    let (mut s, b, a) = fixture();
    s.advance(1000, &a).unwrap();
    let mut next = b.clone();
    next.stream_id = id(9);
    a.0.borrow_mut().as_mut().unwrap().binding = next.clone();
    s.replace_stream(next.clone(), 1000, &a).unwrap();
    assert!(s.motion_state(&b, 1000).is_none());
    let (_, anchor) = s.motion_state(&next, 1000).unwrap();
    assert_eq!(
        (
            anchor.consumed_input_seq,
            anchor.input_applied_ticks,
            anchor.dropped_ticks
        ),
        (0, 0, 16)
    );
    assert!(anchor.held_input.is_none());
    assert_eq!(
        s.submit(
            &next,
            Input::new(b.stream_id, b.room_epoch, 1, 1.0, 0.0, true).unwrap(),
            1000,
            &a
        ),
        Err(SimulationError::StaleStream)
    );
}
#[test]
fn denied_authority_removes_motion_and_no_ghost_anchor_survives() {
    let (mut s, b, a) = fixture();
    *a.0.borrow_mut() = None;
    s.advance(1, &a).unwrap();
    assert_eq!(s.player_count(), 0);
    assert!(s.motion_state(&b, 1).is_none());
}
#[test]
fn strict_auth_negotiation_is_additive_and_rejects_unknown_or_null() {
    let baseline: Value = serde_json::from_str(include_str!(
        "../../../../contracts/fixtures/realtime/motion-v1/negotiated.json"
    ))
    .unwrap();
    let mut auth = baseline["messages"][0]["message"].clone();
    assert!(matches!(
        decode(&serde_json::to_vec(&auth).unwrap()).unwrap(),
        ClientMessage::AuthenticateMotion(_)
    ));
    auth.as_object_mut().unwrap().remove("motionProfileVersion");
    assert!(matches!(
        decode(&serde_json::to_vec(&auth).unwrap()).unwrap(),
        ClientMessage::Authenticate(_)
    ));
    for v in [json!(null), json!(true), json!("2.0.0")] {
        auth["motionProfileVersion"] = v;
        assert!(decode(&serde_json::to_vec(&auth).unwrap()).is_err());
    }
    auth["motionProfileVersion"] = json!("1.0.0");
    auth["clientPosition"] = json!({"x":0,"y":0});
    assert!(decode(&serde_json::to_vec(&auth).unwrap()).is_err());
}
