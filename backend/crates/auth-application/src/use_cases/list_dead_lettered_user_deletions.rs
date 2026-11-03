use crate::dto::auth_dto::{
    DeadLetteredUserDeletionCursorDto, DeadLetteredUserDeletionListDto, UserDeletionSagaDto,
};
use crate::ports::UserDeletionSagaPort;
use chrono::{DateTime, Utc};
use uuid::Uuid;

const MAX_PAGE_SIZE: i32 = 100;

#[derive(Clone)]
pub struct ListDeadLetteredUserDeletionsUseCase<R> {
    repository: R,
}

impl<R> ListDeadLetteredUserDeletionsUseCase<R>
where
    R: UserDeletionSagaPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(
        &self,
        limit: i32,
        after: Option<(DateTime<Utc>, Uuid)>,
    ) -> crate::errors::AuthResult<DeadLetteredUserDeletionListDto> {
        let limit = limit.clamp(1, MAX_PAGE_SIZE);
        let mut sagas = self.repository.list_dead_lettered(after, limit + 1).await?;
        let has_more = sagas.len() > limit as usize;
        if has_more {
            sagas.truncate(limit as usize);
        }
        let next_cursor = has_more
            .then(|| {
                sagas.last().map(|saga| DeadLetteredUserDeletionCursorDto {
                    updated_at: saga.updated_at,
                    saga_id: saga.id.to_string(),
                })
            })
            .flatten();
        Ok(DeadLetteredUserDeletionListDto {
            sagas: sagas
                .into_iter()
                .map(auth_domain::user_deletion::UserDeletionSaga::from)
                .map(UserDeletionSagaDto::from)
                .collect(),
            next_cursor,
        })
    }
}
