//! The identity claim port: the only way an audience decision learns a viewer's
//! age or gender.
//!
//! Contract: [domain-contract.md §1.3](../../../../docs/mcr/integration/domain-contract.md).
//! The claim store lives in the profile service (`identity_claims`, WP-02) and is
//! read through this port by the concert service. There is no cross-database
//! join and no shared table: the port is the whole interface.
//!
//! Three properties are structural, not conventional:
//!
//! * **`Unavailable` is the failure value, never a default.** No method on this
//!   module returns a zero, a `None` that a caller might read as "no
//!   restriction", or a guess. A transport error, an expired claim, a revoked
//!   claim and a malformed stored row all produce [`Attribute::Unavailable`].
//! * **Nothing here renders the age.** [`IdentityClaims`] has a hand-written
//!   `Debug` that redacts both attributes, and it has no `Serialize`, no
//!   `Display` and no `PartialEq` that a test could use to print it. The numeric
//!   age is private input to a decision; it is never a DTO.
//! * **The three attribute states are distinct.** `Known` is a live value,
//!   `Unspecified` is the provider's explicit "not specified" answer, and
//!   `Unavailable` is everything else. `Unspecified` is not "no claim" and it is
//!   not a wildcard: it satisfies only an audience whose author chose
//!   `unknown`.
//!
//! The trait is `async` via `async_trait` rather than returning
//! `impl Future`. `async_trait` is what the rest of this crate uses for every
//! other port (`IntroAuthority`, `ClaimReader`, `CoreRepository`), and the
//! contract's `impl Future<Output = ...> + Send` signature is not object-safe in
//! a trait an adapter is stored behind `Arc<dyn _>`. The reason is recorded in
//! `docs/mcr/backend/wp04-claims.md`; the observable behavior is identical.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_domain::age_policy::arithmetic::ExactAgeClaim;
use uuid::Uuid;

/// A profile identifier, as the profile service defines it (`profile_identities.profile_id`).
///
/// This is not the Auth user UUID and not the concert `AuthUserId`. The concert
/// service stores `profile_id` on every profile-owned row, and the claim adapter
/// resolves it to an account before it asks the profile service. Keeping the two
/// apart is what stops a claim read for the wrong subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProfileId(Uuid);

impl ProfileId {
    pub fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    /// A canonical, non-nil profile id. Anything else is not an identity.
    pub fn parse(value: &str) -> Option<Self> {
        Uuid::parse_str(value)
            .ok()
            .filter(|id| !id.is_nil() && id.to_string() == value)
            .map(Self)
    }

    pub const fn uuid(self) -> Uuid {
        self.0
    }
}

/// The provider gender of a live claim.
///
/// There is deliberately no `Unknown` variant. The provider's explicit "not
/// specified" answer is [`Attribute::Unspecified`], a different state with a
/// different meaning, and folding it into this enum is the donor bug that made
/// `unknown` behave like a wildcard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Gender {
    Female,
    Male,
}

impl Gender {
    /// The stored `identity_claims.gender_bucket` value.
    pub const fn bucket(self) -> &'static str {
        match self {
            Self::Female => "female",
            Self::Male => "male",
        }
    }

    /// Parse a stored bucket. `unknown` is not a `Gender`: the caller maps it to
    /// [`Attribute::Unspecified`] before reaching this function.
    pub fn parse(bucket: &str) -> Option<Self> {
        match bucket {
            "female" => Some(Self::Female),
            "male" => Some(Self::Male),
            _ => None,
        }
    }
}

/// One provider attribute, in the three states an audience decision needs.
///
/// The distinction is the whole security property of this module:
///
/// | State | Meaning | Satisfies |
/// | --- | --- | --- |
/// | `Known` | a live claim carries the value | every condition that names it |
/// | `Unspecified` | the provider explicitly said "not specified" | only `unknown` |
/// | `Unavailable` | no claim, expired, revoked, or the source failed | nothing |
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Attribute<T> {
    Known(T),
    Unspecified,
    Unavailable,
}

impl<T> Attribute<T> {
    /// The live value, if there is one. `Unspecified` is not a value.
    pub fn known(self) -> Option<T> {
        match self {
            Self::Known(value) => Some(value),
            _ => None,
        }
    }

    /// Whether the attribute can be evaluated at all. `Unspecified` can: it is
    /// an answer. `Unavailable` cannot.
    pub const fn is_evaluable(&self) -> bool {
        !matches!(self, Self::Unavailable)
    }

    pub const fn is_unavailable(&self) -> bool {
        matches!(self, Self::Unavailable)
    }

    /// Apply a function to the live value, preserving the other two states.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Attribute<U> {
        match self {
            Self::Known(value) => Attribute::Known(f(value)),
            Self::Unspecified => Attribute::Unspecified,
            Self::Unavailable => Attribute::Unavailable,
        }
    }
}

/// `Debug` shows the state and never the value.
///
/// The value type is deliberately not required to be `Debug`: the age claim is
/// private input with no `Debug` of its own, and this is the impl that keeps it
/// that way. A `Known` attribute renders as `Known([REDACTED])`, so
/// `format!("{:?}", claims)` cannot be the path an age reaches a log.
impl<T> std::fmt::Debug for Attribute<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Known(_) => formatter.write_str("Known([REDACTED])"),
            Self::Unspecified => formatter.write_str("Unspecified"),
            Self::Unavailable => formatter.write_str("Unavailable"),
        }
    }
}

