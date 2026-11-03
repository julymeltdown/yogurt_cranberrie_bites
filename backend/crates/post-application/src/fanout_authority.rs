//! Owner lookup data for a claimed fanout page, not a system capability.
//! Shape checks do not prove a job exists. Only the owning repository validates
//! persisted identity, current source eligibility and lease/cursor fencing.
use crate::dto::timeline_fanout_dto::TimelineFanoutClaimDto;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FanoutAuthorityErrorDto {
    #[error("invalid fanout lookup binding")]
    InvalidBinding,
    #[error("fanout owner scope denied")]
    Denied,
    #[error("fanout owner lookup unavailable")]
    Unavailable,
}

/// Explicit owner policy. The page limit is not persisted on existing jobs;
/// it must be checked against trusted composition and later signed exactly.
#[derive(Clone, Copy)]
pub struct FanoutPagePolicyDto(i32);
impl FanoutPagePolicyDto {
    pub fn new(max_page_size: i32) -> Result<Self, FanoutAuthorityErrorDto> {
        if !(1..=100).contains(&max_page_size) {
            return Err(FanoutAuthorityErrorDto::InvalidBinding);
        }
        Ok(Self(max_page_size))
    }
    pub fn max_page_size(self) -> i32 {
        self.0
    }
}

/// Exact proposed page. A constructible claim/DTO grants no authority.
/// No Debug/Serialize implementation may expose its lease token.
#[derive(Clone)]
pub struct FanoutPageLookupDto {
    deadline: crate::delivery_effect::DeliveryEffectDeadlineDto,
    claim: TimelineFanoutClaimDto,
    page_limit: i32,
}
impl FanoutPageLookupDto {
    pub fn within(
        mut self,
        deadline: crate::delivery_effect::DeliveryEffectDeadlineDto,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            deadline.expires_at() <= self.deadline.expires_at(),
            "Invalid authority deadline"
        );
        self.deadline = deadline;
        Ok(self)
    }
    pub fn deadline(&self) -> crate::delivery_effect::DeliveryEffectDeadlineDto {
        self.deadline
    }

    pub fn new(
        claim: TimelineFanoutClaimDto,
        page_limit: i32,
    ) -> Result<Self, FanoutAuthorityErrorDto> {
        let job = &claim.job;
        if [
            job.job_id,
            job.event_id,
            job.post_id,
            job.author_id,
            claim.lease_token,
        ]
        .iter()
        .any(Uuid::is_nil)
            || job.attempt_count < 1
            || !(1..=100).contains(&page_limit)
            || !(1..=100).contains(&claim.max_page_attempts)
            || !(1..=claim.max_page_attempts).contains(&claim.page_attempt_count)
        {
            return Err(FanoutAuthorityErrorDto::InvalidBinding);
        }
        if let Some(cursor) = &job.cursor {
            let valid = cursor.len() <= 256
                && cursor.split_once('|').is_some_and(|(time, id)| {
                    DateTime::parse_from_rfc3339(time).is_ok()
                        && Uuid::parse_str(id).is_ok_and(|id| !id.is_nil())
                });
            if !valid {
                return Err(FanoutAuthorityErrorDto::InvalidBinding);
            }
        }
        let expires_at = claim.lease_expires_at;
        let budget = (expires_at - Utc::now()).to_std().unwrap_or_default();
        let deadline = crate::delivery_effect::DeliveryEffectDeadlineDto::new(
            expires_at,
            std::time::Instant::now() + budget,
        )
        .map_err(|_| FanoutAuthorityErrorDto::InvalidBinding)?;
        Ok(Self {
            claim,
            page_limit,
            deadline,
        })
    }
    pub fn claim(&self) -> &TimelineFanoutClaimDto {
        &self.claim
    }
    pub fn page_limit(&self) -> i32 {
        self.page_limit
    }
}

/// Data read from the owner, not a signed grant. A later independent issuer
/// must authenticate the lookup and bind purpose, presenter and exact child.
#[derive(Clone)]
pub struct OwnedFanoutPageDto {
    page: FanoutPageLookupDto,
    checked_at: DateTime<Utc>,
}
impl OwnedFanoutPageDto {
    /// Persistence adapter projection only. This validates shape and expiry;
    /// the repository must first compare every binding to authoritative rows.
    pub fn from_persisted_claim(
        claim: TimelineFanoutClaimDto,
        page_limit: i32,
        checked_at: DateTime<Utc>,
    ) -> Result<Self, FanoutAuthorityErrorDto> {
        if claim.lease_expires_at <= checked_at {
            return Err(FanoutAuthorityErrorDto::Denied);
        }
        Ok(Self {
            page: FanoutPageLookupDto::new(claim, page_limit)?,
            checked_at,
        })
    }
    pub fn page(&self) -> &FanoutPageLookupDto {
        &self.page
    }
    pub fn checked_at(&self) -> DateTime<Utc> {
        self.checked_at
    }
    pub fn expires_at(&self) -> DateTime<Utc> {
        self.page.claim.lease_expires_at
    }
}

