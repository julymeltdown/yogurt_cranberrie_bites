use super::PostApplicationService;
use crate::dto::post_dto::FeedDto;
use anyhow::Result;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct SearchPostsUseCase {
    service: Arc<PostApplicationService>,
}

impl SearchPostsUseCase {
    pub fn new(service: Arc<PostApplicationService>) -> Self {
        Self { service }
    }

    pub async fn execute(
        &self,
        viewer_id: Uuid,
        query: &str,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedDto> {
        self.service
            .search_posts(viewer_id, query, cursor, limit)
            .await
    }
}
