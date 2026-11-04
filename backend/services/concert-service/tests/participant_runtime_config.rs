//! Pure production opt-in configuration boundaries; no env mutation or fake owner.
use concert_service::config::participant_runtime::settings;
use std::collections::BTreeMap;
fn values() -> BTreeMap<String, String> {
    [
        ("CONCERT_PARTICIPANT_MODE", "live".into()),
        ("CONCERT_PARTICIPANT_AUTH_GRPC_ENDPOINT", "https://auth.internal:50051".into()),
        ("CONCERT_PARTICIPANT_FRIEND_GRPC_ENDPOINT", "https://friend.internal:50052".into()),
        ("CONCERT_PARTICIPANT_PROFILE_GRPC_ENDPOINT", "https://profile.internal:50053".into()),
        ("CONCERT_PRODUCER_INSTANCE_ID", "16108c1d-4d63-4ae3-9f30-65bbb9f54b1d".into()),
        ("CONCERT_PARTICIPANT_RELEASE_DIGEST", "ab".repeat(32)),
        ("CONCERT_PARTICIPANT_RECOVERY_KEYS_JSON", serde_json::json!([{"id":"active_c03","secret_hex":"21".repeat(32)}, {"id":"previous_c03","secret_hex":"31".repeat(32)}]).to_string()),
        ("GATEWAY_SERVICE_CREDENTIAL", "configured-c03-gateway".into()),
        ("AUTH_SERVICE_CREDENTIAL", "configured-c03-auth".into()),
        ("FRIEND_SERVICE_CREDENTIAL", "configured-c03-friend".into()),
        ("PROFILE_SERVICE_CREDENTIAL", "configured-c03-profile".into()),
        ("CONCERT_SERVICE_CREDENTIAL", "configured-c03-concert".into()),
    ].into_iter().map(|(k,v)|(k.into(),v)).collect()
}
fn valid(v: &BTreeMap<String, String>, grpc: bool) -> bool {
    settings(|name| Ok(v.get(name).cloned()), grpc).is_ok()
}
#[test]
fn default_and_explicit_off_create_no_runtime_or_authority() {
    assert!(settings(|_| Ok(None), false).unwrap().is_none());
    assert!(settings(
        |name| Ok((name == "CONCERT_PARTICIPANT_MODE").then(|| "off".into())),
        false
    )
    .unwrap()
    .is_none());
}
#[test]
fn actual_live_opt_in_requires_guarded_listener_and_exact_mode() {
    let mut v = values();
    assert!(!valid(&v, false));
    for mode in ["", "on", "LIVE", " live "] {
        v.insert("CONCERT_PARTICIPANT_MODE".into(), mode.into());
        assert!(!valid(&v, true));
    }
}
#[test]
fn complete_configuration_is_explicit_and_independent_of_optional_products() {
    let v = values();
    let s = settings(|name| Ok(v.get(name).cloned()), true)
        .unwrap()
        .unwrap();
    assert_eq!(
        s.instance_id().to_string(),
        v["CONCERT_PRODUCER_INSTANCE_ID"]
    );
    assert_eq!(s.release_digest(), v["CONCERT_PARTICIPANT_RELEASE_DIGEST"]);
    for key in v
        .keys()
        .filter(|k| k.as_str() != "CONCERT_PARTICIPANT_MODE")
    {
        let mut incomplete = v.clone();
        incomplete.remove(key);
        assert!(!valid(&incomplete, true), "missing {key}");
    }
}
#[test]
fn endpoint_configuration_rejects_plaintext_credentials_queries_and_implicit_ports() {
    for bad in [
        "http://auth.internal:50051",
        "https://auth.internal",
        "https://auth.internal:0",
        "https://user:secret@auth.internal:50051",
        "https://auth.internal:50051/private",
        "https://auth.internal:50051?token=private",
        "https://auth.internal:99999",
        "not a URI",
    ] {
        for key in [
            "CONCERT_PARTICIPANT_AUTH_GRPC_ENDPOINT",
            "CONCERT_PARTICIPANT_FRIEND_GRPC_ENDPOINT",
            "CONCERT_PARTICIPANT_PROFILE_GRPC_ENDPOINT",
        ] {
            let mut v = values();
            v.insert(key.into(), bad.into());
            assert!(!valid(&v, true));
        }
    }
}
#[test]
fn release_and_instance_are_canonical_not_browser_or_random_defaults() {
    for bad in ["", "a", &"AB".repeat(32), &"gg".repeat(32)] {
        let mut v = values();
        v.insert("CONCERT_PARTICIPANT_RELEASE_DIGEST".into(), bad.into());
        assert!(!valid(&v, true));
    }
    for bad in [
        "00000000-0000-0000-0000-000000000000",
        "16108C1D-4D63-4AE3-9F30-65BBB9F54B1D",
        "16108c1d4d634ae39f3065bbb9f54b1d",
        "bad",
    ] {
        let mut v = values();
        v.insert("CONCERT_PRODUCER_INSTANCE_ID".into(), bad.into());
        assert!(!valid(&v, true));
    }
}
#[test]
fn encryption_key_rotation_is_bounded_unique_nonzero_and_closed() {
    for raw in [
        serde_json::json!([]),
        serde_json::json!([{"id":"zero","secret_hex":"00".repeat(32)}]),
        serde_json::json!([{"id":"a","secret_hex":"21".repeat(32)},{"id":"a","secret_hex":"31".repeat(32)}]),
        serde_json::json!([{"id":"a","secret_hex":"21".repeat(32)},{"id":"b","secret_hex":"21".repeat(32)}]),
        serde_json::json!([{"id":"a","secret_hex":"21".repeat(32),"callback":"untrusted"}]),
        serde_json::json!((0..5).map(|i|serde_json::json!({"id":format!("key{i}"),"secret_hex":format!("{:02x}",i+1).repeat(32)})).collect::<Vec<_>>()),
    ] {let mut v=values();v.insert("CONCERT_PARTICIPANT_RECOVERY_KEYS_JSON".into(),raw.to_string());assert!(!valid(&v,true));}
    let mut v = values();
    v.insert(
        "CONCERT_PARTICIPANT_RECOVERY_KEYS_JSON".into(),
        "x".repeat(2049),
    );
    assert!(!valid(&v, true));
}
#[test]
fn workload_and_rotation_credentials_cannot_collapse_authorities() {
    for key in [
        "AUTH_SERVICE_CREDENTIAL",
        "FRIEND_SERVICE_CREDENTIAL",
        "PROFILE_SERVICE_CREDENTIAL",
        "CONCERT_SERVICE_CREDENTIAL",
        "AUTH_SERVICE_CREDENTIAL_PREVIOUS",
    ] {
        let mut v = values();
        v.insert(key.into(), v["GATEWAY_SERVICE_CREDENTIAL"].clone());
        assert!(!valid(&v, true));
    }
    let mut v = values();
    v.insert(
        "AUTH_SERVICE_CREDENTIAL_PREVIOUS".into(),
        "configured-old-auth".into(),
    );
    assert!(valid(&v, true));
}
#[test]
fn failures_do_not_echo_endpoint_or_key_or_credential_values() {
    let mut v = values();
    v.insert(
        "CONCERT_PARTICIPANT_AUTH_GRPC_ENDPOINT".into(),
        "https://PRIVATE_CANARY:password@host:443".into(),
    );
    let error = settings(|name| Ok(v.get(name).cloned()), true)
        .err()
        .unwrap()
        .to_string();
    assert!(!error.contains("PRIVATE_CANARY"));
    assert!(!error.contains("password"));
}
