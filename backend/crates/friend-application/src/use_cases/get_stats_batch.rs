use crate::dto::friend_dto::FollowStatsDto;
use crate::ports::FollowRepositoryPort;
use friend_domain::error::FriendError;
use std::collections::HashSet;
use uuid::Uuid;

const MAX_BATCH_SIZE: usize = 100;

#[derive(Clone)]
pub struct GetStatsBatchUseCase<R> {
    repository: R,
}

impl<R> GetStatsBatchUseCase<R>
where
    R: FollowRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(&self, user_ids: &[Uuid]) -> Result<Vec<FollowStatsDto>, FriendError> {
        if user_ids.len() > MAX_BATCH_SIZE {
            return Err(FriendError::InvalidInput(
                "Follow stats support at most 100 user IDs".into(),
            ));
        }
        let mut seen = HashSet::with_capacity(user_ids.len());
        let unique_ids = user_ids
            .iter()
            .copied()
            .filter(|user_id| seen.insert(*user_id))
            .collect::<Vec<_>>();
        let counts = self.repository.batch_counts(&unique_ids).await?;
        let ordered_counts = unique_ids
            .iter()
            .map(|user_id| counts.get(user_id).copied())
            .collect::<Option<Vec<_>>>();
        let Some(ordered_counts) = ordered_counts.filter(|_| counts.len() == unique_ids.len())
        else {
            return Err(FriendError::Unexpected(anyhow::anyhow!(
                "Follow count repository returned an incomplete batch"
            )));
        };
        Ok(unique_ids
            .into_iter()
            .zip(ordered_counts)
            .map(|(user_id, stats)| FollowStatsDto {
                user_id: user_id.to_string(),
                follower_count: stats.follower_count,
                following_count: stats.following_count,
            })
            .collect())
    }
}
