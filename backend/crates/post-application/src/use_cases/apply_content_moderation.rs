use crate::dto::post_dto::{ApplyContentModerationDto, ApplyContentModerationResponseDto};
use crate::ports::{ContentModerationRepositoryPort, PostCachePort};
use post_domain::post::{ContentModerationAction, ContentModerationTarget};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct ApplyContentModerationUseCase {
    repository: Arc<dyn ContentModerationRepositoryPort>,
    cache: Arc<dyn PostCachePort>,
}

impl ApplyContentModerationUseCase {
    pub fn new(
        repository: Arc<dyn ContentModerationRepositoryPort>,
        cache: Arc<dyn PostCachePort>,
    ) -> Self {
        Self { repository, cache }
    }

    pub async fn execute(
        &self,
        request: ApplyContentModerationDto,
    ) -> anyhow::Result<ApplyContentModerationResponseDto> {
        let report_id = parse_uuid(&request.report_id, "Report ID is required")?;
        let target = ContentModerationTarget::parse(&request.target_type)?;
        let target_id = parse_uuid(&request.target_id, "Moderation target ID is required")?;
        let actor_id = parse_uuid(&request.actor_id, "Actor ID is required")?;
        let action = ContentModerationAction::parse(&request.action)?;
        let affected_post_id = self
            .repository
            .apply_content_moderation(report_id, target, target_id, actor_id, action)
            .await?;
        if let Some(post_id) = affected_post_id {
            if let Err(error) = self.cache.evict(post_id).await {
                tracing::warn!(%post_id, error = ?error, "failed to evict moderated post cache");
            }
        }
        Ok(ApplyContentModerationResponseDto {
            applied: affected_post_id.is_some(),
        })
    }
}

fn parse_uuid(value: &str, message: &str) -> anyhow::Result<Uuid> {
    Uuid::parse_str(value.trim()).map_err(|_| anyhow::anyhow!(message.to_owned()))
}
