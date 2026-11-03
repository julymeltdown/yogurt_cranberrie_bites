use crate::dto::profile_dto::ProfileDto;
use crate::errors::{ProfileError, ProfileResult as Result};
use crate::ports::ProfileRepositoryPort;
use uuid::Uuid;

#[derive(Clone)]
pub struct GetProfileUseCase<R> {
    repository: R,
}

impl<R> GetProfileUseCase<R>
where
    R: ProfileRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(&self, user_id: Uuid) -> Result<ProfileDto> {
        let profile = self
            .repository
            .find_by_user_id(user_id)
            .await?
            .ok_or(ProfileError::ProfileNotFound)?;
        Ok(profile.into())
    }
}
