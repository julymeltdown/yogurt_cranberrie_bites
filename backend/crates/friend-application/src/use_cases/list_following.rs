use crate::dto::friend_dto::FollowListDto;
use crate::ports::FollowRepositoryPort;
use crate::use_cases::list_followers::normalize_limit;
use anyhow::Result;
use friend_domain::follow_edge::FollowCursor;
use uuid::Uuid;

#[derive(Clone)]
pub struct ListFollowingUseCase<R> {
    repository: R,
}

impl<R> ListFollowingUseCase<R>
where
    R: FollowRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(
        &self,
        user_id: Uuid,
        cursor: Option<FollowCursor>,
        limit: i32,
    ) -> Result<FollowListDto> {
        let (edges, next_cursor, total_count) = self
            .repository
            .list_following(user_id, cursor, normalize_limit(limit))
            .await?;
        Ok(FollowListDto {
            edges: edges.into_iter().map(Into::into).collect(),
            next_cursor,
            total_count,
        })
    }
}
