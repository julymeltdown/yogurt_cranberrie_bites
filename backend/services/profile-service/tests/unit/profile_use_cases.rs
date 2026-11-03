use crate::profile_runtime::{instant, AdvancingProfileRuntime};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use profile_service::application::dto::profile_dto::{
    DeleteProfileDto, PurgeExpiredProfilesDto, UpdateProfileDto, UpdateProfilePrivacyDto,
};
use profile_service::application::dto::repository_dto::{
    ProfileAuditEventDto, ProfileRecordDto, ProfileVisibilityDto,
};
use profile_service::application::errors::ProfileResult as Result;
use profile_service::application::ports::ProfileRepositoryPort;
use profile_service::application::use_cases::delete_profile::DeleteProfileUseCase;
use profile_service::application::use_cases::get_profile::GetProfileUseCase;
use profile_service::application::use_cases::get_profile_by_username::GetProfileByUsernameUseCase;
use profile_service::application::use_cases::get_profiles_batch::GetProfilesBatchUseCase;
use profile_service::application::use_cases::purge_expired_profiles::PurgeExpiredProfilesUseCase;
use profile_service::application::use_cases::search_profiles::SearchProfilesUseCase;
use profile_service::application::use_cases::update_privacy::UpdateProfilePrivacyUseCase;
use profile_service::application::use_cases::update_profile::UpdateProfileUseCase;
use profile_service::domain::profile::{AccountVisibility, Profile, ProfileAuditEvent};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[tokio::test]
async fn missing_profile_returns_not_found_without_persisting() {
    let user_id = uuid("00000000-0000-0000-0000-000000000101");
    let repository = Arc::new(FakeProfileRepository::default());
    let use_case = GetProfileUseCase::new(Arc::clone(&repository));

    let error = use_case
        .execute(user_id)
        .await
        .expect_err("missing profile must not be synthesized");

    assert_eq!(error.to_string(), "Profile not found");
    assert_eq!(
        *repository.find_by_user_id_calls.lock().unwrap(),
        vec![user_id]
    );
    assert!(repository.saved_profiles.lock().unwrap().is_empty());
}

#[tokio::test]
async fn get_profile_by_username_normalizes_before_lookup() {
    let user_id = uuid("00000000-0000-0000-0000-000000000102");
    let repository = Arc::new(FakeProfileRepository::with_profiles(vec![profile(
        user_id, "alice", 4, 5, 6,
    )]));
    let use_case = GetProfileByUsernameUseCase::new(Arc::clone(&repository));

    let response = use_case
        .execute("  ALICE ")
        .await
        .expect("find normalized username");

    assert_eq!(response.user_id, user_id.to_string());
    assert_eq!(response.username, "alice");
    assert_eq!(
        *repository.find_by_username_calls.lock().unwrap(),
        vec!["alice".to_owned()]
    );
}

#[tokio::test]
async fn batch_profiles_preserves_requested_order_and_omits_missing_profiles() {
    let present = uuid("00000000-0000-0000-0000-000000000103");
    let missing = uuid("00000000-0000-0000-0000-000000000104");
    let repository = Arc::new(FakeProfileRepository::with_profiles(vec![profile(
        present, "present", 7, 8, 9,
    )]));
    let use_case = GetProfilesBatchUseCase::new(Arc::clone(&repository));

    let response = use_case
        .execute(&[missing, present])
        .await
        .expect("batch profiles");

    assert_eq!(response.len(), 1);
    assert_eq!(response[0].user_id, present.to_string());
    assert_eq!(response[0].username, "present");
    assert_eq!(
        *repository.find_by_user_ids_calls.lock().unwrap(),
        vec![vec![missing, present]]
    );
}

