//! Uploader-media composition settings only; no canonical owner or HTTP is started.
use concert_service::config::owner_media_runtime::OwnerMediaSettings;
use std::collections::BTreeMap;

fn values() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("CONCERT_OWNER_MEDIA_MODE".into(), "uploader".into()),
        ("CONCERT_MEDIA_SANITIZER_WORKERS".into(), "2".into()),
        ("CONCERT_MEDIA_SANITIZER_QUEUE".into(), "8".into()),
        (
            "CONCERT_MEDIA_RECEIPT_KEYS_JSON".into(),
            r#"[{"id":"media-current","secret_hex":"0909090909090909090909090909090909090909090909090909090909090909"}]"#
                .into(),
        ),
    ])
}

fn parse(v: &BTreeMap<String, String>, shared: bool) -> anyhow::Result<Option<OwnerMediaSettings>> {
    OwnerMediaSettings::from_values(
        |key| Ok(v.get(key).cloned()),
        true,
        shared,
    )
}

#[test]
fn disabled_mode_reads_no_other_configuration() {
    let mut v = values();
    v.insert("CONCERT_OWNER_MEDIA_MODE".into(), "off".into());
    v.remove("CONCERT_MEDIA_RECEIPT_KEYS_JSON");
    assert!(parse(&v, false).unwrap().is_none());
}

#[test]
fn uploader_mode_requires_the_shared_canonical_composition() {
    assert!(parse(&values(), false).is_err());
    assert!(parse(&values(), true).is_ok());
}

#[test]
fn uploader_mode_requires_explicit_bounded_sanitizer_and_receipt_keys() {
    for key in [
        "CONCERT_MEDIA_SANITIZER_WORKERS",
        "CONCERT_MEDIA_SANITIZER_QUEUE",
        "CONCERT_MEDIA_RECEIPT_KEYS_JSON",
    ] {
        let mut v = values();
        v.remove(key);
        assert!(parse(&v, true).is_err(), "accepted missing {key}");
    }
    for (workers, queue) in [("0", "8"), ("65", "8"), ("2", "1025"), ("x", "8")] {
        let mut v = values();
        v.insert("CONCERT_MEDIA_SANITIZER_WORKERS".into(), workers.into());
        v.insert("CONCERT_MEDIA_SANITIZER_QUEUE".into(), queue.into());
        assert!(parse(&v, true).is_err(), "accepted bounds {workers}/{queue}");
    }
}

#[test]
fn uploader_mode_rejects_ambient_or_duplicate_receipt_secrets() {
    let mut v = values();
    v.insert(
        "CONCERT_MEDIA_RECEIPT_KEYS_JSON".into(),
        r#"[{"id":"a","secret_hex":"0909090909090909090909090909090909090909090909090909090909090909"},
            {"id":"b","secret_hex":"0909090909090909090909090909090909090909090909090909090909090909"}]"#
            .into(),
    );
    assert!(parse(&v, true).is_err());
    let mut v = values();
    v.insert(
        "CONCERT_MEDIA_RECEIPT_KEYS_JSON".into(),
        r#"[{"id":"a","secret_hex":"0000000000000000000000000000000000000000000000000000000000000000"}]"#
            .into(),
    );
    assert!(parse(&v, true).is_err());
    let mut v = values();
    v.insert("CONCERT_OWNER_MEDIA_MODE".into(), "live".into());
    assert!(parse(&v, true).is_err());
}
