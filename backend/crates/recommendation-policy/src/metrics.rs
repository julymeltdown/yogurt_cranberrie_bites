#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecommendationMetricSurface {
    People,
    HomeFeed,
}

impl RecommendationMetricSurface {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::People => "people",
            Self::HomeFeed => "home_feed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecommendationMetricOutcome {
    Success,
    Failure,
    Fallback,
    Dropped,
}

impl RecommendationMetricOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Fallback => "fallback",
            Self::Dropped => "dropped",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecommendationMetricSource {
    MutualGraph,
    FollowerOverlap,
    Exploration,
    Timeline,
    SelfHistory,
    HighFanout,
    SocialProof,
    AuthorAffinity,
    Trending,
    Sequence,
}

impl RecommendationMetricSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MutualGraph => "mutual_graph",
            Self::FollowerOverlap => "follower_overlap",
            Self::Exploration => "exploration",
            Self::Timeline => "timeline",
            Self::SelfHistory => "self_history",
            Self::HighFanout => "high_fanout",
            Self::SocialProof => "social_proof",
            Self::AuthorAffinity => "author_affinity",
            Self::Trending => "trending",
            Self::Sequence => "sequence",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecommendationMetricReason {
    ModelUnavailable,
    CandidateSourceFailed,
    EmptyCandidates,
    PolicyFiltered,
    SequenceUnavailable,
    SnapshotUnavailable,
}

impl RecommendationMetricReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ModelUnavailable => "model_unavailable",
            Self::CandidateSourceFailed => "candidate_source_failed",
            Self::EmptyCandidates => "empty_candidates",
            Self::PolicyFiltered => "policy_filtered",
            Self::SequenceUnavailable => "sequence_unavailable",
            Self::SnapshotUnavailable => "snapshot_unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecommendationMetricStage {
    Eligibility,
    CandidateCollection,
    PolicyFilter,
    Ranking,
    Reranking,
    Hydration,
    Snapshot,
    EventIngest,
    Retention,
}

impl RecommendationMetricStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Eligibility => "eligibility",
            Self::CandidateCollection => "candidate_collection",
            Self::PolicyFilter => "policy_filter",
            Self::Ranking => "ranking",
            Self::Reranking => "reranking",
            Self::Hydration => "hydration",
            Self::Snapshot => "snapshot",
            Self::EventIngest => "event_ingest",
            Self::Retention => "retention",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecommendationEntity {
    Candidate,
    Snapshot,
    SnapshotItem,
    Event,
    Impression,
    TimelineEntry,
    FanoutJob,
    ExperimentAssignment,
    Embedding,
    RefreshJob,
}

impl RecommendationEntity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Snapshot => "snapshot",
            Self::SnapshotItem => "snapshot_item",
            Self::Event => "event",
            Self::Impression => "impression",
            Self::TimelineEntry => "timeline_entry",
            Self::FanoutJob => "fanout_job",
            Self::ExperimentAssignment => "experiment_assignment",
            Self::Embedding => "embedding",
            Self::RefreshJob => "refresh_job",
        }
    }
}

/// Diagnostic capability. Implementations own collection/export; policy emits
/// only a finite vocabulary and bounded numeric observations.
pub trait RecommendationMetricsPort: Send + Sync {
    fn record_request(
        &self,
        surface: RecommendationMetricSurface,
        outcome: RecommendationMetricOutcome,
    );
    fn record_source_run(
        &self,
        surface: RecommendationMetricSurface,
        source: RecommendationMetricSource,
        outcome: RecommendationMetricOutcome,
    );
    fn record_candidates(
        &self,
        surface: RecommendationMetricSurface,
        source: RecommendationMetricSource,
        outcome: RecommendationMetricOutcome,
        count: i64,
    );
    fn record_fallback(
        &self,
        surface: RecommendationMetricSurface,
        reason: RecommendationMetricReason,
    );
    fn observe_stage_latency(
        &self,
        surface: RecommendationMetricSurface,
        stage: RecommendationMetricStage,
        elapsed_ms: u64,
    );
}

/// Unconfigured callers have no published metrics endpoint. Production
/// composition injects its recorder through with_metrics.
#[derive(Default)]
pub struct NoopRecommendationMetrics;

impl RecommendationMetricsPort for NoopRecommendationMetrics {
    fn record_request(
        &self,
        _surface: RecommendationMetricSurface,
        _outcome: RecommendationMetricOutcome,
    ) {
    }
    fn record_source_run(
        &self,
        _surface: RecommendationMetricSurface,
        _source: RecommendationMetricSource,
        _outcome: RecommendationMetricOutcome,
    ) {
    }
    fn record_candidates(
        &self,
        _surface: RecommendationMetricSurface,
        _source: RecommendationMetricSource,
        _outcome: RecommendationMetricOutcome,
        _count: i64,
    ) {
    }
    fn record_fallback(
        &self,
        _surface: RecommendationMetricSurface,
        _reason: RecommendationMetricReason,
    ) {
    }
    fn observe_stage_latency(
        &self,
        _surface: RecommendationMetricSurface,
        _stage: RecommendationMetricStage,
        _elapsed_ms: u64,
    ) {
    }
}
