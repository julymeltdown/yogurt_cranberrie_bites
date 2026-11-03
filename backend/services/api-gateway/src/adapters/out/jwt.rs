//! Access-token verification against the issuer's published signing keys.
//!
//! The verifier holds a replaceable key set. A Gateway that learned the
//! issuer's keys only at startup could not follow a key rotation: a key
//! published after it started stayed unknown until a redeploy, and a key
//! withdrawn after a compromise kept verifying. Installing a key set replaces
//! the whole set, so publishing and withdrawing a key take effect the same way.
//!
//! Fetching the set is `adapters::out::signing_keys`. This adapter decides
//! what a usable key set is and how a token is checked against it, and it
//! never takes a key from the token being checked. It is the only place that
//! uses the JWT library; core policy receives the neutral
//! [VerifiedAccessToken] claims.

use base64ct::{Base64UrlUnpadded, Encoding};
pub use gateway_domain::access_token::VerifiedAccessToken;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

pub mod concert_access;

/// Largest key-set document accepted, in bytes.
///
/// A published key is well under 1 KiB of JSON, so real key sets are a few
/// kilobytes. The bound keeps a misbehaving issuer response from costing
/// unbounded memory and parse time on every refresh.
pub const MAX_JWKS_DOCUMENT_BYTES: usize = 64 * 1024;

/// Most keys a key-set document may list, whether or not they are usable here.
pub const MAX_JWKS_KEYS: usize = 16;

/// Smallest accepted RSA modulus, in bits.
pub const MIN_RSA_MODULUS_BITS: usize = 2048;

/// Largest accepted RSA modulus, in bits. The issuer enforces the same bound.
pub const MAX_RSA_MODULUS_BITS: usize = 4096;

/// Verifies access tokens against the installed key set.
///
/// Clones share one key set, so a set installed through any clone is used by
/// every request from then on.
#[derive(Clone)]
pub struct JwtVerifier {
    issuer: String,
    audience: String,
    keys: Arc<RwLock<Arc<VerificationKeys>>>,
}

/// An immutable, validated set of RS256 verification keys indexed by `kid`.
#[derive(Clone, Default)]
pub struct VerificationKeys {
    keys: HashMap<String, DecodingKey>,
}

/// Why a key-set document cannot be installed.
///
/// A document is installed whole or not at all. Installing the usable part of a
/// defective document would silently drop the keys the issuer meant to publish.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeySetError {
    #[error("JWKS document exceeds {MAX_JWKS_DOCUMENT_BYTES} bytes")]
    TooLarge,
    #[error("JWKS document is not a JWK Set: {0}")]
    Malformed(String),
    #[error("JWKS document lists more than {MAX_JWKS_KEYS} keys")]
    TooManyKeys,
    #[error("JWKS key is missing kid")]
    MissingKid,
    #[error("JWKS key id is not unique")]
    DuplicateKid,
    #[error(
        "JWKS key is not a usable RS256 public key of {MIN_RSA_MODULUS_BITS} to {MAX_RSA_MODULUS_BITS} bits"
    )]
    InvalidKey,
    #[error("JWT JWKS does not contain an RS256 key")]
    NoUsableKey,
}

/// Why an access token was not accepted.
///
/// The neutral variant set the core reasons about. Library errors are mapped
/// into it here and their detail is not retained, so a decoded claim cannot
/// travel back out of the adapter through an error.
pub use gateway_domain::access_token::TokenRejection;

impl VerificationKeys {
    /// Parse and validate a JWK Set document.
    ///
    /// Keys for another algorithm or use are skipped, so the issuer can publish
    /// them without breaking this verifier. An RS256 signing key that is
    /// malformed, weak, or ambiguous rejects the whole document.
    pub fn from_jwks_json(document: &str) -> Result<Self, KeySetError> {
        if document.len() > MAX_JWKS_DOCUMENT_BYTES {
            return Err(KeySetError::TooLarge);
        }
        let set: JwkSet = serde_json::from_str(document)
            .map_err(|error| KeySetError::Malformed(error.to_string()))?;
        if set.keys.len() > MAX_JWKS_KEYS {
            return Err(KeySetError::TooManyKeys);
        }

        let mut keys = HashMap::new();
        for key in set.keys {
            let verifies_rs256 = key.kty == "RSA"
                && key.alg.as_deref().is_none_or(|alg| alg == "RS256")
                && key
                    .key_use
                    .as_deref()
                    .is_none_or(|key_use| key_use == "sig");
            if !verifies_rs256 {
                continue;
            }
            let kid = key
                .kid
                .filter(|kid| !kid.trim().is_empty())
                .ok_or(KeySetError::MissingKid)?;
            let (Some(n), Some(e)) = (key.n.as_deref(), key.e.as_deref()) else {
                return Err(KeySetError::InvalidKey);
            };
            if !usable_modulus(n) || !usable_exponent(e) {
                return Err(KeySetError::InvalidKey);
            }
            let decoding =
                DecodingKey::from_rsa_components(n, e).map_err(|_| KeySetError::InvalidKey)?;
            if keys.insert(kid, decoding).is_some() {
                return Err(KeySetError::DuplicateKid);
            }
        }

        if keys.is_empty() {
            return Err(KeySetError::NoUsableKey);
        }
        Ok(Self { keys })
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// The installed key ids, sorted.
    pub fn key_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.keys.keys().cloned().collect();
        ids.sort();
        ids
    }
}

