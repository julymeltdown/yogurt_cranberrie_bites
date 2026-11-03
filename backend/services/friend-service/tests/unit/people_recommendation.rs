use anyhow::Result;
use async_trait::async_trait;
use chrono::{Duration, TimeZone, Utc};
use friend_service::adapters::out::metrics::recommendation_observer::RecommendationMetricsObserver;
use friend_service::application::ports::{
    PeopleRankerPort, PeopleRecommendationClockPort, PeopleRecommendationFallbackReason,
    PeopleRecommendationObserverPort, PeopleRecommendationRepositoryPort,
    PeopleRecommendationRequestOutcome, ProfileDiscoveryPort, RecommendationRefreshClaim,
    RecommendationRollout, RelationshipPolicyRepositoryPort,
};
use friend_service::application::ranking::LinearPeopleRanker;
use friend_service::application::use_cases::get_people_recommendations::{
    GetPeopleRecommendationsQuery, GetPeopleRecommendationsUseCase, PeopleSequenceConfig,
};
use friend_service::domain::recommendation::{
    rank_people_candidates, PeopleCandidate, PeopleCandidateFeatures, PeopleCandidateSource,
    PeopleRankWeights, PeopleSnapshot, PeopleSnapshotItem, ProfileDiscoverySnapshot,
};
use friend_service::domain::relationship_policy::RelationshipPolicy;
use platform::recommendation::{
    RecentActionLimit, RecommendationAction, RecommendationCursor, RecommendationEvent,
    RecommendationSubjectType, RecommendationSurface, RolloutMode, RECOMMENDATION_SCHEMA_VERSION,
};
use platform::recommendation_metrics::RecommendationMetrics;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration as StdDuration;
use uuid::Uuid;

fn uuid(value: u128) -> Uuid {
    Uuid::from_u128(value)
}

fn test_now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap()
}

#[derive(Clone, Copy, Default)]
struct FixedRecommendationClock;

impl PeopleRecommendationClockPort for FixedRecommendationClock {
    fn now_utc(&self) -> chrono::DateTime<Utc> {
        test_now()
    }
}

fn test_clock() -> Arc<dyn PeopleRecommendationClockPort> {
    Arc::new(FixedRecommendationClock)
}

#[derive(Clone, Copy, Default)]
struct NoopPeopleRecommendationObserver;

impl PeopleRecommendationObserverPort for NoopPeopleRecommendationObserver {
    fn record_request(&self, _outcome: PeopleRecommendationRequestOutcome) {}

    fn record_fallback(&self, _reason: PeopleRecommendationFallbackReason) {}

    fn record_candidates(&self, _source: PeopleCandidateSource, _count: i64) {}

    fn observe_snapshot_latency(&self, _elapsed: StdDuration) {}
}

fn noop_observer() -> Arc<dyn PeopleRecommendationObserverPort> {
    Arc::new(NoopPeopleRecommendationObserver)
}

fn test_ranker() -> Arc<dyn PeopleRankerPort> {
    Arc::new(LinearPeopleRanker::new(PeopleRankWeights::default()))
}

fn candidate(
    viewer_id: Uuid,
    candidate_user_id: Uuid,
    source: PeopleCandidateSource,
    source_score: f64,
    mutual_cluster: Option<Uuid>,
) -> PeopleCandidate {
    PeopleCandidate {
        viewer_id,
        candidate_user_id,
        source,
        source_score,
        features: PeopleCandidateFeatures {
            mutual_count: 3,
            mutual_score: 0.7,
            adamic_adar: 0.5,
            jaccard: 0.4,
            profile_overlap: 0.0,
            interaction_affinity: 0.2,
            freshness: 0.8,
            candidate_quality: 0.7,
            impression_penalty: 0.0,
            dismissal_penalty: 0.0,
        },
        mutual_cluster,
        generated_at: Utc.with_ymd_and_hms(2026, 8, 4, 1, 0, 0).unwrap(),
        expires_at: Utc.with_ymd_and_hms(2026, 8, 5, 1, 0, 0).unwrap(),
        model_version: "people-linear-v1".into(),
    }
}

