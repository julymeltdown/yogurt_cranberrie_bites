use crate::dto::friend_dto::FollowListDto;
use crate::ports::FollowRepositoryPort;
use anyhow::Result;
use friend_domain::follow_edge::FollowCursor;
use uuid::Uuid;

#[derive(Clone)]
pub struct ListFollowersUseCase<R> {
    repository: R,
}

impl<R> ListFollowersUseCase<R>
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
            .list_followers(user_id, cursor, normalize_limit(limit))
            .await?;
        Ok(FollowListDto {
            edges: edges.into_iter().map(Into::into).collect(),
            next_cursor,
            total_count,
        })
    }
}

const DEFAULT_LIMIT: i32 = 20;
const MAX_LIMIT: i32 = 100;

pub(super) fn normalize_limit(limit: i32) -> i32 {
    if limit <= 0 {
        DEFAULT_LIMIT
    } else {
        limit.min(MAX_LIMIT)
    }
}
