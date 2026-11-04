use crate::dto::friend_dto::{ApplyUserDeletionDto, ApplyUserDeletionResponseDto};
use crate::ports::UserLifecycleRepositoryPort;
use chrono::{DateTime, Utc};
use friend_domain::error::FriendError;
use uuid::Uuid;

#[derive(Clone)]
pub struct ApplyUserDeletionUseCase<R> {
    repository: R,
}

impl<R> ApplyUserDeletionUseCase<R>
where
    R: UserLifecycleRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(
        &self,
        request: ApplyUserDeletionDto,
    ) -> Result<ApplyUserDeletionResponseDto, FriendError> {
        let saga_id = parse_uuid(&request.saga_id, "Saga ID is required")?;
        let user_id = parse_uuid(&request.user_id, "User ID is required")?;
        let actor_id = parse_uuid(&request.actor_id, "Actor ID is required")?;
        let retention_until = DateTime::parse_from_rfc3339(&request.retention_until)
            .map_err(|_| FriendError::InvalidInput("Retention deadline is invalid".into()))?
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
    ) -> Result<crate::dto::friend_dto::CommittedDeletionReceiptDto, FriendError> {
        let saga_id = parse_uuid(&request.saga_id, "Saga ID is required")?;
        let user_id = parse_uuid(&request.user_id, "User ID is required")?;
        let actor_id = parse_uuid(&request.actor_id, "Actor ID is required")?;
        let retention_until = DateTime::parse_from_rfc3339(&request.retention_until)
            .map_err(|_| FriendError::InvalidInput("Retention deadline is invalid".into()))?
            .with_timezone(&Utc);
        let receipt = self
            .repository
            .apply_user_deletion_with_receipt(saga_id, user_id, actor_id, retention_until)
            .await?;
        Ok(receipt)
    }
}

fn parse_uuid(value: &str, message: &str) -> Result<Uuid, FriendError> {
    Uuid::parse_str(value.trim()).map_err(|_| FriendError::InvalidInput(message.to_owned()))
}
