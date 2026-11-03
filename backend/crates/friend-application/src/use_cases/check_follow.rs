use crate::ports::FollowRepositoryPort;
use anyhow::Result;
use uuid::Uuid;

#[derive(Clone)]
pub struct CheckFollowUseCase<R> {
    repository: R,
}

impl<R> CheckFollowUseCase<R>
where
    R: FollowRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(&self, follower_id: Uuid, followee_id: Uuid) -> Result<bool> {
        self.repository.is_following(follower_id, followee_id).await
    }
}
