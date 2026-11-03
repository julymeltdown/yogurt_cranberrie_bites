use crate::application::ports::{
    FeedCandidateCollectorPort, FeedCandidateSourceRepositoryPort, FeedListRerankerPort,
    FeedRankerPort,
};
use crate::domain::recommendation::{
    diversity_rerank, FeedCandidate, FeedCandidateCollection, FeedCandidateSource, FeedRankWeights,
};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

#[derive(Clone)]
pub struct FeedCandidatePipeline {
    repository: Arc<dyn FeedCandidateSourceRepositoryPort>,
    source_timeout: Duration,
}

impl FeedCandidatePipeline {
    pub fn new(
        repository: Arc<dyn FeedCandidateSourceRepositoryPort>,
        source_timeout: Duration,
    ) -> Self {
        Self {
            repository,
            source_timeout,
        }
    }
}

#[async_trait]
impl FeedCandidateCollectorPort for FeedCandidatePipeline {
    async fn collect(
        &self,
        viewer_id: Uuid,
        following_ids: &[Uuid],
        per_source_limit: i32,
        now: DateTime<Utc>,
    ) -> Result<FeedCandidateCollection> {
        let limit = per_source_limit.clamp(1, 500);
        let repository = self.repository.clone();
        let materialized = timeout(
            self.source_timeout,
            repository.materialized_timeline_candidates(viewer_id, limit, now),
        );
        let repository = self.repository.clone();
        let high_fanout = timeout(
            self.source_timeout,
            repository.high_fanout_candidates(viewer_id, following_ids, limit, now),
        );
        let repository = self.repository.clone();
        let affinity = timeout(
            self.source_timeout,
            repository.author_affinity_candidates(viewer_id, following_ids, limit, now),
        );
        let repository = self.repository.clone();
        let social = timeout(
            self.source_timeout,
            repository.social_proof_candidates(viewer_id, following_ids, limit, now),
        );
        let repository = self.repository.clone();
        let trending = timeout(
            self.source_timeout,
            repository.fresh_trending_candidates(viewer_id, limit, now),
        );
        let repository = self.repository.clone();
        let exploration = timeout(
            self.source_timeout,
            repository.exploration_candidates(viewer_id, following_ids, limit, now),
        );

        let (materialized, high_fanout, affinity, social, trending, exploration) = tokio::join!(
            materialized,
            high_fanout,
            affinity,
            social,
            trending,
            exploration
        );

        let mut failed_sources = Vec::new();
        let mut by_post = HashMap::<Uuid, FeedCandidate>::new();
        merge_source(
            materialized,
            FeedCandidateSource::MaterializedTimeline,
            &mut failed_sources,
            &mut by_post,
        );
        merge_source(
            high_fanout,
            FeedCandidateSource::HighFanoutPull,
            &mut failed_sources,
            &mut by_post,
        );
        merge_source(
            affinity,
            FeedCandidateSource::AuthorAffinity,
            &mut failed_sources,
            &mut by_post,
        );
        merge_source(
            social,
            FeedCandidateSource::SocialProof,
            &mut failed_sources,
            &mut by_post,
        );
        merge_source(
            trending,
            FeedCandidateSource::FreshTrending,
            &mut failed_sources,
            &mut by_post,
        );
        merge_source(
            exploration,
            FeedCandidateSource::Exploration,
            &mut failed_sources,
            &mut by_post,
        );

        let mut candidates = by_post.into_values().collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .source_score
                .total_cmp(&left.source_score)
                .then_with(|| right.generated_at.cmp(&left.generated_at))
                .then_with(|| right.post_id.cmp(&left.post_id))
        });
        Ok(FeedCandidateCollection {
            candidates,
            failed_sources,
        })
    }
}

fn merge_source(
    result: Result<Result<Vec<FeedCandidate>>, tokio::time::error::Elapsed>,
    source: FeedCandidateSource,
    failed_sources: &mut Vec<FeedCandidateSource>,
    by_post: &mut HashMap<Uuid, FeedCandidate>,
) {
    let rows = match result {
        Ok(Ok(rows)) => rows,
        Ok(Err(error)) => {
            tracing::warn!(error = ?error, source = source.as_str(), "feed candidate source failed");
            failed_sources.push(source);
            return;
        }
        Err(error) => {
            tracing::warn!(error = ?error, source = source.as_str(), "feed candidate source timed out");
            failed_sources.push(source);
            return;
        }
    };
    for candidate in rows {
        by_post
            .entry(candidate.post_id)
            .and_modify(|current| {
                let current_quality = current.source_score + current.author_affinity;
                let candidate_quality = candidate.source_score + candidate.author_affinity;
                if candidate_quality > current_quality {
                    *current = candidate.clone();
                }
            })
            .or_insert(candidate);
    }
}

#[derive(Debug, Clone)]
pub struct LinearFeedRanker {
    weights: FeedRankWeights,
}

impl LinearFeedRanker {
    pub fn new(weights: FeedRankWeights) -> Self {
        Self { weights }
    }
}

impl FeedRankerPort for LinearFeedRanker {
    fn version(&self) -> &str {
        &self.weights.version
    }

    fn rank(&self, candidates: Vec<FeedCandidate>) -> Result<Vec<(FeedCandidate, i64)>> {
        let mut ranked = candidates
            .into_iter()
            .map(|candidate| {
                let score = self.weights.score(&candidate)?;
                Ok((candidate, (score * 1_000_000.0).round() as i64))
            })
            .collect::<Result<Vec<_>>>()?;
        ranked.sort_by(|left, right| {
            right
                .1
                .cmp(&left.1)
                .then_with(|| right.0.generated_at.cmp(&left.0.generated_at))
                .then_with(|| right.0.post_id.cmp(&left.0.post_id))
        });
        Ok(ranked)
    }
}

#[derive(Debug, Clone)]
pub struct DeterministicFeedReranker {
    max_per_author: usize,
    max_consecutive_author: usize,
    exploration_quota: usize,
}

impl DeterministicFeedReranker {
    pub fn new(
        max_per_author: usize,
        max_consecutive_author: usize,
        exploration_quota: usize,
    ) -> Self {
        Self {
            max_per_author: max_per_author.max(1),
            max_consecutive_author: max_consecutive_author.max(1),
            exploration_quota,
        }
    }
}

impl FeedListRerankerPort for DeterministicFeedReranker {
    fn rerank(
        &self,
        candidates: Vec<(FeedCandidate, i64)>,
        limit: usize,
    ) -> Vec<(FeedCandidate, i64)> {
        diversity_rerank(
            candidates,
            limit,
            self.max_per_author,
            self.max_consecutive_author,
            self.exploration_quota.min(limit),
        )
    }
}
