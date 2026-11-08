//! WP-07 T-601: the room layout value and its structural validation.
//!
//! Every acceptance case of the task is one of the tests below, written so that
//! it fails if the corresponding rule is removed:
//!
//! * `five_posters_are_rejected` — the contract's "max 4 posters";
//! * `two_furniture_items_on_one_cell_in_one_layer_are_rejected` — the contract's
//!   "unique cell per layer";
//! * `an_unknown_wall_style_is_rejected` — the contract's "known style ids".
//!
//! The remaining tests cover the rest of the bounds (grid, layers, poster span,
//! furniture count, closed item set, strict decoding) and the structural
//! independence from intro posts and events.
use concert_domain::profile_room::*;
use serde_json::{json, Value};
use uuid::Uuid;

fn media() -> String {
    Uuid::new_v4().to_string()
}

fn layout() -> Value {
    json!({
        "version": 1,
        "wall": "plaster",
        "floor": "wood",
        "furniture": [],
        "posters": [],
        "character": {"cell_x": 16, "cell_y": 24, "facing": "RIGHT"},
    })
}

fn poster(slot: u8) -> Value {
    json!({"poster_media_id": media(), "slot": slot, "x": 4, "y": 2, "width": 6, "height": 6})
}

fn furniture(item: &str, x: u16, y: u16, layer: u8) -> Value {
    json!({"item_id": item, "cell_x": x, "cell_y": y, "layer": layer})
}

fn parse(value: &Value) -> Result<RoomLayout, RoomError> {
    RoomLayout::parse(&serde_json::to_vec(value).unwrap())
}

fn body(expected: i64, layout: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"expected_revision": expected, "layout": layout})).unwrap()
}

// ---------------------------------------------------------------------------
// The three named acceptance cases
// ---------------------------------------------------------------------------

#[test]
fn five_posters_are_rejected() {
    let mut value = layout();
    value["posters"] = Value::Array((0..4).map(poster).collect());
    let four = parse(&value).expect("four posters are admitted");
    assert_eq!(four.posters.len(), 4);
    let mut value = layout();
    value["posters"] = Value::Array((0..5).map(poster).collect());
    assert_eq!(parse(&value), Err(RoomError::Invalid));
    // The same rule holds through the save command, not only through the value.
    assert!(SaveRoom::parse(&body(1, value)).is_err());
}

#[test]
fn two_furniture_items_on_one_cell_in_one_layer_are_rejected() {
    let mut value = layout();
    value["furniture"] = json!([furniture("chair", 4, 4, 0), furniture("table", 4, 4, 0)]);
    assert_eq!(parse(&value), Err(RoomError::Invalid));
    // The same cell in a different layer is a different slot: a lamp on a table.
    let mut value = layout();
    value["furniture"] = json!([furniture("table", 4, 4, 0), furniture("lamp", 4, 4, 1)]);
    let stacked = parse(&value).expect("different layers may share a cell");
    assert_eq!(stacked.furniture.len(), 2);
    // A different cell in the same layer is fine.
    let mut value = layout();
    value["furniture"] = json!([furniture("table", 4, 4, 0), furniture("chair", 5, 4, 0)]);
    assert!(parse(&value).is_ok());
    // The collision is detected across a long list, not only between neighbours.
    let mut value = layout();
    let mut items: Vec<Value> = (0..31u16)
        .map(|index| furniture("chair", index % 32, index / 32 * 2, 0))
        .collect();
    items.push(furniture("table", 0, 0, 0));
    value["furniture"] = Value::Array(items);
    assert_eq!(parse(&value), Err(RoomError::Invalid));
}

#[test]
fn an_unknown_wall_style_is_rejected() {
    let mut value = layout();
    value["wall"] = json!("marble");
    assert_eq!(parse(&value), Err(RoomError::Invalid));
    // Every admitted wall style is accepted, and every other string is not.
    for style in WALL_STYLES {
        let mut value = layout();
        value["wall"] = json!(style);
        assert_eq!(parse(&value).unwrap().wall.as_str(), style);
    }
    for style in ["", "Plaster", "PLASTER", "plaster ", "plaster\n", "marble"] {
        let mut value = layout();
        value["wall"] = json!(style);
        assert_eq!(parse(&value), Err(RoomError::Invalid), "{style:?}");
    }
}