#[tokio::test]
async fn batch_profiles_rejects_more_than_one_hundred_ids_without_lookup() {
    let repository = Arc::new(FakeProfileRepository::default());
    let use_case = GetProfilesBatchUseCase::new(Arc::clone(&repository));
    let user_ids = (0..101).map(|_| Uuid::new_v4()).collect::<Vec<_>>();

    let error = use_case
        .execute(&user_ids)
        .await
        .expect_err("oversized profile batch must be rejected");

    assert_eq!(
        error.to_string(),
        "Profile batch supports at most 100 user IDs"
    );
    assert!(repository.find_by_user_ids_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn batch_profiles_deduplicates_requested_ids() {
    let present = uuid("00000000-0000-0000-0000-000000000118");
    let repository = Arc::new(FakeProfileRepository::with_profiles(vec![profile(
        present, "present", 1, 2, 3,
    )]));
    let use_case = GetProfilesBatchUseCase::new(Arc::clone(&repository));

    let response = use_case
        .execute(&[present, present])
        .await
        .expect("deduplicated batch");

    assert_eq!(response.len(), 1);
    assert_eq!(
        *repository.find_by_user_ids_calls.lock().unwrap(),
        vec![vec![present]]
    );
}

#[tokio::test]
async fn batch_profiles_omits_missing_ids_deduplicates_first_occurrences_and_preserves_surviving_order(
) {
    let first = uuid("00000000-0000-0000-0000-000000000119");
    let second = uuid("00000000-0000-0000-0000-000000000120");
    let missing = uuid("00000000-0000-0000-0000-000000000121");
    let repository = Arc::new(FakeProfileRepository::with_profiles(vec![
        profile(first, "first", 1, 2, 3),
        profile(second, "second", 4, 5, 6),
    ]));
    let use_case = GetProfilesBatchUseCase::new(Arc::clone(&repository));

    let response = use_case
        .execute(&[second, missing, first, second])
        .await
        .expect("batch profiles");

    assert_eq!(
        response
            .iter()
            .map(|profile| profile.user_id.clone())
            .collect::<Vec<_>>(),
        vec![second.to_string(), first.to_string()]
    );
    assert_eq!(
        *repository.find_by_user_ids_calls.lock().unwrap(),
        vec![vec![second, missing, first]]
    );
}

#[tokio::test]
async fn search_profiles_caps_limit_at_one_hundred() {
    let repository = Arc::new(FakeProfileRepository::default());
    let use_case = SearchProfilesUseCase::new(Arc::clone(&repository));

    use_case
        .execute("alice", None, 10_000)
        .await
        .expect("bounded search");

    assert_eq!(repository.search_calls.lock().unwrap()[0].limit, 100);
}

#[tokio::test]
async fn empty_batch_profiles_returns_empty_without_repository_lookup() {
    let repository = Arc::new(FakeProfileRepository::default());
    let use_case = GetProfilesBatchUseCase::new(Arc::clone(&repository));

    let response = use_case.execute(&[]).await.expect("empty batch");

    assert!(response.is_empty());
    assert!(repository.find_by_user_ids_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn search_profiles_normalizes_query_and_defaults_non_positive_limit() {
    let user_id = uuid("00000000-0000-0000-0000-000000000105");
    let repository = Arc::new(FakeProfileRepository::with_search_results(
        vec![profile(user_id, "search_hit", 1, 2, 3)],
        Some("next-cursor".to_owned()),
    ));
    let use_case = SearchProfilesUseCase::new(Arc::clone(&repository));

    let response = use_case
        .execute("  Search_Hit  ", Some("cursor-1"), 0)
        .await
        .expect("search profiles");

    assert_eq!(response.profiles.len(), 1);
    assert_eq!(response.profiles[0].username, "search_hit");
    assert_eq!(response.next_cursor.as_deref(), Some("next-cursor"));
    assert_eq!(
        *repository.search_calls.lock().unwrap(),
        vec![SearchCall {
            needle: "search_hit".to_owned(),
            cursor: Some("cursor-1".to_owned()),
            limit: 10,
        }]
    );
}

#[tokio::test]
async fn blank_search_returns_empty_without_repository_lookup() {
    let repository = Arc::new(FakeProfileRepository::default());
    let use_case = SearchProfilesUseCase::new(Arc::clone(&repository));

    let response = use_case
        .execute("   ", None, 25)
        .await
        .expect("blank search");

    assert!(response.profiles.is_empty());
    assert_eq!(response.next_cursor, None);
    assert!(repository.search_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn update_profile_normalizes_username_and_preserves_existing_counts() {
    let user_id = uuid("00000000-0000-0000-0000-000000000106");
    let repository = Arc::new(FakeProfileRepository::with_profiles(vec![profile(
        user_id, "oldname", 11, 12, 13,
    )]));
    let use_case = UpdateProfileUseCase::new(Arc::clone(&repository));

    let response = use_case
        .execute(UpdateProfileDto {
            user_id: format!(" {user_id} "),
            username: " New_Name ".to_owned(),
            nickname: " Alice ".to_owned(),
            avatar_url: " /avatar.png ".to_owned(),
            bio: " hello ".to_owned(),
        })
        .await
        .expect("update profile");

    assert_eq!(response.username, "new_name");
    assert_eq!(response.nickname, "Alice");
    assert_eq!(response.avatar_url, "/avatar.png");
    assert_eq!(response.bio, "hello");
    assert_eq!(response.follower_count, 11);
    assert_eq!(response.post_count, 12);
    assert_eq!(response.following_count, 13);
    let saved = repository.saved_profiles.lock().unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].username, "new_name");
}

#[tokio::test]
async fn update_profile_rejects_username_owned_by_another_user_without_saving() {
    let user_id = uuid("00000000-0000-0000-0000-000000000107");
    let other_user_id = uuid("00000000-0000-0000-0000-000000000108");
    let repository = Arc::new(FakeProfileRepository::with_profiles(vec![profile(
        other_user_id,
        "taken",
        0,
        0,
        0,
    )]));
    let use_case = UpdateProfileUseCase::new(Arc::clone(&repository));

    let error = use_case
        .execute(UpdateProfileDto {
            user_id: user_id.to_string(),
            username: "taken".to_owned(),
            nickname: "Alice".to_owned(),
            avatar_url: "/avatar.png".to_owned(),
            bio: "hello".to_owned(),
        })
        .await
        .expect_err("duplicate username");

    assert_eq!(error.to_string(), "Username already taken");
    assert!(repository.saved_profiles.lock().unwrap().is_empty());
}

#[tokio::test]
async fn owner_can_make_profile_private() {
    let user_id = uuid("00000000-0000-0000-0000-000000000120");
    let repository = Arc::new(FakeProfileRepository::with_profiles(vec![profile(
        user_id,
        "private_me",
        0,
        0,
        0,
    )]));
    let use_case = UpdateProfilePrivacyUseCase::new(Arc::clone(&repository));

    let response = use_case
        .execute(UpdateProfilePrivacyDto {
            user_id: user_id.to_string(),
            actor_id: user_id.to_string(),
            account_visibility: "PRIVATE".to_owned(),
        })
        .await
        .expect("owner privacy update");

    assert_eq!(response.account_visibility, "PRIVATE");
    assert_eq!(
        repository.visibility_updates.lock().unwrap().as_slice(),
        &[(user_id, AccountVisibility::Private)]
    );
}

#[tokio::test]
async fn non_owner_cannot_change_profile_privacy() {
    let user_id = uuid("00000000-0000-0000-0000-000000000121");
    let actor_id = uuid("00000000-0000-0000-0000-000000000122");
    let repository = Arc::new(FakeProfileRepository::with_profiles(vec![profile(
        user_id, "owned", 0, 0, 0,
    )]));
    let use_case = UpdateProfilePrivacyUseCase::new(Arc::clone(&repository));

    let error = use_case
        .execute(UpdateProfilePrivacyDto {
            user_id: user_id.to_string(),
            actor_id: actor_id.to_string(),
            account_visibility: "PRIVATE".to_owned(),
        })
        .await
        .expect_err("non-owner privacy update");

    assert_eq!(error.to_string(), "Forbidden");
    assert!(repository.visibility_updates.lock().unwrap().is_empty());
}

#[tokio::test]
async fn profile_visibility_defaults_to_public() {
    let user_id = uuid("00000000-0000-0000-0000-000000000123");
    let repository = Arc::new(FakeProfileRepository::with_profiles(vec![profile(
        user_id, "public", 0, 0, 0,
    )]));
    let response = GetProfileUseCase::new(repository)
        .execute(user_id)
        .await
        .expect("profile");

    assert_eq!(response.account_visibility, "PUBLIC");
    assert_eq!(response.lifecycle_status, "ACTIVE");
}

#[tokio::test]
async fn owner_delete_soft_deletes_and_records_success_audit() {
    let user_id = uuid("00000000-0000-0000-0000-000000000109");
    let repository = Arc::new(FakeProfileRepository::with_profiles(vec![profile(
        user_id,
        "delete_me",
        0,
        0,
        0,
    )]));
    let runtime = Arc::new(AdvancingProfileRuntime::default());
    let use_case = DeleteProfileUseCase::new(Arc::clone(&repository), runtime.clone());

    let response = use_case
        .execute(delete_request(user_id, user_id))
        .await
        .expect("owner delete");

    assert!(response.deleted);
    assert!(!response.account_deleted);
    assert_eq!(response.user_id, user_id.to_string());
    let deletes = repository.soft_delete_calls.lock().unwrap();
    assert_eq!(deletes.len(), 1);
    assert_eq!(deletes[0].user_id, user_id);
    assert_eq!(deletes[0].actor_id, user_id);
    assert_eq!(deletes[0].reason, "user_request");
    let audits = repository.audit_events.lock().unwrap();
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].action, "PROFILE_DELETED");
    assert_eq!(audits[0].outcome, "SUCCESS");
    assert_eq!(audits[0].request_id.as_deref(), Some("req-1"));
    assert_eq!(audits[0].id, Uuid::from_u128(0xabc0));
    assert_eq!(audits[0].occurred_at, instant());
    assert_eq!(
        deletes[0].retention_until,
        instant() + chrono::Duration::days(30)
    );
    assert_eq!(
        runtime.time_calls.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    assert_eq!(
        runtime.id_calls.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
}

#[tokio::test]
async fn non_owner_delete_records_forbidden_audit_without_soft_delete() {
    let user_id = uuid("00000000-0000-0000-0000-000000000110");
    let actor_id = uuid("00000000-0000-0000-0000-000000000111");
    let repository = Arc::new(FakeProfileRepository::with_profiles(vec![profile(
        user_id, "owned", 0, 0, 0,
    )]));
    let runtime = Arc::new(AdvancingProfileRuntime::default());
    let use_case = DeleteProfileUseCase::new(Arc::clone(&repository), runtime.clone());

    let error = use_case
        .execute(delete_request(user_id, actor_id))
        .await
        .expect_err("forbidden delete");

    assert_eq!(error.to_string(), "Forbidden");
    assert!(repository.soft_delete_calls.lock().unwrap().is_empty());
    let audits = repository.audit_events.lock().unwrap();
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].actor_id, Some(actor_id));
    assert_eq!(audits[0].profile_user_id, Some(user_id));
    assert_eq!(audits[0].outcome, "FORBIDDEN");
    assert_eq!(audits[0].id, Uuid::from_u128(0xabc0));
    assert_eq!(audits[0].occurred_at, instant());
    assert_eq!(
        runtime.time_calls.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
}

#[tokio::test]
async fn purge_profiles_trims_optional_metadata_and_returns_count() {
    let actor_id = uuid("00000000-0000-0000-0000-000000000112");
    let purged_id = uuid("00000000-0000-0000-0000-000000000113");
    let repository = Arc::new(FakeProfileRepository::default());
    *repository.purge_result.lock().unwrap() = vec![purged_id];
    let runtime = Arc::new(AdvancingProfileRuntime::default());
    let use_case = PurgeExpiredProfilesUseCase::new(Arc::clone(&repository), runtime.clone());

    let response = use_case
        .execute(PurgeExpiredProfilesDto {
            actor_id: format!(" {actor_id} "),
            request_id: Some(" req-2 ".to_owned()),
            ip_address: Some(" 10.0.0.1 ".to_owned()),
            user_agent: Some("  ".to_owned()),
            dry_run: true,
        })
        .await
        .expect("purge profiles");

    assert_eq!(response.purged_count, 1);
    let calls = repository.purge_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].cutoff, instant());
    assert_eq!(
        runtime.time_calls.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    assert_eq!(
        runtime.id_calls.load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert_eq!(calls[0].actor_id, actor_id);
    assert_eq!(calls[0].request_id.as_deref(), Some("req-2"));
    assert_eq!(calls[0].ip_address.as_deref(), Some("10.0.0.1"));
    assert_eq!(calls[0].user_agent, None);
    assert!(calls[0].dry_run);
}

#[derive(Default)]
struct FakeProfileRepository {
    profiles: Mutex<Vec<Profile>>,
    search_result: Mutex<(Vec<Profile>, Option<String>)>,
    purge_result: Mutex<Vec<Uuid>>,
    find_by_user_id_calls: Mutex<Vec<Uuid>>,
    find_by_username_calls: Mutex<Vec<String>>,
    find_by_user_ids_calls: Mutex<Vec<Vec<Uuid>>>,
    search_calls: Mutex<Vec<SearchCall>>,
    saved_profiles: Mutex<Vec<Profile>>,
    soft_delete_calls: Mutex<Vec<SoftDeleteCall>>,
    purge_calls: Mutex<Vec<PurgeCall>>,
    audit_events: Mutex<Vec<ProfileAuditEvent>>,
    visibility_updates: Mutex<Vec<(Uuid, AccountVisibility)>>,
}

impl FakeProfileRepository {
    fn with_profiles(profiles: Vec<Profile>) -> Self {
        Self {
            profiles: Mutex::new(profiles),
            ..Self::default()
        }
    }

    fn with_search_results(profiles: Vec<Profile>, next_cursor: Option<String>) -> Self {
        Self {
            search_result: Mutex::new((profiles, next_cursor)),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchCall {
    needle: String,
    cursor: Option<String>,
    limit: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SoftDeleteCall {
    user_id: Uuid,
    actor_id: Uuid,
    reason: String,
    retention_until: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PurgeCall {
    cutoff: DateTime<Utc>,
    actor_id: Uuid,
    request_id: Option<String>,
    ip_address: Option<String>,
    user_agent: Option<String>,
    dry_run: bool,
}

#[async_trait]
impl ProfileRepositoryPort for FakeProfileRepository {
    async fn find_username_owner(&self, username: &str) -> Result<Option<Uuid>> {
        Ok(self
            .profiles
            .lock()
            .unwrap()
            .iter()
            .find(|profile| profile.username == username)
            .map(|profile| profile.user_id))
    }

    async fn find_by_user_id(&self, user_id: Uuid) -> Result<Option<ProfileRecordDto>> {
        self.find_by_user_id_calls.lock().unwrap().push(user_id);
        Ok(self
            .profiles
            .lock()
            .unwrap()
            .iter()
            .find(|profile| profile.user_id == user_id)
            .cloned()
            .map(Into::into))
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<ProfileRecordDto>> {
        self.find_by_username_calls
            .lock()
            .unwrap()
            .push(username.to_owned());
        Ok(self
            .profiles
            .lock()
            .unwrap()
            .iter()
            .find(|profile| profile.username == username)
            .cloned()
            .map(Into::into))
    }

    async fn find_by_user_ids(&self, user_ids: &[Uuid]) -> Result<Vec<ProfileRecordDto>> {
        self.find_by_user_ids_calls
            .lock()
            .unwrap()
            .push(user_ids.to_vec());
        Ok(self
            .profiles
            .lock()
            .unwrap()
            .iter()
            .filter(|profile| user_ids.contains(&profile.user_id))
            .cloned()
            .map(Into::into)
            .collect())
    }

    async fn save(&self, profile: &ProfileRecordDto) -> Result<ProfileRecordDto> {
        self.saved_profiles
            .lock()
            .unwrap()
            .push(profile.clone().into());
        self.profiles.lock().unwrap().push(profile.clone().into());
        Ok(profile.clone())
    }

    async fn search(
        &self,
        needle: &str,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<(Vec<ProfileRecordDto>, Option<String>)> {
        self.search_calls.lock().unwrap().push(SearchCall {
            needle: needle.to_owned(),
            cursor: cursor.map(str::to_owned),
            limit,
        });
        let (profiles, cursor) = self.search_result.lock().unwrap().clone();
        Ok((profiles.into_iter().map(Into::into).collect(), cursor))
    }

    async fn update_visibility(
        &self,
        user_id: Uuid,
        visibility: ProfileVisibilityDto,
    ) -> Result<Option<ProfileRecordDto>> {
        self.visibility_updates
            .lock()
            .unwrap()
            .push((user_id, visibility.into()));
        let mut profiles = self.profiles.lock().unwrap();
        let Some(profile) = profiles
            .iter_mut()
            .find(|profile| profile.user_id == user_id)
        else {
            return Ok(None);
        };
        profile.account_visibility = visibility.into();
        Ok(Some(profile.clone().into()))
    }

    async fn soft_delete_profile(
        &self,
        user_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
        audit: &profile_service::application::dto::profile_dto::ProfileDeletionAuditDto,
    ) -> Result<bool> {
        self.soft_delete_calls.lock().unwrap().push(SoftDeleteCall {
            user_id,
            actor_id,
            reason: reason.to_owned(),
            retention_until,
        });
        let deleted = self
            .profiles
            .lock()
            .unwrap()
            .iter()
            .any(|profile| profile.user_id == user_id);
        if deleted {
            self.audit_events.lock().unwrap().push(ProfileAuditEvent {
                id: audit.id,
                actor_id: Some(actor_id),
                profile_user_id: Some(user_id),
                action: "PROFILE_DELETED".into(),
                outcome: "SUCCESS".into(),
                request_id: audit.request_id.clone(),
                ip_address: audit.ip_address.clone(),
                user_agent: audit.user_agent.clone(),
                metadata_json: audit.metadata_json.clone(),
                occurred_at: audit.occurred_at,
            });
        }
        Ok(deleted)
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
        self.purge_calls.lock().unwrap().push(PurgeCall {
            cutoff,
            actor_id,
            request_id,
            ip_address,
            user_agent,
            dry_run,
        });
        Ok(self.purge_result.lock().unwrap().clone())
    }

    async fn record_audit_event(&self, event: &ProfileAuditEventDto) -> Result<()> {
        self.audit_events.lock().unwrap().push(event.clone().into());
        Ok(())
    }
}

fn profile(
    user_id: Uuid,
    username: impl Into<String>,
    follower_count: i64,
    post_count: i64,
    following_count: i64,
) -> Profile {
    let username = username.into();
    Profile {
        user_id,
        username: username.clone(),
        nickname: format!("{username} nickname"),
        avatar_url: format!("/{username}.png"),
        bio: format!("{username} bio"),
        follower_count,
        post_count,
        following_count,
        account_visibility: AccountVisibility::Public,
        lifecycle_status: "ACTIVE".to_owned(),
    }
}

fn delete_request(user_id: Uuid, actor_id: Uuid) -> DeleteProfileDto {
    DeleteProfileDto {
        user_id: user_id.to_string(),
        actor_id: actor_id.to_string(),
        request_id: Some(" req-1 ".to_owned()),
        ip_address: Some(" 127.0.0.1 ".to_owned()),
        user_agent: Some(" unit-test ".to_owned()),
    }
}

fn uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).expect("valid uuid")
}
