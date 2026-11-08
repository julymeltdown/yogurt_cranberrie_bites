//! T-401: intro post content rules.
//!
//! A draft may have an empty body; a published post may not. The contact-bypass
//! rule from `concert_domain::intro_policy::contact` applies to a published
//! body, so a post that tries to smuggle a phone number, an email address or a
//! messenger handle past the audience restriction fails to publish.
//!
//! The single-line fields (`currently_seeking`, `interaction_style`) are the
//! fields a card shows on one line, so a newline in either would break the card
//! layout and, worse, let a post smuggle multi-line content into a slot the
//! contract bounds to 120 characters. Both are rejected here and the database
//! carries the same rule.
use concert_domain::intro_policy::contact;

/// The frozen `body` bound, for both a draft and a published post.
pub const MAX_BODY: usize = 1000;
/// The frozen single-line bound for `currently_seeking` and `interaction_style`.
pub const MAX_SINGLE_LINE: usize = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentError {
    /// The body exceeded 1000 characters.
    BodyTooLong,
    /// The body contained a NUL byte.
    BodyContainsNul,
    /// Publishing was attempted with an empty or whitespace-only body.
    EmptyBody,
    /// The published body matched the contact-bypass policy.
    ContactBypass,
    /// A single-line field exceeded 120 characters.
    SingleLineTooLong,
    /// A single-line field contained a newline, a carriage return or another
    /// control character.
    SingleLineNotSingleLine,
}

impl std::fmt::Display for ContentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid intro input: {self:?}")
    }
}

impl std::error::Error for ContentError {}

/// The author-editable content of an intro post, before it is stored.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct IntroContent {
    pub body: String,
    pub currently_seeking: Option<String>,
    pub interaction_style: Option<String>,
}

impl IntroContent {
    /// Validate for a draft. An empty body is allowed; the contact rule is not
    /// applied, because a draft is private to its author and is expected to be
    /// half-written.
    pub fn validate_draft(&self) -> Result<(), ContentError> {
        self.validate_shape()?;
        Ok(())
    }

    /// Validate for publication. The body must be non-empty and must pass the
    /// contact-bypass rule.
    pub fn validate_publish(&self) -> Result<(), ContentError> {
        self.validate_shape()?;
        if self.body.trim().is_empty() {
            return Err(ContentError::EmptyBody);
        }
        if contact::contact_bypass(&self.body) {
            return Err(ContentError::ContactBypass);
        }
        Ok(())
    }

    /// The rules both states share.
    fn validate_shape(&self) -> Result<(), ContentError> {
        if self.body.chars().count() > MAX_BODY {
            return Err(ContentError::BodyTooLong);
        }
        if self.body.contains('\0') {
            return Err(ContentError::BodyContainsNul);
        }
        for field in [&self.currently_seeking, &self.interaction_style] {
            if let Some(value) = field {
                validate_single_line(value)?;
            }
        }
        Ok(())
    }
}

/// A single-line field: at most 120 characters and no control characters.
///
/// The contract allows an empty string, so `""` is valid. A newline is not.
pub fn validate_single_line(value: &str) -> Result<(), ContentError> {
    if value.chars().count() > MAX_SINGLE_LINE {
        return Err(ContentError::SingleLineTooLong);
    }
    if value.chars().any(char::is_control) {
        return Err(ContentError::SingleLineNotSingleLine);
    }
    Ok(())
}
