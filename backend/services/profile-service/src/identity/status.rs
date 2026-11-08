//! Verification status read: the owner's own view of their claim.
//!
//! Adapted from Bigyeting `5eecdcaf`
//! `adapters/out/repository/naver.rs` (`naver_verification_response_at`) and
//! `application/dto/api` (`NaverVerificationResponse`, `NaverClaims`,
//! `NaverVerificationState`).
//!
//! The donor's read model is preserved:
//!
//! * the canonical row is selected with the donor's ordering (newest
//!   `verified_at`, then newest `expires_at`, then newest `claim_id`);
//! * the current age is computed with the Seoul-date rule, never stored;
//! * `claims` is present **only** when the status is `VERIFIED`. Every other
//!   status carries `claims: null`, so an expired or revoked claim can never be
//!   read as an attribute by any consumer of this DTO.
//!
//! The destination contract (`contracts/core/v1/core.schema.json`
//! `core.IdentityVerification`) adds two things the donor did not have:
//!
//! * `provider`, so a second provider can be added without a contract change;
//! * `display_on_profile`, which the donor's `NaverAccountStatus` implied by
//!   always showing the claim. Here it is an explicit `false` until a profile
//!   display setting exists; the value is not a claim attribute and never
//!   affects an audience decision.
use crate::identity::repository::{
    seoul_date, ClaimRow, ClaimStatus, GenderBucket, IdentityRepository,
};
use chrono::{DateTime, SecondsFormat, Utc};
use uuid::Uuid;

/// The provider name in the DTO. A closed enum with one value today.
pub const PROVIDER_NAVER: &str = "naver";

/// The status values `core.IdentityVerification.status` admits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationState {
    NotVerified,
    Verified,
    Expired,
    Revoked,
    ProviderUnavailable,
}

impl VerificationState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotVerified => "NOT_VERIFIED",
            Self::Verified => "VERIFIED",
            Self::Expired => "EXPIRED",
            Self::Revoked => "REVOKED",
            Self::ProviderUnavailable => "PROVIDER_UNAVAILABLE",
        }
    }
}

/// The owner-only claim summary. `core.IdentityClaims`.
///
/// `Debug` is hand written and redacted: the age is the one value V-12-o keeps
/// out of logs, and a derived `Debug` would put it in every `tracing` field that
/// ever formats the DTO.
pub struct IdentityClaims {
    pub age: i16,
    pub gender_bucket: GenderBucket,
}

impl std::fmt::Debug for IdentityClaims {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IdentityClaims")
            .field("age", &"[REDACTED]")
            .field("gender_bucket", &self.gender_bucket)
            .finish()
    }
}

/// `core.IdentityVerification`.
pub struct IdentityVerification {
    pub provider: &'static str,
    pub status: VerificationState,
    pub verified_at: Option<String>,
    pub expires_at: Option<String>,
    pub claims: Option<IdentityClaims>,
    pub display_on_profile: bool,
}

impl IdentityVerification {
    /// The `NOT_VERIFIED` shape: no timestamps, no claims.
    pub fn not_verified() -> Self {
        Self {
            provider: PROVIDER_NAVER,
            status: VerificationState::NotVerified,
            verified_at: None,
            expires_at: None,
            claims: None,
            display_on_profile: false,
        }
    }

    /// The `PROVIDER_UNAVAILABLE` shape. Used when the store could not be read:
    /// an unavailable claim is never reported as a live one.
    pub fn provider_unavailable() -> Self {
        Self {
            provider: PROVIDER_NAVER,
            status: VerificationState::ProviderUnavailable,
            verified_at: None,
            expires_at: None,
            claims: None,
            display_on_profile: false,
        }
    }

    /// The JSON representation of `core.IdentityVerification`.
    ///
    /// The contract has `additionalProperties: false` and every field required,
    /// so this is the only shape the gateway will accept.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "provider": self.provider,
            "status": self.status.as_str(),
            "verified_at": self.verified_at,
            "expires_at": self.expires_at,
            "claims": self.claims.as_ref().map(|claims| serde_json::json!({
                "age": claims.age,
                "gender_bucket": claims.gender_bucket.as_str(),
            })),
            "display_on_profile": self.display_on_profile,
        })
    }
}

/// Map one canonical claim row to the DTO at `now`.
///
/// The donor's `case when status = 'verified' and not (supported) then 'expired'
/// else status end`, with the same three outcomes plus the destination's
/// `REVOKED` and `NOT_VERIFIED`.
pub fn project(row: Option<ClaimRow>, now: DateTime<Utc>) -> IdentityVerification {
    let Some(row) = row else {
        return IdentityVerification::not_verified();
    };
    let seoul_today = seoul_date(now);
    let (status, claims) = match row.status {
        ClaimStatus::Verified => {
            if row.is_usable(now, seoul_today) {
                // `is_usable` guarantees both attributes are present and the
                // current age is inside 14..=100, so these unwraps are total.
                let age = row.current_age(seoul_today).unwrap_or_default();
                (
                    VerificationState::Verified,
                    Some(IdentityClaims {
                        age,
                        gender_bucket: row.gender_bucket,
                    }),
                )
            } else {
                // A VERIFIED row past its expiry reads as EXPIRED. The row is
                // not rewritten: a read must not mutate, and the expiry is
                // already recorded.
                (VerificationState::Expired, None)
            }
        }
        ClaimStatus::Revoked => (VerificationState::Revoked, None),
        // A REPLACED row is only ever the canonical row when a newer row has
        // not been written yet, which cannot happen: the replacement is written
        // in the same transaction that replaces this one. If it is seen, the
        // user has no live claim, and `NOT_VERIFIED` is the honest answer.
        ClaimStatus::Replaced => (VerificationState::NotVerified, None),
    };
    IdentityVerification {
        provider: PROVIDER_NAVER,
        status,
        verified_at: Some(format_timestamp(row.verified_at)),
        expires_at: Some(format_timestamp(row.expires_at)),
        claims,
        display_on_profile: false,
    }
}

/// The RFC 3339 UTC rendering the contract's `UtcTimestamp` requires: second
/// precision, `Z` suffix.
fn format_timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// The status read use case.
pub struct ReadVerification {
    repository: IdentityRepository,
}

impl ReadVerification {
    pub fn new(repository: IdentityRepository) -> Self {
        Self { repository }
    }

    /// Read the owner's verification. A store failure is
    /// `PROVIDER_UNAVAILABLE`, never a fabricated live claim.
    pub async fn read(&self, user_id: Uuid, now: DateTime<Utc>) -> IdentityVerification {
        match self.repository.canonical_claim(user_id).await {
            Ok(row) => project(row, now),
            Err(_) => IdentityVerification::provider_unavailable(),
        }
    }
}
