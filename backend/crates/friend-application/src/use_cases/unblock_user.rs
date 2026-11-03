use crate::dto::friend_dto::RelationshipPolicyDto;
use crate::ports::RelationshipPolicyRepositoryPort;
use friend_domain::error::FriendError;
use uuid::Uuid;
#[derive(Clone)]
pub struct UnblockUserUseCase<R> {
    repository: R,
}
impl<R> UnblockUserUseCase<R>
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
            return Err(FriendError::InvalidInput("Cannot unblock yourself".into()));
        }
        Ok(self.repository.unblock(viewer_id, target_id).await?.into())
    }
}
