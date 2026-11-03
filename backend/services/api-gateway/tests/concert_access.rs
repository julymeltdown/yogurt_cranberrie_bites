//! Actual configured-key verification; no HTTP, owner RPC or database claims.
//! RSA/JWKS fixture adapted from tests/unit/signing_key_support.rs at b16a679.
use api_gateway::adapters::out::jwt::{
    concert_access::{
        ConcertAccessContext, ConcertAccessRejection, MAX_CONCERT_ACCESS_TOKEN_BYTES,
    },
    JwtVerifier, TokenRejection, VerificationKeys,
};
use base64ct::{Base64UrlUnpadded, Encoding};
use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use rand::rngs::OsRng;
use rsa::{pkcs8::EncodePrivateKey, traits::PublicKeyParts, RsaPrivateKey, RsaPublicKey};
use serde_json::{json, Value};
use std::{sync::OnceLock, time::Duration};
use uuid::Uuid;

const ISSUER: &str = "concert-access-auth-fixture";
const AUDIENCE: &str = "concert-access-gateway-fixture";
const USER: &str = "7e023b36-5ca9-4a40-850d-d01dc16874a8";
const FAMILY: &str = "fa60b0de-7277-447c-aa36-eaf530dd3018";
const FOREIGN: &str = "bdc6ac96-c145-4278-a3ef-72de355a6562";
const PRIVATE: &str = "private-role-email-provider-profile-canary";

struct Key {
    kid: &'static str,
    encoding: EncodingKey,
    public: RsaPublicKey,
}
impl Key {
    fn generate(kid: &'static str) -> Self {
        let private = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
        let pem = private.to_pkcs8_pem(Default::default()).unwrap();
        Self {
            kid,
            encoding: EncodingKey::from_rsa_pem(pem.as_bytes()).unwrap(),
            public: RsaPublicKey::from(&private),
        }
    }
    fn jwk(&self) -> Value {
        json!({"kid": self.kid, "kty": "RSA", "alg": "RS256", "use": "sig",
            "n": Base64UrlUnpadded::encode_string(&self.public.n().to_bytes_be()),
            "e": Base64UrlUnpadded::encode_string(&self.public.e().to_bytes_be())})
    }
    fn header(&self) -> Header {
        Header {
            kid: Some(self.kid.into()),
            ..Header::new(Algorithm::RS256)
        }
    }
    fn token(&self, claims: &Value) -> String {
        self.with_header(claims, self.header())
    }
    fn with_header(&self, claims: &Value, header: Header) -> String {
        jsonwebtoken::encode(&header, claims, &self.encoding).unwrap()
    }
}
fn first() -> &'static Key {
    static KEY: OnceLock<Key> = OnceLock::new();
    KEY.get_or_init(|| Key::generate("strict-first"))
}
fn second() -> &'static Key {
    static KEY: OnceLock<Key> = OnceLock::new();
    KEY.get_or_init(|| Key::generate("strict-second"))
}
fn jwks(keys: &[&Key]) -> String {
    json!({"keys": keys.iter().map(|key| key.jwk()).collect::<Vec<_>>()}).to_string()
}
fn verifier() -> JwtVerifier {
    JwtVerifier::from_jwks_json(ISSUER, AUDIENCE, &jwks(&[first()])).unwrap()
}
fn claims() -> Value {
    // No iat: this context does not invent one as an admission prerequisite.
    json!({"iss": ISSUER, "aud": AUDIENCE, "sub": USER, "sid": FAMILY,
        "exp": Utc::now().timestamp() + 900, "token_type": "access",
        "roles": [PRIVATE, "ROLE_ADMIN"]})
}
fn stop(after: Duration) -> std::time::Instant {
    (tokio::time::Instant::now() + after).into_std()
}
fn verify(
    verifier: &JwtVerifier,
    token: &str,
) -> Result<ConcertAccessContext, ConcertAccessRejection> {
    verifier.verify_concert_access(token, Some(stop(Duration::from_secs(30))))
}
fn assert_dead(context: &ConcertAccessContext) {
    assert_eq!(
        context.check(),
        Err(ConcertAccessRejection::DeadlineExceeded)
    );
    assert_eq!(
        context.user_id(),
        Err(ConcertAccessRejection::DeadlineExceeded)
    );
    assert_eq!(
        context.session_family(),
        Err(ConcertAccessRejection::DeadlineExceeded)
    );
    assert!(matches!(
        context.rpc_deadline(),
        Err(ConcertAccessRejection::DeadlineExceeded)
    ));
    assert_eq!(
        context.remaining(Duration::from_secs(60)),
        Err(ConcertAccessRejection::DeadlineExceeded)
    );
}