fn recommendation_event(
    event_id: u128,
    viewer_id: Uuid,
    candidate_user_id: Uuid,
    action: RecommendationAction,
    occurred_at: chrono::DateTime<Utc>,
) -> RecommendationEvent {
    RecommendationEvent {
        event_id: uuid(event_id),
        schema_version: RECOMMENDATION_SCHEMA_VERSION,
        occurred_at,
        viewer_id,
        subject_type: RecommendationSubjectType::User,
        subject_id: candidate_user_id,
        actor_id: Some(viewer_id),
        surface: RecommendationSurface::PeopleRecommendation,
        position: None,
        request_id: Some("sequence-request".into()),
        session_id: None,
        action,
        source: "MUTUAL_FOLLOW".into(),
        model_version: "people-linear-v1".into(),
        impression_id: Some(uuid(event_id + 10_000)),
        propensity_micros: None,
        dwell_ms: None,
        metadata_json: "{}".into(),
    }
}

#[test]
fn deterministic_people_ranker_applies_negative_feedback_and_source_quota() {
    let viewer = uuid(1);
    let duplicate = uuid(2);
    let cluster = uuid(90);
    let candidates = vec![
        candidate(
            viewer,
            duplicate,
            PeopleCandidateSource::MutualFollow,
            0.9,
            Some(cluster),
        ),
        candidate(
            viewer,
            duplicate,
            PeopleCandidateSource::CoEngagement,
            0.7,
            None,
        ),
        candidate(
            viewer,
            uuid(3),
            PeopleCandidateSource::MutualFollow,
            0.8,
            Some(cluster),
        ),
        candidate(
            viewer,
            uuid(4),
            PeopleCandidateSource::MutualFollow,
            0.75,
            Some(cluster),
        ),
        candidate(
            viewer,
            uuid(5),
            PeopleCandidateSource::MutualFollow,
            0.7,
            Some(cluster),
        ),
        candidate(viewer, uuid(6), PeopleCandidateSource::NewMember, 0.4, None),
    ];
    let mut weights = PeopleRankWeights::default();
    weights
        .source_quotas
        .insert(PeopleCandidateSource::MutualFollow, 3);
    weights.cluster_cap = 2;
    weights.exploration_quota = 1;

    let ranked = rank_people_candidates(candidates, &weights).expect("rank candidates");

    assert_eq!(
        ranked
            .iter()
            .filter(|item| item.candidate_user_id == duplicate)
            .count(),
        1
    );
    assert_eq!(
        ranked
            .iter()
            .filter(|item| item.mutual_cluster == Some(cluster))
            .count(),
        2
    );
    assert!(ranked
        .iter()
        .any(|item| item.source == PeopleCandidateSource::NewMember));

    let base = ranked
        .iter()
        .find(|item| item.candidate_user_id == duplicate)
        .unwrap()
        .score;
    let mut penalized_candidate = candidate(
        viewer,
        duplicate,
        PeopleCandidateSource::MutualFollow,
        0.9,
        None,
    );
    penalized_candidate.features.impression_penalty = 1.0;
    penalized_candidate.features.dismissal_penalty = 1.0;
    let penalized = rank_people_candidates(vec![penalized_candidate], &weights)
        .unwrap()
        .remove(0)
        .score;
    assert!(penalized < base);
}

