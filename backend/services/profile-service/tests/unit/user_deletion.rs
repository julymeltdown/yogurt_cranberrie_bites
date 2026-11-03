use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use profile_service::application::dto::profile_dto::ApplyUserDeletionDto;
use profile_service::application::dto::repository_dto::{
    ProfileAuditEventDto, ProfileRecordDto, ProfileVisibilityDto,
};
use profile_service::application::errors::ProfileResult as Result;
use profile_service::application::ports::ProfileRepositoryPort;
use profile_service::application::use_cases::apply_user_deletion::ApplyUserDeletionUseCase;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

type UserDeletionCall = (Uuid, Uuid, Uuid, DateTime<Utc>);

#[derive(Clone, Default)]
struct RecordingRepository {
    calls: Arc<Mutex<Vec<UserDeletionCall>>>,
}

#[async_trait]
impl ProfileRepositoryPort for RecordingRepository {
    async fn find_username_owner(&self, _username: &str) -> Result<Option<Uuid>> {
        Ok(None)
    }

    async fn find_by_user_id(&self, _user_id: Uuid) -> Result<Option<ProfileRecordDto>> {
        Ok(None)
    }
    async fn find_by_username(&self, _username: &str) -> Result<Option<ProfileRecordDto>> {
        Ok(None)
    }
    async fn find_by_user_ids(&self, _user_ids: &[Uuid]) -> Result<Vec<ProfileRecordDto>> {
        Ok(vec![])
    }
    async fn save(&self, profile: &ProfileRecordDto) -> Result<ProfileRecordDto> {
        Ok(profile.clone())
    }
    async fn update_visibility(
        &self,
        _user_id: Uuid,
        _visibility: ProfileVisibilityDto,
    ) -> Result<Option<ProfileRecordDto>> {
        Ok(None)
    }
    async fn search(
        &self,
        _needle: &str,
        _cursor: Option<&str>,
        _limit: i32,
    ) -> Result<(Vec<ProfileRecordDto>, Option<String>)> {
        Ok((vec![], None))
    }
    async fn soft_delete_profile(
        &self,
        _user_id: Uuid,
        _actor_id: Uuid,
        _reason: &str,
        _retention_until: DateTime<Utc>,
        _audit: &profile_service::application::dto::profile_dto::ProfileDeletionAuditDto,
    ) -> Result<bool> {
        Ok(false)
    }
    async fn purge_expired_profiles(
        &self,
        _cutoff: DateTime<Utc>,
        _actor_id: Uuid,
        _request_id: Option<String>,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
        _dry_run: bool,
    ) -> Result<Vec<Uuid>> {
        Ok(vec![])
    }
    async fn record_audit_event(&self, _event: &ProfileAuditEventDto) -> Result<()> {
        Ok(())
    }
    async fn apply_user_deletion(
        &self,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> Result<()> {
        self.calls
            .lock()
            .unwrap()
            .push((saga_id, user_id, actor_id, retention_until));
        Ok(())
    }
}

#[tokio::test]
async fn repeated_user_deletion_application_is_idempotent_and_preserves_deadline() {
    let repository = RecordingRepository::default();
    let use_case = ApplyUserDeletionUseCase::new(repository.clone());
    let saga_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let retention = Utc.with_ymd_and_hms(2026, 9, 2, 12, 0, 0).unwrap();
    let request = ApplyUserDeletionDto {
        saga_id: saga_id.to_string(),
        user_id: user_id.to_string(),
        actor_id: user_id.to_string(),
        retention_until: retention.to_rfc3339(),
    };

    assert!(
        use_case
            .execute(request.clone())
            .await
            .expect("first")
            .applied
    );
    assert!(use_case.execute(request).await.expect("second").applied);

    let calls = repository.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0], (saga_id, user_id, user_id, retention));
    assert_eq!(calls[1], calls[0]);
}
