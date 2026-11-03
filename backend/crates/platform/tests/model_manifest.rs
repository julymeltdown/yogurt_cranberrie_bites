use chrono::{TimeZone, Utc};
use platform::model_manifest::{ModelManifest, ModelStatus};

fn valid_manifest() -> ModelManifest {
    ModelManifest {
        model_name: "home-feed-ranker".into(),
        model_version: "feed-learned-v1".into(),
        feature_schema_version: "feed-features-v1".into(),
        artifact_uri: "s3://models/feed-learned-v1/model.onnx".into(),
        sha256: "a".repeat(64),
        embedding_dimension: Some(128),
        activated_at: Some(Utc.with_ymd_and_hms(2026, 8, 4, 0, 0, 0).unwrap()),
        fallback_version: "feed-linear-v1".into(),
        status: ModelStatus::Ready,
    }
}

#[test]
fn production_manifest_accepts_versioned_signed_artifact_contract() {
    valid_manifest()
        .validate(true)
        .expect("valid production manifest");
}

#[test]
fn manifest_rejects_invalid_digest_dimension_and_fallback_cycle() {
    let mut invalid_digest = valid_manifest();
    invalid_digest.sha256 = "ABC".into();
    assert!(invalid_digest.validate(false).is_err());

    let mut invalid_dimension = valid_manifest();
    invalid_dimension.embedding_dimension = Some(4096);
    assert!(invalid_dimension.validate(false).is_err());

    let mut fallback_cycle = valid_manifest();
    fallback_cycle.fallback_version = fallback_cycle.model_version.clone();
    assert!(fallback_cycle.validate(false).is_err());
}

#[test]
fn manifest_rejects_oversized_whitespace_text_fields() {
    let mut manifest = valid_manifest();
    manifest.model_name = " ".repeat(129);
    assert!(manifest.validate(false).is_err());

    let mut manifest = valid_manifest();
    manifest.artifact_uri = " ".repeat(1025);
    assert!(manifest.validate(false).is_err());
}

#[test]
fn production_manifest_rejects_local_artifact_paths() {
    let mut manifest = valid_manifest();
    manifest.artifact_uri = "/tmp/feed-model.onnx".into();

    assert!(manifest.validate(true).is_err());
}

#[test]
fn revoked_and_retired_models_never_serve() {
    assert_eq!(ModelStatus::parse("REVOKED").unwrap(), ModelStatus::Revoked);
    assert!(!ModelStatus::Revoked.can_serve());
    assert!(!ModelStatus::Retired.can_serve());
    assert!(ModelStatus::Ready.can_serve());
    assert!(ModelStatus::Active.can_serve());
}

#[test]
fn revoked_models_are_never_servable() {
    let mut manifest = valid_manifest();
    manifest.status = ModelStatus::Revoked;

    assert!(!manifest.status.can_serve());
    assert_eq!(ModelStatus::parse("revoked").unwrap(), ModelStatus::Revoked);
}

#[test]
fn revoked_model_status_is_parseable_but_never_serves() {
    let status = ModelStatus::parse("REVOKED").expect("revoked status");

    assert_eq!(status.as_str(), "REVOKED");
    assert!(!status.can_serve());
}
