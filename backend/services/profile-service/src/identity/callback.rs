//! Verification callback: consume the state once, validate in order, store.
//!
//! Adapted from Bigyeting `5eecdcaf`
//! `application/use_cases/naver_verification.rs` (`complete_naver_verification`,
//! `validated_claim`, `NaverCallbackResult`, `NaverCallbackRedirect`).
//!
//! The donor's validation order is preserved exactly, because the order is the
//! contract:
//!
//! 1. **subject present** — no subject, no claim;
//! 2. **gender present** — a missing or unrecognized provider gender is
//!    `profile_incomplete`, *never* coerced to `unknown`;
//! 3. **derive age** — `derive_exact_age_claim` from the already-adapted
//!    arithmetic in `concert_domain::age_policy::arithmetic`, with its own
//!    `age_not_supported` outcome for an age outside 14..=100;
//! 4. **store** — and only then is a claim row written.
//!
//! Every failure returns a redirect to the stored `return_path` with a result
//! marker. The marker is a closed string; it carries no claim value, no provider
//! code and no age. That is what makes the redirect safe to put in a browser
//! history entry.
//!
//! What changed:
//!
//! * the donor's `draft_key` coupling to the intro composer is gone. There is no
//!   draft to resume, and the task document explicitly forbids auto-submitting
//!   the work the user was doing before verification.
//! * the donor's `Duplicate` result is kept as its own outcome, because "this
//!   NAVER account already backs another profile" is a different message from
//!   "your profile is incomplete".
//! * the state is consumed inside one transaction that also writes the claim, so
//!   a crash between the two cannot leave a consumed state with no claim or a
//!   claim with a reusable state.
use crate::identity::naver_provider::{NaverProviderClient, NaverProviderError};
use crate::identity::repository::{
    seoul_date, ConsumeStateError, GenderBucket, IdentityRepository, NewClaim, StoreClaimOutcome,
};
use chrono::{DateTime, Utc};
use concert_domain::age_policy::arithmetic::{
    derive_exact_age_claim, ExactAgeError, SeoulDate,
};

/// The result marker appended to the redirect as `?identity=<marker>`.
///
/// The donor's `NaverCallbackResult`, minus the draft coupling. The values are
/// the donor's own strings, so the browser-side vocabulary is unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallbackResult {
    Success,
    Duplicate,
    ProfileIncomplete,
    AgeNotSupported,
    Failed,
    Cancelled,
    Expired,
    Replayed,
}

impl CallbackResult {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Duplicate => "duplicate",
            Self::ProfileIncomplete => "profile_incomplete",
            Self::AgeNotSupported => "age_not_supported",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Expired => "expired",
            Self::Replayed => "replayed",
        }
    }

    /// Whether the outcome means the user now holds a live claim.
    pub const fn is_success(self) -> bool {
        matches!(self, Self::Success)
    }
}

/// The callback outcome the gateway turns into a `303`.
pub struct CallbackOutcome {
    /// Always a same-origin path from the start allowlist, because it is the
    /// value stored at start time and never a request value.
    pub return_path: String,
    pub result: CallbackResult,
}

/// The inputs the provider sent back.
pub struct CallbackCommand {
    pub state: String,
    /// `None` when the provider reported an error instead of a code.
    pub code: Option<String>,
    /// The provider's `error` query parameter, when present.
    pub provider_error: Option<String>,
    pub now: DateTime<Utc>,
}

/// Why the callback could not produce a redirect at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallbackError {
    /// The state is unknown, expired, or already used. The gateway answers with
    /// a closed `400` and reveals nothing about which of those it was.
    InvalidState,
    /// The store failed. The state may or may not have been consumed; the
    /// gateway answers with a closed dependency error.
    Store,
}

/// The callback use case.
pub struct CompleteVerification {
    repository: IdentityRepository,
    provider: NaverProviderClient,
}

impl CompleteVerification {
    pub fn new(repository: IdentityRepository, provider: NaverProviderClient) -> Self {
        Self {
            repository,
            provider,
        }
    }

