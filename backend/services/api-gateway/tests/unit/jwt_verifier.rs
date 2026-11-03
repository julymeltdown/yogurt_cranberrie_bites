use crate::signing_key_support::{first_key, key_set, second_key, weak_key, AUDIENCE, ISSUER};
use api_gateway::adapters::out::jwt::{
    JwtVerifier, KeySetError, TokenRejection, VerificationKeys, MAX_JWKS_DOCUMENT_BYTES,
    MAX_JWKS_KEYS,
};
use base64ct::{Base64UrlUnpadded, Encoding};
use chrono::Utc;
use jsonwebtoken::jwk::{AlgorithmParameters, CommonParameters, Jwk, RSAKeyParameters, RSAKeyType};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use rand::rngs::OsRng;
use rsa::pkcs8::EncodePrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde::Serialize;
use serde_json::{json, Value};
use uuid::Uuid;

const KID: &str = "gateway-unit-test-key";

#[test]
fn jwt_verifier_accepts_token_with_expected_audience() {
    let key = TestJwtKey::new();
    let verifier =
        JwtVerifier::from_jwks_json(ISSUER, AUDIENCE, key.jwks_json.as_str()).expect("verifier");
    let user_id = Uuid::new_v4();

    let verified = verifier
        .verify_access_token(&key.access_token(user_id, AUDIENCE))
        .expect("verified token");

    assert_eq!(verified.subject, user_id.to_string());
    assert_eq!(verified.roles, vec!["ROLE_USER"]);
}

#[test]
fn jwt_verifier_rejects_token_with_wrong_audience() {
    let key = TestJwtKey::new();
    let verifier =
        JwtVerifier::from_jwks_json(ISSUER, AUDIENCE, key.jwks_json.as_str()).expect("verifier");
    let user_id = Uuid::new_v4();

    let error = verifier
        .verify_access_token(&key.access_token(user_id, "different-api"))
        .expect_err("wrong audience should fail");

    // The rejection is asserted, not the library text: the neutral rejection
    // type deliberately carries no decoder detail out of the adapter.
    assert_eq!(error, TokenRejection::Invalid);
    assert_eq!(error.reason(), "invalid_bearer_token");
}

#[test]
fn installing_a_key_set_replaces_it_whole_for_every_clone() {
    // Given: a verifier holding the first key, and a clone of it.
    let verifier =
        JwtVerifier::from_jwks_json(ISSUER, AUDIENCE, &key_set(&[first_key()])).expect("verifier");
    let clone = verifier.clone();
    let withdrawn = first_key().access_token("member");
    let published = second_key().access_token("member");
    assert!(matches!(
        verifier.verify_access_token(&published),
        Err(TokenRejection::UnknownKid)
    ));

    // When: a set holding only the second key is installed through the clone.
    clone.install(VerificationKeys::from_jwks_json(&key_set(&[second_key()])).expect("key set"));

    // Then: the original verifies the new key, and the key missing from the
    // new set stops verifying at once.
    assert_eq!(
        verifier
            .verify_access_token(&published)
            .expect("new key")
            .subject,
        "member"
    );
    assert!(matches!(
        verifier.verify_access_token(&withdrawn),
        Err(TokenRejection::UnknownKid)
    ));
    assert_eq!(verifier.key_ids(), vec![second_key().kid.to_owned()]);
}

