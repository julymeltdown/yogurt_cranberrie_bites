//! Internal moderation persistence transfers. No implicit wire serialization.
use chrono::{DateTime, Utc};
use gateway_domain::moderation as domain;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReportTargetTypeDto {
    User,
    Post,
    Comment,
}

impl From<domain::ReportTargetType> for ReportTargetTypeDto {
    fn from(value: domain::ReportTargetType) -> Self {
        match value {
            domain::ReportTargetType::User => ReportTargetTypeDto::User,
            domain::ReportTargetType::Post => ReportTargetTypeDto::Post,
            domain::ReportTargetType::Comment => ReportTargetTypeDto::Comment,
        }
    }
}

impl From<ReportTargetTypeDto> for domain::ReportTargetType {
    fn from(value: ReportTargetTypeDto) -> Self {
        match value {
            ReportTargetTypeDto::User => domain::ReportTargetType::User,
            ReportTargetTypeDto::Post => domain::ReportTargetType::Post,
            ReportTargetTypeDto::Comment => domain::ReportTargetType::Comment,
        }
    }
}

impl ReportTargetTypeDto {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        domain::ReportTargetType::parse(value).map(Into::into)
    }

    pub fn as_str(self) -> &'static str {
        domain::ReportTargetType::from(self).as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReportCategoryDto {
    Spam,
    Harassment,
    Hate,
    Violence,
    Other,
}

impl From<domain::ReportCategory> for ReportCategoryDto {
    fn from(value: domain::ReportCategory) -> Self {
        match value {
            domain::ReportCategory::Spam => ReportCategoryDto::Spam,
            domain::ReportCategory::Harassment => ReportCategoryDto::Harassment,
            domain::ReportCategory::Hate => ReportCategoryDto::Hate,
            domain::ReportCategory::Violence => ReportCategoryDto::Violence,
            domain::ReportCategory::Other => ReportCategoryDto::Other,
        }
    }
}

impl From<ReportCategoryDto> for domain::ReportCategory {
    fn from(value: ReportCategoryDto) -> Self {
        match value {
            ReportCategoryDto::Spam => domain::ReportCategory::Spam,
            ReportCategoryDto::Harassment => domain::ReportCategory::Harassment,
            ReportCategoryDto::Hate => domain::ReportCategory::Hate,
            ReportCategoryDto::Violence => domain::ReportCategory::Violence,
            ReportCategoryDto::Other => domain::ReportCategory::Other,
        }
    }
}

impl ReportCategoryDto {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        domain::ReportCategory::parse(value).map(Into::into)
    }

    pub fn as_str(self) -> &'static str {
        domain::ReportCategory::from(self).as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReportStatusDto {
    Open,
    Resolved,
    Dismissed,
}

impl From<domain::ReportStatus> for ReportStatusDto {
    fn from(value: domain::ReportStatus) -> Self {
        match value {
            domain::ReportStatus::Open => ReportStatusDto::Open,
            domain::ReportStatus::Resolved => ReportStatusDto::Resolved,
            domain::ReportStatus::Dismissed => ReportStatusDto::Dismissed,
        }
    }
}

impl From<ReportStatusDto> for domain::ReportStatus {
    fn from(value: ReportStatusDto) -> Self {
        match value {
            ReportStatusDto::Open => domain::ReportStatus::Open,
            ReportStatusDto::Resolved => domain::ReportStatus::Resolved,
            ReportStatusDto::Dismissed => domain::ReportStatus::Dismissed,
        }
    }
}

impl ReportStatusDto {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        domain::ReportStatus::parse(value).map(Into::into)
    }

    pub fn as_str(self) -> &'static str {
        domain::ReportStatus::from(self).as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModerationDecisionTypeDto {
    Dismiss,
    HideContent,
    RestoreContent,
    RestrictUser,
    LiftRestriction,
}

impl From<domain::ModerationDecisionType> for ModerationDecisionTypeDto {
    fn from(value: domain::ModerationDecisionType) -> Self {
        match value {
            domain::ModerationDecisionType::Dismiss => ModerationDecisionTypeDto::Dismiss,
            domain::ModerationDecisionType::HideContent => ModerationDecisionTypeDto::HideContent,
            domain::ModerationDecisionType::RestoreContent => {
                ModerationDecisionTypeDto::RestoreContent
            }
            domain::ModerationDecisionType::RestrictUser => ModerationDecisionTypeDto::RestrictUser,
            domain::ModerationDecisionType::LiftRestriction => {
                ModerationDecisionTypeDto::LiftRestriction
            }
        }
    }
}

impl From<ModerationDecisionTypeDto> for domain::ModerationDecisionType {
    fn from(value: ModerationDecisionTypeDto) -> Self {
        match value {
            ModerationDecisionTypeDto::Dismiss => domain::ModerationDecisionType::Dismiss,
            ModerationDecisionTypeDto::HideContent => domain::ModerationDecisionType::HideContent,
            ModerationDecisionTypeDto::RestoreContent => {
                domain::ModerationDecisionType::RestoreContent
            }
            ModerationDecisionTypeDto::RestrictUser => domain::ModerationDecisionType::RestrictUser,
            ModerationDecisionTypeDto::LiftRestriction => {
                domain::ModerationDecisionType::LiftRestriction
            }
        }
    }
}

