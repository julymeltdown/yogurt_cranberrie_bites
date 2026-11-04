//! Social composition from Root's existing canonical fence and delivery context.
//! No environment/default keys, alternate registry, listener or claim is created.
use crate::{
    adapters::out::relationship_authority::guarded::CanonicalOwnerFence,
    product_rpc::{delivery::DeliveryPublisher, scoped::ScopedProductOwner},
    social_live::{
        keys::ManagedSocialKeys, provenance::SocialClaimProvenance, read_child::SocialReadSettler,
        witness::SocialNativeResolver,
    },
};
use sqlx::PgPool;
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;

pub use crate::social_live::SocialRuntime as ScopedSocialProducer;

/// Already validated managed key material and an immutable accepted issuer
/// snapshot. Missing trust disables sharing, not metadata/safety or basic World.
/// Replacing trust requires Root to drain all old held scopes first.
pub struct SocialRuntimeConfig {
    keys: Arc<ManagedSocialKeys>,
    provenance: Arc<SocialClaimProvenance>,
    ttl_seconds: i64,
}
impl SocialRuntimeConfig {
    pub fn new(
        keys: ManagedSocialKeys,
        accepted_claim: Option<(&str, &str)>,
        ttl_seconds: i64,
    ) -> Result<Self, Status> {
        if !(1..=86400).contains(&ttl_seconds) {
            return Err(invalid());
        }
        let provenance = match accepted_claim {
            Some((issuer, revision)) => {
                SocialClaimProvenance::configured(issuer.into(), revision.into())
                    .map_err(|_| invalid())?
            }
            None => SocialClaimProvenance::Unavailable,
        };
        Ok(Self {
            keys: Arc::new(keys),
            provenance: Arc::new(provenance),
            ttl_seconds,
        })
    }

    /// `instance` must be Root's existing registry/native producer instance.
    /// `fence` and `delivery` must use that same registry/lifecycle. Actual
    /// delivery verifies scope Arc identity; mismatches never grant authority.
    /// Root installs `native` in its closed resolver union and `read_settler`
    /// through CompositeChildIntentSettler::with_social before serving.
    pub fn compose(
        self,
        pool: PgPool,
        instance: Uuid,
        fence: Arc<CanonicalOwnerFence>,
        delivery: DeliveryPublisher,
    ) -> Result<SocialRuntimeOwners, Status> {
        let native = Arc::new(SocialNativeResolver::new(pool.clone(), instance)?);
        let read_settler = Arc::new(SocialReadSettler::new(pool.clone()));
        let producer = Arc::new(ScopedSocialProducer::new(
            pool,
            self.keys,
            self.provenance,
            self.ttl_seconds,
        )?);
        let owner = Arc::new(ScopedProductOwner::new(fence, delivery, producer.clone()));
        Ok(SocialRuntimeOwners {
            producer,
            owner,
            native,
            read_settler,
        })
    }

    /// Attach only Social dispatch to the already composed canonical core. Its
    /// Social read settler and native resolver were installed before serving;
    /// do not create another registry, ledger, scope or delivery publisher here.
    pub fn compose_shared(
        self,
        pool: PgPool,
        owners: &super::canonical_runtime::CanonicalOwnerComposition,
        base: super::product_runtime::ProductOwners,
    ) -> Result<super::product_runtime::ProductOwners, Status> {
        let producer = Arc::new(ScopedSocialProducer::new(
            pool,
            self.keys,
            self.provenance,
            self.ttl_seconds,
        )?);
        let delivery = owners.core.delivery();
        Ok(super::product_runtime::ProductOwners {
            product: Arc::new(SharedSocialOwner {
                base: base.product,
                social: ScopedProductOwner::new(owners.fence.clone(), delivery.clone(), producer),
            }),
            delivery,
        })
    }
}

struct SharedSocialOwner {
    base: Arc<dyn crate::product_rpc::ProductOwner>,
    social: ScopedProductOwner,
}
#[async_trait::async_trait]
impl crate::product_rpc::ProductOwner for SharedSocialOwner {
    async fn dispatch(
        &self,
        context: &crate::product_rpc::VerifiedProduct,
        request: crate::product_rpc::proto::ProductRequest,
    ) -> Result<tonic::Response<crate::product_rpc::proto::ProductResponse>, Status> {
        use crate::product_rpc::ProductOwner;
        match context.operation() {
            crate::product_rpc::Operation::Social(_) => {
                self.social.dispatch(context, request).await
            }
            _ => self.base.dispatch(context, request).await,
        }
    }
}

/// These are real adapters, not an activated capability or settlement proof.
/// Root retains single-writer ownership of dispatch/native singleton unions.
pub struct SocialRuntimeOwners {
    pub producer: Arc<ScopedSocialProducer>,
    pub owner: Arc<ScopedProductOwner>,
    pub native: Arc<SocialNativeResolver>,
    pub read_settler: Arc<SocialReadSettler>,
}
fn invalid() -> Status {
    Status::invalid_argument("Invalid managed Social configuration")
}
