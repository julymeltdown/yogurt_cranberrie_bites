use crate::dto::notification_dto::MarkReadResultDto;
use crate::errors::{NotificationError, NotificationInvalidInput, NotificationResult as Result};
use crate::notification_runtime::NotificationRuntimePort;
use crate::ports::NotificationRepositoryPort;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct MarkReadUseCase<R> {
    repository: R,
    runtime: Arc<dyn NotificationRuntimePort>,
}

impl<R> MarkReadUseCase<R>
where
    R: NotificationRepositoryPort,
{
    pub fn new(repository: R, runtime: Arc<dyn NotificationRuntimePort>) -> Self {
        Self {
            repository,
            runtime,
        }
    }

    pub async fn execute(&self, user_id: &str, notification_id: &str) -> Result<MarkReadResultDto> {
        let user_id = parse_uuid(user_id, NotificationInvalidInput::UserIdRequired)?;
        let notification_id = parse_uuid(
            notification_id,
            NotificationInvalidInput::NotificationIdRequired,
        )?;
        let (notification, updated) = self
            .repository
            .mark_read(user_id, notification_id, self.runtime.now())
            .await?
            .ok_or(NotificationError::NotFound)?;
        Ok(MarkReadResultDto {
            notification: notification.into(),
            updated,
        })
    }
}

fn parse_uuid(value: &str, error: NotificationInvalidInput) -> Result<Uuid> {
    if value.trim().is_empty() {
        return Err(error.into());
    }
    Uuid::parse_str(value.trim()).map_err(|_| NotificationError::InvalidInput(error))
}
