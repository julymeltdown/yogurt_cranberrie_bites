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
#[async_trait::async_trait]
pub trait SocialReadIntentSettler: Send + Sync {
    async fn recover_social_read(&self, reference: &ChildRecoveryReference) -> Result<(), Status>;
}
#[async_trait::async_trait]
pub trait NotificationInboxIntentSettler: Send + Sync {
    async fn recover_notification_inbox(
        &self,
        reference: &ChildRecoveryReference,
    ) -> Result<(), Status>;
}
pub struct CompositeChildIntentSettler {
    public: Arc<PublicProfileReader>,
    gift: Arc<dyn GiftPrivateIntentSettler>,
    social: Option<Arc<dyn SocialReadIntentSettler>>,
    notification: Option<Arc<dyn NotificationInboxIntentSettler>>,
}
impl CompositeChildIntentSettler {
    pub fn new(public: Arc<PublicProfileReader>, gift: Arc<dyn GiftPrivateIntentSettler>) -> Self {
        Self {
            public,
            gift,
            social: None,
            notification: None,
        }
    }
    pub fn with_notification(
        mut self,
        notification: Arc<dyn NotificationInboxIntentSettler>,
    ) -> Self {
        self.notification = Some(notification);
        self
    }
    pub fn with_social(mut self, social: Arc<dyn SocialReadIntentSettler>) -> Self {
        self.social = Some(social);
        self
    }
}
#[async_trait::async_trait]
impl ChildIntentSettler for CompositeChildIntentSettler {
    async fn recover_one(&self, reference: &ChildRecoveryReference) -> Result<(), Status> {
        match reference {
            ChildRecoveryReference::NotificationInbox { .. } => {
                self.notification
                    .as_ref()
                    .ok_or_else(|| {
                        Status::failed_precondition("Notification inbox recovery is unavailable")
                    })?
                    .recover_notification_inbox(reference)
                    .await
            }

            ChildRecoveryReference::ProfilePublic { .. }
            | ChildRecoveryReference::LocalPublic { .. } => {
                self.public.recover_one(reference).await
            }
            ChildRecoveryReference::GiftPrivate { .. } => {
                self.gift.recover_gift_private(reference).await
            }
            ChildRecoveryReference::SocialRead { .. } => {
                self.social
                    .as_ref()
                    .ok_or_else(|| {
                        Status::failed_precondition("Social read recovery is unavailable")
                    })?
                    .recover_social_read(reference)
                    .await
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
