//! Private queue and payload transfers, independent of encryption/database APIs.
use super::journal_write_dto::JournalOutcomeEvidenceDto;
use super::operation_view_dto::OperationOutcomeDto;
use chrono::{DateTime, Utc};
use std::{fmt, time::Duration};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq)]
pub struct EncryptedQueuePayloadDto {
    pub key_id: i16,
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}
impl fmt::Debug for EncryptedQueuePayloadDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EncryptedQueuePayloadDto")
            .field("key_id", &self.key_id)
            .field("ciphertext_bytes", &self.ciphertext.len())
            .finish_non_exhaustive()
    }
}
pub struct QueuePayloadEncryptDto<'a> {
    pub operation_id: Uuid,
    pub actor_id: &'a str,
    pub request_fingerprint: &'a str,
    pub plaintext: &'a [u8],
}
pub struct QueuePayloadDecryptDto<'a> {
    pub operation_id: Uuid,
    pub actor_id: &'a str,
    pub request_fingerprint: &'a str,
    pub payload: &'a EncryptedQueuePayloadDto,
}
pub struct QueuePlaintextPayloadDto {
    pub body: Vec<u8>,
}
/// Compatibility input for callers that prepare an encrypted enqueue command.
pub struct QueueEnqueueRequestDto<'a> {
    pub operation_id: Uuid,
    pub actor_id: &'a str,
    pub request_fingerprint: &'a str,
    pub request_uri: &'a str,
    pub content_type: Option<&'a str>,
    pub body: &'a [u8],
}
pub struct QueueEnqueueCommandDto<'a> {
    pub operation_id: Uuid,
    pub actor_id: &'a str,
    pub request_fingerprint: &'a str,
    pub request_uri: &'a str,
    pub content_type: Option<&'a str>,
    pub payload: EncryptedQueuePayloadDto,
    pub origin_traceparent: Option<String>,
}
#[derive(Clone)]
pub struct ClaimedQueueOperationDto {
    pub operation_id: Uuid,
    pub actor_id: String,
    pub action: String,
    pub resource_path: String,
    pub request_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub idempotency_key: String,
    pub request_fingerprint: String,
    pub request_uri: String,
    pub content_type: Option<String>,
    pub origin_traceparent: Option<String>,
    pub lease_owner: Uuid,
    pub attempt_count: u32,
    pub payload: EncryptedQueuePayloadDto,
}
impl fmt::Debug for ClaimedQueueOperationDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClaimedQueueOperationDto")
            .field("operation_id", &self.operation_id)
            .field("attempt_count", &self.attempt_count)
            .finish_non_exhaustive()
    }
}
impl ClaimedQueueOperationDto {
    pub fn lease_identity(&self) -> QueueLeaseIdentityDto {
        QueueLeaseIdentityDto {
            operation_id: self.operation_id,
            lease_owner: self.lease_owner,
        }
    }
    pub fn decrypt_body(
        &self,
        payload: &dyn crate::queue_operation_ports::QueuePayloadPort,
    ) -> crate::queue_operation_error::QueueOperationResult<Vec<u8>> {
        payload
            .decrypt(QueuePayloadDecryptDto {
                operation_id: self.operation_id,
                actor_id: &self.actor_id,
                request_fingerprint: &self.request_fingerprint,
                payload: &self.payload,
            })
            .map(|plaintext| plaintext.body)
    }
}
pub struct QueueClaimQueryDto {
    pub worker_id: Uuid,
    pub lease_duration: Duration,
}
#[derive(Clone, Copy)]
pub struct QueueLeaseIdentityDto {
    pub operation_id: Uuid,
    pub lease_owner: Uuid,
}
pub struct QueueRetryCommandDto<'a> {
    pub lease: QueueLeaseIdentityDto,
    pub available_at: DateTime<Utc>,
    pub error_code: &'a str,
}
pub struct QueueDeadCommandDto<'a> {
    pub lease: QueueLeaseIdentityDto,
    pub error_code: &'a str,
}
pub struct QueueCompleteCommandDto<'a> {
    pub lease: QueueLeaseIdentityDto,
    pub response_status: u16,
    pub outcome: OperationOutcomeDto,
    pub evidence: JournalOutcomeEvidenceDto,
    pub content_type: Option<&'a str>,
    pub response_body: Option<&'a [u8]>,
}
pub struct QueueDeadClaimCommandDto<'a> {
    pub lease: QueueLeaseIdentityDto,
    pub error_code: &'a str,
    pub response_status: u16,
}
pub struct JournalMaintenanceCommandDto {
    pub stale_operation_age_seconds: u64,
}
