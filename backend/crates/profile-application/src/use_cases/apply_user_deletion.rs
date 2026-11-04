use crate::dto::profile_dto::{ApplyUserDeletionDto, ApplyUserDeletionResponseDto};
use crate::errors::{ProfileError, ProfileResult as Result};
use crate::ports::ProfileRepositoryPort;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone)]
pub struct ApplyUserDeletionUseCase<R> {
    repository: R,
}

impl<R> ApplyUserDeletionUseCase<R>
where
    R: ProfileRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(
        &self,
        request: ApplyUserDeletionDto,
    ) -> Result<ApplyUserDeletionResponseDto> {
        let saga_id = parse_uuid(&request.saga_id, "Saga ID is required")?;
        let user_id = parse_uuid(&request.user_id, "User ID is required")?;
        let actor_id = parse_uuid(&request.actor_id, "Actor ID is required")?;
        let retention_until = DateTime::parse_from_rfc3339(&request.retention_until)
            .map_err(|_| ProfileError::InvalidInput("Retention deadline is invalid"))?
            .with_timezone(&Utc);
        self.repository
            .apply_user_deletion(saga_id, user_id, actor_id, retention_until)
            .await?;
        Ok(ApplyUserDeletionResponseDto { applied: true })
    }

    /// RPC entry: preserve the actual owning transaction receipt.
    pub async fn execute_with_receipt(
        &self,
        request: ApplyUserDeletionDto,
    ) -> Result<crate::dto::profile_dto::CommittedDeletionReceiptDto> {
        let saga_id = parse_uuid(&request.saga_id, "Saga ID is required")?;
        let user_id = parse_uuid(&request.user_id, "User ID is required")?;
        let actor_id = parse_uuid(&request.actor_id, "Actor ID is required")?;
        let retention_until = DateTime::parse_from_rfc3339(&request.retention_until)
            .map_err(|_| ProfileError::InvalidInput("Retention deadline is invalid"))?
            .with_timezone(&Utc);
        let receipt = self
            .repository
            .apply_user_deletion_with_receipt(saga_id, user_id, actor_id, retention_until)
            .await?;
        Ok(receipt)
    }
}

fn parse_uuid(value: &str, message: &'static str) -> Result<Uuid> {
    Uuid::parse_str(value.trim()).map_err(|_| ProfileError::InvalidInput(message))
}
