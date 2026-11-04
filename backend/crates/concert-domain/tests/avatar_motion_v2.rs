use concert_domain::avatar_motion_v2::*;
use serde_json::{json, Value};
use std::{fs, path::PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}
fn fixture_dir(kind: &str) -> PathBuf {
    root()
        .join("contracts/fixtures/assets/motion-v2")
        .join(kind)
}
fn value() -> Value {
    serde_json::from_str(MANIFEST_JSON).unwrap()
}

#[test]
fn shared_config_fixtures_round_trip_and_resolve_without_fallback() {
    let catalog = Catalog::decode_candidate(MANIFEST_JSON.as_bytes()).unwrap();
    let mut count = 0;
    for entry in fs::read_dir(fixture_dir("valid")).unwrap() {
        let bytes = fs::read(entry.unwrap().path()).unwrap();
        let config = decode_config(&bytes).unwrap();
        let serialized = serde_json::to_value(&config).unwrap();
        assert_eq!(serialized, serde_json::from_slice::<Value>(&bytes).unwrap());
        match catalog.resolve(&config) {
            ResolvedSelection::Preset(p) => {
                assert_eq!(p.source_frames.len(), 20);
                assert!(p.local_key.starts_with("photo01-front-"));
            }
            ResolvedSelection::Layered {
                body,
                eyes,
                hair,
                outfit,
                accessory,
            } => {
                assert_eq!(body.slot, Slot::Body);
                assert!(body.skin_palette.is_some());
                assert_eq!(eyes.slot, Slot::Eyes);
                assert_eq!(hair.slot, Slot::Hair);
                if let Some(outfit) = outfit {
                    assert_eq!(outfit.slot, Slot::Outfit);
                    assert!(outfit.directions.iter().all(|d| d.hand_frames.is_some()));
                }
                if let Some(accessory) = accessory {
                    assert_eq!(accessory.slot, Slot::Accessory);
                }
            }
        }
        count += 1;
    }
    assert_eq!(count, 15);
}

#[test]
fn shared_invalid_fixtures_are_refused() {
    let mut count = 0;
    for entry in fs::read_dir(fixture_dir("invalid")).unwrap() {
        let path = entry.unwrap().path();
        assert!(
            decode_config(&fs::read(&path).unwrap()).is_err(),
            "{}",
            path.display()
        );
        count += 1;
    }
    assert_eq!(count, 16);
}

#[test]
fn raw_decoder_rejects_duplicate_keys_missing_nullable_and_unbounded_input() {
    let raw = fs::read_to_string(fixture_dir("valid").join("layered-body-0.json")).unwrap();
    for (needle, duplicate) in [
        (
            "\"contract_version\": \"2.0.0\"",
            "\"contract_version\": \"2.0.0\", \"contract_version\": \"2.0.0\"",
        ),
        (
            "\"representation\": \"motion_layered\"",
            "\"representation\": \"motion_layered\", \"representation\": \"motion_layered\"",
        ),
        (
            "\"body\": \"motion_v2_body_0\"",
            "\"body\": \"motion_v2_body_0\", \"body\": \"motion_v2_body_0\"",
        ),
    ] {
        assert!(raw.contains(needle));
        assert!(decode_config(raw.replacen(needle, duplicate, 1).as_bytes()).is_err());
    }
    for key in ["outfit", "accessory", "body", "eyes", "hair"] {
        let mut v: Value = serde_json::from_str(&raw).unwrap();
        v["item_ids"].as_object_mut().unwrap().remove(key);
        assert!(
            decode_config(&serde_json::to_vec(&v).unwrap()).is_err(),
            "missing {key}"
        );
    }
    assert!(matches!(
        decode_config(&vec![b' '; 4097]),
        Err(MotionError::TooLarge)
    ));
    assert!(decode_config(format!("{} null", raw).as_bytes()).is_err());
}

