//! Private journal persistence transfer. Owner verification remains subsequent
//! domain policy; a public response cannot reconstruct its missing actor.
use crate::dto::journal_repository_dto::{
    CallerOperationJournalLookupDto, ManualResolutionJournalCommandDto,
    ManualResolutionJournalWriteDto, OperationJournalLookupDto, ReconciliationBacklog,
};
use crate::journal_repository_error::JournalRepositoryResult;
use crate::ports::OperationJournalRepositoryPort;
use gateway_domain::operation::{
    CallerOperationStatus, ManualOperationResolution, OperationStatus,
};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManualResolutionWriteOutcome {
    Created(ManualOperationResolution),
    Existing(ManualOperationResolution),
    OperationNotFound,
    OperationNotUnknown,
    Conflict,
}

#[derive(Clone)]
pub struct OperationJournalService {
    repository: Arc<dyn OperationJournalRepositoryPort>,
}

impl OperationJournalService {
    pub fn new(repository: Arc<dyn OperationJournalRepositoryPort>) -> Self {
        Self { repository }
    }

    pub async fn operation_status(
        &self,
        operation_id: Uuid,
    ) -> JournalRepositoryResult<Option<OperationStatus>> {
        self.repository
            .operation_status(OperationJournalLookupDto { operation_id })
            .await
            .map(|record| record.map(Into::into))
    }

    pub async fn caller_operation_status(
        &self,
        operation_id: Uuid,
        actor_id: &str,
    ) -> JournalRepositoryResult<Option<CallerOperationStatus>> {
        self.repository
            .caller_operation_status(CallerOperationJournalLookupDto {
                operation_id,
                actor_id: actor_id.to_owned(),
            })
            .await
            .map(|record| record.map(Into::into))
    }

    pub async fn reconciliation_backlog(&self) -> JournalRepositoryResult<ReconciliationBacklog> {
        self.repository.reconciliation_backlog().await
    }

    pub async fn record_manual_resolution(
        &self,
        command: ManualResolutionJournalCommandDto,
    ) -> JournalRepositoryResult<ManualResolutionWriteOutcome> {
        Ok(
            match self.repository.record_manual_resolution(command).await? {
                ManualResolutionJournalWriteDto::Created(record) => {
                    ManualResolutionWriteOutcome::Created(record.into())
                }
                ManualResolutionJournalWriteDto::Existing(record) => {
                    ManualResolutionWriteOutcome::Existing(record.into())
                }
                ManualResolutionJournalWriteDto::OperationNotFound => {
                    ManualResolutionWriteOutcome::OperationNotFound
                }
                ManualResolutionJournalWriteDto::OperationNotUnknown => {
                    ManualResolutionWriteOutcome::OperationNotUnknown
                }
                ManualResolutionJournalWriteDto::Conflict => ManualResolutionWriteOutcome::Conflict,
            },
        )
    }
}
