//! Explicit inbox activation over the existing shared canonical composition.
//! No target policy, workload identity or successful fallback is created here.
use super::{canonical_runtime::CanonicalOwnerComposition, product_runtime::ProductOwners};
use crate::{
    notification_live::NotificationRuntime,
    product_rpc::{proto, scoped::ScopedProductOwner, Operation, ProductOwner, VerifiedProduct},
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InboxRuntimeSettings {
    enabled: bool,
}
impl InboxRuntimeSettings {
    pub fn from_values(mode: Option<&str>, shared_intro: bool) -> anyhow::Result<Self> {
        let enabled = match mode {
            None | Some("off") => false,
            Some("live") => true,
            _ => anyhow::bail!("CONCERT_NOTIFICATION_INBOX_MODE must be off or live"),
        };
        anyhow::ensure!(
            !enabled || shared_intro,
            "Live Notification inbox requires the shared live Intro/Participant composition"
        );
        Ok(Self { enabled })
    }
    pub fn load(shared_intro: bool) -> anyhow::Result<Self> {
        let value = match std::env::var("CONCERT_NOTIFICATION_INBOX_MODE") {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(_) => anyhow::bail!("Invalid Notification inbox mode encoding"),
        };
        Self::from_values(value.as_deref(), shared_intro)
    }
    pub fn enabled(self) -> bool {
        self.enabled
    }
    /// Both branches retain exactly the supplied core/fence/delivery context.
    /// Notification's real client still requires the independently composed
    /// target authority and durable native inbox at the configured service.
    pub fn compose(self, owners: &CanonicalOwnerComposition, base: ProductOwners) -> ProductOwners {
        if !self.enabled {
            return base;
        }
        let delivery = owners.core.delivery();
        ProductOwners {
            product: Arc::new(InboxProductOwner {
                base: base.product,
                inbox: ScopedProductOwner::new(
                    owners.fence.clone(),
                    delivery.clone(),
                    Arc::new(NotificationRuntime::new(owners.notification.clone())),
                ),
            }),
            delivery,
        }
    }
}

struct InboxProductOwner {
    base: Arc<dyn ProductOwner>,
    inbox: ScopedProductOwner,
}
#[async_trait::async_trait]
impl ProductOwner for InboxProductOwner {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        request: proto::ProductRequest,
    ) -> Result<tonic::Response<proto::ProductResponse>, tonic::Status> {
        match context.operation() {
            Operation::Notification(_) => self.inbox.dispatch(context, request).await,
            _ => self.base.dispatch(context, request).await,
        }
    }
}
