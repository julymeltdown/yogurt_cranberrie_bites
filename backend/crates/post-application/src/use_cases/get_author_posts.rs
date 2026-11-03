use super::PostApplicationService;
use crate::dto::post_dto::FeedDto;
use anyhow::Result;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct GetAuthorPostsUseCase {
    service: Arc<PostApplicationService>,
}

impl GetAuthorPostsUseCase {
    pub fn new(service: Arc<PostApplicationService>) -> Self {
        Self { service }
    }

    pub async fn execute(
        &self,
        author_id: Uuid,
        viewer_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedDto> {
        self.service
            .get_author_posts(author_id, viewer_id, cursor, limit)
            .await
    }
}
