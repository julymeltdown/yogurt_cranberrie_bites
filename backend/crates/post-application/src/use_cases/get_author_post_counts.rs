use super::PostApplicationService;
use crate::dto::post_dto::AuthorPostCountDto;
use anyhow::Result;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct GetAuthorPostCountsUseCase {
    service: Arc<PostApplicationService>,
}

impl GetAuthorPostCountsUseCase {
    pub fn new(service: Arc<PostApplicationService>) -> Self {
        Self { service }
    }

    pub async fn execute(&self, author_ids: Vec<Uuid>) -> Result<Vec<AuthorPostCountDto>> {
        self.service.author_post_counts(author_ids).await
    }
}
