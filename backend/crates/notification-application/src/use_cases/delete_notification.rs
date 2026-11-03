use crate::dto::notification_dto::{DeleteNotificationDto, DeleteNotificationResponseDto};
use crate::errors::{NotificationError, NotificationInvalidInput, NotificationResult as Result};
use crate::notification_runtime::NotificationRuntimePort;
use crate::ports::NotificationRepositoryPort;
use chrono::Duration;
use notification_domain::notification::NotificationAuditEvent;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct DeleteNotificationUseCase<R> {
    repository: R,
    runtime: Arc<dyn NotificationRuntimePort>,
}

impl<R> DeleteNotificationUseCase<R>
where
    R: NotificationRepositoryPort,
{
    pub fn new(repository: R, runtime: Arc<dyn NotificationRuntimePort>) -> Self {
        Self {
            repository,
            runtime,
        }
    }

    pub async fn execute(
        &self,
        request: DeleteNotificationDto,
    ) -> Result<DeleteNotificationResponseDto> {
        let user_id = parse_uuid(&request.user_id, NotificationInvalidInput::UserIdRequired)?;
        let notification_id = parse_uuid(
            &request.notification_id,
            NotificationInvalidInput::NotificationIdRequired,
        )?;
        let actor_id = parse_uuid(&request.actor_id, NotificationInvalidInput::ActorIdRequired)?;
        let notification = self
            .repository
            .find_by_id(user_id, notification_id)
            .await?
            .ok_or(NotificationError::NotFound)?;

        if actor_id != user_id {
            self.repository
                .record_audit_event(
                    &build_audit_event(
                        self.runtime.as_ref(),
                        Some(actor_id),
                        Some(notification_id),
                        Some(user_id),
                        "NOTIFICATION_DELETED",
                        "FORBIDDEN",
                        &request,
                        json!({"ownerId": user_id}).to_string(),
                    )
                    .into(),
                )
                .await?;
            return Err(NotificationError::Forbidden);
        }

        let retention_until = self.runtime.now() + Duration::days(30);
        let audit_event = build_audit_event(
            self.runtime.as_ref(),
            Some(actor_id),
            Some(notification_id),
            Some(user_id),
            "NOTIFICATION_DELETED",
            "SUCCESS",
            &request,
            json!({
                "retentionUntil": retention_until,
                "type": notification.notification_type
            })
            .to_string(),
        );
        let deleted = self
            .repository
            .soft_delete_with_audit(
                user_id,
                notification_id,
                actor_id,
                "user_request",
                retention_until,
                &audit_event.into(),
            )
            .await?;
        if !deleted {
            return Err(NotificationError::NotFound);
        }

        Ok(DeleteNotificationResponseDto {
            notification_id: notification_id.to_string(),
            deleted: true,
        })
    }
}

fn build_audit_event(
    runtime: &dyn NotificationRuntimePort,
    actor_id: Option<Uuid>,
    notification_id: Option<Uuid>,
    user_id: Option<Uuid>,
    action: &str,
    outcome: &str,
    request: &DeleteNotificationDto,
    metadata_json: String,
) -> NotificationAuditEvent {
    NotificationAuditEvent {
        id: runtime.new_id(),
        actor_id,
        notification_id,
        user_id,
        action: action.to_owned(),
        outcome: outcome.to_owned(),
        request_id: optional_text(request.request_id.clone()),
        ip_address: optional_text(request.ip_address.clone()),
        user_agent: optional_text(request.user_agent.clone()),
        metadata_json,
        occurred_at: runtime.now(),
    }
}

fn parse_uuid(value: &str, error: NotificationInvalidInput) -> Result<Uuid> {
    if value.trim().is_empty() {
        return Err(error.into());
    }
    Uuid::parse_str(value.trim()).map_err(|_| NotificationError::InvalidInput(error))
}

fn optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
