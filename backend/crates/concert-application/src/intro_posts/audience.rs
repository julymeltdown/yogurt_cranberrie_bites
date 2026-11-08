//! T-401 / T-407: the audience value and its server-derived summary.
//!
//! The wire keeps a half-open upper bound (`maximum_age_exclusive`). The
//! interface shows an inclusive upper bound, and the conversion happens exactly
//! once, at the boundary: "Ages 18–29" is `minimum_age: 18,
//! maximum_age_exclusive: 30`.
//!
//! Two rules the implementation must not soften, both from
//! domain-contract.md §2:
//!
//! * both bounds are present or both are absent, and `minimum < maximum`;
//! * `unknown` is **not** "any". It means the viewer's provider explicitly
//!   reported "not specified", and it never behaves as a wildcard.
//!
//! [`Audience::summary`] is the only place a human summary is produced. The
//! client is forbidden from recomputing it (T-408), so this function is the
//! single source of truth and its output is what the review step displays.

/// The closed gender condition. `Any` ignores gender; every other variant
/// requires a live claim that matches exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GenderCondition {
    Any,
    Female,
    Male,
    /// The provider explicitly reported "not specified". Never a wildcard.
    Unknown,
}

impl GenderCondition {
    /// The wire spelling, matching `social.IntroAudience.allowed_gender`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Female => "female",
            Self::Male => "male",
            Self::Unknown => "unknown",
        }
    }

    /// Closed parsing. An unrecognized value is an error, never a default.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "any" => Some(Self::Any),
            "female" => Some(Self::Female),
            "male" => Some(Self::Male),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }

    /// Whether this condition constrains gender at all.
    pub fn restricted(self) -> bool {
        self != Self::Any
    }
}

/// The inclusive window the interface shows, as the author typed it.
///
/// This type exists so the conversion to the half-open wire form is a named,
/// tested operation rather than an arithmetic expression repeated in the UI, the
/// command and the test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InclusiveAgeRange {
    pub minimum: i16,
    /// Inclusive, as the author understands it. 29 means "up to and including 29".
    pub maximum_inclusive: i16,
}

impl InclusiveAgeRange {
    /// Convert to the half-open wire form: `minimum_age = minimum`,
    /// `maximum_age_exclusive = maximum_inclusive + 1`.
    pub fn to_half_open(self) -> (i16, i16) {
        (self.minimum, self.maximum_inclusive.saturating_add(1))
    }

    /// Convert from the half-open wire form back to the inclusive display form.
    pub fn from_half_open(minimum: i16, maximum_exclusive: i16) -> Self {
        Self {
            minimum,
            maximum_inclusive: maximum_exclusive.saturating_sub(1),
        }
    }
}

/// The frozen bounds: `minimum_age` 14..=100, `maximum_age_exclusive` 15..=101.
pub const MIN_AGE: i16 = 14;
pub const MAX_AGE: i16 = 100;
pub const MIN_MAX_EXCLUSIVE: i16 = 15;
pub const MAX_MAX_EXCLUSIVE: i16 = 101;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudienceError {
    /// A bound was present without its pair.
    UnpairedBounds,
    /// A bound was outside the configured window.
    OutOfRange,
    /// `minimum_age >= maximum_age_exclusive`.
    EmptyRange,
    /// The gender condition was not one of the four closed values.
    InvalidGender,
}

impl std::fmt::Display for AudienceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid intro audience: {self:?}")
    }
}

impl std::error::Error for AudienceError {}

/// The author-only audience. This value never appears in a viewer projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Audience {
    pub gender: GenderCondition,
    pub minimum_age: Option<i16>,
    pub maximum_age_exclusive: Option<i16>,
}

impl Audience {
    /// Anyone, with no age or gender restriction.
    pub const ANYONE: Self = Self {
        gender: GenderCondition::Any,
        minimum_age: None,
        maximum_age_exclusive: None,
    };

