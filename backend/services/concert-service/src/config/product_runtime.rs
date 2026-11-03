//! Explicit independent product modes; a requested mode requires real producers.
use crate::product_rpc::ProductOwner;
use std::sync::Arc;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductSettings {
    pub public_feed_read: bool,
    pub owner_media: bool,
}

/// Production constructor, deliberately without a successful fallback. Owner
/// adapters are attached here only after their canonical fences are available.
/// An explicit unsupported opt-in fails before database/listener composition.
pub fn production_owners(
    settings: ProductSettings,
) -> anyhow::Result<Option<Arc<dyn ProductOwner>>> {
    anyhow::ensure!(
        !settings.public_feed_read,
        "Concert public Feed requires qualified canonical read fences and public-author projection"
    );
    anyhow::ensure!(
        !settings.owner_media,
        "Concert uploader media requires qualified canonical account/session fencing"
    );
    Ok(None)
}
impl ProductSettings {
    pub fn from_values(
        feed: Option<&str>,
        media: Option<&str>,
        grpc_enabled: bool,
    ) -> anyhow::Result<Self> {
        let public_feed_read = match feed {
            None | Some("off") => false,
            Some("read") => true,
            _ => anyhow::bail!("CONCERT_PUBLIC_FEED_MODE must be off or read"),
        };
        let owner_media = match media {
            None | Some("off") => false,
            Some("uploader") => true,
            _ => anyhow::bail!("CONCERT_OWNER_MEDIA_MODE must be off or uploader"),
        };
        anyhow::ensure!(
            grpc_enabled || !(public_feed_read || owner_media),
            "Concert product modes require guarded gRPC bootstrap composition"
        );
        Ok(Self {
            public_feed_read,
            owner_media,
        })
    }
    pub fn load(grpc_enabled: bool) -> anyhow::Result<Self> {
        let read = |key| match std::env::var(key) {
            Ok(value) => Ok(Some(value)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(_) => Err(anyhow::anyhow!("Invalid product mode encoding")),
        };
        Self::from_values(
            read("CONCERT_PUBLIC_FEED_MODE")?.as_deref(),
            read("CONCERT_OWNER_MEDIA_MODE")?.as_deref(),
            grpc_enabled,
        )
    }
}
