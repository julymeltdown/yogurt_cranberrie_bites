use anyhow::Result;
use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use friend_service::application::ports::RecommendationEventRepositoryPort;
use friend_service::application::use_cases::record_recommendation_event::RecordRecommendationEventUseCase;
use platform::recommendation::{
    RecommendationAction, RecommendationEvent, RecommendationSubjectType, RecommendationSurface,
    RECOMMENDATION_SCHEMA_VERSION,
};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

struct RecordingEventRepository {
    accepted: bool,
    events: Mutex<Vec<RecommendationEvent>>,
}

#[async_trait]
impl RecommendationEventRepositoryPort for RecordingEventRepository {
    async fn record_event(&self, event: &RecommendationEvent) -> Result<bool> {
        self.events.lock().unwrap().push(event.clone());
        Ok(self.accepted)
    }
}

fn valid_event() -> RecommendationEvent {
    RecommendationEvent {
        event_id: Uuid::from_u128(1),
        schema_version: RECOMMENDATION_SCHEMA_VERSION,
        occurred_at: Utc.with_ymd_and_hms(2026, 10, 4, 0, 0, 0).unwrap(),
        viewer_id: Uuid::from_u128(2),
        subject_type: RecommendationSubjectType::User,
        subject_id: Uuid::from_u128(3),
        actor_id: None,
        surface: RecommendationSurface::PeopleRecommendation,
        position: Some(0),
        request_id: Some("request-1".into()),
        session_id: None,
        action: RecommendationAction::Hide,
        source: "mutual_follow".into(),
        model_version: "people-linear-v1".into(),
        impression_id: Some(Uuid::from_u128(4)),
        propensity_micros: None,
        dwell_ms: None,
        metadata_json: "{}".into(),
    }
}

#[tokio::test]
async fn valid_event_is_delegated_to_the_narrow_event_port() {
    let repository = Arc::new(RecordingEventRepository {
        accepted: true,
        events: Mutex::new(Vec::new()),
    });
    let use_case = RecordRecommendationEventUseCase::new(Arc::clone(&repository));
    let event = valid_event();

    let accepted = use_case
        .execute(event.clone())
        .await
        .expect("valid event should be recorded");

    assert!(accepted);
    assert_eq!(repository.events.lock().unwrap().as_slice(), &[event]);
}

#[tokio::test]
async fn invalid_event_is_rejected_before_repository_access() {
    let repository = Arc::new(RecordingEventRepository {
        accepted: true,
        events: Mutex::new(Vec::new()),
    });
    let use_case = RecordRecommendationEventUseCase::new(Arc::clone(&repository));
    let mut event = valid_event();
    event.event_id = Uuid::nil();

    assert!(use_case.execute(event).await.is_err());
    assert!(repository.events.lock().unwrap().is_empty());
}
