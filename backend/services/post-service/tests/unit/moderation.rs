use anyhow::Result;
use async_trait::async_trait;
use post_service::application::dto::post_dto::ApplyContentModerationDto;
use post_service::application::ports::{ContentModerationRepositoryPort, PostCachePort};
use post_service::application::use_cases::apply_content_moderation::ApplyContentModerationUseCase;
use post_service::domain::post::{ContentModerationAction, ContentModerationTarget};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const REPORT_ID: &str = "11111111-1111-1111-1111-111111111111";
const TARGET_ID: &str = "22222222-2222-2222-2222-222222222222";
const ACTOR_ID: &str = "33333333-3333-3333-3333-333333333333";
const POST_ID: Uuid = Uuid::from_u128(0x44444444444444444444444444444444);

#[tokio::test]
async fn hide_content_applies_repository_change_and_evicts_affected_post() {
    let repository = Arc::new(RecordingModerationRepository::with_post(POST_ID));
    let cache = Arc::new(RecordingCache::default());
    let use_case = ApplyContentModerationUseCase::new(repository.clone(), cache.clone());

    let response = use_case
        .execute(ApplyContentModerationDto {
            report_id: REPORT_ID.into(),
            target_type: "POST".into(),
            target_id: TARGET_ID.into(),
            actor_id: ACTOR_ID.into(),
            action: "HIDE".into(),
        })
        .await
        .expect("hide content");

    assert!(response.applied);
    assert_eq!(repository.calls.lock().unwrap().len(), 1);
    assert_eq!(
        repository.calls.lock().unwrap()[0].target,
        ContentModerationTarget::Post
    );
    assert_eq!(
        repository.calls.lock().unwrap()[0].action,
        ContentModerationAction::Hide
    );
    assert_eq!(*cache.evicted.lock().unwrap(), vec![POST_ID]);
}

#[tokio::test]
async fn invalid_target_or_action_is_rejected_before_repository_access() {
    let repository = Arc::new(RecordingModerationRepository::with_post(POST_ID));
    let cache = Arc::new(RecordingCache::default());
    let use_case = ApplyContentModerationUseCase::new(repository.clone(), cache);

    let error = use_case
        .execute(ApplyContentModerationDto {
            report_id: REPORT_ID.into(),
            target_type: "USER".into(),
            target_id: TARGET_ID.into(),
            actor_id: ACTOR_ID.into(),
            action: "DELETE".into(),
        })
        .await
        .expect_err("invalid moderation command");

    assert_eq!(
        error.to_string(),
        "Content moderation target must be POST or COMMENT"
    );
    assert!(repository.calls.lock().unwrap().is_empty());
}

#[test]
fn post_repository_visibility_queries_exclude_moderation_hidden_rows() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/adapters/out/persistence/post_repository.rs"),
    )
    .expect("post repository source");

    assert!(source.matches("moderation_hidden_at is null").count() >= 10);
    assert!(source.contains(
        "from comments where post_id = $1 and deleted_at is null and moderation_hidden_at is null"
    ));
    assert!(
        source.contains("where id = $1 and deleted_at is null and moderation_hidden_at is null")
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModerationCall {
    report_id: Uuid,
    target: ContentModerationTarget,
    target_id: Uuid,
    actor_id: Uuid,
    action: ContentModerationAction,
}

struct RecordingModerationRepository {
    affected_post_id: Option<Uuid>,
    calls: Mutex<Vec<ModerationCall>>,
}

impl RecordingModerationRepository {
    fn with_post(post_id: Uuid) -> Self {
        Self {
            affected_post_id: Some(post_id),
            calls: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl ContentModerationRepositoryPort for RecordingModerationRepository {
    async fn apply_content_moderation(
        &self,
        report_id: Uuid,
        target: ContentModerationTarget,
        target_id: Uuid,
        actor_id: Uuid,
        action: ContentModerationAction,
    ) -> Result<Option<Uuid>> {
        self.calls.lock().unwrap().push(ModerationCall {
            report_id,
            target,
            target_id,
            actor_id,
            action,
        });
        Ok(self.affected_post_id)
    }
}

#[derive(Default)]
struct RecordingCache {
    evicted: Mutex<Vec<Uuid>>,
}

#[async_trait]
impl PostCachePort for RecordingCache {
    async fn get_posts(
        &self,
        _: &[Uuid],
    ) -> Result<HashMap<Uuid, post_service::domain::post::Post>> {
        Ok(HashMap::new())
    }
    async fn set_post(&self, _: &post_service::domain::post::Post) -> Result<()> {
        Ok(())
    }
    async fn set_posts(&self, _: &[post_service::domain::post::Post]) -> Result<()> {
        Ok(())
    }
    async fn evict(&self, post_id: Uuid) -> Result<()> {
        self.evicted.lock().unwrap().push(post_id);
        Ok(())
    }
}
