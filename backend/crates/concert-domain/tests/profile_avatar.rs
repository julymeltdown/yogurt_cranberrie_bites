//! WP-08 T-701: the profile-owned avatar domain.
//!
//! Two obligations are proved here:
//!
//! 1. the strict decoder and the catalog inventory checks reject exactly the
//!    same inputs the concert avatar rejects — the port must not loosen them;
//! 2. nothing in the module is keyed by an event or a participant. That is
//!    asserted structurally (the module's own source has no event/participant
//!    path) and behaviourally (a profile id is the only owner, and an event id
//!    cannot even be spelled as one).
use concert_domain::profile_avatar::*;
use serde_json::{json, Value};
use uuid::Uuid;

fn config() -> Value {
    serde_json::from_str(include_str!(
        "../../../../contracts/fixtures/assets/avatar-config.json"
    ))
    .unwrap()
}

fn request(config: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"expected_revision":1,"config":config})).unwrap()
}

fn catalog() -> ProfileAvatarCatalog {
    ProfileAvatarCatalog::from_validated_manifest(
        &serde_json::from_str(include_str!(
            "../../../../contracts/fixtures/assets/avatar-manifest.json"
        ))
        .unwrap(),
    )
    .unwrap()
}

fn selected_manifest() -> Value {
    serde_json::from_str(include_str!(
        "../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json"
    ))
    .unwrap()
}

// ---------------------------------------------------------------------------
// The owner is a profile, never an event or a participant
// ---------------------------------------------------------------------------

#[test]
fn the_owner_is_a_canonical_profile_id() {
    let profile = Uuid::parse_str("00000000-0000-4000-8000-000000000801").unwrap();
    let id = ProfileId::from_uuid(profile).unwrap();
    assert_eq!(id.uuid(), profile);
    assert_eq!(id.to_string(), "00000000-0000-4000-8000-000000000801");
    assert_eq!(
        "00000000-0000-4000-8000-000000000801".parse::<ProfileId>(),
        Ok(id)
    );
    // The nil UUID is the schema's "no owner" sentinel and is never a profile.
    assert_eq!(ProfileId::from_uuid(Uuid::nil()), Err(ConfigError::Invalid));
    assert!("00000000-0000-0000-0000-000000000000"
        .parse::<ProfileId>()
        .is_err());
    // Non-canonical spellings are refused: upper case, braces, urn prefix, and
    // any value that is not a UUID at all.
    for bad in [
        "00000000-0000-4000-8000-00000000080A",
        "{00000000-0000-4000-8000-000000000801}",
        "urn:uuid:00000000-0000-4000-8000-000000000801",
        "00000000000040008000000000000801",
        "not-a-profile",
        "",
    ] {
        assert!(bad.parse::<ProfileId>().is_err(), "{bad}");
    }
}

/// The module must not be able to carry an event or a participant at all. This
/// is the structural half of "no function signature accepts an event id or a
/// participant key": the source of the profile avatar module is inspected, so a
/// later edit that reintroduces the coupling fails here rather than in review.
#[test]
fn the_module_source_has_no_event_or_participant_coupling() {
    let source = include_str!("../src/profile_avatar.rs");
    for forbidden in [
        "ParticipantKey",
        "participant",
        "event_id",
        "EventId",
        "eventId",
        "/events/",
    ] {
        assert!(
            !source.contains(forbidden),
            "profile avatar module must not mention {forbidden}"
        );
    }
}

/// An event id is a UUID too, so "it parses as a profile" is not the point: the
/// point is that no API accepts one as an owner. The set-avatar command carries
/// exactly `expected_revision` and `config`; a request that tries to name its
/// own owner (an event, a participant, or another profile) is refused.
#[test]
fn a_set_avatar_request_cannot_name_its_own_owner() {
    for extra in [
        json!({"event_id": "00000000-0000-4000-8000-000000000701"}),
        json!({"eventId": "00000000-0000-4000-8000-000000000701"}),
        json!({"profile_id": "00000000-0000-4000-8000-000000000802"}),
        json!({"user_id": "00000000-0000-4000-8000-000000000702"}),
    ] {
        let mut body = json!({"expected_revision":1,"config":config()});
        for (key, value) in extra.as_object().unwrap() {
            body[key] = value.clone();
        }
        assert!(
            SetProfileAvatar::parse(&serde_json::to_vec(&body).unwrap()).is_err(),
            "{extra}"
        );
    }
}

