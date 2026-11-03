use crate::application::dto::notification_dto::{
    ApplyUserDeletionDto, DeleteNotificationDto, NotificationCreateDto,
};
use crate::application::notification_creation::NotificationWriteDeadlineDto;
use crate::config::AppState;
use platform::actor_delegation::notification::{
    AdmittedNotification, NotificationBinding, NotificationDecision,
};
use prost::Message;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub mod proto {
    tonic::include_proto!("notification.v1");
}

#[derive(Clone)]
pub struct NotificationGrpcServer {
    state: Arc<AppState>,
    required_admission: bool,
}

impl NotificationGrpcServer {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            required_admission: false,
        }
    }

    /// Explicit selected mode. An omitted/misordered NotificationGuard denies
    /// the original write method before any application or persistence call.
    /// Existing operational configuration does not select this constructor.
    pub fn requiring_claimed_admission(state: Arc<AppState>) -> Self {
        Self {
            state,
            required_admission: true,
        }
    }
}

#[tonic::async_trait]
impl proto::notification_service_server::NotificationService for NotificationGrpcServer {
    async fn create_notification(
        &self,
        request: Request<proto::CreateNotificationRequest>,
    ) -> Result<Response<proto::CreateNotificationResponse>, Status> {
        let deadline = if self.required_admission {
            Some(validate_admitted_write(&request)?)
        } else {
            None
        };
        let request = request.into_inner();
        let dto = NotificationCreateDto {
            user_id: request.user_id,
            r#type: request.r#type,
            payload_json: request.payload_json,
            event_id: request.event_id,
            actor_id: request.actor_id,
            occurred_at: request.occurred_at,
        };
        let result = match deadline {
            Some(deadline) => {
                self.state
                    .create_notification
                    .execute_admitted(dto, deadline)
                    .await
            }
            None => self.state.create_notification.execute(dto).await,
        }
        .map_err(to_status)?;
        Ok(Response::new(proto::CreateNotificationResponse {
            notification: result.notification.map(to_proto),
            suppressed: result.suppressed,
        }))
    }

    async fn list_notifications(
        &self,
        request: Request<proto::ListNotificationsRequest>,
    ) -> Result<Response<proto::ListNotificationsResponse>, Status> {
        let request = request.into_inner();
        let page = self
            .state
            .list_notifications
            .execute(&request.user_id, &request.cursor, request.limit)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::ListNotificationsResponse {
            items: page.items.into_iter().map(to_proto).collect(),
            next_cursor: page.next_cursor.unwrap_or_default(),
        }))
    }

    async fn mark_notification_read(
        &self,
        request: Request<proto::MarkNotificationReadRequest>,
    ) -> Result<Response<proto::MarkNotificationReadResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .state
            .mark_read
            .execute(&request.user_id, &request.notification_id)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::MarkNotificationReadResponse {
            notification: Some(to_proto(result.notification)),
            updated: result.updated,
        }))
    }

    async fn mark_all_notifications_read(
        &self,
        request: Request<proto::MarkAllNotificationsReadRequest>,
    ) -> Result<Response<proto::MarkAllNotificationsReadResponse>, Status> {
        let updated_count = self
            .state
            .mark_all_read
            .execute(&request.into_inner().user_id)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::MarkAllNotificationsReadResponse {
            updated_count,
        }))
    }

    async fn delete_notification(
        &self,
        request: Request<proto::DeleteNotificationRequest>,
    ) -> Result<Response<proto::DeleteNotificationResponse>, Status> {
        // User-facing mutation: only the edge may request it.
        platform::grpc_auth::authorize_call(
            &request,
            platform::grpc_auth::RpcAction::UserMutation,
        )?;
        let request = request.into_inner();
        let response = self
            .state
            .delete_notification
            .execute(DeleteNotificationDto {
                user_id: request.user_id,
                notification_id: request.notification_id,
                actor_id: request.actor_id,
                request_id: empty_string_to_option(request.request_id),
                ip_address: empty_string_to_option(request.ip_address),
                user_agent: empty_string_to_option(request.user_agent),
            })
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::DeleteNotificationResponse {
            notification_id: response.notification_id,
            deleted: response.deleted,
        }))
    }

    async fn apply_user_deletion(
        &self,
        request: Request<proto::ApplyUserDeletionRequest>,
    ) -> Result<Response<proto::ApplyUserDeletionResponse>, Status> {
        // Irreversible account cleanup belongs to the deletion saga alone.
        platform::grpc_auth::authorize_call(
            &request,
            platform::grpc_auth::RpcAction::ApplyUserDeletion,
        )?;
        let request = request.into_inner();
        let response = self
            .state
            .apply_user_deletion
            .execute(ApplyUserDeletionDto {
                saga_id: request.saga_id,
                user_id: request.user_id,
                actor_id: request.actor_id,
                retention_until: request.retention_until,
            })
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::ApplyUserDeletionResponse {
            applied: response.applied,
        }))
    }
}

