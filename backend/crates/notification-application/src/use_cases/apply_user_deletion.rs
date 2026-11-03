use crate::dto::notification_dto::{ApplyUserDeletionDto, ApplyUserDeletionResponseDto};
use crate::errors::{NotificationError, NotificationInvalidInput, NotificationResult as Result};
use crate::ports::NotificationRepositoryPort;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone)]
pub struct ApplyUserDeletionUseCase<R> {
    repository: R,
}

impl<R> ApplyUserDeletionUseCase<R>
where
    R: NotificationRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(
        &self,
        request: ApplyUserDeletionDto,
    ) -> Result<ApplyUserDeletionResponseDto> {
        let saga_id = parse_uuid(&request.saga_id, NotificationInvalidInput::SagaIdRequired)?;
        let user_id = parse_uuid(&request.user_id, NotificationInvalidInput::UserIdRequired)?;
        let actor_id = parse_uuid(&request.actor_id, NotificationInvalidInput::ActorIdRequired)?;
        let retention_until = DateTime::parse_from_rfc3339(&request.retention_until)
            .map_err(|_| {
                NotificationError::InvalidInput(NotificationInvalidInput::RetentionDeadlineInvalid)
            })?
            .with_timezone(&Utc);
        self.repository
            .apply_user_deletion(saga_id, user_id, actor_id, retention_until)
            .await?;
        Ok(ApplyUserDeletionResponseDto { applied: true })
    }
}

fn parse_uuid(value: &str, message: NotificationInvalidInput) -> Result<Uuid> {
    Uuid::parse_str(value.trim()).map_err(|_| NotificationError::InvalidInput(message))
}
