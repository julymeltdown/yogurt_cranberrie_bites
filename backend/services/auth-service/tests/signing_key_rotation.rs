//! A signing-key rotation drill across the real issuer and the Gateway's client.
//!
//! Real PostgreSQL, the real auth use cases and gRPC handler behind the
//! registered-caller interceptor, and the Gateway's own auth client, key
//! refresher and verifier. Each rotation step is a new issuer instance built
//! from changed settings against the same database, which is what a rolling
//! restart does. The Gateway side is never rebuilt. The channel is the
//! plaintext development path; mutual TLS on it is covered by the transport
//! tests. Requires Docker and never skips.
#[path = "support/issuer.rs"]
mod issuer;
#[path = "support/mod.rs"]
mod support;

use anyhow::Context as _;
use api_gateway::adapters::out::auth_client::GrpcAuthClient;
use api_gateway::adapters::out::jwt::{JwtVerifier, TokenRejection};
use api_gateway::adapters::out::signing_keys::{
    RefreshOutcome, RefreshPolicy, SigningKeyRefresher,
};
use api_gateway::application::dto::issuer_dto::SigningKeysDto;
use api_gateway::application::ports::SigningKeySourcePort;
use async_trait::async_trait;
use auth_service::application::ports::TokenServicePort;
use auth_service::config::{AppConfig, JwtTokenService};
use issuer::Issuer;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use support::{rotation_test_rsa_private_key_pem, shared_test_rsa_private_key_pem, TestApp};

const POLICY: RefreshPolicy = RefreshPolicy {
    interval: Duration::from_secs(60),
    unknown_kid_min_interval: Duration::from_secs(10),
};