/// Where a claim came from, and when it stops being valid.
///
/// The values are provider metadata, not an attribute: they identify the claim a
/// decision was made against so the decision can be audited later. They carry no
/// age, no gender and no provider subject.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaimSource {
    /// The verifier name, e.g. `naver`.
    pub verifier: String,
    /// The revision of our mapping of the provider's profile contract.
    pub source_revision: String,
    /// When the provider confirmed the attributes.
    pub verified_at: DateTime<Utc>,
    /// When the claim stops being valid. `expires_at <= now` is expired.
    pub expires_at: DateTime<Utc>,
}

/// The attribute snapshot one audience decision is evaluated against.
///
/// No `Serialize`, no `Display`, and a hand-written redacting `Debug`. The age is
/// the one value V-12-o keeps out of logs, and a derived `Debug` would put it in
/// every `tracing` field that ever formats a claim. `Clone` is derived because a
/// caller may evaluate several posts against one snapshot; the clone carries the
/// same redacting `Debug` and still has no `Serialize`.
#[derive(Clone)]
pub struct IdentityClaims {
    pub profile: ProfileId,
    /// The live age claim, or the reason there is none.
    pub age: Attribute<ExactAgeClaim>,
    /// The live provider gender, or the reason there is none.
    pub gender: Attribute<Gender>,
    /// Present exactly when the snapshot came from a stored claim row. A
    /// snapshot with no row has no source, and no decision may treat that as a
    /// live claim.
    pub source: Option<ClaimSource>,
    /// The instant the snapshot was evaluated at. The current age is a function
    /// of this instant and the Seoul calendar date, never a stored value.
    pub evaluated_at: DateTime<Utc>,
}

impl std::fmt::Debug for IdentityClaims {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IdentityClaims")
            .field("profile", &self.profile)
            .field("age", &self.age)
            .field("gender", &self.gender)
            .field("source", &self.source)
            .field("evaluated_at", &self.evaluated_at)
            .finish()
    }
}

impl IdentityClaims {
    /// The snapshot for a subject with no live claim at all.
    ///
    /// Both attributes are `Unavailable`, never `Unspecified`: "we have nothing"
    /// and "the provider said not specified" are different facts.
    pub fn unavailable(profile: ProfileId, evaluated_at: DateTime<Utc>) -> Self {
        Self {
            profile,
            age: Attribute::Unavailable,
            gender: Attribute::Unavailable,
            source: None,
            evaluated_at,
        }
    }
}

/// Why a claim read failed.
///
/// One variant, on purpose. The caller's only correct response to any of these
/// is the same: both attributes become `Unavailable`. A richer error would
/// invite a caller to distinguish "no claim" from "store down" and to treat one
/// of them as permissive, and the error must never carry an attribute value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ClaimsError {
    #[error("Identity claims are unavailable")]
    Unavailable,
}

/// The claim port.
///
/// Implementations must fail closed: any unavailable source yields
/// [`Attribute::Unavailable`], never a default value, and never a fallback to an
/// older claim. The canonical-row ordering (newest `verified_at`, then newest
/// `expires_at`, then newest `claim_id`) belongs to the store, not to this port;
/// an adapter that re-sorts or filters rows to find a usable one is wrong.
///
/// The `now` argument is the decision's own timestamp, not the adapter's clock.
/// Passing it in is what makes "claim expires at `now`" testable and what keeps
/// the pure decision and the SQL predicate on one clock.
#[async_trait]
pub trait IdentityClaimsPort: Send + Sync {
    async fn read(
        &self,
        profile: ProfileId,
        now: DateTime<Utc>,
    ) -> Result<IdentityClaims, ClaimsError>;
}

/// A port that never answers. The composition root uses it when the profile
/// service endpoint is not configured, so an unconfigured deployment denies
/// every restricted post instead of admitting them.
pub struct UnavailableClaims;

#[async_trait]
impl IdentityClaimsPort for UnavailableClaims {
    async fn read(
        &self,
        _: ProfileId,
        _: DateTime<Utc>,
    ) -> Result<IdentityClaims, ClaimsError> {
        Err(ClaimsError::Unavailable)
    }
}

/// A fixed snapshot for tests and for the case tables.
///
/// This is a test double, not a production adapter: it answers with the values
/// it was built with and never consults a store. It exists so the decision table
/// can be exercised without a database, and so a test can assert that the
/// decision did not read claims at all by giving it a port that panics.
#[derive(Clone)]
pub struct StaticClaims {
    claims: StaticClaimsValue,
}

#[derive(Clone)]
enum StaticClaimsValue {
    Snapshot {
        age: Attribute<ExactAgeClaim>,
        gender: Attribute<Gender>,
        source: ClaimSource,
    },
    Failed,
}

impl StaticClaims {
    /// A live claim with the given attributes.
    pub fn live(
        age: Attribute<ExactAgeClaim>,
        gender: Attribute<Gender>,
        source: ClaimSource,
    ) -> Self {
        Self {
            claims: StaticClaimsValue::Snapshot {
                age,
                gender,
                source,
            },
        }
    }

    /// A read that fails at the transport.
    pub fn failed() -> Self {
        Self {
            claims: StaticClaimsValue::Failed,
        }
    }
}

#[async_trait]
impl IdentityClaimsPort for StaticClaims {
    async fn read(
        &self,
        profile: ProfileId,
        now: DateTime<Utc>,
    ) -> Result<IdentityClaims, ClaimsError> {
        match &self.claims {
            StaticClaimsValue::Snapshot {
                age,
                gender,
                source,
            } => Ok(IdentityClaims {
                profile,
                age: *age,
                gender: *gender,
                source: Some(source.clone()),
                evaluated_at: now,
            }),
            StaticClaimsValue::Failed => Err(ClaimsError::Unavailable),
        }
    }
}
