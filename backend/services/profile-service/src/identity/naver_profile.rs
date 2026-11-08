//! Provider profile decoding for NAVER Login.
//!
//! Adapted from Bigyeting `5eecdcaffd1a9d505b9ab2a974f8d5623217e90a`
//! `backend/services/bigyeting-api/src/adapters/out/oauth_profiles.rs`
//! (`NaverProfileResponse`, `NaverProfile`, `NaverProfile::normalized_gender`,
//! `NaverGender`). The field mapping is preserved exactly; what changed is the
//! shape of the failure. The donor returned `Option<NaverGender>` and let the
//! caller decide, which is the same information expressed less precisely: a
//! missing value and an unrecognized value are both "no usable gender". Here
//! that is one explicit `None`, and the caller (`callback`) turns it into
//! `profile_incomplete` and writes no claim row.
//!
//! Nothing in this module logs, formats or stores the raw provider profile. The
//! decoded value deliberately does not derive `Debug`, because a derived one
//! would render the birth date and the provider subject into any panic message
//! or log line that ever formats it. `NaverProfile`'s own `Debug` is hand
//! written and redacted.
use serde::Deserialize;

/// Largest provider profile body this service will decode.
///
/// The donor bounded its OAuth responses by bucket (`provider_response.rs`,
/// `classify_response_size`: 1024 / 16384 bytes). A NAVER `/v1/nid/me` body is a
/// few hundred bytes; 16 KiB is the donor's own "medium" ceiling and leaves
/// ample room for a provider that adds fields. A body above it is refused
/// **before** deserialization, so a hostile or broken provider cannot make this
/// service allocate or parse an unbounded document.
pub const MAX_PROFILE_BODY_BYTES: usize = 16 * 1024;

/// The provider's `gender` field, normalized.
///
/// This is a *provider* value, not a product value. It maps one-to-one onto the
/// stored `gender_bucket` (`female` / `male` / `unknown`) and onto the domain
/// contract's `Gender` / `Attribute::Unspecified`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NaverGender {
    Female,
    Male,
    /// The provider explicitly said "not specified" (`U`). This is a real
    /// answer, not a default: it is stored as `unknown` and matches only an
    /// audience whose author selected `unknown`.
    Unknown,
}

impl NaverGender {
    /// The stored `gender_bucket` value. The database CHECK constraint admits
    /// exactly these three strings.
    pub const fn gender_bucket(self) -> &'static str {
        match self {
            Self::Female => "female",
            Self::Male => "male",
            Self::Unknown => "unknown",
        }
    }
}

/// The decoded `/v1/nid/me` envelope.
#[derive(Deserialize)]
pub struct NaverProfileResponse {
    pub response: NaverProfile,
}

/// The decoded provider profile. Every field is optional, exactly as the donor
/// models it: the provider may omit any of them, and a missing field is a
/// verification failure rather than an empty string.
///
/// `Debug` is hand written and redacted. The derived one would print the
/// provider subject (`id`) and the birth date (`birthyear`, `birthday`), which
/// V-12-o forbids from appearing in logs.
#[derive(Deserialize)]
pub struct NaverProfile {
    pub id: Option<String>,
    pub email: Option<String>,
    pub age: Option<String>,
    pub gender: Option<String>,
    pub birthyear: Option<String>,
    pub birthday: Option<String>,
}

impl std::fmt::Debug for NaverProfile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Only presence, never a value. Even the presence of a gender is a
        // provider attribute, so it is reported as a boolean, not as a bucket.
        formatter
            .debug_struct("NaverProfile")
            .field("id", &self.id.as_ref().map(|_| "[REDACTED]"))
            .field("email", &self.email.as_ref().map(|_| "[REDACTED]"))
            .field("age", &self.age.as_ref().map(|_| "[REDACTED]"))
            .field("gender", &self.gender.as_ref().map(|_| "[REDACTED]"))
            .field("birthyear", &self.birthyear.as_ref().map(|_| "[REDACTED]"))
            .field("birthday", &self.birthday.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

impl NaverProfile {
    /// The donor's mapping, verbatim:
    ///
    /// | provider value | bucket |
    /// | --- | --- |
    /// | `F`, `female` | `Female` |
    /// | `M`, `male` | `Male` |
    /// | `U`, `unknown` | `Unknown` |
    /// | anything else, or absent | `None` |
    ///
    /// `None` is not "unknown". The caller must fail the verification with
    /// `profile_incomplete`; coercing a missing or unrecognized value to
    /// `unknown` would let a viewer satisfy an `unknown` audience they never
    /// qualified for.
    pub fn normalized_gender(&self) -> Option<NaverGender> {
        match self.gender.as_deref() {
            Some("F") | Some("female") => Some(NaverGender::Female),
            Some("M") | Some("male") => Some(NaverGender::Male),
            Some("U") | Some("unknown") => Some(NaverGender::Unknown),
            Some(_) | None => None,
        }
    }

    /// The provider subject, trimmed. An all-whitespace subject is absent.
    pub fn subject(&self) -> Option<&str> {
        self.id
            .as_deref()
            .map(str::trim)
            .filter(|subject| !subject.is_empty())
    }
}

/// Why a provider profile body could not be decoded.
///
/// The variants carry no provider content: not the body, not the subject, not a
/// parse offset. A caller may log the variant and nothing else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NaverProfileDecodeError {
    /// The body exceeded [`MAX_PROFILE_BODY_BYTES`]. Refused before parsing.
    TooLarge,
    /// The body was not the JSON envelope this provider documents.
    Malformed,
}

impl std::fmt::Display for NaverProfileDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::TooLarge => "NAVER profile response exceeded the size bound",
            Self::Malformed => "NAVER profile response was not a decodable profile",
        })
    }
}

/// Decode a `/v1/nid/me` body.
///
/// The size bound is checked on the raw bytes first, so an oversized body never
/// reaches `serde_json`. `serde_json` also enforces its own recursion and
/// document limits, but those are unrelated to this provider's contract.
pub fn decode_profile(body: &[u8]) -> Result<NaverProfile, NaverProfileDecodeError> {
    if body.len() > MAX_PROFILE_BODY_BYTES {
        return Err(NaverProfileDecodeError::TooLarge);
    }
    serde_json::from_slice::<NaverProfileResponse>(body)
        .map(|response| response.response)
        .map_err(|_| NaverProfileDecodeError::Malformed)
}
