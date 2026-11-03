use crate::auth_runtime::AuthRuntimePort;
use crate::dto::auth_dto::{DeleteUserDto, DeleteUserResponseDto, UserDeletionSagaDto};
use crate::errors::AuthError;
use crate::errors::AuthResult as Result;
use crate::ports::{AuthAuditRepositoryPort, UserDeletionSagaPort, UserRepositoryPort};
use auth_domain::session::AuthAuditEvent;
use auth_domain::user::User;
use auth_domain::user_deletion::{BeginUserDeletion, UserDeletionTarget};
use chrono::Duration;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct DeleteUserUseCase<U, A, D> {
    user_repository: U,
    auth_audit_repository: A,
    deletion_repository: D,
    runtime: Arc<dyn AuthRuntimePort>,
}

impl<U, A, D> DeleteUserUseCase<U, A, D>
where
    U: UserRepositoryPort,
    A: AuthAuditRepositoryPort,
    D: UserDeletionSagaPort,
{
    pub fn new(
        user_repository: U,
        auth_audit_repository: A,
        deletion_repository: D,
        runtime: Arc<dyn AuthRuntimePort>,
    ) -> Self {
        Self {
            user_repository,
            auth_audit_repository,
            deletion_repository,
            runtime,
        }
    }

    pub async fn execute(&self, request: DeleteUserDto) -> Result<DeleteUserResponseDto> {
        let user_id = parse_uuid(&request.user_id, "User ID is required")?;
        let actor_id = parse_uuid(&request.actor_id, "Actor ID is required")?;
        let user = self
            .user_repository
            .find_by_id(user_id)
            .await?
            .map(User::from)
            .ok_or(AuthError::NotFound)?;

        if actor_id != user_id {
            self.record_forbidden_audit(actor_id, user_id, &request, &user.primary_email)
                .await?;
            return Err(AuthError::Forbidden);
        }

        if let Some(existing) = self.deletion_repository.find_by_user_id(user_id).await? {
            let saga = UserDeletionSagaDto::from(
                auth_domain::user_deletion::UserDeletionSaga::from(existing),
            );
            return Ok(DeleteUserResponseDto {
                user_id: user_id.to_string(),
                deleted: true,
                saga_id: saga.id.clone(),
                saga,
            });
        }

        let now = self.runtime.now();
        let retention_until = now + Duration::days(30);
        let saga_id = self.runtime.new_id();
        let audit = AuthAuditEvent {
            id: self.runtime.new_id(),
            actor_id: Some(actor_id),
            subject_user_id: Some(user_id),
            action: "USER_DELETION_REQUESTED".to_owned(),
            outcome: "SUCCESS".to_owned(),
            request_id: optional_text(request.request_id.clone()),
            ip_address: optional_text(request.ip_address.clone()),
            user_agent: optional_text(request.user_agent.clone()),
            metadata_json: json!({
                "email": user.primary_email,
                "sagaId": saga_id,
                "retentionUntil": retention_until
            })
            .to_string(),
            occurred_at: now,
        };
        let saga = self
            .deletion_repository
            .begin(
                (BeginUserDeletion {
                    saga_id,
                    user_id,
                    actor_id,
                    reason: "user_request".to_owned(),
                    retention_until,
                    targets: UserDeletionTarget::ALL.to_vec(),
                    audit,
                })
                .into(),
            )
            .await?
            .map(auth_domain::user_deletion::UserDeletionSaga::from)
            .ok_or(AuthError::NotFound)?;
        let saga = UserDeletionSagaDto::from(saga);
        Ok(DeleteUserResponseDto {
            user_id: user_id.to_string(),
            deleted: true,
            saga_id: saga.id.clone(),
            saga,
        })
    }

    async fn record_forbidden_audit(
        &self,
        actor_id: Uuid,
        user_id: Uuid,
        request: &DeleteUserDto,
        email: &str,
    ) -> Result<()> {
        self.auth_audit_repository
            .record(
                &(&AuthAuditEvent {
                    id: self.runtime.new_id(),
                    actor_id: Some(actor_id),
                    subject_user_id: Some(user_id),
                    action: "USER_DELETION_REQUESTED".to_owned(),
                    outcome: "FORBIDDEN".to_owned(),
                    request_id: optional_text(request.request_id.clone()),
                    ip_address: optional_text(request.ip_address.clone()),
                    user_agent: optional_text(request.user_agent.clone()),
                    metadata_json: json!({"email": email}).to_string(),
                    occurred_at: self.runtime.now(),
                })
                    .into(),
            )
            .await
    }
}

fn parse_uuid(value: &str, error: &'static str) -> Result<Uuid> {
    if value.trim().is_empty() {
        return Err(AuthError::Invariant(error));
    }
    Uuid::parse_str(value.trim()).map_err(|_| AuthError::Invariant(error))
}

fn optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
