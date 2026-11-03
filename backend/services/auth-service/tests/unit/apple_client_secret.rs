//! Tests for the generated Apple client secret.
//!
//! Apple requires an ES256 JWT rather than an issued string, so these tests check
//! the shape Apple validates: algorithm, key id, claims, and lifetime. They also
//! check the failure that matters operationally, which is a key of the wrong
//! curve being accepted here and rejected by Apple later.

use auth_service::adapters::out::security::apple_client_secret::AppleClientSecretGenerator;
use auth_service::domain::oauth_client_secret::{
    AppleClientSecretPolicy, APPLE_CLIENT_SECRET_MAX_TTL_SECONDS, APPLE_TOKEN_AUDIENCE,
};
use chrono::{TimeZone, Utc};
use jsonwebtoken::{decode_header, Algorithm};
use p256::pkcs8::EncodePrivateKey;

/// A deterministic test key. It never leaves this file.
fn test_signing_key() -> p256::ecdsa::SigningKey {
    p256::ecdsa::SigningKey::from_bytes(&[9_u8; 32].into())
        .expect("deterministic P-256 signing key")
}

fn test_key_pem() -> String {
    test_signing_key()
        .to_pkcs8_pem(Default::default())
        .expect("pkcs8 pem")
        .to_string()
}

fn generator() -> AppleClientSecretGenerator {
    AppleClientSecretGenerator::new(
        "TEAMID1234",
        "KEYID5678",
        "com.example.service",
        &test_key_pem(),
        86_400,
    )
    .expect("generator")
}

#[test]
fn generated_secret_is_an_es256_jwt_naming_the_signing_key() {
    let generator = generator();
    let secret = generator
        .generate(Utc.timestamp_opt(1_700_000_000, 0).unwrap())
        .expect("secret");

    let header = decode_header(&secret).expect("header");
    // Apple rejects a secret signed with anything other than ES256, which is the
    // reason a static configured string could never work.
    assert_eq!(header.alg, Algorithm::ES256);
    // Apple selects the key by kid, so the header must name it.
    assert_eq!(header.kid.as_deref(), Some("KEYID5678"));
}

#[test]
fn generated_secret_carries_the_claims_apple_validates() {
    let generator = generator();
    let now = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
    let secret = generator.generate(now).expect("secret");

    // Verified, not merely parsed. Skipping the signature would make this test
    // blind to a secret signed by the wrong key.
    let signing_key = test_signing_key();
    let verifying_key = signing_key.verifying_key();
    let public_point = verifying_key.to_encoded_point(false);
    let decoding = jsonwebtoken::DecodingKey::from_ec_der(public_point.as_bytes());
    let mut validation = jsonwebtoken::Validation::new(Algorithm::ES256);
    // Audience and expiry are asserted below as claims, and the fixture clock is
    // fixed, so this validation is about the signature.
    validation.validate_aud = false;
    validation.validate_exp = false;
    let decoded =
        jsonwebtoken::decode::<serde_json::Value>(&secret, &decoding, &validation).expect("claims");
    let claims = decoded.claims;

    // Apple reads the team as issuer, the service identifier as subject, and
    // itself as audience.
    assert_eq!(claims["iss"], "TEAMID1234");
    assert_eq!(claims["sub"], "com.example.service");
    assert_eq!(claims["aud"], APPLE_TOKEN_AUDIENCE);
    assert_eq!(claims["iat"], now.timestamp());
    assert_eq!(claims["exp"], now.timestamp() + 86_400);
}

#[test]
fn generated_secret_lifetime_is_bounded_by_what_apple_accepts() {
    let now = Utc.timestamp_opt(1_700_000_000, 0).unwrap();

    let generator = AppleClientSecretGenerator::new(
        "TEAMID1234",
        "KEYID5678",
        "com.example.service",
        &test_key_pem(),
        APPLE_CLIENT_SECRET_MAX_TTL_SECONDS,
    )
    .expect("maximum lifetime is accepted");
    assert_eq!(generator.ttl_seconds(), APPLE_CLIENT_SECRET_MAX_TTL_SECONDS);
    assert!(generator.generate(now).is_ok());

    // One second past Apple's maximum is rejected here rather than presented to
    // Apple and refused there, where the cause is far harder to see.
    let error = AppleClientSecretGenerator::new(
        "TEAMID1234",
        "KEYID5678",
        "com.example.service",
        &test_key_pem(),
        APPLE_CLIENT_SECRET_MAX_TTL_SECONDS + 1,
    )
    .expect_err("over-long lifetime must fail");
    assert!(error.to_string().contains("lifetime"));

    let error = AppleClientSecretGenerator::new(
        "TEAMID1234",
        "KEYID5678",
        "com.example.service",
        &test_key_pem(),
        0,
    )
    .expect_err("zero lifetime must fail");
    assert!(error.to_string().contains("lifetime"));
}

