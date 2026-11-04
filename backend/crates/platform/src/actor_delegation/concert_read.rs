//! Dedicated Concert-to-Post qualified-read identity. A signature routes a bound
//! request; real owner-held guards are still mandatory at the Post producer.
use super::{ActorDelegationClient, ActorDelegationSigner, ActorDelegationVerifier};
use crate::grpc_auth::Workload;
use std::fmt;
pub const SIGNING_SEED_ENV: &str = "CONCERT_POST_READ_SIGNING_SEED";
pub const VERIFYING_KEYS_ENV: &str = "CONCERT_POST_READ_VERIFYING_KEYS";
#[derive(Clone)]
pub struct ConcertPostReadSigner(ActorDelegationSigner);
#[derive(Clone)]
pub struct ConcertPostReadVerifier(ActorDelegationVerifier);
impl fmt::Debug for ConcertPostReadSigner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ConcertPostReadSigner([REDACTED])")
    }
}
impl fmt::Debug for ConcertPostReadVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ConcertPostReadVerifier([REDACTED])")
    }
}
fn distinct(key: &[u8; 32], gateway: &ActorDelegationVerifier) -> anyhow::Result<()> {
    anyhow::ensure!(
        !gateway.trusts(key)
            && *key != ActorDelegationSigner::local_parity(Workload::Gateway).public_key(),
        "qualified Post read keys must be independent of Gateway and parity keys"
    );
    Ok(())
}
impl ConcertPostReadSigner {
    pub fn from_seed(seed: &[u8; 32], gateway: &ActorDelegationVerifier) -> anyhow::Result<Self> {
        let signer = ActorDelegationSigner::from_seed(Workload::Concert, seed)?;
        distinct(&signer.public_key(), gateway)?;
        Ok(Self(signer))
    }
    pub fn from_env(gateway: &ActorDelegationVerifier) -> anyhow::Result<Self> {
        let value = std::env::var(SIGNING_SEED_ENV)
            .map_err(|_| anyhow::anyhow!("{SIGNING_SEED_ENV} is required"))?;
        Self::from_seed(&super::decode_key::<32>(SIGNING_SEED_ENV, &value)?, gateway)
    }
    pub fn public_key(&self) -> [u8; 32] {
        self.0.public_key()
    }
    pub fn client<S>(&self, inner: S) -> ActorDelegationClient<S> {
        ActorDelegationClient::new(inner, self.0.clone(), Workload::Post)
    }
}
impl ConcertPostReadVerifier {
    pub fn new(keys: &[[u8; 32]], gateway: &ActorDelegationVerifier) -> anyhow::Result<Self> {
        anyhow::ensure!(
            gateway.audience() == Workload::Post,
            "qualified Post reads require Post audience"
        );
        for key in keys {
            distinct(key, gateway)?;
        }
        Ok(Self(ActorDelegationVerifier::new(Workload::Post, keys)?))
    }
    pub fn configured_from_env(gateway: &ActorDelegationVerifier) -> anyhow::Result<Option<Self>> {
        let value = match std::env::var(VERIFYING_KEYS_ENV) {
            Ok(v) => v,
            Err(std::env::VarError::NotPresent) => return Ok(None),
            Err(_) => anyhow::bail!("invalid qualified Post key configuration"),
        };
        let keys = value
            .split(',')
            .map(|v| super::decode_key::<32>(VERIFYING_KEYS_ENV, v.trim()))
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(Some(Self::new(&keys, gateway)?))
    }
    pub fn into_inner(self) -> ActorDelegationVerifier {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn post_read_keys_are_explicit_independent_and_audience_scoped() {
        let old = ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32]).unwrap();
        let gateway = ActorDelegationVerifier::new(Workload::Post, &[old.public_key()]).unwrap();
        assert!(ConcertPostReadSigner::from_seed(&[41; 32], &gateway).is_err());
        let signer = ConcertPostReadSigner::from_seed(&[39; 32], &gateway).unwrap();
        assert!(ConcertPostReadVerifier::new(&[signer.public_key()], &gateway).is_ok());
        assert!(ConcertPostReadVerifier::new(&[old.public_key()], &gateway).is_err());
        assert!(ConcertPostReadVerifier::new(&[], &gateway).is_err());
        let wrong = ActorDelegationVerifier::new(Workload::Auth, &[old.public_key()]).unwrap();
        assert!(ConcertPostReadVerifier::new(&[signer.public_key()], &wrong).is_err());
    }
}

