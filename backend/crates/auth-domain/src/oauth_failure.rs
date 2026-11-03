use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthProviderFailure {
    code: &'static str,
    message: &'static str,
}

impl OAuthProviderFailure {
    pub fn new(code: &'static str, message: &'static str) -> Self {
        Self { code, message }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &'static str {
        self.message
    }

    pub fn code_invalid() -> Self {
        Self::new("PROVIDER_CODE_INVALID", "OAuth code is invalid")
    }

    pub fn token_request_failed() -> Self {
        Self::new(
            "PROVIDER_TOKEN_REQUEST_FAILED",
            "OAuth token exchange request failed",
        )
    }

    pub fn token_invalid_grant() -> Self {
        Self::new(
            "PROVIDER_TOKEN_INVALID_GRANT",
            "OAuth provider token exchange failed",
        )
    }

    pub fn token_invalid_client() -> Self {
        Self::new(
            "PROVIDER_TOKEN_INVALID_CLIENT",
            "OAuth provider token exchange failed",
        )
    }

    pub fn token_invalid_request() -> Self {
        Self::new(
            "PROVIDER_TOKEN_INVALID_REQUEST",
            "OAuth provider token exchange failed",
        )
    }

    pub fn token_http_failure() -> Self {
        Self::new(
            "PROVIDER_TOKEN_HTTP_FAILURE",
            "OAuth provider token exchange failed",
        )
    }

    pub fn token_response_invalid() -> Self {
        Self::new(
            "PROVIDER_TOKEN_RESPONSE_INVALID",
            "OAuth token response was invalid",
        )
    }

    pub fn token_access_token_missing() -> Self {
        Self::new(
            "PROVIDER_TOKEN_ACCESS_TOKEN_MISSING",
            "OAuth token response missing access_token",
        )
    }

    pub fn id_token_invalid() -> Self {
        Self::new(
            "PROVIDER_ID_TOKEN_INVALID",
            "OAuth provider ID token validation failed",
        )
    }

    pub fn userinfo_request_failed() -> Self {
        Self::new(
            "PROVIDER_USERINFO_REQUEST_FAILED",
            "OAuth userinfo request failed",
        )
    }

    pub fn userinfo_insufficient_scope() -> Self {
        Self::new(
            "PROVIDER_USERINFO_INSUFFICIENT_SCOPE",
            "OAuth provider userinfo request failed",
        )
    }

    pub fn userinfo_http_failure() -> Self {
        Self::new(
            "PROVIDER_USERINFO_HTTP_FAILURE",
            "OAuth provider userinfo request failed",
        )
    }

    pub fn userinfo_response_invalid() -> Self {
        Self::new(
            "PROVIDER_USERINFO_RESPONSE_INVALID",
            "OAuth userinfo response was invalid",
        )
    }

    pub fn userinfo_subject_missing() -> Self {
        Self::new(
            "PROVIDER_USERINFO_SUB_MISSING",
            "OAuth userinfo response missing sub",
        )
    }
}

impl Display for OAuthProviderFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl Error for OAuthProviderFailure {}
