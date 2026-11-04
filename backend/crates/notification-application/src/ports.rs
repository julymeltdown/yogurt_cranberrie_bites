use crate::dto::repository_dto::{
    NotificationAuditRecordDto, NotificationPageRecordDto, NotificationRecordDto,
};
use crate::errors::NotificationResult as Result;
use crate::notification_contracts::{
    NotificationCreateOutcome, NotificationCursor, NotificationQueryPage,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use uuid::Uuid;

/// Repository facade bridging the narrow application capabilities.
/// Implementations must provide privacy checks, atomic creation, exact pagination,
/// bounded purge and user deletion explicitly. No compatibility default can
/// silently substitute a weaker implementation for these capabilities.
#[async_trait]
pub trait NotificationRepositoryPort: Send + Sync {
    async fn save(&self, notification: &NotificationRecordDto) -> Result<NotificationRecordDto>;
    async fn is_recipient_suppressed(&self, user_id: Uuid) -> Result<bool>;
    /// Actor suppression is distinct from recipient suppression. Each adapter
    /// must provide both policies explicitly.
    async fn is_actor_suppressed(&self, user_id: Uuid) -> Result<bool>;
    async fn create_consistent(
        &self,
        notification: &NotificationRecordDto,
    ) -> Result<NotificationCreateOutcome>;
    async fn find_by_event_id_and_type(
        &self,
        user_id: Uuid,
        event_id: Uuid,
        notification_type: &str,
    ) -> Result<Option<NotificationRecordDto>>;
    async fn find_by_id(
        &self,
        user_id: Uuid,
        notification_id: Uuid,
    ) -> Result<Option<NotificationRecordDto>>;
    async fn list(
        &self,
        user_id: Uuid,
        cursor: Option<(i64, Uuid)>,
        limit: i32,
    ) -> Result<NotificationPageRecordDto>;

    async fn list_consistent(
        &self,
        user_id: Uuid,
        cursor: Option<NotificationCursor>,
        limit: i32,
    ) -> Result<NotificationQueryPage>;
    async fn mark_read(
        &self,
        user_id: Uuid,
        notification_id: Uuid,
        read_at: DateTime<Utc>,
    ) -> Result<Option<(NotificationRecordDto, bool)>>;
    async fn mark_all_read(&self, user_id: Uuid, read_at: DateTime<Utc>) -> Result<i64>;
    async fn soft_delete_with_audit(
        &self,
        user_id: Uuid,
        notification_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
        audit_event: &NotificationAuditRecordDto,
    ) -> Result<bool>;
    async fn record_audit_event(&self, event: &NotificationAuditRecordDto) -> Result<()>;
    async fn purge_expired_notifications(
        &self,
        cutoff: DateTime<Utc>,
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
    ) -> Result<Vec<Uuid>>;

    async fn purge_expired_batch_consistent(
        &self,
        cutoff: DateTime<Utc>,
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
        limit: i32,
    ) -> Result<Vec<Uuid>>;
    async fn apply_user_deletion(
        &self,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> Result<()>;

    /// A legacy implementation returning unit cannot produce COMMIT evidence.
    async fn apply_user_deletion_with_receipt(
        &self,
        _saga_id: Uuid,
        _user_id: Uuid,
        _actor_id: Uuid,
        _retention_until: DateTime<Utc>,
    ) -> Result<crate::dto::notification_dto::NotificationDeletionReceiptDto> {
        Err(crate::errors::NotificationError::DependencyUnavailable)
    }
}

#[async_trait]
impl<T> NotificationRepositoryPort for Arc<T>
where
    T: NotificationRepositoryPort + ?Sized,
{
    async fn save(&self, notification: &NotificationRecordDto) -> Result<NotificationRecordDto> {
        (**self).save(notification).await
    }

    async fn is_recipient_suppressed(&self, user_id: Uuid) -> Result<bool> {
        (**self).is_recipient_suppressed(user_id).await
    }

    async fn is_actor_suppressed(&self, user_id: Uuid) -> Result<bool> {
        (**self).is_actor_suppressed(user_id).await
    }

    async fn create_consistent(
        &self,
        notification: &NotificationRecordDto,
    ) -> Result<NotificationCreateOutcome> {
        (**self).create_consistent(notification).await
    }

    async fn find_by_event_id_and_type(
        &self,
        user_id: Uuid,
        event_id: Uuid,
        notification_type: &str,
    ) -> Result<Option<NotificationRecordDto>> {
        (**self)
            .find_by_event_id_and_type(user_id, event_id, notification_type)
            .await
    }

    async fn find_by_id(
        &self,
        user_id: Uuid,
        notification_id: Uuid,
    ) -> Result<Option<NotificationRecordDto>> {
        (**self).find_by_id(user_id, notification_id).await
    }

    async fn list(
        &self,
        user_id: Uuid,
        cursor: Option<(i64, Uuid)>,
        limit: i32,
    ) -> Result<NotificationPageRecordDto> {
        (**self).list(user_id, cursor, limit).await
    }

    async fn list_consistent(
        &self,
        user_id: Uuid,
        cursor: Option<NotificationCursor>,
        limit: i32,
    ) -> Result<NotificationQueryPage> {
        (**self).list_consistent(user_id, cursor, limit).await
    }

    async fn mark_read(
        &self,
        user_id: Uuid,
        notification_id: Uuid,
        read_at: DateTime<Utc>,
    ) -> Result<Option<(NotificationRecordDto, bool)>> {
        (**self).mark_read(user_id, notification_id, read_at).await
    }

    async fn mark_all_read(&self, user_id: Uuid, read_at: DateTime<Utc>) -> Result<i64> {
        (**self).mark_all_read(user_id, read_at).await
    }

    async fn soft_delete_with_audit(
        &self,
        user_id: Uuid,
        notification_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
        audit_event: &NotificationAuditRecordDto,
    ) -> Result<bool> {
        (**self)
            .soft_delete_with_audit(
                user_id,
                notification_id,
                actor_id,
                reason,
                retention_until,
                audit_event,
            )
            .await
    }

    async fn record_audit_event(&self, event: &NotificationAuditRecordDto) -> Result<()> {
        (**self).record_audit_event(event).await
    }

    async fn purge_expired_notifications(
        &self,
        cutoff: DateTime<Utc>,
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
    ) -> Result<Vec<Uuid>> {
        (**self)
            .purge_expired_notifications(
                cutoff, actor_id, request_id, ip_address, user_agent, dry_run,
            )
            .await
    }

    async fn purge_expired_batch_consistent(
        &self,
        cutoff: DateTime<Utc>,
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
        limit: i32,
    ) -> Result<Vec<Uuid>> {
        (**self)
            .purge_expired_batch_consistent(
                cutoff, actor_id, request_id, ip_address, user_agent, dry_run, limit,
            )
            .await
    }

    async fn apply_user_deletion(
        &self,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> Result<()> {
        (**self)
            .apply_user_deletion(saga_id, user_id, actor_id, retention_until)
            .await
    }

    async fn apply_user_deletion_with_receipt(
        &self,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> Result<crate::dto::notification_dto::NotificationDeletionReceiptDto> {
        (**self)
            .apply_user_deletion_with_receipt(saga_id, user_id, actor_id, retention_until)
            .await
    }
}
