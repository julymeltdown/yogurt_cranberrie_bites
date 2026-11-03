use crate::dto::post_dto::{
    RecordFeedRecommendationEventDto, RecordFeedRecommendationEventResponseDto,
};
use crate::dto::recommendation_retry_dto::RecommendationAdmissionDto;
use crate::ports::FeedRecommendationRepositoryPort;
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use post_domain::recommendation::RecommendationAction;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct RecordFeedRecommendationEventUseCase {
    repository: Arc<dyn FeedRecommendationRepositoryPort>,
}

impl RecordFeedRecommendationEventUseCase {
    pub fn new(repository: Arc<dyn FeedRecommendationRepositoryPort>) -> Self {
        Self { repository }
    }

    pub async fn execute(
        &self,
        request: RecordFeedRecommendationEventDto,
    ) -> Result<RecordFeedRecommendationEventResponseDto> {
        let viewer_id = parse_uuid(&request.viewer_id, "Viewer ID is required")?;
        let post_id = parse_uuid(&request.post_id, "Post ID is required")?;
        let action = RecommendationAction::parse(&request.action)?;
        if action == RecommendationAction::Impression {
            bail!("Feed impression events are recorded by GetFeed");
        }
        let event_id = parse_uuid(&request.event_id, "Event ID is required")?;
        let occurred_at = DateTime::parse_from_rfc3339(&request.occurred_at)
            .map_err(|_| anyhow::anyhow!("Occurred at is invalid"))?
            .with_timezone(&Utc);
        let actor_id = request
            .actor_id
            .as_deref()
            .map(|value| parse_uuid(value, "Actor ID is invalid"))
            .transpose()?;
        let session_id = request
            .session_id
            .as_deref()
            .map(|value| parse_uuid(value, "Session ID is invalid"))
            .transpose()?;
        let admission = RecommendationAdmissionDto {
            event_id,
            schema_version: request.schema_version,
            occurred_at,
            viewer_id,
            post_id,
            actor_id,
            position: request.position,
            request_id: request.request_id,
            session_id,
            action,
            source: request.source,
            model_version: request.model_version,
            impression_id: request
                .impression_id
                .as_deref()
                .map(|v| parse_uuid(v, "Impression ID is invalid"))
                .transpose()?,
            propensity_micros: request.propensity_micros,
            dwell_ms: request.dwell_ms,
            metadata_json: request.metadata_json,
        };
        let accepted = self
            .repository
            .append_recommendation_request(&admission)
            .await?;
        Ok(RecordFeedRecommendationEventResponseDto { accepted })
    }
}

fn parse_uuid(value: &str, message: &str) -> Result<Uuid> {
    Uuid::parse_str(value.trim()).map_err(|_| anyhow::anyhow!(message.to_owned()))
}
