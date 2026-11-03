use chrono::{TimeZone, Utc};
use platform::recommendation::MAX_RECOMMENDATION_CURSOR_LENGTH;
use post_service::domain::recommendation::{
    diversity_rerank, FeedCandidate, FeedCandidateSource, FeedRankWeights, FeedSnapshotCursor,
    RolloutMode,
};
use proptest::prelude::*;
use uuid::Uuid;

const POST_ID: Uuid = Uuid::from_u128(0x22222222222222222222222222222222);
const AUTHOR_ID: Uuid = Uuid::from_u128(0x33333333333333333333333333333333);

fn candidate(post_id: Uuid, author_id: Uuid, score: f64, exploration: bool) -> FeedCandidate {
    FeedCandidate {
        post_id,
        author_id,
        source: FeedCandidateSource::MaterializedTimeline,
        source_score: score,
        author_affinity: 0.7,
        freshness: 0.9,
        social_proof: 0.5,
        engagement_velocity: 0.4,
        topic_match: 0.2,
        repeated_impression_penalty: 0.1,
        author_saturation_penalty: 0.0,
        negative_feedback_penalty: 0.0,
        generated_at: Utc.with_ymd_and_hms(2026, 8, 4, 1, 2, 3).unwrap(),
        reason_code: "FOLLOWING_RECENT".into(),
        in_network: true,
        exploration,
    }
}

#[test]
fn feed_cursor_round_trips_snapshot_and_offset() {
    let cursor = FeedSnapshotCursor {
        snapshot_id: Uuid::new_v4(),
        offset: 100,
    };
    let encoded = cursor.encode();
    assert!(!encoded.contains(&cursor.snapshot_id.to_string()));
    assert_eq!(FeedSnapshotCursor::parse(&encoded).unwrap(), Some(cursor));
    assert!(FeedSnapshotCursor::parse("invalid").is_err());
    assert!(FeedSnapshotCursor::parse(&"x".repeat(MAX_RECOMMENDATION_CURSOR_LENGTH + 1)).is_err());
    let padded = format!(
        "{}{}",
        " ".repeat(MAX_RECOMMENDATION_CURSOR_LENGTH + 1),
        encoded
    );
    assert!(FeedSnapshotCursor::parse(&padded).is_err());
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(proptest::test_runner::FileFailurePersistence::WithSource("proptest-regressions"))),
        ..ProptestConfig::default()
    })]

    #[test]
    fn feed_snapshot_cursor_parser_is_bounded_and_canonical(input in prop::collection::vec(any::<u8>(), 0..=2048)) {
        let candidate = String::from_utf8_lossy(&input);
        let parsed = FeedSnapshotCursor::parse(&candidate);
        if candidate.len() > MAX_RECOMMENDATION_CURSOR_LENGTH {
            prop_assert!(parsed.is_err());
        }
        if let Ok(Some(cursor)) = parsed {
            let replayed = FeedSnapshotCursor::parse(&cursor.encode()).expect("encoded cursor remains valid");
            prop_assert_eq!(replayed, Some(cursor));
        }
    }
}

#[test]
fn feed_ranker_rewards_fresh_affinity_and_penalizes_negative_feedback() {
    let weights = FeedRankWeights::default();
    let positive_candidate = candidate(POST_ID, AUTHOR_ID, 0.8, false);
    let positive = weights.score(&positive_candidate).expect("finite score");
    let negative = weights
        .score(&FeedCandidate {
            negative_feedback_penalty: 1.0,
            ..positive_candidate.clone()
        })
        .expect("finite score");
    assert!(positive > negative);
    assert_eq!(weights.version, "feed-linear-v1");
    assert!(positive_candidate.components().contains_key("freshness"));
}

#[test]
fn diversity_reranker_caps_authors_and_exploration() {
    let author_a = AUTHOR_ID;
    let author_b = Uuid::new_v4();
    let rows = vec![
        (candidate(Uuid::new_v4(), author_a, 1.0, false), 100),
        (candidate(Uuid::new_v4(), author_a, 0.9, false), 90),
        (candidate(Uuid::new_v4(), author_a, 0.8, false), 80),
        (candidate(Uuid::new_v4(), author_b, 0.7, false), 70),
        (candidate(Uuid::new_v4(), Uuid::new_v4(), 0.6, true), 60),
        (candidate(Uuid::new_v4(), Uuid::new_v4(), 0.5, true), 50),
    ];

    let selected = diversity_rerank(rows, 5, 2, 1, 1);
    assert_eq!(selected.len(), 4);
    assert!(selected
        .windows(2)
        .all(|window| window[0].0.author_id != window[1].0.author_id));
    assert_eq!(selected.iter().filter(|item| item.0.exploration).count(), 1);
}

#[test]
fn rollout_and_source_names_are_stable() {
    assert_eq!(
        FeedCandidateSource::FreshTrending.as_str(),
        "FRESH_TRENDING"
    );
    assert_eq!(RolloutMode::parse("canary").unwrap(), RolloutMode::Canary);
}

#[test]
fn stable_experiment_bucket_is_deterministic_and_bounded() {
    use post_service::domain::recommendation::stable_experiment_bucket;
    let viewer = Uuid::from_u128(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
    let left = stable_experiment_bucket(viewer, "feed-ranker-v2", 10_000);
    let right = stable_experiment_bucket(viewer, "feed-ranker-v2", 10_000);
    assert_eq!(left, right);
    assert!(left < 10_000);
    assert_eq!(stable_experiment_bucket(viewer, "feed-ranker-v2", 0), 0);
}

#[test]
fn snapshot_page_cursor_uses_snapshot_ownership_and_offset() {
    let snapshot_id = Uuid::new_v4();
    let cursor = FeedSnapshotCursor {
        snapshot_id,
        offset: 50,
    };
    let decoded = FeedSnapshotCursor::parse(&cursor.encode())
        .expect("valid cursor")
        .expect("cursor");
    assert_eq!(decoded.snapshot_id, snapshot_id);
    assert_eq!(decoded.offset, 50);
}
