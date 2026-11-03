//! Internal persistence records, deliberately separate from serializable HTTP
//! views. Only this record supplies the actor needed for owner-receipt checks.
use super::operation_view_dto::{
    ManualResolutionDispositionDto, ManualResolutionEvidenceKindDto, OperationOutcomeDto,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationJournalLookupDto {
    pub operation_id: Uuid,
}

#[derive(Clone, PartialEq, Eq)]
pub struct CallerOperationJournalLookupDto {
    pub operation_id: Uuid,
    pub actor_id: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ManualResolutionJournalRecordDto {
    pub resolution_id: Uuid,
    pub operation_id: Uuid,
    pub revision: i32,
    pub supersedes_resolution_id: Option<Uuid>,
    pub disposition: ManualResolutionDispositionDto,
    pub evidence_kind: ManualResolutionEvidenceKindDto,
    pub evidence_reference: String,
    pub reason: String,
    pub reviewer_id: String,
    pub created_at: DateTime<Utc>,
}

impl From<gateway_domain::operation::ManualOperationResolution>
    for ManualResolutionJournalRecordDto
{
    fn from(value: gateway_domain::operation::ManualOperationResolution) -> Self {
        Self {
            resolution_id: value.resolution_id,
            operation_id: value.operation_id,
            revision: value.revision,
            supersedes_resolution_id: value.supersedes_resolution_id,
            disposition: value.disposition.into(),
            evidence_kind: value.evidence_kind.into(),
            evidence_reference: value.evidence_reference,
            reason: value.reason,
            reviewer_id: value.reviewer_id,
            created_at: value.created_at,
        }
    }
}

impl From<ManualResolutionJournalRecordDto>
    for gateway_domain::operation::ManualOperationResolution
{
    fn from(value: ManualResolutionJournalRecordDto) -> Self {
        Self {
            resolution_id: value.resolution_id,
            operation_id: value.operation_id,
            revision: value.revision,
            supersedes_resolution_id: value.supersedes_resolution_id,
            disposition: value.disposition.into(),
            evidence_kind: value.evidence_kind.into(),
            evidence_reference: value.evidence_reference,
            reason: value.reason,
            reviewer_id: value.reviewer_id,
            created_at: value.created_at,
        }
    }
}

/// Carries durable correlation; has no Serialize, Deserialize or public-view
/// conversion. Debug output must not include actor, paths or reviewer evidence.
#[derive(Clone, PartialEq, Eq)]
pub struct OperationJournalRecordDto {
    pub operation_id: Uuid,
    pub actor_id: Option<String>,
    pub state: String,
    pub outcome: OperationOutcomeDto,
    pub action: String,
    pub resource_path: String,
    pub response_status: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub replayable: bool,
    pub manual_resolution: Option<ManualResolutionJournalRecordDto>,
}

impl From<OperationJournalRecordDto> for gateway_domain::operation::OperationStatus {
    fn from(value: OperationJournalRecordDto) -> Self {
        Self::from_persisted(
            gateway_domain::operation::OperationStatusFields {
                operation_id: value.operation_id,
                state: value.state,
                outcome: value.outcome.into(),
                action: value.action,
                resource_path: value.resource_path,
                response_status: value.response_status,
                created_at: value.created_at,
                finished_at: value.finished_at,
                replayable: value.replayable,
                owner_receipt: gateway_domain::operation::OwnerReceiptStatus::default(),
                manual_resolution: value.manual_resolution.map(Into::into),
            },
            value.actor_id,
        )
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CallerOperationJournalRecordDto {
    pub operation_id: Uuid,
    pub state: String,
    pub outcome: OperationOutcomeDto,
    pub response_status: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub replayable: bool,
}

impl From<CallerOperationJournalRecordDto> for gateway_domain::operation::CallerOperationStatus {
    fn from(value: CallerOperationJournalRecordDto) -> Self {
        Self {
            operation_id: value.operation_id,
            state: value.state,
            outcome: value.outcome.into(),
            response_status: value.response_status,
            created_at: value.created_at,
            finished_at: value.finished_at,
            replayable: value.replayable,
        }
    }
}

/// Repository command owns explicit audit attribution; it is not an HTTP body.
#[derive(Clone, PartialEq, Eq)]
pub struct ManualResolutionJournalCommandDto {
    pub operation_id: Uuid,
    pub reviewer_id: String,
    pub disposition: ManualResolutionDispositionDto,
    pub evidence_kind: ManualResolutionEvidenceKindDto,
    pub evidence_reference: String,
    pub reason: String,
    pub supersedes_resolution_id: Option<Uuid>,
    pub request_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum ManualResolutionJournalWriteDto {
    Created(ManualResolutionJournalRecordDto),
    Existing(ManualResolutionJournalRecordDto),
    OperationNotFound,
    OperationNotUnknown,
    Conflict,
}

/// Outstanding operations that need reconciliation, and the oldest age.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReconciliationBacklog {
    pub outstanding: i64,
    pub oldest_age_seconds: i64,
}
