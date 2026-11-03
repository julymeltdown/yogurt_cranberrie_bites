use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const CREATE_REPORT_OWNER_SCOPE: &str = "gateway.v1.CreateModerationReport";
pub const MAX_CONTENT_DECISION_ATTEMPTS: u32 = 8;

#[derive(Debug, thiserror::Error)]
pub enum ReportMutationReceiptError {
    #[error("moderation report mutation receipt is missing")]
    Missing,
    #[error("moderation report mutation receipt is expired")]
    Expired,
    #[error("moderation report mutation receipt conflicts with the operation")]
    Conflict,
}

#[derive(Debug, thiserror::Error)]
#[error("moderation target is no longer available")]
pub struct ModerationTargetUnavailable;

#[derive(Debug, thiserror::Error)]
#[error("moderation decision is already being processed")]
pub struct ModerationDecisionInProgress;

#[derive(Debug, thiserror::Error)]
#[error("moderation report has a conflicting staged decision")]
pub struct ModerationDecisionConflict;

#[derive(Debug, thiserror::Error)]
#[error("Both moderation decision cursor fields are required")]
pub struct InvalidContentModerationDecisionCursor;

#[derive(Debug, thiserror::Error)]
#[error("a non-empty moderation note is required to dismiss an unavailable target")]
pub struct ModerationResolutionNoteRequired;

#[derive(Debug, thiserror::Error)]
#[error("moderation content decision cannot be replayed in its current state")]
pub struct ContentDecisionReplayConflict;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReportTargetType {
    User,
    Post,
    Comment,
}

