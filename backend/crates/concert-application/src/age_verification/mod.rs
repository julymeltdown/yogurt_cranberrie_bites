//! BE26 private application values and verifier port. No transport or persistence imports.
//! Context/binding values are correlation only; mutations require fenced owner authority.
use crate::{age_policy::Context, identity::AuthorityBinding};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_domain::age_policy::{
    arithmetic::{derive_exact_age_claim, ExactAgeError, SeoulDate},
    seoul_date, AgeStatus, OwnerClaim, StoredStatus,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const RETURN_TARGET: &str = "AGE_PRIVACY";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Invalid,
    Denied,
    ProviderUnavailable,
    Unavailable,
    Pending,
    Conflict,
    Expired,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Invalid => "Invalid verification input",
            Self::Denied => "Verification access denied",
            Self::ProviderUnavailable => "PROVIDER_UNAVAILABLE",
            Self::Unavailable => "Verification dependency unavailable",
            Self::Pending => "Verification outcome pending; recover original command identity",
            Self::Conflict => "Verification command conflict",
            Self::Expired => "Verification expired",
        })
    }
}
impl std::error::Error for Error {}
/// Converts the existing BE04 binding, without upgrading it to a mutation fence.
pub fn context(
    binding: &AuthorityBinding,
    request: Uuid,
    expires_at: DateTime<Utc>,
) -> Result<Context, Error> {
    Ok(Context {
        actor: binding.context.key.user_id,
        event: binding.context.key.event_id,
        session: Uuid::parse_str(binding.session_id.as_str()).map_err(|_| Error::Invalid)?,
        request,
        expires_at,
    })
}
pub fn check(c: Context, now: DateTime<Utc>) -> Result<(), Error> {
    c.check(now).map_err(|_| Error::Denied)
}
pub fn identifier(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 100
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
/// Only the digest is persisted. Raw state is transient and never Debug/Serialize.
pub struct StateToken(String);
impl StateToken {
    pub fn from_random_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes.iter().map(|b| format!("{b:02x}")).collect())
    }
    pub fn parse(value: String) -> Result<Self, Error> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::Invalid);
        }
        Ok(Self(value))
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
    pub fn digest(&self) -> [u8; 32] {
        Sha256::digest(self.0.as_bytes()).into()
    }
}
pub struct ProviderResponse(String);
impl ProviderResponse {
    pub fn new(v: String) -> Result<Self, Error> {
        if v.is_empty() || v.len() > 4096 || v.chars().any(char::is_control) {
            return Err(Error::Invalid);
        }
        Ok(Self(v))
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
#[derive(Clone)]
pub struct ProviderConfig {
    pub verifier: String,
    pub revision: String,
    endpoint: String,
}
impl ProviderConfig {
    /// Server configuration only. Qualification/credentials are an external deployment gate.
    /// Deliberately accepts only canonical HTTPS host/path endpoints, no ports, escapes,
    /// userinfo, query, fragment, IP literals or browser-provided redirect destinations.
    pub fn new(
        verifier: &str,
        revision: &str,
        endpoint: &str,
        allowlist: &[&str],
    ) -> Result<Self, Error> {
        let rest = endpoint.strip_prefix("https://").ok_or(Error::Invalid)?;
        let (host, path) = rest.split_once('/').ok_or(Error::Invalid)?;
        if !identifier(verifier)
            || !identifier(revision)
            || endpoint.len() > 1800
            || !allowlist.contains(&endpoint)
            || host.len() > 253
            || host.split('.').count() < 2
            || host.split('.').any(|l| {
                l.is_empty()
                    || l.len() > 63
                    || l.starts_with('-')
                    || l.ends_with('-')
                    || !l
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            })
            || host.bytes().all(|b| b.is_ascii_digit() || b == b'.')
            || path.is_empty()
            || path.split('/').any(|p| p == "." || p == "..")
            || !path
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/_-".contains(&b))
        {
            return Err(Error::Invalid);
        }
        Ok(Self {
            verifier: verifier.into(),
            revision: revision.into(),
            endpoint: endpoint.into(),
        })
    }
    pub fn authorization_url(&self, id: Uuid, state: &StateToken) -> String {
        format!(
            "{}?verification_id={id}&state={}&return_target={RETURN_TARGET}",
            self.endpoint,
            state.expose()
        )
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct Binding {
    pub verification: Uuid,
    pub actor: Uuid,
    pub event: Uuid,
    pub session: Uuid,
    pub verifier: String,
    pub state_digest: [u8; 32],
}
pub struct Challenge {
    pub binding: Binding,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub source_revision: String,
}
/// Constructed ONLY by the independently verifying trusted server adapter, never from a
/// public callback body. DOB/proof/token fields are never serialized, logged or persisted.
pub struct VerifiedProof {
    pub binding: Binding,
    pub verified_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub source_revision: String,
    pub birthyear: String,
    pub birthday: String,
}
#[async_trait]
pub trait Verifier: Send + Sync {
    fn available(&self) -> bool;
    /// Must independently validate provider proof, audience/issuer/signature, nonce and all
    /// challenge bindings. Caller enforces deadline; implementations must bound network IO.
    async fn verify(
        &self,
        challenge: &Challenge,
        response: &ProviderResponse,
    ) -> Result<VerifiedProof, Error>;
}
pub struct UnavailableVerifier;
#[async_trait]
impl Verifier for UnavailableVerifier {
    fn available(&self) -> bool {
        false
    }
    async fn verify(&self, _: &Challenge, _: &ProviderResponse) -> Result<VerifiedProof, Error> {
        Err(Error::ProviderUnavailable)
    }
}
pub fn claim(
    proof: VerifiedProof,
    expected: &Challenge,
    config: &ProviderConfig,
    now: DateTime<Utc>,
) -> Result<OwnerClaim, Error> {
    if expected.source_revision != config.revision
        || proof.binding != expected.binding
        || proof.source_revision != config.revision
        || proof.binding.verifier != config.verifier
        || proof.verified_at < expected.created_at
        || proof.verified_at > now
        || now >= expected.expires_at
        || proof.expires_at <= now
        || proof.expires_at <= proof.verified_at
        || proof.expires_at - proof.verified_at > Duration::days(90)
    {
        return Err(Error::Denied);
    }
    let derived = derive_exact_age_claim(
        Some(&proof.birthyear),
        Some(&proof.birthday),
        SeoulDate::new(seoul_date(proof.verified_at).map_err(|_| Error::Invalid)?),
    );
    let (status, age, next) = match derived {
        Ok(v) => (
            StoredStatus::Verified,
            Some(i16::from(v.age_at_verification())),
            Some(v.next_birthday_at().as_naive_date()),
        ),
        Err(ExactAgeError::UnsupportedAge) => (StoredStatus::Unsupported, None, None),
        Err(_) => return Err(Error::Invalid),
    };
    Ok(OwnerClaim {
        claim_id: expected.binding.verification,
        user_id: concert_domain::AuthUserId::from_uuid(expected.binding.actor),
        verifier_id: config.verifier.clone(),
        source_revision: config.revision.clone(),
        status,
        verified_at: proof.verified_at,
        expires_at: proof.expires_at,
        age_at_verification: age,
        next_birthday_at: next,
    })
}
/// Private own-read projection only; safe getters, no exact age or proof/DOB fields.
pub struct OwnStatus {
    pub user_id: Uuid,
    pub event_id: Uuid,
    pub status: AgeStatus,
    pub verified_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub verification_available: bool,
}
pub fn own_status(
    c: Context,
    claims: &[OwnerClaim],
    available: bool,
    now: DateTime<Utc>,
) -> Result<OwnStatus, Error> {
    check(c, now)?;
    let a =
        concert_domain::age_policy::assess(c.actor, claims, now).map_err(|_| Error::Unavailable)?;
    let status = if a.status() == AgeStatus::Unknown && !available {
        AgeStatus::ProviderUnavailable
    } else {
        a.status()
    };
    let (verified_at, expires_at) = a.timestamps();
    Ok(OwnStatus {
        user_id: c.actor.uuid(),
        event_id: c.event.uuid(),
        status,
        verified_at,
        expires_at,
        verification_available: available,
    })
}
/// Stable correlation target for BE15, not a permission or a secret.
pub fn verification_id(c: Context, key: Uuid) -> Uuid {
    let mut h = Sha256::new();
    h.update(b"MCR-BE26-start-v1");
    h.update(c.actor.uuid().as_bytes());
    h.update(c.event.uuid().as_bytes());
    h.update(key.as_bytes());
    Uuid::from_bytes(h.finalize()[..16].try_into().expect("sha256 length"))
}
