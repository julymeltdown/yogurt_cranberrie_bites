//! Startup/security configuration only; no live owner or HTTP qualification.
#[path = "canonical_owner_composition_support/environment.rs"]
mod environment;
use concert_service::config::product_runtime::feed::{FeedReadKeys, FeedRuntimeSettings};
use platform::{
    actor_delegation::{
        concert_read::{
            ConcertPostReadSigner, PostConcertRootReadSigner, PostConcertRootReadVerifier,
        },
        ActorDelegationSigner, ActorDelegationVerifier,
    },
    grpc_auth::Workload,
    grpc_tls::TlsMaterial,
};
use std::collections::BTreeMap;
fn gateway() -> ActorDelegationVerifier {
    let signer = ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32]).unwrap();
    ActorDelegationVerifier::new(Workload::Concert, &[signer.public_key()]).unwrap()
}
fn keys(g: &ActorDelegationVerifier, callback_seed: u8) -> FeedReadKeys {
    let callback = PostConcertRootReadSigner::from_seed(&[callback_seed; 32], g).unwrap();
    FeedReadKeys::new(
        ConcertPostReadSigner::from_seed(&[76; 32], g).unwrap(),
        PostConcertRootReadVerifier::new(&[callback.public_key()], g).unwrap(),
    )
}
fn values() -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "CONCERT_POST_GRPC_ENDPOINT".into(),
            "https://post.internal:8443".into(),
        ),
        (
            "CONCERT_SERVICE_CREDENTIAL".into(),
            "feed-concert-current".into(),
        ),
        ("POST_SERVICE_CREDENTIAL".into(), "feed-post-current".into()),
        (
            "POST_SERVICE_CREDENTIAL_PREVIOUS".into(),
            "feed-post-previous".into(),
        ),
        ("CONCERT_FEED_CURSOR_KEY_HEX".into(), "73".repeat(32)),
    ])
}
fn parse(
    v: &BTreeMap<String, String>,
    tls: &TlsMaterial,
) -> anyhow::Result<Option<FeedRuntimeSettings>> {
    let g = gateway();
    FeedRuntimeSettings::from_values(
        |name| Ok(v.get(name).cloned()),
        true,
        true,
        Some(tls),
        Some(&g),
        Some(keys(&g, 77)),
    )
}
#[test]
fn off_does_not_inspect_unrelated_secrets_or_construct_owners() -> anyhow::Result<()> {
    assert!(FeedRuntimeSettings::from_values(
        |_| anyhow::bail!("unexpected off-mode lookup"),
        false,
        false,
        None,
        None,
        None
    )?
    .is_none());
    Ok(())
}
#[test]
fn read_requires_shared_owners_tls_and_dedicated_keys() -> anyhow::Result<()> {
    let tls = environment::tls(Workload::Concert)?;
    let g = gateway();
    let v = values();
    assert!(parse(&v, &tls)?.is_some());
    assert!(FeedRuntimeSettings::from_values(
        |n| Ok(v.get(n).cloned()),
        true,
        false,
        Some(&tls),
        Some(&g),
        Some(keys(&g, 77))
    )
    .is_err());
    assert!(FeedRuntimeSettings::from_values(
        |n| Ok(v.get(n).cloned()),
        true,
        true,
        None,
        Some(&g),
        Some(keys(&g, 77))
    )
    .is_err());
    assert!(FeedRuntimeSettings::from_values(
        |n| Ok(v.get(n).cloned()),
        true,
        true,
        Some(&tls),
        None,
        Some(keys(&g, 77))
    )
    .is_err());
    assert!(FeedRuntimeSettings::from_values(
        |n| Ok(v.get(n).cloned()),
        true,
        true,
        Some(&tls),
        Some(&g),
        None
    )
    .is_err());
    let bad = TlsMaterial::from_pem(b"bad".to_vec(), b"bad".to_vec(), b"bad".to_vec());
    assert!(parse(&v, &bad).is_err());
    Ok(())
}
#[test]
fn missing_private_inputs_and_retargetable_endpoints_are_refused() -> anyhow::Result<()> {
    let tls = environment::tls(Workload::Concert)?;
    for name in [
        "CONCERT_POST_GRPC_ENDPOINT",
        "CONCERT_SERVICE_CREDENTIAL",
        "POST_SERVICE_CREDENTIAL",
        "CONCERT_FEED_CURSOR_KEY_HEX",
    ] {
        let mut v = values();
        v.remove(name);
        assert!(parse(&v, &tls).is_err(), "{name}");
    }
    for endpoint in [
        "http://post.internal:8443",
        "https://user:password@post.internal:8443",
        "https://post.internal:8443/read",
        "https://post.internal:8443/?override=1",
        "https://post.internal:8443/#fragment",
    ] {
        let mut v = values();
        v.insert("CONCERT_POST_GRPC_ENDPOINT".into(), endpoint.into());
        assert!(parse(&v, &tls).is_err());
    }
    Ok(())
}
#[test]
fn secrets_and_workload_rotation_cannot_collapse_identities() -> anyhow::Result<()> {
    let tls = environment::tls(Workload::Concert)?;
    for (name, value) in [
        ("CONCERT_FEED_CURSOR_KEY_HEX", "00".repeat(32)),
        ("CONCERT_FEED_CURSOR_KEY_HEX", "4c".repeat(32)),
        ("POST_SERVICE_CREDENTIAL", "feed-concert-current".into()),
        (
            "POST_SERVICE_CREDENTIAL_PREVIOUS",
            "feed-post-current".into(),
        ),
        (
            "POST_SERVICE_CREDENTIAL_PREVIOUS",
            "feed-concert-current".into(),
        ),
        (
            "POST_SERVICE_CREDENTIAL",
            "local-parity-internal-service-token".into(),
        ),
    ] {
        let mut v = values();
        v.insert(name.into(), value);
        assert!(parse(&v, &tls).is_err(), "{name}");
    }
    let mut reused = values();
    reused.insert(
        "GATEWAY_SERVICE_CREDENTIAL_PREVIOUS".into(),
        "feed-post-current".into(),
    );
    assert!(parse(&reused, &tls).is_err());
    let g = gateway();
    let v = values();
    assert!(FeedRuntimeSettings::from_values(
        |n| Ok(v.get(n).cloned()),
        true,
        true,
        Some(&tls),
        Some(&g),
        Some(keys(&g, 76))
    )
    .is_err());
    Ok(())
}
