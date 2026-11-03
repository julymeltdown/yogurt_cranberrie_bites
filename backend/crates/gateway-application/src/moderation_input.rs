//! Controlled semantic input failures. No caller data or library cause is carried.
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationInputError {
    ReporterId,
    TargetId,
    ReportId,
    ReviewerId,
    ActorId,
    UserId,
    TargetType,
    SelfReport,
    Category,
    Reason,
    DecisionType,
    Note,
    ContentTarget,
    UserTarget,
    ReplayResolutionId,
    ReplayReason,
    ReplayEvidence,
}

impl ModerationInputError {
    pub fn message(self) -> &'static str {
        match self {
            Self::ReporterId => "Reporter ID is required",
            Self::TargetId => "Report target ID is required",
            Self::ReportId => "Report ID is required",
            Self::ReviewerId => "Reviewer ID is required",
            Self::ActorId => "Actor ID is required",
            Self::UserId => "User ID is required",
            Self::TargetType => "Report target must be USER, POST, or COMMENT",
            Self::SelfReport => "Cannot report yourself",
            Self::Category => "Report category must be SPAM, HARASSMENT, HATE, VIOLENCE, or OTHER",
            Self::Reason => "Report reason must be 3-1000 characters",
            Self::DecisionType => "Moderation decision must be DISMISS, HIDE_CONTENT, RESTORE_CONTENT, RESTRICT_USER, or LIFT_RESTRICTION",
            Self::Note => "Moderation note must be at most 2000 characters",
            Self::ContentTarget => "Content decisions require a POST or COMMENT report",
            Self::UserTarget => "User restriction decisions require a USER report",
            Self::ReplayResolutionId => "Replay resolution ID is required",
            Self::ReplayReason => "reason must contain 20 to 1024 non-control characters and at most 1024 UTF-8 bytes",
            Self::ReplayEvidence => "evidenceReference must be a 3 to 256 character protected-record identifier",
        }
    }
}

impl fmt::Display for ModerationInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message())
    }
}

impl std::error::Error for ModerationInputError {}
