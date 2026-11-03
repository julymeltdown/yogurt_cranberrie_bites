use chrono::{TimeZone, Utc};
use platform::recommendation::{
    RecommendationAction, RecommendationCursor, RecommendationEvent, RecommendationSubjectType,
    RecommendationSurface, RolloutMode, MAX_RECOMMENDATION_CURSOR_LENGTH,
};
use proptest::prelude::*;
use uuid::Uuid;

fn uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).expect("uuid")
}

#[test]
fn recommendation_cursor_round_trips_canonical_rank_state() {
    let cursor = RecommendationCursor {
        version: 1,
        snapshot_id: uuid("11111111-1111-1111-1111-111111111111"),
        score_micros: 875_000,
        occurred_at: Utc
            .with_ymd_and_hms(2026, 8, 4, 12, 30, 0)
            .single()
            .expect("timestamp"),
        tie_breaker_id: uuid("22222222-2222-2222-2222-222222222222"),
    };

    let encoded = cursor.encode();
    let decoded = RecommendationCursor::parse(&encoded).expect("valid cursor");

    assert_eq!(decoded, cursor);
    assert!(!encoded.contains('|'));
}

#[test]
fn recommendation_cursor_rejects_malformed_oversized_and_unknown_versions() {
    assert!(RecommendationCursor::parse("not-base64").is_err());
    assert!(RecommendationCursor::parse(&"x".repeat(513)).is_err());

    let cursor = RecommendationCursor {
        version: 2,
        snapshot_id: Uuid::new_v4(),
        score_micros: 1,
        occurred_at: Utc::now(),
        tie_breaker_id: Uuid::new_v4(),
    };
    assert!(RecommendationCursor::parse(&cursor.encode()).is_err());

    let valid = RecommendationCursor {
        version: 1,
        snapshot_id: uuid("66666666-6666-6666-6666-666666666666"),
        score_micros: 1,
        occurred_at: Utc.timestamp_millis_opt(1).single().unwrap(),
        tie_breaker_id: uuid("77777777-7777-7777-7777-777777777777"),
    }
    .encode();
    let padded = format!(
        "{}{}",
        " ".repeat(MAX_RECOMMENDATION_CURSOR_LENGTH + 1),
        valid
    );
    assert!(RecommendationCursor::parse(&padded).is_err());
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(proptest::test_runner::FileFailurePersistence::WithSource("proptest-regressions"))),
        ..ProptestConfig::default()
    })]

    #[test]
    fn recommendation_cursor_parser_is_bounded_and_canonical(input in prop::collection::vec(any::<u8>(), 0..=2048)) {
        let candidate = String::from_utf8_lossy(&input);
        let parsed = RecommendationCursor::parse(&candidate);
        if candidate.len() > MAX_RECOMMENDATION_CURSOR_LENGTH {
            prop_assert!(parsed.is_err());
        }
        if let Ok(cursor) = parsed {
            let replayed = RecommendationCursor::parse(&cursor.encode()).expect("encoded cursor remains valid");
            prop_assert_eq!(replayed, cursor);
        }
    }
}

#[test]
fn impression_event_requires_position_source_model_and_matching_impression_id() {
    let event_id = uuid("33333333-3333-3333-3333-333333333333");
    let event = RecommendationEvent {
        event_id,
        schema_version: 1,
        occurred_at: Utc::now(),
        viewer_id: uuid("44444444-4444-4444-4444-444444444444"),
        subject_type: RecommendationSubjectType::User,
        subject_id: uuid("55555555-5555-5555-5555-555555555555"),
        actor_id: None,
        surface: RecommendationSurface::PeopleRecommendation,
        position: Some(0),
        request_id: Some("request-1".into()),
        session_id: None,
        action: RecommendationAction::Impression,
        source: "MUTUAL_FOLLOW".into(),
        model_version: "people-linear-v1".into(),
        impression_id: Some(event_id),
        propensity_micros: Some(500_000),
        dwell_ms: None,
        metadata_json: "{}".into(),
    };

    event.validate().expect("valid impression");

    let mut invalid = event.clone();
    invalid.position = None;
    assert_eq!(
        invalid.validate().unwrap_err().to_string(),
        "Recommendation impression position is required"
    );

    let mut invalid = event.clone();
    invalid.source.clear();
    assert_eq!(
        invalid.validate().unwrap_err().to_string(),
        "Recommendation source is required"
    );

    let mut invalid_request_id = event.clone();
    invalid_request_id.request_id = Some(" ".repeat(129));
    assert_eq!(
        invalid_request_id.validate().unwrap_err().to_string(),
        "Recommendation request ID is invalid"
    );

    let mut invalid = event;
    invalid.impression_id = Some(Uuid::new_v4());
    assert_eq!(
        invalid.validate().unwrap_err().to_string(),
        "Impression ID must equal event ID for impression events"
    );
}

