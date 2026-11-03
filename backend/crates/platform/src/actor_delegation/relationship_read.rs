//! Dedicated trusted-Concert observation proofs. These are neither Gateway
//! child authority nor transaction fences. No environment/parity defaults.
use super::{ActorDelegationClient, ActorDelegationSigner, ActorDelegationVerifier};
use crate::grpc_auth::Workload;
use std::fmt;
pub const SIGNING_SEED_ENV: &str = "CONCERT_RELATIONSHIP_SIGNING_SEED";
pub const VERIFYING_KEYS_ENV: &str = "CONCERT_RELATIONSHIP_VERIFYING_KEYS";
#[derive(Clone)]
pub struct RelationshipReadSigner(ActorDelegationSigner);
#[derive(Clone)]
pub struct RelationshipReadVerifier(ActorDelegationVerifier);
impl fmt::Debug for RelationshipReadSigner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RelationshipReadSigner([REDACTED])")
    }
}
impl fmt::Debug for RelationshipReadVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RelationshipReadVerifier([REDACTED])")
    }
}
fn distinct(key: &[u8; 32], gateway: &ActorDelegationVerifier) -> anyhow::Result<()> {
    anyhow::ensure!(
        !gateway.trusts(key),
        "relationship read keys must be disjoint from Gateway verification keys"
    );
    anyhow::ensure!(
        *key != ActorDelegationSigner::local_parity(Workload::Gateway).public_key(),
        "relationship read keys cannot use the parity key"
    );
    Ok(())
}
impl RelationshipReadSigner {
    /// Explicit seed and independently configured Gateway trust, also used by fixtures.
    pub fn from_seed(seed: &[u8; 32], gateway: &ActorDelegationVerifier) -> anyhow::Result<Self> {
        let signer = ActorDelegationSigner::from_seed(Workload::Concert, seed)?;
        distinct(&signer.public_key(), gateway)?;
        Ok(Self(signer))
    }
    pub fn from_env(gateway: &ActorDelegationVerifier) -> anyhow::Result<Self> {
        Self::from_lookup(gateway, |name| std::env::var(name).ok())
    }
    fn from_lookup(
        gateway: &ActorDelegationVerifier,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> anyhow::Result<Self> {
        let value = lookup(SIGNING_SEED_ENV)
            .ok_or_else(|| anyhow::anyhow!("{SIGNING_SEED_ENV} is required"))?;
        let seed = super::decode_key::<32>(SIGNING_SEED_ENV, &value)?;
        let result = Self::from_seed(&seed, gateway)?;
        if let Some(value) = lookup(super::SIGNING_SEED_ENV) {
            let gateway_seed = super::decode_key::<32>(super::SIGNING_SEED_ENV, &value)?;
            anyhow::ensure!(
                seed != gateway_seed,
                "relationship read seed must differ from Gateway signing seed"
            );
        }
        Ok(result)
    }
    pub fn public_key(&self) -> [u8; 32] {
        self.0.public_key()
    }
    pub fn client<S>(
        &self,
        inner: S,
        audience: Workload,
    ) -> anyhow::Result<ActorDelegationClient<S>> {
        anyhow::ensure!(
            matches!(audience, Workload::Auth | Workload::Friend),
            "relationship reads require an owner audience"
        );
        Ok(ActorDelegationClient::new(inner, self.0.clone(), audience))
    }
}
impl RelationshipReadVerifier {
    pub fn new(
        audience: Workload,
        keys: &[[u8; 32]],
        gateway: &ActorDelegationVerifier,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            matches!(audience, Workload::Auth | Workload::Friend) && gateway.audience() == audience,
            "relationship read verifier requires matching owner audience"
        );
        for key in keys {
            distinct(key, gateway)?;
        }
        Ok(Self(ActorDelegationVerifier::new(audience, keys)?))
    }
    /// Missing dedicated keys disable the new read service. Empty or invalid
    /// configured keys fail startup; existing Gateway keyrings are never reused.
    pub fn configured_from_env(
        audience: Workload,
        gateway: &ActorDelegationVerifier,
    ) -> anyhow::Result<Option<Self>> {
        let value = match std::env::var(VERIFYING_KEYS_ENV) {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(_) => anyhow::bail!("invalid relationship verification key configuration"),
        };
        Self::from_lookup(audience, gateway, |_| value.clone())
    }
    fn from_lookup(
        audience: Workload,
        gateway: &ActorDelegationVerifier,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> anyhow::Result<Option<Self>> {
        let Some(value) = lookup(VERIFYING_KEYS_ENV) else {
            return Ok(None);
        };
        let keys = value
            .split(',')
            .map(|v| super::decode_key::<32>(VERIFYING_KEYS_ENV, v.trim()))
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(Some(Self::new(audience, &keys, gateway)?))
    }
    pub fn into_inner(self) -> ActorDelegationVerifier {
        self.0
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use base64ct::{Base64, Encoding};
    #[test]
    fn dedicated_keys_are_explicit_disjoint_and_owner_scoped() {
        let gateway_signer =
            ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32]).unwrap();
        let gateway =
            ActorDelegationVerifier::new(Workload::Auth, &[gateway_signer.public_key()]).unwrap();
        assert!(RelationshipReadSigner::from_seed(&[41; 32], &gateway).is_err());
        let signer = RelationshipReadSigner::from_seed(&[37; 32], &gateway).unwrap();
        assert!(
            RelationshipReadVerifier::new(Workload::Auth, &[signer.public_key()], &gateway).is_ok()
        );
        assert!(RelationshipReadVerifier::new(
            Workload::Auth,
            &[gateway_signer.public_key()],
            &gateway
        )
        .is_err());
        assert!(
            RelationshipReadVerifier::new(Workload::Friend, &[signer.public_key()], &gateway)
                .is_err()
        );
        assert!(RelationshipReadVerifier::new(Workload::Auth, &[], &gateway).is_err());
        let parity = ActorDelegationSigner::local_parity(Workload::Gateway).public_key();
        assert!(RelationshipReadVerifier::new(Workload::Auth, &[parity], &gateway).is_err());
        assert!(
            RelationshipReadVerifier::from_lookup(Workload::Auth, &gateway, |_| None)
                .unwrap()
                .is_none()
        );
        for value in [
            String::new(),
            "bad-key".into(),
            Base64::encode_string(&gateway_signer.public_key()),
        ] {
            assert!(
                RelationshipReadVerifier::from_lookup(Workload::Auth, &gateway, |_| Some(
                    value.clone()
                ))
                .is_err()
            );
        }
        let value = Base64::encode_string(&signer.public_key());
        assert!(
            RelationshipReadVerifier::from_lookup(Workload::Auth, &gateway, |_| Some(
                value.clone()
            ))
            .unwrap()
            .is_some()
        );
    }
    #[test]
    fn signing_environment_has_no_fallback_or_gateway_seed_reuse() {
        let old = ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32]).unwrap();
        let gateway = ActorDelegationVerifier::new(Workload::Concert, &[old.public_key()]).unwrap();
        assert!(RelationshipReadSigner::from_lookup(&gateway, |_| None).is_err());
        for value in [
            String::new(),
            "invalid".into(),
            Base64::encode_string(&[41; 32]),
        ] {
            assert!(RelationshipReadSigner::from_lookup(&gateway, |name| (name
                == SIGNING_SEED_ENV)
                .then(|| value.clone()))
            .is_err());
        }
        let value = Base64::encode_string(&[37; 32]);
        assert!(
            RelationshipReadSigner::from_lookup(&gateway, |name| (name == SIGNING_SEED_ENV)
                .then(|| value.clone()))
            .is_ok()
        );
        assert!(RelationshipReadSigner::from_lookup(&gateway, |_| Some(value.clone())).is_err());
    }
}
