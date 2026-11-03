use base64ct::{Base64UrlUnpadded, Encoding};
use chrono::{DateTime, Utc};
use recommendation_policy::recommendation::{
    RecommendationContractError, RecommendationCursor, MAX_RECOMMENDATION_CURSOR_LENGTH,
};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

pub use recommendation_policy::recommendation::RolloutMode;

const PEOPLE_CURSOR_VERSION: u8 = 1;
const SCORE_SCALE: f64 = 1_000_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PeopleCandidateSource {
    MutualFollow,
    FollowerOverlap,
    CoEngagement,
    ProfileOverlap,
    NewMember,
    Exploration,
}

impl PeopleCandidateSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MutualFollow => "MUTUAL_FOLLOW",
            Self::FollowerOverlap => "FOLLOWER_OVERLAP",
            Self::CoEngagement => "CO_ENGAGEMENT",
            Self::ProfileOverlap => "PROFILE_OVERLAP",
            Self::NewMember => "NEW_MEMBER",
            Self::Exploration => "EXPLORATION",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RecommendationContractError> {
        match value.trim().to_ascii_uppercase().as_str() {
            "MUTUAL_FOLLOW" => Ok(Self::MutualFollow),
            "FOLLOWER_OVERLAP" => Ok(Self::FollowerOverlap),
            "CO_ENGAGEMENT" => Ok(Self::CoEngagement),
            "PROFILE_OVERLAP" => Ok(Self::ProfileOverlap),
            "NEW_MEMBER" => Ok(Self::NewMember),
            "EXPLORATION" => Ok(Self::Exploration),
            _ => Err(RecommendationContractError::new(
                "Unsupported people candidate source",
            )),
        }
    }

    pub fn is_exploration(self) -> bool {
        matches!(self, Self::NewMember | Self::Exploration)
    }

    pub fn reason_code(self) -> &'static str {
        match self {
            Self::MutualFollow => "MUTUAL_CONNECTIONS",
            Self::FollowerOverlap => "SIMILAR_NETWORK",
            Self::CoEngagement => "SHARED_ACTIVITY",
            Self::ProfileOverlap => "SHARED_INTERESTS",
            Self::NewMember => "NEW_MEMBER",
            Self::Exploration => "DISCOVER_SOMEONE_NEW",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PeopleCandidateFeatures {
    pub mutual_count: u32,
    pub mutual_score: f64,
    pub adamic_adar: f64,
    pub jaccard: f64,
    pub profile_overlap: f64,
    pub interaction_affinity: f64,
    pub freshness: f64,
    pub candidate_quality: f64,
    pub impression_penalty: f64,
    pub dismissal_penalty: f64,
}

impl PeopleCandidateFeatures {
    pub fn components(&self) -> BTreeMap<String, f64> {
        BTreeMap::from([
            ("mutualCount".into(), f64::from(self.mutual_count)),
            ("mutualScore".into(), self.mutual_score),
            ("adamicAdar".into(), self.adamic_adar),
            ("jaccard".into(), self.jaccard),
            ("profileOverlap".into(), self.profile_overlap),
            ("interactionAffinity".into(), self.interaction_affinity),
            ("freshness".into(), self.freshness),
            ("candidateQuality".into(), self.candidate_quality),
            ("impressionPenalty".into(), self.impression_penalty),
            ("dismissalPenalty".into(), self.dismissal_penalty),
        ])
    }

    fn validate(&self) -> Result<(), RecommendationContractError> {
        if self.components().values().any(|value| !value.is_finite()) {
            return Err(RecommendationContractError::new(
                "People recommendation features must be finite",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PeopleRankWeights {
    pub version: String,
    pub mutual_score: f64,
    pub adamic_adar: f64,
    pub jaccard: f64,
    pub profile_overlap: f64,
    pub interaction_affinity: f64,
    pub freshness: f64,
    pub candidate_quality: f64,
    pub impression_penalty: f64,
    pub dismissal_penalty: f64,
    pub source_quotas: HashMap<PeopleCandidateSource, usize>,
    pub cluster_cap: usize,
    pub exploration_quota: usize,
    pub max_results: usize,
}

impl Default for PeopleRankWeights {
    fn default() -> Self {
        Self {
            version: "people-linear-v1".into(),
            mutual_score: 0.30,
            adamic_adar: 0.20,
            jaccard: 0.15,
            profile_overlap: 0.10,
            interaction_affinity: 0.15,
            freshness: 0.05,
            candidate_quality: 0.05,
            impression_penalty: 0.35,
            dismissal_penalty: 1.0,
            source_quotas: HashMap::from([
                (PeopleCandidateSource::MutualFollow, 80),
                (PeopleCandidateSource::FollowerOverlap, 40),
                (PeopleCandidateSource::CoEngagement, 30),
                (PeopleCandidateSource::ProfileOverlap, 20),
                (PeopleCandidateSource::NewMember, 10),
                (PeopleCandidateSource::Exploration, 10),
            ]),
            cluster_cap: 3,
            exploration_quota: 2,
            max_results: 100,
        }
    }
}

impl PeopleRankWeights {
    pub fn score(
        &self,
        features: &PeopleCandidateFeatures,
    ) -> Result<f64, RecommendationContractError> {
        features.validate()?;
        let weights = [
            self.mutual_score,
            self.adamic_adar,
            self.jaccard,
            self.profile_overlap,
            self.interaction_affinity,
            self.freshness,
            self.candidate_quality,
            self.impression_penalty,
            self.dismissal_penalty,
        ];
        if weights.iter().any(|value| !value.is_finite()) {
            return Err(RecommendationContractError::new(
                "People rank weights must be finite",
            ));
        }
        let score = self.mutual_score * bounded(features.mutual_score)
            + self.adamic_adar * bounded(features.adamic_adar)
            + self.jaccard * bounded(features.jaccard)
            + self.profile_overlap * bounded(features.profile_overlap)
            + self.interaction_affinity * bounded(features.interaction_affinity)
            + self.freshness * bounded(features.freshness)
            + self.candidate_quality * bounded(features.candidate_quality)
            - self.impression_penalty * bounded(features.impression_penalty)
            - self.dismissal_penalty * bounded(features.dismissal_penalty);
        if !score.is_finite() {
            return Err(RecommendationContractError::new(
                "People recommendation score must be finite",
            ));
        }
        Ok(score)
    }
}

fn bounded(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PeopleCandidate {
    pub viewer_id: Uuid,
    pub candidate_user_id: Uuid,
    pub source: PeopleCandidateSource,
    pub source_score: f64,
    pub features: PeopleCandidateFeatures,
    pub mutual_cluster: Option<Uuid>,
    pub generated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub model_version: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RankedPeopleCandidate {
    pub viewer_id: Uuid,
    pub candidate_user_id: Uuid,
    pub source: PeopleCandidateSource,
    pub reason_code: String,
    pub score: f64,
    pub score_micros: i64,
    pub features: PeopleCandidateFeatures,
    pub mutual_cluster: Option<Uuid>,
    pub generated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub model_version: String,
}

pub fn rank_people_candidates(
    candidates: Vec<PeopleCandidate>,
    weights: &PeopleRankWeights,
) -> Result<Vec<RankedPeopleCandidate>, RecommendationContractError> {
    if weights.version.trim().is_empty() || weights.max_results == 0 || weights.cluster_cap == 0 {
        return Err(RecommendationContractError::new(
            "People rank configuration is invalid",
        ));
    }
    let mut best_by_candidate = HashMap::<Uuid, RankedPeopleCandidate>::new();
    for candidate in candidates {
        if candidate.viewer_id.is_nil()
            || candidate.candidate_user_id.is_nil()
            || candidate.viewer_id == candidate.candidate_user_id
            || !candidate.source_score.is_finite()
            || candidate.model_version.trim().is_empty()
            || candidate.expires_at <= candidate.generated_at
        {
            continue;
        }
        let score = weights.score(&candidate.features)? + bounded(candidate.source_score);
        let ranked = RankedPeopleCandidate {
            viewer_id: candidate.viewer_id,
            candidate_user_id: candidate.candidate_user_id,
            source: candidate.source,
            reason_code: candidate.source.reason_code().into(),
            score,
            score_micros: (score * SCORE_SCALE).round() as i64,
            features: candidate.features,
            mutual_cluster: candidate.mutual_cluster,
            generated_at: candidate.generated_at,
            expires_at: candidate.expires_at,
            model_version: candidate.model_version,
        };
        match best_by_candidate.get(&ranked.candidate_user_id) {
            Some(existing) if compare_ranked(existing, &ranked) != Ordering::Less => {}
            _ => {
                best_by_candidate.insert(ranked.candidate_user_id, ranked);
            }
        }
    }
    let mut candidates = best_by_candidate.into_values().collect::<Vec<_>>();
    candidates.sort_by(compare_ranked_desc);
    let mut output = Vec::with_capacity(weights.max_results.min(candidates.len()));
    let mut source_counts = HashMap::<PeopleCandidateSource, usize>::new();
    let mut cluster_counts = HashMap::<Uuid, usize>::new();
    let mut exploration_count = 0usize;
    for candidate in candidates {
        if output.len() >= weights.max_results {
            break;
        }
        let source_limit = weights
            .source_quotas
            .get(&candidate.source)
            .copied()
            .unwrap_or(weights.max_results);
        if source_counts.get(&candidate.source).copied().unwrap_or(0) >= source_limit {
            continue;
        }
        if candidate.source.is_exploration() && exploration_count >= weights.exploration_quota {
            continue;
        }
        if candidate.mutual_cluster.is_some_and(|cluster| {
            cluster_counts.get(&cluster).copied().unwrap_or(0) >= weights.cluster_cap
        }) {
            continue;
        }
        *source_counts.entry(candidate.source).or_default() += 1;
        if candidate.source.is_exploration() {
            exploration_count += 1;
        }
        if let Some(cluster) = candidate.mutual_cluster {
            *cluster_counts.entry(cluster).or_default() += 1;
        }
        output.push(candidate);
    }
    Ok(output)
}

fn compare_ranked(left: &RankedPeopleCandidate, right: &RankedPeopleCandidate) -> Ordering {
    left.score
        .partial_cmp(&right.score)
        .unwrap_or(Ordering::Equal)
        .then_with(|| left.generated_at.cmp(&right.generated_at))
        .then_with(|| left.candidate_user_id.cmp(&right.candidate_user_id))
}

fn compare_ranked_desc(left: &RankedPeopleCandidate, right: &RankedPeopleCandidate) -> Ordering {
    compare_ranked(right, left)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeopleSnapshotCursor {
    pub snapshot_id: Uuid,
    pub offset: u32,
}

impl PeopleSnapshotCursor {
    pub fn encode(&self) -> String {
        Base64UrlUnpadded::encode_string(
            format!(
                "{}|{}|{}",
                PEOPLE_CURSOR_VERSION, self.snapshot_id, self.offset
            )
            .as_bytes(),
        )
    }

    pub fn parse(value: &str) -> Result<Option<Self>, RecommendationContractError> {
        if value.len() > MAX_RECOMMENDATION_CURSOR_LENGTH {
            return Err(RecommendationContractError::new(
                "Invalid people recommendation cursor",
            ));
        }
        let value = value.trim();
        if value.is_empty() {
            return Ok(None);
        }
        let decoded = Base64UrlUnpadded::decode_vec(value).map_err(|_| {
            RecommendationContractError::new("Invalid people recommendation cursor")
        })?;
        let decoded = std::str::from_utf8(&decoded).map_err(|_| {
            RecommendationContractError::new("Invalid people recommendation cursor")
        })?;
        let mut parts = decoded.split('|');
        let version = parts
            .next()
            .and_then(|part| part.parse::<u8>().ok())
            .filter(|version| *version == PEOPLE_CURSOR_VERSION)
            .ok_or_else(|| {
                RecommendationContractError::new("Invalid people recommendation cursor")
            })?;
        let snapshot_id = parts
            .next()
            .and_then(|part| Uuid::parse_str(part).ok())
            .filter(|id| !id.is_nil())
            .ok_or_else(|| {
                RecommendationContractError::new("Invalid people recommendation cursor")
            })?;
        let offset = parts
            .next()
            .and_then(|part| part.parse::<u32>().ok())
            .ok_or_else(|| {
                RecommendationContractError::new("Invalid people recommendation cursor")
            })?;
        if parts.next().is_some() || version != PEOPLE_CURSOR_VERSION {
            return Err(RecommendationContractError::new(
                "Invalid people recommendation cursor",
            ));
        }
        let cursor = Self {
            snapshot_id,
            offset,
        };
        if cursor.encode() != value {
            return Err(RecommendationContractError::new(
                "Invalid people recommendation cursor",
            ));
        }
        Ok(Some(cursor))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileDiscoverySnapshot {
    pub user_id: Uuid,
    pub username: String,
    pub nickname: String,
    pub bio: String,
    pub avatar_url: String,
    pub account_visibility: String,
    pub lifecycle_status: String,
}

impl ProfileDiscoverySnapshot {
    pub fn is_discoverable(&self) -> bool {
        !self.user_id.is_nil()
            && self.account_visibility.eq_ignore_ascii_case("PUBLIC")
            && self.lifecycle_status.eq_ignore_ascii_case("ACTIVE")
    }

    pub fn overlap_score(&self, other: &Self) -> f64 {
        let left = profile_tokens(self);
        let right = profile_tokens(other);
        if left.is_empty() || right.is_empty() {
            return 0.0;
        }
        let intersection = left.intersection(&right).count() as f64;
        let union = left.union(&right).count() as f64;
        if union == 0.0 {
            0.0
        } else {
            (intersection / union).clamp(0.0, 1.0)
        }
    }

    pub fn quality_score(&self) -> f64 {
        let complete = [
            !self.username.trim().is_empty(),
            !self.nickname.trim().is_empty(),
            !self.bio.trim().is_empty(),
            !self.avatar_url.trim().is_empty(),
        ]
        .into_iter()
        .filter(|value| *value)
        .count();
        complete as f64 / 4.0
    }
}

fn profile_tokens(profile: &ProfileDiscoverySnapshot) -> HashSet<String> {
    [&profile.username, &profile.nickname, &profile.bio]
        .into_iter()
        .flat_map(|value| value.split(|character: char| !character.is_alphanumeric()))
        .map(|token| token.trim().to_ascii_lowercase())
        .filter(|token| token.len() >= 2)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeopleSnapshot {
    pub id: Uuid,
    pub viewer_id: Uuid,
    pub rank_version: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeopleSnapshotItem {
    pub snapshot_id: Uuid,
    pub candidate_user_id: Uuid,
    pub source: PeopleCandidateSource,
    pub reason_code: String,
    pub score_micros: i64,
    pub rank_version: String,
    pub feature_snapshot_json: String,
    pub mutual_cluster: Option<Uuid>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeopleRecommendationImpression {
    pub impression_id: Uuid,
    pub snapshot_id: Uuid,
    pub viewer_id: Uuid,
    pub candidate_user_id: Uuid,
    pub source: PeopleCandidateSource,
    pub reason_code: String,
    pub position: i32,
    pub rank_version: String,
    pub score_micros: i64,
    pub request_id: String,
    pub session_id: Option<Uuid>,
    pub occurred_at: DateTime<Utc>,
    pub propensity_micros: u32,
}

pub fn recommendation_cursor_from_item(item: &PeopleSnapshotItem) -> RecommendationCursor {
    RecommendationCursor {
        version: PEOPLE_CURSOR_VERSION,
        snapshot_id: item.snapshot_id,
        score_micros: item.score_micros,
        occurred_at: item.generated_at,
        tie_breaker_id: item.candidate_user_id,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PeopleRecommendationRetentionResult {
    pub candidates: i64,
    pub snapshot_items: i64,
    pub impressions: i64,
    pub snapshots: i64,
    pub events: i64,
    pub experiment_assignments: i64,
    pub embeddings: i64,
    pub refresh_jobs: i64,
}

impl PeopleRecommendationRetentionResult {
    pub fn total(self) -> i64 {
        self.candidates
            + self.snapshot_items
            + self.impressions
            + self.snapshots
            + self.events
            + self.experiment_assignments
            + self.embeddings
            + self.refresh_jobs
    }
}
