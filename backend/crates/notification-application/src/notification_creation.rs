use crate::dto::repository_dto::NotificationRecordDto;
use crate::errors::NotificationResult as Result;
use crate::notification_contracts::NotificationCreateOutcome;
use crate::ports::NotificationRepositoryPort;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;

/// Deadline data derived by the incoming adapter from sealed admission. This
/// DTO does not establish signature origin or authorize a recipient by itself.
#[derive(Clone, Copy)]
pub struct NotificationWriteDeadlineDto {
    pub expires_at: DateTime<Utc>,
}

/// Required selected-write capability. No default or legacy fallback may drop
/// this deadline before the owning atomic suppression/dedup transaction.
#[async_trait]
pub trait NotificationAdmittedCreatePort: Send + Sync {
    async fn create_admitted(
        &self,
        notification: &NotificationRecordDto,
        deadline: NotificationWriteDeadlineDto,
    ) -> Result<NotificationCreateOutcome>;
}
#[async_trait]
impl<T: NotificationAdmittedCreatePort + ?Sized> NotificationAdmittedCreatePort for Arc<T> {
    async fn create_admitted(
        &self,
        notification: &NotificationRecordDto,
        deadline: NotificationWriteDeadlineDto,
    ) -> Result<NotificationCreateOutcome> {
        (**self).create_admitted(notification, deadline).await
    }
}

/// Atomic creation capability used by the application workflow.
#[async_trait]
pub trait NotificationCreatePort: Send + Sync {
    async fn create_unless_suppressed(
        &self,
        notification: &NotificationRecordDto,
    ) -> Result<NotificationCreateOutcome>;
}

/// Facade adapter delegates to the repository's mandatory atomic creation
/// implementation. Recording repositories implement their own test capability;
/// the production contract has no non-atomic compatibility fallback.
#[async_trait]
impl<T> NotificationCreatePort for T
where
    T: NotificationRepositoryPort + ?Sized,
{
    async fn create_unless_suppressed(
        &self,
        notification: &NotificationRecordDto,
    ) -> Result<NotificationCreateOutcome> {
        NotificationRepositoryPort::create_consistent(self, notification).await
    }
}
