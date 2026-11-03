//! Explicit dedicated Profile-to-Concert decision query keys. These are not
//! owner-view capabilities, completion secrets, or a way to decide a mutation.
use crate::{
    actor_delegation::{ActorDelegationClient, ActorDelegationSigner, ActorDelegationVerifier},
    grpc_auth::Workload,
};
use std::fmt;

pub const SIGNING_SEED_ENV: &str = "PROFILE_PARTICIPANT_DECISION_SIGNING_SEED";
pub const VERIFYING_KEYS_ENV: &str = "PROFILE_PARTICIPANT_DECISION_VERIFYING_KEYS";

#[derive(Clone)]
pub struct ParticipantDecisionSigner(ActorDelegationSigner);
pub struct ParticipantDecisionVerifier(ActorDelegationVerifier);
impl fmt::Debug for ParticipantDecisionSigner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ParticipantDecisionSigner([REDACTED])")
    }
}
impl fmt::Debug for ParticipantDecisionVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ParticipantDecisionVerifier([REDACTED])")
    }
}
fn distinct(key: &[u8; 32], gateway: &ActorDelegationVerifier) -> anyhow::Result<()> {
    anyhow::ensure!(
        !gateway.trusts(key),
        "Decision key must differ from Gateway keys"
    );
    for workload in [Workload::Gateway, Workload::Concert, Workload::Profile] {
        anyhow::ensure!(
            *key != ActorDelegationSigner::local_parity(workload).public_key(),
            "Decision key cannot use a parity key"
        );
    }
    Ok(())
}
impl ParticipantDecisionSigner {
    pub fn from_seed(seed: &[u8; 32], gateway: &ActorDelegationVerifier) -> anyhow::Result<Self> {
        let signer = ActorDelegationSigner::from_seed(Workload::Profile, seed)?;
        distinct(&signer.public_key(), gateway)?;
        Ok(Self(signer))
    }
    pub fn from_env(gateway: &ActorDelegationVerifier) -> anyhow::Result<Self> {
        let value = std::env::var(SIGNING_SEED_ENV)
            .map_err(|_| anyhow::anyhow!("{SIGNING_SEED_ENV} is required"))?;
        let seed = super::decode_key::<32>(SIGNING_SEED_ENV, &value)?;
        Self::from_seed(&seed, gateway)
    }
    pub fn public_key(&self) -> [u8; 32] {
        self.0.public_key()
    }
    pub fn client<S>(&self, inner: S) -> ActorDelegationClient<S> {
        ActorDelegationClient::new(inner, self.0.clone(), Workload::Concert)
    }
}
impl ParticipantDecisionVerifier {
    pub fn new(keys: &[[u8; 32]], gateway: &ActorDelegationVerifier) -> anyhow::Result<Self> {
        anyhow::ensure!(
            gateway.audience() == Workload::Concert,
            "Decision trust requires Concert audience"
        );
        for key in keys {
            distinct(key, gateway)?;
        }
        Ok(Self(ActorDelegationVerifier::new(Workload::Concert, keys)?))
    }
    pub fn configured_from_env(gateway: &ActorDelegationVerifier) -> anyhow::Result<Option<Self>> {
        let value = match std::env::var(VERIFYING_KEYS_ENV) {
            Ok(value) => value,
            Err(std::env::VarError::NotPresent) => return Ok(None),
            Err(_) => anyhow::bail!("Invalid decision key configuration"),
        };
        let keys = value
            .split(',')
            .map(|key| super::decode_key::<32>(VERIFYING_KEYS_ENV, key.trim()))
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(Some(Self::new(&keys, gateway)?))
    }
    pub fn into_inner(self) -> ActorDelegationVerifier {
        self.0
    }
}
