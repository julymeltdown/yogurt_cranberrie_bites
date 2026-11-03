use auth_service::domain::oauth_failure::OAuthProviderFailure;

#[test]
fn oauth_provider_failures_expose_stable_audit_codes_and_safe_messages() {
    let cases = [
        (
            OAuthProviderFailure::code_invalid(),
            "PROVIDER_CODE_INVALID",
            "OAuth code is invalid",
        ),
        (
            OAuthProviderFailure::token_request_failed(),
            "PROVIDER_TOKEN_REQUEST_FAILED",
            "OAuth token exchange request failed",
        ),
        (
            OAuthProviderFailure::token_invalid_grant(),
            "PROVIDER_TOKEN_INVALID_GRANT",
            "OAuth provider token exchange failed",
        ),
        (
            OAuthProviderFailure::token_invalid_client(),
            "PROVIDER_TOKEN_INVALID_CLIENT",
            "OAuth provider token exchange failed",
        ),
        (
            OAuthProviderFailure::token_invalid_request(),
            "PROVIDER_TOKEN_INVALID_REQUEST",
            "OAuth provider token exchange failed",
        ),
        (
            OAuthProviderFailure::token_http_failure(),
            "PROVIDER_TOKEN_HTTP_FAILURE",
            "OAuth provider token exchange failed",
        ),
        (
            OAuthProviderFailure::token_response_invalid(),
            "PROVIDER_TOKEN_RESPONSE_INVALID",
            "OAuth token response was invalid",
        ),
        (
            OAuthProviderFailure::token_access_token_missing(),
            "PROVIDER_TOKEN_ACCESS_TOKEN_MISSING",
            "OAuth token response missing access_token",
        ),
        (
            OAuthProviderFailure::id_token_invalid(),
            "PROVIDER_ID_TOKEN_INVALID",
            "OAuth provider ID token validation failed",
        ),
        (
            OAuthProviderFailure::userinfo_request_failed(),
            "PROVIDER_USERINFO_REQUEST_FAILED",
            "OAuth userinfo request failed",
        ),
        (
            OAuthProviderFailure::userinfo_insufficient_scope(),
            "PROVIDER_USERINFO_INSUFFICIENT_SCOPE",
            "OAuth provider userinfo request failed",
        ),
        (
            OAuthProviderFailure::userinfo_http_failure(),
            "PROVIDER_USERINFO_HTTP_FAILURE",
            "OAuth provider userinfo request failed",
        ),
        (
            OAuthProviderFailure::userinfo_response_invalid(),
            "PROVIDER_USERINFO_RESPONSE_INVALID",
            "OAuth userinfo response was invalid",
        ),
        (
            OAuthProviderFailure::userinfo_subject_missing(),
            "PROVIDER_USERINFO_SUB_MISSING",
            "OAuth userinfo response missing sub",
        ),
    ];

    for (failure, code, message) in cases {
        assert_eq!(failure.code(), code);
        assert_eq!(failure.to_string(), message);
    }
}

#[test]
fn oauth_provider_failure_new_preserves_custom_code_and_message() {
    let failure = OAuthProviderFailure::new("CUSTOM_PROVIDER_FAILURE", "Custom safe message");

    assert_eq!(failure.code(), "CUSTOM_PROVIDER_FAILURE");
    assert_eq!(failure.to_string(), "Custom safe message");
}
