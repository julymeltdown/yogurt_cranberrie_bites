use crate::dto::profile_dto::AvatarUploadResponseDto;
use crate::errors::{ProfileError, ProfileResult as Result};
use crate::ports::{AvatarProcessorPort, AvatarRepositoryPort};
use profile_domain::avatar::MAX_AVATAR_BYTES;
use uuid::Uuid;

#[derive(Clone)]
pub struct UploadAvatarUseCase<R, P> {
    repository: R,
    processor: P,
}

impl<R, P> UploadAvatarUseCase<R, P>
where
    R: AvatarRepositoryPort,
    P: AvatarProcessorPort,
{
    pub fn new(repository: R, processor: P) -> Self {
        Self {
            repository,
            processor,
        }
    }

    pub async fn execute(
        &self,
        user_id: &str,
        content: Vec<u8>,
        content_type: &str,
        filename: &str,
    ) -> Result<AvatarUploadResponseDto> {
        let user_id = parse_uuid(user_id)?;
        if content.is_empty() {
            return Err(ProfileError::InvalidInput("Avatar image is required"));
        }
        // The size bound lives in the domain so there is one limit rather than
        // two that can disagree. The message is built from the constant so it
        // cannot claim a limit that is no longer enforced.
        if content.len() > MAX_AVATAR_BYTES {
            return Err(ProfileError::AvatarUploadTooLarge);
        }
        let avatar = self
            .processor
            .process(content, content_type, filename)
            .await?;

        self.repository.save(user_id, &avatar).await?;

        Ok(AvatarUploadResponseDto {
            user_id: user_id.to_string(),
        })
    }
}

fn parse_uuid(user_id: &str) -> Result<Uuid> {
    if user_id.trim().is_empty() {
        return Err(ProfileError::InvalidInput("User ID is required"));
    }
    Uuid::parse_str(user_id.trim()).map_err(|_| ProfileError::InvalidInput("User ID is required"))
}