#[test]
fn a_token_that_carries_its_own_key_or_key_location_is_refused() {
    // Given: a verifier that holds the key the tokens below name and are
    // signed with.
    let key = first_key();
    let verifier =
        JwtVerifier::from_jwks_json(ISSUER, AUDIENCE, &key_set(&[key])).expect("verifier");
    let embedded = Jwk {
        common: CommonParameters::default(),
        algorithm: AlgorithmParameters::RSA(RSAKeyParameters {
            key_type: RSAKeyType::RSA,
            n: key.modulus(),
            e: key.exponent(),
        }),
    };

    for header in [
        Header {
            jku: Some("https://attacker.invalid/jwks.json".into()),
            ..key.header()
        },
        Header {
            x5u: Some("https://attacker.invalid/cert.pem".into()),
            ..key.header()
        },
        Header {
            jwk: Some(embedded),
            ..key.header()
        },
        Header {
            x5c: Some(vec!["MIIBAA==".into()]),
            ..key.header()
        },
    ] {
        // When/Then: the kid is installed and the signature is valid, so the
        // key reference in the header alone causes the refusal.
        let token = key.access_token_with_header("member", header);
        assert!(matches!(
            verifier.verify_access_token(&token),
            Err(TokenRejection::EmbeddedKeyReference)
        ));
    }
    assert!(verifier
        .verify_access_token(&key.access_token("member"))
        .is_ok());
}

#[test]
fn a_key_set_is_refused_whole_when_it_is_oversized_malformed_or_unsafe() {
    let usable = first_key().public_jwk();
    let set = |keys: Vec<Value>| json!({ "keys": keys }).to_string();
    let changed = |field: &str, value: Value| {
        let mut key = usable.clone();
        key[field] = value;
        key
    };
    let refused = |document: String| {
        VerificationKeys::from_jwks_json(&document)
            .err()
            .expect("the key set must be refused")
    };

    // Bounded in size and in key count.
    // Trailing whitespace keeps the document valid JSON, so size alone refuses it.
    let mut padded = set(vec![usable.clone()]);
    padded.push_str(&" ".repeat(MAX_JWKS_DOCUMENT_BYTES + 1 - padded.len()));
    assert_eq!(refused(padded), KeySetError::TooLarge);
    let too_many = (0..=MAX_JWKS_KEYS)
        .map(|index| changed("kid", json!(format!("key-{index}"))))
        .collect();
    assert_eq!(refused(set(too_many)), KeySetError::TooManyKeys);
    assert!(matches!(
        refused("<html>maintenance</html>".into()),
        KeySetError::Malformed(_)
    ));
    // Every usable key names exactly one key id.
    let mut without_kid = usable.clone();
    without_kid
        .as_object_mut()
        .expect("key object")
        .remove("kid");
    assert_eq!(refused(set(vec![without_kid])), KeySetError::MissingKid);
    assert_eq!(
        refused(set(vec![usable.clone(), usable.clone()])),
        KeySetError::DuplicateKid
    );
    // Weak or broken keys: a short modulus, an exponent of 1 that would let
    // anyone produce a valid signature, an even exponent, a modulus that is
    // not base64url.
    assert_eq!(
        refused(set(vec![weak_key().public_jwk()])),
        KeySetError::InvalidKey
    );
    assert_eq!(
        refused(set(vec![changed("e", json!("AQ"))])),
        KeySetError::InvalidKey
    );
    assert_eq!(
        refused(set(vec![changed("e", json!("AAI"))])),
        KeySetError::InvalidKey
    );
    assert_eq!(
        refused(set(vec![changed("n", json!("not base64url!"))])),
        KeySetError::InvalidKey
    );
    let mut noncanonical_modulus = vec![0];
    noncanonical_modulus.extend(
        Base64UrlUnpadded::decode_vec(usable["n"].as_str().expect("RSA modulus"))
            .expect("valid modulus"),
    );
    assert_eq!(
        refused(set(vec![changed(
            "n",
            json!(Base64UrlUnpadded::encode_string(&noncanonical_modulus)),
        )])),
        KeySetError::InvalidKey
    );
    let mut even_modulus =
        Base64UrlUnpadded::decode_vec(usable["n"].as_str().expect("RSA modulus"))
            .expect("valid modulus");
    *even_modulus.last_mut().expect("non-empty modulus") &= !1;
    assert_eq!(
        refused(set(vec![changed(
            "n",
            json!(Base64UrlUnpadded::encode_string(&even_modulus)),
        )])),
        KeySetError::InvalidKey
    );
    let mut noncanonical_exponent = vec![0];
    noncanonical_exponent.extend(
        Base64UrlUnpadded::decode_vec(usable["e"].as_str().expect("RSA exponent"))
            .expect("valid exponent"),
    );
    assert_eq!(
        refused(set(vec![changed(
            "e",
            json!(Base64UrlUnpadded::encode_string(&noncanonical_exponent)),
        )])),
        KeySetError::InvalidKey
    );
    // A set with nothing this verifier can use would reject every token.
    let ec = json!({"kty": "EC", "crv": "P-256", "kid": "ec", "x": "AQ", "y": "AQ"});
    assert_eq!(refused(set(vec![])), KeySetError::NoUsableKey);
    assert_eq!(refused(set(vec![ec.clone()])), KeySetError::NoUsableKey);

    // Control: keys for another algorithm or use are skipped, not refused, so
    // the issuer can publish them without breaking this verifier.
    let keys = VerificationKeys::from_jwks_json(&set(vec![
        ec,
        changed("use", json!("enc")),
        usable.clone(),
    ]))
    .expect("usable key set");
    assert_eq!(keys.key_ids(), vec![first_key().kid.to_owned()]);
}

