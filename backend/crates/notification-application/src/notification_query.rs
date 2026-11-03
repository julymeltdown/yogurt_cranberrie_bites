use crate::errors::NotificationResult as Result;
use crate::notification_contracts::{NotificationCursor, NotificationQueryPage};
use crate::ports::NotificationRepositoryPort;
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait NotificationQueryPort: Send + Sync {
    async fn list_exact(
        &self,
        recipient_id: Uuid,
        cursor: Option<NotificationCursor>,
        limit: i32,
    ) -> Result<NotificationQueryPage>;
}

#[async_trait]
impl<T> NotificationQueryPort for T
where
    T: NotificationRepositoryPort + ?Sized,
{
    async fn list_exact(
        &self,
        recipient_id: Uuid,
        cursor: Option<NotificationCursor>,
        limit: i32,
    ) -> Result<NotificationQueryPage> {
        NotificationRepositoryPort::list_consistent(self, recipient_id, cursor, limit).await
    }
}