impl ModerationDecisionTypeDto {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        domain::ModerationDecisionType::parse(value).map(Into::into)
    }

    pub fn as_str(self) -> &'static str {
        domain::ModerationDecisionType::from(self).as_str()
    }

    pub fn content_action(self) -> Option<&'static str> {
        domain::ModerationDecisionType::from(self).content_action()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentModerationDecisionOutboxStateDto {
    Pending,
    TargetUnavailable,
    RetryExhausted,
}

impl From<domain::ContentModerationDecisionOutboxState>
    for ContentModerationDecisionOutboxStateDto
{
    fn from(value: domain::ContentModerationDecisionOutboxState) -> Self {
        match value {
            domain::ContentModerationDecisionOutboxState::Pending => {
                ContentModerationDecisionOutboxStateDto::Pending
            }
            domain::ContentModerationDecisionOutboxState::TargetUnavailable => {
                ContentModerationDecisionOutboxStateDto::TargetUnavailable
            }
            domain::ContentModerationDecisionOutboxState::RetryExhausted => {
                ContentModerationDecisionOutboxStateDto::RetryExhausted
            }
        }
    }
}

impl From<ContentModerationDecisionOutboxStateDto>
    for domain::ContentModerationDecisionOutboxState
{
    fn from(value: ContentModerationDecisionOutboxStateDto) -> Self {
        match value {
            ContentModerationDecisionOutboxStateDto::Pending => {
                domain::ContentModerationDecisionOutboxState::Pending
            }
            ContentModerationDecisionOutboxStateDto::TargetUnavailable => {
                domain::ContentModerationDecisionOutboxState::TargetUnavailable
            }
            ContentModerationDecisionOutboxStateDto::RetryExhausted => {
                domain::ContentModerationDecisionOutboxState::RetryExhausted
            }
        }
    }
}

impl ContentModerationDecisionOutboxStateDto {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        domain::ContentModerationDecisionOutboxState::parse(value).map(Into::into)
    }

    pub fn as_str(self) -> &'static str {
        domain::ContentModerationDecisionOutboxState::from(self).as_str()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ModerationReportDto {
    pub id: Uuid,
    pub reporter_id: Uuid,
    pub target_type: ReportTargetTypeDto,
    pub target_id: Uuid,
    pub category: ReportCategoryDto,
    pub reason: String,
    pub status: ReportStatusDto,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

impl From<domain::ModerationReport> for ModerationReportDto {
    fn from(value: domain::ModerationReport) -> Self {
        Self {
            id: value.id,
            reporter_id: value.reporter_id,
            target_type: value.target_type.into(),
            target_id: value.target_id,
            category: value.category.into(),
            reason: value.reason,
            status: value.status.into(),
            created_at: value.created_at,
            resolved_at: value.resolved_at,
        }
    }
}

impl From<&domain::ModerationReport> for ModerationReportDto {
    fn from(value: &domain::ModerationReport) -> Self {
        value.to_owned().into()
    }
}

impl From<ModerationReportDto> for domain::ModerationReport {
    fn from(value: ModerationReportDto) -> Self {
        Self {
            id: value.id,
            reporter_id: value.reporter_id,
            target_type: value.target_type.into(),
            target_id: value.target_id,
            category: value.category.into(),
            reason: value.reason,
            status: value.status.into(),
            created_at: value.created_at,
            resolved_at: value.resolved_at,
        }
    }
}

impl From<&ModerationReportDto> for domain::ModerationReport {
    fn from(value: &ModerationReportDto) -> Self {
        value.to_owned().into()
    }
}

impl std::fmt::Debug for ModerationReportDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModerationReportDto")
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
pub struct ModerationDecisionDto {
    pub id: Uuid,
    pub report_id: Uuid,
    pub actor_id: Uuid,
    pub decision_type: ModerationDecisionTypeDto,
    pub note: String,
    pub created_at: DateTime<Utc>,
}

impl From<domain::ModerationDecision> for ModerationDecisionDto {
    fn from(value: domain::ModerationDecision) -> Self {
        Self {
            id: value.id,
            report_id: value.report_id,
            actor_id: value.actor_id,
            decision_type: value.decision_type.into(),
            note: value.note,
            created_at: value.created_at,
        }
    }
}

impl From<&domain::ModerationDecision> for ModerationDecisionDto {
    fn from(value: &domain::ModerationDecision) -> Self {
        value.to_owned().into()
    }
}

impl From<ModerationDecisionDto> for domain::ModerationDecision {
    fn from(value: ModerationDecisionDto) -> Self {
        Self {
            id: value.id,
            report_id: value.report_id,
            actor_id: value.actor_id,
            decision_type: value.decision_type.into(),
            note: value.note,
            created_at: value.created_at,
        }
    }
}

impl From<&ModerationDecisionDto> for domain::ModerationDecision {
    fn from(value: &ModerationDecisionDto) -> Self {
        value.to_owned().into()
    }
}

impl std::fmt::Debug for ModerationDecisionDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModerationDecisionDto")
            .field("id", &"[REDACTED]")
            .field("report_id", &"[REDACTED]")
            .field("actor_id", &"[REDACTED]")
            .field("decision_type", &self.decision_type)
            .field("note", &"[REDACTED]")
            .field("created_at", &self.created_at)
            .finish()
    }
}

#[derive(Clone)]
pub struct PendingContentModerationDecisionDto {
    pub report: ModerationReportDto,
    pub actor_id: Uuid,
    pub decision_type: ModerationDecisionTypeDto,
    pub note: String,
    pub attempt_count: u32,
    pub lease_owner: Uuid,
}

