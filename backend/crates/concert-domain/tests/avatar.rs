use concert_domain::avatar::*;
use serde_json::{json, Value};
fn config() -> Value {
    serde_json::from_str(include_str!(
        "../../../../contracts/fixtures/assets/avatar-config.json"
    ))
    .unwrap()
}
fn request(config: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"expected_revision":1,"config":config})).unwrap()
}
fn catalog() -> AvatarCatalog {
    AvatarCatalog::from_validated_manifest(
        &serde_json::from_str(include_str!(
            "../../../../contracts/fixtures/assets/avatar-manifest.json"
        ))
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn exact_wire_and_catalog_layer_membership() {
    let catalog = catalog();
    let parsed = SetAvatar::parse(&request(config())).unwrap();
    catalog.validate(&parsed.selection).unwrap();
    assert_eq!(config_value(&parsed.selection), config());
    let mut none = config();
    none["item_ids"]["accessory"] = Value::Null;
    catalog
        .validate(&SetAvatar::parse(&request(none)).unwrap().selection)
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
            catalog.validate(&SetAvatar::parse(&request(c)).unwrap().selection),
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
            .validate(&SetAvatar::parse(&request(c)).unwrap().selection)
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
        assert!(SetAvatar::parse(
            &serde_json::to_vec(&json!({"expected_revision":expected,"config":config()})).unwrap()
        )
        .is_err());
    }
    let valid = String::from_utf8(request(config())).unwrap();
    assert!(SetAvatar::parse(
        valid
            .replace(
                "\"expected_revision\":1",
                "\"expected_revision\":1,\"expected_revision\":1"
            )
            .as_bytes()
    )
    .is_err());
    assert!(SetAvatar::parse(&vec![b' '; MAX_REQUEST_BYTES + 1]).is_err());
    for (field, bad) in [
        ("contract_version", json!("2.0.0")),
        ("kind", json!("image")),
        ("url", json!("https://evil.invalid/a.png")),
        ("manifest_id", json!("https://evil.invalid")),
    ] {
        let mut c = config();
        c[field] = bad;
        assert!(SetAvatar::parse(&request(c)).is_err());
    }
    let mut c = config();
    c["item_ids"].as_object_mut().unwrap().remove("accessory");
    assert!(SetAvatar::parse(&request(c)).is_err());
    let mut c = config();
    c["item_ids"]["extra"] = "hidden".into();
    assert!(SetAvatar::parse(&request(c)).is_err());
}

fn selected_manifest() -> Value {
    serde_json::from_str(include_str!(
        "../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json"
    ))
    .unwrap()
}
#[test]
fn selected_six_roundtrip_and_strict_exclusive_union() {
    let manifest = selected_manifest();
    let selected = AvatarCatalog::from_validated_manifest(&manifest).unwrap();
    for preset in manifest["presets"].as_array().unwrap() {
        let config = &preset["config"];
        let command = SetAvatar::parse(&request(config.clone())).unwrap();
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
            assert!(SetAvatar::parse(&request(invalid)).is_err(), "{key}");
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
                SetAvatar::parse(&request(invalid)).is_err(),
                "missing {key}"
            );
        }
        let bytes = String::from_utf8(request(config.clone())).unwrap();
        for key in ["contract_version", "preset_id", "representation"] {
            let needle = format!("\"{key}\":");
            let duplicate = bytes.replacen(&needle, &format!("\"{key}\":null,{needle}"), 1);
            assert!(
                SetAvatar::parse(duplicate.as_bytes()).is_err(),
                "duplicate {key}"
            );
        }
    }
    assert!(selected
        .validate(&SetAvatar::parse(&request(config())).unwrap().selection)
        .is_err());
    let mut invalid = manifest.clone();
    invalid["presets"][5] = invalid["presets"][0].clone();
    assert!(AvatarCatalog::from_validated_manifest(&invalid).is_err());
    invalid = manifest.clone();
    invalid["default_preset_id"] = json!("missing");
    assert!(AvatarCatalog::from_validated_manifest(&invalid).is_err());
}
