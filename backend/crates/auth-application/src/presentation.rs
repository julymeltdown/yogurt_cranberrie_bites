//! Pure public failure projection. Transport codes, headers and logging stay at the edge.
use crate::errors::AuthError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthFailureKind {
    Credential,
    InvalidInput,
    Denied,
    Missing,
    Conflict,
    RefreshConflict,
    RateLimited,
    Busy,
    DependencyUnavailable,
    Internal,
    ProviderFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthFailureDto {
    pub kind: AuthFailureKind,
    pub code: &'static str,
    pub message: &'static str,
    /// Safe historical development HTTP message for known client failures.
    pub legacy_message: &'static str,
}

pub fn present_auth_error(error: Option<&AuthError>) -> AuthFailureDto {
    use AuthFailureKind as K;
    let (kind, code, message, legacy_message) = match error {
        Some(AuthError::Invariant(message)) => {
            (K::Internal, "AUTH_INTERNAL", "AUTH_INTERNAL", *message)
        }
        Some(AuthError::ProviderFailure(failure)) => {
            return present_oauth_provider_failure(failure)
        }
        Some(AuthError::InvalidCredentials) => (
            K::Credential,
            "AUTH_INVALID_CREDENTIALS",
            "AUTH_INVALID_CREDENTIALS",
            "Invalid credentials",
        ),
        Some(AuthError::InvalidInput(message)) => {
            (K::InvalidInput, "AUTH_INVALID_INPUT", *message, *message)
        }
        Some(AuthError::Forbidden) => (K::Denied, "AUTH_FORBIDDEN", "AUTH_FORBIDDEN", "Forbidden"),
        Some(AuthError::NotFound) => (K::Missing, "AUTH_NOT_FOUND", "AUTH_NOT_FOUND", "Not found"),
        Some(AuthError::Conflict(message)) => (K::Conflict, "AUTH_CONFLICT", *message, *message),
        Some(AuthError::RefreshConflict) => (
            K::RefreshConflict,
            "AUTH_REFRESH_CONFLICT",
            "AUTH_REFRESH_CONFLICT",
            "Refresh token conflict",
        ),
        Some(AuthError::CredentialRejected) => (
            K::Credential,
            "AUTH_CREDENTIAL_REJECTED",
            "AUTH_CREDENTIAL_REJECTED",
            "Credential is no longer valid",
        ),
        Some(AuthError::RateLimited) => (
            K::RateLimited,
            "AUTH_RATE_LIMITED",
            "AUTH_RATE_LIMITED",
            "Too many requests",
        ),
        Some(AuthError::Busy) => (K::Busy, "AUTH_BUSY", "AUTH_BUSY", "Service busy"),
        Some(AuthError::DependencyUnavailable) => (
            K::DependencyUnavailable,
            "AUTH_DEPENDENCY_UNAVAILABLE",
            "AUTH_DEPENDENCY_UNAVAILABLE",
            "Dependency unavailable",
        ),
        Some(AuthError::Unexpected) | None => (
            K::Internal,
            "AUTH_INTERNAL",
            "AUTH_INTERNAL",
            "Unexpected failure",
        ),
    };
    AuthFailureDto {
        kind,
        code,
        message,
        legacy_message,
    }
}

/// Preserve controlled provider failures without exposing an arbitrary cause.
/// Development HTTP historically presents these fixed messages as 400, while
/// gRPC keeps its existing generic Internal response.
pub fn present_oauth_provider_failure(
    failure: &auth_domain::oauth_failure::OAuthProviderFailure,
) -> AuthFailureDto {
    AuthFailureDto {
        kind: AuthFailureKind::ProviderFailure,
        code: failure.code(),
        message: "AUTH_INTERNAL",
        legacy_message: failure.message(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_known_provider_failures_have_a_controlled_neutral_projection() {
        use auth_domain::oauth_failure::OAuthProviderFailure as P;
        for failure in [
            P::code_invalid(),
            P::token_request_failed(),
            P::token_invalid_grant(),
            P::token_invalid_client(),
            P::token_invalid_request(),
            P::token_http_failure(),
            P::token_response_invalid(),
            P::token_access_token_missing(),
            P::id_token_invalid(),
            P::userinfo_request_failed(),
            P::userinfo_insufficient_scope(),
            P::userinfo_http_failure(),
            P::userinfo_response_invalid(),
            P::userinfo_subject_missing(),
        ] {
            let dto = present_oauth_provider_failure(&failure);
            assert_eq!(
                present_auth_error(Some(&AuthError::ProviderFailure(failure.clone()))),
                dto
            );
            assert_eq!(dto.kind, AuthFailureKind::ProviderFailure);
            assert_eq!(dto.code, failure.code());
            assert_eq!(dto.legacy_message, failure.to_string());
            assert_eq!(dto.message, "AUTH_INTERNAL");
        }
    }

    #[test]
    fn complete_neutral_auth_failure_projection_preserves_codes_and_safe_messages() {
        use AuthFailureKind as K;
        for (error, kind, message) in [
            (
                AuthError::InvalidCredentials,
                K::Credential,
                "AUTH_INVALID_CREDENTIALS",
            ),
            (
                AuthError::InvalidInput("Invalid email"),
                K::InvalidInput,
                "Invalid email",
            ),
            (AuthError::Forbidden, K::Denied, "AUTH_FORBIDDEN"),
            (AuthError::NotFound, K::Missing, "AUTH_NOT_FOUND"),
            (
                AuthError::Conflict("Email already registered"),
                K::Conflict,
                "Email already registered",
            ),
            (
                AuthError::RefreshConflict,
                K::RefreshConflict,
                "AUTH_REFRESH_CONFLICT",
            ),
            (
                AuthError::CredentialRejected,
                K::Credential,
                "AUTH_CREDENTIAL_REJECTED",
            ),
            (AuthError::RateLimited, K::RateLimited, "AUTH_RATE_LIMITED"),
            (AuthError::Busy, K::Busy, "AUTH_BUSY"),
            (
                AuthError::DependencyUnavailable,
                K::DependencyUnavailable,
                "AUTH_DEPENDENCY_UNAVAILABLE",
            ),
            (AuthError::Unexpected, K::Internal, "AUTH_INTERNAL"),
        ] {
            let dto = present_auth_error(Some(&error));
            assert_eq!(dto.kind, kind);
            assert_eq!(dto.code, error.code());
            assert_eq!(dto.message, message);
            assert_eq!(dto.legacy_message, error.to_string());
            assert!(!format!("{dto:?}").contains("private"));
        }
        let invariant = present_auth_error(Some(&AuthError::Invariant("fixed invariant")));
        assert_eq!(invariant.kind, K::Internal);
        assert_eq!(invariant.message, "AUTH_INTERNAL");
        let unknown = present_auth_error(None);
        assert_eq!(unknown.kind, K::Internal);
        assert_eq!(unknown.message, "AUTH_INTERNAL");
    }
}
