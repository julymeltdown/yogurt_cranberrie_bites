use crate::dto::friend_dto::FollowActionDto;
use crate::ports::FollowRequestRepositoryPort;
use chrono::Utc;
use friend_domain::error::FriendError;
use friend_domain::follow_request::AcceptFollowRequestOutcome;
use uuid::Uuid;

#[derive(Clone)]
pub struct AcceptFollowRequestUseCase<R> {
    repository: R,
}

impl<R> AcceptFollowRequestUseCase<R>
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
    ) -> Result<FollowActionDto, FriendError> {
        let edge = match self
            .repository
            .accept_request(request_id, actor_id, Utc::now())
            .await?
        {
            AcceptFollowRequestOutcome::Accepted { edge, .. } => edge,
            AcceptFollowRequestOutcome::NotFound => {
                return Err(FriendError::NotFound("Follow request not found".into()));
            }
            AcceptFollowRequestOutcome::Blocked => {
                return Err(FriendError::PermissionDenied("Blocked relationship".into()));
            }
        };
        Ok(FollowActionDto {
            edge: Some(edge.into()),
            following: true,
            status: "ACTIVE".to_owned(),
            request_id: Some(request_id.to_string()),
        })
    }
}
