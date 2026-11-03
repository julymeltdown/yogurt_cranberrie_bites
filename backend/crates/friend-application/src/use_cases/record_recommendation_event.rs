use crate::ports::RecommendationEventRepositoryPort;
use anyhow::Result;
use recommendation_policy::recommendation::RecommendationEvent;

#[derive(Clone)]
pub struct RecordRecommendationEventUseCase<R> {
    repository: R,
}

impl<R> RecordRecommendationEventUseCase<R>
where
    R: RecommendationEventRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(&self, event: RecommendationEvent) -> Result<bool> {
        event.validate()?;
        self.repository.record_event(&event).await
    }
}