#[test]
fn every_closed_inventory_selection_resolves() {
    let catalog = Catalog::canonical_candidate();
    let m = catalog.manifest();
    for body in m.items.iter().filter_map(|v| {
        if let ItemId::Body(id) = v.item_id {
            Some(id)
        } else {
            None
        }
    }) {
        for eyes in m.items.iter().filter_map(|v| {
            if let ItemId::Eyes(id) = v.item_id {
                Some(id)
            } else {
                None
            }
        }) {
            for hair in m.items.iter().filter_map(|v| {
                if let ItemId::Hair(id) = v.item_id {
                    Some(id)
                } else {
                    None
                }
            }) {
                let outfits = std::iter::once(None).chain(m.items.iter().filter_map(|v| {
                    if let ItemId::Outfit(id) = v.item_id {
                        Some(Some(id))
                    } else {
                        None
                    }
                }));
                for outfit in outfits {
                    let accessories = std::iter::once(None).chain(m.items.iter().filter_map(|v| {
                        if let ItemId::Accessory(id) = v.item_id {
                            Some(Some(id))
                        } else {
                            None
                        }
                    }));
                    for accessory in accessories {
                        let config = AvatarConfig::layered(ItemIds {
                            body,
                            eyes,
                            hair,
                            outfit,
                            accessory,
                        });
                        let decoded = decode_config(&serde_json::to_vec(&config).unwrap()).unwrap();
                        assert_eq!(decoded, config);
                        assert!(matches!(
                            catalog.resolve(&decoded),
                            ResolvedSelection::Layered { .. }
                        ));
                    }
                }
            }
        }
    }
    for preset in &m.presets {
        assert!(matches!(
            catalog.resolve(&AvatarConfig::preset(preset.preset_id)),
            ResolvedSelection::Preset(_)
        ));
    }
}

#[test]
fn manifest_mutations_cannot_change_inventory_geometry_source_or_admission() {
    for (pointer, replacement) in [
        ("/contract_version", json!("1.1.0")),
        ("/manifest_version", json!(2)),
        ("/production_eligible", json!(true)),
        ("/qualification", json!("ACCEPTED")),
        ("/fallback", json!("legacy")),
        ("/atlas/path", json!("https://untrusted.invalid/atlas.png")),
        ("/atlas/sha256", json!("0".repeat(64))),
        ("/atlas/width", json!(64)),
        ("/atlas/height", json!(64)),
        ("/atlas/bytes", json!(1)),
        ("/geometry/foot/1", json!(59)),
        ("/geometry/cell/0", json!(32)),
        ("/geometry/output_cells", json!(1)),
        ("/geometry/directions/0", json!("up")),
        ("/geometry/phase_order/1", json!("idle")),
        ("/presets/0/preset_id", json!("synthetic_0")),
        (
            "/presets/0/source_frames/5/attribution",
            json!("SUPPLIED_ORIGINAL"),
        ),
        ("/items/0/local_key", json!("body-999")),
        ("/items/0/directions/0/frames/0", json!(576)),
        ("/items/0/skin_palette/colors/2/0", json!(0)),
        ("/items/0/slot", json!("outfit")),
        ("/source/originals/0/resource/sha256", json!("0".repeat(64))),
        ("/composition/dressed_body_clip_y_exclusive", json!(60)),
        ("/composition/hand_palette_color_index", json!(0)),
    ] {
        let mut v = value();
        *v.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            Catalog::decode_candidate(&serde_json::to_vec(&v).unwrap()).is_err(),
            "accepted {pointer}"
        );
    }
    let mut v = value();
    v["arbitrary"] = json!(true);
    assert!(Catalog::decode_candidate(&serde_json::to_vec(&v).unwrap()).is_err());
    let mut v = value();
    v["items"][0]["source_location"] = Value::Null;
    assert!(Catalog::decode_candidate(&serde_json::to_vec(&v).unwrap()).is_err());
    let raw = MANIFEST_JSON.replacen(
        "\"production_eligible\": false",
        "\"production_eligible\": false, \"production_eligible\": false",
        1,
    );
    assert!(Catalog::decode_candidate(raw.as_bytes()).is_err());
    assert!(matches!(
        Catalog::decode_candidate(&vec![b' '; 256 * 1024 + 1]),
        Err(MotionError::TooLarge)
    ));
}

#[test]
fn every_pinned_resource_is_verified_and_bad_bytes_paths_are_refused() {
    let catalog = Catalog::canonical_candidate();
    let mut count = 0;
    for pin in catalog.resources() {
        let bytes = fs::read(root().join(&pin.path)).unwrap();
        catalog.verify_resource(&pin.path, &bytes).unwrap();
        let mut corrupted = bytes.clone();
        corrupted[0] ^= 1;
        assert!(matches!(
            catalog.verify_resource(&pin.path, &corrupted),
            Err(MotionError::ResourceMismatch)
        ));
        assert!(catalog
            .verify_resource(&pin.path, &bytes[..bytes.len() - 1])
            .is_err());
        count += 1;
    }
    assert_eq!(count, 38);
    assert!(matches!(
        catalog.verify_resource("https://untrusted.invalid/atlas.png", b""),
        Err(MotionError::UnknownResource)
    ));
    assert!(matches!(
        catalog.verify_resource("../avatar.png", b""),
        Err(MotionError::UnknownResource)
    ));
}

