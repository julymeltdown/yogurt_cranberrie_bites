use super::PostApplicationService;
use crate::dto::post_dto::{CreatePostDto, PostDto};
use anyhow::Result;
use std::sync::Arc;

#[derive(Clone)]
pub struct CreatePostUseCase {
    service: Arc<PostApplicationService>,
}

impl CreatePostUseCase {
    pub fn new(service: Arc<PostApplicationService>) -> Self {
        Self { service }
    }

    pub async fn execute(&self, request: CreatePostDto) -> Result<PostDto> {
        self.service.create_post(request).await
    }

    pub async fn execute_with_operation(
        &self,
        request: CreatePostDto,
        context: Option<crate::dto::mutation_dto::MutationContextDto>,
    ) -> Result<PostDto> {
        self.service
            .create_post_with_operation(request, context)
            .await
    }
}