#[test]
fn an_rsa_key_is_rejected_instead_of_producing_a_secret_apple_would_refuse() {
    // Apple requires ES256. Accepting an RSA key here would produce an RS256
    // secret that Apple rejects at the token endpoint, turning a deployment
    // mistake into a sign-in failure for every user.
    let rsa_pem = crate::support::shared_test_rsa_private_key_pem();
    let error = AppleClientSecretGenerator::new(
        "TEAMID1234",
        "KEYID5678",
        "com.example.service",
        rsa_pem,
        86_400,
    )
    .expect_err("an RSA key must not be accepted for an ES256 secret");
    assert!(error.to_string().contains("ES256"));
}

#[test]
fn missing_key_material_is_rejected_by_name() {
    for (team_id, key_id, client_id, key_pem, expected) in [
        ("", "KEYID5678", "com.example.service", "pem", "team id"),
        ("TEAMID1234", "", "com.example.service", "pem", "key id"),
        ("TEAMID1234", "KEYID5678", "", "pem", "client id"),
        (
            "TEAMID1234",
            "KEYID5678",
            "com.example.service",
            "",
            "private key",
        ),
    ] {
        let error = AppleClientSecretGenerator::new(team_id, key_id, client_id, key_pem, 86_400)
            .err()
            .unwrap_or_else(|| panic!("expected a failure naming the {expected}"));
        assert!(error.to_string().contains(expected));
    }
}

#[test]
fn debug_output_omits_the_signing_key() {
    let rendered = format!("{:?}", generator());

    // A Debug rendering reaches logs and error reports, so it must not carry key
    // material.
    assert!(rendered.contains("KEYID5678"));
    assert!(!rendered.contains("BEGIN PRIVATE KEY"));
    assert!(!rendered.contains("private_key"));
}

#[test]
fn pure_policy_trims_identity_and_rejects_expiry_overflow() {
    let policy =
        AppleClientSecretPolicy::new(" TEAMID1234 ", " KEYID5678 ", " com.example.service ", 1)
            .expect("policy without any key material");
    let claims = policy.claims_at(1_700_000_000).expect("claims");
    assert_eq!(claims.issuer, "TEAMID1234");
    assert_eq!(claims.subject, "com.example.service");
    assert_eq!(claims.audience, APPLE_TOKEN_AUDIENCE);
    assert_eq!(claims.issued_at, 1_700_000_000);
    assert_eq!(claims.expires_at, 1_700_000_001);
    assert_eq!(policy.key_id(), "KEYID5678");
    assert!(policy.claims_at(i64::MAX).is_err());
    assert!(AppleClientSecretPolicy::new("team", "key", "client", -1).is_err());
}

#[test]
fn a_reused_signer_mints_fresh_lifetimes_without_changing_key_identity() {
    let generator = generator();
    let first = generator
        .generate(Utc.timestamp_opt(1_700_000_000, 0).unwrap())
        .expect("first");
    let second = generator
        .generate(Utc.timestamp_opt(1_700_086_400, 0).unwrap())
        .expect("second");
    let public_point = test_signing_key().verifying_key().to_encoded_point(false);
    let decoding = jsonwebtoken::DecodingKey::from_ec_der(public_point.as_bytes());
    let mut validation = jsonwebtoken::Validation::new(Algorithm::ES256);
    validation.validate_aud = false;
    validation.validate_exp = false;
    let first = jsonwebtoken::decode::<serde_json::Value>(&first, &decoding, &validation)
        .expect("first verified");
    let second = jsonwebtoken::decode::<serde_json::Value>(&second, &decoding, &validation)
        .expect("second verified");
    assert_eq!(first.header.kid, second.header.kid);
    assert_eq!(second.claims["iat"], 1_700_086_400_i64);
    assert_eq!(second.claims["exp"], 1_700_172_800_i64);
    assert!(second.claims["iat"].as_i64().unwrap() >= first.claims["exp"].as_i64().unwrap());
}

