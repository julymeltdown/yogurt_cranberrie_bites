use anyhow::{bail, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use post_service::application::dto::delivery_schedule_dto::{
    DeliveryLeaseOutcomeDto, DeliveryLeaseStatusDto, DeliveryScheduleConfigDto,
    TargetDeliveryBatchDto, TargetDeliveryClaimDto,
};
use post_service::application::ports::{
    DeliverySchedulePort, EventOutboxPort, ExternalEventPublisherPort, FeedCachePort,
    FriendClientPort, InternalEventDispatcherPort, NotificationClientPort, PostCachePort,
    ProfileClientPort,
};
use post_service::application::query_witness::PublishedWitnessStageDto;
use post_service::domain::feed::FeedCachePage;
use post_service::domain::post::{
    DeliveryAttempt, DeliveryAttemptPhase, DeliveryAttemptSource, DeliveryTarget,
    EventReplayFilter, NotificationRequest, OutboxMetricsSnapshot, Post, StoredEvent,
};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Mutex;
use uuid::Uuid;

type FeedCacheSetCall = (Uuid, Option<String>, i32, FeedCachePage);

#[derive(Default)]
pub struct FakeFeedCache {
    pub cached: Mutex<Option<FeedCachePage>>,
    pub set_calls: Mutex<Vec<FeedCacheSetCall>>,
    pub invalidated: Mutex<Vec<Vec<Uuid>>>,
    /// When set, invalidation fails, which models an unavailable cache.
    pub error: Mutex<Option<String>>,
    /// The generation reported to callers. Bumping it models a mutation.
    pub generation: Mutex<u64>,
}

#[async_trait]
impl FeedCachePort for FakeFeedCache {
    async fn get_feed(&self, _: Uuid, _: Option<&str>, _: i32) -> Result<Option<FeedCachePage>> {
        Ok(self.cached.lock().unwrap().clone())
    }

    async fn set_feed(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
        page: &FeedCachePage,
        generation: u64,
    ) -> Result<()> {
        // Mirror the real adapter: a fill against a superseded generation is
        // discarded rather than stored.
        if generation != *self.generation.lock().unwrap() {
            return Ok(());
        }
        self.set_calls.lock().unwrap().push((
            user_id,
            cursor.map(str::to_owned),
            limit,
            page.clone(),
        ));
        Ok(())
    }

    async fn current_generation(&self, _user_id: Uuid) -> Result<u64> {
        Ok(*self.generation.lock().unwrap())
    }

    async fn invalidate_users(&self, user_ids: &[Uuid]) -> Result<()> {
        if let Some(error) = self.error.lock().unwrap().clone() {
            return Err(
                post_service::application::delivery_effect::DeliveryEffectFailure::confirmed(
                    anyhow::anyhow!(error),
                ),
            );
        }
        self.invalidated.lock().unwrap().push(user_ids.to_vec());
        Ok(())
    }
}

#[derive(Default)]
pub struct FakePostCache {
    pub cached: Mutex<HashMap<Uuid, Post>>,
    pub set_posts: Mutex<Vec<Vec<Uuid>>>,
    pub evicted: Mutex<Vec<Uuid>>,
    /// When set, eviction fails, which models an unavailable cache.
    pub error: Mutex<Option<String>>,
}

#[async_trait]
impl PostCachePort for FakePostCache {
    async fn get_posts(&self, post_ids: &[Uuid]) -> Result<HashMap<Uuid, Post>> {
        let cached = self.cached.lock().unwrap();
        Ok(post_ids
            .iter()
            .filter_map(|id| cached.get(id).cloned().map(|post| (*id, post)))
            .collect())
    }

    async fn set_post(&self, post: &Post) -> Result<()> {
        self.set_posts.lock().unwrap().push(vec![post.id]);
        Ok(())
    }

    async fn set_posts(&self, posts: &[Post]) -> Result<()> {
        self.set_posts
            .lock()
            .unwrap()
            .push(posts.iter().map(|post| post.id).collect());
        Ok(())
    }

    async fn evict(&self, post_id: Uuid) -> Result<()> {
        if let Some(error) = self.error.lock().unwrap().clone() {
            return Err(
                post_service::application::delivery_effect::DeliveryEffectFailure::confirmed(
                    anyhow::anyhow!(error),
                ),
            );
        }
        self.evicted.lock().unwrap().push(post_id);
        Ok(())
    }
}

#[derive(Default)]
pub struct FakeFriendClient {
    pub following_ids: Mutex<Vec<Uuid>>,
    pub follower_ids: Mutex<Vec<Uuid>>,
    pub is_following: Mutex<bool>,
    pub follower_calls: Mutex<Vec<Uuid>>,
    pub policies:
        Mutex<HashMap<(Uuid, Uuid), post_service::domain::post::RelationshipPolicySnapshot>>,
    pub omit_batch_policy_for: Mutex<Option<Uuid>>,
    pub fail_batch_policy_for: Mutex<Option<Uuid>>,
    pub extra_batch_policy_for: Mutex<Option<Uuid>>,
    pub batch_policy_calls: Mutex<Vec<(Uuid, Vec<Uuid>)>>,
}

#[async_trait]
impl FriendClientPort for FakeFriendClient {
    async fn fetch_feed_authors(
        &self,
        viewer_id: Uuid,
    ) -> Result<post_service::application::feed_authors::FeedAuthorsDto> {
        let ids = self.fetch_following_ids(viewer_id).await?;
        let mut eligible = Vec::new();
        for id in ids {
            let policy = self.relationship_policy(viewer_id, id, None).await?;
            if policy.following && !policy.is_blocked() && !policy.muted_by_viewer {
                eligible.push(id);
            }
        }
        post_service::application::feed_authors::FeedAuthorsDto::new(viewer_id, eligible)
    }

    async fn fetch_following_ids(&self, _: Uuid) -> Result<Vec<Uuid>> {
        Ok(self.following_ids.lock().unwrap().clone())
    }

    async fn fetch_follower_ids(&self, user_id: Uuid) -> Result<Vec<Uuid>> {
        self.follower_calls.lock().unwrap().push(user_id);
        Ok(self.follower_ids.lock().unwrap().clone())
    }

    async fn is_following(&self, _: Uuid, _: Uuid) -> Result<bool> {
        Ok(*self.is_following.lock().unwrap())
    }

    async fn relationship_policy(
        &self,
        viewer_id: Uuid,
        target_id: Uuid,
        _stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<post_service::domain::post::RelationshipPolicySnapshot> {
        Ok(self
            .policies
            .lock()
            .unwrap()
            .get(&(viewer_id, target_id))
            .copied()
            .unwrap_or_default())
    }

    async fn batch_relationship_policies(
        &self,
        viewer_id: Uuid,
        target_ids: &[Uuid],
        _stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<HashMap<Uuid, post_service::domain::post::RelationshipPolicySnapshot>> {
        self.batch_policy_calls
            .lock()
            .unwrap()
            .push((viewer_id, target_ids.to_vec()));
        if target_ids.len() > 100 {
            bail!("Relationship batch supports at most 100 target IDs");
        }
        if self
            .fail_batch_policy_for
            .lock()
            .unwrap()
            .is_some_and(|target| target_ids.contains(&target))
        {
            bail!("injected relationship batch failure");
        }
        let omitted = *self.omit_batch_policy_for.lock().unwrap();
        let mut policies = HashMap::with_capacity(target_ids.len());
        for target_id in target_ids.iter().copied() {
            if Some(target_id) == omitted {
                continue;
            }
            policies.insert(
                target_id,
                self.relationship_policy(viewer_id, target_id, _stage)
                    .await?,
            );
        }
        let extra = *self.extra_batch_policy_for.lock().unwrap();
        if let Some(target) = extra.filter(|target| !target_ids.contains(target)) {
            policies.insert(
                target,
                self.relationship_policy(viewer_id, target, _stage).await?,
            );
        }
        Ok(policies)
    }
}

#[derive(Default)]
pub struct FakeProfileClient {
    pub fetch_calls: Mutex<Vec<Uuid>>,
}

#[async_trait]
impl ProfileClientPort for FakeProfileClient {
    async fn require_author(
        &self,
        user_id: Uuid,
    ) -> Result<post_service::application::profile_author::ExistingProfileAuthorDto> {
        self.fetch_calls.lock().unwrap().push(user_id);
        post_service::application::profile_author::ExistingProfileAuthorDto::new(user_id)
    }
}

#[derive(Default)]
pub struct FakeOutbox {
    pub events: Mutex<Vec<StoredEvent>>,
    pub unpublished: Mutex<Vec<StoredEvent>>,
    pub find_unpublished_error: Mutex<Option<String>>,
    pub delivery_completed_error: Mutex<Option<String>>,
    pub failures: Mutex<Vec<(Uuid, String, i32)>>,
    pub find_events_calls: Mutex<Vec<(EventReplayFilter, i32)>>,
    pub find_unpublished_calls: Mutex<Vec<(DateTime<Utc>, i32)>>,
    pub snapshot: Mutex<OutboxMetricsSnapshot>,
    pub completed: Mutex<HashSet<(Uuid, String)>>,
    pub published: Mutex<Vec<Uuid>>,
    pub delivery_attempts: Mutex<Vec<DeliveryAttempt>>,
    pub target_claim_calls: Mutex<Vec<(DeliveryTarget, DateTime<Utc>, i32)>>,
    pub claim_error: Mutex<Option<String>>,
    pub lease_status_error: Mutex<Option<String>>,
    schedule_leases: Mutex<HashMap<(Uuid, String), FixtureLease>>,
    schedule_retries: Mutex<HashMap<(Uuid, String), DateTime<Utc>>>,
    schedule_required: Mutex<HashMap<Uuid, Vec<DeliveryTarget>>>,
}

type FixtureLease = (
    TargetDeliveryClaimDto,
    DateTime<Utc>,
    DeliveryScheduleConfigDto,
);

// Recording unit port only. SQL fencing/ordering/rollback acceptance lives in
// post_delivery_schedule.rs against real PostgreSQL, not this fixture.
#[async_trait]
impl DeliverySchedulePort for FakeOutbox {
    async fn lease_status(&self, claim: &TargetDeliveryClaimDto) -> Result<DeliveryLeaseStatusDto> {
        if let Some(error) = self.lease_status_error.lock().unwrap().clone() {
            bail!(error);
        }
        let leases = self.schedule_leases.lock().unwrap();
        Ok(
            match leases.get(&(claim.event.event_id, claim.target.as_str().into())) {
                Some((stored, _, _))
                    if stored.lease_token == claim.lease_token
                        && stored.attempt_id == claim.attempt_id =>
                {
                    DeliveryLeaseStatusDto::Owned {
                        remaining_ms: 300_000,
                    }
                }
                _ => DeliveryLeaseStatusDto::Lost,
            },
        )
    }
    async fn claim_target(
        &self,
        target: DeliveryTarget,
        older_than: DateTime<Utc>,
        limit: i32,
        config: &DeliveryScheduleConfigDto,
    ) -> Result<TargetDeliveryBatchDto> {
        self.target_claim_calls
            .lock()
            .unwrap()
            .push((target, older_than, limit));
        if let Some(error) = self.claim_error.lock().unwrap().clone() {
            bail!(error);
        }
        let events = self.unpublished.lock().unwrap().clone();
        for event in events {
            let required = self
                .schedule_required
                .lock()
                .unwrap()
                .entry(event.event_id)
                .or_insert_with(|| {
                    DeliveryTarget::ALL
                        .into_iter()
                        .filter(|t| *t != DeliveryTarget::External || config.external_enabled)
                        .collect()
                })
                .clone();
            if required.iter().all(|t| {
                self.completed
                    .lock()
                    .unwrap()
                    .contains(&(event.event_id, t.as_str().into()))
            }) {
                self.unpublished
                    .lock()
                    .unwrap()
                    .retain(|e| e.event_id != event.event_id);
                let mut published = self.published.lock().unwrap();
                let new = !published.contains(&event.event_id);
                if new {
                    published.push(event.event_id);
                }
                return Ok(TargetDeliveryBatchDto {
                    claims: vec![],
                    reconciled_published_events: i64::from(new),
                });
            }
            let key = (event.event_id, target.as_str().to_owned());
            if !required.contains(&target)
                || self.completed.lock().unwrap().contains(&key)
                || self.schedule_leases.lock().unwrap().contains_key(&key)
                || self
                    .schedule_retries
                    .lock()
                    .unwrap()
                    .get(&key)
                    .is_some_and(|due| *due > older_than)
            {
                continue;
            }
            let claim = TargetDeliveryClaimDto {
                event,
                target,
                lease_token: Uuid::new_v4(),
                attempt_id: Uuid::new_v4(),
                attempt_number: 1,
                lease_expires_at: Utc::now() + chrono::Duration::minutes(5),
            };
            self.delivery_attempts
                .lock()
                .unwrap()
                .push(DeliveryAttempt {
                    attempt_id: claim.attempt_id,
                    event_id: claim.event.event_id,
                    target,
                    source: DeliveryAttemptSource::OutboxWorker,
                    phase: DeliveryAttemptPhase::Started,
                    resolution_id: None,
                    occurred_at: Utc::now(),
                });
            self.schedule_leases
                .lock()
                .unwrap()
                .insert(key, (claim.clone(), older_than, config.clone()));
            return Ok(TargetDeliveryBatchDto {
                claims: vec![claim],
                reconciled_published_events: 0,
            });
        }
        Ok(TargetDeliveryBatchDto::default())
    }
    async fn complete_target(
        &self,
        claim: &TargetDeliveryClaimDto,
    ) -> Result<DeliveryLeaseOutcomeDto> {
        self.fixture_finish(claim, true)
    }
    async fn fail_target(&self, claim: &TargetDeliveryClaimDto) -> Result<DeliveryLeaseOutcomeDto> {
        self.fixture_finish(claim, false)
    }
}

impl FakeOutbox {
    fn fixture_finish(
        &self,
        claim: &TargetDeliveryClaimDto,
        success: bool,
    ) -> Result<DeliveryLeaseOutcomeDto> {
        let key = (claim.event.event_id, claim.target.as_str().to_owned());
        let Some((stored, older_than, config)) = self.schedule_leases.lock().unwrap().remove(&key)
        else {
            return Ok(DeliveryLeaseOutcomeDto::Lost);
        };
        assert_eq!(stored.lease_token, claim.lease_token);
        let mut terminal = self
            .delivery_attempts
            .lock()
            .unwrap()
            .iter()
            .find(|a| a.attempt_id == claim.attempt_id && a.phase == DeliveryAttemptPhase::Started)
            .cloned()
            .unwrap();
        terminal.phase = if success {
            DeliveryAttemptPhase::Succeeded
        } else {
            DeliveryAttemptPhase::Failed
        };
        self.delivery_attempts.lock().unwrap().push(terminal);
        if success {
            self.completed.lock().unwrap().insert(key);
        } else {
            self.schedule_retries.lock().unwrap().insert(
                key,
                older_than + chrono::Duration::milliseconds(config.retry_base_ms as i64),
            );
            self.failures.lock().unwrap().push((
                claim.event.event_id,
                format!("{}: target delivery failed", claim.target.as_str()),
                config.max_attempts,
            ));
        }
        let required = self
            .schedule_required
            .lock()
            .unwrap()
            .get(&claim.event.event_id)
            .cloned()
            .unwrap();
        let complete = required.iter().all(|t| {
            self.completed
                .lock()
                .unwrap()
                .contains(&(claim.event.event_id, t.as_str().into()))
        });
        if complete {
            self.unpublished
                .lock()
                .unwrap()
                .retain(|e| e.event_id != claim.event.event_id);
            self.published.lock().unwrap().push(claim.event.event_id);
        }
        Ok(DeliveryLeaseOutcomeDto::Recorded {
            event_published: complete,
        })
    }
}

#[async_trait]
impl EventOutboxPort for FakeOutbox {
    async fn save(&self, event: &StoredEvent) -> Result<StoredEvent> {
        Ok(event.clone())
    }

    async fn find_events(
        &self,
        filter: &EventReplayFilter,
        limit: i32,
    ) -> Result<Vec<StoredEvent>> {
        self.find_events_calls
            .lock()
            .unwrap()
            .push((filter.clone(), limit));
        Ok(self.events.lock().unwrap().clone())
    }

    async fn find_unpublished(
        &self,
        older_than: DateTime<Utc>,
        limit: i32,
    ) -> Result<Vec<StoredEvent>> {
        self.find_unpublished_calls
            .lock()
            .unwrap()
            .push((older_than, limit));
        if let Some(error) = self.find_unpublished_error.lock().unwrap().clone() {
            bail!(error);
        }
        Ok(self.unpublished.lock().unwrap().clone())
    }

    async fn delivery_completed(&self, event_id: Uuid, target: &str) -> Result<bool> {
        if let Some(error) = self.delivery_completed_error.lock().unwrap().clone() {
            bail!(error);
        }
        Ok(self
            .completed
            .lock()
            .unwrap()
            .contains(&(event_id, target.to_owned())))
    }

    async fn mark_delivery_completed(
        &self,
        event_id: Uuid,
        target: &str,
        _payload: &str,
        _completed_at: DateTime<Utc>,
    ) -> Result<()> {
        self.completed
            .lock()
            .unwrap()
            .insert((event_id, target.to_owned()));
        Ok(())
    }

    async fn append_delivery_attempt(&self, attempt: &DeliveryAttempt) -> Result<()> {
        self.delivery_attempts.lock().unwrap().push(attempt.clone());
        Ok(())
    }

    async fn mark_published(&self, event_id: Uuid, _: DateTime<Utc>) -> Result<()> {
        self.unpublished
            .lock()
            .unwrap()
            .retain(|event| event.event_id != event_id);
        self.published.lock().unwrap().push(event_id);
        Ok(())
    }

    async fn record_publish_failure(
        &self,
        event_id: Uuid,
        error: &str,
        max_attempts: i32,
    ) -> Result<()> {
        self.failures
            .lock()
            .unwrap()
            .push((event_id, error.to_owned(), max_attempts));
        Ok(())
    }

    async fn metrics_snapshot(&self, _: DateTime<Utc>) -> Result<OutboxMetricsSnapshot> {
        Ok(*self.snapshot.lock().unwrap())
    }
}

#[derive(Default)]
pub struct SpyPublisher {
    pub published: Mutex<Vec<Uuid>>,
    pub error: Mutex<Option<String>>,
    pub traceparents: Mutex<Vec<Option<String>>>,
}

#[async_trait]
impl ExternalEventPublisherPort for SpyPublisher {
    async fn publish(&self, event: &StoredEvent) -> Result<()> {
        if let Some(error) = self.error.lock().unwrap().clone() {
            return Err(
                post_service::application::delivery_effect::DeliveryEffectFailure::confirmed(
                    anyhow::anyhow!(error),
                ),
            );
        }
        self.traceparents
            .lock()
            .unwrap()
            .push(platform::trace_context::current().map(|context| context.traceparent()));
        self.published.lock().unwrap().push(event.event_id);
        Ok(())
    }
}

#[derive(Default)]
pub struct SpyInternalDispatcher {
    pub dispatched: Mutex<Vec<Uuid>>,
    pub error: Mutex<Option<String>>,
}

#[async_trait]
impl InternalEventDispatcherPort for SpyInternalDispatcher {
    async fn dispatch(&self, event: &StoredEvent) -> Result<()> {
        if let Some(error) = self.error.lock().unwrap().clone() {
            return Err(
                post_service::application::delivery_effect::DeliveryEffectFailure::confirmed(
                    anyhow::anyhow!(error),
                ),
            );
        }
        self.dispatched.lock().unwrap().push(event.event_id);
        Ok(())
    }
}

#[derive(Default)]
pub struct SpyNotificationClient {
    pub notifications: Mutex<Vec<NotificationRequest>>,
}

#[async_trait]
impl NotificationClientPort for SpyNotificationClient {
    async fn create_notification(&self, request: NotificationRequest) -> Result<()> {
        self.notifications.lock().unwrap().push(request);
        Ok(())
    }
}
