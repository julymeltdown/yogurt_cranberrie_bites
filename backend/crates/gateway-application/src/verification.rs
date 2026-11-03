//! Typed public resend boundary. Persistence and transport errors remain sanitized.
use crate::dto::verification_dto::{ResendVerificationRequest, ResendVerificationResponse};
use async_trait::async_trait;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum VerificationError {
    #[error("Invalid email")]
    InvalidEmail,
    #[error("Verification service unavailable")]
    Unavailable,
    #[error("Verification request timed out")]
    Timeout,
    #[error("Invalid verification service response")]
    InvalidResponse,
    #[error("Verification request failed")]
    Internal,
}

#[async_trait]
pub trait VerificationClientPort: Send + Sync {
    /// Return only ACCEPTED, or a sanitized typed failure. Do not auto-retry.
    async fn resend_verification(
        &self,
        request: ResendVerificationRequest,
    ) -> Result<ResendVerificationResponse, VerificationError>;
}

#[derive(Clone)]
pub struct VerificationGatewayService {
    client: Arc<dyn VerificationClientPort>,
}

impl VerificationGatewayService {
    pub fn new(client: Arc<dyn VerificationClientPort>) -> Self {
        Self { client }
    }

    pub async fn resend_verification(
        &self,
        request: ResendVerificationRequest,
    ) -> Result<ResendVerificationResponse, VerificationError> {
        let email = normalized_email(&request.email)?;
        self.client
            .resend_verification(ResendVerificationRequest { email })
            .await
    }
}

fn normalized_email(value: &str) -> Result<String, VerificationError> {
    let value = value.trim();
    let parts: Vec<_> = value.split('@').collect();
    if value.len() > 254
        || value
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control())
        || parts.len() != 2
        || parts[0].len() <= 1
        || !parts[1].contains('.')
    {
        return Err(VerificationError::InvalidEmail);
    }
    Ok(value.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_before_transport() {
        assert_eq!(
            normalized_email("  USER@Example.COM ").unwrap(),
            "user@example.com"
        );
    }

    #[test]
    fn rejects_invalid_or_unbounded_input() {
        for value in [
            "",
            "bad",
            "a@example.com",
            "aa@localhost",
            "aa@@example.com",
            "aa\n@example.com",
            "aa\0@example.com",
        ] {
            assert_eq!(
                normalized_email(value),
                Err(VerificationError::InvalidEmail)
            );
        }
        assert_eq!(
            normalized_email(&"a".repeat(255)),
            Err(VerificationError::InvalidEmail)
        );
    }
}
