use crate::application::delivery_effect::DeliveryEffectFailure;
use crate::application::dto::delivery_schedule_dto::{
    DeliveryExecutionConfigDto, TargetDeliveryClaimDto,
};
use crate::application::dto::timeline_projection_dto::{
    TimelineProjectionApplyDto, TimelineProjectionEffectDto, TimelineProjectionEventDto,
    TimelineProjectionPlanDto,
};
use crate::application::ports::{
    DeliverySchedulePort, EventDeliveryMetricsPort, ExternalEventPublisherPort, FeedCachePort,
    FriendClientPort, InternalEventDispatcherPort, PostCachePort, TimelineProjectionRepositoryPort,
};
use crate::domain::post::DeliveryTarget;
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::sync::Arc;
use tracing::Instrument;
use uuid::Uuid;

#[derive(Clone)]
pub struct ExternalEventForwarder {
    claimed_notifications:
        Option<Arc<dyn post_application::claimed_notification::ClaimedNotificationDeliveryPort>>,
    claimed_cache: Option<Arc<dyn post_application::cache_authority::ClaimedCacheInvalidationPort>>,
    schedule: Arc<dyn DeliverySchedulePort>,
    publisher: Arc<dyn ExternalEventPublisherPort>,
    dispatcher: Arc<dyn InternalEventDispatcherPort>,
    feed_cache: Arc<dyn FeedCachePort>,
    post_cache: Arc<dyn PostCachePort>,
    friend_client: Arc<dyn FriendClientPort>,
    timeline_repository: Arc<dyn TimelineProjectionRepositoryPort>,
    delivery_metrics: Option<Arc<dyn EventDeliveryMetricsPort>>,
    high_fanout_threshold: i64,
    execution: DeliveryExecutionConfigDto,
}

impl ExternalEventForwarder {
    pub fn new(
        schedule: Arc<dyn DeliverySchedulePort>,
        publisher: Arc<dyn ExternalEventPublisherPort>,
        dispatcher: Arc<dyn InternalEventDispatcherPort>,
        feed_cache: Arc<dyn FeedCachePort>,
        post_cache: Arc<dyn PostCachePort>,
        friend_client: Arc<dyn FriendClientPort>,
        timeline_repository: Arc<dyn TimelineProjectionRepositoryPort>,
    ) -> Self {
        Self {
            claimed_cache: None,
            claimed_notifications: None,
            schedule,
            publisher,
            dispatcher,
            feed_cache,
            post_cache,
            friend_client,
            timeline_repository,
            delivery_metrics: None,
            high_fanout_threshold: 10_000,
            execution: DeliveryExecutionConfigDto::default(),
        }
    }

    pub fn with_delivery_metrics(mut self, metrics: Arc<dyn EventDeliveryMetricsPort>) -> Self {
        self.delivery_metrics = Some(metrics);
        self
    }

    /// Explicit selection applies only to CACHE. A denial never falls back to
    /// the general Friend client; other targets retain their existing paths.
    pub fn with_claimed_cache(
        mut self,
        cache: Arc<dyn post_application::cache_authority::ClaimedCacheInvalidationPort>,
    ) -> Self {
        self.claimed_cache = Some(cache);
        self
    }

    /// Explicit INTERNAL selection retains the full owning claim. A selected
    /// denial never falls back to the event-only legacy dispatcher.
    pub fn with_claimed_notifications(
        mut self,
        notifications: Arc<
            dyn post_application::claimed_notification::ClaimedNotificationDeliveryPort,
        >,
    ) -> Self {
        self.claimed_notifications = Some(notifications);
        self
    }

    pub fn with_high_fanout_threshold(mut self, threshold: i64) -> Self {
        self.high_fanout_threshold = threshold.max(1);
        self
    }

    pub fn with_execution_config(mut self, config: DeliveryExecutionConfigDto) -> Result<Self> {
        config.validate()?;
        self.execution = config;
        Ok(self)
    }

    /// Convenience drain for callers/tests. Production uses one independent
    /// periodic loop per target and never waits for this aggregate helper.
    pub async fn republish_unpublished(
        &self,
        older_than: DateTime<Utc>,
        limit: i32,
    ) -> Result<i64> {
        // A single diagnostic drain visits every target without skipping its
        // own briefly locked parent. Production loops remain independent.
        let mut published = 0;
        for target in [
            DeliveryTarget::Timeline,
            DeliveryTarget::Cache,
            DeliveryTarget::Internal,
            DeliveryTarget::External,
        ] {
            published += self.republish_target(target, older_than, limit).await?;
        }
        Ok(published)
    }

