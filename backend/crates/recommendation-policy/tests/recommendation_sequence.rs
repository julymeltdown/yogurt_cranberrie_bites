use chrono::{Duration, TimeZone, Utc};
use recommendation_policy::recommendation::{
    RecommendationAction, RecommendationEvent, RecommendationSubjectType, RecommendationSurface,
    RECOMMENDATION_SCHEMA_VERSION,
};
use recommendation_policy::recommendation_sequence::{RecentSequence, RecentSequencePolicy};
use uuid::Uuid;

fn event(
    event_id: u128,
    viewer_id: Uuid,
    subject_id: u128,
    action: RecommendationAction,
    occurred_at: chrono::DateTime<Utc>,
) -> RecommendationEvent {
    RecommendationEvent {
        event_id: Uuid::from_u128(event_id),
        schema_version: RECOMMENDATION_SCHEMA_VERSION,
        occurred_at,
        viewer_id,
        subject_type: RecommendationSubjectType::Post,
        subject_id: Uuid::from_u128(subject_id),
        actor_id: None,
        surface: RecommendationSurface::HomeFeed,
        position: if action == RecommendationAction::Impression {
            Some(0)
        } else {
            None
        },
        request_id: Some("sequence-test".into()),
        session_id: None,
        action,
        source: "MATERIALIZED_TIMELINE".into(),
        model_version: "feed-linear-v1".into(),
        impression_id: Some(if action == RecommendationAction::Impression {
            Uuid::from_u128(event_id)
        } else {
            Uuid::from_u128(event_id + 1_000)
        }),
        propensity_micros: if action == RecommendationAction::Impression {
            Some(1_000_000)
        } else {
            None
        },
        dwell_ms: None,
        metadata_json: "{}".into(),
    }
}

#[test]
fn sequence_deduplicates_orders_filters_and_caps_recent_actions() {
    let now = Utc.with_ymd_and_hms(2026, 8, 4, 12, 0, 0).unwrap();
    let viewer = Uuid::from_u128(1);
    let other_viewer = Uuid::from_u128(2);
    let duplicate = event(
        10,
        viewer,
        100,
        RecommendationAction::Like,
        now - Duration::minutes(2),
    );
    let events = vec![
        event(
            11,
            viewer,
            101,
            RecommendationAction::Hide,
            now - Duration::minutes(1),
        ),
        duplicate.clone(),
        duplicate,
        event(
            12,
            viewer,
            102,
            RecommendationAction::Comment,
            now - Duration::minutes(3),
        ),
        event(
            13,
            viewer,
            103,
            RecommendationAction::Like,
            now - Duration::hours(30),
        ),
        event(
            14,
            other_viewer,
            104,
            RecommendationAction::Like,
            now - Duration::seconds(10),
        ),
    ];

    let sequence = RecentSequence::build(
        viewer,
        RecommendationSurface::HomeFeed,
        now,
        events,
        RecentSequencePolicy {
            max_actions: 2,
            max_age: Duration::hours(24),
            future_tolerance: Duration::minutes(5),
        },
    )
    .expect("sequence");

    assert_eq!(sequence.actions.len(), 2);
    assert_eq!(sequence.actions[0].subject_id, Uuid::from_u128(101));
    assert_eq!(sequence.actions[1].subject_id, Uuid::from_u128(100));
}

#[test]
fn sequence_exposes_bounded_negative_and_repeat_penalties() {
    let now = Utc.with_ymd_and_hms(2026, 8, 4, 12, 0, 0).unwrap();
    let viewer = Uuid::from_u128(1);
    let hidden = Uuid::from_u128(200);
    let impressed = Uuid::from_u128(201);
    let sequence = RecentSequence::build(
        viewer,
        RecommendationSurface::HomeFeed,
        now,
        vec![
            event(
                20,
                viewer,
                200,
                RecommendationAction::Hide,
                now - Duration::minutes(1),
            ),
            event(
                21,
                viewer,
                201,
                RecommendationAction::Impression,
                now - Duration::minutes(2),
            ),
        ],
        RecentSequencePolicy::default(),
    )
    .expect("sequence");

    assert_eq!(sequence.negative_penalty(hidden), 1.0);
    assert_eq!(sequence.repeat_penalty(impressed), 0.25);
    assert_eq!(sequence.negative_penalty(Uuid::from_u128(999)), 0.0);
}
