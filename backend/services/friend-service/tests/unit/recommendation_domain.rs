use friend_service::domain::recommendation::{
    PeopleCandidateFeatures, PeopleCandidateSource, PeopleRankWeights, PeopleSnapshotCursor,
    RolloutMode,
};
use platform::recommendation::MAX_RECOMMENDATION_CURSOR_LENGTH;
use proptest::prelude::*;
use uuid::Uuid;

#[test]
fn people_snapshot_cursor_is_opaque_versioned_and_bounded() {
    let cursor = PeopleSnapshotCursor {
        snapshot_id: Uuid::new_v4(),
        offset: 50,
    };
    let encoded = cursor.encode();
    assert!(!encoded.contains(&cursor.snapshot_id.to_string()));
    assert_eq!(PeopleSnapshotCursor::parse(&encoded).unwrap(), Some(cursor));
    assert_eq!(PeopleSnapshotCursor::parse("  ").unwrap(), None);
    assert!(PeopleSnapshotCursor::parse("not-base64").is_err());
    assert!(
        PeopleSnapshotCursor::parse(&"a".repeat(MAX_RECOMMENDATION_CURSOR_LENGTH + 1)).is_err()
    );
    let padded = format!(
        "{}{}",
        " ".repeat(MAX_RECOMMENDATION_CURSOR_LENGTH + 1),
        encoded
    );
    assert!(PeopleSnapshotCursor::parse(&padded).is_err());
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(proptest::test_runner::FileFailurePersistence::WithSource("proptest-regressions"))),
        ..ProptestConfig::default()
    })]

    #[test]
    fn people_snapshot_cursor_parser_is_bounded_and_canonical(input in prop::collection::vec(any::<u8>(), 0..=2048)) {
        let candidate = String::from_utf8_lossy(&input);
        let parsed = PeopleSnapshotCursor::parse(&candidate);
        if candidate.len() > MAX_RECOMMENDATION_CURSOR_LENGTH {
            prop_assert!(parsed.is_err());
        }
        if let Ok(Some(cursor)) = parsed {
            let replayed = PeopleSnapshotCursor::parse(&cursor.encode()).expect("encoded cursor remains valid");
            prop_assert_eq!(replayed, Some(cursor));
        }
    }
}

#[test]
fn people_ranker_is_explainable_finite_and_versioned() {
    let weights = PeopleRankWeights::default();
    let features = PeopleCandidateFeatures {
        mutual_count: 4,
        mutual_score: 0.8,
        adamic_adar: 0.6,
        jaccard: 0.4,
        profile_overlap: 0.2,
        interaction_affinity: 0.7,
        freshness: 0.9,
        candidate_quality: 0.8,
        impression_penalty: 0.1,
        dismissal_penalty: 0.0,
    };
    let score = weights.score(&features).expect("finite score");
    assert!(score > 0.0);
    assert_eq!(weights.version, "people-linear-v1");
    assert!(weights
        .score(&PeopleCandidateFeatures {
            mutual_score: f64::NAN,
            ..features
        })
        .is_err());
    assert!(features.components().contains_key("adamicAdar"));
}

#[test]
fn recommendation_enums_have_stable_contract_names() {
    assert_eq!(
        PeopleCandidateSource::MutualFollow.as_str(),
        "MUTUAL_FOLLOW"
    );
    assert_eq!(RolloutMode::parse("shadow").unwrap(), RolloutMode::Shadow);
    assert!(RolloutMode::parse("unknown").is_err());
}