#[test]
fn exact_frames_direction_aliases_skin_semantics_and_candidate_denial() {
    let catalog = Catalog::canonical_candidate();
    let m = catalog.manifest();
    assert_eq!(m.presets.len(), 6);
    assert_eq!(m.items.len(), 28);
    assert_eq!(m.geometry.cell, [64, 64]);
    assert_eq!(m.geometry.foot, [32, 60]);
    assert_eq!(m.geometry.output_cells, 20);
    assert_eq!(m.geometry.physical_frames, 576);
    assert_eq!(
        m.geometry.source_direction_aliases.down,
        SourceDirection::Front
    );
    assert_eq!(
        m.geometry.source_direction_aliases.up,
        SourceDirection::Rear
    );
    for (ordinal, preset) in m.presets.iter().enumerate() {
        for (d, frames) in preset.directions.iter().enumerate() {
            for (phase, index) in frames.frames.iter().enumerate() {
                assert_eq!(usize::from(*index), ordinal * 20 + d * 5 + phase);
            }
        }
    }
    let body = &m.items[0];
    assert_eq!(body.item_id.as_str(), "motion_v2_body_0");
    let palette = body.skin_palette.as_ref().unwrap();
    assert_eq!(palette.colors[2], [252, 226, 213]);
    assert_eq!(body.directions[0].frames, [120, 121, 122, 123, 124]);
    assert_eq!(m.composition.dressed_body_clip_y_exclusive, 42);
    assert_eq!(m.composition.hand_palette_color_index, 2);
    assert!(!m.production_eligible);
    assert_eq!(m.qualification, "CANDIDATE_ONLY");
    assert!(matches!(
        catalog.admit_production(),
        Err(MotionError::ArtNotReviewed)
    ));
}

#[test]
fn every_slot_boundary_is_closed_and_optional_means_explicit_null_only() {
    let base: Value = serde_json::from_slice(
        &fs::read(fixture_dir("valid").join("layered-body-0.json")).unwrap(),
    )
    .unwrap();
    for (slot, count) in [
        ("body", 8),
        ("eyes", 4),
        ("hair", 6),
        ("outfit", 6),
        ("accessory", 4),
    ] {
        for unknown in [
            format!("motion_v2_{slot}_{count}"),
            format!("motion_v2_{slot}_-1"),
            format!("motion_v2_{slot}_00"),
            "https://untrusted.invalid/a.png".to_string(),
        ] {
            let mut v = base.clone();
            v["item_ids"][slot] = json!(unknown);
            assert!(
                decode_config(&serde_json::to_vec(&v).unwrap()).is_err(),
                "accepted unknown {slot}"
            );
        }
        let mut v = base.clone();
        v["item_ids"][slot] = Value::Null;
        assert_eq!(
            decode_config(&serde_json::to_vec(&v).unwrap()).is_ok(),
            matches!(slot, "outfit" | "accessory")
        );
    }
    for (field, invalid) in [
        ("contract_version", json!("2.0")),
        ("manifest_id", json!("mcr_selected_motion_20261103 ")),
        ("manifest_version", json!(0)),
        ("manifest_version", json!(-1)),
        ("manifest_version", json!(256)),
        ("manifest_version", json!("1")),
        ("representation", json!("layered")),
    ] {
        let mut v = base.clone();
        v[field] = invalid;
        assert!(
            decode_config(&serde_json::to_vec(&v).unwrap()).is_err(),
            "accepted bad {field}"
        );
    }
}

