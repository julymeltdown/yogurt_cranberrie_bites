//! Closed dispatch to the actual public and Gift prepared-read owners.
use super::scope_ledger::ChildIntentSettler;
use crate::adapters::out::{
    public_profile::PublicProfileReader, relationship_authority::guarded::ChildRecoveryReference,
};
use std::sync::Arc;
use tonic::Status;

/// Implemented by the real Gift private native-read adapter. A caller-selected
/// status, arbitrary GID/SQL, missing memory or expiry cannot prove rollback.
#[async_trait::async_trait]
pub trait GiftPrivateIntentSettler: Send + Sync {
    async fn recover_gift_private(&self, reference: &ChildRecoveryReference) -> Result<(), Status>;
}
pub struct CompositeChildIntentSettler {
    public: Arc<PublicProfileReader>,
    gift: Arc<dyn GiftPrivateIntentSettler>,
}
impl CompositeChildIntentSettler {
    pub fn new(public: Arc<PublicProfileReader>, gift: Arc<dyn GiftPrivateIntentSettler>) -> Self {
        Self { public, gift }
    }
}
#[async_trait::async_trait]
impl ChildIntentSettler for CompositeChildIntentSettler {
    async fn recover_one(&self, reference: &ChildRecoveryReference) -> Result<(), Status> {
        match reference {
            ChildRecoveryReference::ProfilePublic { .. }
            | ChildRecoveryReference::LocalPublic { .. } => {
                self.public.recover_one(reference).await
            }
            ChildRecoveryReference::GiftPrivate { .. } => {
                self.gift.recover_gift_private(reference).await
            }
        }
    }
}
// Feed-only compositions may use the actual public reader directly. The owned
// public reader explicitly refuses Gift references instead of impersonating its
// local avatar child; the complete Gift composition requires the composite.
#[async_trait::async_trait]
impl ChildIntentSettler for PublicProfileReader {
    async fn recover_one(&self, reference: &ChildRecoveryReference) -> Result<(), Status> {
        PublicProfileReader::recover_one(self, reference).await
    }
}
