use crate::dto::friend_dto::FollowActionDto;
use crate::dto::mutation_dto::{MutationOperationDto, MutationReceiptStatusDto};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use friend_domain::follow_counts::FollowCounts;
use friend_domain::follow_edge::{FollowCursor, FollowEdge};
use friend_domain::follow_request::{AcceptFollowRequestOutcome, AccountVisibility, FollowRequest};
use friend_domain::recommendation::{
    PeopleCandidate, PeopleCandidateSource, PeopleRecommendationImpression,
    PeopleRecommendationRetentionResult, PeopleSnapshot, PeopleSnapshotItem,
    ProfileDiscoverySnapshot, RankedPeopleCandidate,
};
use friend_domain::relationship_policy::RelationshipPolicy;
use recommendation_policy::recommendation::{
    RecentActionLimit, RecommendationCursor, RecommendationEvent, RolloutMode,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecommendationRefreshClaim {
    pub viewer_id: Uuid,
    pub claimed_at: DateTime<Utc>,
}

#[async_trait]
pub trait FollowRepositoryPort: Send + Sync {
    async fn follow(&self, edge: &FollowEdge) -> Result<FollowEdge>;
    async fn unfollow(&self, follower_id: Uuid, followee_id: Uuid) -> Result<bool>;
    async fn mutation_receipt_status(
        &self,
        _operation_id: Uuid,
    ) -> Result<Option<MutationReceiptStatusDto>> {
        anyhow::bail!("Friend mutation receipt status is not supported by this repository")
    }
    async fn lookup_mutation(
        &self,
        _operation: &MutationOperationDto,
    ) -> Result<Option<FollowActionDto>> {
        anyhow::bail!("Friend mutation receipts are not supported by this repository")
    }
    async fn follow_with_receipt(
        &self,
        _edge: &FollowEdge,
        _create_request: bool,
        _operation: &MutationOperationDto,
    ) -> Result<FollowActionDto> {
        anyhow::bail!("Atomic friend mutation receipts are not supported by this repository")
    }
    async fn unfollow_with_receipt(
        &self,
        _follower_id: Uuid,
        _followee_id: Uuid,
        _operation: &MutationOperationDto,
    ) -> Result<FollowActionDto> {
        anyhow::bail!("Atomic friend mutation receipts are not supported by this repository")
    }
    async fn list_followers(
        &self,
        user_id: Uuid,
        cursor: Option<FollowCursor>,
        limit: i32,
    ) -> Result<(Vec<FollowEdge>, Option<String>, i64)>;
    async fn list_following(
        &self,
        user_id: Uuid,
        cursor: Option<FollowCursor>,
        limit: i32,
    ) -> Result<(Vec<FollowEdge>, Option<String>, i64)>;
    async fn is_following(&self, follower_id: Uuid, followee_id: Uuid) -> Result<bool>;
    async fn counts(&self, user_id: Uuid) -> Result<FollowCounts>;
    async fn batch_counts(&self, user_ids: &[Uuid]) -> Result<HashMap<Uuid, FollowCounts>>;
}

#[async_trait]
pub trait MutationReceiptMaintenancePort: Send + Sync {
    async fn purge_expired_outcomes(&self, limit: i32) -> Result<i64>;
}

#[async_trait]
pub trait FollowRequestRepositoryPort: Send + Sync {
    async fn create_request(
        &self,
        follower_id: Uuid,
        followee_id: Uuid,
        created_at: DateTime<Utc>,
    ) -> Result<FollowRequest>;
    async fn accept_request(
        &self,
        request_id: Uuid,
        actor_id: Uuid,
        decided_at: DateTime<Utc>,
    ) -> Result<AcceptFollowRequestOutcome>;
    async fn reject_request(
        &self,
        request_id: Uuid,
        actor_id: Uuid,
        decided_at: DateTime<Utc>,
    ) -> Result<Option<FollowRequest>>;
    async fn cancel_request(
        &self,
        request_id: Uuid,
        actor_id: Uuid,
        decided_at: DateTime<Utc>,
    ) -> Result<Option<FollowRequest>>;
    async fn list_incoming_requests(
        &self,
        user_id: Uuid,
        cursor: Option<FollowCursor>,
        limit: i32,
    ) -> Result<(Vec<FollowRequest>, Option<String>)>;
    async fn list_outgoing_requests(
        &self,
        user_id: Uuid,
        cursor: Option<FollowCursor>,
        limit: i32,
    ) -> Result<(Vec<FollowRequest>, Option<String>)>;
}

#[async_trait]
pub trait RelationshipPolicyRepositoryPort: Send + Sync {
    async fn block(
        &self,
        blocker_id: Uuid,
        blocked_id: Uuid,
        created_at: DateTime<Utc>,
    ) -> Result<RelationshipPolicy>;
    async fn unblock(&self, blocker_id: Uuid, blocked_id: Uuid) -> Result<RelationshipPolicy>;
    async fn mute(
        &self,
        muter_id: Uuid,
        muted_id: Uuid,
        created_at: DateTime<Utc>,
    ) -> Result<RelationshipPolicy>;
    async fn unmute(&self, muter_id: Uuid, muted_id: Uuid) -> Result<RelationshipPolicy>;
    async fn policy(&self, viewer_id: Uuid, target_id: Uuid) -> Result<RelationshipPolicy>;
    async fn batch_policy(
        &self,
        viewer_id: Uuid,
        target_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, RelationshipPolicy>>;
}

#[async_trait]
pub trait ProfileDiscoveryPort: Send + Sync {
    async fn batch_profiles(&self, user_ids: &[Uuid]) -> Result<Vec<ProfileDiscoverySnapshot>>;
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

pub trait PeopleRankerPort: Send + Sync {
    fn version(&self) -> &str;
    fn rank(&self, candidates: Vec<PeopleCandidate>) -> Result<Vec<RankedPeopleCandidate>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeopleRecommendationRequestOutcome {
    Success,
    Failure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeopleRecommendationFallbackReason {
    ModelUnavailable,
    EmptyCandidates,
    SequenceUnavailable,
}

pub trait PeopleRecommendationClockPort: Send + Sync {
    fn now_utc(&self) -> DateTime<Utc>;
}

pub trait PeopleRecommendationObserverPort: Send + Sync {
    fn record_request(&self, outcome: PeopleRecommendationRequestOutcome);
    fn record_fallback(&self, reason: PeopleRecommendationFallbackReason);
    fn record_candidates(&self, source: PeopleCandidateSource, count: i64);
    fn observe_snapshot_latency(&self, elapsed: Duration);
}

#[async_trait]
pub trait PeopleRecommendationRepositoryPort: Send + Sync {
    async fn refresh_candidates(
        &self,
        viewer_id: Uuid,
        generated_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        model_version: &str,
        max_candidates: i32,
    ) -> Result<i64>;
    async fn list_candidates(
        &self,
        viewer_id: Uuid,
        now: DateTime<Utc>,
        limit: i32,
    ) -> Result<Vec<PeopleCandidate>>;
    async fn ineligible_candidate_ids(
        &self,
        viewer_id: Uuid,
        candidate_ids: &[Uuid],
    ) -> Result<HashSet<Uuid>>;
    async fn create_snapshot(
        &self,
        snapshot: &PeopleSnapshot,
        items: &[PeopleSnapshotItem],
    ) -> Result<()>;
    async fn find_snapshot(
        &self,
        viewer_id: Uuid,
        snapshot_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<Option<PeopleSnapshot>>;
    async fn load_snapshot_page(
        &self,
        snapshot_id: Uuid,
        cursor: Option<&RecommendationCursor>,
        limit: i32,
    ) -> Result<Vec<PeopleSnapshotItem>>;
    async fn record_impressions(
        &self,
        viewer_id: Uuid,
        snapshot_id: Uuid,
        impressions: &[PeopleRecommendationImpression],
    ) -> Result<()>;
    async fn enqueue_refresh(&self, viewer_ids: &[Uuid], now: DateTime<Utc>) -> Result<()>;
    async fn claim_refresh_batch(
        &self,
        now: DateTime<Utc>,
        lease_timeout: chrono::Duration,
        limit: i32,
    ) -> Result<Vec<RecommendationRefreshClaim>>;
    async fn complete_refresh(&self, claim: RecommendationRefreshClaim) -> Result<()>;
    async fn fail_refresh(
        &self,
        claim: RecommendationRefreshClaim,
        error: &str,
        next_attempt_at: DateTime<Utc>,
        max_attempts: i32,
    ) -> Result<()>;
    async fn rollout(&self, _surface: &str) -> Result<RecommendationRollout> {
        Ok(RecommendationRollout::deterministic("people-linear-v1"))
    }
    async fn recent_actions(
        &self,
        _viewer_id: Uuid,
        _limit: RecentActionLimit,
    ) -> Result<Vec<RecommendationEvent>> {
        Ok(Vec::new())
    }
}

#[async_trait]
pub trait RecommendationEventRepositoryPort: Send + Sync {
    async fn record_event(&self, event: &RecommendationEvent) -> Result<bool>;
}

#[async_trait]
pub trait PeopleRecommendationMaintenancePort: Send + Sync {
    async fn purge_expired(
        &self,
        now: DateTime<Utc>,
        event_cutoff: DateTime<Utc>,
        limit: i32,
    ) -> Result<PeopleRecommendationRetentionResult>;
}

#[async_trait]
pub trait UserLifecycleRepositoryPort: Send + Sync {
    async fn apply_user_deletion(
        &self,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> Result<()>;
}

#[async_trait]
pub trait ProfilePrivacyPort: Send + Sync {
    async fn account_visibility(&self, user_id: Uuid) -> Result<AccountVisibility>;
}

#[async_trait]
impl<T> FollowRepositoryPort for Arc<T>
where
    T: FollowRepositoryPort + ?Sized,
{
    async fn follow(&self, edge: &FollowEdge) -> Result<FollowEdge> {
        (**self).follow(edge).await
    }
    async fn unfollow(&self, follower_id: Uuid, followee_id: Uuid) -> Result<bool> {
        (**self).unfollow(follower_id, followee_id).await
    }
    async fn mutation_receipt_status(
        &self,
        operation_id: Uuid,
    ) -> Result<Option<MutationReceiptStatusDto>> {
        (**self).mutation_receipt_status(operation_id).await
    }
    async fn lookup_mutation(
        &self,
        operation: &MutationOperationDto,
    ) -> Result<Option<FollowActionDto>> {
        (**self).lookup_mutation(operation).await
    }
    async fn follow_with_receipt(
        &self,
        edge: &FollowEdge,
        create_request: bool,
        operation: &MutationOperationDto,
    ) -> Result<FollowActionDto> {
        (**self)
            .follow_with_receipt(edge, create_request, operation)
            .await
    }
    async fn unfollow_with_receipt(
        &self,
        follower_id: Uuid,
        followee_id: Uuid,
        operation: &MutationOperationDto,
    ) -> Result<FollowActionDto> {
        (**self)
            .unfollow_with_receipt(follower_id, followee_id, operation)
            .await
    }
    async fn list_followers(
        &self,
        user_id: Uuid,
        cursor: Option<FollowCursor>,
        limit: i32,
    ) -> Result<(Vec<FollowEdge>, Option<String>, i64)> {
        (**self).list_followers(user_id, cursor, limit).await
    }
    async fn list_following(
        &self,
        user_id: Uuid,
        cursor: Option<FollowCursor>,
        limit: i32,
    ) -> Result<(Vec<FollowEdge>, Option<String>, i64)> {
        (**self).list_following(user_id, cursor, limit).await
    }
    async fn is_following(&self, follower_id: Uuid, followee_id: Uuid) -> Result<bool> {
        (**self).is_following(follower_id, followee_id).await
    }
    async fn counts(&self, user_id: Uuid) -> Result<FollowCounts> {
        (**self).counts(user_id).await
    }
    async fn batch_counts(&self, user_ids: &[Uuid]) -> Result<HashMap<Uuid, FollowCounts>> {
        (**self).batch_counts(user_ids).await
    }
}

#[async_trait]
impl<T> FollowRequestRepositoryPort for Arc<T>
where
    T: FollowRequestRepositoryPort + ?Sized,
{
    async fn create_request(
        &self,
        follower_id: Uuid,
        followee_id: Uuid,
        created_at: DateTime<Utc>,
    ) -> Result<FollowRequest> {
        (**self)
            .create_request(follower_id, followee_id, created_at)
            .await
    }
    async fn accept_request(
        &self,
        request_id: Uuid,
        actor_id: Uuid,
        decided_at: DateTime<Utc>,
    ) -> Result<AcceptFollowRequestOutcome> {
        (**self)
            .accept_request(request_id, actor_id, decided_at)
            .await
    }
    async fn reject_request(
        &self,
        request_id: Uuid,
        actor_id: Uuid,
        decided_at: DateTime<Utc>,
    ) -> Result<Option<FollowRequest>> {
        (**self)
            .reject_request(request_id, actor_id, decided_at)
            .await
    }
    async fn cancel_request(
        &self,
        request_id: Uuid,
        actor_id: Uuid,
        decided_at: DateTime<Utc>,
    ) -> Result<Option<FollowRequest>> {
        (**self)
            .cancel_request(request_id, actor_id, decided_at)
            .await
    }
    async fn list_incoming_requests(
        &self,
        user_id: Uuid,
        cursor: Option<FollowCursor>,
        limit: i32,
    ) -> Result<(Vec<FollowRequest>, Option<String>)> {
        (**self)
            .list_incoming_requests(user_id, cursor, limit)
            .await
    }
    async fn list_outgoing_requests(
        &self,
        user_id: Uuid,
        cursor: Option<FollowCursor>,
        limit: i32,
    ) -> Result<(Vec<FollowRequest>, Option<String>)> {
        (**self)
            .list_outgoing_requests(user_id, cursor, limit)
            .await
    }
}

#[async_trait]
impl<T> RelationshipPolicyRepositoryPort for Arc<T>
where
    T: RelationshipPolicyRepositoryPort + ?Sized,
{
    async fn block(
        &self,
        blocker_id: Uuid,
        blocked_id: Uuid,
        created_at: DateTime<Utc>,
    ) -> Result<RelationshipPolicy> {
        (**self).block(blocker_id, blocked_id, created_at).await
    }
    async fn unblock(&self, blocker_id: Uuid, blocked_id: Uuid) -> Result<RelationshipPolicy> {
        (**self).unblock(blocker_id, blocked_id).await
    }
    async fn mute(
        &self,
        muter_id: Uuid,
        muted_id: Uuid,
        created_at: DateTime<Utc>,
    ) -> Result<RelationshipPolicy> {
        (**self).mute(muter_id, muted_id, created_at).await
    }
    async fn unmute(&self, muter_id: Uuid, muted_id: Uuid) -> Result<RelationshipPolicy> {
        (**self).unmute(muter_id, muted_id).await
    }
    async fn policy(&self, viewer_id: Uuid, target_id: Uuid) -> Result<RelationshipPolicy> {
        (**self).policy(viewer_id, target_id).await
    }
    async fn batch_policy(
        &self,
        viewer_id: Uuid,
        target_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, RelationshipPolicy>> {
        (**self).batch_policy(viewer_id, target_ids).await
    }
}

#[async_trait]
impl<T> ProfilePrivacyPort for Arc<T>
where
    T: ProfilePrivacyPort + ?Sized,
{
    async fn account_visibility(&self, user_id: Uuid) -> Result<AccountVisibility> {
        (**self).account_visibility(user_id).await
    }
}

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
    ) -> Result<()> {
        (**self)
            .apply_user_deletion(saga_id, user_id, actor_id, retention_until)
            .await
    }
}

#[async_trait]
impl<T> ProfileDiscoveryPort for Arc<T>
where
    T: ProfileDiscoveryPort + ?Sized,
{
    async fn batch_profiles(&self, user_ids: &[Uuid]) -> Result<Vec<ProfileDiscoverySnapshot>> {
        (**self).batch_profiles(user_ids).await
    }
}

#[async_trait]
impl<T> PeopleRecommendationRepositoryPort for Arc<T>
where
    T: PeopleRecommendationRepositoryPort + ?Sized,
{
    async fn refresh_candidates(
        &self,
        viewer_id: Uuid,
        generated_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        model_version: &str,
        max_candidates: i32,
    ) -> Result<i64> {
        (**self)
            .refresh_candidates(
                viewer_id,
                generated_at,
                expires_at,
                model_version,
                max_candidates,
            )
            .await
    }

    async fn list_candidates(
        &self,
        viewer_id: Uuid,
        now: DateTime<Utc>,
        limit: i32,
    ) -> Result<Vec<PeopleCandidate>> {
        (**self).list_candidates(viewer_id, now, limit).await
    }

    async fn ineligible_candidate_ids(
        &self,
        viewer_id: Uuid,
        candidate_ids: &[Uuid],
    ) -> Result<HashSet<Uuid>> {
        (**self)
            .ineligible_candidate_ids(viewer_id, candidate_ids)
            .await
    }

    async fn create_snapshot(
        &self,
        snapshot: &PeopleSnapshot,
        items: &[PeopleSnapshotItem],
    ) -> Result<()> {
        (**self).create_snapshot(snapshot, items).await
    }

    async fn find_snapshot(
        &self,
        viewer_id: Uuid,
        snapshot_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<Option<PeopleSnapshot>> {
        (**self).find_snapshot(viewer_id, snapshot_id, now).await
    }

    async fn load_snapshot_page(
        &self,
        snapshot_id: Uuid,
        cursor: Option<&RecommendationCursor>,
        limit: i32,
    ) -> Result<Vec<PeopleSnapshotItem>> {
        (**self)
            .load_snapshot_page(snapshot_id, cursor, limit)
            .await
    }

    async fn record_impressions(
        &self,
        viewer_id: Uuid,
        snapshot_id: Uuid,
        impressions: &[PeopleRecommendationImpression],
    ) -> Result<()> {
        (**self)
            .record_impressions(viewer_id, snapshot_id, impressions)
            .await
    }

    async fn enqueue_refresh(&self, viewer_ids: &[Uuid], now: DateTime<Utc>) -> Result<()> {
        (**self).enqueue_refresh(viewer_ids, now).await
    }

    async fn claim_refresh_batch(
        &self,
        now: DateTime<Utc>,
        lease_timeout: chrono::Duration,
        limit: i32,
    ) -> Result<Vec<RecommendationRefreshClaim>> {
        (**self)
            .claim_refresh_batch(now, lease_timeout, limit)
            .await
    }

    async fn complete_refresh(&self, claim: RecommendationRefreshClaim) -> Result<()> {
        (**self).complete_refresh(claim).await
    }

    async fn fail_refresh(
        &self,
        claim: RecommendationRefreshClaim,
        error: &str,
        next_attempt_at: DateTime<Utc>,
        max_attempts: i32,
    ) -> Result<()> {
        (**self)
            .fail_refresh(claim, error, next_attempt_at, max_attempts)
            .await
    }

    async fn rollout(&self, surface: &str) -> Result<RecommendationRollout> {
        (**self).rollout(surface).await
    }

    async fn recent_actions(
        &self,
        viewer_id: Uuid,
        limit: RecentActionLimit,
    ) -> Result<Vec<RecommendationEvent>> {
        (**self).recent_actions(viewer_id, limit).await
    }
}

#[async_trait]
impl<T> RecommendationEventRepositoryPort for Arc<T>
where
    T: RecommendationEventRepositoryPort + ?Sized,
{
    async fn record_event(&self, event: &RecommendationEvent) -> Result<bool> {
        (**self).record_event(event).await
    }
}

#[async_trait]
impl<T> PeopleRecommendationMaintenancePort for Arc<T>
where
    T: PeopleRecommendationMaintenancePort + ?Sized,
{
    async fn purge_expired(
        &self,
        now: DateTime<Utc>,
        event_cutoff: DateTime<Utc>,
        limit: i32,
    ) -> Result<PeopleRecommendationRetentionResult> {
        (**self).purge_expired(now, event_cutoff, limit).await
    }
}

#[async_trait]
impl<T> MutationReceiptMaintenancePort for Arc<T>
where
    T: MutationReceiptMaintenancePort + ?Sized,
{
    async fn purge_expired_outcomes(&self, limit: i32) -> Result<i64> {
        (**self).purge_expired_outcomes(limit).await
    }
}
