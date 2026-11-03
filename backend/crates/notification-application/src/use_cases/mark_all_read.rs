use crate::errors::{NotificationError, NotificationInvalidInput, NotificationResult as Result};
use crate::notification_runtime::NotificationRuntimePort;
use crate::ports::NotificationRepositoryPort;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct MarkAllReadUseCase<R> {
    repository: R,
    runtime: Arc<dyn NotificationRuntimePort>,
}

impl<R> MarkAllReadUseCase<R>
where
    R: NotificationRepositoryPort,
{
    pub fn new(repository: R, runtime: Arc<dyn NotificationRuntimePort>) -> Self {
        Self {
            repository,
            runtime,
        }
    }

    pub async fn execute(&self, user_id: &str) -> Result<i64> {
        if user_id.trim().is_empty() {
            return Err(NotificationInvalidInput::UserIdRequired.into());
        }
        let user_id = Uuid::parse_str(user_id.trim()).map_err(|_| {
            NotificationError::InvalidInput(NotificationInvalidInput::UserIdRequired)
        })?;
        self.repository
            .mark_all_read(user_id, self.runtime.now())
            .await
    }
}
