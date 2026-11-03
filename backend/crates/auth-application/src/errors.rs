//! Transport-free classification of failures that may reach a public client.
//!
//! Application and domain code return these values instead of ad-hoc strings so
//! inbound adapters can choose a status code and a fixed public message without
//! ever forwarding internal detail. Nothing here imports `tonic`, `axum`, or
//! `sqlx`, so the hexagonal boundary holds.
//!
//! Messages are `&'static str` on purpose: a dynamically built string cannot be
//! passed by accident, which keeps database, pool, and provider text out of
//! responses.

pub type AuthResult<T> = Result<T, AuthError>;

/// Why an auth request failed, in terms the transport layer can act on.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    /// Unknown account, wrong password, inactive account, or locked account.
    /// Callers must not distinguish these to a public client.
    #[error("Invalid credentials")]
    InvalidCredentials,
    /// Malformed or out-of-policy request input.
    #[error("{0}")]
    InvalidInput(&'static str),
    /// Authenticated but not permitted to act on the target resource.
    #[error("Forbidden")]
    Forbidden,
    /// The requested resource does not exist.
    #[error("Not found")]
    NotFound,
    /// The request conflicts with existing state; the message is safe to show.
    #[error("{0}")]
    Conflict(&'static str),
    /// Another refresh attempt already rotated this token. Not a security
    /// failure and must not invalidate unrelated sessions.
    #[error("Refresh token conflict")]
    RefreshConflict,
    /// A presented credential is no longer valid.
    #[error("Credential is no longer valid")]
    CredentialRejected,
    /// Caller is over a quota.
    #[error("Too many requests")]
    RateLimited,
    /// Server is saturated and the caller may retry later.
    #[error("Service busy")]
    Busy,
    /// A required dependency could not be reached.
    #[error("Dependency unavailable")]
    DependencyUnavailable,
    /// Unclassified dependency or library failure; no cause crosses the port.
    #[error("Unexpected failure")]
    Unexpected,
    /// Controlled internal invariant rejection; retains only a fixed safe diagnostic.
    #[error("{0}")]
    Invariant(&'static str),
    /// Controlled domain provider failure with fixed safe code and message.
    #[error(transparent)]
    ProviderFailure(#[from] auth_domain::oauth_failure::OAuthProviderFailure),
}

impl AuthError {
    /// Stable machine identifier returned to clients alongside the status.
    pub fn code(&self) -> &'static str {
        match self {
            AuthError::InvalidCredentials => "AUTH_INVALID_CREDENTIALS",
            AuthError::InvalidInput(_) => "AUTH_INVALID_INPUT",
            AuthError::Forbidden => "AUTH_FORBIDDEN",
            AuthError::NotFound => "AUTH_NOT_FOUND",
            AuthError::Conflict(_) => "AUTH_CONFLICT",
            AuthError::RefreshConflict => "AUTH_REFRESH_CONFLICT",
            AuthError::CredentialRejected => "AUTH_CREDENTIAL_REJECTED",
            AuthError::RateLimited => "AUTH_RATE_LIMITED",
            AuthError::Busy => "AUTH_BUSY",
            AuthError::DependencyUnavailable => "AUTH_DEPENDENCY_UNAVAILABLE",
            AuthError::Unexpected | AuthError::Invariant(_) => "AUTH_INTERNAL",
            AuthError::ProviderFailure(failure) => failure.code(),
        }
    }

    /// Whether this failure describes a defect or outage rather than caller
    /// input. Telemetry treats these as availability events, not client errors.
    pub fn is_server_fault(&self) -> bool {
        matches!(
            self,
            AuthError::Busy
                | AuthError::DependencyUnavailable
                | AuthError::Unexpected
                | AuthError::ProviderFailure(_)
                | AuthError::Invariant(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_faults_are_distinguished_from_client_errors() {
        assert!(AuthError::Busy.is_server_fault());
        assert!(AuthError::DependencyUnavailable.is_server_fault());
        assert!(AuthError::Unexpected.is_server_fault());
        assert!(!AuthError::InvalidCredentials.is_server_fault());
        assert!(!AuthError::InvalidInput("bad email").is_server_fault());
    }

    #[test]
    fn credential_families_share_no_distinguishing_code() {
        // Every public credential failure uses one code, so the response cannot
        // be used to tell an unknown account from a wrong password.
        assert_eq!(
            AuthError::InvalidCredentials.code(),
            "AUTH_INVALID_CREDENTIALS"
        );
    }

    #[test]
    fn opaque_port_errors_cannot_hold_a_library_cause() {
        use std::error::Error;
        let error = AuthError::Unexpected;
        assert_eq!(error.to_string(), "Unexpected failure");
        assert!(error.source().is_none());
        assert_eq!(
            AuthError::Invariant("fixed invariant").code(),
            "AUTH_INTERNAL"
        );
    }
}