/// Independently configured Post-to-Concert deferred subject admission channel.
#[derive(Clone)]
pub struct PostConcertRootReadSigner(ActorDelegationSigner);
pub struct PostConcertRootReadVerifier(ActorDelegationVerifier);
pub const ROOT_READ_SIGNING_SEED_ENV: &str = "POST_CONCERT_ROOT_READ_SIGNING_SEED";
pub const ROOT_READ_VERIFYING_KEYS_ENV: &str = "POST_CONCERT_ROOT_READ_VERIFYING_KEYS";
impl PostConcertRootReadSigner {
    pub fn from_seed(seed: &[u8; 32], gateway: &ActorDelegationVerifier) -> anyhow::Result<Self> {
        let signer = ActorDelegationSigner::from_seed(Workload::Post, seed)?;
        distinct(&signer.public_key(), gateway)?;
        Ok(Self(signer))
    }
    pub fn from_env(gateway: &ActorDelegationVerifier) -> anyhow::Result<Self> {
        let raw = std::env::var(ROOT_READ_SIGNING_SEED_ENV)
            .map_err(|_| anyhow::anyhow!("{ROOT_READ_SIGNING_SEED_ENV} is required"))?;
        Self::from_seed(
            &super::decode_key::<32>(ROOT_READ_SIGNING_SEED_ENV, &raw)?,
            gateway,
        )
    }
    pub fn public_key(&self) -> [u8; 32] {
        self.0.public_key()
    }
    pub fn client<S>(&self, inner: S) -> ActorDelegationClient<S> {
        ActorDelegationClient::new(inner, self.0.clone(), Workload::Concert)
    }
}
impl PostConcertRootReadVerifier {
    pub fn new(keys: &[[u8; 32]], gateway: &ActorDelegationVerifier) -> anyhow::Result<Self> {
        anyhow::ensure!(
            gateway.audience() == Workload::Concert,
            "root read audience must be Concert"
        );
        for key in keys {
            distinct(key, gateway)?;
        }
        Ok(Self(ActorDelegationVerifier::new(Workload::Concert, keys)?))
    }
    pub fn configured_from_env(gateway: &ActorDelegationVerifier) -> anyhow::Result<Option<Self>> {
        let raw = match std::env::var(ROOT_READ_VERIFYING_KEYS_ENV) {
            Ok(v) => v,
            Err(std::env::VarError::NotPresent) => return Ok(None),
            Err(_) => anyhow::bail!("Invalid root read verification configuration"),
        };
        let keys = raw
            .split(',')
            .map(|v| super::decode_key::<32>(ROOT_READ_VERIFYING_KEYS_ENV, v.trim()))
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(Some(Self::new(&keys, gateway)?))
    }
    pub fn into_inner(self) -> ActorDelegationVerifier {
        self.0
    }
}
#[cfg(test)]
mod root_tests {
    use super::*;
    #[test]
    fn root_callback_keys_have_no_gateway_or_parity_fallback() {
        let old = ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32]).unwrap();
        let gateway = ActorDelegationVerifier::new(Workload::Concert, &[old.public_key()]).unwrap();
        assert!(PostConcertRootReadSigner::from_seed(&[41; 32], &gateway).is_err());
        let signer = PostConcertRootReadSigner::from_seed(&[42; 32], &gateway).unwrap();
        assert!(PostConcertRootReadVerifier::new(&[signer.public_key()], &gateway).is_ok());
        assert!(PostConcertRootReadVerifier::new(&[], &gateway).is_err());
        assert!(PostConcertRootReadVerifier::new(&[old.public_key()], &gateway).is_err());
    }
}
