use crate::auth_runtime::AuthRuntimePort;
use crate::dto::auth_dto::{
    RetryUserDeletionDto, RetryUserDeletionResponseDto, UserDeletionSagaDto,
};
use crate::errors::AuthError;
use crate::ports::UserDeletionSagaPort;
use auth_domain::user_deletion::RetryUserDeletion;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct RetryUserDeletionUseCase<R> {
    repository: R,
    runtime: Arc<dyn AuthRuntimePort>,
}

impl<R> RetryUserDeletionUseCase<R>
where
    R: UserDeletionSagaPort,
{
    pub fn new(repository: R, runtime: Arc<dyn AuthRuntimePort>) -> Self {
        Self {
            repository,
            runtime,
        }
    }

    pub async fn execute(
        &self,
        request: RetryUserDeletionDto,
    ) -> crate::errors::AuthResult<RetryUserDeletionResponseDto> {
        let saga_id = parse_uuid(&request.saga_id, "Saga ID must be a UUID")?;
        let resolution_id = parse_uuid(&request.resolution_id, "Resolution ID must be a UUID")?;
        let actor_id = parse_uuid(&request.actor_id, "Actor ID must be a UUID")?;
        if saga_id.is_nil() {
            return Err(AuthError::InvalidInput("Saga ID must not be nil"));
        }
        if resolution_id.is_nil() {
            return Err(AuthError::InvalidInput("Resolution ID must not be nil"));
        }
        if actor_id.is_nil() {
            return Err(AuthError::InvalidInput("Actor ID must not be nil"));
        }

        let reason = request.reason.trim().to_owned();
        if !(20..=1024).contains(&reason.chars().count()) {
            return Err(AuthError::InvalidInput(
                "Reason must contain between 20 and 1024 characters",
            ));
        }

        let evidence_reference = request.evidence_reference.trim().to_owned();
        if !(3..=256).contains(&evidence_reference.len())
            || !evidence_reference.is_ascii()
            || !evidence_reference.chars().all(|character| {
                character.is_ascii_alphanumeric()
                    || matches!(character, '.' | '_' | ':' | '/' | '#' | '-')
            })
        {
            return Err(AuthError::InvalidInput(
                "Evidence reference must be a 3 to 256 character protected-record identifier",
            ));
        }

        let saga = self
            .repository
            .retry_dead_lettered(
                (RetryUserDeletion {
                    saga_id,
                    resolution_id,
                    actor_id,
                    reason,
                    evidence_reference,
                    occurred_at: self.runtime.now(),
                })
                .into(),
            )
            .await?
            .map(auth_domain::user_deletion::UserDeletionSaga::from)
            .ok_or(AuthError::NotFound)?;

        Ok(RetryUserDeletionResponseDto {
            resolution_id: resolution_id.to_string(),
            accepted: true,
            saga: UserDeletionSagaDto::from(saga),
        })
    }
}

fn parse_uuid(value: &str, error: &'static str) -> crate::errors::AuthResult<Uuid> {
    Uuid::parse_str(value.trim()).map_err(|_| AuthError::InvalidInput(error))
}
