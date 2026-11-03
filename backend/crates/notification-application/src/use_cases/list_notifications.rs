use crate::dto::notification_dto::NotificationPageDto;
use crate::errors::{NotificationError, NotificationInvalidInput, NotificationResult as Result};
use crate::notification_cursor;
use crate::notification_query::NotificationQueryPort;
use uuid::Uuid;

#[derive(Clone)]
pub struct ListNotificationsUseCase<R> {
    repository: R,
}

impl<R> ListNotificationsUseCase<R>
where
    R: NotificationQueryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(
        &self,
        user_id: &str,
        cursor: &str,
        limit: i32,
    ) -> Result<NotificationPageDto> {
        let user_id = parse_uuid(user_id, NotificationInvalidInput::UserIdRequired)?;
        let cursor = notification_cursor::parse(user_id, cursor)?;
        let limit = if limit > 0 { limit.min(100) } else { 20 };
        let page = self.repository.list_exact(user_id, cursor, limit).await?;
        Ok(NotificationPageDto {
            items: page.items.into_iter().map(Into::into).collect(),
            next_cursor: page.legacy_next_cursor.or_else(|| {
                page.next_position
                    .map(|position| notification_cursor::encode(user_id, position))
            }),
        })
    }
}

fn parse_uuid(value: &str, error: NotificationInvalidInput) -> Result<Uuid> {
    if value.trim().is_empty() {
        return Err(error.into());
    }
    Uuid::parse_str(value.trim()).map_err(|_| NotificationError::InvalidInput(error))
}
