//! Explicit accepted claim sources, independent of operational provider health.
use crate::age_verification::{identifier, Error, ProviderConfig};
#[derive(Clone, Default)]
pub struct ClaimTrust {
    accepted: Vec<(String, String)>,
}
impl ClaimTrust {
    pub fn none() -> Self {
        Self::default()
    }
    pub fn new(sources: &[(&str, &str)]) -> Result<Self, Error> {
        if sources.len() > 16
            || sources
                .iter()
                .any(|(v, r)| !identifier(v) || !identifier(r))
        {
            return Err(Error::Invalid);
        }
        let mut accepted = sources
            .iter()
            .map(|(v, r)| (v.to_string(), r.to_string()))
            .collect::<Vec<_>>();
        accepted.sort();
        accepted.dedup();
        Ok(Self { accepted })
    }
    pub fn accepts(&self, verifier: &str, revision: &str) -> bool {
        self.accepted
            .iter()
            .any(|(v, r)| v == verifier && r == revision)
    }
    pub(super) fn configured(config: &ProviderConfig) -> Self {
        Self {
            accepted: vec![(config.verifier.clone(), config.revision.clone())],
        }
    }
}
