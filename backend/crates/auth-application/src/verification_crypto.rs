use crate::dto::verification_code_dto::VerificationCodeDto;

/// Email challenge entropy and encoding are outbound security capabilities.
pub trait VerificationCryptoPort: Send + Sync {
    fn generate(&self) -> crate::errors::AuthResult<VerificationCodeDto>;
}
