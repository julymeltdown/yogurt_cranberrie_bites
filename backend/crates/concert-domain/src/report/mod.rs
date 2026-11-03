//! Typed private report input and deliberately separate safe/status projections.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! closed_enum {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name { $(#[serde(rename=$wire)] $variant),+ }
        impl $name {
            pub fn as_str(self) -> &'static str { match self { $(Self::$variant => $wire),+ } }
            pub fn parse(s: &str) -> Result<Self, ReportInputError> { match s { $($wire => Ok(Self::$variant),)+ _ => Err(ReportInputError) } }
        }
    }
}
closed_enum!(TargetType { User=>"USER", Post=>"POST", Comment=>"COMMENT", Party=>"PARTY", Giveaway=>"GIVEAWAY", Mural=>"MURAL" });
closed_enum!(Reason { Harassment=>"HARASSMENT", Privacy=>"PRIVACY", UnsafeMeeting=>"UNSAFE_MEETING", Spam=>"SPAM", Other=>"OTHER" });
closed_enum!(ReportState { Open=>"OPEN", InReview=>"IN_REVIEW", Resolved=>"RESOLVED", Dismissed=>"DISMISSED" });
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid report input")]
pub struct ReportInputError;

/// Private content deliberately has no Debug implementation.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateReport {
    pub target_type: TargetType,
    pub target_id: Uuid,
    pub reason: Reason,
    pub text: String,
    pub evidence_media_ids: Vec<Uuid>,
}
impl CreateReport {
    pub fn validate(&self) -> Result<(), ReportInputError> {
        if self.target_id.is_nil()
            || self.text.chars().count() > 2000
            || self
                .text
                .chars()
                .any(|v| v.is_control() && !matches!(v, '\n' | '\t' | '\r'))
            || self.evidence_media_ids.len() > 4
            || self.evidence_media_ids.iter().any(Uuid::is_nil)
            || self
                .evidence_media_ids
                .iter()
                .enumerate()
                .any(|(i, v)| self.evidence_media_ids[..i].contains(v))
        {
            return Err(ReportInputError);
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Report {
    pub report_id: Uuid,
    pub event_id: Uuid,
    pub target_type: TargetType,
    pub target_id: Uuid,
    pub state: ReportState,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
}
/// Constructed only after current event-scoped MODERATOR/OPERATOR authorization.
#[derive(Clone)]
pub struct ReportDetail {
    pub report: Report,
    pub reporter_id: Uuid,
    pub reason: Reason,
    pub text: String,
    pub evidence_media_ids: Vec<Uuid>,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_closed_private_input() {
        let mut input = CreateReport {
            target_type: TargetType::Post,
            target_id: Uuid::new_v4(),
            reason: Reason::Spam,
            text: "x".repeat(2000),
            evidence_media_ids: vec![],
        };
        assert!(input.validate().is_ok());
        input.text.push('x');
        assert!(input.validate().is_err());
        input.text.clear();
        let id = Uuid::new_v4();
        input.evidence_media_ids = vec![id, id];
        assert!(input.validate().is_err());
        assert!(serde_json::from_str::<CreateReport>(r#"{"target_type":"POST","target_id":"00000000-0000-0000-0000-000000000001","reason":"SPAM","text":"","evidence_media_ids":[],"reporter_id":"spoof"}"#).is_err());
    }
    #[test]
    fn public_report_has_no_private_fields() {
        let r = Report {
            report_id: Uuid::new_v4(),
            event_id: Uuid::new_v4(),
            target_type: TargetType::User,
            target_id: Uuid::new_v4(),
            state: ReportState::Open,
            revision: 1,
            created_at: Utc::now(),
        };
        let value = serde_json::to_value(r).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 7);
        for key in ["reporter_id", "reason", "text", "evidence_media_ids"] {
            assert!(value.get(key).is_none());
        }
    }
}