#[test]
fn a_verifier_without_keys_rejects_every_token_as_naming_an_unknown_key() {
    let verifier = JwtVerifier::without_keys(ISSUER, AUDIENCE).expect("verifier");
    assert_eq!(verifier.key_count(), 0);
    assert!(matches!(
        verifier.verify_access_token(&first_key().access_token("member")),
        Err(TokenRejection::UnknownKid)
    ));
}

#[test]
fn rejection_reasons_are_bounded_labels() {
    // The kid is chosen by whoever sent the token, so it never becomes a label.
    assert_eq!(TokenRejection::UnknownKid.reason(), "unknown_signing_key");
    assert_eq!(
        TokenRejection::EmbeddedKeyReference.reason(),
        "embedded_key_reference"
    );
    assert_eq!(TokenRejection::MissingKid.reason(), "invalid_bearer_token");
    assert_eq!(
        TokenRejection::WrongAudience.reason(),
        "invalid_bearer_token"
    );
}

struct TestJwtKey {
    encoding: EncodingKey,
    jwks_json: String,
}

impl TestJwtKey {
    fn new() -> Self {
        let private_key = RsaPrivateKey::new(&mut OsRng, 2048).expect("test RSA key");
        let public_key = RsaPublicKey::from(&private_key);
        let private_pem = private_key
            .to_pkcs8_pem(Default::default())
            .expect("private pem");
        let jwks_json = serde_json::json!({
            "keys": [{
                "kty": "RSA",
                "use": "sig",
                "alg": "RS256",
                "kid": KID,
                "n": Base64UrlUnpadded::encode_string(&public_key.n().to_bytes_be()),
                "e": Base64UrlUnpadded::encode_string(&public_key.e().to_bytes_be())
            }]
        })
        .to_string();

        Self {
            encoding: EncodingKey::from_rsa_pem(private_pem.as_bytes()).expect("encoding key"),
            jwks_json,
        }
    }

    fn access_token(&self, user_id: Uuid, audience: &str) -> String {
        let now = Utc::now().timestamp();
        let claims = TestAccessClaims {
            sub: user_id.to_string(),
            iss: ISSUER,
            aud: audience,
            iat: now,
            exp: now + 900,
            jti: Uuid::new_v4().to_string(),
            token_type: "access",
            roles: vec!["ROLE_USER".into()],
        };
        let header = Header {
            alg: Algorithm::RS256,
            kid: Some(KID.to_owned()),
            ..Header::default()
        };

        jsonwebtoken::encode(&header, &claims, &self.encoding).expect("signed token")
    }
}

#[derive(Serialize)]
struct TestAccessClaims<'a> {
    sub: String,
    iss: &'a str,
    aud: &'a str,
    iat: i64,
    exp: i64,
    jti: String,
    token_type: &'a str,
    roles: Vec<String>,
}
