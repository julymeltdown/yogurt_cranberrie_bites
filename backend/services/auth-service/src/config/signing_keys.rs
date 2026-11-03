//! Public keys the issuer publishes and accepts beside its signing key.
//!
//! A signing key cannot be replaced in one step. Every verifier has to know the
//! next key before the first token signed with it arrives, and tokens signed
//! with the previous key stay in circulation until they expire: access tokens
//! for minutes, refresh tokens for up to the refresh lifetime. The issuer used
//! to publish and accept exactly one key, so replacing it rejected every
//! outstanding refresh token and signed every user out.
//!
//! The rotation set is configuration (`AUTH_JWT_ROTATION_JWKS_JSON`). It holds
//! public keys only and stays small: the next key before a switch and the
//! previous key after it, not a key history. The procedure that uses it is in
//! `docs/operations/signing-key-rotation.md`.

use base64ct::{Base64UrlUnpadded, Encoding};
use jsonwebtoken::DecodingKey;
use rsa::traits::PublicKeyParts;
use rsa::{BigUint, RsaPublicKey};
use serde::Deserialize;
use serde_json::Value;

/// Most public keys the rotation set may hold.
///
/// An ordinary rotation needs at most two: the next key before a switch and the
/// previous key after it. The bound leaves room for an emergency replacement
/// that overlaps an ordinary rotation, without letting the published document
/// grow without limit.
pub const MAX_ROTATION_KEYS: usize = 4;

/// Smallest accepted RSA modulus, in bits. The largest is the RSA crate's
/// default of 4096 bits, which the Gateway enforces as well.
pub const MIN_RSA_MODULUS_BITS: usize = 2048;

/// Named in every error so a failed start says which setting to fix.
const SETTING: &str = "AUTH_JWT_ROTATION_JWKS_JSON";

/// A public key that is published and accepted but never signs.
#[derive(Clone)]
pub(crate) struct RotationKey {
    pub(crate) kid: String,
    pub(crate) public_key: RsaPublicKey,
    pub(crate) decoding: DecodingKey,
}

#[derive(Deserialize)]
struct PublicJwkSet {
    keys: Vec<PublicJwk>,
}

#[derive(Deserialize)]
struct PublicJwk {
    kty: String,
    kid: Option<String>,
    alg: Option<String>,
    #[serde(rename = "use")]
    key_use: Option<String>,
    n: Option<String>,
    e: Option<String>,
    /// The private exponent. Every RSA private JWK carries `d`, so its absence
    /// is what shows an entry is public. A set that carries it is a private key
    /// pasted into a setting whose contents are published.
    d: Option<Value>,
}

/// Parse the rotation set, refusing anything that is not a distinct public
/// RS256 signing key.
///
/// Every problem fails startup rather than skipping the key. A skipped key
/// would make a rotation fail silently: the issuer would start, publish less
/// than the operator configured, and the first token signed with the missing
/// key would be rejected everywhere.
pub(crate) fn parse_rotation_keys(
    jwks_json: &str,
    signing_kid: &str,
) -> anyhow::Result<Vec<RotationKey>> {
    let set: PublicJwkSet = serde_json::from_str(jwks_json)
        .map_err(|error| anyhow::anyhow!("{SETTING} is not a JWK Set: {error}"))?;
    if set.keys.len() > MAX_ROTATION_KEYS {
        anyhow::bail!(
            "{SETTING} lists {} keys; at most {MAX_ROTATION_KEYS} are allowed",
            set.keys.len()
        );
    }

    let mut keys: Vec<RotationKey> = Vec::with_capacity(set.keys.len());
    for jwk in set.keys {
        let kid = jwk
            .kid
            .filter(|kid| !kid.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("{SETTING} contains a key without a kid"))?;
        if jwk.d.is_some() {
            anyhow::bail!(
                "{SETTING} key {kid} contains private key material; it must hold public keys only"
            );
        }
        let signs_rs256 = jwk.kty == "RSA"
            && jwk.alg.as_deref().is_none_or(|alg| alg == "RS256")
            && jwk
                .key_use
                .as_deref()
                .is_none_or(|key_use| key_use == "sig");
        if !signs_rs256 {
            anyhow::bail!("{SETTING} key {kid} must be an RSA RS256 signing key");
        }
        // A token names its key by kid, so two keys with one kid would make the
        // verifying key depend on lookup order.
        if kid == signing_kid || keys.iter().any(|key| key.kid == kid) {
            anyhow::bail!("{SETTING} key id {kid} is not unique");
        }

        let public_key = rsa_public_key(&kid, jwk.n.as_deref(), jwk.e.as_deref())?;
        let decoding =
            DecodingKey::from_rsa_components(&encode(public_key.n()), &encode(public_key.e()))
                .map_err(|error| {
                    anyhow::anyhow!("{SETTING} key {kid} cannot verify tokens: {error}")
                })?;
        keys.push(RotationKey {
            kid,
            public_key,
            decoding,
        });
    }
    Ok(keys)
}

/// The published form of a public key.
pub(crate) fn public_jwk(
    kid: &str,
    public_key: &RsaPublicKey,
) -> crate::application::dto::token_dto::AuthPublicJwkDto {
    crate::application::dto::token_dto::AuthPublicJwkDto {
        kty: "RSA".into(),
        key_use: "sig".into(),
        alg: "RS256".into(),
        kid: kid.into(),
        n: encode(public_key.n()),
        e: encode(public_key.e()),
    }
}

fn rsa_public_key(kid: &str, n: Option<&str>, e: Option<&str>) -> anyhow::Result<RsaPublicKey> {
    let component = |value: Option<&str>, name: &str| -> anyhow::Result<BigUint> {
        let value =
            value.ok_or_else(|| anyhow::anyhow!("{SETTING} key {kid} is missing \"{name}\""))?;
        let bytes = Base64UrlUnpadded::decode_vec(value).map_err(|_| {
            anyhow::anyhow!("{SETTING} key {kid} has an invalid base64url \"{name}\"")
        })?;
        Ok(BigUint::from_bytes_be(&bytes))
    };
    let public_key =
        RsaPublicKey::new(component(n, "n")?, component(e, "e")?).map_err(|error| {
            anyhow::anyhow!("{SETTING} key {kid} is not a usable RSA public key: {error}")
        })?;
    if public_key.n().bits() < MIN_RSA_MODULUS_BITS {
        anyhow::bail!("{SETTING} key {kid} is shorter than {MIN_RSA_MODULUS_BITS} bits");
    }
    Ok(public_key)
}

fn encode(value: &BigUint) -> String {
    Base64UrlUnpadded::encode_string(&value.to_bytes_be())
}
