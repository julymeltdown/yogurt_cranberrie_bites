//! Explicit production composition for the closed PrivateData categories.
//!
//! This factory never substitutes a fixture or turns a missing provider into a
//! terminal receipt. Only the already-published canonical Auth authority and
//! actual provider factories may contribute entries to the closed registry.
use crate::{
    account_private_data::{
        CorePrivateDataOwner, PrivateDataAggregateCoordinator, PrivateDataCategory,
        PrivateDataCategoryOwner, PrivateDataCategoryOwners, PrivateDataCoreDeletion,
    },
    adapters::out::{
        age_provider::account_deletion::AgeAccountDeletion,
        media::account_private_data::MediaPrivateDataOwner,
    },
    intro_live::account_deletion::{
        IntroAccountDeletion, IntroConsentDeletionEvidence, PgIntroConsentDeletionEvidence,
    },
    meeting::account_deletion::MeetingPrivateInputDeletion,
    mural_tools_b::deletion::MuralPrivateDataOwner,
    party_live::account_deletion::PartyAndGiftInputDeletion,
    world::account_deletion::DeletionAuthority,
};
use sqlx::PgPool;
use std::sync::Arc;

/// All inputs use the existing Concert serving pool and the one canonical Auth
/// deletion authority. Provider construction stays in a dedicated leaf so the
/// serving runtime cannot accidentally select a test owner.
pub struct PrivateDataRuntimeOwners {
    pub categories: PrivateDataCategoryOwners,
}

impl PrivateDataRuntimeOwners {
    /// Compose the actual factory entries available on this source revision.
    /// Every append rejects a duplicate; an absent category remains durable
    /// INCOMPLETE and cannot turn into a positive aggregate receipt.
    pub fn production(pool: PgPool, authority: Arc<dyn DeletionAuthority>) -> Self {
        let core: Arc<dyn PrivateDataCategoryOwner> = Arc::new(CorePrivateDataOwner::new(
            PrivateDataCoreDeletion::new(pool.clone(), authority.clone()),
        ));
        let mut categories = PrivateDataCategoryOwners::core_only(core);
        categories = categories
            .add_owners([(
                PrivateDataCategory::AgeClaims,
                Arc::new(AgeAccountDeletion::new(pool.clone(), authority.clone()))
                    as Arc<dyn PrivateDataCategoryOwner>,
            )])
            .expect("age category is disjoint from core");
        categories = categories
            .add_owners([(
                PrivateDataCategory::MuralPrivateMetadata,
                Arc::new(MuralPrivateDataOwner::new(pool.clone(), authority.clone()))
                    as Arc<dyn PrivateDataCategoryOwner>,
            )])
            .expect("mural category is disjoint from prior categories");
        categories = categories
            .add_owners([(
                PrivateDataCategory::MediaAndMeetingPhotos,
                Arc::new(MediaPrivateDataOwner::new(pool.clone(), authority.clone()))
                    as Arc<dyn PrivateDataCategoryOwner>,
            )])
            .expect("media category is disjoint from prior categories");
        categories = categories
            .add_owners([(
                PrivateDataCategory::PartyAndGiftInput,
                Arc::new(PartyAndGiftInputDeletion::new(
                    pool.clone(),
                    authority.clone(),
                )) as Arc<dyn PrivateDataCategoryOwner>,
            )])
            .expect("party/gift category is disjoint from prior categories");
        categories = categories
            .add_owners([(
                PrivateDataCategory::BeaconsAndRally,
                Arc::new(MeetingPrivateInputDeletion::new(
                    pool.clone(),
                    authority.clone(),
                )) as Arc<dyn PrivateDataCategoryOwner>,
            )])
            .expect("meeting category is disjoint from prior categories");
        let consent: Arc<dyn IntroConsentDeletionEvidence> =
            Arc::new(PgIntroConsentDeletionEvidence);
        let intro = IntroAccountDeletion::new(pool, authority, consent).category_owners();
        categories = categories
            .add_owners([(
                PrivateDataCategory::IntroText,
                intro
                    .get(PrivateDataCategory::IntroText)
                    .expect("Intro factory supplies its exact category")
                    .clone(),
            )])
            .expect("intro category is disjoint from prior categories");
        Self { categories }
    }
    /// Observable registry membership for production readiness and tests. This
    /// exposes only the closed category names, never owner handles or authority.
    pub fn registered_categories(&self) -> Vec<PrivateDataCategory> {
        PrivateDataCategory::ALL
            .into_iter()
            .filter(|category| self.categories.get(*category).is_some())
            .collect()
    }
    pub fn is_complete(&self) -> bool {
        self.registered_categories().len() == PrivateDataCategory::ALL.len()
    }
}

/// Deterministic readiness shape used by the production packet. It reports
/// category names and completeness, never category receipts or authority data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivateDataReadiness {
    pub registered_categories: Vec<String>,
    pub complete: bool,
}
impl PrivateDataRuntimeOwners {
    pub fn readiness(&self) -> PrivateDataReadiness {
        let registered_categories = self
            .registered_categories()
            .into_iter()
            .map(|category| category.as_str().to_owned())
            .collect::<Vec<_>>();
        PrivateDataReadiness {
            complete: registered_categories.len() == PrivateDataCategory::ALL.len(),
            registered_categories,
        }
    }
}

/// Construct the coordinator with the explicit production registry. The
/// authority remains sourced from Auth; this layer never creates a portable
/// source proof from a database row.
pub fn production_coordinator(
    pool: PgPool,
    authority: Arc<dyn DeletionAuthority>,
) -> PrivateDataAggregateCoordinator {
    let owners = PrivateDataRuntimeOwners::production(pool.clone(), authority.clone());
    PrivateDataAggregateCoordinator::new(pool, authority, owners.categories)
}
