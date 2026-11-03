//! Explicit synthetic authority/geometry for socket regressions only, never IN01.
use chrono::{DateTime, Utc};
use concert_domain::{
    place::{Geometry, Interaction, PlaceId, Spawn},
    world::{Limits, WorldMap},
    AssetId, AvatarSelection, Revision,
};
use concert_service::world::{interest::*, membership::*, runtime::*, simulation::*};
use std::str::FromStr;
use uuid::Uuid;
pub fn id(v: u128) -> Uuid {
    Uuid::from_u128(v)
}
pub fn utc() -> DateTime<Utc> {
    "2026-11-03T16:00:00Z".parse().unwrap()
}
pub fn asset(s: &str) -> AssetId {
    AssetId::new(s.into()).unwrap()
}
pub fn pins() -> ResourcePin {
    ResourcePin {
        map_id: asset("map_fixture"),
        map_revision: Revision::initial(),
        avatar_manifest_id: asset("avatar_fixture"),
        avatar_manifest_version: Revision::initial(),
    }
}
pub fn identity() -> PublicIdentity {
    PublicIdentity {
        display_name: "Fixture user".into(),
        avatar_revision: Revision::initial(),
        avatar: AvatarSelection {
            manifest_id: asset("avatar_fixture"),
            manifest_version: Revision::initial(),
            body: asset("body_fixture"),
            eyes: asset("eyes_fixture"),
            hair: asset("hair_fixture"),
            outfit: asset("outfit_fixture"),
            accessory: None,
            skin_tone: asset("tone_fixture"),
        },
        facing: Facing::Down,
        moving: false,
    }
}
pub fn spec(room: Uuid, capacity: usize) -> RoomSpec {
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
