use super::PostApplicationService;
use crate::dto::post_dto::{DeleteCommentDto, DeleteCommentResponseDto};
use anyhow::Result;
use std::sync::Arc;

#[derive(Clone)]
pub struct DeleteCommentUseCase {
    service: Arc<PostApplicationService>,
}

impl DeleteCommentUseCase {
    pub fn new(service: Arc<PostApplicationService>) -> Self {
        Self { service }
    }

    pub async fn execute(&self, request: DeleteCommentDto) -> Result<DeleteCommentResponseDto> {
        self.service.delete_comment(request).await
    }
}
