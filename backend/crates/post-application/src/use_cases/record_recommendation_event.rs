use crate::ports::FeedRecommendationRepositoryPort;
use anyhow::Result;
use recommendation_policy::recommendation::RecommendationEvent;
use std::sync::Arc;

#[derive(Clone)]
pub struct RecordFeedRecommendationEventUseCase {
    repository: Arc<dyn FeedRecommendationRepositoryPort>,
}

impl RecordFeedRecommendationEventUseCase {
    pub fn new(repository: Arc<dyn FeedRecommendationRepositoryPort>) -> Self {
        Self { repository }
    }

    pub async fn execute(&self, event: RecommendationEvent) -> Result<bool> {
        event.validate()?;
        self.repository.record_event(&event).await
    }
}
