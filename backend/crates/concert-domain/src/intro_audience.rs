//! The audience an Introduction author configured: a gender condition and an
//! optional paired age window.
//!
//! Adapted from Bigyeting `5eecdcaf`
//! `domain/privacy.rs` (`GenderBucket`, `AgeBounds`) and the donor's
//! `allowed_gender` / `minimum_age` / `maximum_age_exclusive` columns. The donor
//! stored the pair as two nullable integers with no ordering rule and treated
//! `unknown` as a wildcard in one code path. Both are corrected here:
//!
//! * **The bounds are paired.** A half-open window is meaningless: "18 and up"
//!   and "under 30" are different audiences, and a row carrying only one of them
//!   cannot be evaluated. `None`/`None` is the only unbounded shape.
//! * **`unknown` is a condition, not a wildcard.** It matches only a viewer
//!   whose provider explicitly reported "not specified". It is the donor's
//!   `gender_bucket = 'unknown'`, which exists only when NAVER returned `U`.
//!   `Any` is the wildcard, and it is a separate value.
//! * **The upper bound is exclusive on the wire.** The interface shows
//!   "Ages 18–29" and the value carries `minimum_age: 18,
//!   maximum_age_exclusive: 30`. Keeping the half-open form in the value means
//!   the inclusive/exclusive conversion happens exactly once, in the composer,
//!   and never in a comparison.
//!
//! The 14..=100 / 15..=101 window is configuration (charter D-08), not the final
//! global age policy. It is expressed here as constants so the schema CHECK, the
//! value type and the tests cannot disagree about it.
//!
//! `ExactAge` and `MaximumAgeExclusive` are separate types on purpose. Both are
//! small integers, and the one bug this module must make impossible is passing
//! the exclusive upper bound where an inclusive age is expected. The compiler
//! refuses it.
use crate::age_policy::arithmetic::{MAX_SUPPORTED_AGE, MIN_SUPPORTED_AGE};
use serde::{Deserialize, Serialize};

/// The inclusive lower bound range. Configuration, not policy.
pub const MIN_AUDIENCE_AGE: i16 = MIN_SUPPORTED_AGE as i16;
/// The inclusive lower bound range's upper end.
pub const MAX_AUDIENCE_MINIMUM_AGE: i16 = MAX_SUPPORTED_AGE as i16;
/// The exclusive upper bound range: one more than the highest supported age.
pub const MIN_AUDIENCE_MAXIMUM_EXCLUSIVE: i16 = MIN_SUPPORTED_AGE as i16 + 1;
/// The exclusive upper bound range's upper end.
pub const MAX_AUDIENCE_MAXIMUM_EXCLUSIVE: i16 = MAX_SUPPORTED_AGE as i16 + 1;

/// Why an audience value was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AudienceError {
    #[error("audience age bounds are invalid")]
    InvalidBounds,
    #[error("audience age bounds must be paired")]
    UnpairedBounds,
    #[error("audience gender condition is invalid")]
    InvalidGender,
}

/// The author's gender condition.
///
/// The wire names are the donor's `gender_bucket` values plus `any`, and they
/// are lowercase in the stored column and in the request body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GenderCondition {
    /// No gender restriction.
    Any,
    /// Only a viewer with a live claim reporting `female`.
    Female,
    /// Only a viewer with a live claim reporting `male`.
    Male,
    /// Only a viewer whose provider explicitly reported "not specified".
    Unknown,
}

impl GenderCondition {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Female => "female",
            Self::Male => "male",
            Self::Unknown => "unknown",
        }
    }

    /// Closed parsing of the stored column and the request body.
    pub fn parse(value: &str) -> Result<Self, AudienceError> {
        match value {
            "any" => Ok(Self::Any),
            "female" => Ok(Self::Female),
            "male" => Ok(Self::Male),
            "unknown" => Ok(Self::Unknown),
            _ => Err(AudienceError::InvalidGender),
        }
    }

    /// Whether this condition restricts who may view.
    ///
    /// `Unknown` restricts: it admits only the explicit "not specified" bucket,
    /// which is a much smaller set than "anybody". Treating it as a wildcard is
    /// the donor bug this method exists to make visible.
    pub const fn restricts(self) -> bool {
        !matches!(self, Self::Any)
    }
}

