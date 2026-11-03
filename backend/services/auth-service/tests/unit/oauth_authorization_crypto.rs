use auth_service::adapters::out::security::oauth_authorization::SecureOAuthAuthorization;
use auth_service::application::auth_runtime::OAuthAuthorizationCryptoPort;
use base64ct::{Base64UrlUnpadded, Encoding};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[test]
fn real_authorization_credentials_preserve_uuid_and_s256_pkce_contract() {
    let first = SecureOAuthAuthorization.generate().expect("credentials");
    let second = SecureOAuthAuthorization.generate().expect("credentials");
    assert_ne!(first.state, second.state);
    assert_ne!(first.nonce, second.nonce);
    assert_ne!(first.code_verifier, second.code_verifier);
    for material in [first, second] {
        assert_eq!(
            Uuid::parse_str(&material.state).unwrap().get_version_num(),
            4
        );
        assert_eq!(
            Uuid::parse_str(&material.nonce).unwrap().get_version_num(),
            4
        );
        assert_ne!(material.state, material.nonce);
        assert_eq!(material.code_verifier.len(), 64);
        assert!(material
            .code_verifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric()));
        assert_eq!(material.code_challenge.len(), 43);
        assert_eq!(
            Base64UrlUnpadded::decode_vec(&material.code_challenge).unwrap(),
            Sha256::digest(material.code_verifier.as_bytes()).as_slice()
        );
    }
}

#[test]
fn credential_dto_debug_omits_all_protocol_secrets() {
    let material = SecureOAuthAuthorization.generate().unwrap();
    let debug = format!("{material:?}");
    for secret in [
        &material.state,
        &material.nonce,
        &material.code_verifier,
        &material.code_challenge,
    ] {
        assert!(!debug.contains(secret));
    }
    assert!(debug.contains("[REDACTED]"));
}
