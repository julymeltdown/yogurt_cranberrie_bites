use super::PostApplicationService;
use crate::dto::post_dto::PostLikeDto;
use anyhow::Result;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct UnlikePostUseCase {
    service: Arc<PostApplicationService>,
}

impl UnlikePostUseCase {
    pub fn new(service: Arc<PostApplicationService>) -> Self {
        Self { service }
    }

    pub async fn execute(&self, post_id: Uuid, user_id: Uuid) -> Result<PostLikeDto> {
        self.service.unlike_post(post_id, user_id).await
    }

    pub async fn execute_with_operation(
        &self,
        post_id: Uuid,
        user_id: Uuid,
        context: Option<crate::dto::mutation_dto::MutationContextDto>,
    ) -> Result<PostLikeDto> {
        self.service
            .unlike_post_with_operation(post_id, user_id, context)
            .await
    }
}