#[test]
fn profile_overlap_is_normalized_and_private_or_deleted_profiles_are_ineligible() {
    let viewer = ProfileDiscoverySnapshot {
        user_id: uuid(1),
        username: "rust_engineer".into(),
        nickname: "Rust Engineer".into(),
        bio: "distributed systems databases".into(),
        avatar_url: "viewer.png".into(),
        account_visibility: "PUBLIC".into(),
        lifecycle_status: "ACTIVE".into(),
    };
    let related = ProfileDiscoverySnapshot {
        user_id: uuid(2),
        username: "rust_backend".into(),
        nickname: "Backend Engineer".into(),
        bio: "rust distributed systems".into(),
        avatar_url: "candidate.png".into(),
        account_visibility: "PUBLIC".into(),
        lifecycle_status: "ACTIVE".into(),
    };
    let private = ProfileDiscoverySnapshot {
        account_visibility: "PRIVATE".into(),
        ..related.clone()
    };
    let deleted = ProfileDiscoverySnapshot {
        lifecycle_status: "DELETED".into(),
        ..related.clone()
    };

    assert!(viewer.overlap_score(&related) > 0.0);
    assert!(related.is_discoverable());
    assert!(!private.is_discoverable());
    assert!(!deleted.is_discoverable());
}

#[tokio::test]
async fn people_recommendation_use_case_reads_projection_filters_snapshots_and_records_impressions()
{
    let viewer = uuid(1);
    let allowed = uuid(2);
    let private = uuid(3);
    let blocked = uuid(4);
    let repository = Arc::new(FakePeopleRepository::default());
    repository.candidates.lock().unwrap().extend([
        candidate(
            viewer,
            allowed,
            PeopleCandidateSource::MutualFollow,
            0.9,
            Some(uuid(90)),
        ),
        candidate(
            viewer,
            private,
            PeopleCandidateSource::ProfileOverlap,
            0.8,
            None,
        ),
        candidate(
            viewer,
            blocked,
            PeopleCandidateSource::CoEngagement,
            0.7,
            None,
        ),
    ]);
    let profiles = Arc::new(FakeProfileDiscovery::default());
    profiles.values.lock().unwrap().extend([
        (viewer, profile(viewer, "PUBLIC", "ACTIVE")),
        (allowed, profile(allowed, "PUBLIC", "ACTIVE")),
        (private, profile(private, "PRIVATE", "ACTIVE")),
        (blocked, profile(blocked, "PUBLIC", "ACTIVE")),
    ]);
    let policies = Arc::new(FakeRelationshipPolicies::default());
    policies.blocked.lock().unwrap().insert(blocked);
    let metrics = Arc::new(RecommendationMetrics::default());
    let use_case = GetPeopleRecommendationsUseCase::new(
        repository.clone(),
        profiles,
        policies,
        test_ranker(),
        test_clock(),
        Arc::new(RecommendationMetricsObserver::new(metrics.clone())),
    );

    let page = use_case
        .execute(GetPeopleRecommendationsQuery {
            viewer_id: viewer,
            cursor: None,
            limit: 20,
            request_id: "request-1".into(),
            session_id: Some(uuid(100)),
        })
        .await
        .expect("recommendations");

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].candidate_user_id, allowed);
    assert!(repository.refresh_calls.lock().unwrap().is_empty());
    assert!(repository.enqueued_refreshes.lock().unwrap().is_empty());
    assert_eq!(repository.recorded_impressions.lock().unwrap().len(), 1);
    assert_eq!(page.items[0].profile.user_id, allowed);
    assert_eq!(page.items[0].reason_code, "MUTUAL_CONNECTIONS");
    assert!(page.items[0].impression_id != Uuid::nil());
    let snapshot = repository
        .snapshots
        .lock()
        .unwrap()
        .get(&page.snapshot_id)
        .cloned()
        .expect("snapshot should be persisted");
    assert_eq!(snapshot.created_at, test_now());
    assert_eq!(snapshot.expires_at, test_now() + Duration::minutes(30));
    let rendered = metrics.render_prometheus();
    assert!(rendered
        .contains("recommendation_requests_total{surface=\"people\",outcome=\"success\"} 1"));
    assert!(rendered.contains(
        "recommendation_candidates_total{surface=\"people\",source=\"mutual_graph\",outcome=\"success\"} 1"
    ));
    assert!(rendered.contains(
        "recommendation_candidates_total{surface=\"people\",source=\"follower_overlap\",outcome=\"success\"} 1"
    ));
    assert!(rendered.contains(
        "recommendation_candidates_total{surface=\"people\",source=\"social_proof\",outcome=\"success\"} 1"
    ));
    assert!(rendered.contains(
        "recommendation_stage_latency_milliseconds_count{surface=\"people\",stage=\"snapshot\"} 1"
    ));
}

