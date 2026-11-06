//! Explicit venue producer composition over the shared canonical owners. The
//! policy is operator-supplied; absence is unavailable, never a default.
use crate::{
    config::{canonical_runtime::CanonicalOwnerComposition, product_runtime::ProductOwners},
    product_rpc::{
        proto,
        scoped::ScopedProductOwner,
        venue_product::{VenuePolicyConfig, VenueProductProducer},
        Operation, ProductOwner, VerifiedProduct,
    },
};
use std::sync::Arc;

pub struct VenueRuntimeSettings {
    config: Arc<VenuePolicyConfig>,
}

impl VenueRuntimeSettings {
    pub fn from_lookup(
        mut read: impl FnMut(&str) -> anyhow::Result<Option<String>>,
        grpc: bool,
        shared_canonical: bool,
    ) -> anyhow::Result<Option<Self>> {
        match read("CONCERT_VENUE_MODE")?.as_deref() {
            None | Some("off") => return Ok(None),
            Some("live") => (),
            _ => anyhow::bail!("CONCERT_VENUE_MODE must be off or live"),
        }
        anyhow::ensure!(
            grpc && shared_canonical,
            "Live venue requires guarded gRPC and the shared canonical composition"
        );
        let raw = read("CONCERT_VENUE_POLICY_JSON")?
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!("CONCERT_VENUE_POLICY_JSON is required for live venue")
            })?;
        anyhow::ensure!(
            raw.len() <= 16 * 1024,
            "Venue policy configuration is too large"
        );
        let config: VenuePolicyConfig = serde_json::from_str(&raw)
            .map_err(|_| anyhow::anyhow!("Venue policy requires closed JSON"))?;
        config
            .validate()
            .map_err(|reason| anyhow::anyhow!("{reason}"))?;
        Ok(Some(Self {
            config: Arc::new(config),
        }))
    }
    pub fn load(grpc: bool, shared_canonical: bool) -> anyhow::Result<Option<Self>> {
        Self::from_lookup(
            |name| match std::env::var(name) {
                Ok(value) => Ok(Some(value)),
                Err(std::env::VarError::NotPresent) => Ok(None),
                Err(_) => anyhow::bail!("Invalid Venue configuration encoding"),
            },
            grpc,
            shared_canonical,
        )
    }
    pub fn compose(
        self,
        pool: sqlx::PgPool,
        owners: &CanonicalOwnerComposition,
        base: ProductOwners,
    ) -> anyhow::Result<ProductOwners> {
        let producer = Arc::new(VenueProductProducer::new(
            pool,
            self.config,
            owners.public_profile.clone(),
            owners.relationship.clone(),
        ));
        let delivery = owners.core.delivery();
        let venue = Arc::new(ScopedProductOwner::new(
            owners.fence.clone(),
            delivery.clone(),
            producer,
        ));
        Ok(ProductOwners {
            product: Arc::new(VenueOverlay {
                base: base.product,
                venue,
            }),
            delivery,
        })
    }
}

struct VenueOverlay {
    base: Arc<dyn ProductOwner>,
    venue: Arc<dyn ProductOwner>,
}

#[async_trait::async_trait]
impl ProductOwner for VenueOverlay {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        request: proto::ProductRequest,
    ) -> Result<tonic::Response<proto::ProductResponse>, tonic::Status> {
        match context.operation() {
            Operation::Venue(_) | Operation::Meeting(_) => {
                self.venue.dispatch(context, request).await
            }
            _ => self.base.dispatch(context, request).await,
        }
    }
}
