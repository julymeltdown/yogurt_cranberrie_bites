use super::PostApplicationService;
use crate::dto::post_dto::FeedDto;
use anyhow::Result;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct GetTrendingUseCase {
    service: Arc<PostApplicationService>,
}

impl GetTrendingUseCase {
    pub fn new(service: Arc<PostApplicationService>) -> Self {
        Self { service }
    }

    pub async fn execute(
        &self,
        viewer_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedDto> {
        self.service.get_trending(viewer_id, cursor, limit).await
    }
}
