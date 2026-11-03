use super::support::{instant, post, uuid, Harness};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use platform::recommendation::{RecentActionLimit, RolloutMode};
use platform::recommendation_metrics::RecommendationMetrics;
use post_service::adapters::out::recommendation::feed_candidate_pipeline::{
    DeterministicFeedReranker, LinearFeedRanker,
};
use post_service::application::ports::{
    FeedCandidateCollectorPort, FeedRecommendationRepositoryPort, RecommendationRollout,
};
use post_service::application::use_cases::get_ranked_feed::{RankedFeedConfig, RankedFeedService};
use post_service::domain::post::RelationshipPolicySnapshot;
use post_service::domain::recommendation::{
    FeedCandidate, FeedCandidateCollection, FeedCandidateSource, FeedRankWeights,
    FeedRecommendationImpression, FeedSnapshot, FeedSnapshotCursor, FeedSnapshotPage,
    RecommendationEvent,
};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone)]
struct StubCollector {
    result: Arc<Mutex<Option<Result<FeedCandidateCollection>>>>,
    following_counts: Arc<Mutex<Vec<usize>>>,
}

impl StubCollector {
    fn successful(
        candidates: Vec<FeedCandidate>,
        failed_sources: Vec<FeedCandidateSource>,
    ) -> Self {
        Self {
            result: Arc::new(Mutex::new(Some(Ok(FeedCandidateCollection {
                candidates,
                failed_sources,
            })))),
            following_counts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn failing(message: &str) -> Self {
        Self {
            result: Arc::new(Mutex::new(Some(Err(anyhow::anyhow!(message.to_owned()))))),
            following_counts: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl FeedCandidateCollectorPort for StubCollector {
    async fn collect(
        &self,
        _: Uuid,
        following_ids: &[Uuid],
        _: i32,
        _: DateTime<Utc>,
    ) -> Result<FeedCandidateCollection> {
        self.following_counts
            .lock()
            .unwrap()
            .push(following_ids.len());
        self.result.lock().unwrap().take().unwrap_or_else(|| {
            Ok(FeedCandidateCollection {
                candidates: Vec::new(),
                failed_sources: Vec::new(),
            })
        })
    }
}

#[derive(Default)]
struct MemoryFeedRepository {
    snapshot: Mutex<Option<FeedSnapshot>>,
    impressions: Mutex<Vec<FeedRecommendationImpression>>,
    events: Mutex<Vec<RecommendationEvent>>,
    ready: Mutex<bool>,
    rollout: Mutex<Option<RecommendationRollout>>,
    recent_actions: Mutex<Vec<RecommendationEvent>>,
    recent_actions_error: Mutex<Option<String>>,
    recent_action_limits: Mutex<Vec<u16>>,
}

impl MemoryFeedRepository {
    fn ready() -> Self {
        Self {
            ready: Mutex::new(true),
            ..Self::default()
        }
    }
}

#[async_trait]
impl FeedRecommendationRepositoryPort for MemoryFeedRepository {
    async fn create_snapshot(&self, snapshot: &FeedSnapshot) -> Result<()> {
        *self.snapshot.lock().unwrap() = Some(snapshot.clone());
        Ok(())
    }

    async fn load_snapshot_page(
        &self,
        viewer_id: Uuid,
        snapshot_id: Uuid,
        offset: u32,
        limit: i32,
        now: DateTime<Utc>,
    ) -> Result<Option<FeedSnapshotPage>> {
        let snapshot = self.snapshot.lock().unwrap().clone();
        let Some(snapshot) = snapshot else {
            return Ok(None);
        };
        if snapshot.viewer_id != viewer_id
            || snapshot.snapshot_id != snapshot_id
            || snapshot.expires_at <= now
        {
            return Ok(None);
        }
        let limit = limit.clamp(1, 100) as usize;
        let start = offset as usize;
        let end = (start + limit).min(snapshot.items.len());
        let items = snapshot.items[start.min(snapshot.items.len())..end].to_vec();
        let next_cursor = (end < snapshot.items.len()).then(|| {
            FeedSnapshotCursor {
                snapshot_id,
                offset: end as u32,
            }
            .encode()
        });
        Ok(Some(FeedSnapshotPage {
            snapshot_id,
            request_id: snapshot.request_id,
            session_id: snapshot.session_id,
            rank_version: snapshot.rank_version,
            fallback_used: snapshot.fallback_used,
            failed_sources: snapshot.failed_sources,
            items,
            next_cursor,
        }))
    }

    async fn record_impressions(&self, impressions: &[FeedRecommendationImpression]) -> Result<()> {
        self.impressions
            .lock()
            .unwrap()
            .extend_from_slice(impressions);
        self.events
            .lock()
            .unwrap()
            .extend(impressions.iter().map(|impression| {
                RecommendationEvent::new(
                    impression.impression_id,
                    platform::recommendation::RECOMMENDATION_SCHEMA_VERSION,
                    impression.occurred_at,
                    impression.viewer_id,
                    post_service::domain::recommendation::RecommendationSubjectType::Post,
                    impression.post_id,
                    Some(impression.viewer_id),
                    post_service::domain::recommendation::RecommendationSurface::HomeFeed,
                    Some(impression.position as i32),
                    Some(impression.request_id.clone()),
                    impression.session_id,
                    post_service::domain::recommendation::RecommendationAction::Impression,
                    impression.source.as_str(),
                    impression.rank_version.clone(),
                    "{}",
                    Some(f64::from(impression.propensity_micros) / 1_000_000.0),
                )
                .expect("valid impression event")
            }));
        Ok(())
    }

    async fn append_event(&self, event: &RecommendationEvent) -> Result<bool> {
        self.events.lock().unwrap().push(event.clone());
        Ok(true)
    }

    async fn latest_impression(
        &self,
        viewer_id: Uuid,
        post_id: Uuid,
    ) -> Result<Option<FeedRecommendationImpression>> {
        Ok(self
            .impressions
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|value| value.viewer_id == viewer_id && value.post_id == post_id)
            .cloned())
    }

    async fn cleanup_user(&self, _: Uuid) -> Result<()> {
        Ok(())
    }

    async fn model_ready(&self, _: &str) -> Result<bool> {
        Ok(*self.ready.lock().unwrap())
    }

    async fn rollout(&self, _: &str) -> Result<RecommendationRollout> {
        Ok(self
            .rollout
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| RecommendationRollout::deterministic("feed-linear-v1")))
    }

    async fn recent_actions(
        &self,
        _: Uuid,
        limit: RecentActionLimit,
    ) -> Result<Vec<RecommendationEvent>> {
        self.recent_action_limits.lock().unwrap().push(limit.get());
        if let Some(message) = self.recent_actions_error.lock().unwrap().clone() {
            return Err(anyhow::anyhow!(message));
        }
        Ok(self.recent_actions.lock().unwrap().clone())
    }
}

fn candidate(
    post_id: Uuid,
    author_id: Uuid,
    source: FeedCandidateSource,
    score: f64,
) -> FeedCandidate {
    FeedCandidate {
        post_id,
        author_id,
        source,
        source_score: score,
        author_affinity: 0.5,
        freshness: 0.9,
        social_proof: 0.2,
        engagement_velocity: 0.3,
        topic_match: 0.0,
        repeated_impression_penalty: 0.0,
        author_saturation_penalty: 0.0,
        negative_feedback_penalty: 0.0,
        generated_at: instant(1_700_000_000),
        reason_code: source.as_str().to_owned(),
        in_network: source != FeedCandidateSource::Exploration,
        exploration: source == FeedCandidateSource::Exploration,
    }
}

fn ranked_service(
    harness: &Harness,
    collector: StubCollector,
    repository: Arc<MemoryFeedRepository>,
) -> RankedFeedService {
    RankedFeedService::new(
        harness.service.clone(),
        Arc::new(collector),
        Arc::new(LinearFeedRanker::new(FeedRankWeights::default())),
        Arc::new(DeterministicFeedReranker::new(2, 1, 1)),
        repository,
        RankedFeedConfig {
            per_source_limit: 10,
            snapshot_candidate_limit: 10,
            snapshot_ttl_seconds: 900,
            following_author_limit: 5_000,
            recent_sequence_enabled: false,
            recent_sequence_limit: 20,
        },
    )
}

#[tokio::test]
async fn ranked_feed_returns_snapshot_metadata_and_records_only_delivered_impressions() {
    let harness = Harness::new();
    let viewer_id = uuid("11111111-1111-1111-1111-111111111111");
    let author_a = uuid("22222222-2222-2222-2222-222222222222");
    let author_b = uuid("33333333-3333-3333-3333-333333333333");
    let post_a = uuid("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    let post_b = uuid("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
    harness.posts.posts_by_id.lock().unwrap().extend([
        (post_a, post(post_a, author_a, "a")),
        (post_b, post(post_b, author_b, "b")),
    ]);
    *harness.friend.following_ids.lock().unwrap() = vec![author_a, author_b];
    let repository = Arc::new(MemoryFeedRepository::ready());
    let metrics = Arc::new(RecommendationMetrics::default());
    let service = ranked_service(
        &harness,
        StubCollector::successful(
            vec![
                candidate(
                    post_a,
                    author_a,
                    FeedCandidateSource::MaterializedTimeline,
                    0.9,
                ),
                candidate(post_b, author_b, FeedCandidateSource::Exploration, 0.8),
            ],
            vec![FeedCandidateSource::SocialProof],
        ),
        repository.clone(),
    )
    .with_metrics(metrics.clone());
    let session_id = uuid("44444444-4444-4444-4444-444444444444");

    let response = service
        .execute(viewer_id, None, 10, Some("feed-request"), Some(session_id))
        .await
        .expect("ranked feed");

    assert_eq!(response.posts.len(), 2);
    assert_eq!(response.request_id.as_deref(), Some("feed-request"));
    assert!(response.snapshot_id.is_some());
    assert_eq!(response.rank_version.as_deref(), Some("feed-linear-v1"));
    assert!(!response.fallback_used);
    assert_eq!(response.failed_sources, vec!["SOCIAL_PROOF"]);
    assert_eq!(response.items.len(), 2);
    assert_eq!(response.items[0].position, 0);
    assert!(!response.items[0].impression_id.is_empty());
    assert!(response.items.iter().any(|item| item.exploration));
    assert_eq!(repository.impressions.lock().unwrap().len(), 2);
    assert_eq!(repository.events.lock().unwrap().len(), 2);
    let rendered = metrics.render_prometheus();
    assert!(rendered.contains(
        "recommendation_source_runs_total{surface=\"home_feed\",source=\"timeline\",outcome=\"success\"} 1"
    ));
    assert!(rendered.contains(
        "recommendation_source_runs_total{surface=\"home_feed\",source=\"exploration\",outcome=\"success\"} 1"
    ));
    assert!(rendered.contains(
        "recommendation_source_runs_total{surface=\"home_feed\",source=\"social_proof\",outcome=\"failure\"} 1"
    ));
    assert!(rendered.contains(
        "recommendation_fallbacks_total{surface=\"home_feed\",reason=\"candidate_source_failed\"} 1"
    ));
}

#[tokio::test]
async fn ranked_feed_rejects_invalid_opaque_cursor_instead_of_reinterpreting_it() {
    let harness = Harness::new();
    let repository = Arc::new(MemoryFeedRepository::ready());
    let service = ranked_service(
        &harness,
        StubCollector::successful(Vec::new(), Vec::new()),
        repository,
    );

    let error = service
        .execute(
            uuid("11111111-1111-1111-1111-111111111111"),
            Some("malformed-ranked-cursor"),
            10,
            Some("request"),
            None,
        )
        .await
        .expect_err("invalid ranked cursor must fail");

    assert_eq!(error.to_string(), "Invalid feed cursor");
}

#[tokio::test]
async fn unavailable_model_uses_explicit_chronological_fallback_metadata() {
    let harness = Harness::new();
    let viewer_id = uuid("11111111-1111-1111-1111-111111111111");
    let fallback_post = uuid("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    *harness.posts.feed_page.lock().unwrap() = Some(post_service::domain::feed::FeedPage {
        posts: vec![post(fallback_post, viewer_id, "fallback")],
        next_cursor: None,
    });
    let repository = Arc::new(MemoryFeedRepository::default());
    let metrics = Arc::new(RecommendationMetrics::default());
    let service = ranked_service(
        &harness,
        StubCollector::failing("must not be called"),
        repository,
    )
    .with_metrics(metrics.clone());

    let response = service
        .execute(viewer_id, None, 10, Some("fallback-request"), None)
        .await
        .expect("fallback feed");

    assert_eq!(response.posts.len(), 1);
    assert!(response.fallback_used);
    assert_eq!(response.request_id.as_deref(), Some("fallback-request"));
    assert_eq!(response.rank_version.as_deref(), Some("chronological-v1"));
    assert!(response.snapshot_id.is_none());
    assert!(response.items.is_empty());
    assert!(metrics.render_prometheus().contains(
        "recommendation_fallbacks_total{surface=\"home_feed\",reason=\"model_unavailable\"} 1"
    ));
}

#[tokio::test]
async fn blocked_candidate_is_removed_before_snapshot_and_impression_logging() {
    let harness = Harness::new();
    let viewer_id = uuid("11111111-1111-1111-1111-111111111111");
    let blocked_author = uuid("22222222-2222-2222-2222-222222222222");
    let allowed_author = uuid("33333333-3333-3333-3333-333333333333");
    let blocked_post = uuid("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    let allowed_post = uuid("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
    harness.posts.posts_by_id.lock().unwrap().extend([
        (blocked_post, post(blocked_post, blocked_author, "blocked")),
        (allowed_post, post(allowed_post, allowed_author, "allowed")),
    ]);
    harness.friend.policies.lock().unwrap().insert(
        (viewer_id, blocked_author),
        RelationshipPolicySnapshot {
            blocked_by_viewer: true,
            ..RelationshipPolicySnapshot::default()
        },
    );
    let repository = Arc::new(MemoryFeedRepository::ready());
    let service = ranked_service(
        &harness,
        StubCollector::successful(
            vec![
                candidate(
                    blocked_post,
                    blocked_author,
                    FeedCandidateSource::MaterializedTimeline,
                    1.0,
                ),
                candidate(
                    allowed_post,
                    allowed_author,
                    FeedCandidateSource::FreshTrending,
                    0.8,
                ),
            ],
            Vec::new(),
        ),
        repository.clone(),
    );

    let response = service
        .execute(viewer_id, None, 10, Some("policy-request"), None)
        .await
        .expect("policy filtered feed");

    assert_eq!(response.posts.len(), 1);
    assert_eq!(response.posts[0].id, allowed_post.to_string());
    assert_eq!(repository.impressions.lock().unwrap().len(), 1);
    assert_eq!(
        repository.impressions.lock().unwrap()[0].post_id,
        allowed_post
    );
}

#[tokio::test]
async fn all_policy_filtered_candidates_return_an_empty_ranked_snapshot_without_chronological_fallback(
) {
    let harness = Harness::new();
    let viewer_id = uuid("11111111-1111-1111-1111-111111111111");
    let blocked_author = uuid("22222222-2222-2222-2222-222222222222");
    let blocked_post = uuid("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    let chronological_only_post = uuid("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
    harness
        .posts
        .posts_by_id
        .lock()
        .unwrap()
        .insert(blocked_post, post(blocked_post, blocked_author, "blocked"));
    *harness.posts.feed_page.lock().unwrap() = Some(post_service::domain::feed::FeedPage {
        posts: vec![post(
            chronological_only_post,
            viewer_id,
            "must-not-switch-feed-path",
        )],
        next_cursor: None,
    });
    *harness.friend.following_ids.lock().unwrap() = vec![blocked_author];
    harness.friend.policies.lock().unwrap().insert(
        (viewer_id, blocked_author),
        RelationshipPolicySnapshot {
            blocked_by_viewer: true,
            ..RelationshipPolicySnapshot::default()
        },
    );
    let repository = Arc::new(MemoryFeedRepository::ready());
    let service = ranked_service(
        &harness,
        StubCollector::successful(
            vec![candidate(
                blocked_post,
                blocked_author,
                FeedCandidateSource::MaterializedTimeline,
                1.0,
            )],
            Vec::new(),
        ),
        repository.clone(),
    );

    let response = service
        .execute(viewer_id, None, 10, Some("all-blocked-request"), None)
        .await
        .expect("an all-blocked ranked feed should be a successful empty page");

    assert!(response.posts.is_empty());
    assert_eq!(response.request_id.as_deref(), Some("all-blocked-request"));
    assert!(response.snapshot_id.is_some());
    assert_eq!(response.rank_version.as_deref(), Some("feed-linear-v1"));
    assert!(!response.fallback_used);
    assert!(response.items.is_empty());
    let snapshot = repository
        .snapshot
        .lock()
        .unwrap()
        .clone()
        .expect("empty ranked snapshot should be persisted");
    assert!(snapshot.items.is_empty());
    assert!(repository.impressions.lock().unwrap().is_empty());
    assert!(!response
        .posts
        .iter()
        .any(|post| post.id == chronological_only_post.to_string()));
}

#[tokio::test]
async fn ranked_feed_fails_closed_when_relationship_batch_omits_candidate_author() {
    let harness = Harness::new();
    let viewer_id = uuid("11111111-1111-1111-1111-111111111111");
    let author_id = uuid("22222222-2222-2222-2222-222222222222");
    let post_id = uuid("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    harness
        .posts
        .posts_by_id
        .lock()
        .unwrap()
        .insert(post_id, post(post_id, author_id, "must not be served"));
    *harness.friend.omit_batch_policy_for.lock().unwrap() = Some(author_id);
    let repository = Arc::new(MemoryFeedRepository::ready());
    let service = ranked_service(
        &harness,
        StubCollector::successful(
            vec![candidate(
                post_id,
                author_id,
                FeedCandidateSource::MaterializedTimeline,
                0.9,
            )],
            Vec::new(),
        ),
        repository.clone(),
    );

    let error = service
        .execute(viewer_id, None, 10, Some("missing-policy"), None)
        .await
        .expect_err("a candidate without authoritative relationship policy must be rejected");

    assert!(error
        .to_string()
        .contains("omitted a requested relationship policy"));
    assert!(repository.snapshot.lock().unwrap().is_none());
    assert!(repository.impressions.lock().unwrap().is_empty());
}

#[tokio::test]
async fn ranked_feed_bounds_following_authors_before_candidate_collection() {
    let harness = Harness::new();
    *harness.friend.following_ids.lock().unwrap() = (0..250)
        .map(|index| Uuid::from_u128(10_000 + index))
        .collect();
    let collector = StubCollector::successful(Vec::new(), Vec::new());
    let observed = collector.following_counts.clone();
    let repository = Arc::new(MemoryFeedRepository::ready());
    let service = RankedFeedService::new(
        harness.service.clone(),
        Arc::new(collector),
        Arc::new(LinearFeedRanker::new(FeedRankWeights::default())),
        Arc::new(DeterministicFeedReranker::new(2, 1, 1)),
        repository,
        RankedFeedConfig {
            per_source_limit: 10,
            snapshot_candidate_limit: 10,
            snapshot_ttl_seconds: 900,
            following_author_limit: 100,
            recent_sequence_enabled: false,
            recent_sequence_limit: 20,
        },
    );

    let response = service
        .execute(
            uuid("11111111-1111-1111-1111-111111111111"),
            None,
            10,
            Some("bounded-request"),
            None,
        )
        .await
        .expect("bounded fallback");

    assert!(response.fallback_used);
    assert_eq!(*observed.lock().unwrap(), vec![100]);
}

#[tokio::test]
async fn learned_rollout_without_adapter_keeps_ranked_feed_and_marks_deterministic_fallback() {
    let harness = Harness::new();
    let viewer_id = uuid("11111111-1111-1111-1111-111111111111");
    let author_id = uuid("22222222-2222-2222-2222-222222222222");
    let post_id = uuid("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    harness
        .posts
        .posts_by_id
        .lock()
        .unwrap()
        .insert(post_id, post(post_id, author_id, "ranked fallback"));
    *harness.friend.following_ids.lock().unwrap() = vec![author_id];
    let repository = Arc::new(MemoryFeedRepository::ready());
    *repository.rollout.lock().unwrap() = Some(RecommendationRollout {
        mode: RolloutMode::Learned,
        deterministic_version: "feed-linear-v1".into(),
        learned_version: Some("feed-two-tower-v1".into()),
        canary_percent: 100,
        learned_ready: true,
    });
    let metrics = Arc::new(RecommendationMetrics::default());
    let service = ranked_service(
        &harness,
        StubCollector::successful(
            vec![candidate(
                post_id,
                author_id,
                FeedCandidateSource::MaterializedTimeline,
                0.9,
            )],
            Vec::new(),
        ),
        repository.clone(),
    )
    .with_metrics(metrics.clone());

    let first = service
        .execute(viewer_id, None, 10, Some("learned-fallback"), None)
        .await
        .expect("deterministic ranked fallback");

    assert_eq!(first.posts.len(), 1);
    assert_eq!(first.rank_version.as_deref(), Some("feed-linear-v1"));
    assert!(first.fallback_used);
    let cursor = first.next_cursor.clone();
    assert!(repository
        .snapshot
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|snapshot| snapshot.fallback_used));

    let rendered = metrics.render_prometheus();
    assert!(rendered.contains(
        "recommendation_fallbacks_total{surface=\"home_feed\",reason=\"model_unavailable\"} 1"
    ));
    assert!(rendered
        .contains("recommendation_requests_total{surface=\"home_feed\",outcome=\"success\"} 1"));

    if let Some(cursor) = cursor {
        let second = service
            .execute(viewer_id, Some(&cursor), 10, Some("ignored"), None)
            .await
            .expect("snapshot page retains fallback metadata");
        assert!(second.fallback_used);
        assert_eq!(second.rank_version.as_deref(), Some("feed-linear-v1"));
    }
}

#[tokio::test]
async fn recent_sequence_penalizes_recent_negative_feedback_before_ranking() {
    let harness = Harness::new();
    let viewer_id = uuid("11111111-1111-1111-1111-111111111111");
    let author_a = uuid("22222222-2222-2222-2222-222222222222");
    let author_b = uuid("33333333-3333-3333-3333-333333333333");
    let hidden_post = uuid("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    let clean_post = uuid("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
    harness.posts.posts_by_id.lock().unwrap().extend([
        (hidden_post, post(hidden_post, author_a, "recently hidden")),
        (clean_post, post(clean_post, author_b, "clean candidate")),
    ]);
    *harness.friend.following_ids.lock().unwrap() = vec![author_a, author_b];
    let repository = Arc::new(MemoryFeedRepository::ready());
    repository.recent_actions.lock().unwrap().push(
        RecommendationEvent::new(
            uuid("99999999-9999-9999-9999-999999999999"),
            platform::recommendation::RECOMMENDATION_SCHEMA_VERSION,
            Utc::now() - Duration::minutes(1),
            viewer_id,
            post_service::domain::recommendation::RecommendationSubjectType::Post,
            hidden_post,
            Some(viewer_id),
            post_service::domain::recommendation::RecommendationSurface::HomeFeed,
            None,
            Some("negative-sequence".into()),
            None,
            post_service::domain::recommendation::RecommendationAction::Hide,
            "MATERIALIZED_TIMELINE",
            "feed-linear-v1",
            "{}",
            None,
        )
        .expect("valid recent hide"),
    );
    let service = RankedFeedService::new(
        harness.service.clone(),
        Arc::new(StubCollector::successful(
            vec![
                candidate(
                    hidden_post,
                    author_a,
                    FeedCandidateSource::MaterializedTimeline,
                    1.0,
                ),
                candidate(
                    clean_post,
                    author_b,
                    FeedCandidateSource::MaterializedTimeline,
                    0.9,
                ),
            ],
            Vec::new(),
        )),
        Arc::new(LinearFeedRanker::new(FeedRankWeights::default())),
        Arc::new(DeterministicFeedReranker::new(2, 1, 1)),
        repository.clone(),
        RankedFeedConfig {
            recent_sequence_enabled: true,
            recent_sequence_limit: 20,
            ..RankedFeedConfig::default()
        },
    );

    let response = service
        .execute(viewer_id, None, 10, Some("negative-sequence"), None)
        .await
        .expect("ranked feed with sequence features");

    assert_eq!(response.posts[0].id, clean_post.to_string());
    assert_eq!(*repository.recent_action_limits.lock().unwrap(), vec![20]);
    assert!(!response.fallback_used);
}

#[tokio::test]
async fn recent_sequence_lookup_failure_keeps_ranked_feed_and_marks_feature_fallback() {
    let harness = Harness::new();
    let viewer_id = uuid("11111111-1111-1111-1111-111111111111");
    let author_id = uuid("22222222-2222-2222-2222-222222222222");
    let post_id = uuid("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    harness
        .posts
        .posts_by_id
        .lock()
        .unwrap()
        .insert(post_id, post(post_id, author_id, "sequence fallback"));
    *harness.friend.following_ids.lock().unwrap() = vec![author_id];
    let repository = Arc::new(MemoryFeedRepository::ready());
    *repository.recent_actions_error.lock().unwrap() = Some("sequence unavailable".into());
    let service = RankedFeedService::new(
        harness.service.clone(),
        Arc::new(StubCollector::successful(
            vec![candidate(
                post_id,
                author_id,
                FeedCandidateSource::MaterializedTimeline,
                0.9,
            )],
            Vec::new(),
        )),
        Arc::new(LinearFeedRanker::new(FeedRankWeights::default())),
        Arc::new(DeterministicFeedReranker::new(2, 1, 1)),
        repository,
        RankedFeedConfig {
            recent_sequence_enabled: true,
            recent_sequence_limit: 20,
            ..RankedFeedConfig::default()
        },
    );

    let response = service
        .execute(viewer_id, None, 10, Some("sequence-fallback"), None)
        .await
        .expect("ranked feed remains available");

    assert_eq!(response.posts.len(), 1);
    assert!(response.fallback_used);
    assert_eq!(response.rank_version.as_deref(), Some("feed-linear-v1"));
}

#[tokio::test]
async fn ranked_policies_require_author_in_own_batch_before_snapshot_effects() {
    // Complete, omitted, cross-chunk unsolicited, and failed-late-batch cases.
    for scenario in 0..4 {
        let harness = Harness::new();
        let viewer = Uuid::from_u128(10_000);
        let authors = (1..=101).map(Uuid::from_u128).collect::<Vec<_>>();
        let candidates = authors
            .iter()
            .enumerate()
            .map(|(n, author)| {
                let id = Uuid::from_u128(20_000 + n as u128);
                harness
                    .posts
                    .posts_by_id
                    .lock()
                    .unwrap()
                    .insert(id, post(id, *author, "ranked batch boundary"));
                candidate(id, *author, FeedCandidateSource::FreshTrending, 0.9)
            })
            .collect();
        if scenario == 1 || scenario == 2 {
            *harness.friend.omit_batch_policy_for.lock().unwrap() = Some(authors[100]);
        }
        if scenario == 2 {
            *harness.friend.extra_batch_policy_for.lock().unwrap() = Some(authors[100]);
        }
        if scenario == 3 {
            *harness.friend.fail_batch_policy_for.lock().unwrap() = Some(authors[100]);
        }
        let repository = Arc::new(MemoryFeedRepository::ready());
        let service = ranked_service(
            &harness,
            StubCollector::successful(candidates, Vec::new()),
            repository.clone(),
        );
        let result = service
            .execute(viewer, None, 10, Some("ranked-batch-boundary"), None)
            .await;
        if scenario == 0 {
            let result = result.expect("complete bounded policies permit ranked delivery");
            assert_eq!(result.posts.len(), 10);
            assert!(repository.snapshot.lock().unwrap().is_some());
            assert_eq!(repository.impressions.lock().unwrap().len(), 10);
            assert_eq!(repository.events.lock().unwrap().len(), 10);
        } else {
            let error =
                result.expect_err("another chunk cannot authorize a missing requested policy");
            assert!(error.to_string().contains(if scenario == 3 {
                "injected relationship batch failure"
            } else {
                "omitted a requested relationship policy"
            }));
            assert!(repository.snapshot.lock().unwrap().is_none());
            assert!(repository.impressions.lock().unwrap().is_empty());
            assert!(repository.events.lock().unwrap().is_empty());
        }
        let calls = harness.friend.batch_policy_calls.lock().unwrap();
        assert_eq!(calls[0].1, authors[..100]);
        assert_eq!(calls[1].1, vec![authors[100]]);
        assert_eq!(calls.len(), if scenario == 0 { 3 } else { 2 });
        assert!(calls
            .iter()
            .all(|(actor, targets)| *actor == viewer && targets.len() <= 100));
        assert!(
            harness.posts.fetch_feed_calls.lock().unwrap().is_empty(),
            "policy error must not trigger chronological fallback"
        );
        assert!(harness.feed_cache.set_calls.lock().unwrap().is_empty());
    }
}
