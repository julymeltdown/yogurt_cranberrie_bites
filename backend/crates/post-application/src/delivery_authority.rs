//! Persisted delivery ownership data, never a signed system capability.
//! A later purpose-specific lookup must still prove projection/recipient scope.
use crate::dto::delivery_schedule_dto::TargetDeliveryClaimDto;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use post_domain::post::DeliveryAttemptSource;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DeliveryAuthorityErrorDto {
    #[error("invalid delivery lookup binding")]
    InvalidBinding,
    #[error("delivery owner scope denied")]
    Denied,
    #[error("delivery owner lookup unavailable")]
    Unavailable,
}

/// Construction checks shape only; it cannot establish persisted ownership.
/// No Debug or serialization may expose the event payload or lease token.
#[derive(Clone)]
pub struct DeliveryClaimLookupDto(TargetDeliveryClaimDto);
impl DeliveryClaimLookupDto {
    pub fn new(claim: TargetDeliveryClaimDto) -> Result<Self, DeliveryAuthorityErrorDto> {
        if [
            claim.event.event_id,
            claim.event.aggregate_id,
            claim.lease_token,
            claim.attempt_id,
        ]
        .iter()
        .any(Uuid::is_nil)
            || claim.attempt_number < 0
            || claim.event.event_type.is_empty()
            || claim.event.aggregate_version.is_some_and(|v| v < 1)
        {
            return Err(DeliveryAuthorityErrorDto::InvalidBinding);
        }
        Ok(Self(claim))
    }
    pub fn claim(&self) -> &TargetDeliveryClaimDto {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryReplayRestoreDto {
    Pending,
    Retry,
    Succeeded,
    Skipped,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryLeaseKindDto {
    Automatic,
    Replay(DeliveryReplayRestoreDto),
}

/// Stored scheduling provenance. Neither callers nor this DTO can reset budget.
#[derive(Clone, Copy)]
pub struct DeliveryAttemptProvenanceDto {
    pub lease_kind: DeliveryLeaseKindDto,
    pub source: DeliveryAttemptSource,
    pub resolution_id: Option<Uuid>,
    pub max_attempts: i32,
    pub progress_credits: i32,
    pub target_required: bool,
}

#[derive(Clone)]
pub struct OwnedDeliveryClaimDto {
    lookup: DeliveryClaimLookupDto,
    provenance: DeliveryAttemptProvenanceDto,
    checked_at: DateTime<Utc>,
}
impl OwnedDeliveryClaimDto {
    /// Adapter projection only: the repository must first check authoritative
    /// rows and immutable STARTED provenance. This is data, not admission proof.
    pub fn from_persisted_claim(
        claim: TargetDeliveryClaimDto,
        provenance: DeliveryAttemptProvenanceDto,
        checked_at: DateTime<Utc>,
    ) -> Result<Self, DeliveryAuthorityErrorDto> {
        use DeliveryAuthorityErrorDto::Denied;
        let lookup = DeliveryClaimLookupDto::new(claim)?;
        let claim = lookup.claim();
        let completed_replay = matches!(
            provenance.lease_kind,
            DeliveryLeaseKindDto::Replay(
                DeliveryReplayRestoreDto::Succeeded | DeliveryReplayRestoreDto::Skipped
            )
        );
        let charged = claim
            .attempt_number
            .checked_sub(provenance.progress_credits)
            .ok_or(Denied)?;
        if claim.lease_expires_at <= checked_at
            || !(1..=1000).contains(&provenance.max_attempts)
            || !(0..=claim.attempt_number).contains(&provenance.progress_credits)
            || (!completed_replay && !(1..=provenance.max_attempts).contains(&charged))
            || (claim.target != post_domain::post::DeliveryTarget::Timeline
                && provenance.progress_credits != 0)
            || (matches!(provenance.lease_kind, DeliveryLeaseKindDto::Automatic)
                && !provenance.target_required)
            || (matches!(provenance.lease_kind, DeliveryLeaseKindDto::Replay(_))
                && provenance.source != DeliveryAttemptSource::AdminReplay)
            || (matches!(provenance.lease_kind, DeliveryLeaseKindDto::Replay(_))
                && !matches!(
                    claim.target,
                    post_domain::post::DeliveryTarget::Internal
                        | post_domain::post::DeliveryTarget::External
                ))
            || (provenance.source == DeliveryAttemptSource::OutboxWorker
                && provenance.resolution_id.is_some())
            || (matches!(provenance.lease_kind, DeliveryLeaseKindDto::Automatic)
                && provenance.source == DeliveryAttemptSource::AdminReplay
                && provenance.resolution_id.is_none())
        {
            return Err(Denied);
        }
        Ok(Self {
            lookup,
            provenance,
            checked_at,
        })
    }
    pub fn lookup(&self) -> &DeliveryClaimLookupDto {
        &self.lookup
    }
    pub fn provenance(&self) -> DeliveryAttemptProvenanceDto {
        self.provenance
    }
    pub fn checked_at(&self) -> DateTime<Utc> {
        self.checked_at
    }
    pub fn expires_at(&self) -> DateTime<Utc> {
        self.lookup.claim().lease_expires_at
    }
}

#[async_trait]
pub trait DeliveryAuthorityRepositoryPort: Send + Sync {
    /// Read only. No claim, renewal, completion, retry, replay or publication.
    /// Does not authorize a predecessor, live Post or notification recipient.
    async fn resolve_claimed_delivery(
        &self,
        request: &DeliveryClaimLookupDto,
    ) -> Result<OwnedDeliveryClaimDto, DeliveryAuthorityErrorDto>;
}
#[async_trait]
impl<T: DeliveryAuthorityRepositoryPort + ?Sized> DeliveryAuthorityRepositoryPort for Arc<T> {
    async fn resolve_claimed_delivery(
        &self,
        request: &DeliveryClaimLookupDto,
    ) -> Result<OwnedDeliveryClaimDto, DeliveryAuthorityErrorDto> {
        (**self).resolve_claimed_delivery(request).await
    }
}