    pub async fn republish_target(
        &self,
        target: DeliveryTarget,
        older_than: DateTime<Utc>,
        limit: i32,
    ) -> Result<i64> {
        anyhow::ensure!(
            (1..=10_000).contains(&limit),
            "invalid outbox delivery limit"
        );
        let mut config = self.execution.schedule.clone();
        config.external_enabled = self.publisher.is_enabled();
        let mut published = 0;
        for _ in 0..limit {
            // Only one lease is acquired immediately before its effect. Never
            // queue leases while earlier deliveries consume their lifetime.
            let mut batch = self
                .schedule
                .claim_target(target, older_than, 1, &config)
                .await?;
            published += batch.reconciled_published_events;
            if let Some(claim) = batch.claims.pop() {
                anyhow::ensure!(
                    batch.claims.is_empty() && claim.target == target,
                    "scheduler returned an invalid target claim batch"
                );
                published += i64::from(self.forward_claim(&claim).await?);
            } else if batch.reconciled_published_events == 0 {
                break;
            }
        }
        Ok(published)
    }

    async fn forward_claim(&self, claim: &TargetDeliveryClaimDto) -> Result<bool> {
        let mut context = claim
            .event
            .origin_traceparent
            .as_deref()
            .and_then(platform::trace_context::TraceContext::child_from_traceparent);
        let span = tracing::info_span!(
            target: "post_service::adapters::in::messaging::external_event_forwarder",
            parent: None,
            "post_outbox_delivery",
            trace_id = tracing::field::Empty,
            span_id = tracing::field::Empty,
            parent_span_id = tracing::field::Empty,
            sampled = tracing::field::Empty,
            delivery_target = claim.target.as_str(),
            outcome = tracing::field::Empty,
        );
        if let Some(context) = context.as_mut() {
            context.bind_to_span(&span);
            context.record_on_span(&span);
        }
        let operation = self.execute_claim(claim).instrument(span.clone());
        let result = if let Some(context) = context {
            platform::trace_context::scope(context, operation).await
        } else {
            operation.await
        };
        match result {
            Ok((published, outcome)) => {
                span.record("outcome", outcome);
                Ok(published)
            }
            Err(error) => {
                span.record("outcome", "dependency_error");
                Err(error)
            }
        }
    }

    async fn execute_claim(&self, claim: &TargetDeliveryClaimDto) -> Result<(bool, &'static str)> {
        let result = super::fenced_delivery::execute(
            self.schedule.as_ref(),
            &self.execution,
            self.delivery_metrics.as_deref(),
            claim,
            |deadline| self.deliver(claim, deadline),
        )
        .await?;
        Ok((result.event_published(), result.label()))
    }

    /// Deliver one event to one target.
    async fn deliver(
        &self,
        claim: &TargetDeliveryClaimDto,
        deadline: post_application::delivery_effect::DeliveryEffectDeadlineDto,
    ) -> Result<()> {
        let event = &claim.event;
        match claim.target {
            DeliveryTarget::Timeline => self.project_feed_event(claim).await,
            DeliveryTarget::Cache => match &self.claimed_cache {
                Some(cache) => cache.invalidate_claimed_cache(claim, deadline).await,
                None => self.invalidate_caches(event).await,
            },
            DeliveryTarget::Internal => match &self.claimed_notifications {
                Some(notifications) => {
                    notifications
                        .deliver_claimed_notifications(claim, deadline)
                        .await
                }
                None => self.dispatcher.dispatch(event).await,
            },
            DeliveryTarget::External => self.publisher.publish(event).await,
        }
    }

