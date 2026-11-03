use anyhow::{bail, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use platform::recommendation::{RecommendationCursor, RecommendationEvent, RolloutMode};
use post_service::application::ports::{FeedRecommendationRepositoryPort, RecommendationRollout};
use post_service::domain::ranking::{
    FeedCandidate, FeedRecommendationImpression, FeedSnapshot, FeedSnapshotItem,
};
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Default)]
pub struct FakeFeedRecommendationRepository {
    pub candidate_error: Mutex<Option<String>>,
    pub candidates: Mutex<Vec<FeedCandidate>>,
    pub snapshots: Mutex<Vec<FeedSnapshot>>,
    pub snapshot_items: Mutex<Vec<FeedSnapshotItem>>,
    pub impressions: Mutex<Vec<FeedRecommendationImpression>>,
    pub events: Mutex<Vec<RecommendationEvent>>,
    pub materialized: Mutex<Vec<MaterializeCall>>,
    pub removed: Mutex<Vec<Uuid>>,
    pub rollout: Mutex<Option<RecommendationRollout>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializeCall {
    pub post_id: Uuid,
    pub author_id: Uuid,
    pub follower_ids: Vec<Uuid>,
    pub high_fanout_threshold: usize,
}

#[async_trait]
impl FeedRecommendationRepositoryPort for FakeFeedRecommendationRepository {
    async fn materialize_post(
        &self,
        post_id: Uuid,
        author_id: Uuid,
        _created_at: DateTime<Utc>,
        follower_ids: &[Uuid],
        high_fanout_threshold: usize,
    ) -> Result<()> {
        self.materialized.lock().unwrap().push(MaterializeCall {
            post_id,
            author_id,
            follower_ids: follower_ids.to_vec(),
            high_fanout_threshold,
        });
        Ok(())
    }

    async fn remove_post(&self, post_id: Uuid) -> Result<()> {
        self.removed.lock().unwrap().push(post_id);
        Ok(())
    }

    async fn load_candidates(
        &self,
        _viewer_id: Uuid,
        _following_ids: &[Uuid],
        _now: DateTime<Utc>,
        _limit: i32,
    ) -> Result<Vec<FeedCandidate>> {
        if let Some(error) = self.candidate_error.lock().unwrap().clone() {
            bail!(error);
        }
        Ok(self.candidates.lock().unwrap().clone())
    }

    async fn create_snapshot(
        &self,
        snapshot: &FeedSnapshot,
        items: &[FeedSnapshotItem],
    ) -> Result<()> {
        self.snapshots.lock().unwrap().push(snapshot.clone());
        *self.snapshot_items.lock().unwrap() = items.to_vec();
        Ok(())
    }

    async fn find_snapshot(
        &self,
        viewer_id: Uuid,
        snapshot_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<Option<FeedSnapshot>> {
        Ok(self
            .snapshots
            .lock()
            .unwrap()
            .iter()
            .find(|snapshot| {
                snapshot.id == snapshot_id
                    && snapshot.viewer_id == viewer_id
                    && snapshot.expires_at > now
            })
            .cloned())
    }

    async fn load_snapshot_page(
        &self,
        _snapshot_id: Uuid,
        cursor: Option<&RecommendationCursor>,
        limit: i32,
    ) -> Result<Vec<FeedSnapshotItem>> {
        let items = self.snapshot_items.lock().unwrap();
        let start = cursor
            .and_then(|cursor| {
                items
                    .iter()
                    .position(|item| item.post_id == cursor.tie_breaker_id)
                    .map(|index| index + 1)
            })
            .unwrap_or_default();
        Ok(items
            .iter()
            .skip(start)
            .take(limit.max(0) as usize)
            .cloned()
            .collect())
    }

    async fn record_impressions(&self, impressions: &[FeedRecommendationImpression]) -> Result<()> {
        self.impressions
            .lock()
            .unwrap()
            .extend_from_slice(impressions);
        Ok(())
    }

    async fn record_event(&self, event: &RecommendationEvent) -> Result<bool> {
        let mut events = self.events.lock().unwrap();
        if events
            .iter()
            .any(|existing| existing.event_id == event.event_id)
        {
            return Ok(false);
        }
        events.push(event.clone());
        Ok(true)
    }

    async fn rollout(&self, _surface: &str) -> Result<RecommendationRollout> {
        Ok(self
            .rollout
            .lock()
            .unwrap()
            .clone()
            .unwrap_or(RecommendationRollout {
                mode: RolloutMode::Deterministic,
                deterministic_version: "feed-linear-v1".into(),
                learned_version: None,
                canary_percent: 0,
                learned_ready: false,
            }))
    }
}
