//! Dedicated Concert-to-Profile qualified-read identity. A signature routes a bound
//! request; real owner-held guards are still mandatory at the Profile producer.
use super::{ActorDelegationClient, ActorDelegationSigner, ActorDelegationVerifier};
use crate::grpc_auth::Workload;
use std::fmt;
pub const SIGNING_SEED_ENV: &str = "CONCERT_PROFILE_SELF_SIGNING_SEED";
pub const VERIFYING_KEYS_ENV: &str = "CONCERT_PROFILE_SELF_VERIFYING_KEYS";
#[derive(Clone)]
pub struct ConcertSelfProfileSigner(ActorDelegationSigner);
#[derive(Clone)]
pub struct ConcertSelfProfileVerifier(ActorDelegationVerifier);
impl fmt::Debug for ConcertSelfProfileSigner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ConcertSelfProfileSigner([REDACTED])")
    }
}
impl fmt::Debug for ConcertSelfProfileVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ConcertSelfProfileVerifier([REDACTED])")
    }
}
fn distinct(key: &[u8; 32], gateway: &ActorDelegationVerifier) -> anyhow::Result<()> {
    anyhow::ensure!(
        !gateway.trusts(key)
            && ![Workload::Gateway, Workload::Concert, Workload::Profile]
                .iter()
                .any(|w| *key == ActorDelegationSigner::local_parity(*w).public_key()),
        "SELF Profile producer keys must be independent of Gateway and parity keys"
    );
    Ok(())
}
impl ConcertSelfProfileSigner {
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
        ActorDelegationClient::new(inner, self.0.clone(), Workload::Profile)
    }
}
impl ConcertSelfProfileVerifier {
    pub fn new(keys: &[[u8; 32]], gateway: &ActorDelegationVerifier) -> anyhow::Result<Self> {
        anyhow::ensure!(
            gateway.audience() == Workload::Profile,
            "SELF Profile producers require Profile audience"
        );
        for key in keys {
            distinct(key, gateway)?;
        }
        Ok(Self(ActorDelegationVerifier::new(Workload::Profile, keys)?))
    }
    pub fn configured_from_env(gateway: &ActorDelegationVerifier) -> anyhow::Result<Option<Self>> {
        let value = match std::env::var(VERIFYING_KEYS_ENV) {
            Ok(v) => v,
            Err(std::env::VarError::NotPresent) => return Ok(None),
            Err(_) => anyhow::bail!("invalid SELF Profile key configuration"),
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
    fn self_profile_keys_are_explicit_independent_and_audience_scoped() {
        let old = ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32]).unwrap();
        let gateway = ActorDelegationVerifier::new(Workload::Profile, &[old.public_key()]).unwrap();
        assert!(ConcertSelfProfileSigner::from_seed(&[41; 32], &gateway).is_err());
        let signer = ConcertSelfProfileSigner::from_seed(&[39; 32], &gateway).unwrap();
        assert!(ConcertSelfProfileVerifier::new(&[signer.public_key()], &gateway).is_ok());
        assert!(ConcertSelfProfileVerifier::new(&[old.public_key()], &gateway).is_err());
        assert!(ConcertSelfProfileVerifier::new(&[], &gateway).is_err());
        let wrong = ActorDelegationVerifier::new(Workload::Auth, &[old.public_key()]).unwrap();
        assert!(ConcertSelfProfileVerifier::new(&[signer.public_key()], &wrong).is_err());
    }
}
