use crate::profile_runtime::AdvancingProfileRuntime;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use profile_service::adapters::r#in::messaging::retention_purge_worker::ProfileRetentionPurgeWorker;
use profile_service::application::dto::repository_dto::{
    ProfileAuditEventDto, ProfileRecordDto, ProfileVisibilityDto,
};
use profile_service::application::errors::{ProfileError, ProfileResult as Result};
use profile_service::application::ports::ProfileRepositoryPort;
use profile_service::application::use_cases::purge_expired_profiles::PurgeExpiredProfilesUseCase;
use profile_service::config::ProfileMetrics;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[tokio::test]
async fn retention_worker_tick_purges_with_worker_metadata_and_records_success_metrics() {
    let actor_id = uuid("11111111-1111-1111-1111-111111111111");
    let purged_id = uuid("22222222-2222-2222-2222-222222222222");
    let repository = Arc::new(FakeProfileRepository::default());
    *repository.purge_result.lock().expect("purge result") = vec![actor_id, purged_id];
    let metrics = Arc::new(ProfileMetrics::default());
    let worker = ProfileRetentionPurgeWorker::new(
        PurgeExpiredProfilesUseCase::new(
            Arc::clone(&repository),
            Arc::new(AdvancingProfileRuntime::default()),
        ),
        actor_id.to_string(),
        Arc::clone(&metrics),
    );

    let purged_count = worker.tick().await.expect("worker tick");

    assert_eq!(purged_count, 2);
    assert_eq!(
        *repository.purge_calls.lock().expect("purge calls"),
        vec![PurgeCall {
            actor_id,
            request_id: Some("profile-retention-worker".into()),
            ip_address: None,
            user_agent: Some("profile-retention-worker".into()),
            dry_run: false,
        }]
    );
    let rendered = metrics.render_prometheus();
    assert!(rendered.contains("profile_retention_purge_runs_total{outcome=\"success\"} 1"));
    assert!(rendered.contains("profile_retention_purged_profiles_total{outcome=\"success\"} 2"));
}

#[tokio::test]
async fn retention_worker_tick_records_failure_metrics_when_purge_fails() {
    let actor_id = uuid("33333333-3333-3333-3333-333333333333");
    let repository = Arc::new(FakeProfileRepository::default());
    *repository.purge_error.lock().expect("purge error") = Some("storage unavailable".into());
    let metrics = Arc::new(ProfileMetrics::default());
    let worker = ProfileRetentionPurgeWorker::new(
        PurgeExpiredProfilesUseCase::new(
            Arc::clone(&repository),
            Arc::new(AdvancingProfileRuntime::default()),
        ),
        actor_id.to_string(),
        Arc::clone(&metrics),
    );

    let error = worker.tick().await.expect_err("worker tick fails");

    assert_eq!(error.to_string(), "Profile persistence operation failed");
    let rendered = metrics.render_prometheus();
    assert!(rendered.contains("profile_retention_purge_runs_total{outcome=\"failure\"} 1"));
    assert!(rendered.contains("profile_retention_purged_profiles_total{outcome=\"failure\"} 0"));
}

#[derive(Default)]
struct FakeProfileRepository {
    purge_result: Mutex<Vec<Uuid>>,
    purge_error: Mutex<Option<String>>,
    purge_calls: Mutex<Vec<PurgeCall>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PurgeCall {
    actor_id: Uuid,
    request_id: Option<String>,
    ip_address: Option<String>,
    user_agent: Option<String>,
    dry_run: bool,
}

#[async_trait]
impl ProfileRepositoryPort for FakeProfileRepository {
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
        Ok(Vec::new())
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
        Ok((Vec::new(), None))
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
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
    ) -> Result<Vec<Uuid>> {
        self.purge_calls
            .lock()
            .expect("purge calls")
            .push(PurgeCall {
                actor_id,
                request_id,
                ip_address,
                user_agent,
                dry_run,
            });
        if self.purge_error.lock().expect("purge error").is_some() {
            return Err(ProfileError::Persistence);
        }
        Ok(self.purge_result.lock().expect("purge result").clone())
    }

    async fn record_audit_event(&self, _event: &ProfileAuditEventDto) -> Result<()> {
        Ok(())
    }
}

fn uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).expect("valid uuid")
}
