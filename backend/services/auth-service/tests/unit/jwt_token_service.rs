use crate::config_behavior::oauth_config;
use crate::support::{rotation_test_rsa_private_key_pem, shared_test_rsa_private_key_pem};
use auth_service::adapters::r#in::http;
use auth_service::application::ports::TokenServicePort;
use auth_service::config::{
    build_state, JwtTokenService, RuntimeMode, MAX_ROTATION_KEYS, MIN_RSA_MODULUS_BITS,
};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64ct::{Base64UrlUnpadded, Encoding};
use chrono::Utc;
use http_body_util::BodyExt;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use rand::rngs::StdRng;
use rand::SeedableRng;
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

const ISSUER: &str = "auth-service-test";
const AUDIENCE: &str = "port-rust-api-test";

#[test]
fn configured_jwt_private_key_is_stable_across_service_instances() {
    let private_pem = shared_test_rsa_private_key_pem();
    let issuer = "auth-service-test";
    let audience = "port-rust-api-test";
    let kid = "configured-test-key";
    let first =
        JwtTokenService::from_private_key_pem(issuer, audience, 900, 1209600, private_pem, kid)
            .expect("first service");
    let second =
        JwtTokenService::from_private_key_pem(issuer, audience, 900, 1209600, private_pem, kid)
            .expect("second service");

    let user_id = Uuid::new_v4();
    let token_pair = first
        .issue_token_pair(user_id, vec!["ROLE_USER".into()])
        .expect("issue");
    let decoded = second
        .decode_access_token(&token_pair.access_token)
        .expect("decode with second instance");

    assert_eq!(decoded.subject, user_id);
    assert_eq!(
        auth_service::adapters::r#in::jwks::public_jwks(&(second.jwk_set()))["keys"][0]["kid"],
        kid
    );
}

#[test]
fn access_token_decode_rejects_wrong_audience() {
    let private_pem = shared_test_rsa_private_key_pem();
    let issuer = "auth-service-test";
    let kid = "configured-test-key";
    let issuer_service = JwtTokenService::from_private_key_pem(
        issuer,
        "port-rust-api-test",
        900,
        1209600,
        private_pem,
        kid,
    )
    .expect("issuer service");
    let wrong_audience_service = JwtTokenService::from_private_key_pem(
        issuer,
        "different-api",
        900,
        1209600,
        private_pem,
        kid,
    )
    .expect("wrong audience service");

    let token_pair = issuer_service
        .issue_token_pair(Uuid::new_v4(), vec!["ROLE_USER".into()])
        .expect("issue");

    assert!(wrong_audience_service
        .decode_access_token(&token_pair.access_token)
        .is_err());
    assert!(wrong_audience_service
        .decode_refresh_token(&token_pair.refresh_token)
        .is_ok());
}

#[test]
fn a_rotation_publishes_the_signing_key_first_and_only_public_keys() {
    // Given: an issuer that pre-publishes its next key.
    let next = issuer(rotation_test_rsa_private_key_pem(), "next");
    let current = issuer(shared_test_rsa_private_key_pem(), "current")
        .with_rotation_keys(
            &auth_service::adapters::r#in::jwks::public_jwks(&(next.jwk_set())).to_string(),
        )
        .expect("rotation keys");

    // When: the published key set is read.
    let keys = published(&current);

    // Then: the signing key stays first, the next key follows exactly as its own
    // issuer will publish it, and nothing private is published.
    assert_eq!(kids(&keys), ["current", "next"]);
    assert_eq!(keys[1], published(&next)[0]);
    for key in &keys {
        assert_eq!(key["kty"], "RSA");
        assert_eq!(key["alg"], "RS256");
        assert_eq!(key["use"], "sig");
        assert!(key.get("d").is_none(), "a published key must be public");
    }
}

#[test]
fn after_a_switch_the_previous_key_still_verifies_until_it_is_retired() {
    // Given: a session issued before the switch, signed by the previous key.
    let user_id = Uuid::new_v4();
    let before = issuer(shared_test_rsa_private_key_pem(), "previous");
    let session = before
        .issue_token_pair(user_id, vec!["ROLE_USER".into()])
        .expect("issue before the switch");

    // When: the issuer signs with the next key and keeps the previous key.
    let switched = issuer(rotation_test_rsa_private_key_pem(), "next")
        .with_rotation_keys(
            &auth_service::adapters::r#in::jwks::public_jwks(&(before.jwk_set())).to_string(),
        )
        .expect("rotation keys");

    // Then: the outstanding tokens still verify, so the switch signs nobody out.
    // Before rotation keys existed, this refresh token was rejected here.
    let access = switched
        .decode_access_token(&session.access_token)
        .expect("previous-key access token");
    let refresh = switched
        .decode_refresh_token(&session.refresh_token)
        .expect("previous-key refresh token");
    assert_eq!((access.subject, refresh.subject), (user_id, user_id));
    // And: new tokens are signed only with the next key.
    let fresh = switched
        .issue_token_pair(user_id, vec!["ROLE_USER".into()])
        .expect("issue after the switch");
    assert_eq!(kid_of(&fresh.access_token), "next");
    assert_eq!(kid_of(&fresh.refresh_token), "next");

    // When: the previous key is retired.
    let retired = issuer(rotation_test_rsa_private_key_pem(), "next");

    // Then: its tokens are refused while the next key's tokens are unaffected.
    assert!(retired.decode_access_token(&session.access_token).is_err());
    assert!(retired
        .decode_refresh_token(&session.refresh_token)
        .is_err());
    assert!(retired.decode_refresh_token(&fresh.refresh_token).is_ok());
}