#[tokio::test]
async fn people_recommendation_fails_closed_when_relationship_policy_is_missing() {
    let viewer = uuid(1);
    let candidate_id = uuid(2);
    let repository = Arc::new(FakePeopleRepository::default());
    repository.candidates.lock().unwrap().push(candidate(
        viewer,
        candidate_id,
        PeopleCandidateSource::MutualFollow,
        0.9,
        None,
    ));
    let profiles = Arc::new(FakeProfileDiscovery::default());
    profiles.values.lock().unwrap().extend([
        (viewer, profile(viewer, "PUBLIC", "ACTIVE")),
        (candidate_id, profile(candidate_id, "PUBLIC", "ACTIVE")),
    ]);
    let policies = Arc::new(FakeRelationshipPolicies::default());
    policies.missing.lock().unwrap().insert(candidate_id);
    let metrics = Arc::new(RecommendationMetrics::default());
    let use_case = GetPeopleRecommendationsUseCase::new(
        repository.clone(),
        profiles,
        policies,
        test_ranker(),
        test_clock(),
        Arc::new(RecommendationMetricsObserver::new(metrics.clone())),
    );

    let error = use_case
        .execute(GetPeopleRecommendationsQuery {
            viewer_id: viewer,
            cursor: None,
            limit: 20,
            request_id: "missing-relationship-policy".into(),
            session_id: None,
        })
        .await
        .expect_err("a candidate without authoritative relationship policy must be rejected");

    assert!(error.to_string().contains("relationship policy batch"));
    assert!(repository.recorded_impressions.lock().unwrap().is_empty());
    assert!(metrics
        .render_prometheus()
        .contains("recommendation_requests_total{surface=\"people\",outcome=\"failure\"} 1"));
}

#[tokio::test]
async fn empty_people_projection_enqueues_refresh_without_running_graph_expansion_inline() {
    let viewer = uuid(1);
    let repository = Arc::new(FakePeopleRepository::default());
    let profiles = Arc::new(FakeProfileDiscovery::default());
    profiles
        .values
        .lock()
        .unwrap()
        .insert(viewer, profile(viewer, "PUBLIC", "ACTIVE"));
    let use_case = GetPeopleRecommendationsUseCase::new(
        repository.clone(),
        profiles,
        Arc::new(FakeRelationshipPolicies::default()),
        test_ranker(),
        test_clock(),
        noop_observer(),
    );

    let page = use_case
        .execute(GetPeopleRecommendationsQuery {
            viewer_id: viewer,
            cursor: None,
            limit: 20,
            request_id: "cold-projection".into(),
            session_id: None,
        })
        .await
        .expect("empty projection response");

    assert!(page.items.is_empty());
    assert!(repository.refresh_calls.lock().unwrap().is_empty());
    assert_eq!(
        repository.enqueued_refreshes.lock().unwrap().as_slice(),
        &[viewer]
    );
}