// ---------------------------------------------------------------------------
// The strict decoder rejects what it rejected before
// ---------------------------------------------------------------------------

#[test]
fn exact_wire_and_catalog_layer_membership() {
    let catalog = catalog();
    assert_eq!(catalog.manifest(), ("avatar_fixture", 1));
    let parsed = SetProfileAvatar::parse(&request(config())).unwrap();
    catalog.validate(&parsed.selection).unwrap();
    assert_eq!(config_value(&parsed.selection), config());
    let mut none = config();
    none["item_ids"]["accessory"] = Value::Null;
    catalog
        .validate(&SetProfileAvatar::parse(&request(none)).unwrap().selection)
        .unwrap();
    for (field, bad) in [
        ("body", "eyes_fixture"),
        ("hair", "sheet_01"),
        ("outfit", "unknown"),
        ("accessory", "body_fixture"),
    ] {
        let mut c = config();
        c["item_ids"][field] = bad.into();
        assert_eq!(
            catalog.validate(&SetProfileAvatar::parse(&request(c)).unwrap().selection),
            Err(ConfigError::Invalid)
        );
    }
    for (field, bad) in [
        ("skin_tone_id", json!("foreign")),
        ("manifest_id", json!("other")),
        ("manifest_version", json!(2)),
    ] {
        let mut c = config();
        c[field] = bad;
        assert!(catalog
            .validate(&SetProfileAvatar::parse(&request(c)).unwrap().selection)
            .is_err());
    }
}

#[test]
fn bounded_unambiguous_input_without_urls_or_extra_fields() {
    for expected in [
        json!(0),
        json!(-1),
        json!(1.0),
        json!(true),
        json!(9007199254740992_i64),
    ] {
        assert!(SetProfileAvatar::parse(
            &serde_json::to_vec(&json!({"expected_revision":expected,"config":config()})).unwrap()
        )
        .is_err());
    }
    let valid = String::from_utf8(request(config())).unwrap();
    assert!(SetProfileAvatar::parse(
        valid
            .replace(
                "\"expected_revision\":1",
                "\"expected_revision\":1,\"expected_revision\":1"
            )
            .as_bytes()
    )
    .is_err());
    assert!(SetProfileAvatar::parse(&vec![b' '; MAX_REQUEST_BYTES + 1]).is_err());
    assert!(SetProfileAvatar::parse(b"").is_err());
    for (field, bad) in [
        ("contract_version", json!("2.0.0")),
        ("kind", json!("image")),
        ("url", json!("https://evil.invalid/a.png")),
        ("manifest_id", json!("https://evil.invalid")),
    ] {
        let mut c = config();
        c[field] = bad;
        assert!(SetProfileAvatar::parse(&request(c)).is_err());
    }
    let mut c = config();
    c["item_ids"].as_object_mut().unwrap().remove("accessory");
    assert!(SetProfileAvatar::parse(&request(c)).is_err());
    let mut c = config();
    c["item_ids"]["extra"] = "hidden".into();
    assert!(SetProfileAvatar::parse(&request(c)).is_err());
}

#[test]
fn selected_six_roundtrip_and_strict_exclusive_union() {
    let manifest = selected_manifest();
    let selected = ProfileAvatarCatalog::from_validated_manifest(&manifest).unwrap();
    assert_eq!(selected.manifest(), ("mcr_selected_static_20261103", 1));
    for preset in manifest["presets"].as_array().unwrap() {
        let config = &preset["config"];
        let command = SetProfileAvatar::parse(&request(config.clone())).unwrap();
        selected.validate(&command.selection).unwrap();
        assert_eq!(config_value(&command.selection), *config);
        assert!(catalog().validate(&command.selection).is_err());
        for (key, value) in [
            ("item_ids", json!({})),
            ("skin_tone_id", json!("tone_fixture")),
            ("url", json!("https://evil.invalid/a.png")),
            ("frame", json!(0)),
            ("extra", json!(null)),
            ("preset_id", json!("photo01_front_07")),
            ("manifest_id", json!("other")),
            ("manifest_version", json!(2)),
            ("representation", json!("layered")),
            ("contract_version", json!("1.0.0")),
        ] {
            let mut invalid = config.clone();
            invalid[key] = value;
            assert!(SetProfileAvatar::parse(&request(invalid)).is_err(), "{key}");
        }
        for key in [
            "kind",
            "contract_version",
            "manifest_id",
            "manifest_version",
            "representation",
            "preset_id",
        ] {
            let mut invalid = config.clone();
            invalid.as_object_mut().unwrap().remove(key);
            assert!(
                SetProfileAvatar::parse(&request(invalid)).is_err(),
                "missing {key}"
            );
        }
        let bytes = String::from_utf8(request(config.clone())).unwrap();
        for key in ["contract_version", "preset_id", "representation"] {
            let needle = format!("\"{key}\":");
            let duplicate = bytes.replacen(&needle, &format!("\"{key}\":null,{needle}"), 1);
            assert!(
                SetProfileAvatar::parse(duplicate.as_bytes()).is_err(),
                "duplicate {key}"
            );
        }
    }
    assert!(selected
        .validate(&SetProfileAvatar::parse(&request(config())).unwrap().selection)
        .is_err());
    let mut invalid = manifest.clone();
    invalid["presets"][5] = invalid["presets"][0].clone();
    assert!(ProfileAvatarCatalog::from_validated_manifest(&invalid).is_err());
    invalid = manifest.clone();
    invalid["default_preset_id"] = json!("missing");
    assert!(ProfileAvatarCatalog::from_validated_manifest(&invalid).is_err());
}

