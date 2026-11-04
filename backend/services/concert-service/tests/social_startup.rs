//! Security-sensitive opt-in parsing only; no positive Auth or sharing proof.
#[path = "canonical_owner_composition_support/environment.rs"]
mod environment;
use concert_service::config::social_startup::SocialStartupSettings;
use platform::{grpc_auth::Workload, grpc_tls::TlsMaterial};
use std::collections::BTreeMap;
fn values() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("CONCERT_SOCIAL_MODE".into(), "live".into()),
        ("CONCERT_SOCIAL_CURSOR_KEY_HEX".into(), "11".repeat(32)),
        (
            "CONCERT_SOCIAL_HANDLE_KEYS_JSON".into(),
            format!(
                r#"[{{"id":"handle-1","secret_hex":"{}"}}]"#,
                "22".repeat(32)
            ),
        ),
        (
            "CONCERT_SOCIAL_COMMAND_KEYS_JSON".into(),
            format!(
                r#"[{{"id":"command-1","secret_hex":"{}"}}]"#,
                "33".repeat(32)
            ),
        ),
        ("CONCERT_SOCIAL_REQUEST_TTL_SECONDS".into(), "3600".into()),
        (
            "CONCERT_NOTIFICATION_GRPC_ENDPOINT".into(),
            "https://notification.internal:8443".into(),
        ),
        (
            "CONCERT_SERVICE_CREDENTIAL".into(),
            "social-concert-current".into(),
        ),
    ])
}
fn parse(
    v: &BTreeMap<String, String>,
    tls: &TlsMaterial,
) -> anyhow::Result<Option<SocialStartupSettings>> {
    SocialStartupSettings::from_values(|n| Ok(v.get(n).cloned()), true, true, true, Some(tls))
}
#[test]
fn off_never_reads_private_configuration() -> anyhow::Result<()> {
    let mut reads = 0;
    assert!(SocialStartupSettings::from_values(
        |name| {
            reads += 1;
            assert_eq!(name, "CONCERT_SOCIAL_MODE");
            Ok(None)
        },
        false,
        false,
        false,
        None
    )?
    .is_none());
    assert_eq!(reads, 1);
    assert!(SocialStartupSettings::from_values(
        |_| Ok(Some("synthetic".into())),
        true,
        true,
        true,
        None
    )
    .is_err());
    Ok(())
}
#[tokio::test]
async fn live_requires_real_composition_and_tls() -> anyhow::Result<()> {
    let tls = environment::tls(Workload::Concert)?;
    let v = values();
    assert!(parse(&v, &tls)?.is_some());
    for (shared, world, notification) in [
        (false, true, true),
        (true, false, true),
        (true, true, false),
    ] {
        assert!(SocialStartupSettings::from_values(
            |n| Ok(v.get(n).cloned()),
            shared,
            world,
            notification,
            Some(&tls)
        )
        .is_err());
    }
    assert!(
        SocialStartupSettings::from_values(|n| Ok(v.get(n).cloned()), true, true, true, None)
            .is_err()
    );
    let bad = TlsMaterial::from_pem(b"bad".to_vec(), b"bad".to_vec(), b"bad".to_vec());
    assert!(parse(&v, &bad).is_err());
    Ok(())
}
#[tokio::test]
async fn keys_rotation_trust_and_ttl_cannot_silently_degrade() -> anyhow::Result<()> {
    let tls = environment::tls(Workload::Concert)?;
    let original = values();
    for name in [
        "CONCERT_SOCIAL_CURSOR_KEY_HEX",
        "CONCERT_SOCIAL_HANDLE_KEYS_JSON",
        "CONCERT_SOCIAL_COMMAND_KEYS_JSON",
        "CONCERT_SOCIAL_REQUEST_TTL_SECONDS",
        "CONCERT_NOTIFICATION_GRPC_ENDPOINT",
        "CONCERT_SERVICE_CREDENTIAL",
    ] {
        let mut v = original.clone();
        v.remove(name);
        assert!(parse(&v, &tls).is_err(), "{name}");
    }
    for (name, value) in [
        ("CONCERT_SOCIAL_CURSOR_KEY_HEX", "00".repeat(32)),
        ("CONCERT_SOCIAL_CURSOR_KEY_HEX", "22".repeat(32)),
        (
            "CONCERT_SOCIAL_COMMAND_KEYS_JSON",
            format!(r#"[{{"id":"c","secret_hex":"{}"}}]"#, "22".repeat(32)),
        ),
        (
            "CONCERT_SOCIAL_HANDLE_KEYS_JSON",
            format!(
                r#"[{{"id":"h","secret_hex":"{}","extra":true}}]"#,
                "22".repeat(32)
            ),
        ),
        (
            "CONCERT_SOCIAL_HANDLE_KEYS_JSON",
            format!(
                r#"[{{"id":"h","secret_hex":"{}"}},{{"id":"h","secret_hex":"{}"}}]"#,
                "22".repeat(32),
                "44".repeat(32)
            ),
        ),
        ("CONCERT_SOCIAL_REQUEST_TTL_SECONDS", "0".into()),
        ("CONCERT_SOCIAL_REQUEST_TTL_SECONDS", "86401".into()),
        ("CONCERT_SOCIAL_REQUEST_TTL_SECONDS", "03600".into()),
        ("CONCERT_SOCIAL_ACCEPTED_CLAIM_ISSUER", "unpaired".into()),
        (
            "CONCERT_SERVICE_CREDENTIAL",
            "local-parity-internal-service-token".into(),
        ),
    ] {
        let mut v = original.clone();
        v.insert(name.into(), value);
        assert!(parse(&v, &tls).is_err(), "{name}");
    }
    let mut trusted = original.clone();
    trusted.insert(
        "CONCERT_SOCIAL_ACCEPTED_CLAIM_ISSUER".into(),
        "accepted-issuer".into(),
    );
    trusted.insert(
        "CONCERT_SOCIAL_ACCEPTED_CLAIM_REVISION".into(),
        "revision-1".into(),
    );
    assert!(parse(&trusted, &tls)?.is_some());
    trusted.insert(
        "CONCERT_SOCIAL_ACCEPTED_CLAIM_REVISION".into(),
        "rev/1".into(),
    );
    assert!(parse(&trusted, &tls).is_err());
    Ok(())
}
#[tokio::test]
async fn delivery_endpoint_cannot_be_retargeted_by_url_components() -> anyhow::Result<()> {
    let tls = environment::tls(Workload::Concert)?;
    for endpoint in [
        "http://notification.internal:8443",
        "https://notification.internal",
        "https://user:secret@notification.internal:8443",
        "https://notification.internal:8443/path",
        "https://notification.internal:8443/?x=1",
        "https://notification.internal:8443/#private",
    ] {
        let mut v = values();
        v.insert("CONCERT_NOTIFICATION_GRPC_ENDPOINT".into(), endpoint.into());
        assert!(parse(&v, &tls).is_err());
    }
    Ok(())
}
