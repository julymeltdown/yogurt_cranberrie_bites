use crate::dto::mutation_dto::{
    MutationOperationDto, MutationOutcomeDto, MutationReceiptStatusDto,
};
use crate::dto::timeline_fanout_dto::{
    TimelineFanoutClaimDto, TimelineFanoutClaimRequestDto, TimelineFanoutFailureDto,
    TimelineFanoutOutcomeDto, TimelineFanoutPageDto,
};
use crate::dto::timeline_projection_dto::{
    TimelineProjectionApplyDto, TimelineProjectionEventDto, TimelineProjectionOutcomeDto,
    TimelineProjectionPlanDto,
};
use crate::query_witness::PublishedWitnessStageDto;
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use post_domain::comment::Comment;
use post_domain::feed::{FeedCachePage, FeedPage};
use post_domain::post::{
    ContentModerationAction, ContentModerationTarget, DeadLetterReplayOutcome,
    DeadLetterReplayResolution, DeliveryAttempt, DeliveryTarget, EventReplayFilter,
    NotificationRequest, OutboxMetricsSnapshot, Post, PostAccessSnapshot, PostAuditEvent,
    PostDetail, RelationshipPolicySnapshot, RetentionPurgeResult, StoredEvent,
};
use post_domain::ranking::{AuthorDeliveryMode, FeedEngagementAction};
use post_domain::recommendation::{
    FeedCandidate, FeedCandidateCollection, FeedRecommendationImpression,
    FeedRecommendationRetentionResult, FeedSnapshot, FeedSnapshotPage, FollowerPage,
};
use recommendation_policy::recommendation::{RecentActionLimit, RolloutMode};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