#[tokio::test]
async fn learned_rollout_without_serving_adapter_uses_deterministic_fallback() {
    let viewer = uuid(1);
    let candidate_id = uuid(2);
    let repository = Arc::new(FakePeopleRepository::default());
    *repository.rollout.lock().unwrap() = Some(RecommendationRollout {
        mode: RolloutMode::Learned,
        deterministic_version: "people-linear-v1".into(),
        learned_version: Some("people-two-tower-v1".into()),
        canary_percent: 100,
        learned_ready: true,
    });
    repository.candidates.lock().unwrap().push(candidate(
        viewer,
        candidate_id,
        PeopleCandidateSource::MutualFollow,
        0.9,
        None,
    ));
    let profiles = Arc::new(FakeProfileDiscovery::default());
    profiles.values.lock().unwrap().extend([
        (viewer, profile(viewer, "PUBLIC", "ACTIVE")),
        (candidate_id, profile(candidate_id, "PUBLIC", "ACTIVE")),
    ]);
    let metrics = Arc::new(RecommendationMetrics::default());
    let use_case = GetPeopleRecommendationsUseCase::new(
        repository,
        profiles,
        Arc::new(FakeRelationshipPolicies::default()),
        test_ranker(),
        test_clock(),
        Arc::new(RecommendationMetricsObserver::new(metrics.clone())),
    );

    let page = use_case
        .execute(GetPeopleRecommendationsQuery {
            viewer_id: viewer,
            cursor: None,
            limit: 20,
            request_id: "learned-fallback".into(),
            session_id: None,
        })
        .await
        .expect("deterministic fallback");

    assert!(page.fallback_used);
    assert_eq!(page.rank_version, "people-linear-v1");
    assert!(page
        .items
        .iter()
        .all(|item| item.rank_version == "people-linear-v1"));
    let rendered = metrics.render_prometheus();
    assert!(rendered.contains(
        "recommendation_fallbacks_total{surface=\"people\",reason=\"model_unavailable\"} 1"
    ));
    assert!(rendered
        .contains("recommendation_requests_total{surface=\"people\",outcome=\"success\"} 1"));
}

#[tokio::test]
async fn people_recommendation_cursor_is_bound_to_viewer_and_snapshot() {
    let viewer = uuid(1);
    let other = uuid(2);
    let repository = Arc::new(FakePeopleRepository::default());
    let snapshot_id = uuid(999);
    repository.snapshots.lock().unwrap().insert(
        snapshot_id,
        PeopleSnapshot {
            id: snapshot_id,
            viewer_id: viewer,
            rank_version: "people-linear-v1".into(),
            created_at: test_now(),
            expires_at: test_now() + Duration::hours(1),
        },
    );
    let cursor = RecommendationCursor {
        version: 1,
        snapshot_id,
        score_micros: 500_000,
        occurred_at: test_now(),
        tie_breaker_id: uuid(10),
    }
    .encode();
    let use_case = GetPeopleRecommendationsUseCase::new(
        repository,
        Arc::new(FakeProfileDiscovery::default()),
        Arc::new(FakeRelationshipPolicies::default()),
        test_ranker(),
        test_clock(),
        noop_observer(),
    );

    let error = use_case
        .execute(GetPeopleRecommendationsQuery {
            viewer_id: other,
            cursor: Some(cursor),
            limit: 20,
            request_id: "request-2".into(),
            session_id: None,
        })
        .await
        .expect_err("foreign snapshot must fail");

    assert_eq!(error.to_string(), "Recommendation snapshot not found");
}

fn profile(user_id: Uuid, visibility: &str, lifecycle: &str) -> ProfileDiscoverySnapshot {
    ProfileDiscoverySnapshot {
        user_id,
        username: format!("user_{}", user_id.simple()),
        nickname: "User".into(),
        bio: "rust systems".into(),
        avatar_url: "avatar.png".into(),
        account_visibility: visibility.into(),
        lifecycle_status: lifecycle.into(),
    }
}

#[derive(Default)]
struct FakePeopleRepository {
    candidates: Mutex<Vec<PeopleCandidate>>,
    refresh_calls: Mutex<Vec<Uuid>>,
    enqueued_refreshes: Mutex<Vec<Uuid>>,
    snapshots: Mutex<HashMap<Uuid, PeopleSnapshot>>,
    snapshot_items: Mutex<HashMap<Uuid, Vec<PeopleSnapshotItem>>>,
    recorded_impressions: Mutex<Vec<(Uuid, Uuid, Uuid)>>,
    claimed_refreshes: Mutex<Vec<Uuid>>,
    refresh_failures: Mutex<HashSet<Uuid>>,
    completed_refreshes: Mutex<Vec<Uuid>>,
    failed_refreshes: Mutex<Vec<(Uuid, chrono::DateTime<Utc>)>>,
    rollout: Mutex<Option<RecommendationRollout>>,
    recent_actions: Mutex<Vec<RecommendationEvent>>,
}

