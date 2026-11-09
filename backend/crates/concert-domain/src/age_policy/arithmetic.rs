//! Adapted pure arithmetic from Bigyeting 5eecdcaffd1a9d505b9ab2a974f8d5623217e90a
//! domain/privacy.rs. See docs/mcr/backend/intro-age-policy.md for exact hashes.
//! A derived age is private input, never verification or admission authority.
use chrono::{Datelike, NaiveDate};
use thiserror::Error;

pub const MIN_SUPPORTED_AGE: i32 = 14;
pub const MAX_SUPPORTED_AGE: i32 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SeoulDate(NaiveDate);

impl SeoulDate {
    pub const fn new(date: NaiveDate) -> Self {
        Self(date)
    }

    pub const fn as_naive_date(self) -> NaiveDate {
        self.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ExactAgeClaim {
    age_at_verification: u8,
    next_birthday_at: NaiveDate,
}

impl ExactAgeClaim {
    /// The stored pair, as the claim store records it.
    ///
    /// Used by the visibility SQL predicate's reconstruction of a scope and by
    /// the claim adapter, which receives the pair from the profile service. The
    /// range is the arithmetic module's own supported window, so a pair that
    /// could not have come from `derive_exact_age_claim` is refused.
    pub fn from_parts(age_at_verification: i16, next_birthday_at: SeoulDate) -> Option<Self> {
        if !(MIN_SUPPORTED_AGE..=MAX_SUPPORTED_AGE).contains(&i32::from(age_at_verification)) {
            return None;
        }
        Some(Self {
            age_at_verification: u8::try_from(age_at_verification).ok()?,
            next_birthday_at: next_birthday_at.as_naive_date(),
        })
    }

    pub const fn age_at_verification(self) -> u8 {
        self.age_at_verification
    }

    pub const fn next_birthday_at(self) -> SeoulDate {
        SeoulDate(self.next_birthday_at)
    }

    /// Returns claim age with the sole birthday transition possible in its 90-day lifetime.
    pub fn current_age(self, on: SeoulDate) -> u8 {
        self.age_at_verification + u8::from(on.0 >= self.next_birthday_at)
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ExactAgeError {
    #[error("Verifier birthyear is missing")]
    MissingBirthyear,
    #[error("Verifier birthday is missing")]
    MissingBirthday,
    #[error("Verifier birthyear must use YYYY format")]
    MalformedBirthyear,
    #[error("Verifier birthday must use MM-DD format")]
    MalformedBirthday,
    #[error("Verifier birthyear and birthday do not form a calendar date")]
    ImpossibleBirthday,
    #[error("Verifier birth date is after the verification date")]
    FutureBirthDate,
    #[error("claim is outside the supported range")]
    UnsupportedAge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProviderBirthDate {
    year: i32,
    month: u32,
    day: u32,
}

impl ProviderBirthDate {
    fn parse(birthyear: Option<&str>, birthday: Option<&str>) -> Result<Self, ExactAgeError> {
        let birthyear = birthyear.ok_or(ExactAgeError::MissingBirthyear)?;
        let birthday = birthday.ok_or(ExactAgeError::MissingBirthday)?;
        if birthyear.len() != 4 || !birthyear.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(ExactAgeError::MalformedBirthyear);
        }
        let birthday_bytes = birthday.as_bytes();
        if birthday_bytes.len() != 5
            || birthday_bytes[2] != b'-'
            || !birthday_bytes[..2]
                .iter()
                .chain(&birthday_bytes[3..])
                .all(u8::is_ascii_digit)
        {
            return Err(ExactAgeError::MalformedBirthday);
        }
        let year = birthyear
            .parse::<i32>()
            .map_err(|_| ExactAgeError::MalformedBirthyear)?;
        let month = birthday[..2]
            .parse::<u32>()
            .map_err(|_| ExactAgeError::MalformedBirthday)?;
        let day = birthday[3..]
            .parse::<u32>()
            .map_err(|_| ExactAgeError::MalformedBirthday)?;
        NaiveDate::from_ymd_opt(year, month, day).ok_or(ExactAgeError::ImpossibleBirthday)?;
        Ok(Self { year, month, day })
    }

    fn birthday_in_year(self, year: i32) -> Result<NaiveDate, ExactAgeError> {
        NaiveDate::from_ymd_opt(year, self.month, self.day)
            .or_else(|| NaiveDate::from_ymd_opt(year, 2, 28).filter(|_| self.is_leap_day()))
            .ok_or(ExactAgeError::ImpossibleBirthday)
    }

    const fn is_leap_day(self) -> bool {
        self.month == 2 && self.day == 29
    }
}

pub fn derive_exact_age_claim(
    birthyear: Option<&str>,
    birthday: Option<&str>,
    verification_date: SeoulDate,
) -> Result<ExactAgeClaim, ExactAgeError> {
    let birth_date = ProviderBirthDate::parse(birthyear, birthday)?;
    let birthday_at_verification_year = birth_date.birthday_in_year(verification_date.0.year())?;
    let full_birth_date =
        NaiveDate::from_ymd_opt(birth_date.year, birth_date.month, birth_date.day)
            .ok_or(ExactAgeError::ImpossibleBirthday)?;
    if full_birth_date > verification_date.0 {
        return Err(ExactAgeError::FutureBirthDate);
    }

    let age = verification_date.0.year()
        - birth_date.year
        - i32::from(verification_date.0 < birthday_at_verification_year);
    if !(MIN_SUPPORTED_AGE..=MAX_SUPPORTED_AGE).contains(&age) {
        return Err(ExactAgeError::UnsupportedAge);
    }
    let age_at_verification = u8::try_from(age).map_err(|_| ExactAgeError::UnsupportedAge)?;
    let next_birthday_at = if birthday_at_verification_year > verification_date.0 {
        birthday_at_verification_year
    } else {
        birth_date.birthday_in_year(verification_date.0.year() + 1)?
    };
    Ok(ExactAgeClaim {
        age_at_verification,
        next_birthday_at,
    })
}
