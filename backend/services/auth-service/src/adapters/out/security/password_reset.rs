use crate::application::dto::password_reset_material_dto::PasswordResetMaterialDto;
use crate::application::password_reset_crypto::PasswordResetCryptoPort;
use base64ct::{Base64UrlUnpadded, Encoding};
use rand::RngCore;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Default)]
pub struct SecurePasswordReset;

impl PasswordResetCryptoPort for SecurePasswordReset {
    fn generate(&self) -> crate::application::errors::AuthResult<PasswordResetMaterialDto> {
        let result: anyhow::Result<_> = (|| {
            let mut raw = [0_u8; 32];
            rand::rngs::OsRng.try_fill_bytes(&mut raw)?;
            let token = Base64UrlUnpadded::encode_string(&raw);
            let fingerprint = self.fingerprint(&token);
            Ok(PasswordResetMaterialDto { token, fingerprint })
        })();
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    fn fingerprint(&self, token: &str) -> String {
        Base64UrlUnpadded::encode_string(&Sha256::digest(token.as_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_material_preserves_entropy_encoding_fingerprint_and_redaction() {
        let first = SecurePasswordReset.generate().unwrap();
        let second = SecurePasswordReset.generate().unwrap();
        assert_ne!(first.token, second.token);
        assert_eq!(first.token.len(), 43);
        assert_eq!(
            Base64UrlUnpadded::decode_vec(&first.token).unwrap().len(),
            32
        );
        assert_eq!(
            first.fingerprint,
            SecurePasswordReset.fingerprint(&first.token)
        );
        let rendered = format!("{first:?}");
        assert!(!rendered.contains(&first.token));
        assert!(!rendered.contains(&first.fingerprint));
        assert!(rendered.contains("[REDACTED]"));
    }
}
