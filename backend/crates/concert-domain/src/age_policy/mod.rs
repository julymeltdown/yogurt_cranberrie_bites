//! Private exact-age rules. Values alone never attest a verified provider/session.
pub mod arithmetic;
use crate::AuthUserId;
use arithmetic::{MAX_SUPPORTED_AGE, MIN_SUPPORTED_AGE};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AgeError {
    #[error("Age policy input is invalid")]
    Invalid,
    #[error("Required policy authority is unavailable")]
    Unavailable,
}
/// C04 paired nullable bounds; not a service eligibility or admission limit.
#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AgeBounds {
    minimum_age: Option<i16>,
    maximum_age_exclusive: Option<i16>,
}
impl AgeBounds {
    pub fn new(minimum: Option<i16>, maximum: Option<i16>) -> Result<Self, AgeError> {
        match (minimum, maximum) {
            (None, None) => Ok(Self {
                minimum_age: None,
                maximum_age_exclusive: None,
            }),
            (Some(a), Some(b)) if (14..=100).contains(&a) && (15..=101).contains(&b) && a < b => {
                Ok(Self {
                    minimum_age: minimum,
                    maximum_age_exclusive: maximum,
                })
            }
            _ => Err(AgeError::Invalid),
        }
    }
    pub fn restricted(self) -> bool {
        self.minimum_age.is_some()
    }
    /// Sharing always requires verified age; unbounded discovery does not.
    /// This pure result is never a session authorization or mutation fence.
    pub fn permits(self, assessment: &AgeAssessment, sharing: bool) -> bool {
        if !self.restricted() && !sharing {
            return true;
        }
        if assessment.status != AgeStatus::Verified {
            return false;
        }
        match (assessment.age, self.minimum_age, self.maximum_age_exclusive) {
            (Some(age), Some(min), Some(max)) => min <= age && age < max,
            (Some(_), None, None) => true,
            _ => false,
        }
    }
}
fn required_nullable<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<i16>, D::Error> {
    Option::<i16>::deserialize(d)
}
impl<'de> Deserialize<'de> for AgeBounds {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            #[serde(deserialize_with = "required_nullable")]
            minimum_age: Option<i16>,
            #[serde(deserialize_with = "required_nullable")]
            maximum_age_exclusive: Option<i16>,
        }
        let raw = Raw::deserialize(d)?;
        Self::new(raw.minimum_age, raw.maximum_age_exclusive).map_err(serde::de::Error::custom)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgeStatus {
    Unknown,
    Verified,
    Expired,
    Unsupported,
    ProviderUnavailable,
    Revoked,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StoredStatus {
    Unknown,
    Verified,
    Replaced,
    Revoked,
    Unsupported,
}
/// Owner-only input, deliberately not Debug/Serialize/Deserialize. The owning
/// claim adapter must attest these fields; no API accepts client age assertions.
#[derive(Clone)]
pub struct OwnerClaim {
    pub claim_id: Uuid,
    pub user_id: AuthUserId,
    pub verifier_id: String,
    pub source_revision: String,
    pub status: StoredStatus,
    pub verified_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub age_at_verification: Option<i16>,
    pub next_birthday_at: Option<NaiveDate>,
}
/// Private evaluation. Only safe status/timestamps are projected by application.
#[derive(Clone)]
pub struct AgeAssessment {
    status: AgeStatus,
    age: Option<i16>,
    verified_at: Option<DateTime<Utc>>,
    expires_at: Option<DateTime<Utc>>,
    recheck_at: Option<DateTime<Utc>>,
}
impl AgeAssessment {
    pub fn status(&self) -> AgeStatus {
        self.status
    }
    pub fn recheck_at(&self) -> Option<DateTime<Utc>> {
        self.recheck_at
    }
    pub fn timestamps(&self) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
        (self.verified_at, self.expires_at)
    }
}
pub fn seoul_date(now: DateTime<Utc>) -> Result<NaiveDate, AgeError> {
    now.checked_add_signed(Duration::hours(9))
        .map(|d| d.date_naive())
        .ok_or(AgeError::Invalid)
}
/// Select BEFORE checking validity. Never fall back to an older usable proof.
/// Ordering exactly matches C04 and source current_age.rs, including expiry tie.
pub fn assess(
    user: AuthUserId,
    claims: &[OwnerClaim],
    now: DateTime<Utc>,
) -> Result<AgeAssessment, AgeError> {
    if user.uuid().is_nil()
        || claims.len() > 1024
        || claims
            .iter()
            .any(|c| c.user_id != user || c.claim_id.is_nil())
    {
        return Err(AgeError::Unavailable);
    }
    let mut ids = std::collections::HashSet::new();
    if claims.iter().any(|c| !ids.insert(c.claim_id)) {
        return Err(AgeError::Unavailable);
    }
    let canonical = claims
        .iter()
        .max_by_key(|c| (c.verified_at, c.expires_at, c.claim_id));
    let Some(c) = canonical else {
        return Ok(AgeAssessment {
            status: AgeStatus::Unknown,
            age: None,
            verified_at: None,
            expires_at: None,
            recheck_at: None,
        });
    };
    let mut result = AgeAssessment {
        status: AgeStatus::Unknown,
        age: None,
        verified_at: Some(c.verified_at),
        expires_at: Some(c.expires_at),
        recheck_at: Some(c.expires_at),
    };
    result.status = match c.status {
        StoredStatus::Unknown => AgeStatus::Unknown,
        StoredStatus::Replaced | StoredStatus::Revoked => AgeStatus::Revoked,
        StoredStatus::Unsupported => AgeStatus::Unsupported,
        StoredStatus::Verified => {
            if c.expires_at <= now {
                AgeStatus::Expired
            } else if !valid_shape(c, now)? {
                AgeStatus::Unsupported
            } else {
                let age = c.age_at_verification.ok_or(AgeError::Unavailable)?
                    + i16::from(
                        seoul_date(now)? >= c.next_birthday_at.ok_or(AgeError::Unavailable)?,
                    );
                if (MIN_SUPPORTED_AGE as i16..=MAX_SUPPORTED_AGE as i16).contains(&age) {
                    result.age = Some(age);
                    let birthday = c
                        .next_birthday_at
                        .and_then(|d| d.and_hms_opt(0, 0, 0))
                        .and_then(|d| d.checked_sub_signed(Duration::hours(9)))
                        .map(|d| d.and_utc());
                    if let Some(boundary) = birthday.filter(|b| *b > now) {
                        result.recheck_at = Some(boundary.min(c.expires_at));
                    }
                    AgeStatus::Verified
                } else {
                    AgeStatus::Unsupported
                }
            }
        }
    };
    Ok(result)
}
fn valid_shape(c: &OwnerClaim, now: DateTime<Utc>) -> Result<bool, AgeError> {
    let verification = seoul_date(c.verified_at)?;
    let next = c.next_birthday_at;
    Ok(c.verified_at <= now
        && c.expires_at > c.verified_at
        && c.expires_at - c.verified_at <= Duration::days(90)
        && c.age_at_verification
            .is_some_and(|a| (14..=100).contains(&a))
        && next.is_some_and(|n| n > verification && n - verification <= Duration::days(366))
        && !c.verifier_id.trim().is_empty()
        && c.verifier_id.len() <= 100
        && !c.verifier_id.chars().any(char::is_control)
        && !c.source_revision.trim().is_empty()
        && c.source_revision.len() <= 100
        && !c.source_revision.chars().any(char::is_control))
}
