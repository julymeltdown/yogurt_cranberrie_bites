use crate::application::auth_runtime::OAuthAuthorizationCryptoPort;
use crate::application::dto::oauth_authorization_dto::OAuthAuthorizationMaterialDto;
use base64ct::{Base64UrlUnpadded, Encoding};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Clone, Copy, Default)]
pub struct SecureOAuthAuthorization;

impl OAuthAuthorizationCryptoPort for SecureOAuthAuthorization {
    fn generate(&self) -> crate::application::errors::AuthResult<OAuthAuthorizationMaterialDto> {
        let result: anyhow::Result<_> = {
            let code_verifier: String = thread_rng()
                .sample_iter(&Alphanumeric)
                .take(64)
                .map(char::from)
                .collect();
            let code_challenge = Base64UrlUnpadded::encode_string(
                Sha256::digest(code_verifier.as_bytes()).as_slice(),
            );
            Ok(OAuthAuthorizationMaterialDto {
                state: Uuid::new_v4().to_string(),
                nonce: Uuid::new_v4().to_string(),
                code_verifier,
                code_challenge,
            })
        };
        result.map_err(crate::adapters::out::errors::boundary_error)
    }
}
