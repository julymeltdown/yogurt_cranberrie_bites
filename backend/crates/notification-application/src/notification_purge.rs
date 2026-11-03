use crate::errors::NotificationResult as Result;
use crate::ports::NotificationRepositoryPort;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[async_trait]
pub trait NotificationPurgePort: Send + Sync {
    async fn purge_expired_batch(
        &self,
        cutoff: DateTime<Utc>,
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
        limit: i32,
    ) -> Result<Vec<Uuid>>;
}

#[async_trait]
impl<T> NotificationPurgePort for T
where
    T: NotificationRepositoryPort + ?Sized,
{
    async fn purge_expired_batch(
        &self,
        cutoff: DateTime<Utc>,
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
        limit: i32,
    ) -> Result<Vec<Uuid>> {
        NotificationRepositoryPort::purge_expired_batch_consistent(
            self, cutoff, actor_id, request_id, ip_address, user_agent, dry_run, limit,
        )
        .await
    }
}
