use crate::dto::mutation_dto::MutationReceiptStatusDto;
use crate::ports::PostRepositoryPort;
use anyhow::Result;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct GetMutationReceiptStatusUseCase {
    repository: Arc<dyn PostRepositoryPort>,
}

impl GetMutationReceiptStatusUseCase {
    pub fn new(repository: Arc<dyn PostRepositoryPort>) -> Self {
        Self { repository }
    }

    pub async fn execute(&self, operation_id: Uuid) -> Result<Option<MutationReceiptStatusDto>> {
        self.repository
            .as_ref()
            .mutation_receipt_status(operation_id)
            .await
    }
}
