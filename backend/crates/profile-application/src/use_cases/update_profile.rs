use crate::dto::mutation_dto::{MutationContextDto, MutationError, ProfileMutationOperationDto};
use crate::dto::profile_dto::{ProfileDto, UpdateProfileDto};
use crate::errors::{ProfileError, ProfileResult as Result};
use crate::ports::ProfileRepositoryPort;
use profile_domain::profile::{AccountVisibility, Profile};
use uuid::Uuid;

#[derive(Clone)]
pub struct UpdateProfileUseCase<R> {
    repository: R,
}

impl<R> UpdateProfileUseCase<R>
where
    R: ProfileRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(&self, request: UpdateProfileDto) -> Result<ProfileDto> {
        self.execute_with_operation(request, None).await
    }

    pub async fn execute_with_operation(
        &self,
        request: UpdateProfileDto,
        context: Option<MutationContextDto>,
    ) -> Result<ProfileDto> {
        let user_id = parse_uuid(&request.user_id, "User ID is required")?;
        let username = validate_username(&request.username)?;
        let nickname = required_trimmed(&request.nickname, "Display name is required")?;
        let avatar_url = required_trimmed(&request.avatar_url, "Avatar URL is required")?;
        let bio = required_trimmed(&request.bio, "Profile message is required")?;

        if let Some(context) = context {
            let operation = ProfileMutationOperationDto::profile_edit(
                context.clone(),
                user_id,
                &username,
                &nickname,
                &avatar_url,
                &bio,
            )
            .map_err(|_| ProfileError::Mutation(MutationError::InvalidContext))?;
            if let Some(profile) = self.repository.lookup_profile_mutation(&operation).await? {
                return Ok(profile);
            }
            if context.lookup_only {
                return Err(MutationError::NotCommitted.into());
            }
            return self
                .repository
                .update_profile_with_receipt(
                    user_id,
                    &username,
                    &nickname,
                    &avatar_url,
                    &bio,
                    &operation,
                )
                .await?
                .ok_or(ProfileError::InvalidInput("Profile is deleted"));
        }

        // Ownership is checked against every row holding the name, including a
        // soft-deleted one. The ordinary lookup hides deleted profiles, and the
        // unique index does not, so relying on the lookup let the insert fail on the
        // constraint and surface as an unexpected server error rather than a
        // conflict. A deleted profile keeps its name reserved, which is the intent.
        if let Some(existing_owner) = self.repository.find_username_owner(&username).await? {
            if existing_owner != user_id {
                return Err(ProfileError::InvalidInput("Username already taken"));
            }
        }

        let current = self
            .repository
            .find_by_user_id(user_id)
            .await?
            .map(Profile::from);
        let profile = Profile {
            user_id,
            username,
            nickname,
            avatar_url,
            bio,
            follower_count: current
                .as_ref()
                .map(|value| value.follower_count)
                .unwrap_or(0),
            post_count: current.as_ref().map(|value| value.post_count).unwrap_or(0),
            following_count: current
                .as_ref()
                .map(|value| value.following_count)
                .unwrap_or(0),
            account_visibility: current
                .as_ref()
                .map(|value| value.account_visibility)
                .unwrap_or(AccountVisibility::Public),
            lifecycle_status: current
                .as_ref()
                .map(|value| value.lifecycle_status.clone())
                .unwrap_or_else(|| "ACTIVE".to_owned()),
        };

        Ok(self.repository.save(&profile.into()).await?.into())
    }
}

fn validate_username(value: &str) -> Result<String> {
    let normalized = value.trim().to_lowercase();
    if normalized.len() < 3 || normalized.len() > 20 {
        return Err(ProfileError::InvalidInput(
            "Username must be 3-20 characters (letters, numbers, underscore)",
        ));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Err(ProfileError::InvalidInput(
            "Username must be 3-20 characters (letters, numbers, underscore)",
        ));
    }
    Ok(normalized)
}

fn required_trimmed(value: &str, error: &'static str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ProfileError::InvalidInput(error));
    }
    Ok(trimmed.to_owned())
}

fn parse_uuid(value: &str, error: &'static str) -> Result<Uuid> {
    if value.trim().is_empty() {
        return Err(ProfileError::InvalidInput(error));
    }
    Uuid::parse_str(value.trim()).map_err(|_| ProfileError::InvalidInput(error))
}
