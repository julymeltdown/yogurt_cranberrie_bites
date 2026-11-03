use crate::dto::profile_dto::AvatarDto;
use crate::errors::{ProfileError, ProfileResult as Result};
use crate::ports::AvatarRepositoryPort;
use uuid::Uuid;

#[derive(Clone)]
pub struct GetAvatarUseCase<R> {
    repository: R,
}

impl<R> GetAvatarUseCase<R>
where
    R: AvatarRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(&self, user_id: &str) -> Result<Option<AvatarDto>> {
        let user_id = parse_uuid(user_id)?;
        self.repository.find(user_id).await
    }
}

fn parse_uuid(user_id: &str) -> Result<Uuid> {
    if user_id.trim().is_empty() {
        return Err(ProfileError::InvalidInput("User ID is required"));
    }
    Uuid::parse_str(user_id.trim()).map_err(|_| ProfileError::InvalidInput("User ID is required"))
}