// Reviewed successor checks deliberately exercise the public selection parser,
// not only deserialization of the new leaf enum.
#[test]
fn reviewed_fixtures_round_trip_through_public_selection_and_commands() {
    use concert_domain::avatar::{config_value, parse_config, AvatarCatalog, SetAvatar};
    let manifest: Value = serde_json::from_str(reviewed::MANIFEST_JSON).unwrap();
    let catalog = AvatarCatalog::from_validated_manifest(&manifest).unwrap();
    for (kind, expected, count) in [("valid", true, 15), ("invalid", false, 16)] {
        let mut seen = 0;
        for entry in fs::read_dir(fixture_dir("reviewed").join(kind)).unwrap() {
            let path = entry.unwrap().path();
            let bytes = fs::read(&path).unwrap();
            let result = parse_config(&bytes);
            assert_eq!(result.is_ok(), expected, "{}", path.display());
            assert_eq!(reviewed::Config::decode(&bytes).is_ok(), expected);
            if let Ok(selection) = result {
                catalog.validate(&selection).unwrap();
                let original: Value = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(config_value(&selection), original);
                let wire = json!({"expected_revision":1,"config":original});
                assert_eq!(
                    SetAvatar::parse(&serde_json::to_vec(&wire).unwrap())
                        .unwrap()
                        .selection,
                    selection
                );
            }
            seen += 1;
        }
        assert_eq!(seen, count);
    }
    // The immutable candidate remains structurally usable only in its own API.
    for entry in fs::read_dir(fixture_dir("valid")).unwrap() {
        assert!(parse_config(&fs::read(entry.unwrap().path()).unwrap()).is_err());
    }
    assert!(AvatarCatalog::from_validated_manifest(&value()).is_err());
}

#[test]
fn reviewed_public_parser_refuses_duplicate_headers_and_nested_nullable_slots() {
    use concert_domain::avatar::{parse_config, SetAvatar};
    let raw =
        fs::read_to_string(fixture_dir("reviewed/valid").join("layered-body-0.json")).unwrap();
    for key in [
        "contract_version",
        "representation",
        "body",
        "outfit",
        "accessory",
    ] {
        let v: Value = serde_json::from_str(&raw).unwrap();
        let item = if ["body", "outfit", "accessory"].contains(&key) {
            &v["item_ids"][key]
        } else {
            &v[key]
        };
        let token = format!("\"{key}\": {}", serde_json::to_string(item).unwrap());
        assert!(raw.contains(&token));
        let duplicate = raw.replacen(&token, &format!("{token}, {token}"), 1);
        assert!(parse_config(duplicate.as_bytes()).is_err(), "{key}");
        let request = format!("{{\"expected_revision\":1,\"config\":{duplicate}}}");
        assert!(
            SetAvatar::parse(request.as_bytes()).is_err(),
            "command {key}"
        );
    }
    for key in ["body", "eyes", "hair", "outfit", "accessory"] {
        let mut v: Value = serde_json::from_str(&raw).unwrap();
        v["item_ids"].as_object_mut().unwrap().remove(key);
        assert!(parse_config(&serde_json::to_vec(&v).unwrap()).is_err());
    }
    assert!(parse_config(
        raw.replace("\"manifest_version\": 2", "\"manifest_version\": 2.0")
            .as_bytes()
    )
    .is_err());
}

#[test]
fn reviewed_admission_requires_exact_manifest_review_and_actual_resource_bytes() {
    let structure =
        reviewed::StructuralCatalog::decode(reviewed::MANIFEST_JSON.as_bytes()).unwrap();
    let mut review = reviewed::REVIEW_JSON.as_bytes().to_vec();
    review[0] ^= 1;
    assert!(structure.clone().bind_visual_review(&review).is_err());
    let reviewed = structure
        .bind_visual_review(reviewed::REVIEW_JSON.as_bytes())
        .unwrap();
    let mut paths = std::collections::BTreeSet::new();
    let eligible = reviewed
        .clone()
        .verify_resources(|path| {
            paths.insert(path.to_owned());
            fs::read(root().join(path)).map_err(|_| MotionError::UnknownResource)
        })
        .unwrap();
    assert!(paths.len() >= 30);
    assert_eq!(
        eligible.reviewed().document()["manifest_id"],
        reviewed::MANIFEST_ID
    );
    for target in paths {
        assert!(
            reviewed
                .clone()
                .verify_resources(|path| {
                    let mut bytes =
                        fs::read(root().join(path)).map_err(|_| MotionError::UnknownResource)?;
                    if path == target {
                        bytes[0] ^= 1;
                    }
                    Ok(bytes)
                })
                .is_err(),
            "tampered {target}"
        );
    }
    for (pointer, replacement) in [
        ("/manifest_id", json!(MANIFEST_ID)),
        ("/manifest_version", json!(1)),
        ("/production_eligible", json!(false)),
        ("/content_hash", json!("sha256:0000")),
        ("/review/sha256", json!("0".repeat(64))),
        ("/atlas/width", json!(64)),
        ("/items/0/item_id", json!("legacy")),
    ] {
        let mut v: Value = serde_json::from_str(reviewed::MANIFEST_JSON).unwrap();
        *v.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            reviewed::StructuralCatalog::decode(&serde_json::to_vec(&v).unwrap()).is_err(),
            "{pointer}"
        );
    }
}