/// An inclusive lower age bound. Constructing one validates the range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExactAge(i16);

impl ExactAge {
    pub fn new(value: i16) -> Result<Self, AudienceError> {
        if (MIN_AUDIENCE_AGE..=MAX_AUDIENCE_MINIMUM_AGE).contains(&value) {
            Ok(Self(value))
        } else {
            Err(AudienceError::InvalidBounds)
        }
    }

    pub const fn get(self) -> i16 {
        self.0
    }
}

impl Serialize for ExactAge {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i16(self.0)
    }
}

impl<'de> Deserialize<'de> for ExactAge {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(i16::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// An exclusive upper age bound. Constructing one validates the range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MaximumAgeExclusive(i16);

impl MaximumAgeExclusive {
    pub fn new(value: i16) -> Result<Self, AudienceError> {
        if (MIN_AUDIENCE_MAXIMUM_EXCLUSIVE..=MAX_AUDIENCE_MAXIMUM_EXCLUSIVE).contains(&value) {
            Ok(Self(value))
        } else {
            Err(AudienceError::InvalidBounds)
        }
    }

    pub const fn get(self) -> i16 {
        self.0
    }

    /// The inclusive upper age the interface shows. `30` exclusive is "up to
    /// 29". This is the single place the conversion happens.
    pub const fn inclusive_upper(self) -> i16 {
        self.0 - 1
    }
}

impl Serialize for MaximumAgeExclusive {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i16(self.0)
    }
}

impl<'de> Deserialize<'de> for MaximumAgeExclusive {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(i16::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// The author-configured audience.
///
/// The fields are private so the paired-bounds and ordering invariants cannot be
/// bypassed by a struct literal. Every construction goes through
/// [`IntroAudience::new`] or [`IntroAudience::deserialize`], and both enforce the
/// same rules the database CHECK enforces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntroAudience {
    gender: GenderCondition,
    minimum_age: Option<ExactAge>,
    maximum_age_exclusive: Option<MaximumAgeExclusive>,
}

impl IntroAudience {
    /// The unrestricted audience: no age bound, no gender condition.
    pub const ANYONE: Self = Self {
        gender: GenderCondition::Any,
        minimum_age: None,
        maximum_age_exclusive: None,
    };

    /// Build an audience, enforcing the paired-bounds and ordering invariants.
    pub fn new(
        gender: GenderCondition,
        minimum_age: Option<ExactAge>,
        maximum_age_exclusive: Option<MaximumAgeExclusive>,
    ) -> Result<Self, AudienceError> {
        match (minimum_age, maximum_age_exclusive) {
            (None, None) => Ok(Self {
                gender,
                minimum_age: None,
                maximum_age_exclusive: None,
            }),
            (Some(minimum), Some(maximum)) if minimum.get() < maximum.get() => Ok(Self {
                gender,
                minimum_age: Some(minimum),
                maximum_age_exclusive: Some(maximum),
            }),
            // A paired but unordered window is `minimum >= maximum`, which the
            // contract rejects. An unpaired window is its own error so a caller
            // can tell "you sent one bound" from "your bounds are backwards".
            (Some(_), Some(_)) => Err(AudienceError::InvalidBounds),
            _ => Err(AudienceError::UnpairedBounds),
        }
    }

    /// Build an audience from raw column or body values.
    pub fn from_parts(
        gender: &str,
        minimum_age: Option<i16>,
        maximum_age_exclusive: Option<i16>,
    ) -> Result<Self, AudienceError> {
        let gender = GenderCondition::parse(gender)?;
        let minimum = minimum_age.map(ExactAge::new).transpose()?;
        let maximum = maximum_age_exclusive
            .map(MaximumAgeExclusive::new)
            .transpose()?;
        Self::new(gender, minimum, maximum)
    }

    pub const fn gender(self) -> GenderCondition {
        self.gender
    }

    pub const fn minimum_age(self) -> Option<ExactAge> {
        self.minimum_age
    }

    pub const fn maximum_age_exclusive(self) -> Option<MaximumAgeExclusive> {
        self.maximum_age_exclusive
    }

