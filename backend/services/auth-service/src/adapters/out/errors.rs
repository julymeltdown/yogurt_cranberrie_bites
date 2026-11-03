//! Classify library failures before they leave an outbound Auth capability.
use crate::application::errors::AuthError;
use crate::domain::oauth_failure::OAuthProviderFailure;

pub fn boundary_error(error: anyhow::Error) -> AuthError {
    if let Some(known) = error.downcast_ref::<AuthError>() {
        return known.clone();
    }
    if let Some(provider) = error.downcast_ref::<OAuthProviderFailure>() {
        return AuthError::ProviderFailure(provider.clone());
    }
    if error.chain().any(|cause| cause.is::<tonic::Status>()) {
        return AuthError::DependencyUnavailable;
    }
    AuthError::Unexpected
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn library_failures_are_opaque_and_transport_failures_are_unavailable() {
        for raw in [
            anyhow::Error::new(sqlx::Error::Protocol(
                "private database/password/user".into(),
            )),
            anyhow::Error::new(
                serde_json::from_str::<serde_json::Value>("private-json-body").unwrap_err(),
            ),
            anyhow::anyhow!("private upstream/secret"),
        ] {
            let error = boundary_error(raw.context("private request/payload"));
            assert_eq!(error, AuthError::Unexpected);
            assert!(error.source().is_none());
            assert!(!format!("{error:?}").contains("private"));
        }
        let error = boundary_error(
            anyhow::Error::new(tonic::Status::unavailable("private grpc peer"))
                .context("private endpoint"),
        );
        assert_eq!(error, AuthError::DependencyUnavailable);
        assert!(error.source().is_none());
    }

    #[test]
    fn explicit_auth_policy_precedes_provider_cause_and_safe_provider_codes_survive() {
        let provider = OAuthProviderFailure::code_invalid();
        let error = boundary_error(
            anyhow::Error::new(provider.clone()).context("private provider/payload"),
        );
        assert_eq!(error, AuthError::ProviderFailure(provider.clone()));
        let error = boundary_error(
            anyhow::Error::new(provider)
                .context(AuthError::Busy)
                .context("private context"),
        );
        assert_eq!(error, AuthError::Busy);
        assert!(!format!("{error:?}").contains("private"));
    }
}
