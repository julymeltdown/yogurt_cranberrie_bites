//! Closed production registration. Missing domain providers remain INCOMPLETE.
//! The real core owner is retained exactly once; no fixture can be substituted
//! by the main composition path and this module never writes category receipts.
use crate::{
    account_private_data::{
        PrivateDataAggregateCoordinator, PrivateDataCategory, PrivateDataCategoryOwner,
        PrivateDataCategoryOwners,
    },
    world::account_deletion::{DeletionAuthority, DeletionFailure},
};
use sqlx::PgPool;
use std::sync::Arc;

pub struct PrivateDataComposition {
    domains: Vec<(PrivateDataCategory, Arc<dyn PrivateDataCategoryOwner>)>,
}
impl PrivateDataComposition {
    /// Explicit source state while actual domain-provider inputs are arriving.
    /// This is one real core category, never seven-category availability.
    pub fn awaiting_domain_providers() -> Self {
        Self {
            domains: Vec::new(),
        }
    }
    pub fn register(
        mut self,
        category: PrivateDataCategory,
        owner: Arc<dyn PrivateDataCategoryOwner>,
    ) -> Result<Self, DeletionFailure> {
        if category == PrivateDataCategory::CorePreferencesAndAvatar
            || self.domains.iter().any(|(current, _)| *current == category)
        {
            return Err(DeletionFailure::InvalidCommand);
        }
        self.domains.push((category, owner));
        Ok(self)
    }
    pub fn configured_categories(&self) -> usize {
        1 + self.domains.len()
    }
    pub fn compose(
        self,
        pool: PgPool,
        authority: Arc<dyn DeletionAuthority>,
    ) -> anyhow::Result<Arc<PrivateDataAggregateCoordinator>> {
        let owners = PrivateDataCategoryOwners::production(pool.clone(), authority.clone())
            .add_owners(self.domains)
            .map_err(|_| anyhow::anyhow!("PrivateData category registration is invalid"))?;
        Ok(Arc::new(PrivateDataAggregateCoordinator::new(
            pool, authority, owners,
        )))
    }
}
