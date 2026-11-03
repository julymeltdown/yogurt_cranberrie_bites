//! Private persistence transfers; bodies, fingerprints and actor identities are
//! neither serializable nor exposed through Debug.
use super::operation_view_dto::OperationOutcomeDto;
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct JournalIntentDto<'a> {
    pub id: Uuid,
    pub actor: Option<&'a str>,
    pub action: &'a str,
    pub path: &'a str,
    pub request_id: Option<&'a str>,
    pub ip: Option<&'a str>,
    pub agent: Option<&'a str>,
    pub key: Option<&'a str>,
    pub context: &'a str,
    pub fingerprint_body: &'a [u8],
}
pub struct JournalReplayQueryDto<'a> {
    pub actor: Option<&'a str>,
    pub key: Option<&'a str>,
    pub context: &'a str,
    pub body: &'a [u8],
}
pub struct JournalReplayIdentityDto {
    pub id: Uuid,
    pub state: String,
    pub same_request: bool,
    pub unexpired: bool,
}
pub struct JournalReplayResponseDto {
    pub identity: JournalReplayIdentityDto,
    pub response_status: Option<i32>,
    pub response_content_type: Option<String>,
    pub response_body: Option<Vec<u8>>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum JournalCompletionCallerDto {
    Completion,
    OwnerReplay,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalCompletionWriteDto {
    Recorded,
    AlreadyResolved,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum JournalOutcomeEvidenceDto {
    IntentRecorded,
    QueueAccepted,
    SuccessResponse,
    PreDispatchRejection,
    ResponseStatusOnly,
    OwnerReplayReceipt,
    ManualResolution,
    StaleOperation,
    LegacyUnresolved,
}
impl From<gateway_domain::operation::OperationOutcomeEvidence> for JournalOutcomeEvidenceDto {
    fn from(value: gateway_domain::operation::OperationOutcomeEvidence) -> Self {
        use gateway_domain::operation::OperationOutcomeEvidence as E;
        match value {
            E::IntentRecorded => Self::IntentRecorded,
            E::QueueAccepted => Self::QueueAccepted,
            E::SuccessResponse => Self::SuccessResponse,
            E::PreDispatchRejection => Self::PreDispatchRejection,
            E::ResponseStatusOnly => Self::ResponseStatusOnly,
            E::OwnerReplayReceipt => Self::OwnerReplayReceipt,
            E::ManualResolution => Self::ManualResolution,
            E::StaleOperation => Self::StaleOperation,
            E::LegacyUnresolved => Self::LegacyUnresolved,
        }
    }
}
impl From<JournalOutcomeEvidenceDto> for gateway_domain::operation::OperationOutcomeEvidence {
    fn from(value: JournalOutcomeEvidenceDto) -> Self {
        match value {
            JournalOutcomeEvidenceDto::IntentRecorded => Self::IntentRecorded,
            JournalOutcomeEvidenceDto::QueueAccepted => Self::QueueAccepted,
            JournalOutcomeEvidenceDto::SuccessResponse => Self::SuccessResponse,
            JournalOutcomeEvidenceDto::PreDispatchRejection => Self::PreDispatchRejection,
            JournalOutcomeEvidenceDto::ResponseStatusOnly => Self::ResponseStatusOnly,
            JournalOutcomeEvidenceDto::OwnerReplayReceipt => Self::OwnerReplayReceipt,
            JournalOutcomeEvidenceDto::ManualResolution => Self::ManualResolution,
            JournalOutcomeEvidenceDto::StaleOperation => Self::StaleOperation,
            JournalOutcomeEvidenceDto::LegacyUnresolved => Self::LegacyUnresolved,
        }
    }
}
pub struct JournalCompletionCommandDto<'a> {
    pub id: Uuid,
    pub outcome: OperationOutcomeDto,
    pub evidence: JournalOutcomeEvidenceDto,
    pub status: u16,
    pub content_type: Option<&'a str>,
    pub body: Option<&'a [u8]>,
}
pub struct JournalOwnerReplayCommandDto<'a> {
    pub id: Uuid,
    pub owner: &'a str,
    pub status: u16,
    pub content_type: Option<&'a str>,
    pub body: Option<&'a [u8]>,
}
pub struct JournalPurgeCommandDto {
    pub telemetry_retention_days: i64,
    pub audit_retention_days: i64,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PurgeOutcome {
    pub telemetry_records: u64,
    pub audit_events: u64,
    pub rate_limit_keys: u64,
}
impl PurgeOutcome {
    pub fn total(&self) -> u64 {
        self.telemetry_records + self.audit_events + self.rate_limit_keys
    }
}
pub struct OwnerReceiptClaimRequestDto {
    pub lease_seconds: f64,
}
pub struct OwnerReceiptClaimDto {
    pub operation_id: Uuid,
    pub token: Uuid,
    pub attempt: i32,
}
pub struct OwnerReceiptIdentityDto<'a> {
    pub actor: Option<&'a str>,
    pub action: &'a str,
    pub resource_path: &'a str,
}
pub struct OwnerReceiptFinishLookupDto<'a> {
    pub claim: &'a OwnerReceiptClaimDto,
    pub identity: OwnerReceiptIdentityDto<'a>,
    pub suspended_reason: Option<&'a str>,
    pub retry_seconds: f64,
}
pub struct OwnerReceiptCommitDto<'a> {
    pub claim: &'a OwnerReceiptClaimDto,
    pub identity: OwnerReceiptIdentityDto<'a>,
    pub scope: Option<&'a str>,
    pub receipt_state: Option<&'a str>,
    pub committed_at: Option<DateTime<Utc>>,
}
pub struct LocalOwnerReceiptRecordDto {
    pub actor_id: Uuid,
    pub scope: String,
    pub state: String,
    pub committed_at: DateTime<Utc>,
    pub replay_until: DateTime<Utc>,
    pub replayable: bool,
}
