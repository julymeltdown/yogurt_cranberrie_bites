use crate::dto::friend_dto::RelationshipPolicyDto;
use crate::ports::RelationshipPolicyRepositoryPort;
use chrono::Utc;
use friend_domain::error::FriendError;
use uuid::Uuid;
#[derive(Clone)]
pub struct MuteUserUseCase<R> {
    repository: R,
}
impl<R> MuteUserUseCase<R>
where
    R: RelationshipPolicyRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
    pub async fn execute(
        &self,
        viewer_id: Uuid,
        target_id: Uuid,
    ) -> Result<RelationshipPolicyDto, FriendError> {
        if viewer_id == target_id {
            return Err(FriendError::InvalidInput("Cannot mute yourself".into()));
        }
        Ok(self
            .repository
            .mute(viewer_id, target_id, Utc::now())
            .await?
            .into())
    }
}