#[tokio::test]
async fn a_signing_key_rotation_reaches_the_gateway_without_signing_anyone_out(
) -> anyhow::Result<()> {
    let app = support::start_postgres_app(true).await?;
    let previous_key = shared_test_rsa_private_key_pem();
    let next_key = rotation_test_rsa_private_key_pem();
    let previous_public = public_key_set(&app.config, previous_key, "previous")?;
    let next_public = public_key_set(&app.config, next_key, "next")?;

    // The Gateway starts without configured keys and follows whichever issuer
    // instance is current, as it would follow a rolling restart.
    let verifier = JwtVerifier::without_keys(&app.config.issuer, &app.config.access_audience)?;
    let issuer = Arc::new(CurrentIssuer::default());
    let refresher = SigningKeyRefresher::new(verifier.clone(), issuer.clone(), POLICY, None);

    // Steady state: the issuer signs with the previous key.
    let steady = Issuer::serve(&app, signing(&app, previous_key, "previous", None)).await?;
    issuer.switch_to(&steady);
    assert_eq!(refresher.refresh().await, RefreshOutcome::Installed);
    assert_eq!(verifier.key_ids(), ["previous"]);
    let session = steady
        .state
        .login
        .login("hana@example.com", "Passw0rd!", None, None)
        .await
        .context("signing in before the rotation")?;
    verifier
        .verify_access_token(&session.access_token)
        .context("verifying a pre-rotation access token")?;

    // Step 1: publish the next key ahead of the switch.
    let publishing = Issuer::serve(
        &app,
        signing(&app, previous_key, "previous", Some(next_public)),
    )
    .await?;
    issuer.switch_to(&publishing);
    assert_eq!(refresher.refresh().await, RefreshOutcome::Installed);
    assert_eq!(verifier.key_ids(), ["next", "previous"]);

    // Step 2: sign with the next key and keep the previous key published.
    let switched =
        Issuer::serve(&app, signing(&app, next_key, "next", Some(previous_public))).await?;
    issuer.switch_to(&switched);
    // The session from before the switch refreshes: nobody is signed out.
    let refreshed = switched
        .state
        .login
        .refresh(&session.refresh_token)
        .await
        .context("refreshing the pre-switch session after the switch")?;
    assert_eq!(kid_of(&refreshed.access_token), "next");
    // The Gateway already holds the next key, so the first token signed with
    // it verifies without any fetch.
    let fetches = refresher.fetches();
    verifier
        .verify_access_token(&refreshed.access_token)
        .context("verifying the first token signed with the next key")?;
    assert_eq!(refresher.fetches(), fetches);
    // Access tokens signed before the switch verify until they expire.
    verifier
        .verify_access_token(&session.access_token)
        .context("verifying a pre-switch access token after the switch")?;

    // Step 3: retire the previous key once its tokens have expired.
    let retired = Issuer::serve(&app, signing(&app, next_key, "next", None)).await?;
    issuer.switch_to(&retired);
    assert_eq!(refresher.refresh().await, RefreshOutcome::Installed);
    assert_eq!(verifier.key_ids(), ["next"]);
    assert!(matches!(
        verifier.verify_access_token(&session.access_token),
        Err(TokenRejection::UnknownKid)
    ));
    // The issuer refuses the retired key's refresh token, and refusing it
    // touches no other session: the one that moved on still refreshes.
    assert!(retired
        .state
        .login
        .refresh(&session.refresh_token)
        .await
        .is_err());
    let again = retired
        .state
        .login
        .refresh(&refreshed.refresh_token)
        .await
        .context("refreshing the moved-on session after retirement")?;
    verifier
        .verify_access_token(&again.access_token)
        .context("verifying a token issued after retirement")?;

    for instance in [steady, publishing, switched, retired] {
        instance.stop().await?;
    }
    app.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn made_up_key_ids_cost_the_real_issuer_one_fetch() -> anyhow::Result<()> {
    // Given: a Gateway refreshing from the real issuer.
    let app = support::start_postgres_app(false).await?;
    let instance = Issuer::serve(&app, app.config.clone()).await?;
    let issuer = Arc::new(CurrentIssuer::default());
    issuer.switch_to(&instance);
    let verifier = JwtVerifier::without_keys(&app.config.issuer, &app.config.access_audience)?;
    let refresher = SigningKeyRefresher::new(verifier.clone(), issuer, POLICY, None);

    // When: two hundred requests at once name keys the issuer never published.
    let mut requests = tokio::task::JoinSet::new();
    for _ in 0..200 {
        let refresher = refresher.clone();
        requests.spawn(async move { refresher.refresh_for_unknown_kid().await });
    }
    while let Some(result) = requests.join_next().await {
        result?;
    }

    // Then: the issuer was asked once, and that answer installed its keys.
    assert_eq!(refresher.fetches(), 1);
    assert_eq!(verifier.key_count(), 1);
    instance.stop().await?;
    app.pool.close().await;
    Ok(())
}

/// Settings for an issuer instance that signs with `private_key_pem` and
/// publishes `rotation` beside it.
///
/// Seeding is off: a rolling restart does not re-seed. Re-seeding re-hashes
/// the demo password, and a credential change correctly revokes that user's
/// sessions, which would end the session this drill follows.
fn signing(app: &TestApp, private_key_pem: &str, kid: &str, rotation: Option<String>) -> AppConfig {
    let mut config = app.config.clone();
    config.seed_enabled = false;
    config.jwt_private_key_pem = Some(private_key_pem.to_owned());
    config.jwt_key_id = Some(kid.to_owned());
    config.jwt_rotation_jwks_json = rotation;
    config
}

/// The public key set an issuer signing with `private_key_pem` publishes.
fn public_key_set(config: &AppConfig, private_key_pem: &str, kid: &str) -> anyhow::Result<String> {
    let service = JwtTokenService::from_private_key_pem(
        &config.issuer,
        &config.access_audience,
        config.access_ttl_seconds,
        config.refresh_ttl_seconds,
        private_key_pem,
        kid,
    )?;
    Ok(auth_service::adapters::r#in::jwks::public_jwks(&(service.jwk_set())).to_string())
}

fn kid_of(token: &str) -> String {
    jsonwebtoken::decode_header(token)
        .expect("token header")
        .kid
        .expect("token names its key")
}

/// The issuer instance the Gateway currently reaches.
#[derive(Default)]
struct CurrentIssuer(Mutex<Option<GrpcAuthClient>>);

impl CurrentIssuer {
    fn switch_to(&self, instance: &Issuer) {
        *self.0.lock().expect("current issuer") = Some(instance.client.clone());
    }
}

impl CurrentIssuer {
    async fn signing_keys_gateway_client_inner(&self) -> anyhow::Result<SigningKeysDto> {
        let client = self
            .0
            .lock()
            .expect("current issuer")
            .clone()
            .ok_or_else(|| anyhow::anyhow!("no issuer instance is running"))?;
        client.signing_keys().await.map_err(Into::into)
    }
}

#[async_trait]
impl SigningKeySourcePort for CurrentIssuer {
    async fn signing_keys(
        &self,
    ) -> api_gateway::application::client_error::GatewayClientResult<SigningKeysDto> {
        self.signing_keys_gateway_client_inner()
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
}
