use crate::dto::mutation_dto::{
    MutationContextDto, MutationError, MutationReceiptStatusDto, PrivacyMutationOperationDto,
};
use crate::dto::profile_dto::{ProfileDto, UpdateProfilePrivacyDto};
use crate::errors::{ProfileError, ProfileResult as Result};
use crate::ports::ProfileRepositoryPort;
use profile_domain::profile::AccountVisibility;
use uuid::Uuid;

#[derive(Clone)]
pub struct UpdateProfilePrivacyUseCase<R> {
    repository: R,
}

impl<R> UpdateProfilePrivacyUseCase<R>
where
    R: ProfileRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn mutation_receipt_status(
        &self,
        operation_id: uuid::Uuid,
    ) -> Result<Option<MutationReceiptStatusDto>> {
        self.repository.mutation_receipt_status(operation_id).await
    }

    pub async fn execute(&self, request: UpdateProfilePrivacyDto) -> Result<ProfileDto> {
        self.execute_with_operation(request, None).await
    }

    pub async fn execute_with_operation(
        &self,
        request: UpdateProfilePrivacyDto,
        context: Option<MutationContextDto>,
    ) -> Result<ProfileDto> {
        let user_id = parse_uuid(&request.user_id, "User ID is required")?;
        let actor_id = parse_uuid(&request.actor_id, "Actor ID is required")?;
        if user_id != actor_id {
            return Err(ProfileError::Forbidden);
        }
        let visibility = AccountVisibility::parse(&request.account_visibility).map_err(|_| {
            ProfileError::InvalidInput("Account visibility must be PUBLIC or PRIVATE")
        })?;
        if let Some(context) = context {
            let operation = PrivacyMutationOperationDto::new(context, user_id, visibility)
                .map_err(|_| ProfileError::Mutation(MutationError::InvalidContext))?;
            if let Some(profile) = self.repository.lookup_privacy_mutation(&operation).await? {
                return Ok(profile);
            }
            if operation.context.lookup_only {
                return Err(MutationError::NotCommitted.into());
            }
            return self
                .repository
                .update_visibility_with_receipt(user_id, visibility.into(), &operation)
                .await?
                .ok_or(ProfileError::ProfileNotFound);
        }
        self.repository
            .update_visibility(user_id, visibility.into())
            .await?
            .ok_or(ProfileError::ProfileNotFound)
            .map(Into::into)
    }
}

fn parse_uuid(value: &str, error: &'static str) -> Result<Uuid> {
    if value.trim().is_empty() {
        return Err(ProfileError::InvalidInput(error));
    }
    Uuid::parse_str(value.trim()).map_err(|_| ProfileError::InvalidInput(error))
}