#[async_trait]
pub trait PostRepositoryPort: Send + Sync {
    async fn mutation_receipt_status(
        &self,
        _operation_id: Uuid,
    ) -> Result<Option<MutationReceiptStatusDto>> {
        anyhow::bail!("Owner mutation receipt status is not supported by this repository")
    }
    async fn lookup_mutation(
        &self,
        _operation: &MutationOperationDto,
    ) -> Result<Option<MutationOutcomeDto>> {
        anyhow::bail!("Owner mutation receipts are not supported by this repository")
    }
    async fn save_post(&self, post: &Post) -> Result<Post>;
    async fn save_post_with_event(
        &self,
        post: &Post,
        event: &StoredEvent,
        operation: Option<&MutationOperationDto>,
    ) -> Result<(Post, StoredEvent)>;
    async fn save_comment(&self, comment: &Comment) -> Result<Comment>;
    async fn save_comment_with_counter_event(
        &self,
        comment: &Comment,
        event: &StoredEvent,
        operation: Option<&MutationOperationDto>,
    ) -> Result<(Comment, StoredEvent)>;
    async fn fetch_comment(&self, comment_id: Uuid) -> Result<Option<Comment>>;
    async fn fetch_feed(
        &self,
        viewer_id: Uuid,
        author_ids: &[Uuid],
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedPage>;
    async fn fetch_by_author(
        &self,
        author_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedPage>;
    async fn search_by_content(
        &self,
        query: &str,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedPage>;
    async fn fetch_trending(&self, cursor: Option<&str>, limit: i32) -> Result<FeedPage>;

    /// Count the posts that currently exist for each author.
    ///
    /// The count is derived from the stored posts at read time rather than kept
    /// as a separate counter, so it cannot drift from the rows it describes.
    /// Authors with no matching posts are absent from the result, which lets a
    /// caller tell "this author has no posts" apart from "this author is
    /// unknown"; reporting a missing author as zero would invent an observation.
    async fn count_posts_by_author(&self, author_ids: &[Uuid]) -> Result<HashMap<Uuid, i64>>;

    /// Return authorization-relevant state for existing, non-deleted, visible
    /// posts. Cached post payloads must not be returned without this check.
    async fn fetch_post_access_snapshots(
        &self,
        post_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, PostAccessSnapshot>>;
    async fn fetch_posts_by_ids(&self, post_ids: &[Uuid]) -> Result<Vec<Post>>;
    async fn fetch_post_header(&self, post_id: Uuid) -> Result<Option<Post>>;
    async fn fetch_post(&self, post_id: Uuid) -> Result<Option<PostDetail>>;
    async fn soft_delete_post(
        &self,
        post_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
    ) -> Result<bool>;
    async fn soft_delete_post_with_event_audit(
        &self,
        post_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
        event: &StoredEvent,
        audit_event: &PostAuditEvent,
        operation: Option<&MutationOperationDto>,
    ) -> Result<bool>;
    async fn soft_delete_comment_with_event_audit(
        &self,
        comment_id: Uuid,
        post_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
        event: &StoredEvent,
        audit_event: &PostAuditEvent,
    ) -> Result<bool>;
    async fn purge_expired_posts(
        &self,
        cutoff: DateTime<Utc>,
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
    ) -> Result<RetentionPurgeResult>;
    async fn record_audit_event(&self, event: &PostAuditEvent) -> Result<()>;
    async fn adjust_like_count(&self, post_id: Uuid, delta: i64) -> Result<()>;
    async fn adjust_comment_count(&self, post_id: Uuid, delta: i64) -> Result<()>;
}

#[async_trait]
pub trait ContentModerationRepositoryPort: Send + Sync {
    async fn apply_content_moderation(
        &self,
        report_id: Uuid,
        target: ContentModerationTarget,
        target_id: Uuid,
        actor_id: Uuid,
        action: ContentModerationAction,
    ) -> Result<Option<Uuid>>;
}

#[async_trait]
pub trait UserLifecycleRepositoryPort: Send + Sync {
    async fn apply_user_deletion(
        &self,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> Result<Vec<Uuid>>;
}

#[async_trait]
pub trait FeedCandidateSourceRepositoryPort: Send + Sync {
    async fn materialized_timeline_candidates(
        &self,
        viewer_id: Uuid,
        limit: i32,
        now: DateTime<Utc>,
    ) -> Result<Vec<FeedCandidate>>;
    async fn high_fanout_candidates(
        &self,
        viewer_id: Uuid,
        following_ids: &[Uuid],
        limit: i32,
        now: DateTime<Utc>,
    ) -> Result<Vec<FeedCandidate>>;
    async fn author_affinity_candidates(
        &self,
        viewer_id: Uuid,
        following_ids: &[Uuid],
        limit: i32,
        now: DateTime<Utc>,
    ) -> Result<Vec<FeedCandidate>>;
    async fn social_proof_candidates(
        &self,
        viewer_id: Uuid,
        following_ids: &[Uuid],
        limit: i32,
        now: DateTime<Utc>,
    ) -> Result<Vec<FeedCandidate>>;
    async fn fresh_trending_candidates(
        &self,
        viewer_id: Uuid,
        limit: i32,
        now: DateTime<Utc>,
    ) -> Result<Vec<FeedCandidate>>;
    async fn exploration_candidates(
        &self,
        viewer_id: Uuid,
        excluded_author_ids: &[Uuid],
        limit: i32,
        now: DateTime<Utc>,
    ) -> Result<Vec<FeedCandidate>>;
}

#[async_trait]
pub trait FeedCandidateCollectorPort: Send + Sync {
    async fn collect(
        &self,
        viewer_id: Uuid,
        following_ids: &[Uuid],
        per_source_limit: i32,
        now: DateTime<Utc>,
    ) -> Result<FeedCandidateCollection>;
}

pub trait FeedRankerPort: Send + Sync {
    fn version(&self) -> &str;
    fn rank(&self, candidates: Vec<FeedCandidate>) -> Result<Vec<(FeedCandidate, i64)>>;
}

pub trait FeedListRerankerPort: Send + Sync {
    fn rerank(
        &self,
        candidates: Vec<(FeedCandidate, i64)>,
        limit: usize,
    ) -> Vec<(FeedCandidate, i64)>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecommendationRollout {
    pub mode: RolloutMode,
    pub deterministic_version: String,
    pub learned_version: Option<String>,
    pub canary_percent: u8,
    pub learned_ready: bool,
}

impl RecommendationRollout {
    pub fn deterministic(version: impl Into<String>) -> Self {
        Self {
            mode: RolloutMode::Deterministic,
            deterministic_version: version.into(),
            learned_version: None,
            canary_percent: 0,
            learned_ready: false,
        }
    }
}

#[async_trait]
pub trait FeedRecommendationRepositoryPort: Send + Sync {
    async fn create_snapshot(&self, snapshot: &FeedSnapshot) -> Result<()>;
    async fn load_snapshot_page(
        &self,
        viewer_id: Uuid,
        snapshot_id: Uuid,
        offset: u32,
        limit: i32,
        now: DateTime<Utc>,
    ) -> Result<Option<FeedSnapshotPage>>;
    async fn record_impressions(&self, impressions: &[FeedRecommendationImpression]) -> Result<()>;
    async fn append_event(
        &self,
        event: &post_domain::recommendation::RecommendationEvent,
    ) -> Result<bool>;
    async fn latest_impression(
        &self,
        viewer_id: Uuid,
        post_id: Uuid,
    ) -> Result<Option<FeedRecommendationImpression>>;
    async fn append_recommendation_request(
        &self,
        request: &crate::dto::recommendation_retry_dto::RecommendationAdmissionDto,
    ) -> Result<bool> {
        let impression = self
            .latest_impression(request.viewer_id, request.post_id)
            .await?
            .filter(|i| request.impression_id.is_none_or(|id| id == i.impression_id))
            .ok_or_else(|| anyhow::anyhow!("Feed impression not found"))?;
        self.append_event(&request.normalize(impression.into())?)
            .await
    }
    async fn cleanup_user(&self, user_id: Uuid) -> Result<()>;
    async fn model_ready(&self, model_version: &str) -> Result<bool>;
    async fn rollout(&self, _surface: &str) -> Result<RecommendationRollout> {
        Ok(RecommendationRollout::deterministic("feed-linear-v1"))
    }
    async fn recent_actions(
        &self,
        _viewer_id: Uuid,
        _limit: RecentActionLimit,
    ) -> Result<Vec<post_domain::recommendation::RecommendationEvent>> {
        Ok(Vec::new())
    }
}

#[async_trait]
pub trait FeedRecommendationMaintenancePort: Send + Sync {
    async fn purge_expired(
        &self,
        now: DateTime<Utc>,
        event_cutoff: DateTime<Utc>,
        limit: i32,
    ) -> Result<FeedRecommendationRetentionResult>;
}

/// Mandatory local TIMELINE consumer. Preparation is advisory; apply must recheck
/// owner identity, ordering and historical evidence inside the effect transaction.
#[async_trait]
pub trait TimelineProjectionRepositoryPort: Send + Sync {
    async fn prepare(
        &self,
        event: &TimelineProjectionEventDto,
        limit: i32,
    ) -> Result<TimelineProjectionPlanDto>;
    async fn apply(
        &self,
        request: &TimelineProjectionApplyDto,
    ) -> Result<TimelineProjectionOutcomeDto>;
}

#[async_trait]
pub trait TimelineFanoutRepositoryPort: Send + Sync {
    async fn enqueue_post(
        &self,
        event_id: Uuid,
        post_id: Uuid,
        author_id: Uuid,
        post_created_at: DateTime<Utc>,
        follower_count: i64,
        high_fanout_threshold: i64,
        now: DateTime<Utc>,
    ) -> Result<AuthorDeliveryMode>;
    async fn claim_jobs(
        &self,
        request: &TimelineFanoutClaimRequestDto,
    ) -> Result<Vec<TimelineFanoutClaimDto>>;
    /// Commit recipient entries and page progress together under exact ownership.
    async fn apply_page(
        &self,
        claim: &TimelineFanoutClaimDto,
        page: &TimelineFanoutPageDto,
    ) -> Result<TimelineFanoutOutcomeDto>;
    async fn record_job_failure(
        &self,
        claim: &TimelineFanoutClaimDto,
        failure: &TimelineFanoutFailureDto,
    ) -> Result<TimelineFanoutOutcomeDto>;
    async fn remove_post(&self, post_id: Uuid) -> Result<()>;
}

#[async_trait]
pub trait FeedFeatureRepositoryPort: Send + Sync {
    /// Apply one engagement event to the ranking features.
    ///
    /// `event_id` identifies the event rather than the effect, because delivery
    /// is at-least-once and the projection is a delta. The implementation records
    /// the event in a consumer inbox inside the same transaction as the update,
    /// so a replay leaves the features unchanged instead of applying the delta
    /// twice.
    async fn record_engagement(
        &self,
        event_id: Uuid,
        post_id: Uuid,
        actor_id: Uuid,
        author_id: Uuid,
        action: FeedEngagementAction,
        now: DateTime<Utc>,
    ) -> Result<()>;
    async fn cleanup_user(&self, user_id: Uuid) -> Result<()>;
}

pub trait FeedEmbeddingRetrieverPort: Send + Sync {
    fn enabled(&self) -> bool;
    fn model_version(&self) -> Option<&str>;
}

pub trait PrimaryFeedRankerPort: Send + Sync {
    fn rollout_mode(&self) -> recommendation_policy::recommendation::RolloutMode;
    fn model_version(&self) -> Option<&str>;
}

#[async_trait]
pub trait PostLikeRepositoryPort: Send + Sync {
    async fn add_like(&self, post_id: Uuid, user_id: Uuid) -> Result<bool>;
    async fn add_like_with_event(
        &self,
        post_id: Uuid,
        user_id: Uuid,
        event: &StoredEvent,
        operation: Option<&MutationOperationDto>,
    ) -> Result<(bool, i64)>;
    async fn remove_like(&self, post_id: Uuid, user_id: Uuid) -> Result<bool>;
    async fn remove_like_with_event(
        &self,
        post_id: Uuid,
        user_id: Uuid,
        event: &StoredEvent,
        operation: Option<&MutationOperationDto>,
    ) -> Result<(bool, i64)>;
    async fn count_likes(&self, post_ids: &[Uuid]) -> Result<HashMap<Uuid, i64>>;
    async fn liked_post_ids(&self, user_id: Uuid, post_ids: &[Uuid]) -> Result<HashSet<Uuid>>;
}

#[async_trait]
pub trait FeedCachePort: Send + Sync {
    async fn get_feed(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<Option<FeedCachePage>>;

    /// The generation a cached page must be filled against.
    ///
    /// A caller reads this *before* computing the page, then passes it to
    /// [FeedCachePort::set_feed]. If a mutation bumps the generation while the
    /// page is being computed, the stored page is refused rather than written
    /// under the new generation, which is what stops a stale fill from
    /// resurrecting data that a mutation just removed.
    async fn current_generation(&self, user_id: Uuid) -> Result<u64>;

    async fn set_feed(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
        page: &FeedCachePage,
        generation: u64,
    ) -> Result<()>;
    async fn invalidate_users(&self, user_ids: &[Uuid]) -> Result<()>;
}

#[async_trait]
pub trait PostCachePort: Send + Sync {
    async fn get_posts(&self, post_ids: &[Uuid]) -> Result<HashMap<Uuid, Post>>;
    async fn set_post(&self, post: &Post) -> Result<()>;
    async fn set_posts(&self, posts: &[Post]) -> Result<()>;
    async fn evict(&self, post_id: Uuid) -> Result<()>;
}

#[async_trait]
pub trait FriendClientPort: Send + Sync {
    /// Friend-owned active follows excluding either-direction blocks and viewer
    /// mutes. This required result is data for the query, never a child grant.
    async fn fetch_feed_authors(
        &self,
        viewer_id: Uuid,
    ) -> Result<crate::feed_authors::FeedAuthorsDto>;
    async fn fetch_following_ids(&self, user_id: Uuid) -> Result<Vec<Uuid>>;
    async fn fetch_following_ids_bounded(
        &self,
        user_id: Uuid,
        max_user_ids: usize,
    ) -> Result<Vec<Uuid>> {
        let mut user_ids = self.fetch_following_ids(user_id).await?;
        user_ids.truncate(max_user_ids.clamp(1, 10_000));
        Ok(user_ids)
    }
    async fn fetch_follower_ids(&self, user_id: Uuid) -> Result<Vec<Uuid>>;
    async fn fetch_follower_page(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FollowerPage> {
        let all = self.fetch_follower_ids(user_id).await?;
        let offset = cursor
            .and_then(|value| value.strip_prefix("offset:"))
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        let page_limit = limit.clamp(1, 100) as usize;
        let end = (offset + page_limit).min(all.len());
        let next_cursor = (end < all.len()).then(|| format!("offset:{end}"));
        Ok(FollowerPage {
            follower_ids: all[offset.min(all.len())..end].to_vec(),
            next_cursor,
            total_count: all.len() as i64,
        })
    }
    async fn follower_count(&self, user_id: Uuid) -> Result<i64> {
        Ok(self.fetch_follower_ids(user_id).await?.len() as i64)
    }
    async fn is_following(&self, follower_id: Uuid, followee_id: Uuid) -> Result<bool>;
    async fn relationship_policy(
        &self,
        viewer_id: Uuid,
        target_id: Uuid,
        stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<RelationshipPolicySnapshot> {
        anyhow::ensure!(
            stage.is_none(),
            "Forwarded relationship policy requires an explicit client adapter"
        );
        Ok(RelationshipPolicySnapshot {
            following: self.is_following(viewer_id, target_id).await?,
            ..RelationshipPolicySnapshot::default()
        })
    }
    async fn batch_relationship_policies(
        &self,
        viewer_id: Uuid,
        target_ids: &[Uuid],
        stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<HashMap<Uuid, RelationshipPolicySnapshot>> {
        let mut output = HashMap::new();
        for target_id in target_ids {
            output.insert(
                *target_id,
                self.relationship_policy(viewer_id, *target_id, stage)
                    .await?,
            );
        }
        Ok(output)
    }
}

#[async_trait]
pub trait NotificationClientPort: Send + Sync {
    async fn create_notification(&self, request: NotificationRequest) -> Result<()>;
}

#[async_trait]
pub trait ProfileClientPort: Send + Sync {
    async fn require_author(
        &self,
        user_id: Uuid,
    ) -> Result<crate::profile_author::ExistingProfileAuthorDto>;
}

#[async_trait]
pub trait EventOutboxPort: Send + Sync {
    async fn save(&self, event: &StoredEvent) -> Result<StoredEvent>;
    async fn find_events(&self, filter: &EventReplayFilter, limit: i32)
        -> Result<Vec<StoredEvent>>;
    async fn find_unpublished(
        &self,
        older_than: DateTime<Utc>,
        limit: i32,
    ) -> Result<Vec<StoredEvent>>;
    async fn resolve_dead_letter(
        &self,
        _resolution: &DeadLetterReplayResolution,
    ) -> Result<DeadLetterReplayOutcome> {
        anyhow::bail!("audited dead-letter replay is unavailable")
    }
    async fn append_delivery_attempt(&self, attempt: &DeliveryAttempt) -> Result<()>;
    async fn delivery_completed(&self, _event_id: Uuid, _target: &str) -> Result<bool> {
        Ok(false)
    }
    async fn mark_delivery_completed(
        &self,
        _event_id: Uuid,
        _target: &str,
        _payload: &str,
        _completed_at: DateTime<Utc>,
    ) -> Result<()> {
        Ok(())
    }
    async fn mark_published(&self, event_id: Uuid, published_at: DateTime<Utc>) -> Result<()>;
    async fn record_publish_failure(
        &self,
        event_id: Uuid,
        error: &str,
        max_attempts: i32,
    ) -> Result<()>;
    async fn metrics_snapshot(&self, now: DateTime<Utc>) -> Result<OutboxMetricsSnapshot>;
}

/// Records bounded-cardinality telemetry for outbox delivery failures.
///
/// Target is a closed domain enum; event identifiers, payloads, and error text
/// deliberately do not become metric labels.
pub trait EventDeliveryMetricsPort: Send + Sync {
    fn record_target_failure(&self, target: DeliveryTarget);
}

/// Claims and immutable attempt markers are one repository transaction.
/// Terminal updates require the same unexpired lease; broker effects remain
/// at-least-once and are deduplicated by their existing owner/inbox identities.
#[async_trait]
pub trait DeliverySchedulePort: Send + Sync {
    async fn lease_status(
        &self,
        claim: &crate::dto::delivery_schedule_dto::TargetDeliveryClaimDto,
    ) -> Result<crate::dto::delivery_schedule_dto::DeliveryLeaseStatusDto>;
    async fn claim_target(
        &self,
        target: DeliveryTarget,
        older_than: DateTime<Utc>,
        limit: i32,
        config: &crate::dto::delivery_schedule_dto::DeliveryScheduleConfigDto,
    ) -> Result<crate::dto::delivery_schedule_dto::TargetDeliveryBatchDto>;
    async fn complete_target(
        &self,
        claim: &crate::dto::delivery_schedule_dto::TargetDeliveryClaimDto,
    ) -> Result<crate::dto::delivery_schedule_dto::DeliveryLeaseOutcomeDto>;
    async fn fail_target(
        &self,
        claim: &crate::dto::delivery_schedule_dto::TargetDeliveryClaimDto,
    ) -> Result<crate::dto::delivery_schedule_dto::DeliveryLeaseOutcomeDto>;
}

/// Manual replay uses the same target lease and completion fence. Claiming
/// must preserve automatic policy/budgets and refuse unaudited dead letters.
#[async_trait]
pub trait ReplaySchedulePort: DeliverySchedulePort {
    async fn claim_replay_target(
        &self,
        event_id: Uuid,
        target: DeliveryTarget,
        config: &crate::dto::delivery_schedule_dto::DeliveryScheduleConfigDto,
    ) -> Result<crate::dto::delivery_schedule_dto::TargetDeliveryClaimDto>;
}

#[async_trait]
pub trait ReplayEventDeliveryPort: Send + Sync {
    async fn replay_target(&self, event_id: Uuid, target: DeliveryTarget) -> Result<()>;
}

#[async_trait]
pub trait ExternalEventPublisherPort: Send + Sync {
    /// Errors must preserve delivery-effect classification. Unclassified errors
    /// are uncertain; only ConfirmedNoEffect permits immediate retry scheduling.
    async fn publish(&self, event: &StoredEvent) -> Result<()>;

    /// Whether this publisher actually delivers events to a broker.
    ///
    /// A deployment may run with external delivery switched off. Saying so
    /// explicitly lets the forwarder treat the external target as not required,
    /// rather than recording it as delivered when nothing was sent. Defaults to
    /// enabled so a publisher has to opt out on purpose rather than by omission.
    fn is_enabled(&self) -> bool {
        true
    }
}

#[async_trait]
pub trait InternalEventDispatcherPort: Send + Sync {
    /// Partial fanout and lost acknowledgements are uncertain outcomes.
    async fn dispatch(&self, event: &StoredEvent) -> Result<()>;
}

macro_rules! arc_impl {
    ($trait_name:ident, $($method:item),* $(,)?) => {
        #[async_trait]
        impl<T> $trait_name for Arc<T>
        where
            T: $trait_name + ?Sized,
        {
            $($method)*
        }
    };
}

arc_impl!(
    PostRepositoryPort,
    async fn mutation_receipt_status(
        &self,
        operation_id: Uuid,
    ) -> Result<Option<MutationReceiptStatusDto>> {
        (**self).mutation_receipt_status(operation_id).await
    },
    async fn lookup_mutation(
        &self,
        operation: &MutationOperationDto,
    ) -> Result<Option<MutationOutcomeDto>> {
        (**self).lookup_mutation(operation).await
    },
    async fn save_post(&self, post: &Post) -> Result<Post> {
        (**self).save_post(post).await
    },
    async fn save_post_with_event(
        &self,
        post: &Post,
        event: &StoredEvent,
        operation: Option<&MutationOperationDto>,
    ) -> Result<(Post, StoredEvent)> {
        (**self).save_post_with_event(post, event, operation).await
    },
    async fn save_comment(&self, comment: &Comment) -> Result<Comment> {
        (**self).save_comment(comment).await
    },
    async fn save_comment_with_counter_event(
        &self,
        comment: &Comment,
        event: &StoredEvent,
        operation: Option<&MutationOperationDto>,
    ) -> Result<(Comment, StoredEvent)> {
        (**self)
            .save_comment_with_counter_event(comment, event, operation)
            .await
    },
    async fn fetch_comment(&self, comment_id: Uuid) -> Result<Option<Comment>> {
        (**self).fetch_comment(comment_id).await
    },
    async fn fetch_feed(
        &self,
        viewer_id: Uuid,
        author_ids: &[Uuid],
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedPage> {
        (**self)
            .fetch_feed(viewer_id, author_ids, cursor, limit)
            .await
    },
    async fn fetch_by_author(
        &self,
        author_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedPage> {
        (**self).fetch_by_author(author_id, cursor, limit).await
    },
    async fn search_by_content(
        &self,
        query: &str,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedPage> {
        (**self).search_by_content(query, cursor, limit).await
    },
    async fn fetch_trending(&self, cursor: Option<&str>, limit: i32) -> Result<FeedPage> {
        (**self).fetch_trending(cursor, limit).await
    },
    async fn count_posts_by_author(&self, author_ids: &[Uuid]) -> Result<HashMap<Uuid, i64>> {
        (**self).count_posts_by_author(author_ids).await
    },
    async fn fetch_post_access_snapshots(
        &self,
        post_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, PostAccessSnapshot>> {
        (**self).fetch_post_access_snapshots(post_ids).await
    },
    async fn fetch_posts_by_ids(&self, post_ids: &[Uuid]) -> Result<Vec<Post>> {
        (**self).fetch_posts_by_ids(post_ids).await
    },
    async fn fetch_post_header(&self, post_id: Uuid) -> Result<Option<Post>> {
        (**self).fetch_post_header(post_id).await
    },
    async fn fetch_post(&self, post_id: Uuid) -> Result<Option<PostDetail>> {
        (**self).fetch_post(post_id).await
    },
    async fn soft_delete_post(
        &self,
        post_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
    ) -> Result<bool> {
        (**self)
            .soft_delete_post(post_id, actor_id, reason, retention_until)
            .await
    },
    async fn soft_delete_post_with_event_audit(
        &self,
        post_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
        event: &StoredEvent,
        audit_event: &PostAuditEvent,
        operation: Option<&MutationOperationDto>,
    ) -> Result<bool> {
        (**self)
            .soft_delete_post_with_event_audit(
                post_id,
                actor_id,
                reason,
                retention_until,
                event,
                audit_event,
                operation,
            )
            .await
    },
    async fn soft_delete_comment_with_event_audit(
        &self,
        comment_id: Uuid,
        post_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
        event: &StoredEvent,
        audit_event: &PostAuditEvent,
    ) -> Result<bool> {
        (**self)
            .soft_delete_comment_with_event_audit(
                comment_id,
                post_id,
                actor_id,
                reason,
                retention_until,
                event,
                audit_event,
            )
            .await
    },
    async fn record_audit_event(&self, event: &PostAuditEvent) -> Result<()> {
        (**self).record_audit_event(event).await
    },
    async fn purge_expired_posts(
        &self,
        cutoff: DateTime<Utc>,
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
    ) -> Result<RetentionPurgeResult> {
        (**self)
            .purge_expired_posts(
                cutoff, actor_id, request_id, ip_address, user_agent, dry_run,
            )
            .await
    },
    async fn adjust_like_count(&self, post_id: Uuid, delta: i64) -> Result<()> {
        (**self).adjust_like_count(post_id, delta).await
    },
    async fn adjust_comment_count(&self, post_id: Uuid, delta: i64) -> Result<()> {
        (**self).adjust_comment_count(post_id, delta).await
    }
);

arc_impl!(
    PostLikeRepositoryPort,
    async fn add_like(&self, post_id: Uuid, user_id: Uuid) -> Result<bool> {
        (**self).add_like(post_id, user_id).await
    },
    async fn add_like_with_event(
        &self,
        post_id: Uuid,
        user_id: Uuid,
        event: &StoredEvent,
        operation: Option<&MutationOperationDto>,
    ) -> Result<(bool, i64)> {
        (**self)
            .add_like_with_event(post_id, user_id, event, operation)
            .await
    },
    async fn remove_like(&self, post_id: Uuid, user_id: Uuid) -> Result<bool> {
        (**self).remove_like(post_id, user_id).await
    },
    async fn remove_like_with_event(
        &self,
        post_id: Uuid,
        user_id: Uuid,
        event: &StoredEvent,
        operation: Option<&MutationOperationDto>,
    ) -> Result<(bool, i64)> {
        (**self)
            .remove_like_with_event(post_id, user_id, event, operation)
            .await
    },
    async fn count_likes(&self, post_ids: &[Uuid]) -> Result<HashMap<Uuid, i64>> {
        (**self).count_likes(post_ids).await
    },
    async fn liked_post_ids(&self, user_id: Uuid, post_ids: &[Uuid]) -> Result<HashSet<Uuid>> {
        (**self).liked_post_ids(user_id, post_ids).await
    }
);

arc_impl!(
    FeedCachePort,
    async fn get_feed(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<Option<FeedCachePage>> {
        (**self).get_feed(user_id, cursor, limit).await
    },
    async fn set_feed(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
        page: &FeedCachePage,
        generation: u64,
    ) -> Result<()> {
        (**self)
            .set_feed(user_id, cursor, limit, page, generation)
            .await
    },
    async fn current_generation(&self, user_id: Uuid) -> Result<u64> {
        (**self).current_generation(user_id).await
    },
    async fn invalidate_users(&self, user_ids: &[Uuid]) -> Result<()> {
        (**self).invalidate_users(user_ids).await
    }
);

arc_impl!(
    PostCachePort,
    async fn get_posts(&self, post_ids: &[Uuid]) -> Result<HashMap<Uuid, Post>> {
        (**self).get_posts(post_ids).await
    },
    async fn set_post(&self, post: &Post) -> Result<()> {
        (**self).set_post(post).await
    },
    async fn set_posts(&self, posts: &[Post]) -> Result<()> {
        (**self).set_posts(posts).await
    },
    async fn evict(&self, post_id: Uuid) -> Result<()> {
        (**self).evict(post_id).await
    }
);

arc_impl!(
    FriendClientPort,
    async fn fetch_following_ids_bounded(
        &self,
        user_id: Uuid,
        max_user_ids: usize,
    ) -> Result<Vec<Uuid>> {
        (**self)
            .fetch_following_ids_bounded(user_id, max_user_ids)
            .await
    },
    async fn fetch_follower_page(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FollowerPage> {
        (**self).fetch_follower_page(user_id, cursor, limit).await
    },
    async fn follower_count(&self, user_id: Uuid) -> Result<i64> {
        (**self).follower_count(user_id).await
    },
    async fn fetch_following_ids(&self, user_id: Uuid) -> Result<Vec<Uuid>> {
        (**self).fetch_following_ids(user_id).await
    },
    async fn fetch_feed_authors(
        &self,
        viewer_id: Uuid,
    ) -> Result<crate::feed_authors::FeedAuthorsDto> {
        (**self).fetch_feed_authors(viewer_id).await
    },
    async fn fetch_follower_ids(&self, user_id: Uuid) -> Result<Vec<Uuid>> {
        (**self).fetch_follower_ids(user_id).await
    },
    async fn is_following(&self, follower_id: Uuid, followee_id: Uuid) -> Result<bool> {
        (**self).is_following(follower_id, followee_id).await
    },
    async fn relationship_policy(
        &self,
        viewer_id: Uuid,
        target_id: Uuid,
        stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<RelationshipPolicySnapshot> {
        (**self)
            .relationship_policy(viewer_id, target_id, stage)
            .await
    },
    async fn batch_relationship_policies(
        &self,
        viewer_id: Uuid,
        target_ids: &[Uuid],
        stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<HashMap<Uuid, RelationshipPolicySnapshot>> {
        (**self)
            .batch_relationship_policies(viewer_id, target_ids, stage)
            .await
    }
);

arc_impl!(
    NotificationClientPort,
    async fn create_notification(&self, request: NotificationRequest) -> Result<()> {
        (**self).create_notification(request).await
    }
);

arc_impl!(
    ProfileClientPort,
    async fn require_author(
        &self,
        user_id: Uuid,
    ) -> Result<crate::profile_author::ExistingProfileAuthorDto> {
        (**self).require_author(user_id).await
    }
);

arc_impl!(
    ReplaySchedulePort,
    async fn claim_replay_target(
        &self,
        event_id: Uuid,
        target: DeliveryTarget,
        config: &crate::dto::delivery_schedule_dto::DeliveryScheduleConfigDto,
    ) -> Result<crate::dto::delivery_schedule_dto::TargetDeliveryClaimDto> {
        (**self).claim_replay_target(event_id, target, config).await
    }
);
arc_impl!(
    ReplayEventDeliveryPort,
    async fn replay_target(&self, event_id: Uuid, target: DeliveryTarget) -> Result<()> {
        (**self).replay_target(event_id, target).await
    }
);

arc_impl!(
    DeliverySchedulePort,
    async fn lease_status(
        &self,
        claim: &crate::dto::delivery_schedule_dto::TargetDeliveryClaimDto,
    ) -> Result<crate::dto::delivery_schedule_dto::DeliveryLeaseStatusDto> {
        (**self).lease_status(claim).await
    },
    async fn claim_target(
        &self,
        target: DeliveryTarget,
        older_than: DateTime<Utc>,
        limit: i32,
        config: &crate::dto::delivery_schedule_dto::DeliveryScheduleConfigDto,
    ) -> Result<crate::dto::delivery_schedule_dto::TargetDeliveryBatchDto> {
        (**self)
            .claim_target(target, older_than, limit, config)
            .await
    },
    async fn complete_target(
        &self,
        claim: &crate::dto::delivery_schedule_dto::TargetDeliveryClaimDto,
    ) -> Result<crate::dto::delivery_schedule_dto::DeliveryLeaseOutcomeDto> {
        (**self).complete_target(claim).await
    },
    async fn fail_target(
        &self,
        claim: &crate::dto::delivery_schedule_dto::TargetDeliveryClaimDto,
    ) -> Result<crate::dto::delivery_schedule_dto::DeliveryLeaseOutcomeDto> {
        (**self).fail_target(claim).await
    }
);

arc_impl!(
    EventOutboxPort,
    async fn save(&self, event: &StoredEvent) -> Result<StoredEvent> {
        (**self).save(event).await
    },
    async fn find_events(
        &self,
        filter: &EventReplayFilter,
        limit: i32,
    ) -> Result<Vec<StoredEvent>> {
        (**self).find_events(filter, limit).await
    },
    async fn find_unpublished(
        &self,
        older_than: DateTime<Utc>,
        limit: i32,
    ) -> Result<Vec<StoredEvent>> {
        (**self).find_unpublished(older_than, limit).await
    },
    async fn resolve_dead_letter(
        &self,
        resolution: &DeadLetterReplayResolution,
    ) -> Result<DeadLetterReplayOutcome> {
        (**self).resolve_dead_letter(resolution).await
    },
    async fn append_delivery_attempt(&self, attempt: &DeliveryAttempt) -> Result<()> {
        (**self).append_delivery_attempt(attempt).await
    },
    async fn delivery_completed(&self, event_id: Uuid, target: &str) -> Result<bool> {
        (**self).delivery_completed(event_id, target).await
    },
    async fn mark_delivery_completed(
        &self,
        event_id: Uuid,
        target: &str,
        payload: &str,
        completed_at: DateTime<Utc>,
    ) -> Result<()> {
        (**self)
            .mark_delivery_completed(event_id, target, payload, completed_at)
            .await
    },
    async fn mark_published(&self, event_id: Uuid, published_at: DateTime<Utc>) -> Result<()> {
        (**self).mark_published(event_id, published_at).await
    },
    async fn record_publish_failure(
        &self,
        event_id: Uuid,
        error: &str,
        max_attempts: i32,
    ) -> Result<()> {
        (**self)
            .record_publish_failure(event_id, error, max_attempts)
            .await
    },
    async fn metrics_snapshot(&self, now: DateTime<Utc>) -> Result<OutboxMetricsSnapshot> {
        (**self).metrics_snapshot(now).await
    }
);

arc_impl!(
    ExternalEventPublisherPort,
    async fn publish(&self, event: &StoredEvent) -> Result<()> {
        (**self).publish(event).await
    },
    fn is_enabled(&self) -> bool {
        (**self).is_enabled()
    }
);

arc_impl!(
    InternalEventDispatcherPort,
    async fn dispatch(&self, event: &StoredEvent) -> Result<()> {
        (**self).dispatch(event).await
    }
);

#[async_trait]
impl<T> UserLifecycleRepositoryPort for Arc<T>
where
    T: UserLifecycleRepositoryPort + ?Sized,
{
    async fn apply_user_deletion(
        &self,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> Result<Vec<Uuid>> {
        (**self)
            .apply_user_deletion(saga_id, user_id, actor_id, retention_until)
            .await
    }
}

#[async_trait]
impl<T> FeedRecommendationMaintenancePort for Arc<T>
where
    T: FeedRecommendationMaintenancePort + ?Sized,
{
    async fn purge_expired(
        &self,
        now: DateTime<Utc>,
        event_cutoff: DateTime<Utc>,
        limit: i32,
    ) -> Result<FeedRecommendationRetentionResult> {
        (**self).purge_expired(now, event_cutoff, limit).await
    }
}
