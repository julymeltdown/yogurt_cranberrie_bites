//! Settings are explicit operator input. Feature gates must be consumed server-side.
use crate::commands::{CanonicalValue, CommandError};
use concert_domain::{Event, EventPhase, Revision};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Features {
    pub parties: bool,
    pub giveaways: bool,
    pub intros: bool,
    pub social: bool,
    pub mural: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    Parties,
    Giveaways,
    Intros,
    Social,
    Mural,
}
impl Features {
    pub fn enabled(self, f: Feature) -> bool {
        match f {
            Feature::Parties => self.parties,
            Feature::Giveaways => self.giveaways,
            Feature::Intros => self.intros,
            Feature::Social => self.social,
            Feature::Mural => self.mural,
        }
    }
    pub fn canonical(self) -> CanonicalValue {
        CanonicalValue::Object(vec![
            ("parties".into(), CanonicalValue::Bool(self.parties)),
            ("giveaways".into(), CanonicalValue::Bool(self.giveaways)),
            ("intros".into(), CanonicalValue::Bool(self.intros)),
            ("social".into(), CanonicalValue::Bool(self.social)),
            ("mural".into(), CanonicalValue::Bool(self.mural)),
        ])
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub features: Features,
    pub beacon_ttl_seconds: i64,
    pub pickup_grace_seconds: i64,
    pub social_grant_ttl_seconds: i64,
    pub mural_cooldown_ms: i64,
}
impl Settings {
    pub fn validate(self) -> Result<Self, CommandError> {
        if !(1..=900).contains(&self.beacon_ttl_seconds)
            || !(0..=86400).contains(&self.pickup_grace_seconds)
            || !(1..=86400).contains(&self.social_grant_ttl_seconds)
            || !(0..=3600000).contains(&self.mural_cooldown_ms)
        {
            Err(CommandError::InvalidInput)
        } else {
            Ok(self)
        }
    }
    pub fn canonical(self) -> Vec<(String, CanonicalValue)> {
        vec![
            ("features".into(), self.features.canonical()),
            (
                "beacon_ttl_seconds".into(),
                CanonicalValue::Integer(self.beacon_ttl_seconds),
            ),
            (
                "pickup_grace_seconds".into(),
                CanonicalValue::Integer(self.pickup_grace_seconds),
            ),
            (
                "social_grant_ttl_seconds".into(),
                CanonicalValue::Integer(self.social_grant_ttl_seconds),
            ),
            (
                "mural_cooldown_ms".into(),
                CanonicalValue::Integer(self.mural_cooldown_ms),
            ),
        ]
    }
}
#[derive(Clone, Debug)]
pub struct SettingsSnapshot {
    pub event: Event,
    pub settings: Settings,
}
pub fn reason(value: &str) -> Result<(), CommandError> {
    if value.trim().is_empty() || value.chars().count() > 500 || value.chars().any(char::is_control)
    {
        Err(CommandError::InvalidInput)
    } else {
        Ok(())
    }
}
pub fn phase_transition(current: EventPhase, next: EventPhase) -> bool {
    matches!(
        (current, next),
        (EventPhase::Draft, EventPhase::Open) | (EventPhase::Open, EventPhase::Live)
    )
}
pub fn revision(value: i64) -> Result<Revision, CommandError> {
    Revision::new(value).map_err(|_| CommandError::InvalidInput)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn phase_is_bounded() {
        assert!(phase_transition(EventPhase::Draft, EventPhase::Open));
        assert!(phase_transition(EventPhase::Open, EventPhase::Live));
        assert!(!phase_transition(EventPhase::Live, EventPhase::ReadOnly));
        assert!(!phase_transition(EventPhase::Open, EventPhase::Open));
    }
}
