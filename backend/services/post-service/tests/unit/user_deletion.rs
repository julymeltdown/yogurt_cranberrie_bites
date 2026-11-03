use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use post_service::application::dto::post_dto::ApplyUserDeletionDto;
use post_service::application::ports::{PostCachePort, UserLifecycleRepositoryPort};
use post_service::application::use_cases::apply_user_deletion::ApplyUserDeletionUseCase;
use post_service::domain::post::Post;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

type UserDeletionCall = (Uuid, Uuid, Uuid, DateTime<Utc>);

#[derive(Clone)]
struct RecordingRepository {
    post_ids: Vec<Uuid>,
    calls: Arc<Mutex<Vec<UserDeletionCall>>>,
}

#[async_trait]
impl UserLifecycleRepositoryPort for RecordingRepository {
    async fn apply_user_deletion(
        &self,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> Result<Vec<Uuid>> {
        self.calls
            .lock()
            .unwrap()
            .push((saga_id, user_id, actor_id, retention_until));
        Ok(self.post_ids.clone())
    }
}

#[derive(Clone, Default)]
struct RecordingCache {
    evicted: Arc<Mutex<Vec<Uuid>>>,
}

#[async_trait]
impl PostCachePort for RecordingCache {
    async fn get_posts(&self, _post_ids: &[Uuid]) -> Result<HashMap<Uuid, Post>> {
        Ok(HashMap::new())
    }
    async fn set_post(&self, _post: &Post) -> Result<()> {
        Ok(())
    }
    async fn set_posts(&self, _posts: &[Post]) -> Result<()> {
        Ok(())
    }
    async fn evict(&self, post_id: Uuid) -> Result<()> {
        self.evicted.lock().unwrap().push(post_id);
        Ok(())
    }
}

#[tokio::test]
async fn account_deletion_hides_content_idempotently_and_evicts_authored_posts() {
    let post_ids = vec![Uuid::new_v4(), Uuid::new_v4()];
    let repository = RecordingRepository {
        post_ids: post_ids.clone(),
        calls: Arc::default(),
    };
    let cache = RecordingCache::default();
    let use_case = ApplyUserDeletionUseCase::new(repository.clone(), cache.clone());
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
    assert!(use_case.execute(request).await.expect("repeat").applied);

    assert_eq!(repository.calls.lock().unwrap().len(), 2);
    assert_eq!(
        cache.evicted.lock().unwrap().as_slice(),
        &[post_ids[0], post_ids[1], post_ids[0], post_ids[1]]
    );
}
