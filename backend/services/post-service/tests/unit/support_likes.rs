use anyhow::Result;
use async_trait::async_trait;
use post_service::application::ports::PostLikeRepositoryPort;
use post_service::domain::post::StoredEvent;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Default)]
pub struct FakeLikeRepository {
    pub like_counts: Mutex<HashMap<Uuid, i64>>,
    pub liked_ids: Mutex<HashSet<Uuid>>,
    pub add_like_count: Mutex<i64>,
    pub remove_like_count: Mutex<i64>,
    pub add_events: Mutex<Vec<StoredEvent>>,
    pub remove_events: Mutex<Vec<StoredEvent>>,
}

#[async_trait]
impl PostLikeRepositoryPort for FakeLikeRepository {
    async fn add_like(&self, _: Uuid, _: Uuid) -> Result<bool> {
        Ok(true)
    }

    async fn add_like_with_event(
        &self,
        _: Uuid,
        _: Uuid,
        event: &StoredEvent,
        operation: Option<&post_service::application::dto::mutation_dto::MutationOperationDto>,
    ) -> Result<(bool, i64)> {
        assert!(
            operation.is_none(),
            "Use real PostgreSQL to test owner receipts"
        );
        self.add_events.lock().unwrap().push(event.clone());
        Ok((true, *self.add_like_count.lock().unwrap()))
    }

    async fn remove_like(&self, _: Uuid, _: Uuid) -> Result<bool> {
        Ok(true)
    }

    async fn remove_like_with_event(
        &self,
        _: Uuid,
        _: Uuid,
        event: &StoredEvent,
        operation: Option<&post_service::application::dto::mutation_dto::MutationOperationDto>,
    ) -> Result<(bool, i64)> {
        assert!(
            operation.is_none(),
            "Use real PostgreSQL to test owner receipts"
        );
        self.remove_events.lock().unwrap().push(event.clone());
        Ok((true, *self.remove_like_count.lock().unwrap()))
    }

    async fn count_likes(&self, post_ids: &[Uuid]) -> Result<HashMap<Uuid, i64>> {
        let counts = self.like_counts.lock().unwrap();
        Ok(post_ids
            .iter()
            .filter_map(|id| counts.get(id).copied().map(|count| (*id, count)))
            .collect())
    }

    async fn liked_post_ids(&self, _: Uuid, post_ids: &[Uuid]) -> Result<HashSet<Uuid>> {
        let liked = self.liked_ids.lock().unwrap();
        Ok(post_ids
            .iter()
            .copied()
            .filter(|id| liked.contains(id))
            .collect())
    }
}
