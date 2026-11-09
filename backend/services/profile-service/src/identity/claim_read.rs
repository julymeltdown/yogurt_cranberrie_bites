//! The internal claim read: the port behind `IdentityClaimsPort`.
//!
//! Contract: [identity_claim_reader.proto](../../../../proto/profile/v1/identity_claim_reader.proto)
//! and [domain-contract.md §1.3](../../../../docs/mcr/integration/domain-contract.md).
//!
//! The concert service owns the Introduction audience decision but does not own
//! the claim store. This module is the profile service's side of that split: it
//! reads the canonical claim row for one profile and reports the raw facts, and
//! it makes no decision at all.
//!
//! Three properties are structural:
//!
//! * **The subject is a profile, not an Auth UUID.** The caller names a
//!   `profile_id`; this module resolves it to the account through
//!   `profile_identities` itself. Neither service has to trust the other's text
//!   for the mapping, and a caller cannot read a claim for an arbitrary account.
//! * **The evaluation instant is the caller's.** The caller passes the instant
//!   its decision is evaluated at, and this module reports whether the row is
//!   live *at that instant*. That is what keeps "the claim expires at `now`" a
//!   single, testable boundary instead of two clocks that can disagree.
//! * **No attribute is rendered.** The reply carries the derived claim facts
//!   (the age at verification and the next birthday), never a current age and
//!   never a birth date. The caller computes the current age with the Seoul-date
//!   arithmetic it already imports.
use crate::identity::repository::{seoul_date, ClaimStatus, GenderBucket, IdentityRepository};
use chrono::{DateTime, SecondsFormat, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// The canonical claim row, as the audience decision needs it.
///
/// `Debug` is hand written and redacted for the same reason `ClaimRow`'s is: the
/// derived age is private input and must not reach a log through a formatted
/// value.
pub struct ClaimObservation {
    pub status: ClaimStatus,
    pub gender_bucket: GenderBucket,
    pub age_at_verification: Option<i16>,
    pub next_birthday_at: Option<chrono::NaiveDate>,
    pub verifier: String,
    pub source_revision: String,
    pub verified_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    /// Whether the row is a live claim at the evaluation instant.
    pub live: bool,
}

impl std::fmt::Debug for ClaimObservation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ClaimObservation")
            .field("status", &self.status)
            .field("gender_bucket", &self.gender_bucket)
            .field("age_at_verification", &"[REDACTED]")
            .field("next_birthday_at", &"[REDACTED]")
            .field("verifier", &self.verifier)
            .field("source_revision", &self.source_revision)
            .field("verified_at", &self.verified_at)
            .field("expires_at", &self.expires_at)
            .field("live", &self.live)
            .finish()
    }
}

/// The outcome of one claim read.
#[derive(Debug)]
pub enum ClaimRead {
    /// The subject has a claim row.
    Observed(ClaimObservation),
    /// The subject has no claim row, or the profile does not exist. Both are
    /// "no claim", and the caller maps both to `Attribute::Unavailable`.
    None,
}

/// Why a claim read could not answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimReadError {
    /// The profile id was not a canonical non-nil UUID.
    InvalidProfile,
    /// The store could not be read.
    Unavailable,
}

/// Read the canonical claim for one profile at one instant.
///
/// The row is selected with the store's own canonical ordering (newest
/// `verified_at`, then newest `expires_at`, then newest `claim_id`), so an older
/// usable claim is never returned over a newer expired one. Liveness is
/// evaluated with the store's own `is_usable` rule.
pub struct ReadClaim {
    pool: PgPool,
    repository: IdentityRepository,
}

impl ReadClaim {
    pub fn new(pool: PgPool, repository: IdentityRepository) -> Self {
        Self { pool, repository }
    }

    pub async fn read(
        &self,
        profile_id: &str,
        evaluated_at: DateTime<Utc>,
    ) -> Result<ClaimRead, ClaimReadError> {
        let profile = Uuid::parse_str(profile_id)
            .ok()
            .filter(|id| !id.is_nil() && id.to_string() == profile_id)
            .ok_or(ClaimReadError::InvalidProfile)?;
        // Resolve the profile to its account here, in this service's own
        // database. A profile that does not exist has no claim.
        let account: Option<Uuid> = sqlx::query_scalar(
            "select account_id from profile_identities \
             where profile_id = $1 and deleted_at is null",
        )
        .bind(profile)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| ClaimReadError::Unavailable)?;
        let Some(account) = account else {
            return Ok(ClaimRead::None);
        };
        let row = self
            .repository
            .canonical_claim_with_source(account)
            .await
            .map_err(|_| ClaimReadError::Unavailable)?;
        let Some(row) = row else {
            return Ok(ClaimRead::None);
        };
        let seoul_today = seoul_date(evaluated_at);
        let live = row.row.is_usable(evaluated_at, seoul_today);
        Ok(ClaimRead::Observed(ClaimObservation {
            status: row.row.status,
            gender_bucket: row.row.gender_bucket,
            age_at_verification: row.row.age_at_verification,
            next_birthday_at: row.row.next_birthday_at,
            verifier: row.verifier,
            source_revision: row.source_revision,
            verified_at: row.row.verified_at,
            expires_at: row.row.expires_at,
            live,
        }))
    }
}

/// The RFC 3339 UTC rendering the contract's timestamp fields require: second
/// precision, `Z` suffix.
pub fn format_timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}
