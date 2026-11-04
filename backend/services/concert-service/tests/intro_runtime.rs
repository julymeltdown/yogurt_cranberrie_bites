use concert_service::{
    config::intro_runtime::IntroRuntimeSettings, product_rpc::child_recovery::ChildRecoverySettings,
};
use serde_json::json;
use std::collections::BTreeMap;

fn config() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("CONCERT_INTRO_MODE".into(), "live".into()),
        ("CONCERT_PARTICIPANT_PROFILE_GRPC_ENDPOINT".into(), "https://profile.internal:8443".into()),
        ("CONCERT_NOTIFICATION_GRPC_ENDPOINT".into(), "https://notification.internal:8443".into()),
        ("CONCERT_INTRO_CURSOR_KEY_HEX".into(), "11".repeat(32)),
        ("CONCERT_INTRO_RECEIPT_KEYS_JSON".into(), json!([{"id":"current","secret_hex":"22".repeat(32)},{"id":"previous","secret_hex":"33".repeat(32)}]).to_string()),
        ("CONCERT_PICKUP_KEYS_JSON".into(), json!([{"id":"pickup","secret_hex":"44".repeat(32)}]).to_string()),
    ])
}
fn parse(values: &BTreeMap<String, String>) -> anyhow::Result<Option<IntroRuntimeSettings>> {
    IntroRuntimeSettings::from_values(
        |key| Ok(values.get(key).cloned()),
        true,
        true,
        ChildRecoverySettings::default(),
    )
}
#[test]
fn off_does_not_read_optional_secrets_or_owner_configuration() {
    for mode in [None, Some("off")] {
        let mut names = Vec::new();
        let result = IntroRuntimeSettings::from_values(
            |name| {
                names.push(name.to_string());
                if name == "CONCERT_INTRO_MODE" {
                    Ok(mode.map(str::to_string))
                } else {
                    anyhow::bail!("unexpected secret access")
                }
            },
            false,
            false,
            ChildRecoverySettings::default(),
        )
        .unwrap();
        assert!(result.is_none());
        assert_eq!(names, ["CONCERT_INTRO_MODE"]);
    }
}
#[test]
fn activation_requires_real_shared_listener_and_every_explicit_input() {
    let values = config();
    assert!(parse(&values).unwrap().is_some());
    for (grpc, participant) in [(false, false), (false, true), (true, false)] {
        assert!(IntroRuntimeSettings::from_values(
            |key| Ok(values.get(key).cloned()),
            grpc,
            participant,
            ChildRecoverySettings::default()
        )
        .is_err());
    }
    for name in values
        .keys()
        .filter(|name| name.as_str() != "CONCERT_INTRO_MODE")
    {
        let mut missing = values.clone();
        missing.remove(name);
        assert!(parse(&missing).is_err(), "missing {name}");
    }
    for mode in ["", "on", "true", "LIVE", "fixture", " live"] {
        let mut invalid = values.clone();
        invalid.insert("CONCERT_INTRO_MODE".into(), mode.into());
        assert!(parse(&invalid).is_err());
    }
}
#[test]
fn owner_endpoint_configuration_cannot_smuggle_credentials_or_paths() {
    for name in [
        "CONCERT_PARTICIPANT_PROFILE_GRPC_ENDPOINT",
        "CONCERT_NOTIFICATION_GRPC_ENDPOINT",
    ] {
        for bad in [
            "http://localhost:8443",
            "https://localhost",
            "https://localhost:0",
            "https://user@localhost:8443",
            "https://localhost:8443/rpc",
            "https://localhost:8443/?key=value",
        ] {
            let mut values = config();
            values.insert(name.into(), bad.into());
            assert!(parse(&values).is_err(), "endpoint accepted: {bad}");
        }
    }
}
#[test]
fn retained_keys_have_distinct_roles_ids_and_closed_shapes() {
    for raw in [
        json!([]),
        json!([{"id":"same","secret_hex":"22".repeat(32)},{"id":"same","secret_hex":"33".repeat(32)}]),
        json!([{"id":"one","secret_hex":"22".repeat(32)},{"id":"two","secret_hex":"22".repeat(32)}]),
        json!([{"id":"active","secret_hex":"22".repeat(32),"enabled":true}]),
        json!([{"id":"active","secret_hex":"00".repeat(32)}]),
        json!([{"id":"active","secret_hex":"AA".repeat(32)}]),
        json!([{"id":"active","secret_hex":"11".repeat(32)}]),
    ] {
        let mut values = config();
        values.insert("CONCERT_INTRO_RECEIPT_KEYS_JSON".into(), raw.to_string());
        assert!(parse(&values).is_err());
    }
    let mut values = config();
    values.insert(
        "CONCERT_PICKUP_KEYS_JSON".into(),
        json!([{"id":"pickup","secret_hex":"33".repeat(32)}]).to_string(),
    );
    assert!(
        parse(&values).is_err(),
        "retained receipt key must not become pickup material"
    );
    // JSON duplicate fields are rejected before any typed key is constructed.
    values = config();
    values.insert(
        "CONCERT_INTRO_RECEIPT_KEYS_JSON".into(),
        format!(
            "[{{\"id\":\"a\",\"id\":\"b\",\"secret_hex\":\"{}\"}}]",
            "22".repeat(32)
        ),
    );
    assert!(parse(&values).is_err());
}
#[test]
fn validated_configuration_passes_actual_owner_inputs_without_synthetic_defaults() {
    let values = config();
    let settings = parse(&values).unwrap().unwrap();
    let additional = settings.participant_owners();
    assert_eq!(
        additional.profile_endpoint,
        values["CONCERT_PARTICIPANT_PROFILE_GRPC_ENDPOINT"]
    );
    assert_eq!(
        additional.notification_endpoint,
        values["CONCERT_NOTIFICATION_GRPC_ENDPOINT"]
    );
    assert_eq!(
        settings.recovery_settings(),
        ChildRecoverySettings::default()
    );
}