#[async_trait]
impl PeopleRecommendationRepositoryPort for FakePeopleRepository {
    async fn refresh_candidates(
        &self,
        viewer_id: Uuid,
        _generated_at: chrono::DateTime<Utc>,
        _expires_at: chrono::DateTime<Utc>,
        _model_version: &str,
        _max_candidates: i32,
    ) -> Result<i64> {
        self.refresh_calls.lock().unwrap().push(viewer_id);
        if self.refresh_failures.lock().unwrap().contains(&viewer_id) {
            anyhow::bail!("candidate refresh failed");
        }
        Ok(self.candidates.lock().unwrap().len() as i64)
    }

    async fn list_candidates(
        &self,
        viewer_id: Uuid,
        _now: chrono::DateTime<Utc>,
        _limit: i32,
    ) -> Result<Vec<PeopleCandidate>> {
        Ok(self
            .candidates
            .lock()
            .unwrap()
            .iter()
            .filter(|candidate| candidate.viewer_id == viewer_id)
            .cloned()
            .collect())
    }

    async fn ineligible_candidate_ids(
        &self,
        _viewer_id: Uuid,
        _candidate_ids: &[Uuid],
    ) -> Result<HashSet<Uuid>> {
        Ok(HashSet::new())
    }

    async fn create_snapshot(
        &self,
        snapshot: &PeopleSnapshot,
        items: &[PeopleSnapshotItem],
    ) -> Result<()> {
        self.snapshots
            .lock()
            .unwrap()
            .insert(snapshot.id, snapshot.clone());
        self.snapshot_items
            .lock()
            .unwrap()
            .insert(snapshot.id, items.to_vec());
        Ok(())
    }

    async fn find_snapshot(
        &self,
        viewer_id: Uuid,
        snapshot_id: Uuid,
        now: chrono::DateTime<Utc>,
    ) -> Result<Option<PeopleSnapshot>> {
        Ok(self
            .snapshots
            .lock()
            .unwrap()
            .get(&snapshot_id)
            .filter(|snapshot| snapshot.viewer_id == viewer_id && snapshot.expires_at > now)
            .cloned())
    }

    async fn load_snapshot_page(
        &self,
        snapshot_id: Uuid,
        cursor: Option<&RecommendationCursor>,
        limit: i32,
    ) -> Result<Vec<PeopleSnapshotItem>> {
        let mut values = self
            .snapshot_items
            .lock()
            .unwrap()
            .get(&snapshot_id)
            .cloned()
            .unwrap_or_default();
        if let Some(cursor) = cursor {
            values.retain(|item| {
                (item.score_micros, item.candidate_user_id)
                    < (cursor.score_micros, cursor.tie_breaker_id)
            });
        }
        values.truncate(limit as usize + 1);
        Ok(values)
    }

    async fn record_impressions(
        &self,
        viewer_id: Uuid,
        snapshot_id: Uuid,
        impressions: &[friend_service::domain::recommendation::PeopleRecommendationImpression],
    ) -> Result<()> {
        self.recorded_impressions.lock().unwrap().extend(
            impressions
                .iter()
                .map(|impression| (viewer_id, snapshot_id, impression.impression_id)),
        );
        Ok(())
    }

    async fn enqueue_refresh(
        &self,
        viewer_ids: &[Uuid],
        _now: chrono::DateTime<Utc>,
    ) -> Result<()> {
        self.enqueued_refreshes
            .lock()
            .unwrap()
            .extend_from_slice(viewer_ids);
        Ok(())
    }

