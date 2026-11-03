use crate::dto::mutation_dto::MutationError;
use anyhow::Result;
use chrono::{DateTime, Utc};
use post_domain::recommendation::{
    FeedRecommendationImpression, RecommendationAction, RecommendationEvent,
    RecommendationSubjectType, RecommendationSurface,
};
use uuid::Uuid;

/// Original impression defaults for verifying an already accepted outcome.
/// This is not an impression or permission to admit a new event. Metadata and
/// the client event body remain represented only by their durable fingerprint.
#[derive(Clone)]
pub struct RecommendationRetryContextDto {
    pub impression_id: Uuid,
    pub position: i32,
    pub request_id: String,
    pub session_id: Option<Uuid>,
    pub source: String,
    pub model_version: String,
    pub propensity_micros: u32,
}

impl From<FeedRecommendationImpression> for RecommendationRetryContextDto {
    fn from(i: FeedRecommendationImpression) -> Self {
        Self {
            impression_id: i.impression_id,
            position: i.position as i32,
            request_id: i.request_id,
            session_id: i.session_id,
            source: i.source.as_str().into(),
            model_version: i.rank_version,
            propensity_micros: i.propensity_micros,
        }
    }
}

/// Parsed input with optional fields preserved until transactional admission.
#[derive(Clone)]
pub struct RecommendationAdmissionDto {
    pub event_id: Uuid,
    pub schema_version: u16,
    pub occurred_at: DateTime<Utc>,
    pub viewer_id: Uuid,
    pub post_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub position: Option<i32>,
    pub request_id: Option<String>,
    pub session_id: Option<Uuid>,
    pub action: RecommendationAction,
    pub source: Option<String>,
    pub model_version: Option<String>,
    pub impression_id: Option<Uuid>,
    pub propensity_micros: Option<u32>,
    pub dwell_ms: Option<u32>,
    pub metadata_json: String,
}
impl RecommendationAdmissionDto {
    pub fn normalize(
        &self,
        defaults: RecommendationRetryContextDto,
    ) -> Result<RecommendationEvent> {
        if self
            .impression_id
            .is_some_and(|id| id != defaults.impression_id)
        {
            return Err(MutationError::Conflict.into());
        }
        let mut event = RecommendationEvent::new(
            self.event_id,
            self.schema_version,
            self.occurred_at,
            self.viewer_id,
            RecommendationSubjectType::Post,
            self.post_id,
            self.actor_id.or(Some(self.viewer_id)),
            RecommendationSurface::HomeFeed,
            self.position.or(Some(defaults.position)),
            self.request_id.clone().or(Some(defaults.request_id)),
            self.session_id.or(defaults.session_id),
            self.action,
            self.source.clone().unwrap_or(defaults.source),
            self.model_version.clone().unwrap_or(defaults.model_version),
            self.metadata_json.clone(),
            self.propensity_micros
                .or(Some(defaults.propensity_micros))
                .map(|p| f64::from(p) / 1_000_000.0),
        )?;
        event.impression_id = Some(defaults.impression_id);
        event.dwell_ms = self.dwell_ms;
        event.validate()?;
        Ok(event)
    }
}
