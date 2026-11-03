use crate::application::ports::{
    PeopleRecommendationFallbackReason, PeopleRecommendationObserverPort,
    PeopleRecommendationRequestOutcome,
};
use crate::domain::recommendation::PeopleCandidateSource;
use platform::recommendation_metrics::{
    RecommendationMetricOutcome, RecommendationMetricReason, RecommendationMetricSource,
    RecommendationMetricStage, RecommendationMetricSurface, RecommendationMetrics,
};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct RecommendationMetricsObserver {
    metrics: Arc<RecommendationMetrics>,
}

impl RecommendationMetricsObserver {
    pub fn new(metrics: Arc<RecommendationMetrics>) -> Self {
        Self { metrics }
    }
}

impl PeopleRecommendationObserverPort for RecommendationMetricsObserver {
    fn record_request(&self, outcome: PeopleRecommendationRequestOutcome) {
        self.metrics.record_request(
            RecommendationMetricSurface::People,
            match outcome {
                PeopleRecommendationRequestOutcome::Success => RecommendationMetricOutcome::Success,
                PeopleRecommendationRequestOutcome::Failure => RecommendationMetricOutcome::Failure,
            },
        );
    }

    fn record_fallback(&self, reason: PeopleRecommendationFallbackReason) {
        self.metrics.record_fallback(
            RecommendationMetricSurface::People,
            match reason {
                PeopleRecommendationFallbackReason::ModelUnavailable => {
                    RecommendationMetricReason::ModelUnavailable
                }
                PeopleRecommendationFallbackReason::EmptyCandidates => {
                    RecommendationMetricReason::EmptyCandidates
                }
                PeopleRecommendationFallbackReason::SequenceUnavailable => {
                    RecommendationMetricReason::SequenceUnavailable
                }
            },
        );
    }

    fn record_candidates(&self, source: PeopleCandidateSource, count: i64) {
        self.metrics.record_candidates(
            RecommendationMetricSurface::People,
            metric_source(source),
            RecommendationMetricOutcome::Success,
            count,
        );
    }

    fn observe_snapshot_latency(&self, elapsed: Duration) {
        let millis = elapsed.as_millis().min(u128::from(u64::MAX)) as u64;
        self.metrics.observe_stage_latency(
            RecommendationMetricSurface::People,
            RecommendationMetricStage::Snapshot,
            millis,
        );
    }
}

fn metric_source(source: PeopleCandidateSource) -> RecommendationMetricSource {
    match source {
        PeopleCandidateSource::MutualFollow => RecommendationMetricSource::MutualGraph,
        PeopleCandidateSource::FollowerOverlap | PeopleCandidateSource::ProfileOverlap => {
            RecommendationMetricSource::FollowerOverlap
        }
        PeopleCandidateSource::CoEngagement => RecommendationMetricSource::SocialProof,
        PeopleCandidateSource::NewMember | PeopleCandidateSource::Exploration => {
            RecommendationMetricSource::Exploration
        }
    }
}