#[test]
fn a_token_naming_a_key_the_issuer_does_not_accept_is_refused() {
    // Given: claims signed by the issuer's own private key.
    let current = issuer(shared_test_rsa_private_key_pem(), "current");
    let signed_as = |kid: &str| {
        let now = Utc::now().timestamp();
        let claims = json!({
            "sub": Uuid::new_v4().to_string(),
            "iss": ISSUER,
            "aud": AUDIENCE,
            "iat": now,
            "exp": now + 900,
            "jti": Uuid::new_v4().to_string(),
            "token_type": "access",
            "roles": ["ROLE_USER"]
        });
        let header = Header {
            alg: Algorithm::RS256,
            kid: Some(kid.to_owned()),
            ..Header::default()
        };
        let key = EncodingKey::from_rsa_pem(shared_test_rsa_private_key_pem().as_bytes())
            .expect("encoding key");
        jsonwebtoken::encode(&header, &claims, &key).expect("signed token")
    };

    // When/Then: naming a key the issuer does not hold is refused, even though
    // the signature itself would verify. The same claims under the configured
    // kid verify, so the refusal is caused by the kid alone.
    assert!(current
        .decode_access_token(&signed_as("not-configured"))
        .is_err());
    assert!(current.decode_access_token(&signed_as("current")).is_ok());
}

#[test]
fn a_rotation_set_that_cannot_rotate_safely_is_refused() {
    let next_key = published(&issuer(rotation_test_rsa_private_key_pem(), "next"))[0].clone();
    let refusal = |keys: Vec<Value>| -> String {
        match issuer(shared_test_rsa_private_key_pem(), "current")
            .with_rotation_keys(&json!({ "keys": keys }).to_string())
        {
            Ok(_) => panic!("the rotation set must be refused"),
            Err(error) => error.to_string(),
        }
    };
    let changed = |field: &str, value: Value| {
        let mut key = next_key.clone();
        key[field] = value;
        key
    };

    // A private key pasted into a published setting.
    let private = refusal(vec![changed("d", json!("AQAB"))]);
    assert!(private.contains("private key material"), "{private}");
    assert!(private.contains("AUTH_JWT_ROTATION_JWKS_JSON"), "{private}");
    // Key ids must select exactly one key.
    assert!(refusal(vec![changed("kid", json!("current"))]).contains("not unique"));
    assert!(refusal(vec![next_key.clone(), next_key.clone()]).contains("not unique"));
    // Only RS256 signing keys verify the tokens this issuer signs.
    assert!(refusal(vec![changed("alg", json!("RS512"))]).contains("RS256"));
    assert!(refusal(vec![changed("use", json!("enc"))]).contains("RS256"));
    let mut without_kid = next_key.clone();
    without_kid
        .as_object_mut()
        .expect("key object")
        .remove("kid");
    assert!(refusal(vec![without_kid]).contains("without a kid"));
    // The set is bounded.
    let too_many = (0..=MAX_ROTATION_KEYS)
        .map(|index| changed("kid", json!(format!("next-{index}"))))
        .collect();
    assert!(refusal(too_many).contains(&format!("at most {MAX_ROTATION_KEYS}")));
    // A weak key would lower the strength of every token it verifies.
    let weak = RsaPrivateKey::new(&mut StdRng::seed_from_u64(7), 1024).expect("weak key");
    let weak = RsaPublicKey::from(&weak);
    let weak_key = json!({
        "kty": "RSA",
        "use": "sig",
        "alg": "RS256",
        "kid": "weak",
        "n": Base64UrlUnpadded::encode_string(&weak.n().to_bytes_be()),
        "e": Base64UrlUnpadded::encode_string(&weak.e().to_bytes_be())
    });
    assert!(refusal(vec![weak_key]).contains(&format!("shorter than {MIN_RSA_MODULUS_BITS} bits")));
    let malformed = issuer(shared_test_rsa_private_key_pem(), "current")
        .with_rotation_keys("not json")
        .err()
        .expect("malformed set must be refused");
    assert!(malformed.to_string().contains("not a JWK Set"));

    // Control: the unchanged key is accepted, so each refusal is its one defect.
    issuer(shared_test_rsa_private_key_pem(), "current")
        .with_rotation_keys(&json!({ "keys": [next_key] }).to_string())
        .expect("valid rotation set");
}

