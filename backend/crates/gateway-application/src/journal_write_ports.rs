//! Required private write capabilities. A held session retains its connection
//! until it is dropped; no persistence connection type crosses these ports.
use crate::dto::journal_repository_dto::OperationJournalLookupDto;
use crate::dto::journal_write_dto::*;
use crate::journal_write_error::JournalWriteResult;
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait JournalIntentConnectionPort: Send {
    async fn insert_intent(
        &mut self,
        command: JournalIntentDto<'_>,
    ) -> JournalWriteResult<Option<Uuid>>;
}
#[async_trait]
pub trait JournalCompletionConnectionPort: Send {
    async fn record_completion(
        &mut self,
        command: JournalCompletionCommandDto<'_>,
    ) -> JournalWriteResult<JournalCompletionWriteDto>;
    async fn record_owner_replay(
        &mut self,
        command: JournalOwnerReplayCommandDto<'_>,
    ) -> JournalWriteResult<JournalCompletionWriteDto>;
}
#[async_trait]
pub trait JournalDispatchRepositoryPort: Send + Sync {
    async fn acquire_intent_connection(
        &self,
    ) -> JournalWriteResult<Box<dyn JournalIntentConnectionPort>>;
    async fn acquire_completion_connection(
        &self,
        caller: JournalCompletionCallerDto,
    ) -> JournalWriteResult<Box<dyn JournalCompletionConnectionPort>>;
    async fn owner_replay_identity(
        &self,
        query: JournalReplayQueryDto<'_>,
    ) -> JournalWriteResult<Option<JournalReplayIdentityDto>>;
    async fn response_replay(
        &self,
        query: JournalReplayQueryDto<'_>,
    ) -> JournalWriteResult<Option<JournalReplayResponseDto>>;
}
#[async_trait]
pub trait JournalRetentionRepositoryPort: Send + Sync {
    async fn purge(&self, command: JournalPurgeCommandDto) -> JournalWriteResult<PurgeOutcome>;
}
#[async_trait]
pub trait OwnerReceiptJournalRepositoryPort: Send + Sync {
    async fn claim_next(
        &self,
        command: OwnerReceiptClaimRequestDto,
    ) -> JournalWriteResult<Option<OwnerReceiptClaimDto>>;
    async fn finish_lookup(
        &self,
        command: OwnerReceiptFinishLookupDto<'_>,
    ) -> JournalWriteResult<bool>;
    async fn record_commit(&self, command: OwnerReceiptCommitDto<'_>) -> JournalWriteResult<bool>;
    async fn local_receipt(
        &self,
        query: OperationJournalLookupDto,
    ) -> JournalWriteResult<Option<LocalOwnerReceiptRecordDto>>;
}