impl ReportTargetType {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "USER" => Ok(Self::User),
            "POST" => Ok(Self::Post),
            "COMMENT" => Ok(Self::Comment),
            _ => anyhow::bail!("Report target must be USER, POST, or COMMENT"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "USER",
            Self::Post => "POST",
            Self::Comment => "COMMENT",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReportCategory {
    Spam,
    Harassment,
    Hate,
    Violence,
    Other,
}

impl ReportCategory {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "SPAM" => Ok(Self::Spam),
            "HARASSMENT" => Ok(Self::Harassment),
            "HATE" => Ok(Self::Hate),
            "VIOLENCE" => Ok(Self::Violence),
            "OTHER" => Ok(Self::Other),
            _ => {
                anyhow::bail!("Report category must be SPAM, HARASSMENT, HATE, VIOLENCE, or OTHER")
            }
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Spam => "SPAM",
            Self::Harassment => "HARASSMENT",
            Self::Hate => "HATE",
            Self::Violence => "VIOLENCE",
            Self::Other => "OTHER",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReportStatus {
    Open,
    Resolved,
    Dismissed,
}

impl ReportStatus {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "OPEN" => Ok(Self::Open),
            "RESOLVED" => Ok(Self::Resolved),
            "DISMISSED" => Ok(Self::Dismissed),
            _ => anyhow::bail!("Invalid report status"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "OPEN",
            Self::Resolved => "RESOLVED",
            Self::Dismissed => "DISMISSED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ModerationDecisionType {
    Dismiss,
    HideContent,
    RestoreContent,
    RestrictUser,
    LiftRestriction,
}

impl ModerationDecisionType {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "DISMISS" => Ok(Self::Dismiss),
            "HIDE_CONTENT" => Ok(Self::HideContent),
            "RESTORE_CONTENT" => Ok(Self::RestoreContent),
            "RESTRICT_USER" => Ok(Self::RestrictUser),
            "LIFT_RESTRICTION" => Ok(Self::LiftRestriction),
            _ => anyhow::bail!(
                "Moderation decision must be DISMISS, HIDE_CONTENT, RESTORE_CONTENT, RESTRICT_USER, or LIFT_RESTRICTION"
            ),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dismiss => "DISMISS",
            Self::HideContent => "HIDE_CONTENT",
            Self::RestoreContent => "RESTORE_CONTENT",
            Self::RestrictUser => "RESTRICT_USER",
            Self::LiftRestriction => "LIFT_RESTRICTION",
        }
    }

    pub fn content_action(self) -> Option<&'static str> {
        match self {
            Self::HideContent => Some("HIDE"),
            Self::RestoreContent => Some("RESTORE"),
            _ => None,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModerationReport {
    pub id: Uuid,
    pub reporter_id: Uuid,
    pub target_type: ReportTargetType,
    pub target_id: Uuid,
    pub category: ReportCategory,
    pub reason: String,
    pub status: ReportStatus,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for ModerationReport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModerationReport")
            .field("id", &"[REDACTED]")
            .field("reporter_id", &"[REDACTED]")
            .field("target_type", &self.target_type)
            .field("target_id", &"[REDACTED]")
            .field("category", &self.category)
            .field("reason", &"[REDACTED]")
            .field("status", &self.status)
            .field("created_at", &self.created_at)
            .field("resolved_at", &self.resolved_at)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ModerationDecision {
    pub id: Uuid,
    pub report_id: Uuid,
    pub actor_id: Uuid,
    pub decision_type: ModerationDecisionType,
    pub note: String,
    pub created_at: DateTime<Utc>,
}

impl std::fmt::Debug for ModerationDecision {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModerationDecision")
            .field("id", &"[REDACTED]")
            .field("report_id", &"[REDACTED]")
            .field("actor_id", &"[REDACTED]")
            .field("decision_type", &self.decision_type)
            .field("note", &"[REDACTED]")
            .field("created_at", &self.created_at)
            .finish()
    }
}

/// Durable Gateway intent for a content moderation action that may need to be
/// replayed after PostService succeeds but Gateway persistence is unavailable.
#[derive(Clone)]
pub struct PendingContentModerationDecision {
    pub report: ModerationReport,
    pub actor_id: Uuid,
    pub decision_type: ModerationDecisionType,
    pub note: String,
    pub attempt_count: u32,
    pub lease_owner: Uuid,
}

impl std::fmt::Debug for PendingContentModerationDecision {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PendingContentModerationDecision")
            .field("report", &self.report)
            .field("actor_id", &"[REDACTED]")
            .field("decision_type", &self.decision_type)
            .field("note", &"[REDACTED]")
            .field("attempt_count", &self.attempt_count)
            .field("lease_owner", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod secret_debug_tests {
    use super::{
        ModerationDecision, ModerationDecisionType, ModerationReport,
        PendingContentModerationDecision, ReportCategory, ReportStatus, ReportTargetType,
    };
    use chrono::Utc;
    use uuid::Uuid;

    fn report(reason: &str) -> ModerationReport {
        ModerationReport {
            id: Uuid::new_v4(),
            reporter_id: Uuid::new_v4(),
            target_type: ReportTargetType::Post,
            target_id: Uuid::new_v4(),
            category: ReportCategory::Harassment,
            reason: reason.into(),
            status: ReportStatus::Open,
            created_at: Utc::now(),
            resolved_at: None,
        }
    }

    #[test]
    fn moderation_report_debug_redacts_reporter_target_and_freeform_reason() {
        let report = report("private@example.test alleges a sensitive incident");

        let rendered = format!("{report:?}");

        assert!(!rendered.contains("private@example.test"), "{rendered}");
        assert!(!rendered.contains("sensitive incident"), "{rendered}");
        assert!(rendered.contains("[REDACTED]"), "{rendered}");
    }

    #[test]
    fn moderation_decision_debug_redacts_actor_and_operator_note() {
        let decision = ModerationDecision {
            id: Uuid::new_v4(),
            report_id: Uuid::new_v4(),
            actor_id: Uuid::new_v4(),
            decision_type: ModerationDecisionType::HideContent,
            note: "private operator evidence".into(),
            created_at: Utc::now(),
        };

        let rendered = format!("{decision:?}");

        assert!(
            !rendered.contains("private operator evidence"),
            "{rendered}"
        );
        assert!(rendered.contains("[REDACTED]"), "{rendered}");
    }

    #[test]
    fn pending_decision_debug_redacts_nested_report_and_replay_metadata() {
        let pending = PendingContentModerationDecision {
            report: report("pending-private-report-reason"),
            actor_id: Uuid::new_v4(),
            decision_type: ModerationDecisionType::RestrictUser,
            note: "pending-private-review-note".into(),
            attempt_count: 3,
            lease_owner: Uuid::new_v4(),
        };

        let rendered = format!("{pending:?}");

        assert!(
            !rendered.contains("pending-private-report-reason"),
            "{rendered}"
        );
        assert!(
            !rendered.contains("pending-private-review-note"),
            "{rendered}"
        );
        assert!(rendered.contains("[REDACTED]"), "{rendered}");
    }
}

/// Aggregate health of the durable content moderation decision queue.
///
/// This deliberately contains no report identifiers so it can be exported as
/// bounded-cardinality service metrics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContentModerationDecisionBacklog {
    pub pending: i64,
    pub oldest_pending_age_seconds: i64,
    pub target_unavailable: i64,
    pub retry_exhausted: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentModerationDecisionOutboxState {
    Pending,
    TargetUnavailable,
    RetryExhausted,
}

impl ContentModerationDecisionOutboxState {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value {
            "PENDING" => Ok(Self::Pending),
            "TARGET_UNAVAILABLE" => Ok(Self::TargetUnavailable),
            "RETRY_EXHAUSTED" => Ok(Self::RetryExhausted),
            _ => anyhow::bail!("unsupported moderation outbox state"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::TargetUnavailable => "TARGET_UNAVAILABLE",
            Self::RetryExhausted => "RETRY_EXHAUSTED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentModerationDecisionOutboxEntry {
    pub report_id: Uuid,
    pub target_type: ReportTargetType,
    pub target_id: Uuid,
    pub decision_type: ModerationDecisionType,
    pub state: ContentModerationDecisionOutboxState,
    pub attempt_count: u32,
    pub replay_count: u32,
    pub last_error_code: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub next_attempt_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentModerationDecisionCursor {
    pub created_at: DateTime<Utc>,
    pub report_id: Uuid,
}

#[derive(Debug, Clone)]
pub enum ContentModerationDecisionStage {
    Pending(PendingContentModerationDecision),
    AlreadyRecorded {
        report: ModerationReport,
        decision: ModerationDecision,
    },
}