#[tokio::test]
async fn production_composition_signs_fresh_apple_secrets_for_actual_http_exchanges() {
    use auth_service::config::{build_oauth_provider, RuntimeMode};
    use auth_service::domain::user::IdentityProvider;
    use axum::{extract::Form, http::StatusCode, routing::post, Json, Router};
    use base64ct::{Base64UrlUnpadded, Encoding};
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    let signer = test_signing_key();
    let public_point = signer.verifying_key().to_encoded_point(false);
    let jwks = serde_json::json!({"keys": [{
        "kty": "EC", "use": "sig", "alg": "ES256", "crv": "P-256", "kid": "fixture-apple-key",
        "x": Base64UrlUnpadded::encode_string(public_point.x().expect("x")),
        "y": Base64UrlUnpadded::encode_string(public_point.y().expect("y"))
    }]})
    .to_string();
    let now = Utc::now().timestamp();
    let mut header = jsonwebtoken::Header::new(Algorithm::ES256);
    header.kid = Some("fixture-apple-key".into());
    let id_token = jsonwebtoken::encode(
        &header,
        &serde_json::json!({
            "iss": APPLE_TOKEN_AUDIENCE, "aud": "apple-client", "sub": "fixture-subject",
            "iat": now, "exp": now + 300, "nonce": "fixture-nonce"
        }),
        &jsonwebtoken::EncodingKey::from_ec_pem(test_key_pem().as_bytes()).expect("fixture key"),
    )
    .expect("fixture identity token");
    let decoding = jsonwebtoken::DecodingKey::from_ec_der(public_point.as_bytes());
    let issued = Arc::new(Mutex::new(Vec::<i64>::new()));
    let observed = issued.clone();
    let app = Router::new().route("/token", post(move |Form(form): Form<HashMap<String, String>>| {
        let decoding = decoding.clone();
        let observed = observed.clone();
        let id_token = id_token.clone();
        async move {
            if form.get("client_id").map(String::as_str) != Some("apple-client")
                || form.get("grant_type").map(String::as_str) != Some("authorization_code") {
                return Err(StatusCode::BAD_REQUEST);
            }
            let secret = form.get("client_secret").ok_or(StatusCode::UNAUTHORIZED)?;
            let mut validation = jsonwebtoken::Validation::new(Algorithm::ES256);
            validation.set_issuer(&["TEAMID1234"]);
            validation.set_audience(&[APPLE_TOKEN_AUDIENCE]);
            validation.leeway = 0;
            let token = jsonwebtoken::decode::<serde_json::Value>(secret, &decoding, &validation)
                .map_err(|_| StatusCode::UNAUTHORIZED)?;
            let issued_at = token.claims["iat"].as_i64().ok_or(StatusCode::UNAUTHORIZED)?;
            if token.header.kid.as_deref() != Some("KEYID5678")
                || token.claims["sub"] != "apple-client"
                || token.claims["exp"].as_i64() != Some(issued_at + 2) {
                return Err(StatusCode::UNAUTHORIZED);
            }
            observed.lock().expect("observations").push(issued_at);
            Ok(Json(serde_json::json!({"access_token": "fixture-access", "id_token": id_token, "token_type": "Bearer"})))
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fixture");
    let address = listener.local_addr().expect("fixture address");
    let mut config = crate::config_behavior::oauth_config(RuntimeMode::Production);
    config.apple_private_key_pem = Some(test_key_pem());
    config.apple_team_id = Some("TEAMID1234".into());
    config.apple_key_id = Some("KEYID5678".into());
    config.apple_client_secret_ttl_seconds = 2;
    config.apple_client_secret = Some("unused-static-credential".into());
    config.apple_jwks_json = Some(jwks);
    config.apple_token_uri = Some(format!("http://{address}/token"));
    config.apple_userinfo_uri = Some(format!("http://{address}/must-not-be-called"));
    let provider = build_oauth_provider(&config)
        .expect("actual production composition with local provider fixture");
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = stop_rx.await;
            })
            .await
    });
    let scenario = async {
        for attempt in 0..2 {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
            }
            let user = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                provider.fetch_user_info(
                    (IdentityProvider::Apple).into(),
                    "fixture-code",
                    "http://127.0.0.1/callback",
                    None,
                    "fixture-nonce",
                ),
            )
            .await??;
            anyhow::ensure!(
                user.subject == "fixture-subject",
                "provider identity changed"
            );
        }
        anyhow::Ok(())
    }
    .await;
    let _ = stop_tx.send(());
    tokio::time::timeout(std::time::Duration::from_secs(5), server)
        .await
        .expect("fixture server stopped")
        .expect("fixture server joined")
        .expect("fixture server completed");
    scenario.expect("both actual HTTP exchanges verified fresh signed secrets");
    let issued = issued.lock().expect("observations");
    assert_eq!(issued.len(), 2);
    assert!(
        issued[1] > issued[0],
        "second exchange must mint after the first two-second lifetime"
    );
}