#[test]
fn actual_rs256_claims_keep_exact_identity_and_signed_expiry_without_roles_or_foreign_assertions() {
    let verifier = verifier();
    let mut claims = claims();
    for name in [
        "actor",
        "user_id",
        "session_family",
        "event_id",
        "provider_subject",
        "profile",
    ] {
        claims[name] = json!({"id": FOREIGN, "private": PRIVATE});
    }
    let token = first().token(&claims);
    let context = verify(&verifier, &token).unwrap();
    assert_eq!(context.user_id().unwrap(), Uuid::parse_str(USER).unwrap());
    assert_eq!(
        context.session_family().unwrap(),
        Uuid::parse_str(FAMILY).unwrap()
    );
    assert_eq!(
        context.access_expires_micros(),
        claims["exp"].as_i64().unwrap() * 1_000_000
    );
    assert!(context.remaining(Duration::from_millis(5)).unwrap() <= Duration::from_millis(5));
    assert_eq!(
        format!("{context:?}"),
        "ConcertAccessContext { [REDACTED] }"
    );
    for private in [USER, FAMILY, FOREIGN, PRIVATE, "ROLE_ADMIN", &token] {
        assert!(!format!("{context:?}").contains(private));
    }
}

#[test]
fn live_future_exp_has_no_arbitrary_maximum_lifetime_or_iat_requirement() {
    let verifier = verifier();
    let mut claims = claims();
    claims["exp"] = json!(Utc::now().timestamp() + 10 * 365 * 24 * 3600);
    let original = stop(Duration::from_secs(5));
    let context = verifier
        .verify_concert_access(&first().token(&claims), Some(original))
        .unwrap();
    assert!(context.rpc_deadline().unwrap().stop().into_std() <= original);
    assert!(context.remaining(Duration::from_secs(60)).unwrap() <= Duration::from_secs(5));
}

#[test]
fn canonical_nonnil_user_and_mandatory_session_are_stricter_than_legacy_projection() {
    let verifier = verifier();
    for field in ["sub", "sid"] {
        for invalid in [
            "",
            "member",
            "00000000-0000-0000-0000-000000000000",
            "7E023B36-5CA9-4A40-850D-D01DC16874A8",
            "7e023b365ca94a40850dd01dc16874a8",
            "urn:uuid:7e023b36-5ca9-4a40-850d-d01dc16874a8",
            " 7e023b36-5ca9-4a40-850d-d01dc16874a8",
        ] {
            let mut c = claims();
            c[field] = json!(invalid);
            let token = first().token(&c);
            assert!(
                verifier.verify_access_token(&token).is_ok(),
                "legacy string {field}"
            );
            assert!(verify(&verifier, &token).is_err(), "strict {field}");
        }
        for invalid in [Value::Null, json!(123)] {
            let mut c = claims();
            c[field] = invalid;
            assert!(verify(&verifier, &first().token(&c)).is_err());
        }
        let mut c = claims();
        c.as_object_mut().unwrap().remove(field);
        assert!(verify(&verifier, &first().token(&c)).is_err());
    }
    let mut c = claims();
    c.as_object_mut().unwrap().remove("sid");
    assert!(verifier
        .verify_access_token(&first().token(&c))
        .unwrap()
        .session_id
        .is_none());
}

#[test]
fn missing_elapsed_and_zero_original_budgets_fail_before_context_creation() {
    let verifier = verifier();
    let token = first().token(&claims());
    assert!(matches!(
        verifier.verify_concert_access(&token, None),
        Err(ConcertAccessRejection::MissingDeadline)
    ));
    for original in [
        stop(Duration::ZERO),
        (tokio::time::Instant::now() - Duration::from_secs(1)).into_std(),
    ] {
        assert!(matches!(
            verifier.verify_concert_access(&token, Some(original)),
            Err(ConcertAccessRejection::DeadlineExceeded)
        ));
    }
    let context = verify(&verifier, &token).unwrap();
    assert_eq!(
        context.remaining(Duration::ZERO),
        Err(ConcertAccessRejection::DeadlineExceeded)
    );
}

#[tokio::test(start_paused = true)]
async fn original_stop_survives_preverification_and_owner_waits_while_wall_expiry_is_live() {
    let verifier = verifier();
    let token = first().token(&claims());
    let original = stop(Duration::from_secs(10));
    tokio::time::advance(Duration::from_secs(4)).await;
    let context = verifier
        .verify_concert_access(&token, Some(original))
        .unwrap();
    let deadline = context.rpc_deadline().unwrap();
    assert_eq!(deadline.stop().into_std(), original);
    assert!(context.remaining(Duration::from_secs(100)).unwrap() <= Duration::from_secs(6));
    tokio::time::advance(Duration::from_secs(7)).await;
    assert!(Utc::now().timestamp_micros() < context.access_expires_micros());
    assert_dead(&context);
    assert!(deadline
        .remaining(Duration::from_secs(100), context.access_expires_micros())
        .is_err());
    assert!(matches!(
        verifier.verify_concert_access(&token, Some(original)),
        Err(ConcertAccessRejection::DeadlineExceeded)
    ));
}

