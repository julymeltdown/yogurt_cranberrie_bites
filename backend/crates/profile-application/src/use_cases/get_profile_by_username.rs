use crate::dto::profile_dto::ProfileDto;
use crate::errors::{ProfileError, ProfileResult as Result};
use crate::ports::ProfileRepositoryPort;

#[derive(Clone)]
pub struct GetProfileByUsernameUseCase<R> {
    repository: R,
}

impl<R> GetProfileByUsernameUseCase<R>
where
    R: ProfileRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(&self, username: &str) -> Result<ProfileDto> {
        let normalized = normalize_username(username)?;
        let profile = self
            .repository
            .find_by_username(&normalized)
            .await?
            .ok_or(ProfileError::ProfileNotFound)?;
        Ok(profile.into())
    }
}

pub fn normalize_username(username: &str) -> Result<String> {
    let trimmed = username.trim().to_lowercase();
    if trimmed.is_empty() {
        return Err(ProfileError::InvalidInput("Username is required"));
    }
    Ok(trimmed)
}
