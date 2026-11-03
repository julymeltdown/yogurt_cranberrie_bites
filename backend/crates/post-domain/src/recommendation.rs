use chrono::{DateTime, TimeZone, Utc};
use recommendation_policy::recommendation::{
    RecommendationContractError, RecommendationCursor, MAX_RECOMMENDATION_CURSOR_LENGTH,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use friend_compatible_event::RecommendationEvent;
pub use recommendation_policy::recommendation::{
    RecommendationAction, RecommendationSubjectType, RecommendationSurface, RolloutMode,
};

const SNAPSHOT_CURSOR_VERSION: u8 = 1;

mod friend_compatible_event {
    pub use crate::recommendation_event::RecommendationEvent;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FeedCandidateSource {
    MaterializedTimeline,
    HighFanoutPull,
    AuthorAffinity,
    SocialProof,
    FreshTrending,
    SelfHistory,
    Exploration,
}

impl FeedCandidateSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MaterializedTimeline => "MATERIALIZED_TIMELINE",
            Self::HighFanoutPull => "HIGH_FANOUT_PULL",
            Self::AuthorAffinity => "AUTHOR_AFFINITY",
            Self::SocialProof => "SOCIAL_PROOF",
            Self::FreshTrending => "FRESH_TRENDING",
            Self::SelfHistory => "SELF_HISTORY",
            Self::Exploration => "EXPLORATION",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RecommendationContractError> {
        match value.trim().to_ascii_uppercase().as_str() {
            "MATERIALIZED_TIMELINE" => Ok(Self::MaterializedTimeline),
            "HIGH_FANOUT_PULL" => Ok(Self::HighFanoutPull),
            "AUTHOR_AFFINITY" => Ok(Self::AuthorAffinity),
            "SOCIAL_PROOF" => Ok(Self::SocialProof),
            "FRESH_TRENDING" | "TRENDING" => Ok(Self::FreshTrending),
            "SELF_HISTORY" => Ok(Self::SelfHistory),
            "EXPLORATION" | "DISCOVERY" => Ok(Self::Exploration),
            _ => Err(RecommendationContractError::new(
                "Unsupported feed candidate source",
            )),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct FeedCandidate {
    pub post_id: Uuid,
    pub author_id: Uuid,
    pub source: FeedCandidateSource,
    pub source_score: f64,
    pub author_affinity: f64,
    pub freshness: f64,
    pub social_proof: f64,
    pub engagement_velocity: f64,
    pub topic_match: f64,
    pub repeated_impression_penalty: f64,
    pub author_saturation_penalty: f64,
    pub negative_feedback_penalty: f64,
    pub generated_at: DateTime<Utc>,
    pub reason_code: String,
    pub in_network: bool,
    pub exploration: bool,
}

impl FeedCandidate {
    pub fn components(&self) -> std::collections::BTreeMap<String, f64> {
        std::collections::BTreeMap::from([
            ("sourceScore".into(), self.source_score),
            ("authorAffinity".into(), self.author_affinity),
            ("freshness".into(), self.freshness),
            ("socialProof".into(), self.social_proof),
            ("engagementVelocity".into(), self.engagement_velocity),
            ("topicMatch".into(), self.topic_match),
            (
                "repeatedImpressionPenalty".into(),
                self.repeated_impression_penalty,
            ),
            (
                "authorSaturationPenalty".into(),
                self.author_saturation_penalty,
            ),
            (
                "negativeFeedbackPenalty".into(),
                self.negative_feedback_penalty,
            ),
        ])
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FeedRankWeights {
    pub version: String,
    pub source_score: f64,
    pub author_affinity: f64,
    pub freshness: f64,
    pub social_proof: f64,
    pub engagement_velocity: f64,
    pub topic_match: f64,
    pub repeated_impression_penalty: f64,
    pub author_saturation_penalty: f64,
    pub negative_feedback_penalty: f64,
}

impl Default for FeedRankWeights {
    fn default() -> Self {
        Self {
            version: "feed-linear-v1".into(),
            source_score: 1.4,
            author_affinity: 1.2,
            freshness: 1.0,
            social_proof: 0.8,
            engagement_velocity: 0.7,
            topic_match: 0.4,
            repeated_impression_penalty: 0.8,
            author_saturation_penalty: 0.7,
            negative_feedback_penalty: 1.2,
        }
    }
}

impl FeedRankWeights {
    pub fn score(&self, candidate: &FeedCandidate) -> Result<f64, RecommendationContractError> {
        let values = [
            candidate.source_score,
            candidate.author_affinity,
            candidate.freshness,
            candidate.social_proof,
            candidate.engagement_velocity,
            candidate.topic_match,
            candidate.repeated_impression_penalty,
            candidate.author_saturation_penalty,
            candidate.negative_feedback_penalty,
        ];
        if values.iter().any(|value| !value.is_finite()) {
            return Err(RecommendationContractError::new(
                "Feed recommendation features must be finite",
            ));
        }
        Ok(self.source_score * clamp01(candidate.source_score)
            + self.author_affinity * clamp01(candidate.author_affinity)
            + self.freshness * clamp01(candidate.freshness)
            + self.social_proof * clamp01(candidate.social_proof)
            + self.engagement_velocity * clamp01(candidate.engagement_velocity)
            + self.topic_match * clamp01(candidate.topic_match)
            - self.repeated_impression_penalty * clamp01(candidate.repeated_impression_penalty)
            - self.author_saturation_penalty * clamp01(candidate.author_saturation_penalty)
            - self.negative_feedback_penalty * clamp01(candidate.negative_feedback_penalty))
    }
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct FeedSnapshotCursor {
    pub snapshot_id: Uuid,
    pub offset: u32,
}

impl FeedSnapshotCursor {
    pub fn encode(&self) -> String {
        RecommendationCursor {
            version: SNAPSHOT_CURSOR_VERSION,
            snapshot_id: self.snapshot_id,
            score_micros: i64::from(self.offset),
            occurred_at: Utc.timestamp_opt(0, 0).single().expect("epoch"),
            tie_breaker_id: self.snapshot_id,
        }
        .encode()
    }

    pub fn parse(value: &str) -> Result<Option<Self>, RecommendationContractError> {
        if value.len() > MAX_RECOMMENDATION_CURSOR_LENGTH {
            return Err(RecommendationContractError::new("Invalid feed cursor"));
        }
        if value.trim().is_empty() {
            return Ok(None);
        }
        let cursor = RecommendationCursor::parse(value)?;
        if cursor.occurred_at.timestamp_millis() != 0
            || cursor.tie_breaker_id != cursor.snapshot_id
            || cursor.score_micros < 0
            || cursor.score_micros > i64::from(u32::MAX)
        {
            return Err(RecommendationContractError::new("Invalid feed cursor"));
        }
        Ok(Some(Self {
            snapshot_id: cursor.snapshot_id,
            offset: cursor.score_micros as u32,
        }))
    }
}

pub fn diversity_rerank(
    mut candidates: Vec<(FeedCandidate, i64)>,
    limit: usize,
    max_per_author: usize,
    max_consecutive_author: usize,
    exploration_quota: usize,
) -> Vec<(FeedCandidate, i64)> {
    candidates.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.0.generated_at.cmp(&left.0.generated_at))
            .then_with(|| right.0.post_id.cmp(&left.0.post_id))
    });

    let mut selected = Vec::with_capacity(limit.min(candidates.len()));
    let mut author_counts = std::collections::HashMap::<Uuid, usize>::new();
    let mut exploration_count = 0usize;

    while selected.len() < limit {
        let last_author = selected
            .last()
            .map(|item: &(FeedCandidate, i64)| item.0.author_id);
        let consecutive = last_author.map_or(0, |author| {
            selected
                .iter()
                .rev()
                .take_while(|item| item.0.author_id == author)
                .count()
        });
        let position = candidates.iter().position(|(candidate, _)| {
            let author_count = author_counts
                .get(&candidate.author_id)
                .copied()
                .unwrap_or(0);
            let author_allowed = author_count < max_per_author;
            let consecutive_allowed =
                last_author != Some(candidate.author_id) || consecutive < max_consecutive_author;
            let exploration_allowed =
                !candidate.exploration || exploration_count < exploration_quota;
            author_allowed && consecutive_allowed && exploration_allowed
        });
        let Some(position) = position else {
            break;
        };
        let item = candidates.remove(position);
        *author_counts.entry(item.0.author_id).or_default() += 1;
        if item.0.exploration {
            exploration_count += 1;
        }
        selected.push(item);
    }
    selected
}

#[derive(Clone, PartialEq, Eq)]
pub struct FollowerPage {
    pub follower_ids: Vec<Uuid>,
    pub next_cursor: Option<String>,
    pub total_count: i64,
}

#[derive(Clone, PartialEq, Eq)]
pub struct FeedSnapshotItem {
    pub post_id: Uuid,
    pub author_id: Uuid,
    pub source: FeedCandidateSource,
    pub reason_code: String,
    pub score_micros: i64,
    pub position: u32,
    pub impression_id: Uuid,
    pub propensity_micros: u32,
}

#[derive(Clone, PartialEq, Eq)]
pub struct FeedSnapshot {
    pub snapshot_id: Uuid,
    pub viewer_id: Uuid,
    pub request_id: String,
    pub session_id: Option<Uuid>,
    pub rank_version: String,
    pub fallback_used: bool,
    pub failed_sources: Vec<FeedCandidateSource>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub items: Vec<FeedSnapshotItem>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct FeedSnapshotPage {
    pub snapshot_id: Uuid,
    pub request_id: String,
    pub session_id: Option<Uuid>,
    pub rank_version: String,
    pub fallback_used: bool,
    pub failed_sources: Vec<FeedCandidateSource>,
    pub items: Vec<FeedSnapshotItem>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct FeedRecommendationImpression {
    pub impression_id: Uuid,
    pub snapshot_id: Uuid,
    pub viewer_id: Uuid,
    pub post_id: Uuid,
    pub author_id: Uuid,
    pub source: FeedCandidateSource,
    pub reason_code: String,
    pub position: u32,
    pub rank_version: String,
    pub request_id: String,
    pub session_id: Option<Uuid>,
    pub score_micros: i64,
    pub propensity_micros: u32,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct TimelineFanoutJob {
    pub job_id: Uuid,
    pub event_id: Uuid,
    pub post_id: Uuid,
    pub author_id: Uuid,
    pub post_created_at: DateTime<Utc>,
    pub cursor: Option<String>,
    pub attempt_count: i32,
}

#[derive(Clone, PartialEq)]
pub struct FeedCandidateCollection {
    pub candidates: Vec<FeedCandidate>,
    pub failed_sources: Vec<FeedCandidateSource>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct FeedExperimentAssignment {
    pub experiment_key: String,
    pub variant: String,
    pub rollout_mode: RolloutMode,
}

pub fn stable_experiment_bucket(viewer_id: Uuid, experiment_key: &str, buckets: u32) -> u32 {
    if buckets == 0 {
        return 0;
    }
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in viewer_id
        .as_bytes()
        .iter()
        .copied()
        .chain(experiment_key.as_bytes().iter().copied())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    (hash % u64::from(buckets)) as u32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FeedRecommendationRetentionResult {
    pub timeline_entries: i64,
    pub high_fanout_posts: i64,
    pub snapshot_items: i64,
    pub impressions: i64,
    pub snapshots: i64,
    pub events: i64,
    pub experiment_assignments: i64,
    pub embeddings: i64,
    pub fanout_jobs: i64,
}

impl FeedRecommendationRetentionResult {
    pub fn total(self) -> i64 {
        self.timeline_entries
            + self.high_fanout_posts
            + self.snapshot_items
            + self.impressions
            + self.snapshots
            + self.events
            + self.experiment_assignments
            + self.embeddings
            + self.fanout_jobs
    }
}

macro_rules! redact_debug {
    ($($type:ty),+ $(,)?) => {
        $(
            impl std::fmt::Debug for $type {
                fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    formatter
                        .debug_struct(stringify!($type))
                        .field("details", &"[REDACTED]")
                        .finish()
                }
            }
        )+
    };
}

redact_debug!(
    FeedCandidate,
    FeedSnapshotCursor,
    FollowerPage,
    FeedSnapshotItem,
    FeedSnapshot,
    FeedSnapshotPage,
    FeedRecommendationImpression,
    TimelineFanoutJob,
    FeedCandidateCollection,
    FeedExperimentAssignment,
);

#[cfg(test)]
mod debug_redaction_tests {
    use super::*;

    #[test]
    fn recommendation_debug_hides_identity_and_behavioral_signals() {
        let post_id = Uuid::new_v4();
        let author_id = Uuid::new_v4();
        let viewer_id = Uuid::new_v4();
        let candidate = FeedCandidate {
            post_id,
            author_id,
            source: FeedCandidateSource::AuthorAffinity,
            source_score: 0.7,
            author_affinity: 0.8,
            freshness: 0.9,
            social_proof: 0.4,
            engagement_velocity: 0.3,
            topic_match: 0.2,
            repeated_impression_penalty: 0.1,
            author_saturation_penalty: 0.0,
            negative_feedback_penalty: 0.6,
            generated_at: Utc::now(),
            reason_code: "private-reason-47bc".into(),
            in_network: true,
            exploration: false,
        };
        let snapshot_item = FeedSnapshotItem {
            post_id,
            author_id,
            source: FeedCandidateSource::AuthorAffinity,
            reason_code: "private-snapshot-reason".into(),
            score_micros: 700_000,
            position: 1,
            impression_id: Uuid::new_v4(),
            propensity_micros: 500_000,
        };
        let snapshot = FeedSnapshot {
            snapshot_id: Uuid::new_v4(),
            viewer_id,
            request_id: "private-feed-request".into(),
            session_id: Some(Uuid::new_v4()),
            rank_version: "private-rank-version".into(),
            fallback_used: false,
            failed_sources: vec![FeedCandidateSource::SocialProof],
            created_at: Utc::now(),
            expires_at: Utc::now(),
            items: vec![snapshot_item.clone()],
        };
        let impression = FeedRecommendationImpression {
            impression_id: Uuid::new_v4(),
            snapshot_id: snapshot.snapshot_id,
            viewer_id,
            post_id,
            author_id,
            source: FeedCandidateSource::AuthorAffinity,
            reason_code: "private-impression-reason".into(),
            position: 1,
            rank_version: "private-impression-rank".into(),
            request_id: "private-impression-request".into(),
            session_id: Some(Uuid::new_v4()),
            score_micros: 900_000,
            propensity_micros: 400_000,
            occurred_at: Utc::now(),
        };
        let values = [
            format!("{candidate:?}"),
            format!("{snapshot:?}"),
            format!(
                "{:?}",
                FeedSnapshotPage {
                    snapshot_id: snapshot.snapshot_id,
                    request_id: snapshot.request_id.clone(),
                    session_id: snapshot.session_id,
                    rank_version: snapshot.rank_version.clone(),
                    fallback_used: snapshot.fallback_used,
                    failed_sources: snapshot.failed_sources.clone(),
                    items: snapshot.items.clone(),
                    next_cursor: Some("private-page-cursor".into()),
                }
            ),
            format!("{impression:?}"),
            format!(
                "{:?}",
                FollowerPage {
                    follower_ids: vec![viewer_id],
                    next_cursor: Some("private-follower-cursor".into()),
                    total_count: 1,
                }
            ),
            format!(
                "{:?}",
                TimelineFanoutJob {
                    job_id: Uuid::new_v4(),
                    event_id: Uuid::new_v4(),
                    post_id,
                    author_id,
                    post_created_at: Utc::now(),
                    cursor: Some("private-fanout-cursor".into()),
                    attempt_count: 0,
                }
            ),
            format!(
                "{:?}",
                FeedCandidateCollection {
                    candidates: vec![candidate.clone()],
                    failed_sources: vec![FeedCandidateSource::SocialProof],
                }
            ),
            format!(
                "{:?}",
                FeedExperimentAssignment {
                    experiment_key: "private-experiment-key".into(),
                    variant: "private-experiment-variant".into(),
                    rollout_mode: RolloutMode::Deterministic,
                }
            ),
            format!(
                "{:?}",
                FeedSnapshotCursor {
                    snapshot_id: snapshot.snapshot_id,
                    offset: 4
                }
            ),
        ];

        for rendered in values {
            for secret in [
                "private-",
                &post_id.to_string(),
                &author_id.to_string(),
                &viewer_id.to_string(),
            ] {
                assert!(
                    !rendered.contains(secret),
                    "debug leaked {secret}: {rendered}"
                );
            }
            assert!(rendered.contains("[REDACTED]"));
        }
    }
}