#[tokio::test(start_paused = true)]
async fn captured_jwt_monotonic_expiry_cannot_be_renewed_while_wall_is_live() {
    let verifier = verifier();
    let token = first().token(&claims());
    let context = verifier
        .verify_concert_access(&token, Some(stop(Duration::from_secs(3600))))
        .unwrap();
    let captured = context.rpc_deadline().unwrap();
    tokio::time::advance(Duration::from_secs(901)).await;
    assert!(Utc::now().timestamp_micros() < context.access_expires_micros());
    assert_dead(&context);
    assert!(captured
        .remaining(Duration::from_secs(3600), context.access_expires_micros())
        .is_err());
}

#[tokio::test(start_paused = true)]
async fn actual_signed_wall_expiry_fences_an_existing_context_before_original_request_stop() {
    let verifier = verifier();
    let mut c = claims();
    c["exp"] = json!(Utc::now().timestamp() + 3);
    let token = first().token(&c);
    let original = stop(Duration::from_secs(10));
    let context = verifier
        .verify_concert_access(&token, Some(original))
        .unwrap();
    let captured_stop = context.rpc_deadline().unwrap().stop();
    let remaining = context.access_expires_micros() - Utc::now().timestamp_micros();
    assert!(remaining > 0);
    // Let actual signed wall time expire while the test-owned Tokio monotonic
    // clock stays paused/live. A monotonic-only check would incorrectly pass.
    std::thread::sleep(Duration::from_micros(remaining as u64) + Duration::from_millis(20));
    assert!(Utc::now().timestamp_micros() >= context.access_expires_micros());
    assert!(tokio::time::Instant::now().into_std() < original);
    assert!(tokio::time::Instant::now() < captured_stop);
    assert_dead(&context);
    // The old verifier remains deliberately skew-tolerant for legacy routes.
    assert!(verifier.verify_access_token(&token).is_ok());
}

#[test]
fn already_expired_within_legacy_skew_is_never_live_strict_authority() {
    let verifier = verifier();
    let mut c = claims();
    c["exp"] = json!(Utc::now().timestamp() - 1);
    let token = first().token(&c);
    assert!(verifier.verify_access_token(&token).is_ok());
    assert!(matches!(
        verify(&verifier, &token),
        Err(ConcertAccessRejection::DeadlineExceeded)
    ));
}

#[test]
fn signed_numeric_dates_reject_missing_fractional_string_null_negative_and_overflow_exp() {
    let verifier = verifier();
    for invalid in [
        Value::Null,
        json!("1900000000"),
        json!(-1),
        json!(true),
        json!(Utc::now().timestamp() as f64 + 900.25),
        json!(u64::MAX),
        json!(i64::MAX),
        json!(i64::MAX / 1_000_000 + 1),
        json!({"exp": 9}),
    ] {
        let mut c = claims();
        c["exp"] = invalid;
        assert!(
            verify(&verifier, &first().token(&c)).is_err(),
            "invalid numeric date"
        );
    }
    let mut c = claims();
    c.as_object_mut().unwrap().remove("exp");
    assert!(verify(&verifier, &first().token(&c)).is_err());
    let mut c = claims();
    c["exp"] = json!(Utc::now().timestamp() as f64 + 900.25);
    assert!(
        verifier.verify_access_token(&first().token(&c)).is_ok(),
        "retained library rounding"
    );
}

#[test]
fn future_nbf_never_inherits_legacy_disabled_validation_or_skew() {
    let verifier = verifier();
    for future in [Utc::now().timestamp() + 30, Utc::now().timestamp() + 3600] {
        let mut c = claims();
        c["nbf"] = json!(future);
        let token = first().token(&c);
        assert!(verifier.verify_access_token(&token).is_ok());
        assert!(matches!(
            verify(&verifier, &token),
            Err(ConcertAccessRejection::InvalidClaims)
        ));
    }
    let mut c = claims();
    c["nbf"] = json!(Utc::now().timestamp() - 1);
    assert!(verify(&verifier, &first().token(&c)).is_ok());
    for invalid in [Value::Null, json!("tomorrow"), json!(1.5), json!(i64::MAX)] {
        c["nbf"] = invalid;
        assert!(verify(&verifier, &first().token(&c)).is_err());
    }
}