// ---------------------------------------------------------------------------
// The reviewed motion branch stays admitted exactly as before
// ---------------------------------------------------------------------------

#[test]
fn reviewed_motion_configs_roundtrip_and_validate() {
    let manifest: Value =
        serde_json::from_str(concert_domain::avatar_motion_v2::reviewed::MANIFEST_JSON).unwrap();
    let reviewed = ProfileAvatarCatalog::from_validated_manifest(&manifest).unwrap();
    assert_eq!(reviewed.manifest(), ("mcr_selected_motion_reviewed_20261104", 2));
    let preset = json!({
        "contract_version": "2.0.0", "kind": "avatar_config",
        "manifest_id": "mcr_selected_motion_reviewed_20261104", "manifest_version": 2,
        "representation": "motion_preset", "preset_id": "motion_v2_photo01_front_01"
    });
    let parsed = SetProfileAvatar::parse(&request(preset.clone())).unwrap();
    reviewed.validate(&parsed.selection).unwrap();
    assert_eq!(config_value(&parsed.selection), preset);
    let layered = json!({
        "contract_version": "2.0.0", "kind": "avatar_config",
        "manifest_id": "mcr_selected_motion_reviewed_20261104", "manifest_version": 2,
        "representation": "motion_layered",
        "item_ids": {"body": "motion_v2_body_0", "eyes": "motion_v2_eyes_0", "hair": "motion_v2_hair_0",
            "outfit": null, "accessory": null}
    });
    let parsed = SetProfileAvatar::parse(&request(layered.clone())).unwrap();
    reviewed.validate(&parsed.selection).unwrap();
    assert_eq!(config_value(&parsed.selection), layered);
    // A reviewed config is not admitted by the layered fixture catalog, and a
    // candidate-family config is not admitted by the reviewed catalog.
    assert!(reviewed
        .validate(&SetProfileAvatar::parse(&request(config())).unwrap().selection)
        .is_err());
    let mut candidate = preset.clone();
    candidate["manifest_id"] = json!("mcr_selected_motion_20261103");
    candidate["manifest_version"] = json!(1);
    assert!(SetProfileAvatar::parse(&request(candidate)).is_err());
}

#[test]
fn parse_config_is_the_same_strict_decoder_as_the_command() {
    let bytes = serde_json::to_vec(&config()).unwrap();
    let selection = parse_config(&bytes).unwrap();
    assert_eq!(config_value(&selection), config());
    assert!(parse_config(b"").is_err());
    assert!(parse_config(&vec![b' '; MAX_REQUEST_BYTES + 1]).is_err());
    let mut unknown = config();
    unknown["extra"] = json!(1);
    assert!(parse_config(&serde_json::to_vec(&unknown).unwrap()).is_err());
}

#[test]
fn avatar_state_is_exactly_revision_and_config() {
    let selection = parse_config(&serde_json::to_vec(&config()).unwrap()).unwrap();
    let state = avatar_state(concert_domain::Revision::new(3).unwrap(), &selection);
    assert_eq!(state, json!({"revision": 3, "config": config()}));
    assert_eq!(
        state.as_object().unwrap().keys().collect::<Vec<_>>(),
        vec!["config", "revision"]
    );
}