    /// Complete one verification.
    pub async fn complete(&self, command: CallbackCommand) -> Result<CallbackOutcome, CallbackError> {
        let digest = crate::identity::start::state_digest(&command.state);
        // One transaction for the whole callback. The state is reserved first,
        // so a provider timeout cannot leave it reusable, and the claim write
        // commits with the state's completion or not at all.
        let mut tx = self
            .repository
            .pool()
            .begin()
            .await
            .map_err(|_| CallbackError::Store)?;
        let consumed = self
            .repository
            .consume_state(&mut tx, digest, command.now)
            .await
            .map_err(|_| CallbackError::Store)?;
        let consumed = match consumed {
            Ok(consumed) => consumed,
            Err(ConsumeStateError::Expired) => {
                // The row exists and has expired. Its `return_path` is still the
                // only safe redirect target, so the user lands back where they
                // started with an "expired" marker instead of a dead end.
                let return_path = self
                    .expired_return_path(digest)
                    .await
                    .unwrap_or_else(|| crate::identity::start::DEFAULT_RETURN_PATH.to_owned());
                tx.rollback().await.ok();
                return Ok(CallbackOutcome {
                    return_path,
                    result: CallbackResult::Expired,
                });
            }
            Err(ConsumeStateError::Unknown) => {
                tx.rollback().await.ok();
                return Err(CallbackError::InvalidState);
            }
            Err(ConsumeStateError::NotPending) => {
                // A row that is RESERVED, EXCHANGING or DONE is a state already
                // in use. The donor reported this as `replayed`, which is the
                // same refusal expressed with a redirect the user can act on.
                let return_path = self
                    .reserved_return_path(digest)
                    .await
                    .unwrap_or_else(|| crate::identity::start::DEFAULT_RETURN_PATH.to_owned());
                tx.rollback().await.ok();
                return Ok(CallbackOutcome {
                    return_path,
                    result: CallbackResult::Replayed,
                });
            }
        };
        let return_path = consumed.return_path.clone();

        // The provider may report an error instead of a code. A user who
        // cancelled consent gets `cancelled`; anything else is `failed`. Neither
        // writes a claim.
        if let Some(provider_error) = command.provider_error.as_deref() {
            tx.rollback().await.ok();
            return Ok(CallbackOutcome {
                return_path,
                result: if provider_error == "access_denied" {
                    CallbackResult::Cancelled
                } else {
                    CallbackResult::Failed
                },
            });
        }
        let Some(code) = command.code.as_deref().filter(|code| !code.trim().is_empty()) else {
            tx.rollback().await.ok();
            return Ok(CallbackOutcome {
                return_path,
                result: CallbackResult::Failed,
            });
        };

        // The provider calls happen while the state is reserved. A failure here
        // is a `failed` redirect, not an error: the user is back on the page
        // they came from and can try again with a new state.
        let token = match self
            .provider
            .exchange_authorization_code(code, &command.state)
            .await
        {
            Ok(token) => token,
            Err(error) => {
                log_provider_failure(&error);
                tx.rollback().await.ok();
                return Ok(CallbackOutcome {
                    return_path,
                    result: CallbackResult::Failed,
                });
            }
        };
        let profile = match self.provider.load_profile(&token).await {
            Ok(profile) => profile,
            Err(error) => {
                log_provider_failure(&error);
                tx.rollback().await.ok();
                return Ok(CallbackOutcome {
                    return_path,
                    result: CallbackResult::Failed,
                });
            }
        };

        // The donor's `validated_claim`, in the donor's order.
        let validated = match validated_claim(&profile, command.now) {
            Ok(validated) => validated,
            Err(result) => {
                tx.rollback().await.ok();
                return Ok(CallbackOutcome {
                    return_path,
                    result,
                });
            }
        };

        let subject_hash = self.repository.subject_hash(&validated.subject);
        let outcome = self
            .repository
            .store_claim(
                &mut tx,
                NewClaim {
                    user_id: consumed.user_id,
                    subject_hash,
                    gender_bucket: validated.gender,
                    age_at_verification: validated.age_at_verification,
                    next_birthday_at: validated.next_birthday_at,
                    verified_at: command.now,
                },
            )
            .await
            .map_err(|error| {
                // The database error is logged by kind only: it can name a
                // constraint, never a claim value.
                tracing::warn!(error = %error, "identity claim write failed");
                CallbackError::Store
            })?;
        let result = match outcome {
            StoreClaimOutcome::Stored { .. } => {
                self.repository
                    .complete_state(&mut tx, consumed.verification_id, command.now)
                    .await
                    .map_err(|error| {
                        tracing::warn!(error = %error, "identity state completion failed");
                        CallbackError::Store
                    })?;
                CallbackResult::Success
            }
            StoreClaimOutcome::DuplicateSubject => CallbackResult::Duplicate,
        };
        tx.commit().await.map_err(|_| CallbackError::Store)?;
        Ok(CallbackOutcome {
            return_path,
            result,
        })
    }