/// Complete selected service composition; the existing outer authenticated
/// workload/actor method policy must wrap this service as usual. Neither this
/// function nor the required constructor activates a runtime listener.
pub fn claimed_notification_service(
    state: Arc<AppState>,
    verifier: platform::actor_delegation::notification::NotificationVerifier,
    admission: Arc<dyn platform::actor_delegation::notification::NotificationAdmissionCheck>,
) -> anyhow::Result<
    platform::actor_delegation::notification::NotificationGuard<
        proto::notification_service_server::NotificationServiceServer<NotificationGrpcServer>,
    >,
> {
    platform::actor_delegation::notification::NotificationGuard::new(
        proto::notification_service_server::NotificationServiceServer::new(
            NotificationGrpcServer::requiring_claimed_admission(state),
        ),
        verifier,
        admission,
    )
}

#[allow(clippy::result_large_err)]
fn validate_admitted_write(
    request: &Request<proto::CreateNotificationRequest>,
) -> Result<NotificationWriteDeadlineDto, Status> {
    let denied = || Status::permission_denied("Notification admission required");
    let admitted = request
        .extensions()
        .get::<AdmittedNotification>()
        .ok_or_else(denied)?;
    if !matches!(admitted.decision(), NotificationDecision::Allowed { .. })
        || chrono::Utc::now().timestamp_micros() >= admitted.expires_micros()
        || !matches!(admitted.binding(), NotificationBinding::Delivery(_))
    {
        return Err(denied());
    }
    // Reconstruct from the generated original request. A cloned sealed
    // extension cannot authorize a different decoded request or defaulted IDs.
    let message = request.get_ref().encode_to_vec();
    let mut body = vec![0];
    body.extend_from_slice(&(message.len() as u32).to_be_bytes());
    body.extend_from_slice(&message);
    if admitted.binding().frame().map_err(|_| denied())? != body {
        return Err(denied());
    }
    Ok(NotificationWriteDeadlineDto {
        expires_at: chrono::DateTime::from_timestamp_micros(admitted.expires_micros())
            .ok_or_else(denied)?,
    })
}

fn to_proto(
    notification: crate::application::dto::notification_dto::NotificationDto,
) -> proto::Notification {
    proto::Notification {
        id: notification.id,
        user_id: notification.user_id,
        r#type: notification.r#type,
        payload_json: notification.payload_json,
        created_at: notification.created_at,
        read_at: notification.read_at,
        event_id: notification.event_id,
        actor_id: notification.actor_id,
        occurred_at: notification.occurred_at,
    }
}

fn to_status(error: crate::application::errors::NotificationError) -> Status {
    use crate::application::failure_presentation::{present, NotificationFailureKind as Kind};
    let failure = present(error);
    match failure.kind {
        Kind::InvalidArgument => Status::invalid_argument(failure.message),
        Kind::NotFound => Status::not_found(failure.message),
        Kind::PermissionDenied => Status::permission_denied(failure.message),
        Kind::Unavailable => Status::unavailable(failure.message),
        Kind::Internal => Status::internal(failure.message),
    }
}

fn empty_string_to_option(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::to_status;
    use crate::application::errors::{NotificationError as Error, NotificationInvalidInput};
    use tonic::Code;

    #[test]
    fn rpc_status_preserves_policy_categories_and_hides_backend_failures() {
        for (error, code, message) in [
            (
                Error::InvalidInput(NotificationInvalidInput::InvalidCursor),
                Code::InvalidArgument,
                "Invalid cursor format",
            ),
            (Error::NotFound, Code::NotFound, "Notification not found"),
            (Error::Forbidden, Code::PermissionDenied, "Forbidden"),
            (
                Error::DependencyUnavailable,
                Code::Unavailable,
                "Service unavailable",
            ),
            (Error::Unexpected, Code::Internal, "Internal error"),
        ] {
            let status = to_status(error);
            assert_eq!(status.code(), code);
            assert_eq!(status.message(), message);
            assert!(status.details().is_empty());
            assert!(status.metadata().is_empty());
        }
    }
}
