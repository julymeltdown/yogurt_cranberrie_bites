//! Required queue, payload and maintenance capabilities with private transfers.
use crate::dto::{operation_dto::ReconciliationOutcome, queue_repository_dto::*};
use crate::journal_write_error::JournalWriteResult;
use crate::queue_operation_error::QueueOperationResult;
use async_trait::async_trait;

pub trait QueuePayloadPort: Send + Sync {
    fn encrypt(
        &self,
        command: QueuePayloadEncryptDto<'_>,
    ) -> QueueOperationResult<EncryptedQueuePayloadDto>;
    fn decrypt(
        &self,
        query: QueuePayloadDecryptDto<'_>,
    ) -> QueueOperationResult<QueuePlaintextPayloadDto>;
}
#[async_trait]
pub trait QueueClaimConnectionPort: Send {
    async fn claim_next(
        &mut self,
        query: QueueClaimQueryDto,
    ) -> QueueOperationResult<Option<ClaimedQueueOperationDto>>;
}
#[async_trait]
pub trait AsyncOperationQueueRepositoryPort: Send + Sync {
    async fn enqueue(&self, command: QueueEnqueueCommandDto<'_>) -> QueueOperationResult<bool>;
    async fn acquire_claim_connection(
        &self,
    ) -> QueueOperationResult<Box<dyn QueueClaimConnectionPort>>;
    async fn claim_next(
        &self,
        query: QueueClaimQueryDto,
    ) -> QueueOperationResult<Option<ClaimedQueueOperationDto>>;
    async fn mark_done(&self, lease: QueueLeaseIdentityDto) -> QueueOperationResult<bool>;
    async fn retry_later(&self, command: QueueRetryCommandDto<'_>) -> QueueOperationResult<bool>;
    async fn dead_letter(&self, command: QueueDeadCommandDto<'_>) -> QueueOperationResult<bool>;
    async fn complete_claim(
        &self,
        command: QueueCompleteCommandDto<'_>,
    ) -> QueueOperationResult<bool>;
    async fn dead_letter_claim(
        &self,
        command: QueueDeadClaimCommandDto<'_>,
    ) -> QueueOperationResult<bool>;
}
#[async_trait]
pub trait JournalMaintenanceRepositoryPort: Send + Sync {
    async fn reconcile_stale(
        &self,
        command: JournalMaintenanceCommandDto,
    ) -> JournalWriteResult<ReconciliationOutcome>;
}
