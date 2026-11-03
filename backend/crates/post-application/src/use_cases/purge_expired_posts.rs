use super::PostApplicationService;
use crate::dto::post_dto::{PurgeExpiredPostsDto, PurgeExpiredPostsResponseDto};
use anyhow::Result;
use std::sync::Arc;

#[derive(Clone)]
pub struct PurgeExpiredPostsUseCase {
    service: Arc<PostApplicationService>,
}

impl PurgeExpiredPostsUseCase {
    pub fn new(service: Arc<PostApplicationService>) -> Self {
        Self { service }
    }

    pub async fn execute(
        &self,
        request: PurgeExpiredPostsDto,
    ) -> Result<PurgeExpiredPostsResponseDto> {
        self.service.purge_expired_posts(request).await
    }
}
