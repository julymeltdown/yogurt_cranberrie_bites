use super::PostApplicationService;
use crate::dto::post_dto::{CommentDto, CreateCommentDto};
use anyhow::Result;
use std::sync::Arc;

#[derive(Clone)]
pub struct CreateCommentUseCase {
    service: Arc<PostApplicationService>,
}

impl CreateCommentUseCase {
    pub fn new(service: Arc<PostApplicationService>) -> Self {
        Self { service }
    }

    pub async fn execute(&self, request: CreateCommentDto) -> Result<CommentDto> {
        self.service.create_comment(request).await
    }

    pub async fn execute_with_operation(
        &self,
        request: CreateCommentDto,
        context: Option<crate::dto::mutation_dto::MutationContextDto>,
    ) -> Result<CommentDto> {
        self.service
            .create_comment_with_operation(request, context)
            .await
    }
}
