use super::PostApplicationService;
use crate::authority_witness::WitnessMemberDto;
use crate::dto::post_dto::{FeedDto, FeedItemMetadataDto};
use crate::ports::{
    FeedCandidateCollectorPort, FeedListRerankerPort, FeedRankerPort,
    FeedRecommendationRepositoryPort, RecommendationRollout,
};
use crate::query_witness::{
    OwnedQueryInputDto, OwnedQueryObservationDto, PublishedWitnessStageDto,
};
use anyhow::Result;
use chrono::{Duration, Utc};
use post_domain::recommendation::{
    FeedRecommendationImpression, FeedSnapshot, FeedSnapshotCursor, FeedSnapshotItem,
    RecommendationSurface,
};
use recommendation_policy::metrics::{
    NoopRecommendationMetrics, RecommendationMetricOutcome, RecommendationMetricReason,
    RecommendationMetricSource, RecommendationMetricStage, RecommendationMetricSurface,
    RecommendationMetricsPort,
};
use recommendation_policy::recommendation::RecentActionLimit;
use recommendation_policy::recommendation_sequence::{RecentSequence, RecentSequencePolicy};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RankedFeedConfig {
    pub per_source_limit: i32,
    pub snapshot_candidate_limit: usize,
    pub snapshot_ttl_seconds: i64,
    pub following_author_limit: usize,
    pub recent_sequence_enabled: bool,
    pub recent_sequence_limit: i32,
}

impl Default for RankedFeedConfig {
    fn default() -> Self {
        Self {
            per_source_limit: 200,
            snapshot_candidate_limit: 500,
            snapshot_ttl_seconds: 900,
            following_author_limit: 5_000,
            recent_sequence_enabled: false,
            recent_sequence_limit: 20,
        }
    }
}

#[derive(Clone)]
pub struct RankedFeedService {
    fallback: Arc<PostApplicationService>,
    collector: Arc<dyn FeedCandidateCollectorPort>,
    ranker: Arc<dyn FeedRankerPort>,
    reranker: Arc<dyn FeedListRerankerPort>,
    repository: Arc<dyn FeedRecommendationRepositoryPort>,
    config: RankedFeedConfig,
    metrics: Arc<dyn RecommendationMetricsPort>,
}

impl RankedFeedService {
    pub fn new(
        fallback: Arc<PostApplicationService>,
        collector: Arc<dyn FeedCandidateCollectorPort>,
        ranker: Arc<dyn FeedRankerPort>,
        reranker: Arc<dyn FeedListRerankerPort>,
        repository: Arc<dyn FeedRecommendationRepositoryPort>,
        config: RankedFeedConfig,
    ) -> Self {
        Self {
            fallback,
            collector,
            ranker,
            reranker,
            repository,
            config,
            metrics: Arc::new(NoopRecommendationMetrics),
        }
    }

    pub fn with_metrics(mut self, metrics: Arc<dyn RecommendationMetricsPort>) -> Self {
        self.metrics = metrics;
        self
    }

    pub async fn execute(
        &self,
        viewer_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
        request_id: Option<&str>,
        session_id: Option<Uuid>,
    ) -> Result<FeedDto> {
        let started = Instant::now();
        let result = self
            .execute_inner(viewer_id, cursor, limit, request_id, session_id)
            .await;
        self.metrics.record_request(
            RecommendationMetricSurface::HomeFeed,
            if result.is_ok() {
                RecommendationMetricOutcome::Success
            } else {
                RecommendationMetricOutcome::Failure
            },
        );
        self.metrics.observe_stage_latency(
            RecommendationMetricSurface::HomeFeed,
            RecommendationMetricStage::Ranking,
            started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        );
        result
    }

    async fn execute_inner(
        &self,
        viewer_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
        request_id: Option<&str>,
        session_id: Option<Uuid>,
    ) -> Result<FeedDto> {
        let limit = limit.clamp(1, 100);
        if let Some(value) = cursor.filter(|value| !value.trim().is_empty()) {
            return match FeedSnapshotCursor::parse(value) {
                Ok(Some(cursor)) => self.deliver_page(viewer_id, cursor, limit).await,
                Ok(None) => {
                    self.create_and_deliver(viewer_id, limit, request_id, session_id)
                        .await
                }
                Err(_) => Err(anyhow::anyhow!("Invalid feed cursor")),
            };
        }
        self.create_and_deliver(viewer_id, limit, request_id, session_id)
            .await
    }

