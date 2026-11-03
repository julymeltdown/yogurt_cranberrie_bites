use chrono::{DateTime, Utc};
use recommendation_policy::recommendation::{
    RecommendationAction, RecommendationContractError, RecommendationSubjectType,
    RecommendationSurface,
};
use uuid::Uuid;

#[derive(Clone, PartialEq)]
pub struct RecommendationEvent {
    pub event_id: Uuid,
    pub schema_version: u16,
    pub occurred_at: DateTime<Utc>,
    pub viewer_id: Uuid,
    pub subject_type: RecommendationSubjectType,
    pub subject_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub surface: RecommendationSurface,
    pub position: Option<i32>,
    pub request_id: Option<String>,
    pub session_id: Option<Uuid>,
    pub action: RecommendationAction,
    pub source: String,
    pub model_version: String,
    pub metadata_json: String,
    pub propensity: Option<f64>,
    pub impression_id: Option<Uuid>,
    pub dwell_ms: Option<u32>,
}

impl RecommendationEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        event_id: Uuid,
        schema_version: u16,
        occurred_at: DateTime<Utc>,
        viewer_id: Uuid,
        subject_type: RecommendationSubjectType,
        subject_id: Uuid,
        actor_id: Option<Uuid>,
        surface: RecommendationSurface,
        position: Option<i32>,
        request_id: Option<String>,
        session_id: Option<Uuid>,
        action: RecommendationAction,
        source: impl Into<String>,
        model_version: impl Into<String>,
        metadata_json: impl Into<String>,
        propensity: Option<f64>,
    ) -> Result<Self, RecommendationContractError> {
        if position.is_some_and(|value| value < 0) {
            return Err(RecommendationContractError::new(
                "Recommendation position is out of range",
            ));
        }
        if propensity.is_some_and(|value| !value.is_finite() || value <= 0.0 || value > 1.0) {
            return Err(RecommendationContractError::new(
                "Recommendation propensity is out of range",
            ));
        }
        let event = Self {
            event_id,
            schema_version,
            occurred_at,
            viewer_id,
            subject_type,
            subject_id,
            actor_id,
            surface,
            position,
            request_id,
            session_id,
            action,
            source: source.into(),
            model_version: model_version.into(),
            metadata_json: metadata_json.into(),
            propensity,
            impression_id: Some(event_id),
            dwell_ms: None,
        };
        event.validate()?;
        Ok(event)
    }

    pub fn validate(&self) -> Result<(), RecommendationContractError> {
        self.to_contract()?.validate()
    }

    pub fn to_contract(
        &self,
    ) -> Result<
        recommendation_policy::recommendation::RecommendationEvent,
        RecommendationContractError,
    > {
        Ok(recommendation_policy::recommendation::RecommendationEvent {
            event_id: self.event_id,
            schema_version: self.schema_version,
            occurred_at: self.occurred_at,
            viewer_id: self.viewer_id,
            subject_type: self.subject_type,
            subject_id: self.subject_id,
            actor_id: self.actor_id,
            surface: self.surface,
            position: self
                .position
                .map(|value| {
                    u32::try_from(value).map_err(|_| {
                        RecommendationContractError::new("Recommendation position is out of range")
                    })
                })
                .transpose()?,
            request_id: self.request_id.clone(),
            session_id: self.session_id,
            action: self.action,
            source: self.source.clone(),
            model_version: self.model_version.clone(),
            impression_id: self.impression_id,
            propensity_micros: self
                .propensity
                .map(|value| (value * 1_000_000.0).round() as u32),
            dwell_ms: self.dwell_ms,
            metadata_json: self.metadata_json.clone(),
        })
    }
}

impl std::fmt::Debug for RecommendationEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecommendationEvent")
            .field("details", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod debug_redaction_tests {
    use super::*;

    #[test]
    fn recommendation_event_debug_hides_identity_and_private_metadata() {
        let event_id = Uuid::new_v4();
        let event = RecommendationEvent {
            event_id,
            schema_version: 1,
            occurred_at: Utc::now(),
            viewer_id: Uuid::new_v4(),
            subject_type: RecommendationSubjectType::Post,
            subject_id: Uuid::new_v4(),
            actor_id: Some(Uuid::new_v4()),
            surface: RecommendationSurface::HomeFeed,
            position: Some(1),
            request_id: Some("private-recommendation-request".into()),
            session_id: Some(Uuid::new_v4()),
            action: RecommendationAction::Click,
            source: "private-recommendation-source".into(),
            model_version: "private-recommendation-model".into(),
            metadata_json: r#"{"signal":"private-recommendation-signal"}"#.into(),
            propensity: Some(0.7),
            impression_id: Some(Uuid::new_v4()),
            dwell_ms: Some(500),
        };

        let rendered = format!("{event:?}");
        for secret in [
            &event_id.to_string(),
            "private-recommendation-request",
            "private-recommendation-source",
            "private-recommendation-model",
            "private-recommendation-signal",
        ] {
            assert!(!rendered.contains(secret));
        }
        assert!(rendered.contains("[REDACTED]"));
    }
}
