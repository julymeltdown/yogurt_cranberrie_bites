use crate::dto::friend_dto::FollowRequestPageDto;
use crate::ports::FollowRequestRepositoryPort;
use friend_domain::error::FriendError;
use friend_domain::follow_edge::FollowCursor;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub enum FollowRequestListKind {
    Incoming,
    Outgoing,
}

#[derive(Clone)]
pub struct ListFollowRequestsUseCase<R> {
    repository: R,
}
impl<R> ListFollowRequestsUseCase<R>
where
    R: FollowRequestRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
    pub async fn execute(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
        kind: FollowRequestListKind,
    ) -> Result<FollowRequestPageDto, FriendError> {
        let cursor = match cursor {
            Some(value) => FollowCursor::parse(value)
                .map_err(|_| FriendError::InvalidInput("Invalid cursor".into()))?,
            None => None,
        };
        let limit = if limit <= 0 { 20 } else { limit.min(100) };
        let (requests, next_cursor) = match kind {
            FollowRequestListKind::Incoming => {
                self.repository
                    .list_incoming_requests(user_id, cursor, limit)
                    .await?
            }
            FollowRequestListKind::Outgoing => {
                self.repository
                    .list_outgoing_requests(user_id, cursor, limit)
                    .await?
            }
        };
        Ok(FollowRequestPageDto {
            requests: requests.into_iter().map(Into::into).collect(),
            next_cursor,
        })
    }
}