impl From<domain::PendingContentModerationDecision> for PendingContentModerationDecisionDto {
    fn from(value: domain::PendingContentModerationDecision) -> Self {
        Self {
            report: value.report.into(),
            actor_id: value.actor_id,
            decision_type: value.decision_type.into(),
            note: value.note,
            attempt_count: value.attempt_count,
            lease_owner: value.lease_owner,
        }
    }
}

impl From<&domain::PendingContentModerationDecision> for PendingContentModerationDecisionDto {
    fn from(value: &domain::PendingContentModerationDecision) -> Self {
        value.to_owned().into()
    }
}

impl From<PendingContentModerationDecisionDto> for domain::PendingContentModerationDecision {
    fn from(value: PendingContentModerationDecisionDto) -> Self {
        Self {
            report: value.report.into(),
            actor_id: value.actor_id,
            decision_type: value.decision_type.into(),
            note: value.note,
            attempt_count: value.attempt_count,
            lease_owner: value.lease_owner,
        }
    }
}

impl From<&PendingContentModerationDecisionDto> for domain::PendingContentModerationDecision {
    fn from(value: &PendingContentModerationDecisionDto) -> Self {
        value.to_owned().into()
    }
}

impl std::fmt::Debug for PendingContentModerationDecisionDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PendingContentModerationDecisionDto")
            .field("report", &self.report)
            .field("actor_id", &"[REDACTED]")
            .field("decision_type", &self.decision_type)
            .field("note", &"[REDACTED]")
            .field("attempt_count", &self.attempt_count)
            .field("lease_owner", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Copy, Default, Debug)]
pub struct ContentModerationDecisionBacklogDto {
    pub pending: i64,
    pub oldest_pending_age_seconds: i64,
    pub target_unavailable: i64,
    pub retry_exhausted: i64,
}

impl From<domain::ContentModerationDecisionBacklog> for ContentModerationDecisionBacklogDto {
    fn from(value: domain::ContentModerationDecisionBacklog) -> Self {
        Self {
            pending: value.pending,
            oldest_pending_age_seconds: value.oldest_pending_age_seconds,
            target_unavailable: value.target_unavailable,
            retry_exhausted: value.retry_exhausted,
        }
    }
}

impl From<&domain::ContentModerationDecisionBacklog> for ContentModerationDecisionBacklogDto {
    fn from(value: &domain::ContentModerationDecisionBacklog) -> Self {
        value.to_owned().into()
    }
}

impl From<ContentModerationDecisionBacklogDto> for domain::ContentModerationDecisionBacklog {
    fn from(value: ContentModerationDecisionBacklogDto) -> Self {
        Self {
            pending: value.pending,
            oldest_pending_age_seconds: value.oldest_pending_age_seconds,
            target_unavailable: value.target_unavailable,
            retry_exhausted: value.retry_exhausted,
        }
    }
}

impl From<&ContentModerationDecisionBacklogDto> for domain::ContentModerationDecisionBacklog {
    fn from(value: &ContentModerationDecisionBacklogDto) -> Self {
        value.to_owned().into()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ContentModerationDecisionOutboxEntryDto {
    pub report_id: Uuid,
    pub target_type: ReportTargetTypeDto,
    pub target_id: Uuid,
    pub decision_type: ModerationDecisionTypeDto,
    pub state: ContentModerationDecisionOutboxStateDto,
    pub attempt_count: u32,
    pub replay_count: u32,
    pub last_error_code: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub next_attempt_at: DateTime<Utc>,
}

impl From<domain::ContentModerationDecisionOutboxEntry>
    for ContentModerationDecisionOutboxEntryDto
{
    fn from(value: domain::ContentModerationDecisionOutboxEntry) -> Self {
        Self {
            report_id: value.report_id,
            target_type: value.target_type.into(),
            target_id: value.target_id,
            decision_type: value.decision_type.into(),
            state: value.state.into(),
            attempt_count: value.attempt_count,
            replay_count: value.replay_count,
            last_error_code: value.last_error_code,
            created_at: value.created_at,
            updated_at: value.updated_at,
            next_attempt_at: value.next_attempt_at,
        }
    }
}

impl From<&domain::ContentModerationDecisionOutboxEntry>
    for ContentModerationDecisionOutboxEntryDto
{
    fn from(value: &domain::ContentModerationDecisionOutboxEntry) -> Self {
        value.to_owned().into()
    }
}

impl From<ContentModerationDecisionOutboxEntryDto>
    for domain::ContentModerationDecisionOutboxEntry
{
    fn from(value: ContentModerationDecisionOutboxEntryDto) -> Self {
        Self {
            report_id: value.report_id,
            target_type: value.target_type.into(),
            target_id: value.target_id,
            decision_type: value.decision_type.into(),
            state: value.state.into(),
            attempt_count: value.attempt_count,
            replay_count: value.replay_count,
            last_error_code: value.last_error_code,
            created_at: value.created_at,
            updated_at: value.updated_at,
            next_attempt_at: value.next_attempt_at,
        }
    }
}

impl From<&ContentModerationDecisionOutboxEntryDto>
    for domain::ContentModerationDecisionOutboxEntry
{
    fn from(value: &ContentModerationDecisionOutboxEntryDto) -> Self {
        value.to_owned().into()
    }
}

impl std::fmt::Debug for ContentModerationDecisionOutboxEntryDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ContentModerationDecisionOutboxEntryDto")
            .field("report_id", &"[REDACTED]")
            .field("target_type", &self.target_type)
            .field("target_id", &"[REDACTED]")
            .field("decision_type", &self.decision_type)
            .field("state", &self.state)
            .field("attempt_count", &self.attempt_count)
            .field("replay_count", &self.replay_count)
            .field("last_error_code", &"[REDACTED]")
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .field("next_attempt_at", &self.next_attempt_at)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Copy)]
pub struct ContentModerationDecisionCursorDto {
    pub created_at: DateTime<Utc>,
    pub report_id: Uuid,
}