    async fn claim_refresh_batch(
        &self,
        now: chrono::DateTime<Utc>,
        _lease_timeout: Duration,
        _limit: i32,
    ) -> Result<Vec<RecommendationRefreshClaim>> {
        Ok(std::mem::take(&mut *self.claimed_refreshes.lock().unwrap())
            .into_iter()
            .map(|viewer_id| RecommendationRefreshClaim {
                viewer_id,
                claimed_at: now,
            })
            .collect())
    }

    async fn complete_refresh(&self, claim: RecommendationRefreshClaim) -> Result<()> {
        self.completed_refreshes
            .lock()
            .unwrap()
            .push(claim.viewer_id);
        Ok(())
    }

    async fn fail_refresh(
        &self,
        claim: RecommendationRefreshClaim,
        _error: &str,
        next_attempt_at: chrono::DateTime<Utc>,
        _max_attempts: i32,
    ) -> Result<()> {
        self.failed_refreshes
            .lock()
            .unwrap()
            .push((claim.viewer_id, next_attempt_at));
        Ok(())
    }

    async fn rollout(&self, _surface: &str) -> Result<RecommendationRollout> {
        Ok(self
            .rollout
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| RecommendationRollout::deterministic("people-linear-v1")))
    }

    async fn recent_actions(
        &self,
        viewer_id: Uuid,
        limit: RecentActionLimit,
    ) -> Result<Vec<RecommendationEvent>> {
        Ok(self
            .recent_actions
            .lock()
            .unwrap()
            .iter()
            .filter(|event| event.viewer_id == viewer_id)
            .take(limit.get() as usize)
            .cloned()
            .collect())
    }
}

#[derive(Default)]
struct FakeProfileDiscovery {
    values: Mutex<HashMap<Uuid, ProfileDiscoverySnapshot>>,
}

#[async_trait]
impl ProfileDiscoveryPort for FakeProfileDiscovery {
    async fn batch_profiles(&self, user_ids: &[Uuid]) -> Result<Vec<ProfileDiscoverySnapshot>> {
        let values = self.values.lock().unwrap();
        Ok(user_ids
            .iter()
            .filter_map(|user_id| values.get(user_id).cloned())
            .collect())
    }
}

#[derive(Default)]
struct FakeRelationshipPolicies {
    blocked: Mutex<HashSet<Uuid>>,
    missing: Mutex<HashSet<Uuid>>,
}

#[async_trait]
impl RelationshipPolicyRepositoryPort for FakeRelationshipPolicies {
    async fn block(
        &self,
        _blocker_id: Uuid,
        _blocked_id: Uuid,
        _created_at: chrono::DateTime<Utc>,
    ) -> Result<RelationshipPolicy> {
        unreachable!()
    }

    async fn unblock(&self, _blocker_id: Uuid, _blocked_id: Uuid) -> Result<RelationshipPolicy> {
        unreachable!()
    }

    async fn mute(
        &self,
        _muter_id: Uuid,
        _muted_id: Uuid,
        _created_at: chrono::DateTime<Utc>,
    ) -> Result<RelationshipPolicy> {
        unreachable!()
    }

    async fn unmute(&self, _muter_id: Uuid, _muted_id: Uuid) -> Result<RelationshipPolicy> {
        unreachable!()
    }

    async fn policy(&self, _viewer_id: Uuid, _target_id: Uuid) -> Result<RelationshipPolicy> {
        Ok(RelationshipPolicy::default())
    }

