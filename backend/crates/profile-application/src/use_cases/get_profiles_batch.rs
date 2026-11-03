use crate::dto::profile_dto::ProfileDto;
use crate::errors::{ProfileError, ProfileResult as Result};
use crate::ports::ProfileRepositoryPort;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

const MAX_BATCH_SIZE: usize = 100;

#[derive(Clone)]
pub struct GetProfilesBatchUseCase<R> {
    repository: R,
}

impl<R> GetProfilesBatchUseCase<R>
where
    R: ProfileRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(&self, user_ids: &[Uuid]) -> Result<Vec<ProfileDto>> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }
        if user_ids.len() > MAX_BATCH_SIZE {
            return Err(ProfileError::InvalidInput(
                "Profile batch supports at most 100 user IDs",
            ));
        }

        let mut seen = HashSet::with_capacity(user_ids.len());
        let unique_ids = user_ids
            .iter()
            .copied()
            .filter(|user_id| seen.insert(*user_id))
            .collect::<Vec<_>>();
        let existing = self.repository.find_by_user_ids(&unique_ids).await?;
        let mut by_id = existing
            .into_iter()
            .map(|profile| (profile.user_id, profile))
            .collect::<HashMap<_, _>>();

        Ok(unique_ids
            .into_iter()
            .filter_map(|user_id| by_id.remove(&user_id).map(Into::into))
            .collect())
    }
}
