use crate::dto::post_dto::{ApplyUserDeletionDto, ApplyUserDeletionResponseDto};
use crate::ports::{PostCachePort, UserLifecycleRepositoryPort};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone)]
pub struct ApplyUserDeletionUseCase<R, C> {
    repository: R,
    cache: C,
}

impl<R, C> ApplyUserDeletionUseCase<R, C>
where
    R: UserLifecycleRepositoryPort,
    C: PostCachePort,
{
    pub fn new(repository: R, cache: C) -> Self {
        Self { repository, cache }
    }

    pub async fn execute(
        &self,
        request: ApplyUserDeletionDto,
    ) -> anyhow::Result<ApplyUserDeletionResponseDto> {
        let saga_id = parse_uuid(&request.saga_id, "Saga ID is required")?;
        let user_id = parse_uuid(&request.user_id, "User ID is required")?;
        let actor_id = parse_uuid(&request.actor_id, "Actor ID is required")?;
        let retention_until = DateTime::parse_from_rfc3339(&request.retention_until)
            .map_err(|_| anyhow::anyhow!("Retention deadline is invalid"))?
            .with_timezone(&Utc);
        let post_ids = self
            .repository
            .apply_user_deletion(saga_id, user_id, actor_id, retention_until)
            .await?;
        for post_id in post_ids {
            if let Err(error) = self.cache.evict(post_id).await {
                tracing::warn!(%post_id, error=?error, "failed to evict deleted account post cache");
            }
        }
        Ok(ApplyUserDeletionResponseDto { applied: true })
    }
}

fn parse_uuid(value: &str, message: &str) -> anyhow::Result<Uuid> {
    Uuid::parse_str(value.trim()).map_err(|_| anyhow::anyhow!(message.to_owned()))
}
