//! Verification disconnect: revoke the live claim and touch nothing else.
//!
//! Adapted from Bigyeting `5eecdcaf`
//! `adapters/out/repository/naver.rs` (`revoke_naver_verification`) and
//! `adapters/in/http/naver_verification.rs` (the `DELETE /v1/naver-verification`
//! handler).
//!
//! The effect is exactly one statement: the user's live claim becomes `REVOKED`
//! with `expires_at` capped at the revocation instant. What that means for the
//! rest of the product is a consequence, not an action:
//!
//! * the user's own restricted intro posts keep their audience, because the
//!   audience column is on the post and this function never reads or writes it;
//! * rooms, profile posts, favorites and media are untouched;
//! * the user's own attributes become `Attribute::Unavailable`, so they can no
//!   longer satisfy another author's `female` / `male` / `unknown` audience.
//!
//! Revocation is idempotent: a second call finds no `VERIFIED` row and changes
//! nothing, and the status read returns the same `REVOKED` state. That is the
//! donor's `where status in ('verified', 'expired')` narrowed to `VERIFIED`,
//! because a `REPLACED` row is already not live and must not be rewritten.
use crate::identity::repository::IdentityRepository;
use crate::identity::status::{IdentityVerification, ReadVerification};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// The disconnect use case.
pub struct DisconnectVerification {
    repository: IdentityRepository,
    reader: ReadVerification,
}

impl DisconnectVerification {
    pub fn new(repository: IdentityRepository) -> Self {
        Self {
            reader: ReadVerification::new(repository.clone()),
            repository,
        }
    }

    /// Revoke the live claim and return the resulting status.
    ///
    /// A store failure is reported as `PROVIDER_UNAVAILABLE` rather than a
    /// fabricated `REVOKED`: claiming a revocation that did not happen would be
    /// worse than reporting that the store is unavailable.
    pub async fn disconnect(&self, user_id: Uuid, now: DateTime<Utc>) -> IdentityVerification {
        match self.repository.revoke(user_id, now).await {
            Ok(_) => self.reader.read(user_id, now).await,
            Err(_) => IdentityVerification::provider_unavailable(),
        }
    }
}
