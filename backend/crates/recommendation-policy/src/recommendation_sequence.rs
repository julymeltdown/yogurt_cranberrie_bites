use crate::recommendation::{
    RecommendationAction, RecommendationContractError, RecommendationEvent, RecommendationSurface,
};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashSet;
use uuid::Uuid;

pub const MAX_SEQUENCE_ACTIONS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecentSequencePolicy {
    pub max_actions: usize,
    pub max_age: Duration,
    pub future_tolerance: Duration,
}

impl Default for RecentSequencePolicy {
    fn default() -> Self {
        Self {
            max_actions: MAX_SEQUENCE_ACTIONS,
            max_age: Duration::hours(24),
            future_tolerance: Duration::minutes(5),
        }
    }
}

impl RecentSequencePolicy {
    fn validate(self) -> Result<Self, RecommendationContractError> {
        if self.max_actions == 0 || self.max_actions > MAX_SEQUENCE_ACTIONS {
            return Err(RecommendationContractError::new(
                "Recent sequence action limit is out of range",
            ));
        }
        if self.max_age <= Duration::zero() || self.max_age > Duration::days(30) {
            return Err(RecommendationContractError::new(
                "Recent sequence maximum age is out of range",
            ));
        }
        if self.future_tolerance < Duration::zero() || self.future_tolerance > Duration::minutes(5)
        {
            return Err(RecommendationContractError::new(
                "Recent sequence future tolerance is out of range",
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentActionFeature {
    pub event_id: Uuid,
    pub subject_id: Uuid,
    pub action: RecommendationAction,
    pub source: String,
    pub age_seconds: u32,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecentSequence {
    pub actions: Vec<RecentActionFeature>,
}

impl RecentSequence {
    pub fn build(
        viewer_id: Uuid,
        surface: RecommendationSurface,
        now: DateTime<Utc>,
        mut events: Vec<RecommendationEvent>,
        policy: RecentSequencePolicy,
    ) -> Result<Self, RecommendationContractError> {
        let policy = policy.validate()?;
        if viewer_id.is_nil() {
            return Err(RecommendationContractError::new(
                "Recent sequence viewer ID must be non-nil",
            ));
        }
        for event in &events {
            event.validate()?;
        }
        events.retain(|event| {
            event.viewer_id == viewer_id
                && event.surface == surface
                && event.occurred_at >= now - policy.max_age
                && event.occurred_at <= now + policy.future_tolerance
        });
        events.sort_by(|left, right| {
            right
                .occurred_at
                .cmp(&left.occurred_at)
                .then_with(|| right.event_id.cmp(&left.event_id))
        });

        let mut seen = HashSet::new();
        let mut actions = Vec::with_capacity(events.len().min(policy.max_actions));
        for event in events {
            if !seen.insert(event.event_id) {
                continue;
            }
            let age_seconds = (now - event.occurred_at)
                .num_seconds()
                .max(0)
                .min(i64::from(u32::MAX)) as u32;
            actions.push(RecentActionFeature {
                event_id: event.event_id,
                subject_id: event.subject_id,
                action: event.action,
                source: event.source,
                age_seconds,
                occurred_at: event.occurred_at,
            });
            if actions.len() >= policy.max_actions {
                break;
            }
        }
        Ok(Self { actions })
    }

    pub fn negative_penalty(&self, subject_id: Uuid) -> f64 {
        self.actions
            .iter()
            .filter(|action| action.subject_id == subject_id)
            .map(|action| match action.action {
                RecommendationAction::Hide
                | RecommendationAction::NotInterested
                | RecommendationAction::Block
                | RecommendationAction::Mute
                | RecommendationAction::Report => 1.0,
                RecommendationAction::FastScroll => 0.5,
                _ => 0.0,
            })
            .fold(0.0_f64, f64::max)
    }

    pub fn repeat_penalty(&self, subject_id: Uuid) -> f64 {
        self.actions
            .iter()
            .filter(|action| action.subject_id == subject_id)
            .map(|action| match action.action {
                RecommendationAction::Impression => 0.25,
                RecommendationAction::Click | RecommendationAction::Dwell => 0.10,
                _ => 0.0,
            })
            .sum::<f64>()
            .min(1.0)
    }
}
