//! Operation HTTP views contain only declared wire fields. Internal owner
//! correlation stays in the domain until policy has finished; view transfers are
//! intentionally one-way and do not reconstruct missing actor identity.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ManualResolutionDispositionDto {
    Committed,
    NotCommitted,
    Unresolved,
}
impl From<gateway_domain::operation::ManualResolutionDisposition>
    for ManualResolutionDispositionDto
{
    fn from(value: gateway_domain::operation::ManualResolutionDisposition) -> Self {
        match value {
            gateway_domain::operation::ManualResolutionDisposition::Committed => Self::Committed,
            gateway_domain::operation::ManualResolutionDisposition::NotCommitted => {
                Self::NotCommitted
            }
            gateway_domain::operation::ManualResolutionDisposition::Unresolved => Self::Unresolved,
        }
    }
}
impl From<ManualResolutionDispositionDto>
    for gateway_domain::operation::ManualResolutionDisposition
{
    fn from(value: ManualResolutionDispositionDto) -> Self {
        match value {
            ManualResolutionDispositionDto::Committed => Self::Committed,
            ManualResolutionDispositionDto::NotCommitted => Self::NotCommitted,
            ManualResolutionDispositionDto::Unresolved => Self::Unresolved,
        }
    }
}
impl ManualResolutionDispositionDto {
    pub fn as_str(self) -> &'static str {
        gateway_domain::operation::ManualResolutionDisposition::from(self).as_str()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ManualResolutionEvidenceKindDto {
    DatabaseAudit,
    OwnerConfirmation,
    ServiceLog,
    IncidentRecord,
    Other,
}
impl From<gateway_domain::operation::ManualResolutionEvidenceKind>
    for ManualResolutionEvidenceKindDto
{
    fn from(value: gateway_domain::operation::ManualResolutionEvidenceKind) -> Self {
        match value {
            gateway_domain::operation::ManualResolutionEvidenceKind::DatabaseAudit => {
                Self::DatabaseAudit
            }
            gateway_domain::operation::ManualResolutionEvidenceKind::OwnerConfirmation => {
                Self::OwnerConfirmation
            }
            gateway_domain::operation::ManualResolutionEvidenceKind::ServiceLog => Self::ServiceLog,
            gateway_domain::operation::ManualResolutionEvidenceKind::IncidentRecord => {
                Self::IncidentRecord
            }
            gateway_domain::operation::ManualResolutionEvidenceKind::Other => Self::Other,
        }
    }
}
impl From<ManualResolutionEvidenceKindDto>
    for gateway_domain::operation::ManualResolutionEvidenceKind
{
    fn from(value: ManualResolutionEvidenceKindDto) -> Self {
        match value {
            ManualResolutionEvidenceKindDto::DatabaseAudit => Self::DatabaseAudit,
            ManualResolutionEvidenceKindDto::OwnerConfirmation => Self::OwnerConfirmation,
            ManualResolutionEvidenceKindDto::ServiceLog => Self::ServiceLog,
            ManualResolutionEvidenceKindDto::IncidentRecord => Self::IncidentRecord,
            ManualResolutionEvidenceKindDto::Other => Self::Other,
        }
    }
}
impl ManualResolutionEvidenceKindDto {
    pub fn as_str(self) -> &'static str {
        gateway_domain::operation::ManualResolutionEvidenceKind::from(self).as_str()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OwnerReceiptLookupStatusDto {
    #[default]
    NotApplicable,
    Found,
    NotFound,
    Unavailable,
    Mismatch,
}
impl From<gateway_domain::operation::OwnerReceiptLookupStatus> for OwnerReceiptLookupStatusDto {
    fn from(value: gateway_domain::operation::OwnerReceiptLookupStatus) -> Self {
        match value {
            gateway_domain::operation::OwnerReceiptLookupStatus::NotApplicable => {
                Self::NotApplicable
            }
            gateway_domain::operation::OwnerReceiptLookupStatus::Found => Self::Found,
            gateway_domain::operation::OwnerReceiptLookupStatus::NotFound => Self::NotFound,
            gateway_domain::operation::OwnerReceiptLookupStatus::Unavailable => Self::Unavailable,
            gateway_domain::operation::OwnerReceiptLookupStatus::Mismatch => Self::Mismatch,
        }
    }
}
impl From<OwnerReceiptLookupStatusDto> for gateway_domain::operation::OwnerReceiptLookupStatus {
    fn from(value: OwnerReceiptLookupStatusDto) -> Self {
        match value {
            OwnerReceiptLookupStatusDto::NotApplicable => Self::NotApplicable,
            OwnerReceiptLookupStatusDto::Found => Self::Found,
            OwnerReceiptLookupStatusDto::NotFound => Self::NotFound,
            OwnerReceiptLookupStatusDto::Unavailable => Self::Unavailable,
            OwnerReceiptLookupStatusDto::Mismatch => Self::Mismatch,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OperationOutcomeDto {
    Pending,
    Committed,
    RejectedBeforeCommit,
    Unknown,
}
impl From<gateway_domain::operation::OperationOutcome> for OperationOutcomeDto {
    fn from(value: gateway_domain::operation::OperationOutcome) -> Self {
        match value {
            gateway_domain::operation::OperationOutcome::Pending => Self::Pending,
            gateway_domain::operation::OperationOutcome::Committed => Self::Committed,
            gateway_domain::operation::OperationOutcome::RejectedBeforeCommit => {
                Self::RejectedBeforeCommit
            }
            gateway_domain::operation::OperationOutcome::Unknown => Self::Unknown,
        }
    }
}
impl From<OperationOutcomeDto> for gateway_domain::operation::OperationOutcome {
    fn from(value: OperationOutcomeDto) -> Self {
        match value {
            OperationOutcomeDto::Pending => Self::Pending,
            OperationOutcomeDto::Committed => Self::Committed,
            OperationOutcomeDto::RejectedBeforeCommit => Self::RejectedBeforeCommit,
            OperationOutcomeDto::Unknown => Self::Unknown,
        }
    }
}
impl OperationOutcomeDto {
    pub fn as_str(self) -> &'static str {
        gateway_domain::operation::OperationOutcome::from(self).as_str()
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualOperationResolutionResponseDto {
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
impl fmt::Debug for ManualOperationResolutionResponseDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ManualOperationResolutionResponseDto")
            .finish_non_exhaustive()
    }
}
impl From<gateway_domain::operation::ManualOperationResolution>
    for ManualOperationResolutionResponseDto
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
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OwnerReceiptStatusResponseDto {
    pub lookup_status: OwnerReceiptLookupStatusDto,
    pub receipt_state: Option<String>,
    pub scope: Option<String>,
    pub committed_at: Option<DateTime<Utc>>,
    pub replay_until: Option<DateTime<Utc>>,
}
impl fmt::Debug for OwnerReceiptStatusResponseDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnerReceiptStatusResponseDto")
            .finish_non_exhaustive()
    }
}
impl From<gateway_domain::operation::OwnerReceiptStatus> for OwnerReceiptStatusResponseDto {
    fn from(value: gateway_domain::operation::OwnerReceiptStatus) -> Self {
        Self {
            lookup_status: value.lookup_status.into(),
            receipt_state: value.receipt_state,
            scope: value.scope,
            committed_at: value.committed_at,
            replay_until: value.replay_until,
        }
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationStatusResponseDto {
    pub operation_id: Uuid,
    /// One of `IN_PROGRESS`, `QUEUED`, `COMPLETED`, or `UNKNOWN`. A separate manual
    /// disposition may be present while the immutable Gateway state is `UNKNOWN`.
    ///
    /// `state` is the Gateway journal lifecycle. Use `outcome` to distinguish a
    /// successful result, a proven pre-commit rejection, and an unresolved one.
    pub state: String,
    pub outcome: OperationOutcomeDto,
    pub action: String,
    pub resource_path: String,
    /// The recorded downstream status, present only once the operation finished.
    pub response_status: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    /// Whether the recorded response is still replayable.
    ///
    /// Replay bodies expire before their rows do, so a completed operation can be
    /// old enough that its outcome is known while its response is gone. Reporting
    /// the distinction keeps a caller from reading an absent body as an absent
    /// outcome.
    pub replayable: bool,
    /// Owner-side receipt evidence. A missing receipt is not proof that the
    /// write did not commit; callers must not use this status to retry blindly.
    #[serde(default)]
    pub owner_receipt: OwnerReceiptStatusResponseDto,
    /// Latest human decision. The underlying Gateway journal state remains
    /// unchanged; this metadata is an audited disposition, not a replay token.
    #[serde(default)]
    pub manual_resolution: Option<ManualOperationResolutionResponseDto>,
}
impl fmt::Debug for OperationStatusResponseDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OperationStatusResponseDto")
            .finish_non_exhaustive()
    }
}
impl From<gateway_domain::operation::OperationStatus> for OperationStatusResponseDto {
    fn from(value: gateway_domain::operation::OperationStatus) -> Self {
        Self {
            operation_id: value.operation_id,
            state: value.state,
            outcome: value.outcome.into(),
            action: value.action,
            resource_path: value.resource_path,
            response_status: value.response_status,
            created_at: value.created_at,
            finished_at: value.finished_at,
            replayable: value.replayable,
            owner_receipt: value.owner_receipt.into(),
            manual_resolution: value.manual_resolution.map(Into::into),
        }
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallerOperationStatusResponseDto {
    pub operation_id: Uuid,
    pub state: String,
    pub outcome: OperationOutcomeDto,
    pub response_status: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub replayable: bool,
}
impl fmt::Debug for CallerOperationStatusResponseDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CallerOperationStatusResponseDto")
            .finish_non_exhaustive()
    }
}
impl From<gateway_domain::operation::CallerOperationStatus> for CallerOperationStatusResponseDto {
    fn from(value: gateway_domain::operation::CallerOperationStatus) -> Self {
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
