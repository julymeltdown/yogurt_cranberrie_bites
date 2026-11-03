use crate::dto::notification_dto::{
    PurgeExpiredNotificationsDto, PurgeExpiredNotificationsResponseDto,
};
use crate::errors::{NotificationError, NotificationInvalidInput, NotificationResult as Result};
use crate::notification_purge::NotificationPurgePort;
use crate::notification_runtime::NotificationRuntimePort;
use std::sync::Arc;
use uuid::Uuid;

const DEFAULT_BATCH_LIMIT: i32 = 500;
const MAX_BATCH_LIMIT: i32 = 1_000;

#[derive(Clone)]
pub struct PurgeExpiredNotificationsUseCase<R> {
    repository: R,
    runtime: Arc<dyn NotificationRuntimePort>,
    batch_limit: i32,
}

impl<R> PurgeExpiredNotificationsUseCase<R>
where
    R: NotificationPurgePort,
{
    pub fn new(repository: R, runtime: Arc<dyn NotificationRuntimePort>) -> Self {
        Self {
            repository,
            runtime,
            batch_limit: DEFAULT_BATCH_LIMIT,
        }
    }

    #[must_use]
    pub fn with_batch_limit(mut self, batch_limit: i32) -> Self {
        self.batch_limit = batch_limit.clamp(1, MAX_BATCH_LIMIT);
        self
    }

    pub async fn execute(
        &self,
        request: PurgeExpiredNotificationsDto,
    ) -> Result<PurgeExpiredNotificationsResponseDto> {
        let actor_id = parse_uuid(&request.actor_id, NotificationInvalidInput::ActorIdRequired)?;
        let purged = self
            .repository
            .purge_expired_batch(
                self.runtime.now(),
                actor_id,
                optional_text(request.request_id),
                optional_text(request.ip_address),
                optional_text(request.user_agent),
                request.dry_run,
                self.batch_limit,
            )
            .await?;

        Ok(PurgeExpiredNotificationsResponseDto {
            purged_count: purged.len() as i64,
        })
    }
}

fn parse_uuid(value: &str, error: NotificationInvalidInput) -> Result<Uuid> {
    if value.trim().is_empty() {
        return Err(error.into());
    }
    Uuid::parse_str(value.trim()).map_err(|_| NotificationError::InvalidInput(error))
}

fn optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