// ---------------------------------------------------------------------------
// The rest of the bounds
// ---------------------------------------------------------------------------

#[test]
fn an_unknown_floor_style_or_furniture_item_is_rejected() {
    let mut value = layout();
    value["floor"] = json!("carpet");
    assert_eq!(parse(&value), Err(RoomError::Invalid));
    for style in FLOOR_STYLES {
        let mut value = layout();
        value["floor"] = json!(style);
        assert_eq!(parse(&value).unwrap().floor.as_str(), style);
    }
    let mut value = layout();
    value["furniture"] = json!([furniture("piano", 1, 1, 0)]);
    assert_eq!(parse(&value), Err(RoomError::Invalid));
    for item in FURNITURE_ITEMS {
        let mut value = layout();
        value["furniture"] = json!([furniture(item, 1, 1, 0)]);
        assert_eq!(parse(&value).unwrap().furniture[0].item.as_str(), item);
    }
}

#[test]
fn a_coordinate_outside_the_grid_is_rejected() {
    for (key, value) in [
        ("cell_x", json!(32)),
        ("cell_y", json!(32)),
        ("cell_x", json!(65535)),
    ] {
        let mut layout_value = layout();
        layout_value["character"][key] = value.clone();
        assert_eq!(parse(&layout_value), Err(RoomError::Invalid), "character {key}={value}");
        let mut layout_value = layout();
        layout_value["furniture"] = json!([furniture("chair", 31, 31, 0)]);
        layout_value["furniture"][0][key] = value.clone();
        assert_eq!(parse(&layout_value), Err(RoomError::Invalid), "furniture {key}={value}");
        let mut layout_value = layout();
        let mut placement = poster(0);
        placement[key] = value.clone();
        layout_value["posters"] = json!([placement]);
        assert_eq!(parse(&layout_value), Err(RoomError::Invalid), "poster {key}={value}");
    }
    // The last legal cell is admitted.
    let mut value = layout();
    value["character"] = json!({"cell_x": 31, "cell_y": 31, "facing": "LEFT"});
    assert!(parse(&value).is_ok());
}

#[test]
fn a_furniture_layer_outside_the_closed_set_is_rejected() {
    for layer in [4u8, 5, 255] {
        let mut value = layout();
        value["furniture"] = json!([furniture("chair", 1, 1, layer)]);
        assert_eq!(parse(&value), Err(RoomError::Invalid), "layer {layer}");
    }
    for layer in 0..=MAX_LAYER {
        let mut value = layout();
        value["furniture"] = json!([furniture("chair", 1, 1, layer)]);
        assert!(parse(&value).is_ok(), "layer {layer}");
    }
}

#[test]
fn more_than_thirty_two_furniture_items_are_rejected() {
    let items: Vec<Value> = (0..MAX_FURNITURE as u16)
        .map(|index| furniture("chair", index % 32, index / 32, 0))
        .collect();
    let mut value = layout();
    value["furniture"] = Value::Array(items.clone());
    assert_eq!(parse(&value).unwrap().furniture.len(), MAX_FURNITURE);
    let mut too_many = items;
    too_many.push(furniture("table", 31, 31, 1));
    let mut value = layout();
    value["furniture"] = Value::Array(too_many);
    assert_eq!(parse(&value), Err(RoomError::Invalid));
}