    async fn batch_policy(
        &self,
        _viewer_id: Uuid,
        target_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, RelationshipPolicy>> {
        let blocked = self.blocked.lock().unwrap();
        let missing = self.missing.lock().unwrap();
        Ok(target_ids
            .iter()
            .filter(|target_id| !missing.contains(target_id))
            .map(|target_id| {
                (
                    *target_id,
                    RelationshipPolicy {
                        blocked_by_viewer: blocked.contains(target_id),
                        ..RelationshipPolicy::default()
                    },
                )
            })
            .collect())
    }
}

#[tokio::test]
async fn recommendation_refresh_worker_completes_successes_and_retries_only_failures() {
    use friend_service::adapters::r#in::messaging::recommendation_refresh_worker::RecommendationRefreshWorker;

    let success = uuid(700);
    let failure = uuid(701);
    let repository = Arc::new(FakePeopleRepository::default());
    repository
        .claimed_refreshes
        .lock()
        .unwrap()
        .extend([success, failure]);
    repository.refresh_failures.lock().unwrap().insert(failure);
    let metrics = Arc::new(RecommendationMetrics::default());
    let worker = RecommendationRefreshWorker::new(
        repository.clone(),
        "people-linear-v1".into(),
        Duration::minutes(5),
        100,
        5,
        Duration::seconds(1),
    )
    .with_metrics(metrics.clone());

    let result = worker
        .tick(Utc.with_ymd_and_hms(2026, 8, 4, 2, 0, 0).unwrap())
        .await
        .expect("worker tick");

    assert_eq!(result.claimed, 2);
    assert_eq!(result.succeeded, 1);
    assert_eq!(result.failed, 1);
    assert_eq!(
        repository.completed_refreshes.lock().unwrap().as_slice(),
        &[success]
    );
    let failures = repository.failed_refreshes.lock().unwrap();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].0, failure);
    assert!(failures[0].1 > Utc.with_ymd_and_hms(2026, 8, 4, 2, 0, 0).unwrap());
    let rendered = metrics.render_prometheus();
    assert!(rendered
        .contains("recommendation_refresh_runs_total{surface=\"people\",outcome=\"success\"} 1"));
    assert!(rendered.contains(
        "recommendation_refresh_viewers_total{surface=\"people\",outcome=\"success\"} 1"
    ));
    assert!(rendered.contains(
        "recommendation_refresh_viewers_total{surface=\"people\",outcome=\"failure\"} 1"
    ));
}

#[tokio::test]
async fn recent_negative_feedback_penalizes_people_candidates_before_snapshot_ranking() {
    let viewer = uuid(1);
    let hidden = uuid(2);
    let clean = uuid(3);
    let repository = Arc::new(FakePeopleRepository::default());
    repository.candidates.lock().unwrap().extend([
        candidate(
            viewer,
            hidden,
            PeopleCandidateSource::MutualFollow,
            0.95,
            None,
        ),
        candidate(
            viewer,
            clean,
            PeopleCandidateSource::MutualFollow,
            0.80,
            None,
        ),
    ]);
    repository
        .recent_actions
        .lock()
        .unwrap()
        .push(recommendation_event(
            90_001,
            viewer,
            hidden,
            RecommendationAction::Hide,
            test_now() - Duration::minutes(1),
        ));
    let profiles = Arc::new(FakeProfileDiscovery::default());
    profiles.values.lock().unwrap().extend([
        (viewer, profile(viewer, "PUBLIC", "ACTIVE")),
        (hidden, profile(hidden, "PUBLIC", "ACTIVE")),
        (clean, profile(clean, "PUBLIC", "ACTIVE")),
    ]);
    let use_case = GetPeopleRecommendationsUseCase::new(
        repository,
        profiles,
        Arc::new(FakeRelationshipPolicies::default()),
        test_ranker(),
        test_clock(),
        noop_observer(),
    )
    .with_sequence_config(PeopleSequenceConfig {
        enabled: true,
        limit: 20,
    });

    let page = use_case
        .execute(GetPeopleRecommendationsQuery {
            viewer_id: viewer,
            cursor: None,
            limit: 20,
            request_id: "sequence-penalty".into(),
            session_id: None,
        })
        .await
        .expect("recommendations");

    assert_eq!(
        page.items.first().map(|item| item.candidate_user_id),
        Some(clean)
    );
    assert!(page
        .items
        .iter()
        .any(|item| item.candidate_user_id == hidden));
}
