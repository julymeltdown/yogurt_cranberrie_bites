use crate::dto::auth_dto::UserDeletionSagaDto;
use crate::ports::UserDeletionSagaPort;
use uuid::Uuid;

#[derive(Clone)]
pub struct GetUserDeletionUseCase<R> {
    repository: R,
}

impl<R> GetUserDeletionUseCase<R>
where
    R: UserDeletionSagaPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(
        &self,
        user_id: Uuid,
    ) -> crate::errors::AuthResult<Option<UserDeletionSagaDto>> {
        Ok(self
            .repository
            .find_by_user_id(user_id)
            .await?
            .map(auth_domain::user_deletion::UserDeletionSaga::from)
            .map(Into::into))
    }

    pub async fn execute_by_saga_id(
        &self,
        saga_id: Uuid,
    ) -> crate::errors::AuthResult<Option<UserDeletionSagaDto>> {
        Ok(self
            .repository
            .find_by_saga_id(saga_id)
            .await?
            .map(auth_domain::user_deletion::UserDeletionSaga::from)
            .map(Into::into))
    }
}
