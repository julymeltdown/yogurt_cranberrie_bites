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