impl From<domain::ContentModerationDecisionCursor> for ContentModerationDecisionCursorDto {
    fn from(value: domain::ContentModerationDecisionCursor) -> Self {
        Self {
            created_at: value.created_at,
            report_id: value.report_id,
        }
    }
}

impl From<&domain::ContentModerationDecisionCursor> for ContentModerationDecisionCursorDto {
    fn from(value: &domain::ContentModerationDecisionCursor) -> Self {
        value.to_owned().into()
    }
}

impl From<ContentModerationDecisionCursorDto> for domain::ContentModerationDecisionCursor {
    fn from(value: ContentModerationDecisionCursorDto) -> Self {
        Self {
            created_at: value.created_at,
            report_id: value.report_id,
        }
    }
}

impl From<&ContentModerationDecisionCursorDto> for domain::ContentModerationDecisionCursor {
    fn from(value: &ContentModerationDecisionCursorDto) -> Self {
        value.to_owned().into()
    }
}

impl std::fmt::Debug for ContentModerationDecisionCursorDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ContentModerationDecisionCursorDto")
            .field("created_at", &self.created_at)
            .field("report_id", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone)]
pub enum ContentModerationDecisionStageDto {
    Pending(PendingContentModerationDecisionDto),
    AlreadyRecorded {
        report: ModerationReportDto,
        decision: ModerationDecisionDto,
    },
}

impl From<domain::ContentModerationDecisionStage> for ContentModerationDecisionStageDto {
    fn from(value: domain::ContentModerationDecisionStage) -> Self {
        match value {
            domain::ContentModerationDecisionStage::Pending(pending) => {
                Self::Pending(pending.into())
            }
            domain::ContentModerationDecisionStage::AlreadyRecorded { report, decision } => {
                Self::AlreadyRecorded {
                    report: report.into(),
                    decision: decision.into(),
                }
            }
        }
    }
}

impl From<&domain::ContentModerationDecisionStage> for ContentModerationDecisionStageDto {
    fn from(value: &domain::ContentModerationDecisionStage) -> Self {
        value.clone().into()
    }
}

impl From<ContentModerationDecisionStageDto> for domain::ContentModerationDecisionStage {
    fn from(value: ContentModerationDecisionStageDto) -> Self {
        match value {
            ContentModerationDecisionStageDto::Pending(pending) => Self::Pending(pending.into()),
            ContentModerationDecisionStageDto::AlreadyRecorded { report, decision } => {
                Self::AlreadyRecorded {
                    report: report.into(),
                    decision: decision.into(),
                }
            }
        }
    }
}