#[tokio::test]
async fn the_jwks_endpoint_serves_the_configured_rotation_keys() {
    // Given: the auth service configured with a pre-published next key.
    let next = issuer(rotation_test_rsa_private_key_pem(), "rotation-next");
    let mut config = oauth_config(RuntimeMode::Parity);
    config.jwt_private_key_pem = Some(shared_test_rsa_private_key_pem().to_owned());
    config.jwt_key_id = Some("rotation-current".into());
    config.jwt_rotation_jwks_json =
        Some(auth_service::adapters::r#in::jwks::public_jwks(&(next.jwk_set())).to_string());
    let pool = PgPoolOptions::new()
        .connect_lazy(&config.database_url)
        .expect("lazy pool");
    let state = Arc::new(build_state(&config, pool).await.expect("state"));

    // When: a verifier reads the published key set.
    let response = http::router(state)
        .oneshot(
            Request::builder()
                .uri("/.well-known/jwks.json")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    // Then: both keys are published, signing key first.
    assert_eq!(response.status(), StatusCode::OK);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let document: Value = serde_json::from_slice(&body).expect("JWK Set");
    let keys = document["keys"].as_array().expect("keys").clone();
    assert_eq!(kids(&keys), ["rotation-current", "rotation-next"]);
}

#[tokio::test]
async fn an_unusable_rotation_set_stops_startup_and_names_the_setting() {
    // Given: a rotation set that carries a private key.
    let mut private = published(&issuer(rotation_test_rsa_private_key_pem(), "next"))[0].clone();
    private["d"] = json!("AQAB");
    let mut config = oauth_config(RuntimeMode::Parity);
    config.jwt_private_key_pem = Some(shared_test_rsa_private_key_pem().to_owned());
    config.jwt_key_id = Some("current".into());
    config.jwt_rotation_jwks_json = Some(json!({ "keys": [private] }).to_string());
    let pool = PgPoolOptions::new()
        .connect_lazy(&config.database_url)
        .expect("lazy pool");

    // When: the service is assembled.
    let error = match build_state(&config, pool).await {
        Ok(_) => panic!("startup must fail"),
        Err(error) => error.to_string(),
    };

    // Then: it refuses to start and says which setting to fix.
    assert!(error.contains("AUTH_JWT_ROTATION_JWKS_JSON"), "{error}");
}

fn issuer(private_key_pem: &str, kid: &str) -> JwtTokenService {
    JwtTokenService::from_private_key_pem(ISSUER, AUDIENCE, 900, 1_209_600, private_key_pem, kid)
        .expect("token service")
}

fn published(service: &JwtTokenService) -> Vec<Value> {
    auth_service::adapters::r#in::jwks::public_jwks(&(service.jwk_set()))["keys"]
        .as_array()
        .expect("published keys")
        .clone()
}

fn kids(keys: &[Value]) -> Vec<&str> {
    keys.iter()
        .map(|key| key["kid"].as_str().expect("kid"))
        .collect()
}

fn kid_of(token: &str) -> String {
    jsonwebtoken::decode_header(token)
        .expect("token header")
        .kid
        .expect("token names its key")
}

#[test]
fn a_session_is_named_by_its_first_refresh_token_and_a_refresh_keeps_that_name() {
    // Given: an issuer.
    let service = JwtTokenService::from_private_key_pem(
        ISSUER,
        AUDIENCE,
        900,
        1_209_600,
        shared_test_rsa_private_key_pem(),
        "session-test-key",
    )
    .expect("service");
    let user_id = Uuid::new_v4();

    // When: a session starts and is then continued by a refresh.
    let first = service
        .issue_token_pair(user_id, vec!["ROLE_USER".into()])
        .expect("issue");
    let next = service
        .continue_session(user_id, vec!["ROLE_USER".into()], first.session_id)
        .expect("continue");

    // Then: the session id is the family id a new login stores (its first
    // refresh jti), both access tokens carry it, and the refresh token is new.
    assert_eq!(first.session_id.to_string(), first.refresh_jti);
    assert_eq!(next.session_id, first.session_id);
    assert_ne!(next.refresh_jti, first.refresh_jti);
    for tokens in [&first, &next] {
        let decoded = service
            .decode_access_token(&tokens.access_token)
            .expect("decode");
        assert_eq!(decoded.session_id, Some(first.session_id));
    }
}
