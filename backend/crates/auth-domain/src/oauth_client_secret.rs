//! Pure Apple client-secret identity and lifetime policy.
//!
//! Key material, JWT serialization and signing belong to the outbound security
//! adapter. This policy accepts an explicit clock value and never reads a key,
//! clock, database or provider endpoint.

use anyhow::{bail, Context, Result};

pub const APPLE_TOKEN_AUDIENCE: &str = "https://appleid.apple.com";
pub const APPLE_CLIENT_SECRET_MAX_TTL_SECONDS: i64 = 15_777_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppleClientSecretClaims {
    pub issuer: String,
    pub subject: String,
    pub audience: String,
    pub issued_at: i64,
    pub expires_at: i64,
}

#[derive(Debug, Clone)]
pub struct AppleClientSecretPolicy {
    team_id: String,
    client_id: String,
    key_id: String,
    ttl_seconds: i64,
}

impl AppleClientSecretPolicy {
    pub fn new(team_id: &str, key_id: &str, client_id: &str, ttl_seconds: i64) -> Result<Self> {
        for (label, value) in [
            ("Apple team id", team_id),
            ("Apple key id", key_id),
            ("Apple client id", client_id),
        ] {
            if value.trim().is_empty() {
                bail!("{label} is required to generate the Apple client secret");
            }
        }
        if ttl_seconds <= 0 || ttl_seconds > APPLE_CLIENT_SECRET_MAX_TTL_SECONDS {
            bail!("Apple client secret lifetime must be between 1 and {APPLE_CLIENT_SECRET_MAX_TTL_SECONDS} seconds");
        }
        Ok(Self {
            team_id: team_id.trim().to_owned(),
            client_id: client_id.trim().to_owned(),
            key_id: key_id.trim().to_owned(),
            ttl_seconds,
        })
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn ttl_seconds(&self) -> i64 {
        self.ttl_seconds
    }

    pub fn claims_at(&self, issued_at: i64) -> Result<AppleClientSecretClaims> {
        let expires_at = issued_at
            .checked_add(self.ttl_seconds)
            .context("Apple client secret expiry is outside the timestamp range")?;
        Ok(AppleClientSecretClaims {
            issuer: self.team_id.clone(),
            subject: self.client_id.clone(),
            audience: APPLE_TOKEN_AUDIENCE.to_owned(),
            issued_at,
            expires_at,
        })
    }
}
