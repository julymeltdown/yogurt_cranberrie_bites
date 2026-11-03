use super::PostApplicationService;
use crate::dto::post_dto::PostDetailDto;
use anyhow::Result;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct GetPostDetailUseCase {
    service: Arc<PostApplicationService>,
}

impl GetPostDetailUseCase {
    pub fn new(service: Arc<PostApplicationService>) -> Self {
        Self { service }
    }

    pub async fn execute(&self, post_id: Uuid, viewer_id: Uuid) -> Result<PostDetailDto> {
        self.service.get_post_detail(post_id, viewer_id).await
    }
}
