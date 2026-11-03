use chrono::{TimeZone, Utc};
use platform::model_manifest::{ModelManifest, ModelStatus};
use platform::recommendation_readiness::{
    DataReadinessSnapshot, ModelActivationGate, ReadinessThresholds, ShadowReadinessSnapshot,
};

fn manifest() -> ModelManifest {
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

fn data_ready() -> DataReadinessSnapshot {
    DataReadinessSnapshot {
        observation_days: 35,
        impressions: 250_000,
        positive_outcomes: 25_000,
        negative_outcomes: 2_500,
        distinct_viewers: 30_000,
        source_coverage_basis_points: 9_800,
        position_coverage_basis_points: 9_900,
        event_loss_basis_points: 2,
        snapshot_replay_basis_points: 9_950,
    }
}

fn shadow_ready() -> ShadowReadinessSnapshot {
    ShadowReadinessSnapshot {
        requests: 50_000,
        error_basis_points: 20,
        p95_latency_ms: 35,
        policy_difference_basis_points: 0,
        top_k_overlap_basis_points: 7_500,
    }
}

#[test]
fn activation_gate_accepts_valid_manifest_data_and_shadow_evidence() {
    let report = ModelActivationGate::evaluate(
        &manifest(),
        true,
        true,
        &data_ready(),
        &shadow_ready(),
        &ReadinessThresholds::default(),
    );

    assert!(report.ready);
    assert!(report.blockers.is_empty());
}

#[test]
fn activation_gate_reports_each_missing_data_contract() {
    let data = DataReadinessSnapshot {
        observation_days: 3,
        impressions: 100,
        positive_outcomes: 1,
        negative_outcomes: 0,
        distinct_viewers: 10,
        source_coverage_basis_points: 5_000,
        position_coverage_basis_points: 5_000,
        event_loss_basis_points: 500,
        snapshot_replay_basis_points: 5_000,
    };

    let report = ModelActivationGate::evaluate(
        &manifest(),
        true,
        true,
        &data,
        &shadow_ready(),
        &ReadinessThresholds::default(),
    );

    assert!(!report.ready);
    for expected in [
        "OBSERVATION_WINDOW",
        "IMPRESSION_VOLUME",
        "POSITIVE_OUTCOME_VOLUME",
        "NEGATIVE_OUTCOME_VOLUME",
        "VIEWER_COVERAGE",
        "SOURCE_COVERAGE",
        "POSITION_COVERAGE",
        "EVENT_LOSS",
        "SNAPSHOT_REPLAY",
    ] {
        assert!(report.blockers.iter().any(|value| value == expected));
    }
}

#[test]
fn activation_gate_rejects_unhealthy_shadow_or_missing_fallback() {
    let shadow = ShadowReadinessSnapshot {
        requests: 10,
        error_basis_points: 2_000,
        p95_latency_ms: 500,
        policy_difference_basis_points: 100,
        top_k_overlap_basis_points: 500,
    };

    let report = ModelActivationGate::evaluate(
        &manifest(),
        true,
        false,
        &data_ready(),
        &shadow,
        &ReadinessThresholds::default(),
    );

    assert!(!report.ready);
    for expected in [
        "FALLBACK_NOT_READY",
        "SHADOW_REQUEST_VOLUME",
        "SHADOW_ERROR_RATE",
        "SHADOW_LATENCY",
        "SHADOW_POLICY_DIFFERENCE",
        "SHADOW_TOP_K_OVERLAP",
    ] {
        assert!(report.blockers.iter().any(|value| value == expected));
    }
}