#[test]
fn a_poster_must_stay_inside_the_wall_and_own_its_slot() {
    // A poster hanging past the right or bottom edge is refused.
    for (key, value) in [("x", 28u16), ("y", 30u16)] {
        let mut placement = poster(0);
        placement[key] = json!(value);
        let mut layout_value = layout();
        layout_value["posters"] = json!([placement]);
        assert_eq!(parse(&layout_value), Err(RoomError::Invalid), "{key}={value}");
    }
    // A zero-sized poster is not a poster.
    for key in ["width", "height"] {
        let mut placement = poster(0);
        placement[key] = json!(0);
        let mut layout_value = layout();
        layout_value["posters"] = json!([placement]);
        assert_eq!(parse(&layout_value), Err(RoomError::Invalid), "{key}=0");
    }
    // A slot outside 0..=3 is refused.
    let mut value = layout();
    value["posters"] = json!([poster(4)]);
    assert_eq!(parse(&value), Err(RoomError::Invalid));
    // Two posters cannot share a slot.
    let mut value = layout();
    value["posters"] = json!([poster(1), poster(1)]);
    assert_eq!(parse(&value), Err(RoomError::Invalid));
    // The same image cannot hang twice.
    let mut value = layout();
    let first = poster(0);
    let mut second = poster(1);
    second["poster_media_id"] = first["poster_media_id"].clone();
    value["posters"] = json!([first, second]);
    assert_eq!(parse(&value), Err(RoomError::Invalid));
    // A nil media id is not a poster.
    let mut value = layout();
    value["posters"] = json!([{"poster_media_id": Uuid::nil().to_string(), "slot": 0,
        "x": 0, "y": 0, "width": 1, "height": 1}]);
    assert_eq!(parse(&value), Err(RoomError::Invalid));
    // A poster may span the whole wall.
    let mut value = layout();
    value["posters"] = json!([{"poster_media_id": media(), "slot": 0,
        "x": 0, "y": 0, "width": 32, "height": 32}]);
    assert!(parse(&value).is_ok());
}

// ---------------------------------------------------------------------------
// Strict decoding
// ---------------------------------------------------------------------------

#[test]
fn unknown_fields_are_rejected_at_every_level() {
    let mut value = layout();
    value["extra"] = json!(1);
    assert_eq!(parse(&value), Err(RoomError::Invalid));
    let mut value = layout();
    value["character"]["extra"] = json!(1);
    assert_eq!(parse(&value), Err(RoomError::Invalid));
    let mut value = layout();
    let mut item = furniture("chair", 1, 1, 0);
    item["extra"] = json!(1);
    value["furniture"] = json!([item]);
    assert_eq!(parse(&value), Err(RoomError::Invalid));
    let mut value = layout();
    let mut placement = poster(0);
    placement["extra"] = json!(1);
    value["posters"] = json!([placement]);
    assert_eq!(parse(&value), Err(RoomError::Invalid));
    // A missing required member is refused too.
    for key in ["version", "wall", "floor", "furniture", "posters", "character"] {
        let mut value = layout();
        value.as_object_mut().unwrap().remove(key);
        assert_eq!(parse(&value), Err(RoomError::Invalid), "missing {key}");
    }
}

#[test]
fn a_non_integer_or_non_object_layout_is_rejected() {
    for value in [
        json!([]),
        json!("layout"),
        json!(1),
        json!(null),
        json!({"version": 1.5, "wall": "plaster", "floor": "wood", "furniture": [],
               "posters": [], "character": {"cell_x": 0, "cell_y": 0, "facing": "LEFT"}}),
        json!({"version": 1, "wall": "plaster", "floor": "wood", "furniture": {},
               "posters": [], "character": {"cell_x": 0, "cell_y": 0, "facing": "LEFT"}}),
        json!({"version": 1, "wall": "plaster", "floor": "wood", "furniture": [],
               "posters": [], "character": {"cell_x": 0, "cell_y": 0, "facing": "UP"}}),
    ] {
        assert_eq!(parse(&value), Err(RoomError::Invalid), "{value}");
    }
    // Duplicate keys are refused before serde can pick one of them.
    let duplicate = br#"{"version":1,"wall":"plaster","wall":"brick","floor":"wood",
        "furniture":[],"posters":[],"character":{"cell_x":0,"cell_y":0,"facing":"LEFT"}}"#;
    assert_eq!(RoomLayout::parse(duplicate), Err(RoomError::Invalid));
    // An empty body is not a layout.
    assert_eq!(RoomLayout::parse(b""), Err(RoomError::Invalid));
}

