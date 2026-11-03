//! Apple client-secret key parsing and ES256 signing.
//!
//! The provider adapter generates a fresh JWT on each token exchange. Reusing
//! this signer does not rotate or retire its private key; key rotation remains
//! a separate deployment responsibility.

use crate::application::dto::oauth_client_secret_dto::AppleClientSecretClaimsDto;
use crate::domain::oauth_client_secret::AppleClientSecretPolicy;
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};

pub struct AppleClientSecretGenerator {
    policy: AppleClientSecretPolicy,
    encoding: EncodingKey,
}

impl std::fmt::Debug for AppleClientSecretGenerator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never render the signing key into logs or error reports.
        f.debug_struct("AppleClientSecretGenerator")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

impl AppleClientSecretGenerator {
    pub fn new(
        team_id: &str,
        key_id: &str,
        client_id: &str,
        private_key_pem: &str,
        ttl_seconds: i64,
    ) -> Result<Self> {
        let policy = AppleClientSecretPolicy::new(team_id, key_id, client_id, ttl_seconds)?;
        if private_key_pem.trim().is_empty() {
            bail!("Apple private key is required to generate the Apple client secret");
        }
        let encoding = EncodingKey::from_ec_pem(private_key_pem.as_bytes())
            .context("Apple private key must be an ES256 (prime256v1) private key in PEM form")?;
        Ok(Self { policy, encoding })
    }

    pub fn ttl_seconds(&self) -> i64 {
        self.policy.ttl_seconds()
    }

    pub fn generate(&self, now: DateTime<Utc>) -> Result<String> {
        let claims = AppleClientSecretClaimsDto::from(self.policy.claims_at(now.timestamp())?);
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(self.policy.key_id().to_owned());
        encode(&header, &claims, &self.encoding).context("signing the Apple client secret")
    }
}