impl JwtVerifier {
    /// A verifier for a key set given in configuration.
    pub fn from_jwks_json(
        issuer: impl Into<String>,
        audience: impl Into<String>,
        jwks_json: &str,
    ) -> anyhow::Result<Self> {
        if jwks_json.trim().is_empty() {
            anyhow::bail!("JWT JWKS JSON is required");
        }
        let keys = VerificationKeys::from_jwks_json(jwks_json)?;
        let verifier = Self::without_keys(issuer, audience)?;
        verifier.install(keys);
        Ok(verifier)
    }

    /// A verifier that accepts no token until a key set is installed.
    ///
    /// For a Gateway that obtains its keys from the issuer after startup. Until
    /// then every token names an unknown key, and readiness reports the missing
    /// keys, so no traffic is routed to it.
    pub fn without_keys(
        issuer: impl Into<String>,
        audience: impl Into<String>,
    ) -> anyhow::Result<Self> {
        let issuer = issuer.into();
        let audience = audience.into();
        if issuer.trim().is_empty() {
            anyhow::bail!("JWT issuer is required");
        }
        if audience.trim().is_empty() {
            anyhow::bail!("JWT audience is required");
        }
        Ok(Self {
            issuer,
            audience,
            keys: Arc::new(RwLock::new(Arc::new(VerificationKeys::default()))),
        })
    }

    /// Replace the whole key set. A key missing from `keys` stops verifying
    /// for the next request.
    pub fn install(&self, keys: VerificationKeys) {
        *self.keys.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(keys);
    }

    /// Number of installed keys.
    pub fn key_count(&self) -> usize {
        self.current().len()
    }

    /// The installed key ids, sorted.
    pub fn key_ids(&self) -> Vec<String> {
        self.current().key_ids()
    }

    pub fn verify_access_token(&self, token: &str) -> Result<VerifiedAccessToken, TokenRejection> {
        let claims = self.verified_access_claims::<LegacyClaims>(token)?;
        Ok(VerifiedAccessToken {
            subject: claims.sub,
            roles: claims.roles,
            session_id: claims.sid.filter(|sid| !sid.is_empty()),
        })
    }

    // Both projections use this one configured-key verifier. Additional strict
    // claims are decoded from the authenticated payload, never reparsed unsigned.
    fn verified_access_claims<T: serde::de::DeserializeOwned>(
        &self,
        token: &str,
    ) -> Result<AccessClaims<T>, TokenRejection> {
        let header = decode_header(token).map_err(|_| TokenRejection::Malformed)?;
        // Keys come only from the configured issuer. A header that names where
        // to fetch a key (`jku`, `x5u`) or carries one (`jwk`, `x5c`) is an
        // attempt to choose the verifying key, so it is refused, not ignored.
        if header.jku.is_some()
            || header.x5u.is_some()
            || header.jwk.is_some()
            || header.x5c.is_some()
        {
            return Err(TokenRejection::EmbeddedKeyReference);
        }
        let kid = header.kid.ok_or(TokenRejection::MissingKid)?;
        // The set is cloned out of the lock, so an install never waits for a
        // signature check and a check never sees a half-replaced set.
        let keys = self.current();
        let key = keys.keys.get(&kid).ok_or(TokenRejection::UnknownKid)?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.set_audience(&[self.audience.as_str()]);
        validation.set_required_spec_claims(&["aud", "exp", "iss", "sub"]);

        let data = decode::<AccessClaims<T>>(token, key, &validation)
            .map_err(|_| TokenRejection::Invalid)?;
        if data.claims.aud != self.audience {
            return Err(TokenRejection::WrongAudience);
        }
        if data.claims.token_type != "access" {
            return Err(TokenRejection::NotAccessToken);
        }

        Ok(data.claims)
    }

    fn current(&self) -> Arc<VerificationKeys> {
        self.keys
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// Whether a base64url modulus is between the accepted bounds.
fn usable_modulus(n: &str) -> bool {
    let Ok(bytes) = Base64UrlUnpadded::decode_vec(n) else {
        return false;
    };
    let Some(first_byte) = bytes.first() else {
        return false;
    };
    // JWA Base64urlUInt values use the minimum unsigned big-endian octet
    // sequence; accepting a leading zero creates a second encoding of n.
    if *first_byte == 0 || bytes.last().is_none_or(|byte| byte % 2 == 0) {
        return false;
    }
    let bits = bytes.len() * 8 - first_byte.leading_zeros() as usize;
    (MIN_RSA_MODULUS_BITS..=MAX_RSA_MODULUS_BITS).contains(&bits)
}

/// Whether a base64url public exponent is an odd value of at least 3.
///
/// An exponent of 1 makes a signature equal to its own padded message, so
/// anyone could produce one; an even exponent is not a valid RSA key.
fn usable_exponent(e: &str) -> bool {
    let Ok(bytes) = Base64UrlUnpadded::decode_vec(e) else {
        return false;
    };
    let Some(first_byte) = bytes.first() else {
        return false;
    };
    if *first_byte == 0 || bytes.len() > 8 {
        return false;
    }
    let value = bytes
        .iter()
        .fold(0u64, |value, byte| (value << 8) | u64::from(*byte));
    value >= 3 && value % 2 == 1
}

#[derive(Debug, Deserialize)]
struct AccessClaims<T> {
    sub: String,
    aud: String,
    #[serde(default)]
    roles: Vec<String>,
    token_type: String,
    #[serde(default)]
    sid: Option<String>,
    #[serde(flatten)]
    extra: T,
}

// An empty projection preserves legacy handling of additional JWT claims.
#[derive(Debug, Deserialize)]
struct LegacyClaims {}

#[derive(Debug, Deserialize)]
struct JwkSet {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
struct Jwk {
    kid: Option<String>,
    kty: String,
    alg: Option<String>,
    #[serde(rename = "use")]
    key_use: Option<String>,
    n: Option<String>,
    e: Option<String>,
}