#[async_trait]
pub trait FanoutAuthorityRepositoryPort: Send + Sync {
    /// Never claims/renews a job or advances its page. The database clock is
    /// checked after row waits; denial cannot select workload-only authority.
    async fn resolve_claimed_page(
        &self,
        request: &FanoutPageLookupDto,
    ) -> Result<OwnedFanoutPageDto, FanoutAuthorityErrorDto>;
}
#[async_trait]
impl<T: FanoutAuthorityRepositoryPort + ?Sized> FanoutAuthorityRepositoryPort for Arc<T> {
    async fn resolve_claimed_page(
        &self,
        request: &FanoutPageLookupDto,
    ) -> Result<OwnedFanoutPageDto, FanoutAuthorityErrorDto> {
        (**self).resolve_claimed_page(request).await
    }
}

/// Minimal owner-filtered projection data. Construction is validation, not
/// authority; the dedicated adapter must authenticate and authorize its RPC.
pub struct ClaimedFanoutPageDto {
    expires_at: DateTime<Utc>,
    author_id: Uuid,
    follower_ids: Vec<Uuid>,
    next_cursor: Option<String>,
}
impl ClaimedFanoutPageDto {
    pub fn new(
        request: &FanoutPageLookupDto,
        author_id: Uuid,
        follower_ids: Vec<Uuid>,
        next_cursor: Option<String>,
        expires_at: DateTime<Utc>,
    ) -> anyhow::Result<Self> {
        let page = Self {
            expires_at,
            author_id,
            follower_ids,
            next_cursor,
        };
        page.validate(request)?;
        Ok(page)
    }
    pub fn validate(&self, request: &FanoutPageLookupDto) -> anyhow::Result<()> {
        if let Some(cursor) = &self.next_cursor {
            // The dedicated wire path must also satisfy Friend's actual
            // cursor bound. Do not persist a page boundary that the next
            // claimed read will reject after permissive normalization.
            anyhow::ensure!(
                cursor.len() <= 128 && cursor.trim() == cursor,
                "Invalid claimed fanout cursor"
            );
        }
        let mut seen = std::collections::HashSet::new();
        anyhow::ensure!(
            self.expires_at <= request.deadline().expires_at()
                && Utc::now() < self.expires_at
                && self.author_id == request.claim().job.author_id
                && self.follower_ids.len() <= request.page_limit() as usize
                && self
                    .follower_ids
                    .iter()
                    .all(|id| !id.is_nil() && *id != self.author_id && seen.insert(*id)),
            "Invalid claimed fanout response"
        );
        crate::dto::timeline_fanout_dto::validate_fanout_page_progress(
            request.claim().job.cursor.as_deref(),
            &crate::dto::timeline_fanout_dto::TimelineFanoutPageDto {
                follower_ids: self.follower_ids.clone(),
                next_cursor: self.next_cursor.clone(),
                // Only page progression is checked here. Grant/lease expiry
                // must never supply timeline-entry retention.
                expires_at: request.claim().job.post_created_at,
            },
        )
    }
    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }
    pub fn into_parts(self) -> (Vec<Uuid>, Option<String>, DateTime<Utc>) {
        (self.follower_ids, self.next_cursor, self.expires_at)
    }
}

#[async_trait]
pub trait ClaimedFanoutReadPort: Send + Sync {
    /// Required explicit system path; failures cannot fall back to a general
    /// Friend read or change the persisted lease/attempt budget.
    async fn fetch_claimed_page(
        &self,
        request: &FanoutPageLookupDto,
    ) -> anyhow::Result<ClaimedFanoutPageDto>;
}

#[async_trait]
pub trait ClaimedFanoutApplyPort: Send + Sync {
    async fn apply_claimed_page(
        &self,
        claim: &TimelineFanoutClaimDto,
        page: &crate::dto::timeline_fanout_dto::TimelineFanoutPageDto,
        deadline: crate::delivery_effect::DeliveryEffectDeadlineDto,
    ) -> anyhow::Result<crate::dto::timeline_fanout_dto::TimelineFanoutOutcomeDto>;
}
