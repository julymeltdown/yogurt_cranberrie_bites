use crate::dto::password_reset_material_dto::PasswordResetMaterialDto;

/// Reset credential generation and fingerprinting are outbound security work.
pub trait PasswordResetCryptoPort: Send + Sync {
    fn generate(&self) -> crate::errors::AuthResult<PasswordResetMaterialDto>;
    fn fingerprint(&self, token: &str) -> String;
}
