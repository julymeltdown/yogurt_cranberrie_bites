//! Durable onboarding operations. Provider proofs must already be authenticated.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResendVerificationRequest {
    pub email: String,
}

impl std::fmt::Debug for ResendVerificationRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResendVerificationRequest")
            .field("email", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResendVerificationResponse {
    pub status: String,
}

#[async_trait]
pub trait AuthOnboardingPort: Send + Sync {
    /// Resolve/create the user and bind a verified provider identity atomically.
    /// Never transfer a provider subject between users or reactivate an account.
    async fn resolve_oauth_user(
        &self,
        proof: &crate::dto::repository_dto::AuthOAuthProofDto,
    ) -> crate::errors::AuthResult<crate::dto::repository_dto::AuthUserRecordDto>;

    /// Rotate a pending email challenge and enqueue delivery atomically.
    /// Unknown, active, deleted and rate-limited accounts are indistinguishable.
    async fn resend_verification(&self, email: &str, code: &str) -> crate::errors::AuthResult<()>;
}

#[cfg(test)]
mod secret_debug_tests {
    use super::ResendVerificationRequest;

    #[test]
    fn resend_verification_request_debug_redacts_email() {
        let request = ResendVerificationRequest {
            email: "private-verification-email@example.test".into(),
        };

        let rendered = format!("{request:?}");

        assert!(
            !rendered.contains("private-verification-email@example.test"),
            "{rendered}"
        );
        assert!(rendered.contains("[REDACTED]"));
    }
}
