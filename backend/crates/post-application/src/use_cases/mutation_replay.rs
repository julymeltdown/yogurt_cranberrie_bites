use super::PostApplicationService;
use crate::dto::mutation_dto::{
    MutationError, MutationKind, MutationOperationDto, MutationOutcomeDto,
};
use anyhow::{bail, Result};
use uuid::Uuid;

impl PostApplicationService {
    pub(super) async fn replay_mutation(
        &self,
        operation: Option<&MutationOperationDto>,
    ) -> Result<Option<MutationOutcomeDto>> {
        let Some(operation) = operation else {
            return Ok(None);
        };
        let result = self.posts.lookup_mutation(operation).await?;
        if result.is_none() && operation.context.lookup_only {
            return Err(MutationError::NotCommitted.into());
        }
        if let Some(result) = &result {
            let matches = matches!(
                (operation.kind, result),
                (MutationKind::CreatePost, MutationOutcomeDto::Post(..))
                    | (MutationKind::CreateComment, MutationOutcomeDto::Comment(..))
                    | (MutationKind::DeletePost, MutationOutcomeDto::Deleted { .. })
                    | (
                        MutationKind::LikePost,
                        MutationOutcomeDto::Like { liked: true, .. }
                    )
                    | (
                        MutationKind::UnlikePost,
                        MutationOutcomeDto::Like { liked: false, .. }
                    )
            );
            if !matches {
                return Err(MutationError::Conflict.into());
            }
            match result {
                MutationOutcomeDto::Post(post, _) => {
                    self.ensure_replay_visible(post.id, operation.context.actor_id)
                        .await?;
                }
                MutationOutcomeDto::Comment(comment, _) => {
                    if self.posts.fetch_comment(comment.id).await?.is_none() {
                        bail!("Comment not found");
                    }
                    self.ensure_replay_visible(comment.post_id, operation.context.actor_id)
                        .await?;
                }
                MutationOutcomeDto::Like { post_id, .. } => {
                    self.ensure_replay_visible(*post_id, operation.context.actor_id)
                        .await?;
                }
                // This receipt discloses no content. The immutable actor/key
                // binding proves who performed the authorized deletion.
                MutationOutcomeDto::Deleted { .. } => {}
            }
        }
        Ok(result)
    }

    pub(super) async fn ensure_replay_visible(&self, post_id: Uuid, actor: Uuid) -> Result<()> {
        let post = self
            .posts
            .fetch_post_header(post_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Post not found"))?;
        if !self.can_view(&post, actor).await? {
            bail!("Post not found");
        }
        Ok(())
    }
}
