use crate::dto::friend_dto::FollowRequestDto;
use crate::ports::FollowRequestRepositoryPort;
use chrono::Utc;
use friend_domain::error::FriendError;
use uuid::Uuid;

#[derive(Clone)]
pub struct CancelFollowRequestUseCase<R> {
    repository: R,
}
impl<R> CancelFollowRequestUseCase<R>
where
    R: FollowRequestRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
    pub async fn execute(
        &self,
        request_id: Uuid,
        actor_id: Uuid,
    ) -> Result<FollowRequestDto, FriendError> {
        self.repository
            .cancel_request(request_id, actor_id, Utc::now())
            .await?
            .ok_or_else(|| FriendError::NotFound("Follow request not found".into()))
            .map(Into::into)
    }
}
