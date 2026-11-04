use crate::dto::mutation_dto::{MutationReceiptStatusDto, ProfileMutationOperationDto};
use crate::dto::profile_dto::AvatarDto;
use crate::dto::profile_dto::{ProfileDeletionAuditDto, ProfileDto};
use crate::dto::repository_dto::{ProfileAuditEventDto, ProfileRecordDto, ProfileVisibilityDto};
use crate::errors::{ProfileError, ProfileResult as Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use uuid::Uuid;

#[async_trait]
pub trait ProfileRepositoryPort: Send + Sync {
    async fn find_by_user_id(&self, user_id: Uuid) -> Result<Option<ProfileRecordDto>>;
    async fn find_by_username(&self, username: &str) -> Result<Option<ProfileRecordDto>>;

    /// The user holding a username, including a soft-deleted holder.
    ///
    /// A deleted profile keeps its row and the username column is unique, so the
    /// name stays reserved. Checking ownership through the ordinary lookup, which
    /// excludes deleted rows, misses that reservation and lets the insert fail on
    /// the index instead of being reported as a conflict.
    async fn find_username_owner(&self, username: &str) -> Result<Option<Uuid>>;
    async fn find_by_user_ids(&self, user_ids: &[Uuid]) -> Result<Vec<ProfileRecordDto>>;
    async fn save(&self, profile: &ProfileRecordDto) -> Result<ProfileRecordDto>;
    async fn update_visibility(
        &self,
        user_id: Uuid,
        visibility: ProfileVisibilityDto,
    ) -> Result<Option<ProfileRecordDto>>;
    async fn lookup_privacy_mutation(
        &self,
        operation: &ProfileMutationOperationDto,
    ) -> Result<Option<ProfileDto>> {
        self.lookup_profile_mutation(operation).await
    }
    async fn lookup_profile_mutation(
        &self,
        _operation: &ProfileMutationOperationDto,
    ) -> Result<Option<ProfileDto>> {
        Err(ProfileError::Persistence)
    }
    async fn update_visibility_with_receipt(
        &self,
        _user_id: Uuid,
        _visibility: ProfileVisibilityDto,
        _operation: &ProfileMutationOperationDto,
    ) -> Result<Option<ProfileDto>> {
        Err(ProfileError::Persistence)
    }
    async fn update_profile_with_receipt(
        &self,
        _user_id: Uuid,
        _username: &str,
        _nickname: &str,
        _avatar_url: &str,
        _bio: &str,
        _operation: &ProfileMutationOperationDto,
    ) -> Result<Option<ProfileDto>> {
        Err(ProfileError::Persistence)
    }
    async fn mutation_receipt_status(
        &self,
        _operation_id: Uuid,
    ) -> Result<Option<MutationReceiptStatusDto>> {
        Err(ProfileError::Persistence)
    }
    async fn search(
        &self,
        needle: &str,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<(Vec<ProfileRecordDto>, Option<String>)>;
    async fn soft_delete_profile(
        &self,
        user_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
        audit: &ProfileDeletionAuditDto,
    ) -> Result<bool>;
    async fn purge_expired_profiles(
        &self,
        cutoff: DateTime<Utc>,
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
    ) -> Result<Vec<Uuid>>;
    async fn record_audit_event(&self, event: &ProfileAuditEventDto) -> Result<()>;
    async fn apply_user_deletion(
        &self,
        _saga_id: Uuid,
        _user_id: Uuid,
        _actor_id: Uuid,
        _retention_until: DateTime<Utc>,
    ) -> Result<()> {
        Err(ProfileError::Persistence)
    }
}

#[async_trait]
pub trait ProfileMutationReceiptMaintenancePort: Send + Sync {
    async fn purge_expired_mutation_outcomes(&self, limit: i32) -> Result<i64>;

    /// Old unit success cannot be upgraded into owning COMMIT evidence.
    async fn apply_user_deletion_with_receipt(
        &self,
        _saga_id: Uuid,
        _user_id: Uuid,
        _actor_id: Uuid,
        _retention_until: DateTime<Utc>,
    ) -> Result<crate::dto::profile_dto::CommittedDeletionReceiptDto> {
        Err(crate::errors::ProfileError::ReceiptUnavailable)
    }
}

#[async_trait]
pub trait AvatarRepositoryPort: Send + Sync {
    async fn save(&self, user_id: Uuid, avatar: &AvatarDto) -> Result<()>;
    async fn find(&self, user_id: Uuid) -> Result<Option<AvatarDto>>;
}

#[async_trait]
pub trait AvatarProcessorPort: Send + Sync {
    async fn process(
        &self,
        content: Vec<u8>,
        content_type: &str,
        filename: &str,
    ) -> Result<AvatarDto>;
}

#[async_trait]
impl<T> ProfileRepositoryPort for Arc<T>
where
    T: ProfileRepositoryPort + ?Sized,
{
    async fn find_by_user_id(&self, user_id: Uuid) -> Result<Option<ProfileRecordDto>> {
        (**self).find_by_user_id(user_id).await
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<ProfileRecordDto>> {
        (**self).find_by_username(username).await
    }

    async fn find_username_owner(&self, username: &str) -> Result<Option<Uuid>> {
        (**self).find_username_owner(username).await
    }

    async fn find_by_user_ids(&self, user_ids: &[Uuid]) -> Result<Vec<ProfileRecordDto>> {
        (**self).find_by_user_ids(user_ids).await
    }

    async fn save(&self, profile: &ProfileRecordDto) -> Result<ProfileRecordDto> {
        (**self).save(profile).await
    }

    async fn update_visibility(
        &self,
        user_id: Uuid,
        visibility: ProfileVisibilityDto,
    ) -> Result<Option<ProfileRecordDto>> {
        (**self).update_visibility(user_id, visibility).await
    }

    async fn lookup_privacy_mutation(
        &self,
        operation: &ProfileMutationOperationDto,
    ) -> Result<Option<ProfileDto>> {
        (**self).lookup_privacy_mutation(operation).await
    }

    async fn lookup_profile_mutation(
        &self,
        operation: &ProfileMutationOperationDto,
    ) -> Result<Option<ProfileDto>> {
        (**self).lookup_profile_mutation(operation).await
    }

    async fn update_visibility_with_receipt(
        &self,
        user_id: Uuid,
        visibility: ProfileVisibilityDto,
        operation: &ProfileMutationOperationDto,
    ) -> Result<Option<ProfileDto>> {
        (**self)
            .update_visibility_with_receipt(user_id, visibility, operation)
            .await
    }

    async fn update_profile_with_receipt(
        &self,
        user_id: Uuid,
        username: &str,
        nickname: &str,
        avatar_url: &str,
        bio: &str,
        operation: &ProfileMutationOperationDto,
    ) -> Result<Option<ProfileDto>> {
        (**self)
            .update_profile_with_receipt(user_id, username, nickname, avatar_url, bio, operation)
            .await
    }

    async fn mutation_receipt_status(
        &self,
        operation_id: Uuid,
    ) -> Result<Option<MutationReceiptStatusDto>> {
        (**self).mutation_receipt_status(operation_id).await
    }

    async fn search(
        &self,
        needle: &str,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<(Vec<ProfileRecordDto>, Option<String>)> {
        (**self).search(needle, cursor, limit).await
    }

    async fn soft_delete_profile(
        &self,
        user_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
        audit: &ProfileDeletionAuditDto,
    ) -> Result<bool> {
        (**self)
            .soft_delete_profile(user_id, actor_id, reason, retention_until, audit)
            .await
    }

    async fn record_audit_event(&self, event: &ProfileAuditEventDto) -> Result<()> {
        (**self).record_audit_event(event).await
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
    ) -> Result<crate::dto::profile_dto::CommittedDeletionReceiptDto> {
        (**self)
            .apply_user_deletion_with_receipt(saga_id, user_id, actor_id, retention_until)
            .await
    }

    async fn purge_expired_profiles(
        &self,
        cutoff: DateTime<Utc>,
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
    ) -> Result<Vec<Uuid>> {
        (**self)
            .purge_expired_profiles(
                cutoff, actor_id, request_id, ip_address, user_agent, dry_run,
            )
            .await
    }
}

#[async_trait]
impl<T> ProfileMutationReceiptMaintenancePort for Arc<T>
where
    T: ProfileMutationReceiptMaintenancePort + ?Sized,
{
    async fn purge_expired_mutation_outcomes(&self, limit: i32) -> Result<i64> {
        (**self).purge_expired_mutation_outcomes(limit).await
    }
}

#[async_trait]
impl<T> AvatarRepositoryPort for Arc<T>
where
    T: AvatarRepositoryPort + ?Sized,
{
    async fn save(&self, user_id: Uuid, avatar: &AvatarDto) -> Result<()> {
        (**self).save(user_id, avatar).await
    }

    async fn find(&self, user_id: Uuid) -> Result<Option<AvatarDto>> {
        (**self).find(user_id).await
    }
}