    /// Whether the audience restricts who may view.
    ///
    /// False exactly when there is no age bound and the gender condition is
    /// `Any`. This is decision-table row 6, and it is the value the visibility
    /// decision uses to decide whether to read an identity claim at all.
    pub const fn is_restricted(self) -> bool {
        self.minimum_age.is_some() || self.gender.restricts()
    }

    /// Whether an age bound is present. Row 7 asks this, not `is_restricted`:
    /// a gender-only audience must not deny a viewer for an unknown age.
    pub const fn has_age_bound(self) -> bool {
        self.minimum_age.is_some()
    }

    /// Whether the half-open window admits an exact age.
    ///
    /// An unbounded audience admits every age. This method is only reachable
    /// with a `Known` age; an unknown age is refused by the decision, not here.
    pub fn admits_age(self, age: i16) -> bool {
        match (self.minimum_age, self.maximum_age_exclusive) {
            (Some(minimum), Some(maximum)) => {
                minimum.get() <= age && age < maximum.get()
            }
            _ => true,
        }
    }

    /// The author-facing summary, for the owner projection only.
    ///
    /// `Women · Ages 18–29`, `Anyone`, `Gender not specified · Ages 18+`. The
    /// server derives it so the client can never disagree with the value.
    pub fn summary(self) -> String {
        let gender = match self.gender {
            GenderCondition::Any => None,
            GenderCondition::Female => Some("Women"),
            GenderCondition::Male => Some("Men"),
            GenderCondition::Unknown => Some("Gender not specified"),
        };
        let ages = match (self.minimum_age, self.maximum_age_exclusive) {
            (Some(minimum), Some(maximum)) => Some(format!(
                "Ages {}–{}",
                minimum.get(),
                maximum.inclusive_upper()
            )),
            _ => None,
        };
        match (gender, ages) {
            (None, None) => "Anyone".to_owned(),
            (Some(gender), None) => gender.to_owned(),
            (None, Some(ages)) => ages,
            (Some(gender), Some(ages)) => format!("{gender} · {ages}"),
        }
    }
}

impl Default for IntroAudience {
    fn default() -> Self {
        Self::ANYONE
    }
}

/// The wire shape is the frozen `social.IntroAudience` object, with the donor's
/// `allowed_gender` name. Serialization and deserialization are both written out
/// here rather than derived, so the field names exist in exactly one place and
/// cannot drift from the contract.
impl Serialize for IntroAudience {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("IntroAudience", 3)?;
        state.serialize_field("allowed_gender", &self.gender)?;
        state.serialize_field("minimum_age", &self.minimum_age)?;
        state.serialize_field("maximum_age_exclusive", &self.maximum_age_exclusive)?;
        state.end()
    }
}

/// Strict deserialization: unknown fields are refused, and both bounds must be
/// present together.
///
/// The three fields are required, exactly as the frozen `social.IntroAudience`
/// schema declares them. A body that omits `minimum_age` is not "no bound"; it
/// is a malformed audience, and accepting it would let a client publish a
/// restriction it did not intend.
impl<'de> Deserialize<'de> for IntroAudience {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            allowed_gender: GenderCondition,
            // A missing key and a present `null` are different values here:
            // the first is a malformed audience, the second is "no bound". The
            // `Option<Option<..>>` shape is what makes the two distinguishable,
            // because serde's default for `Option<T>` collapses them.
            #[serde(default, deserialize_with = "present_or_absent")]
            minimum_age: Option<Option<i16>>,
            #[serde(default, deserialize_with = "present_or_absent")]
            maximum_age_exclusive: Option<Option<i16>>,
        }
        fn present_or_absent<'de, D: serde::Deserializer<'de>>(
            deserializer: D,
        ) -> Result<Option<Option<i16>>, D::Error> {
            Option::<i16>::deserialize(deserializer).map(Some)
        }
        let raw = Raw::deserialize(deserializer)?;
        let minimum_age = raw
            .minimum_age
            .ok_or_else(|| serde::de::Error::missing_field("minimum_age"))?;
        let maximum_age_exclusive = raw
            .maximum_age_exclusive
            .ok_or_else(|| serde::de::Error::missing_field("maximum_age_exclusive"))?;
        Self::from_parts(
            raw.allowed_gender.as_str(),
            minimum_age,
            maximum_age_exclusive,
        )
        .map_err(serde::de::Error::custom)
    }
}