    /// Build from the wire form, validating every invariant.
    ///
    /// The pairing rule and the ordering rule are checked here rather than left
    /// to the database, so a caller learns what is wrong before a transaction
    /// opens. The database constraint `intro_posts_audience_bounds_paired`
    /// remains the guarantee for every writer.
    pub fn new(
        gender: GenderCondition,
        minimum_age: Option<i16>,
        maximum_age_exclusive: Option<i16>,
    ) -> Result<Self, AudienceError> {
        match (minimum_age, maximum_age_exclusive) {
            (None, None) => Ok(Self {
                gender,
                minimum_age: None,
                maximum_age_exclusive: None,
            }),
            (Some(min), Some(max)) => {
                if !(MIN_AGE..=MAX_AGE).contains(&min)
                    || !(MIN_MAX_EXCLUSIVE..=MAX_MAX_EXCLUSIVE).contains(&max)
                {
                    return Err(AudienceError::OutOfRange);
                }
                if min >= max {
                    return Err(AudienceError::EmptyRange);
                }
                Ok(Self {
                    gender,
                    minimum_age: Some(min),
                    maximum_age_exclusive: Some(max),
                })
            }
            _ => Err(AudienceError::UnpairedBounds),
        }
    }

    /// Build from an inclusive range as the interface presents it.
    pub fn from_inclusive(
        gender: GenderCondition,
        range: Option<InclusiveAgeRange>,
    ) -> Result<Self, AudienceError> {
        match range {
            None => Self::new(gender, None, None),
            Some(range) => {
                let (min, max) = range.to_half_open();
                Self::new(gender, Some(min), Some(max))
            }
        }
    }

    /// The inclusive range as the interface should show it, if any.
    pub fn inclusive_range(self) -> Option<InclusiveAgeRange> {
        match (self.minimum_age, self.maximum_age_exclusive) {
            (Some(min), Some(max)) => Some(InclusiveAgeRange::from_half_open(min, max)),
            _ => None,
        }
    }

    /// Whether this audience restricts anybody at all.
    ///
    /// An unrestricted audience is the row-6 case of the decision table: it
    /// allows anyone whose other conditions pass, and requires no claim.
    pub fn is_restricted(self) -> bool {
        self.gender.restricted() || self.minimum_age.is_some()
    }

    /// The author-facing summary, for example `Women · Ages 18–29`.
    ///
    /// This is the **only** place the summary is produced. The client renders
    /// the string the server returns and never recomputes it, so the two can
    /// never disagree.
    pub fn summary(self) -> String {
        let gender = match self.gender {
            GenderCondition::Any => None,
            GenderCondition::Female => Some("Women"),
            GenderCondition::Male => Some("Men"),
            GenderCondition::Unknown => Some("Gender not specified"),
        };
        let ages = self.inclusive_range().map(|range| {
            if range.maximum_inclusive >= MAX_AGE {
                format!("Ages {}+", range.minimum)
            } else {
                // An en dash, as the contract's examples spell it.
                format!("Ages {}\u{2013}{}", range.minimum, range.maximum_inclusive)
            }
        });
        match (gender, ages) {
            (None, None) => "Anyone".to_owned(),
            (Some(gender), None) => gender.to_owned(),
            (None, Some(ages)) => ages,
            (Some(gender), Some(ages)) => format!("{gender} \u{00b7} {ages}"),
        }
    }

    /// The `social.IntroAudience` wire projection.
    pub fn to_wire(self) -> serde_json::Value {
        serde_json::json!({
            "allowed_gender": self.gender.as_str(),
            "minimum_age": self.minimum_age,
            "maximum_age_exclusive": self.maximum_age_exclusive,
        })
    }

    /// Parse the `social.IntroAudience` wire projection.
    pub fn from_wire(value: &serde_json::Value) -> Result<Self, AudienceError> {
        let object = value.as_object().ok_or(AudienceError::InvalidGender)?;
        // Closed: an unknown field is a caller error, never ignored.
        if object
            .keys()
            .any(|key| !matches!(key.as_str(), "allowed_gender" | "minimum_age" | "maximum_age_exclusive"))
        {
            return Err(AudienceError::InvalidGender);
        }
        let gender = object
            .get("allowed_gender")
            .and_then(serde_json::Value::as_str)
            .and_then(GenderCondition::parse)
            .ok_or(AudienceError::InvalidGender)?;
        let read = |name: &str| -> Result<Option<i16>, AudienceError> {
            match object.get(name) {
                None | Some(serde_json::Value::Null) => Ok(None),
                Some(value) => value
                    .as_i64()
                    .and_then(|value| i16::try_from(value).ok())
                    .map(Some)
                    .ok_or(AudienceError::OutOfRange),
            }
        };
        Self::new(gender, read("minimum_age")?, read("maximum_age_exclusive")?)
    }
}
