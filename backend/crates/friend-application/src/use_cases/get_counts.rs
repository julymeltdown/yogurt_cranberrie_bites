use crate::dto::friend_dto::FollowCountsDto;
use crate::ports::FollowRepositoryPort;
use anyhow::Result;
use uuid::Uuid;

#[derive(Clone)]
pub struct GetCountsUseCase<R> {
    repository: R,
}

impl<R> GetCountsUseCase<R>
where
    R: FollowRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(&self, user_id: Uuid) -> Result<FollowCountsDto> {
        Ok(self.repository.counts(user_id).await?.into())
    }
}
