use crate::dto::friend_dto::RelationshipPolicyDto;
use crate::ports::RelationshipPolicyRepositoryPort;
use anyhow::Result;
use uuid::Uuid;
#[derive(Clone)]
pub struct GetRelationshipPolicyUseCase<R> {
    repository: R,
}
impl<R> GetRelationshipPolicyUseCase<R>
where
    R: RelationshipPolicyRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
    pub async fn execute(&self, viewer_id: Uuid, target_id: Uuid) -> Result<RelationshipPolicyDto> {
        self.repository
            .policy(viewer_id, target_id)
            .await
            .map(Into::into)
    }
}