#[test]
fn outcome_event_requires_existing_impression_and_bounded_metadata() {
    let event = RecommendationEvent {
        event_id: Uuid::new_v4(),
        schema_version: 1,
        occurred_at: Utc::now(),
        viewer_id: Uuid::new_v4(),
        subject_type: RecommendationSubjectType::Post,
        subject_id: Uuid::new_v4(),
        actor_id: None,
        surface: RecommendationSurface::HomeFeed,
        position: None,
        request_id: Some("request-2".into()),
        session_id: None,
        action: RecommendationAction::Hide,
        source: "FRESH_TRENDING".into(),
        model_version: "feed-linear-v1".into(),
        impression_id: None,
        propensity_micros: None,
        dwell_ms: None,
        metadata_json: "{}".into(),
    };

    assert_eq!(
        event.validate().unwrap_err().to_string(),
        "Recommendation outcome requires an impression ID"
    );

    let mut invalid_json = event.clone();
    invalid_json.impression_id = Some(Uuid::new_v4());
    invalid_json.metadata_json = "[]".into();
    assert_eq!(
        invalid_json.validate().unwrap_err().to_string(),
        "Recommendation metadata must be a JSON object"
    );

    let mut oversized = event;
    oversized.impression_id = Some(Uuid::new_v4());
    oversized.metadata_json = format!("{{\"value\":\"{}\"}}", "x".repeat(4097));
    assert_eq!(
        oversized.validate().unwrap_err().to_string(),
        "Recommendation metadata is too large"
    );
}

#[test]
fn rollout_modes_have_stable_names() {
    assert_eq!(RolloutMode::Shadow.as_str(), "SHADOW");
    assert_eq!(RolloutMode::parse("learned").unwrap(), RolloutMode::Learned);
    assert!(RolloutMode::parse("unknown").is_err());
}

#[test]
fn recent_action_limit_is_bounded() {
    use platform::recommendation::RecentActionLimit;

    assert_eq!(RecentActionLimit::new(0).get(), 20);
    assert_eq!(RecentActionLimit::new(42).get(), 42);
    assert_eq!(RecentActionLimit::new(500).get(), 100);
}

#[test]
fn rollout_control_plane_aliases_map_to_safe_serving_modes() {
    assert_eq!(
        RolloutMode::parse("off").unwrap(),
        RolloutMode::Deterministic
    );
    assert_eq!(RolloutMode::parse("active").unwrap(), RolloutMode::Learned);
    assert!(RolloutMode::Deterministic.is_deterministic_primary());
    assert!(RolloutMode::Shadow.uses_deterministic_primary());
    assert!(RolloutMode::Canary.requests_learned_scoring());
    assert!(RolloutMode::Learned.requests_learned_scoring());
}

#[test]
fn stable_experiment_bucket_is_repeatable_and_bounded() {
    use platform::recommendation::stable_experiment_bucket;
    use uuid::Uuid;

    let viewer_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
    let first = stable_experiment_bucket(viewer_id, "home-feed-v1", 10_000);
    let second = stable_experiment_bucket(viewer_id, "home-feed-v1", 10_000);

    assert_eq!(first, second);
    assert!(first < 10_000);
    assert_eq!(stable_experiment_bucket(viewer_id, "home-feed-v1", 0), 0);
}