    async fn project_feed_event(&self, claim: &TargetDeliveryClaimDto) -> Result<()> {
        let requested = TimelineProjectionEventDto::from(&claim.event);
        let mut effect_started = false;
        loop {
            let plan = self
                .timeline_repository
                .prepare(&requested, 100)
                .await
                .map_err(|error| {
                    if effect_started {
                        DeliveryEffectFailure::uncertain(error)
                    } else {
                        DeliveryEffectFailure::confirmed(error)
                    }
                })?;
            let TimelineProjectionPlanDto::Events(works) = plan else {
                return Ok(());
            };
            anyhow::ensure!(
                !works.is_empty() && works.len() <= 100,
                "Invalid projection preparation batch"
            );
            let mut reached = false;
            for work in works {
                anyhow::ensure!(
                    work.event.aggregate_id == requested.aggregate_id,
                    "Projection returned another aggregate"
                );
                let follower_count = match work.effect {
                    TimelineProjectionEffectDto::Create { author_id } => {
                        match self.friend_client.follower_count(author_id).await {
                            Ok(count) => Some(count),
                            Err(error) if effect_started => {
                                return Err(DeliveryEffectFailure::uncertain(error))
                            }
                            Err(error) => return Err(DeliveryEffectFailure::confirmed(error)),
                        }
                    }
                    _ => None,
                };
                reached |= work.event == requested;
                effect_started = true;
                self.timeline_repository
                    .apply(&TimelineProjectionApplyDto {
                        authority: Some(claim.clone()),
                        work,
                        follower_count,
                        high_fanout_threshold: self.high_fanout_threshold,
                    })
                    .await
                    .map_err(DeliveryEffectFailure::uncertain)?;
            }
            if reached {
                return Ok(());
            }
            // Continue bounded source reads under the ORIGINAL effect deadline.
            // A batch boundary is progress, not a failed delivery attempt.
        }
    }

    async fn invalidate_caches(&self, event: &crate::domain::post::StoredEvent) -> Result<()> {
        // Validate all fields before the first eviction, so malformed input
        // can be a confirmed failure without masking a partial cache effect.
        let payload: Value = (|| -> Result<Value> {
            let payload: Value = serde_json::from_str(&event.payload)?;
            let fields: &[&str] = match event.event_type.as_str() {
                "POST_CREATED" => &["authorId"],
                "POST_COMMENTED" => &["postId", "notifyUserId"],
                "POST_COMMENT_DELETED" => &["postId", "postAuthorId"],
                "POST_DELETED" => &["postId", "actorId"],
                "POST_LIKED" | "POST_UNLIKED" => &["postId"],
                _ => &[],
            };
            for field in fields {
                uuid_field(&payload, field)?;
            }
            Ok(payload)
        })()
        .map_err(DeliveryEffectFailure::confirmed)?;
        match event.event_type.as_str() {
            "POST_CREATED" => {
                let author_id = uuid_field(&payload, "authorId")?;
                self.invalidate_feed_for_author(author_id).await?;
            }
            "POST_COMMENTED" => {
                self.post_cache
                    .evict(uuid_field(&payload, "postId")?)
                    .await?;
                self.invalidate_feed_for_author(uuid_field(&payload, "notifyUserId")?)
                    .await
                    .map_err(DeliveryEffectFailure::uncertain)?;
            }
            "POST_COMMENT_DELETED" => {
                self.post_cache
                    .evict(uuid_field(&payload, "postId")?)
                    .await?;
                self.invalidate_feed_for_author(uuid_field(&payload, "postAuthorId")?)
                    .await
                    .map_err(DeliveryEffectFailure::uncertain)?;
            }
            "POST_DELETED" => {
                self.post_cache
                    .evict(uuid_field(&payload, "postId")?)
                    .await?;
                self.invalidate_feed_for_author(uuid_field(&payload, "actorId")?)
                    .await
                    .map_err(DeliveryEffectFailure::uncertain)?;
            }
            "POST_LIKED" | "POST_UNLIKED" => {
                self.post_cache
                    .evict(uuid_field(&payload, "postId")?)
                    .await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn invalidate_feed_for_author(&self, author_id: Uuid) -> Result<()> {
        let followers = self.friend_client.fetch_follower_ids(author_id).await?;
        let mut affected = followers;
        affected.push(author_id);
        self.feed_cache.invalidate_users(&affected).await
    }
}

fn uuid_field(payload: &Value, field: &str) -> Result<Uuid> {
    let Some(value) = payload.get(field).and_then(Value::as_str) else {
        bail!("Missing {field}");
    };
    Ok(Uuid::parse_str(value)?)
}
