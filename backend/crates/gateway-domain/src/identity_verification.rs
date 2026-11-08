//! The identity verification callback's closed vocabulary.
//!
//! The callback is the one browser-facing operation that answers with a
//! redirect instead of a body, and it is the one operation that runs without a
//! session. Both facts make it the most dangerous route in the product, so the
//! values it can produce are enumerated here rather than assembled ad hoc:
//!
//! * [`CallbackReply`] can only be constructed from a known result marker and a
//!   return path from the closed start allowlist. A producer cannot inject an
//!   arbitrary `Location`.
//! * [`CallbackReplyError`] has exactly two shapes: the state was not usable
//!   (`400`, revealing nothing) and the dependency was unavailable (`503`).
//!   There is no third answer, so the route cannot leak which of "unknown",
//!   "expired" or "already used" applied.
//!
//! The result markers are the donor's `NaverCallbackResult` strings, unchanged,
//! so the browser-side vocabulary and the donor's are the same.

/// The closed set of result markers a callback may report.
pub const RESULT_MARKERS: [&str; 8] = [
    "success",
    "duplicate",
    "profile_incomplete",
    "age_not_supported",
    "failed",
    "cancelled",
    "expired",
    "replayed",
];

/// The closed set of return paths the start operation accepts.
///
/// This is deliberately duplicated from the profile service rather than shared:
/// the two services communicate only through the frozen contract, and the
/// gateway must be able to refuse a producer reply it did not expect even if the
/// producer is buggy. The values are contract constants, not configuration.
pub const RETURN_PATHS: [&str; 3] = ["/me/identity", "/me/settings", "/me/profile"];

/// A validated callback reply: a result marker and a same-origin return path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackReply {
    return_path: &'static str,
    result: &'static str,
}

impl CallbackReply {
    /// Validate a producer reply.
    ///
    /// Both values must be members of their closed sets. A path that is not one
    /// of the allowlist entries is refused rather than sanitized: sanitizing
    /// would leave the redirect target partly caller-influenced.
    pub fn validate(
        return_path: String,
        result: String,
    ) -> Result<Self, CallbackReplyError> {
        let return_path = RETURN_PATHS
            .into_iter()
            .find(|allowed| *allowed == return_path)
            .ok_or(CallbackReplyError::Unavailable)?;
        let result = RESULT_MARKERS
            .into_iter()
            .find(|allowed| *allowed == result)
            .ok_or(CallbackReplyError::Unavailable)?;
        Ok(Self {
            return_path,
            result,
        })
    }

    pub const fn return_path(&self) -> &'static str {
        self.return_path
    }

    pub const fn result(&self) -> &'static str {
        self.result
    }

    /// The `Location` header value.
    ///
    /// Always a same-origin absolute path. The only variable part is the result
    /// marker, which is one of eight constants, so this cannot carry a claim
    /// value, a provider code or a caller-supplied string.
    pub fn location(&self) -> String {
        format!("{}?identity={}", self.return_path, self.result)
    }
}

/// Why a callback did not produce a redirect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackReplyError {
    /// The state was missing, unknown, expired, or already used. The route
    /// answers `400` with a fixed body.
    InvalidState,
    /// The profile service could not be reached, or answered something outside
    /// the closed vocabulary. The route answers `503`.
    Unavailable,
}

impl CallbackReplyError {
    pub const fn is_invalid_state(self) -> bool {
        matches!(self, Self::InvalidState)
    }
}
