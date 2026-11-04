//! C03 assembly tests only; these do not mint a canonical authority scope.
use concert_domain::avatar::AvatarCatalog;
use concert_service::adapters::out::public_profile::validate;
use serde_json::{json, Value};
use uuid::Uuid;
fn profile(config: Value) -> Value {
    json!({"user_id":Uuid::new_v4(),"display_name":"Canonical 音楽","bio":null,"avatar":{"revision":3,"config":config}})
}
fn layered() -> Value {
    serde_json::from_str(include_str!(
        "../../../../contracts/fixtures/assets/avatar-config.json"
    ))
    .unwrap()
}
#[test]
fn canonical_layered_and_all_six_selected_whole_character_configs_fit_exact_public_schema() {
    validate(&profile(layered())).unwrap();
    let manifest: Value = serde_json::from_str(include_str!(
        "../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json"
    ))
    .unwrap();
    let catalog = AvatarCatalog::from_validated_manifest(&manifest).unwrap();
    for n in 1..=6 {
        let config = json!({"contract_version":"1.1.0","kind":"avatar_config","manifest_id":"mcr_selected_static_20261103","manifest_version":1,"representation":"whole_character","preset_id":format!("photo01_front_{n:02}")});
        let command = concert_domain::avatar::SetAvatar::parse(
            &serde_json::to_vec(&json!({"expected_revision":1,"config":config})).unwrap(),
        )
        .unwrap();
        catalog.validate(&command.selection).unwrap();
        assert_eq!(
            concert_domain::avatar::config_value(&command.selection),
            config
        );
        validate(&profile(config)).unwrap();
    }
}
#[test]
fn private_fields_and_every_unconsented_link_or_age_field_are_rejected() {
    let original = profile(layered());
    for key in [
        "email",
        "social_handles",
        "social_links",
        "age",
        "verified_age",
        "provider_claims",
        "private_meeting",
        "account_visibility",
        "avatar_url",
        "follower_count",
        "username",
        "session_family",
    ] {
        let mut changed = original.clone();
        changed[key] = json!("private-canary");
        assert!(validate(&changed).is_err(), "field {key}");
    }
    for key in ["user_id", "display_name", "bio", "avatar"] {
        let mut changed = original.clone();
        changed.as_object_mut().unwrap().remove(key);
        assert!(validate(&changed).is_err());
    }
}
#[test]
fn malformed_avatar_union_and_boundaries_fail_closed() {
    let original = profile(layered());
    for (path, value) in [
        ("revision", json!(0)),
        ("revision", json!(1.5)),
        ("revision", Value::Null),
    ] {
        let mut changed = original.clone();
        changed["avatar"][path] = value;
        assert!(validate(&changed).is_err());
    }
    let mut changed = original.clone();
    changed["avatar"]["config"]["preset_id"] = json!("photo01_front_01");
    assert!(validate(&changed).is_err());
    let mut changed = original.clone();
    changed["display_name"] = json!("x".repeat(41));
    assert!(validate(&changed).is_err());
    let mut changed = original.clone();
    changed["bio"] = json!("音".repeat(161));
    assert!(validate(&changed).is_err());
    let mut changed = original.clone();
    changed["avatar"]["config"] = json!({"contract_version":"1.1.0","kind":"avatar_config","manifest_id":"mcr_selected_static_20261103","manifest_version":1,"representation":"whole_character","preset_id":"photo01_front_07"});
    assert!(validate(&changed).is_err());
}