#[test]
fn issuer_audience_type_algorithm_and_signature_are_always_verified() {
    let verifier = verifier();
    for (field, wrong) in [
        ("iss", json!("foreign-issuer")),
        ("iss", json!([ISSUER, "other"])),
        ("aud", json!("foreign-audience")),
        ("aud", json!([AUDIENCE])),
        ("token_type", json!("refresh")),
    ] {
        let mut c = claims();
        c[field] = wrong;
        assert!(verify(&verifier, &first().token(&c)).is_err());
    }
    let c = claims();
    // A valid signature from a foreign private key naming an installed key.
    assert!(verify(&verifier, &second().with_header(&c, first().header())).is_err());
    let mut header = Header::new(Algorithm::HS256);
    header.kid = Some(first().kid.into());
    let hmac =
        jsonwebtoken::encode(&header, &c, &EncodingKey::from_secret(b"attacker-secret")).unwrap();
    assert!(verify(&verifier, &hmac).is_err());
    let mut altered = first().token(&c).into_bytes();
    let index = altered.len() - 20;
    altered[index] = if altered[index] == b'A' { b'B' } else { b'A' };
    assert!(verify(&verifier, &String::from_utf8(altered).unwrap()).is_err());
}

#[test]
fn header_keys_locations_missing_kid_malformed_and_huge_tokens_fail_safely() {
    let verifier = verifier();
    let c = claims();
    let base = first().header();
    for header in [
        Header {
            jku: Some("https://attacker.invalid/jwks".into()),
            ..base.clone()
        },
        Header {
            x5u: Some("https://attacker.invalid/cert".into()),
            ..base.clone()
        },
        Header {
            jwk: Some(serde_json::from_value(first().jwk()).unwrap()),
            ..base.clone()
        },
        Header {
            x5c: Some(vec!["untrusted-certificate".into()]),
            ..base.clone()
        },
    ] {
        assert!(matches!(
            verify(&verifier, &first().with_header(&c, header)),
            Err(ConcertAccessRejection::Token(
                TokenRejection::EmbeddedKeyReference
            ))
        ));
    }
    assert!(matches!(
        verify(
            &verifier,
            &first().with_header(&c, Header { kid: None, ..base })
        ),
        Err(ConcertAccessRejection::Token(TokenRejection::MissingKid))
    ));
    for invalid in ["", "malformed", "e30.e30.invalid", "Bearer abc.def.ghi"] {
        let error = verify(&verifier, invalid).unwrap_err();
        assert!(!format!("{error:?} {error}").contains(PRIVATE));
    }
    let mut c = c;
    c["private"] = json!(PRIVATE.repeat(MAX_CONCERT_ACCESS_TOKEN_BYTES / PRIVATE.len()));
    let token = first().token(&c);
    assert!(token.len() > MAX_CONCERT_ACCESS_TOKEN_BYTES);
    assert!(
        verifier.verify_access_token(&token).is_ok(),
        "strict resource bound does not alter legacy"
    );
    assert!(matches!(
        verify(&verifier, &token),
        Err(ConcertAccessRejection::Token(TokenRejection::Malformed))
    ));
}

#[test]
fn clones_share_rotation_and_withdrawal_and_only_unknown_key_remains_refreshable() {
    let verifier = verifier();
    let clone = verifier.clone();
    let first_token = first().token(&claims());
    let second_token = second().token(&claims());
    let held = verify(&verifier, &first_token).unwrap();
    let error = verify(&clone, &second_token).unwrap_err();
    assert_eq!(
        error,
        ConcertAccessRejection::Token(TokenRejection::UnknownKid)
    );
    assert_eq!(TokenRejection::UnknownKid.reason(), "unknown_signing_key");
    clone.install(VerificationKeys::from_jwks_json(&jwks(&[first(), second()])).unwrap());
    assert!(verify(&verifier, &first_token).is_ok());
    assert!(verify(&verifier, &second_token).is_ok());
    clone.install(VerificationKeys::from_jwks_json(&jwks(&[second()])).unwrap());
    assert_eq!(
        verify(&verifier, &first_token).unwrap_err(),
        ConcertAccessRejection::Token(TokenRejection::UnknownKid)
    );
    assert!(verify(&verifier, &second_token).is_ok());
    // Honest boundary: this type captures verification; it does not implement
    // key withdrawal across awaits or silently pretend it revokes held contexts.
    assert!(held.check().is_ok());
    let empty = JwtVerifier::without_keys(ISSUER, AUDIENCE).unwrap();
    assert_eq!(
        verify(&empty, &second_token).unwrap_err(),
        ConcertAccessRejection::Token(TokenRejection::UnknownKid)
    );
}