#[test]
fn a_layout_from_a_newer_version_is_unsupported_rather_than_reinterpreted() {
    let mut value = layout();
    value["version"] = json!(2);
    assert_eq!(parse(&value), Err(RoomError::UnsupportedVersion));
    let mut value = layout();
    value["version"] = json!(0);
    assert_eq!(parse(&value), Err(RoomError::UnsupportedVersion));
}

// ---------------------------------------------------------------------------
// Round trip
// ---------------------------------------------------------------------------

#[test]
fn the_wire_projection_round_trips_exactly() {
    let mut value = layout();
    value["wall"] = json!("brick");
    value["floor"] = json!("tile");
    value["furniture"] = json!([
        furniture("sofa", 3, 18, 0),
        furniture("lamp", 3, 18, 1),
        furniture("desk", 20, 12, 2),
    ]);
    value["posters"] = json!([poster(0), poster(3)]);
    value["character"] = json!({"cell_x": 9, "cell_y": 27, "facing": "LEFT"});
    let parsed = parse(&value).expect("valid layout");
    assert_eq!(parsed.to_wire(), value);
    assert_eq!(RoomLayout::parse_value(&parsed.to_wire()).unwrap(), parsed);
    // The initial layout is itself a valid, drawable room.
    let initial = RoomLayout::initial();
    initial.validate().unwrap();
    assert_eq!(RoomLayout::parse_value(&initial.to_wire()).unwrap(), initial);
    assert_eq!(initial.posters.len(), 0);
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[test]
fn the_save_command_is_closed_and_carries_no_owner() {
    let value = layout();
    let command = SaveRoom::parse(&body(7, value.clone())).unwrap();
    assert_eq!(command.expected.get(), 7);
    assert_eq!(command.layout.to_wire(), value);
    // An owner, an intro post or an event in the body is refused: the owner is the
    // session and the room is never tied to an intro post.
    for (key, extra) in [
        ("owner_profile_id", json!(media())),
        ("profile_id", json!(media())),
        ("intro_id", json!(media())),
        ("event_id", json!(media())),
        ("visibility", json!("PUBLIC")),
    ] {
        let mut body_value = json!({"expected_revision": 1, "layout": value.clone()});
        body_value[key] = extra;
        assert!(
            SaveRoom::parse(&serde_json::to_vec(&body_value).unwrap()).is_err(),
            "{key} must be refused"
        );
    }
    // A revision outside the positive safe-integer range is refused.
    for revision in [0i64, -1, i64::MAX] {
        assert!(SaveRoom::parse(&body(revision, value.clone())).is_err(), "{revision}");
    }
    // An invalid layout is refused before it can reach a repository.
    let mut invalid = value;
    invalid["wall"] = json!("marble");
    assert!(SaveRoom::parse(&body(1, invalid)).is_err());
}

#[test]
fn the_visibility_command_carries_visibility_and_revision_and_nothing_else() {
    let parse_visibility = |value: Value| {
        SetRoomVisibility::parse(&serde_json::to_vec(&value).unwrap())
    };
    let command = parse_visibility(json!({"expected_revision": 3, "visibility": "PRIVATE"})).unwrap();
    assert_eq!(command.visibility, RoomVisibility::Private);
    assert_eq!(command.expected.get(), 3);
    assert_eq!(
        parse_visibility(json!({"expected_revision": 1, "visibility": "PUBLIC"}))
            .unwrap()
            .visibility,
        RoomVisibility::Public
    );
    for value in [
        json!({"expected_revision": 1, "visibility": "FRIENDS"}),
        json!({"expected_revision": 1, "visibility": "public"}),
        json!({"expected_revision": 1}),
        json!({"visibility": "PUBLIC"}),
        json!({"expected_revision": 0, "visibility": "PUBLIC"}),
        // No audience field exists on this command, so one cannot be smuggled in.
        json!({"expected_revision": 1, "visibility": "PUBLIC", "audience_gender": "any"}),
        json!({"expected_revision": 1, "visibility": "PUBLIC", "audience_min_age": 18}),
        json!({"expected_revision": 1, "visibility": "PUBLIC", "intro_id": media()}),
    ] {
        assert!(parse_visibility(value.clone()).is_err(), "{value}");
    }
}

#[test]
fn the_poster_command_is_png_only_and_bounded() {
    let png = b"\x89PNG\r\n\x1a\npayload";
    let encoded = base64_encode(png);
    let upload = |preset: &str, width: u32, height: u32, content_type: &str, encoded: &str| {
        UploadPoster::parse(
            &serde_json::to_vec(&json!({"preset": preset, "width": width, "height": height,
                "content_type": content_type, "bytes_base64": encoded}))
            .unwrap(),
        )
    };
    let fine = upload("FINE", 64, 64, "image/png", &encoded).unwrap();
    assert_eq!(fine.bytes, png);
    assert_eq!(fine.preset, PosterPreset::Fine);
    assert!(fine.matches_preset());
    assert!(upload("CHUNKY", 32, 32, "image/png", &encoded).unwrap().matches_preset());
    // A declared size that is not the preset's native size is refused.
    assert!(!upload("FINE", 32, 32, "image/png", &encoded).unwrap().matches_preset());
    assert!(!upload("CHUNKY", 64, 64, "image/png", &encoded).unwrap().matches_preset());
    // The content type is exactly image/png.
    for content_type in ["image/jpeg", "image/webp", "image/gif", "IMAGE/PNG", ""] {
        assert!(upload("FINE", 64, 64, content_type, &encoded).is_err(), "{content_type}");
    }
    // The preset set is closed.
    for preset in ["fine", "FINE ", "LOW", ""] {
        assert!(upload(preset, 64, 64, "image/png", &encoded).is_err(), "{preset}");
    }
    // The side is 8..=128.
    for side in [0u32, 7, 129, 4096] {
        assert!(upload("FINE", side, side, "image/png", &encoded).is_err(), "{side}");
    }
    // A body larger than one MiB of bytes is refused after decoding.
    let large = base64_encode(&vec![0u8; MAX_POSTER_BYTES + 1]);
    assert!(upload("FINE", 64, 64, "image/png", &large).is_err());
    // Non-canonical base64 is refused.
    assert!(upload("FINE", 64, 64, "image/png", "!!").is_err());
    assert!(upload("FINE", 64, 64, "image/png", "").is_err());
}

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        out.push(ALPHABET[(b[0] >> 2) as usize] as char);
        out.push(ALPHABET[(((b[0] & 0b11) << 4) | (b[1] >> 4)) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(((b[1] & 0b1111) << 2) | (b[2] >> 6)) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(b[2] & 0b111111) as usize] as char
        } else {
            '='
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Structural independence
// ---------------------------------------------------------------------------

#[test]
fn the_room_layout_has_no_intro_post_or_event_reference() {
    let wire = RoomLayout::initial().to_wire();
    let text = serde_json::to_string(&wire).unwrap();
    for forbidden in ["intro", "event", "audience", "participant"] {
        assert!(!text.contains(forbidden), "{forbidden} must not appear in a layout");
    }
    // And the module itself cannot name one: the source has no type that could
    // carry an intro post or an event.
    let source = include_str!("../src/profile_room.rs");
    let code = source
        .split("#[cfg(test)]")
        .next()
        .expect("module source");
    let code = code
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in ["EventId", "ParticipantKey", "IntroAudience", "intro_id", "event_id"] {
        assert!(
            !code.contains(forbidden),
            "the room module must not mention {forbidden}"
        );
    }
    // A character placement carries no presence or activity field.
    let character = &RoomLayout::initial().to_wire()["character"];
    let keys: Vec<&String> = character.as_object().unwrap().keys().collect();
    assert_eq!(keys, vec!["cell_x", "cell_y", "facing"]);
}

#[test]
fn the_room_visibility_default_is_public() {
    // http-contract.md §2.4 declares public_read; product-plan.md §6 says
    // authorized visitors see the saved room.
    assert_eq!(RoomVisibility::default_for_new_room(), RoomVisibility::Public);
    assert_eq!(RoomVisibility::Public.as_str(), "PUBLIC");
    assert_eq!(RoomVisibility::Private.as_str(), "PRIVATE");
}