impl From<&ContentModerationDecisionStageDto> for domain::ContentModerationDecisionStage {
    fn from(value: &ContentModerationDecisionStageDto) -> Self {
        value.clone().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn time() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2024-02-29T12:34:56.123456Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn report() -> domain::ModerationReport {
        domain::ModerationReport {
            id: Uuid::from_u128(1),
            reporter_id: Uuid::from_u128(2),
            target_type: domain::ReportTargetType::Comment,
            target_id: Uuid::from_u128(3),
            category: domain::ReportCategory::Harassment,
            reason: " private 報告\0 ".into(),
            status: domain::ReportStatus::Resolved,
            created_at: time(),
            resolved_at: Some(time() + chrono::Duration::microseconds(1)),
        }
    }

    #[test]
    fn reports_preserve_distinct_identity_fields_text_and_optional_timestamps() {
        let original = report();
        let dto = ModerationReportDto::from(&original);
        assert_eq!(dto.id, Uuid::from_u128(1));
        assert_eq!(dto.reporter_id, Uuid::from_u128(2));
        assert_eq!(dto.target_id, Uuid::from_u128(3));
        assert_eq!(dto.target_type, ReportTargetTypeDto::Comment);
        assert_eq!(dto.category, ReportCategoryDto::Harassment);
        assert_eq!(dto.reason, " private 報告\0 ");
        assert_eq!(dto.status, ReportStatusDto::Resolved);
        assert_eq!(dto.created_at, time());
        assert_eq!(
            dto.resolved_at,
            Some(time() + chrono::Duration::microseconds(1))
        );
        assert_eq!(domain::ModerationReport::from(&dto), original);
        assert_eq!(domain::ModerationReport::from(dto), original);
        let mut open = original;
        open.resolved_at = None;
        open.status = domain::ReportStatus::Open;
        assert_eq!(ModerationReportDto::from(open.clone()).resolved_at, None);
        assert_eq!(
            domain::ModerationReport::from(ModerationReportDto::from(open.clone())),
            open
        );
    }

    #[test]
    fn pending_and_recorded_stages_preserve_actor_lease_attempts_and_decisions() {
        let original = domain::PendingContentModerationDecision {
            report: report(),
            actor_id: Uuid::from_u128(4),
            decision_type: domain::ModerationDecisionType::RestoreContent,
            note: " private decision\0 復元 ".into(),
            attempt_count: u32::MAX,
            lease_owner: Uuid::from_u128(5),
        };
        let dto = PendingContentModerationDecisionDto::from(&original);
        assert_eq!(dto.report, ModerationReportDto::from(report()));
        assert_eq!(dto.actor_id, Uuid::from_u128(4));
        assert_eq!(dto.lease_owner, Uuid::from_u128(5));
        assert_eq!(dto.attempt_count, u32::MAX);
        assert_eq!(dto.note, " private decision\0 復元 ");
        assert_eq!(dto.decision_type, ModerationDecisionTypeDto::RestoreContent);
        for recovered in [
            domain::PendingContentModerationDecision::from(&dto),
            domain::PendingContentModerationDecision::from(dto),
        ] {
            assert_eq!(recovered.report, original.report);
            assert_eq!(recovered.actor_id, original.actor_id);
            assert_eq!(recovered.lease_owner, original.lease_owner);
            assert_eq!(recovered.attempt_count, original.attempt_count);
            assert_eq!(recovered.note, original.note);
            assert_eq!(recovered.decision_type, original.decision_type);
        }
        let stage = domain::ContentModerationDecisionStage::Pending(original);
        let dto = ContentModerationDecisionStageDto::from(&stage);
        assert!(
            matches!(&dto, ContentModerationDecisionStageDto::Pending(value) if value.attempt_count == u32::MAX && value.lease_owner == Uuid::from_u128(5))
        );
        assert!(
            matches!(domain::ContentModerationDecisionStage::from(dto), domain::ContentModerationDecisionStage::Pending(value) if value.actor_id == Uuid::from_u128(4))
        );
        let decision = domain::ModerationDecision {
            id: Uuid::from_u128(6),
            report_id: Uuid::from_u128(1),
            actor_id: Uuid::from_u128(4),
            decision_type: domain::ModerationDecisionType::RestoreContent,
            note: " private recorded note ".into(),
            created_at: time(),
        };
        let record = ModerationDecisionDto::from(&decision);
        assert_eq!(record.id, Uuid::from_u128(6));
        assert_eq!(record.report_id, Uuid::from_u128(1));
        assert_eq!(record.actor_id, Uuid::from_u128(4));
        assert_eq!(record.note, " private recorded note ");
        assert_eq!(record.created_at, time());
        assert_eq!(
            record.decision_type,
            ModerationDecisionTypeDto::RestoreContent
        );
        assert_eq!(domain::ModerationDecision::from(&record), decision);
        assert_eq!(domain::ModerationDecision::from(record), decision);
        let stage = domain::ContentModerationDecisionStage::AlreadyRecorded {
            report: report(),
            decision: decision.clone(),
        };
        for dto in [
            ContentModerationDecisionStageDto::from(&stage),
            ContentModerationDecisionStageDto::from(stage),
        ] {
            match domain::ContentModerationDecisionStage::from(&dto) {
                domain::ContentModerationDecisionStage::AlreadyRecorded {
                    report: actual,
                    decision: actual_decision,
                } => {
                    assert_eq!(actual, report());
                    assert_eq!(actual_decision, decision);
                }
                _ => panic!("recorded state became pending"),
            }
        }
    }

    #[test]
    fn cursor_outbox_and_backlog_preserve_ordering_and_recovery_state() {
        let cursor = domain::ContentModerationDecisionCursor {
            created_at: time(),
            report_id: Uuid::from_u128(7),
        };
        let dto = ContentModerationDecisionCursorDto::from(&cursor);
        assert_eq!(dto.created_at, time());
        assert_eq!(dto.report_id, Uuid::from_u128(7));
        assert_eq!(domain::ContentModerationDecisionCursor::from(&dto), cursor);
        assert_eq!(domain::ContentModerationDecisionCursor::from(dto), cursor);
        let original = domain::ContentModerationDecisionOutboxEntry {
            report_id: Uuid::from_u128(7),
            target_type: domain::ReportTargetType::Post,
            target_id: Uuid::from_u128(8),
            decision_type: domain::ModerationDecisionType::HideContent,
            state: domain::ContentModerationDecisionOutboxState::RetryExhausted,
            attempt_count: u32::MAX,
            replay_count: 17,
            last_error_code: Some(" private failure ".into()),
            created_at: time(),
            updated_at: time() + chrono::Duration::seconds(1),
            next_attempt_at: time() + chrono::Duration::seconds(2),
        };
        let dto = ContentModerationDecisionOutboxEntryDto::from(&original);
        assert_eq!(dto.report_id, Uuid::from_u128(7));
        assert_eq!(dto.target_id, Uuid::from_u128(8));
        assert_eq!(dto.target_type, ReportTargetTypeDto::Post);
        assert_eq!(dto.decision_type, ModerationDecisionTypeDto::HideContent);
        assert_eq!(
            dto.state,
            ContentModerationDecisionOutboxStateDto::RetryExhausted
        );
        assert_eq!(dto.attempt_count, u32::MAX);
        assert_eq!(dto.replay_count, 17);
        assert_eq!(dto.last_error_code.as_deref(), Some(" private failure "));
        assert_eq!(dto.created_at, time());
        assert_eq!(dto.updated_at, time() + chrono::Duration::seconds(1));
        assert_eq!(dto.next_attempt_at, time() + chrono::Duration::seconds(2));
        assert_eq!(
            domain::ContentModerationDecisionOutboxEntry::from(&dto),
            original
        );
        assert_eq!(
            domain::ContentModerationDecisionOutboxEntry::from(dto),
            original
        );
        let backlog = domain::ContentModerationDecisionBacklog {
            pending: 3,
            oldest_pending_age_seconds: 4,
            target_unavailable: 5,
            retry_exhausted: 6,
        };
        let dto = ContentModerationDecisionBacklogDto::from(&backlog);
        assert_eq!(dto.pending, 3);
        assert_eq!(dto.oldest_pending_age_seconds, 4);
        assert_eq!(dto.target_unavailable, 5);
        assert_eq!(dto.retry_exhausted, 6);
        assert_eq!(
            domain::ContentModerationDecisionBacklog::from(&dto),
            backlog
        );
        assert_eq!(domain::ContentModerationDecisionBacklog::from(dto), backlog);
    }

    #[test]
    fn transfer_diagnostics_redact_private_payload_and_all_identity_fields() {
        let report = report();
        let decision = domain::ModerationDecision {
            id: Uuid::from_u128(6),
            report_id: report.id,
            actor_id: Uuid::from_u128(4),
            decision_type: domain::ModerationDecisionType::HideContent,
            note: "private decision marker".into(),
            created_at: time(),
        };
        let pending = domain::PendingContentModerationDecision {
            report: report.clone(),
            actor_id: decision.actor_id,
            decision_type: decision.decision_type,
            note: decision.note.clone(),
            attempt_count: 2,
            lease_owner: Uuid::from_u128(5),
        };
        let outbox = domain::ContentModerationDecisionOutboxEntry {
            report_id: report.id,
            target_type: report.target_type,
            target_id: report.target_id,
            decision_type: decision.decision_type,
            state: domain::ContentModerationDecisionOutboxState::Pending,
            attempt_count: 2,
            replay_count: 3,
            last_error_code: Some("private error marker".into()),
            created_at: time(),
            updated_at: time(),
            next_attempt_at: time(),
        };
        let rendered = format!(
            "{:?} {:?} {:?} {:?} {:?} {:?} {:?}",
            ModerationReportDto::from(report.clone()),
            ModerationDecisionDto::from(decision.clone()),
            PendingContentModerationDecisionDto::from(pending.clone()),
            ContentModerationDecisionOutboxEntryDto::from(outbox),
            ContentModerationDecisionCursorDto::from(domain::ContentModerationDecisionCursor {
                created_at: time(),
                report_id: report.id
            }),
            ContentModerationDecisionStageDto::from(
                domain::ContentModerationDecisionStage::Pending(pending)
            ),
            ContentModerationDecisionStageDto::from(
                domain::ContentModerationDecisionStage::AlreadyRecorded { report, decision }
            )
        );
        for marker in [
            "private 報告",
            "private decision marker",
            "private error marker",
        ] {
            assert!(!rendered.contains(marker), "{rendered}");
        }
        for id in 1..=6 {
            assert!(
                !rendered.contains(&Uuid::from_u128(id).to_string()),
                "{rendered}"
            );
        }
        assert!(rendered.contains("[REDACTED]"));
    }

    #[test]
    fn enum_tags_and_content_actions_retain_domain_parsing_rules() {
        assert_eq!(
            ReportTargetTypeDto::from(domain::ReportTargetType::User),
            ReportTargetTypeDto::User
        );
        assert_eq!(
            domain::ReportTargetType::from(ReportTargetTypeDto::User),
            domain::ReportTargetType::User
        );
        assert_eq!(ReportTargetTypeDto::User.as_str(), "USER");
        assert_eq!(
            ReportTargetTypeDto::parse("USER").unwrap(),
            ReportTargetTypeDto::User
        );
        assert_eq!(
            ReportTargetTypeDto::from(domain::ReportTargetType::Post),
            ReportTargetTypeDto::Post
        );
        assert_eq!(
            domain::ReportTargetType::from(ReportTargetTypeDto::Post),
            domain::ReportTargetType::Post
        );
        assert_eq!(ReportTargetTypeDto::Post.as_str(), "POST");
        assert_eq!(
            ReportTargetTypeDto::parse("POST").unwrap(),
            ReportTargetTypeDto::Post
        );
        assert_eq!(
            ReportTargetTypeDto::from(domain::ReportTargetType::Comment),
            ReportTargetTypeDto::Comment
        );
        assert_eq!(
            domain::ReportTargetType::from(ReportTargetTypeDto::Comment),
            domain::ReportTargetType::Comment
        );
        assert_eq!(ReportTargetTypeDto::Comment.as_str(), "COMMENT");
        assert_eq!(
            ReportTargetTypeDto::parse("COMMENT").unwrap(),
            ReportTargetTypeDto::Comment
        );
        assert_eq!(
            ReportTargetTypeDto::parse("invalid")
                .unwrap_err()
                .to_string(),
            domain::ReportTargetType::parse("invalid")
                .unwrap_err()
                .to_string()
        );
        assert_eq!(
            ReportCategoryDto::from(domain::ReportCategory::Spam),
            ReportCategoryDto::Spam
        );
        assert_eq!(
            domain::ReportCategory::from(ReportCategoryDto::Spam),
            domain::ReportCategory::Spam
        );
        assert_eq!(ReportCategoryDto::Spam.as_str(), "SPAM");
        assert_eq!(
            ReportCategoryDto::parse("SPAM").unwrap(),
            ReportCategoryDto::Spam
        );
        assert_eq!(
            ReportCategoryDto::from(domain::ReportCategory::Harassment),
            ReportCategoryDto::Harassment
        );
        assert_eq!(
            domain::ReportCategory::from(ReportCategoryDto::Harassment),
            domain::ReportCategory::Harassment
        );
        assert_eq!(ReportCategoryDto::Harassment.as_str(), "HARASSMENT");
        assert_eq!(
            ReportCategoryDto::parse("HARASSMENT").unwrap(),
            ReportCategoryDto::Harassment
        );
        assert_eq!(
            ReportCategoryDto::from(domain::ReportCategory::Hate),
            ReportCategoryDto::Hate
        );
        assert_eq!(
            domain::ReportCategory::from(ReportCategoryDto::Hate),
            domain::ReportCategory::Hate
        );
        assert_eq!(ReportCategoryDto::Hate.as_str(), "HATE");
        assert_eq!(
            ReportCategoryDto::parse("HATE").unwrap(),
            ReportCategoryDto::Hate
        );
        assert_eq!(
            ReportCategoryDto::from(domain::ReportCategory::Violence),
            ReportCategoryDto::Violence
        );
        assert_eq!(
            domain::ReportCategory::from(ReportCategoryDto::Violence),
            domain::ReportCategory::Violence
        );
        assert_eq!(ReportCategoryDto::Violence.as_str(), "VIOLENCE");
        assert_eq!(
            ReportCategoryDto::parse("VIOLENCE").unwrap(),
            ReportCategoryDto::Violence
        );
        assert_eq!(
            ReportCategoryDto::from(domain::ReportCategory::Other),
            ReportCategoryDto::Other
        );
        assert_eq!(
            domain::ReportCategory::from(ReportCategoryDto::Other),
            domain::ReportCategory::Other
        );
        assert_eq!(ReportCategoryDto::Other.as_str(), "OTHER");
        assert_eq!(
            ReportCategoryDto::parse("OTHER").unwrap(),
            ReportCategoryDto::Other
        );
        assert_eq!(
            ReportCategoryDto::parse("invalid").unwrap_err().to_string(),
            domain::ReportCategory::parse("invalid")
                .unwrap_err()
                .to_string()
        );
        assert_eq!(
            ReportStatusDto::from(domain::ReportStatus::Open),
            ReportStatusDto::Open
        );
        assert_eq!(
            domain::ReportStatus::from(ReportStatusDto::Open),
            domain::ReportStatus::Open
        );
        assert_eq!(ReportStatusDto::Open.as_str(), "OPEN");
        assert_eq!(
            ReportStatusDto::parse("OPEN").unwrap(),
            ReportStatusDto::Open
        );
        assert_eq!(
            ReportStatusDto::from(domain::ReportStatus::Resolved),
            ReportStatusDto::Resolved
        );
        assert_eq!(
            domain::ReportStatus::from(ReportStatusDto::Resolved),
            domain::ReportStatus::Resolved
        );
        assert_eq!(ReportStatusDto::Resolved.as_str(), "RESOLVED");
        assert_eq!(
            ReportStatusDto::parse("RESOLVED").unwrap(),
            ReportStatusDto::Resolved
        );
        assert_eq!(
            ReportStatusDto::from(domain::ReportStatus::Dismissed),
            ReportStatusDto::Dismissed
        );
        assert_eq!(
            domain::ReportStatus::from(ReportStatusDto::Dismissed),
            domain::ReportStatus::Dismissed
        );
        assert_eq!(ReportStatusDto::Dismissed.as_str(), "DISMISSED");
        assert_eq!(
            ReportStatusDto::parse("DISMISSED").unwrap(),
            ReportStatusDto::Dismissed
        );
        assert_eq!(
            ReportStatusDto::parse("invalid").unwrap_err().to_string(),
            domain::ReportStatus::parse("invalid")
                .unwrap_err()
                .to_string()
        );
        assert_eq!(
            ModerationDecisionTypeDto::from(domain::ModerationDecisionType::Dismiss),
            ModerationDecisionTypeDto::Dismiss
        );
        assert_eq!(
            domain::ModerationDecisionType::from(ModerationDecisionTypeDto::Dismiss),
            domain::ModerationDecisionType::Dismiss
        );
        assert_eq!(ModerationDecisionTypeDto::Dismiss.as_str(), "DISMISS");
        assert_eq!(
            ModerationDecisionTypeDto::parse("DISMISS").unwrap(),
            ModerationDecisionTypeDto::Dismiss
        );
        assert_eq!(
            ModerationDecisionTypeDto::from(domain::ModerationDecisionType::HideContent),
            ModerationDecisionTypeDto::HideContent
        );
        assert_eq!(
            domain::ModerationDecisionType::from(ModerationDecisionTypeDto::HideContent),
            domain::ModerationDecisionType::HideContent
        );
        assert_eq!(
            ModerationDecisionTypeDto::HideContent.as_str(),
            "HIDE_CONTENT"
        );
        assert_eq!(
            ModerationDecisionTypeDto::parse("HIDE_CONTENT").unwrap(),
            ModerationDecisionTypeDto::HideContent
        );
        assert_eq!(
            ModerationDecisionTypeDto::from(domain::ModerationDecisionType::RestoreContent),
            ModerationDecisionTypeDto::RestoreContent
        );
        assert_eq!(
            domain::ModerationDecisionType::from(ModerationDecisionTypeDto::RestoreContent),
            domain::ModerationDecisionType::RestoreContent
        );
        assert_eq!(
            ModerationDecisionTypeDto::RestoreContent.as_str(),
            "RESTORE_CONTENT"
        );
        assert_eq!(
            ModerationDecisionTypeDto::parse("RESTORE_CONTENT").unwrap(),
            ModerationDecisionTypeDto::RestoreContent
        );
        assert_eq!(
            ModerationDecisionTypeDto::from(domain::ModerationDecisionType::RestrictUser),
            ModerationDecisionTypeDto::RestrictUser
        );
        assert_eq!(
            domain::ModerationDecisionType::from(ModerationDecisionTypeDto::RestrictUser),
            domain::ModerationDecisionType::RestrictUser
        );
        assert_eq!(
            ModerationDecisionTypeDto::RestrictUser.as_str(),
            "RESTRICT_USER"
        );
        assert_eq!(
            ModerationDecisionTypeDto::parse("RESTRICT_USER").unwrap(),
            ModerationDecisionTypeDto::RestrictUser
        );
        assert_eq!(
            ModerationDecisionTypeDto::from(domain::ModerationDecisionType::LiftRestriction),
            ModerationDecisionTypeDto::LiftRestriction
        );
        assert_eq!(
            domain::ModerationDecisionType::from(ModerationDecisionTypeDto::LiftRestriction),
            domain::ModerationDecisionType::LiftRestriction
        );
        assert_eq!(
            ModerationDecisionTypeDto::LiftRestriction.as_str(),
            "LIFT_RESTRICTION"
        );
        assert_eq!(
            ModerationDecisionTypeDto::parse("LIFT_RESTRICTION").unwrap(),
            ModerationDecisionTypeDto::LiftRestriction
        );
        assert_eq!(
            ModerationDecisionTypeDto::parse("invalid")
                .unwrap_err()
                .to_string(),
            domain::ModerationDecisionType::parse("invalid")
                .unwrap_err()
                .to_string()
        );
        assert_eq!(
            ContentModerationDecisionOutboxStateDto::from(
                domain::ContentModerationDecisionOutboxState::Pending
            ),
            ContentModerationDecisionOutboxStateDto::Pending
        );
        assert_eq!(
            domain::ContentModerationDecisionOutboxState::from(
                ContentModerationDecisionOutboxStateDto::Pending
            ),
            domain::ContentModerationDecisionOutboxState::Pending
        );
        assert_eq!(
            ContentModerationDecisionOutboxStateDto::Pending.as_str(),
            "PENDING"
        );
        assert_eq!(
            ContentModerationDecisionOutboxStateDto::parse("PENDING").unwrap(),
            ContentModerationDecisionOutboxStateDto::Pending
        );
        assert_eq!(
            ContentModerationDecisionOutboxStateDto::from(
                domain::ContentModerationDecisionOutboxState::TargetUnavailable
            ),
            ContentModerationDecisionOutboxStateDto::TargetUnavailable
        );
        assert_eq!(
            domain::ContentModerationDecisionOutboxState::from(
                ContentModerationDecisionOutboxStateDto::TargetUnavailable
            ),
            domain::ContentModerationDecisionOutboxState::TargetUnavailable
        );
        assert_eq!(
            ContentModerationDecisionOutboxStateDto::TargetUnavailable.as_str(),
            "TARGET_UNAVAILABLE"
        );
        assert_eq!(
            ContentModerationDecisionOutboxStateDto::parse("TARGET_UNAVAILABLE").unwrap(),
            ContentModerationDecisionOutboxStateDto::TargetUnavailable
        );
        assert_eq!(
            ContentModerationDecisionOutboxStateDto::from(
                domain::ContentModerationDecisionOutboxState::RetryExhausted
            ),
            ContentModerationDecisionOutboxStateDto::RetryExhausted
        );
        assert_eq!(
            domain::ContentModerationDecisionOutboxState::from(
                ContentModerationDecisionOutboxStateDto::RetryExhausted
            ),
            domain::ContentModerationDecisionOutboxState::RetryExhausted
        );
        assert_eq!(
            ContentModerationDecisionOutboxStateDto::RetryExhausted.as_str(),
            "RETRY_EXHAUSTED"
        );
        assert_eq!(
            ContentModerationDecisionOutboxStateDto::parse("RETRY_EXHAUSTED").unwrap(),
            ContentModerationDecisionOutboxStateDto::RetryExhausted
        );
        assert_eq!(
            ContentModerationDecisionOutboxStateDto::parse("invalid")
                .unwrap_err()
                .to_string(),
            domain::ContentModerationDecisionOutboxState::parse("invalid")
                .unwrap_err()
                .to_string()
        );
        assert_eq!(
            ModerationDecisionTypeDto::HideContent.content_action(),
            Some("HIDE")
        );
        assert_eq!(
            ModerationDecisionTypeDto::RestoreContent.content_action(),
            Some("RESTORE")
        );
        for value in [
            ModerationDecisionTypeDto::Dismiss,
            ModerationDecisionTypeDto::RestrictUser,
            ModerationDecisionTypeDto::LiftRestriction,
        ] {
            assert_eq!(value.content_action(), None);
        }
        assert_eq!(
            ReportTargetTypeDto::parse(" post ").unwrap(),
            ReportTargetTypeDto::Post
        );
        assert!(ContentModerationDecisionOutboxStateDto::parse(" pending ").is_err());
    }
}
