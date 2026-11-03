use super::PostApplicationService;
use crate::dto::post_dto::{DeletePostDto, DeletePostResponseDto};
use anyhow::Result;
use std::sync::Arc;

#[derive(Clone)]
pub struct DeletePostUseCase {
    service: Arc<PostApplicationService>,
}

impl DeletePostUseCase {
    pub fn new(service: Arc<PostApplicationService>) -> Self {
        Self { service }
    }

    pub async fn execute(&self, request: DeletePostDto) -> Result<DeletePostResponseDto> {
        self.service.delete_post(request).await
    }

    pub async fn execute_with_operation(
        &self,
        request: DeletePostDto,
        context: Option<crate::dto::mutation_dto::MutationContextDto>,
    ) -> Result<DeletePostResponseDto> {
        self.service
            .delete_post_with_operation(request, context)
            .await
    }
}
