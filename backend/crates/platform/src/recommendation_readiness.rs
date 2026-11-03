use crate::model_manifest::ModelManifest;

pub const BASIS_POINTS_MAX: u16 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadinessThresholds {
    pub minimum_observation_days: u16,
    pub minimum_impressions: u64,
    pub minimum_positive_outcomes: u64,
    pub minimum_negative_outcomes: u64,
    pub minimum_distinct_viewers: u64,
    pub minimum_source_coverage_basis_points: u16,
    pub minimum_position_coverage_basis_points: u16,
    pub maximum_event_loss_basis_points: u16,
    pub minimum_snapshot_replay_basis_points: u16,
    pub minimum_shadow_requests: u64,
    pub maximum_shadow_error_basis_points: u16,
    pub maximum_shadow_p95_latency_ms: u32,
    pub maximum_shadow_policy_difference_basis_points: u16,
    pub minimum_shadow_top_k_overlap_basis_points: u16,
}

impl Default for ReadinessThresholds {
    fn default() -> Self {
        Self {
            minimum_observation_days: 28,
            minimum_impressions: 100_000,
            minimum_positive_outcomes: 5_000,
            minimum_negative_outcomes: 500,
            minimum_distinct_viewers: 10_000,
            minimum_source_coverage_basis_points: 9_000,
            minimum_position_coverage_basis_points: 9_500,
            maximum_event_loss_basis_points: 10,
            minimum_snapshot_replay_basis_points: 9_900,
            minimum_shadow_requests: 10_000,
            maximum_shadow_error_basis_points: 100,
            maximum_shadow_p95_latency_ms: 50,
            maximum_shadow_policy_difference_basis_points: 0,
            minimum_shadow_top_k_overlap_basis_points: 6_000,
        }
    }
}

impl ReadinessThresholds {
    pub fn validate(self) -> Result<Self, &'static str> {
        for value in [
            self.minimum_source_coverage_basis_points,
            self.minimum_position_coverage_basis_points,
            self.maximum_event_loss_basis_points,
            self.minimum_snapshot_replay_basis_points,
            self.maximum_shadow_error_basis_points,
            self.maximum_shadow_policy_difference_basis_points,
            self.minimum_shadow_top_k_overlap_basis_points,
        ] {
            if value > BASIS_POINTS_MAX {
                return Err("Readiness basis-point threshold is out of range");
            }
        }
        if self.minimum_observation_days == 0
            || self.minimum_impressions == 0
            || self.minimum_distinct_viewers == 0
            || self.minimum_shadow_requests == 0
            || self.maximum_shadow_p95_latency_ms == 0
        {
            return Err("Readiness threshold must be positive");
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DataReadinessSnapshot {
    pub observation_days: u16,
    pub impressions: u64,
    pub positive_outcomes: u64,
    pub negative_outcomes: u64,
    pub distinct_viewers: u64,
    pub source_coverage_basis_points: u16,
    pub position_coverage_basis_points: u16,
    pub event_loss_basis_points: u16,
    pub snapshot_replay_basis_points: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShadowReadinessSnapshot {
    pub requests: u64,
    pub error_basis_points: u16,
    pub p95_latency_ms: u32,
    pub policy_difference_basis_points: u16,
    pub top_k_overlap_basis_points: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelActivationReport {
    pub ready: bool,
    pub blockers: Vec<String>,
}

impl ModelActivationReport {
    fn from_blockers(blockers: Vec<String>) -> Self {
        Self {
            ready: blockers.is_empty(),
            blockers,
        }
    }
}

pub struct ModelActivationGate;

impl ModelActivationGate {
    pub fn evaluate(
        manifest: &ModelManifest,
        production: bool,
        fallback_ready: bool,
        data: &DataReadinessSnapshot,
        shadow: &ShadowReadinessSnapshot,
        thresholds: &ReadinessThresholds,
    ) -> ModelActivationReport {
        let mut blockers = Vec::new();
        if thresholds.validate().is_err() {
            blockers.push("INVALID_THRESHOLDS".to_owned());
            return ModelActivationReport::from_blockers(blockers);
        }
        if manifest.validate(production).is_err() || !manifest.status.can_serve() {
            blockers.push("MODEL_MANIFEST".to_owned());
        }
        if !fallback_ready {
            blockers.push("FALLBACK_NOT_READY".to_owned());
        }
        if data.observation_days < thresholds.minimum_observation_days {
            blockers.push("OBSERVATION_WINDOW".to_owned());
        }
        if data.impressions < thresholds.minimum_impressions {
            blockers.push("IMPRESSION_VOLUME".to_owned());
        }
        if data.positive_outcomes < thresholds.minimum_positive_outcomes {
            blockers.push("POSITIVE_OUTCOME_VOLUME".to_owned());
        }
        if data.negative_outcomes < thresholds.minimum_negative_outcomes {
            blockers.push("NEGATIVE_OUTCOME_VOLUME".to_owned());
        }
        if data.distinct_viewers < thresholds.minimum_distinct_viewers {
            blockers.push("VIEWER_COVERAGE".to_owned());
        }
        if data.source_coverage_basis_points < thresholds.minimum_source_coverage_basis_points {
            blockers.push("SOURCE_COVERAGE".to_owned());
        }
        if data.position_coverage_basis_points < thresholds.minimum_position_coverage_basis_points {
            blockers.push("POSITION_COVERAGE".to_owned());
        }
        if data.event_loss_basis_points > thresholds.maximum_event_loss_basis_points {
            blockers.push("EVENT_LOSS".to_owned());
        }
        if data.snapshot_replay_basis_points < thresholds.minimum_snapshot_replay_basis_points {
            blockers.push("SNAPSHOT_REPLAY".to_owned());
        }
        if shadow.requests < thresholds.minimum_shadow_requests {
            blockers.push("SHADOW_REQUEST_VOLUME".to_owned());
        }
        if shadow.error_basis_points > thresholds.maximum_shadow_error_basis_points {
            blockers.push("SHADOW_ERROR_RATE".to_owned());
        }
        if shadow.p95_latency_ms > thresholds.maximum_shadow_p95_latency_ms {
            blockers.push("SHADOW_LATENCY".to_owned());
        }
        if shadow.policy_difference_basis_points
            > thresholds.maximum_shadow_policy_difference_basis_points
        {
            blockers.push("SHADOW_POLICY_DIFFERENCE".to_owned());
        }
        if shadow.top_k_overlap_basis_points < thresholds.minimum_shadow_top_k_overlap_basis_points
        {
            blockers.push("SHADOW_TOP_K_OVERLAP".to_owned());
        }
        ModelActivationReport::from_blockers(blockers)
    }
}