    /// The stored return path for a state that exists but has expired.
    async fn expired_return_path(&self, digest: [u8; 32]) -> Option<String> {
        sqlx::query_scalar(
            "select return_path from identity_verifications \
             where verifier = $1 and state_digest = $2 and state in ('PENDING','EXPIRED')",
        )
        .bind(crate::identity::repository::VERIFIER_NAVER)
        .bind(digest.as_slice())
        .fetch_optional(self.repository.pool())
        .await
        .ok()
        .flatten()
    }

    /// The stored return path for a state that is already in use.
    async fn reserved_return_path(&self, digest: [u8; 32]) -> Option<String> {
        sqlx::query_scalar(
            "select return_path from identity_verifications \
             where verifier = $1 and state_digest = $2",
        )
        .bind(crate::identity::repository::VERIFIER_NAVER)
        .bind(digest.as_slice())
        .fetch_optional(self.repository.pool())
        .await
        .ok()
        .flatten()
    }
}

/// The validated claim inputs.
struct ValidatedClaim {
    subject: String,
    gender: GenderBucket,
    age_at_verification: i16,
    next_birthday_at: chrono::NaiveDate,
}

/// The donor's `validated_claim`, with the donor's order and its two distinct
/// failure results.
fn validated_claim(
    profile: &crate::identity::naver_provider::NaverProviderProfile,
    now: DateTime<Utc>,
) -> Result<ValidatedClaim, CallbackResult> {
    // 1. Subject present. A profile with no subject identifies nobody.
    let subject = profile
        .subject
        .as_deref()
        .map(str::trim)
        .filter(|subject| !subject.is_empty())
        .ok_or(CallbackResult::ProfileIncomplete)?
        .to_owned();
    // 2. Gender present. `None` is "missing or unrecognized", and it is
    //    `profile_incomplete`. It is never mapped to `unknown`: only an explicit
    //    provider `U` produces the `unknown` bucket.
    let gender = profile
        .gender
        .map(|gender| match gender {
            crate::identity::naver_profile::NaverGender::Female => GenderBucket::Female,
            crate::identity::naver_profile::NaverGender::Male => GenderBucket::Male,
            crate::identity::naver_profile::NaverGender::Unknown => GenderBucket::Unknown,
        })
        .ok_or(CallbackResult::ProfileIncomplete)?;
    // 3. Derive the age with the already-adapted arithmetic. Not reimplemented.
    let age_claim = derive_exact_age_claim(
        profile.birthyear.as_deref(),
        profile.birthday.as_deref(),
        SeoulDate::new(seoul_date(now)),
    )
    .map_err(|error| match error {
        ExactAgeError::UnsupportedAge => CallbackResult::AgeNotSupported,
        // A missing, malformed, impossible or future birth date is an
        // incomplete profile, exactly as the donor classifies it.
        ExactAgeError::MissingBirthyear
        | ExactAgeError::MissingBirthday
        | ExactAgeError::MalformedBirthyear
        | ExactAgeError::MalformedBirthday
        | ExactAgeError::ImpossibleBirthday
        | ExactAgeError::FutureBirthDate => CallbackResult::ProfileIncomplete,
    })?;
    Ok(ValidatedClaim {
        subject,
        gender,
        age_at_verification: i16::from(age_claim.age_at_verification()),
        next_birthday_at: age_claim.next_birthday_at().as_naive_date(),
    })
}

/// Log a provider failure by classification only.
///
/// The error type cannot carry a token, subject, birth date or body, and this
/// function formats nothing else. That is the whole reason the provider error is
/// a closed enum rather than a wrapped `reqwest::Error`.
fn log_provider_failure(error: &NaverProviderError) {
    tracing::warn!(
        stage = error.stage.as_str(),
        classification = error.classification(),
        "NAVER provider request failed"
    );
}
