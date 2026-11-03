//! RSA signing keys for verifier and refresher tests.
//!
//! Generating an RSA key is slow in a debug build, so each key is generated
//! once per test binary and shared.

use base64ct::{Base64UrlUnpadded, Encoding};
use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use rand::rngs::OsRng;
use rsa::pkcs8::EncodePrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde_json::{json, Value};
use std::sync::OnceLock;
use uuid::Uuid;

pub const ISSUER: &str = "auth-service-test";
pub const AUDIENCE: &str = "port-rust-api-test";

pub struct SigningKey {
    pub kid: &'static str,
    encoding: EncodingKey,
    public_key: RsaPublicKey,
}

impl SigningKey {
    fn generate(kid: &'static str, bits: usize) -> Self {
        let private_key = RsaPrivateKey::new(&mut OsRng, bits).expect("test RSA key");
        let pem = private_key
            .to_pkcs8_pem(Default::default())
            .expect("private key PEM");
        Self {
            kid,
            encoding: EncodingKey::from_rsa_pem(pem.as_bytes()).expect("encoding key"),
            public_key: RsaPublicKey::from(&private_key),
        }
    }

    /// The key as an issuer publishes it.
    pub fn public_jwk(&self) -> Value {
        json!({
            "kty": "RSA",
            "use": "sig",
            "alg": "RS256",
            "kid": self.kid,
            "n": self.modulus(),
            "e": self.exponent()
        })
    }

    pub fn modulus(&self) -> String {
        Base64UrlUnpadded::encode_string(&self.public_key.n().to_bytes_be())
    }

    pub fn exponent(&self) -> String {
        Base64UrlUnpadded::encode_string(&self.public_key.e().to_bytes_be())
    }

    /// A header naming this key.
    pub fn header(&self) -> Header {
        Header {
            alg: Algorithm::RS256,
            kid: Some(self.kid.to_owned()),
            ..Header::default()
        }
    }

    /// An access token for `subject`, signed with this key and naming it.
    pub fn access_token(&self, subject: &str) -> String {
        self.access_token_with_header(subject, self.header())
    }

    /// An access token signed with this key under an arbitrary header.
    pub fn access_token_with_header(&self, subject: &str, header: Header) -> String {
        let now = Utc::now().timestamp();
        let claims = json!({
            "sub": subject,
            "iss": ISSUER,
            "aud": AUDIENCE,
            "iat": now,
            "exp": now + 900,
            "jti": Uuid::new_v4().to_string(),
            "token_type": "access",
            "roles": ["ROLE_USER"]
        });
        jsonwebtoken::encode(&header, &claims, &self.encoding).expect("signed token")
    }
}

/// A published key set holding `keys`, in order.
pub fn key_set(keys: &[&SigningKey]) -> String {
    json!({ "keys": keys.iter().map(|key| key.public_jwk()).collect::<Vec<_>>() }).to_string()
}

pub fn first_key() -> &'static SigningKey {
    static KEY: OnceLock<SigningKey> = OnceLock::new();
    KEY.get_or_init(|| SigningKey::generate("rotation-first", 2048))
}

pub fn second_key() -> &'static SigningKey {
    static KEY: OnceLock<SigningKey> = OnceLock::new();
    KEY.get_or_init(|| SigningKey::generate("rotation-second", 2048))
}

/// A 1024-bit key, below the accepted modulus size.
pub fn weak_key() -> &'static SigningKey {
    static KEY: OnceLock<SigningKey> = OnceLock::new();
    KEY.get_or_init(|| SigningKey::generate("weak", 1024))
}
