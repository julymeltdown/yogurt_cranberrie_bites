use crate::dto::profile_dto::{
    DeleteProfileDto, DeleteProfileResponseDto, ProfileDeletionAuditDto,
};
use crate::dto::repository_dto::ProfileAuditEventDto;
use crate::errors::{ProfileError, ProfileResult as Result};
use crate::ports::ProfileRepositoryPort;
use crate::profile_runtime::ProfileRuntimePort;
use chrono::Duration;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct DeleteProfileUseCase<R> {
    repository: R,
    runtime: Arc<dyn ProfileRuntimePort>,
}

impl<R> DeleteProfileUseCase<R>
where
    R: ProfileRepositoryPort,
{
    pub fn new(repository: R, runtime: Arc<dyn ProfileRuntimePort>) -> Self {
        Self {
            repository,
            runtime,
        }
    }

    pub async fn execute(&self, request: DeleteProfileDto) -> Result<DeleteProfileResponseDto> {
        let user_id = parse_uuid(&request.user_id, "User ID is required")?;
        let actor_id = parse_uuid(&request.actor_id, "Actor ID is required")?;
        let profile = self
            .repository
            .find_by_user_id(user_id)
            .await?
            .ok_or(ProfileError::ProfileNotFound)?;

        if actor_id != user_id {
            self.record_audit(
                Some(actor_id),
                Some(user_id),
                "PROFILE_DELETED",
                "FORBIDDEN",
                &request,
                json!({"username": profile.username}).to_string(),
            )
            .await?;
            return Err(ProfileError::Forbidden);
        }

        let observed_at = self.runtime.now();
        let retention_until = observed_at + Duration::days(30);
        let audit = ProfileDeletionAuditDto {
            id: self.runtime.new_id(),
            request_id: optional_text(request.request_id.clone()),
            ip_address: optional_text(request.ip_address.clone()),
            user_agent: optional_text(request.user_agent.clone()),
            metadata_json: json!({
                "username": profile.username,
                "retentionUntil": retention_until
            })
            .to_string(),
            occurred_at: observed_at,
        };
        let deleted = self
            .repository
            .soft_delete_profile(user_id, actor_id, "user_request", retention_until, &audit)
            .await?;
        if !deleted {
            return Err(ProfileError::ProfileNotFound);
        }

        Ok(DeleteProfileResponseDto {
            user_id: user_id.to_string(),
            deleted: true,
            account_deleted: false,
        })
    }

    async fn record_audit(
        &self,
        actor_id: Option<Uuid>,
        profile_user_id: Option<Uuid>,
        action: &str,
        outcome: &str,
        request: &DeleteProfileDto,
        metadata_json: String,
    ) -> Result<()> {
        self.repository
            .record_audit_event(&ProfileAuditEventDto {
                id: self.runtime.new_id(),
                actor_id,
                profile_user_id,
                action: action.to_owned(),
                outcome: outcome.to_owned(),
                request_id: optional_text(request.request_id.clone()),
                ip_address: optional_text(request.ip_address.clone()),
                user_agent: optional_text(request.user_agent.clone()),
                metadata_json,
                occurred_at: self.runtime.now(),
            })
            .await
    }
}

fn parse_uuid(value: &str, error: &'static str) -> Result<Uuid> {
    if value.trim().is_empty() {
        return Err(ProfileError::InvalidInput(error));
    }
    Uuid::parse_str(value.trim()).map_err(|_| ProfileError::InvalidInput(error))
}

fn optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