    async fn create_and_deliver(
        &self,
        viewer_id: Uuid,
        limit: i32,
        request_id: Option<&str>,
        session_id: Option<Uuid>,
    ) -> Result<FeedDto> {
        let now = Utc::now();
        let serving = self.serving_selection().await;
        if serving.fallback_used {
            self.metrics.record_fallback(
                RecommendationMetricSurface::HomeFeed,
                RecommendationMetricReason::ModelUnavailable,
            );
        }
        match self.repository.model_ready(self.ranker.version()).await {
            Ok(true) => {}
            Ok(false) | Err(_) => {
                self.metrics.record_fallback(
                    RecommendationMetricSurface::HomeFeed,
                    RecommendationMetricReason::ModelUnavailable,
                );
                return self.fallback_feed(viewer_id, None, limit, request_id).await;
            }
        }
        let following_ids = self
            .fallback
            .friend_client
            .fetch_following_ids_bounded(viewer_id, self.config.following_author_limit)
            .await?;
        let collection = match self
            .collector
            .collect(viewer_id, &following_ids, self.config.per_source_limit, now)
            .await
        {
            Ok(collection) => collection,
            Err(error) => {
                self.metrics.record_fallback(
                    RecommendationMetricSurface::HomeFeed,
                    RecommendationMetricReason::CandidateSourceFailed,
                );
                tracing::warn!(error = ?error, %viewer_id, "ranked feed collection failed; using chronological fallback");
                return self.fallback_feed(viewer_id, None, limit, request_id).await;
            }
        };
        for source in [
            post_domain::recommendation::FeedCandidateSource::MaterializedTimeline,
            post_domain::recommendation::FeedCandidateSource::HighFanoutPull,
            post_domain::recommendation::FeedCandidateSource::AuthorAffinity,
            post_domain::recommendation::FeedCandidateSource::SocialProof,
            post_domain::recommendation::FeedCandidateSource::FreshTrending,
            post_domain::recommendation::FeedCandidateSource::SelfHistory,
            post_domain::recommendation::FeedCandidateSource::Exploration,
        ] {
            let failed = collection.failed_sources.contains(&source);
            self.metrics.record_source_run(
                RecommendationMetricSurface::HomeFeed,
                feed_metric_source(source),
                if failed {
                    RecommendationMetricOutcome::Failure
                } else {
                    RecommendationMetricOutcome::Success
                },
            );
            if failed {
                self.metrics.record_fallback(
                    RecommendationMetricSurface::HomeFeed,
                    RecommendationMetricReason::CandidateSourceFailed,
                );
                continue;
            }
            let count = collection
                .candidates
                .iter()
                .filter(|candidate| candidate.source == source)
                .count() as i64;
            if count > 0 {
                self.metrics.record_candidates(
                    RecommendationMetricSurface::HomeFeed,
                    feed_metric_source(source),
                    RecommendationMetricOutcome::Success,
                    count,
                );
            }
        }
        if collection.candidates.is_empty() {
            self.metrics.record_fallback(
                RecommendationMetricSurface::HomeFeed,
                RecommendationMetricReason::EmptyCandidates,
            );
            return self.fallback_feed(viewer_id, None, limit, request_id).await;
        }

        // Capture only the successful merged collection. Discarded source rows
        // never become members, and later filtering must use this exact stage.
        let members = collection
            .candidates
            .iter()
            .map(|candidate| {
                WitnessMemberDto::from_owned_query(candidate.post_id, candidate.author_id)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let stage = self
            .fallback
            .query_witnesses
            .observe(OwnedQueryObservationDto::new(
                viewer_id,
                OwnedQueryInputDto::Ranked {
                    per_source_limit: self.config.per_source_limit.clamp(1, 500),
                    following_ids,
                    failed_sources: collection
                        .failed_sources
                        .iter()
                        .map(|source| source.as_str().to_owned())
                        .collect(),
                    collected_at: now,
                },
                members,
                vec![],
            )?)
            .await?;
        let (candidates, sequence_fallback) = self
            .apply_recent_sequence(viewer_id, collection.candidates, now)
            .await;
        if sequence_fallback {
            self.metrics.record_fallback(
                RecommendationMetricSurface::HomeFeed,
                RecommendationMetricReason::SequenceUnavailable,
            );
        }
        let candidates = self
            .filter_candidates(viewer_id, candidates, stage.as_ref())
            .await?;
        let selected = if candidates.is_empty() {
            // Every collected candidate was excluded by the live relationship
            // policy check. Re-running the chronological path would issue more
            // Friend RPCs and can exceed the Gateway -> Post deadline without
            // adding any visible posts. Preserve this as an empty ranked page.
            Vec::new()
        } else {
            let ranked = match self.ranker.rank(candidates) {
                Ok(ranked) => ranked,
                Err(error) => {
                    tracing::warn!(error = ?error, %viewer_id, "ranked feed scorer failed; using chronological fallback");
                    return self.fallback_feed(viewer_id, None, limit, request_id).await;
                }
            };
            let selected = self
                .reranker
                .rerank(ranked, self.config.snapshot_candidate_limit);
            if selected.is_empty() {
                return self.fallback_feed(viewer_id, None, limit, request_id).await;
            }
            selected
        };

        let snapshot_id = Uuid::new_v4();
        let request_id = request_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let items = selected
            .into_iter()
            .enumerate()
            .map(|(position, (candidate, score_micros))| FeedSnapshotItem {
                post_id: candidate.post_id,
                author_id: candidate.author_id,
                source: candidate.source,
                reason_code: candidate.reason_code,
                score_micros,
                position: position as u32,
                impression_id: Uuid::new_v4(),
                propensity_micros: if candidate.exploration {
                    100_000
                } else {
                    1_000_000
                },
            })
            .collect::<Vec<_>>();
        self.repository
            .create_snapshot(&FeedSnapshot {
                snapshot_id,
                viewer_id,
                request_id,
                session_id,
                rank_version: self.ranker.version().to_owned(),
                fallback_used: serving.fallback_used || sequence_fallback,
                failed_sources: collection.failed_sources,
                created_at: now,
                expires_at: now + Duration::seconds(self.config.snapshot_ttl_seconds.max(60)),
                items,
            })
            .await?;
        self.deliver_page(
            viewer_id,
            FeedSnapshotCursor {
                snapshot_id,
                offset: 0,
            },
            limit,
        )
        .await
    }

    async fn deliver_page(
        &self,
        viewer_id: Uuid,
        cursor: FeedSnapshotCursor,
        limit: i32,
    ) -> Result<FeedDto> {
        let now = Utc::now();
        let page = self
            .repository
            .load_snapshot_page(viewer_id, cursor.snapshot_id, cursor.offset, limit, now)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Feed snapshot not found or expired"))?;
        let post_ids = page
            .items
            .iter()
            .map(|item| item.post_id)
            .collect::<Vec<_>>();
        let posts = self
            .fallback
            .hydrate_recommendation_posts(
                &post_ids,
                viewer_id,
                OwnedQueryInputDto::Snapshot {
                    snapshot_id: cursor.snapshot_id,
                    offset: cursor.offset,
                    limit,
                    page_post_ids: post_ids.clone(),
                },
            )
            .await?;
        let delivered_ids = posts
            .posts
            .iter()
            .filter_map(|post| Uuid::parse_str(&post.id).ok())
            .collect::<HashSet<_>>();
        let impressions = page
            .items
            .iter()
            .filter(|item| delivered_ids.contains(&item.post_id))
            .map(|item| FeedRecommendationImpression {
                impression_id: item.impression_id,
                snapshot_id: page.snapshot_id,
                viewer_id,
                post_id: item.post_id,
                author_id: item.author_id,
                source: item.source,
                reason_code: item.reason_code.clone(),
                position: item.position,
                rank_version: page.rank_version.clone(),
                request_id: page.request_id.clone(),
                session_id: page.session_id,
                score_micros: item.score_micros,
                propensity_micros: item.propensity_micros,
                occurred_at: now,
            })
            .collect::<Vec<_>>();
        self.repository.record_impressions(&impressions).await?;
        let metadata_by_post = page
            .items
            .iter()
            .map(|item| {
                (
                    item.post_id,
                    FeedItemMetadataDto {
                        post_id: item.post_id.to_string(),
                        source: item.source.as_str().to_owned(),
                        reason_code: item.reason_code.clone(),
                        score_micros: item.score_micros,
                        position: item.position,
                        impression_id: item.impression_id.to_string(),
                        propensity_micros: item.propensity_micros,
                        exploration: item.source
                            == post_domain::recommendation::FeedCandidateSource::Exploration,
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        let items = posts
            .posts
            .iter()
            .filter_map(|post| Uuid::parse_str(&post.id).ok())
            .filter_map(|post_id| metadata_by_post.get(&post_id).cloned())
            .collect();
        Ok(FeedDto {
            posts: posts.posts,
            next_cursor: page.next_cursor,
            request_id: Some(page.request_id),
            snapshot_id: Some(page.snapshot_id.to_string()),
            rank_version: Some(page.rank_version),
            fallback_used: page.fallback_used,
            items,
            failed_sources: page
                .failed_sources
                .into_iter()
                .map(|source| source.as_str().to_owned())
                .collect(),
        })
    }

    async fn serving_selection(&self) -> ServingSelection {
        let fallback = ServingSelection {
            fallback_used: true,
        };
        let rollout = match self.repository.rollout("HOME_FEED").await {
            Ok(rollout) => rollout,
            Err(error) => {
                tracing::warn!(error = ?error, "feed recommendation rollout lookup failed; using deterministic ranker");
                return fallback;
            }
        };
        select_serving_ranker(&rollout, self.ranker.version())
    }

    async fn fallback_feed(
        &self,
        viewer_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
        request_id: Option<&str>,
    ) -> Result<FeedDto> {
        let mut feed = self.fallback.get_feed(viewer_id, cursor, limit).await?;
        feed.request_id = Some(
            request_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
        );
        feed.rank_version = Some("chronological-v1".into());
        feed.fallback_used = true;
        Ok(feed)
    }

    async fn apply_recent_sequence(
        &self,
        viewer_id: Uuid,
        mut candidates: Vec<post_domain::recommendation::FeedCandidate>,
        now: chrono::DateTime<Utc>,
    ) -> (Vec<post_domain::recommendation::FeedCandidate>, bool) {
        if !self.config.recent_sequence_enabled {
            return (candidates, false);
        }
        let limit = RecentActionLimit::new(self.config.recent_sequence_limit);
        let events = match self.repository.recent_actions(viewer_id, limit).await {
            Ok(events) => events,
            Err(error) => {
                tracing::warn!(error = ?error, %viewer_id, "feed recent sequence lookup failed; using projection features");
                return (candidates, true);
            }
        };
        let contract_events = match events
            .iter()
            .map(post_domain::recommendation::RecommendationEvent::to_contract)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(events) => events,
            Err(error) => {
                tracing::warn!(error = ?error, %viewer_id, "feed recent sequence contained an invalid event; using projection features");
                return (candidates, true);
            }
        };
        let sequence = match RecentSequence::build(
            viewer_id,
            RecommendationSurface::HomeFeed,
            now,
            contract_events,
            RecentSequencePolicy {
                max_actions: usize::from(limit.get()),
                ..RecentSequencePolicy::default()
            },
        ) {
            Ok(sequence) => sequence,
            Err(error) => {
                tracing::warn!(error = ?error, %viewer_id, "invalid feed recent sequence; using projection features");
                return (candidates, true);
            }
        };
        for candidate in &mut candidates {
            candidate.repeated_impression_penalty = candidate
                .repeated_impression_penalty
                .max(sequence.repeat_penalty(candidate.post_id));
            candidate.negative_feedback_penalty = candidate
                .negative_feedback_penalty
                .max(sequence.negative_penalty(candidate.post_id));
        }
        (candidates, false)
    }

    async fn filter_candidates(
        &self,
        viewer_id: Uuid,
        candidates: Vec<post_domain::recommendation::FeedCandidate>,
        stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<Vec<post_domain::recommendation::FeedCandidate>> {
        let mut author_ids = candidates
            .iter()
            .map(|candidate| candidate.author_id)
            .filter(|author_id| *author_id != viewer_id)
            .collect::<Vec<_>>();
        author_ids.sort_unstable();
        author_ids.dedup();
        // Validate every author within its own bounded request. A response from
        // an earlier chunk cannot supply authority omitted by a later one.
        let policies = self
            .fallback
            .relationship_policies(viewer_id, &author_ids, stage)
            .await?;
        let mut visible = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            if candidate.author_id == viewer_id {
                visible.push(candidate);
                continue;
            }
            let policy = policies.get(&candidate.author_id).copied().ok_or_else(|| {
                anyhow::anyhow!("Friend service omitted a requested relationship policy")
            })?;
            if !policy.is_blocked() && !policy.muted_by_viewer {
                visible.push(candidate);
            }
        }
        Ok(visible)
    }
}

fn feed_metric_source(
    source: post_domain::recommendation::FeedCandidateSource,
) -> RecommendationMetricSource {
    use post_domain::recommendation::FeedCandidateSource;
    match source {
        FeedCandidateSource::MaterializedTimeline => RecommendationMetricSource::Timeline,
        FeedCandidateSource::SelfHistory => RecommendationMetricSource::SelfHistory,
        FeedCandidateSource::HighFanoutPull => RecommendationMetricSource::HighFanout,
        FeedCandidateSource::AuthorAffinity => RecommendationMetricSource::AuthorAffinity,
        FeedCandidateSource::SocialProof => RecommendationMetricSource::SocialProof,
        FeedCandidateSource::FreshTrending => RecommendationMetricSource::Trending,
        FeedCandidateSource::Exploration => RecommendationMetricSource::Exploration,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ServingSelection {
    fallback_used: bool,
}

fn select_serving_ranker(
    rollout: &RecommendationRollout,
    deterministic_version: &str,
) -> ServingSelection {
    let deterministic_matches = rollout.deterministic_version == deterministic_version;
    let learned_requested = rollout.mode.requests_learned_scoring();
    if learned_requested {
        tracing::warn!(
            mode = rollout.mode.as_str(),
            learned_ready = rollout.learned_ready,
            learned_version = rollout.learned_version.as_deref().unwrap_or(""),
            canary_percent = rollout.canary_percent,
            "learned feed ranker is not configured; using deterministic fallback"
        );
    }
    ServingSelection {
        fallback_used: learned_requested || !deterministic_matches,
    }
}
