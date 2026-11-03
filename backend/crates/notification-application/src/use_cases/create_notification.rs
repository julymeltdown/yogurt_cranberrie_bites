use crate::dto::notification_dto::{
    NotificationCreateDto, NotificationCreateResultDto, NotificationDto,
};
use crate::errors::{NotificationError, NotificationInvalidInput, NotificationResult as Result};
use crate::notification_contracts::NotificationCreateOutcome;
use crate::notification_creation::{
    NotificationAdmittedCreatePort, NotificationCreatePort, NotificationWriteDeadlineDto,
};
use crate::notification_runtime::NotificationRuntimePort;
use chrono::{DateTime, Utc};
use notification_domain::notification::Notification;
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct CreateNotificationUseCase<R> {
    repository: R,
    runtime: Arc<dyn NotificationRuntimePort>,
}

impl<R> CreateNotificationUseCase<R>
where
    R: NotificationCreatePort,
{
    pub fn new(repository: R, runtime: Arc<dyn NotificationRuntimePort>) -> Self {
        Self {
            repository,
            runtime,
        }
    }

    pub async fn execute(
        &self,
        request: NotificationCreateDto,
    ) -> Result<NotificationCreateResultDto> {
        let notification = self.build_notification(request)?;
        self.repository
            .create_unless_suppressed(&notification.into())
            .await
            .map(result_dto)
    }

    fn build_notification(&self, request: NotificationCreateDto) -> Result<Notification> {
        let user_id = parse_uuid(&request.user_id, NotificationInvalidInput::UserIdRequired)?;
        let notification_type = normalize_type(&request.r#type)?;
        let event_id = if request.event_id.trim().is_empty() {
            self.runtime.new_id()
        } else {
            parse_uuid(&request.event_id, NotificationInvalidInput::InvalidEventId)?
        };
        let actor_id = if request.actor_id.trim().is_empty() {
            None
        } else {
            Some(parse_uuid(
                &request.actor_id,
                NotificationInvalidInput::InvalidActorId,
            )?)
        };
        let occurred_at = if request.occurred_at.trim().is_empty() {
            self.runtime.now()
        } else {
            DateTime::parse_from_rfc3339(request.occurred_at.trim())
                .map(|value| value.with_timezone(&Utc))
                .map_err(|_| {
                    NotificationError::InvalidInput(NotificationInvalidInput::InvalidOccurredAt)
                })?
        };

        Ok(Notification {
            id: self.runtime.new_id(),
            user_id,
            notification_type,
            payload_json: request.payload_json,
            created_at: self.runtime.now(),
            read_at: None,
            event_id,
            actor_id,
            occurred_at,
        })
    }
}
impl<R: NotificationCreatePort + NotificationAdmittedCreatePort> CreateNotificationUseCase<R> {
    pub async fn execute_admitted(
        &self,
        request: NotificationCreateDto,
        deadline: NotificationWriteDeadlineDto,
    ) -> Result<NotificationCreateResultDto> {
        if self.runtime.now() >= deadline.expires_at {
            return Err(NotificationError::AdmissionExpired);
        }
        let notification = self.build_notification(request)?;
        self.repository
            .create_admitted(&notification.into(), deadline)
            .await
            .map(result_dto)
    }
}
fn result_dto(outcome: NotificationCreateOutcome) -> NotificationCreateResultDto {
    match outcome {
        NotificationCreateOutcome::Created(value) | NotificationCreateOutcome::Existing(value) => {
            NotificationCreateResultDto {
                notification: Some(NotificationDto::from(value)),
                suppressed: false,
            }
        }
        NotificationCreateOutcome::Suppressed(_) => NotificationCreateResultDto {
            notification: None,
            suppressed: true,
        },
    }
}

fn normalize_type(value: &str) -> Result<String> {
    let normalized = value.trim().to_uppercase();
    if normalized.is_empty() {
        return Err(NotificationInvalidInput::TypeRequired.into());
    }
    let allowed: HashSet<&'static str> = HashSet::from([
        "POST_CREATED",
        "POST_LIKED",
        "POST_COMMENTED",
        "COMMENT",
        "FRIEND_REQUEST",
        "FRIEND_ACCEPTED",
        "SYSTEM",
    ]);
    if !allowed.contains(normalized.as_str()) {
        return Err(NotificationInvalidInput::UnsupportedType.into());
    }
    Ok(normalized)
}

fn parse_uuid(value: &str, error: NotificationInvalidInput) -> Result<Uuid> {
    if value.trim().is_empty() {
        return Err(error.into());
    }
    Uuid::parse_str(value.trim()).map_err(|_| NotificationError::InvalidInput(error))
}
