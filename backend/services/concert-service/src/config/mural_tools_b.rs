//! Opt-in B composition retains exactly the incumbent core, fence and delivery.
use super::{canonical_runtime::CanonicalOwnerComposition, product_runtime::ProductOwners};
use crate::{
    mural_tools_b::MuralToolsRuntime,
    product_rpc::{proto, scoped::ScopedProductOwner, ProductOwner, VerifiedProduct},
};
use concert_application::commands::{FingerprintKeys, SigningKey};
use serde::Deserialize;
use std::{collections::BTreeSet, sync::Arc};
pub struct WallBSettings {
    keys: Arc<FingerprintKeys>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Key {
    id: String,
    secret_hex: String,
}
impl WallBSettings {
    pub fn from_values(
        mode: Option<&str>,
        raw: Option<&str>,
        shared: bool,
    ) -> anyhow::Result<Option<Self>> {
        match mode {
            None | Some("off") => return Ok(None),
            Some("live") => {}
            _ => anyhow::bail!("CONCERT_WALL_TOOLS_B_MODE must be off or live"),
        }
        anyhow::ensure!(
            shared,
            "Wall B requires the incumbent shared canonical composition"
        );
        let raw = raw
            .ok_or_else(|| anyhow::anyhow!("CONCERT_WALL_TOOLS_B_RECEIPT_KEYS_JSON is required"))?;
        anyhow::ensure!(raw.len() <= 16384, "Wall B key configuration is too large");
        let values: Vec<Key> =
            serde_json::from_str(raw).map_err(|_| anyhow::anyhow!("Invalid Wall B keys"))?;
        anyhow::ensure!(
            (1..=32).contains(&values.len()),
            "Wall B requires one active key and at most31 retained keys"
        );
        let mut ids = BTreeSet::new();
        let mut secrets = BTreeSet::new();
        let mut keys = Vec::new();
        for k in values {
            anyhow::ensure!(
                k.secret_hex.len() == 64
                    && k.secret_hex
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "Invalid Wall B key encoding"
            );
            let mut secret = [0u8; 32];
            for (i, byte) in secret.iter_mut().enumerate() {
                *byte = u8::from_str_radix(&k.secret_hex[i * 2..i * 2 + 2], 16)?;
            }
            anyhow::ensure!(
                secret != [0; 32] && ids.insert(k.id.clone()) && secrets.insert(secret),
                "Wall B keys must be unique and nonzero"
            );
            keys.push(SigningKey::new(&k.id, &secret)?);
        }
        let active = keys.remove(0);
        Ok(Some(Self {
            keys: Arc::new(FingerprintKeys::new(active, keys)?),
        }))
    }
    pub fn load(shared: bool) -> anyhow::Result<Option<Self>> {
        let read = |key| match std::env::var(key) {
            Ok(v) => Ok(Some(v)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(_) => Err(anyhow::anyhow!("Invalid Wall B mode encoding")),
        };
        Self::from_values(
            read("CONCERT_WALL_TOOLS_B_MODE")?.as_deref(),
            read("CONCERT_WALL_TOOLS_B_RECEIPT_KEYS_JSON")?.as_deref(),
            shared,
        )
    }
    pub fn compose(
        self,
        pool: sqlx::PgPool,
        owners: &CanonicalOwnerComposition,
        base: ProductOwners,
    ) -> ProductOwners {
        let delivery = owners.core.delivery();
        ProductOwners {
            product: Arc::new(WallProductOwner {
                base: base.product,
                wall: ScopedProductOwner::new(
                    owners.fence.clone(),
                    delivery.clone(),
                    Arc::new(MuralToolsRuntime::new(pool, self.keys)),
                ),
            }),
            delivery,
        }
    }
}
struct WallProductOwner {
    base: Arc<dyn ProductOwner>,
    wall: ScopedProductOwner,
}
#[async_trait::async_trait]
impl ProductOwner for WallProductOwner {
    async fn dispatch(
        &self,
        c: &VerifiedProduct,
        r: proto::ProductRequest,
    ) -> Result<tonic::Response<proto::ProductResponse>, tonic::Status> {
        match c.operation() {
            crate::product_rpc::Operation::Wall(_) => self.wall.dispatch(c, r).await,
            crate::product_rpc::Operation::CommandStatus
                if c.command_query().is_some_and(|q| {
                    matches!(
                        q.operation.id(),
                        "placeConcertPixel"
                            | "eraseConcertMuralPixel"
                            | "fillConcertMuralRegion"
                            | "undoConcertMuralAction"
                    )
                }) =>
            {
                self.wall.dispatch(c, r).await
            }
            _ => self.base.dispatch(c, r).await,
        }
    }
}
