use chrono::{DateTime, Utc};
use gateway_domain::moderation::{
    ContentModerationDecisionCursor, ContentModerationDecisionOutboxEntry, ModerationDecision,
    ModerationReport,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationMutationContextDto {
    pub operation_id: Uuid,
    pub actor_id: Uuid,
    pub idempotency_key: Option<String>,
    pub lookup_only: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateReportRequest {
    pub target_type: String,
    pub target_id: String,
    pub category: String,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateModerationDecisionRequest {
    pub decision_type: String,
    #[serde(default)]
    pub note: String,
    pub restriction_expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplayContentDecisionRequest {
    pub resolution_id: Uuid,
    pub reason: String,
    pub evidence_reference: String,
}

impl ReplayContentDecisionRequest {
    pub fn validate(&self) -> Result<(), &'static str> {
        self.validate_input().map_err(|error| error.message())
    }

    pub fn validate_input(&self) -> Result<(), crate::moderation_input::ModerationInputError> {
        use crate::moderation_input::ModerationInputError;
        let reason = self.reason.trim();
        if !(20..=1024).contains(&reason.chars().count())
            || reason.len() > 1024
            || reason.chars().any(char::is_control)
        {
            return Err(ModerationInputError::ReplayReason);
        }

        let reference = self.evidence_reference.trim();
        if !(3..=256).contains(&reference.len())
            || !reference.is_ascii()
            || !reference
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._:/#-".contains(&byte))
            || !reference
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
        {
            return Err(ModerationInputError::ReplayEvidence);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModerationReportResponse {
    pub id: String,
    pub reporter_id: String,
    pub target_type: String,
    pub target_id: String,
    pub category: String,
    pub reason: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

impl From<ModerationReport> for ModerationReportResponse {
    fn from(value: ModerationReport) -> Self {
        Self {
            id: value.id.to_string(),
            reporter_id: value.reporter_id.to_string(),
            target_type: value.target_type.as_str().to_owned(),
            target_id: value.target_id.to_string(),
            category: value.category.as_str().to_owned(),
            reason: value.reason,
            status: value.status.as_str().to_owned(),
            created_at: value.created_at,
            resolved_at: value.resolved_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModerationDecisionRecordResponse {
    pub id: String,
    pub report_id: String,
    pub actor_id: String,
    pub decision_type: String,
    pub note: String,
    pub created_at: DateTime<Utc>,
}

impl From<ModerationDecision> for ModerationDecisionRecordResponse {
    fn from(value: ModerationDecision) -> Self {
        Self {
            id: value.id.to_string(),
            report_id: value.report_id.to_string(),
            actor_id: value.actor_id.to_string(),
            decision_type: value.decision_type.as_str().to_owned(),
            note: value.note,
            created_at: value.created_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModerationDecisionResponse {
    pub report: ModerationReportResponse,
    pub decision: ModerationDecisionRecordResponse,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModerationReportListResponse {
    pub reports: Vec<ModerationReportResponse>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModerationContentDecisionStatusResponse {
    pub report_id: String,
    pub target_type: String,
    pub target_id: String,
    pub decision_type: String,
    pub state: String,
    pub attempt_count: u32,
    pub replay_count: u32,
    pub last_error_code: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub next_attempt_at: DateTime<Utc>,
}

impl From<ContentModerationDecisionOutboxEntry> for ModerationContentDecisionStatusResponse {
    fn from(value: ContentModerationDecisionOutboxEntry) -> Self {
        Self {
            report_id: value.report_id.to_string(),
            target_type: value.target_type.as_str().to_owned(),
            target_id: value.target_id.to_string(),
            decision_type: value.decision_type.as_str().to_owned(),
            state: value.state.as_str().to_owned(),
            attempt_count: value.attempt_count,
            replay_count: value.replay_count,
            last_error_code: value.last_error_code,
            created_at: value.created_at,
            updated_at: value.updated_at,
            next_attempt_at: value.next_attempt_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModerationContentDecisionCursorResponse {
    pub created_at: DateTime<Utc>,
    pub report_id: String,
}

impl From<ContentModerationDecisionCursor> for ModerationContentDecisionCursorResponse {
    fn from(value: ContentModerationDecisionCursor) -> Self {
        Self {
            created_at: value.created_at,
            report_id: value.report_id.to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModerationContentDecisionListResponse {
    pub decisions: Vec<ModerationContentDecisionStatusResponse>,
    pub next_cursor: Option<ModerationContentDecisionCursorResponse>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContentDecisionReplayAcceptedResponse {
    pub report_id: String,
    pub resolution_id: String,
    pub accepted: bool,
}

impl From<super::moderation_repository_dto::ModerationReportDto> for ModerationReportResponse {
    fn from(value: super::moderation_repository_dto::ModerationReportDto) -> Self {
        ModerationReport::from(value).into()
    }
}

impl From<super::moderation_repository_dto::ModerationDecisionDto>
    for ModerationDecisionRecordResponse
{
    fn from(value: super::moderation_repository_dto::ModerationDecisionDto) -> Self {
        ModerationDecision::from(value).into()
    }
}

impl From<super::moderation_repository_dto::ContentModerationDecisionOutboxEntryDto>
    for ModerationContentDecisionStatusResponse
{
    fn from(
        value: super::moderation_repository_dto::ContentModerationDecisionOutboxEntryDto,
    ) -> Self {
        ContentModerationDecisionOutboxEntry::from(value).into()
    }
}

impl From<super::moderation_repository_dto::ContentModerationDecisionCursorDto>
    for ModerationContentDecisionCursorResponse
{
    fn from(value: super::moderation_repository_dto::ContentModerationDecisionCursorDto) -> Self {
        ContentModerationDecisionCursor::from(value).into()
    }
}
