//! Qualified Feed reads over the existing shared canonical owners and listener.
use crate::config::{canonical_runtime::CanonicalOwnerComposition, product_runtime::ProductOwners};
use crate::{
    adapters::out::SystemClock,
    feed::product::{FeedProductConfig, FeedProductProducer},
    product_rpc::{
        proto, root_read::ConcertRootReadService, scoped::ScopedProductOwner, ProductOwner,
        VerifiedProduct,
    },
};
use platform::{
    actor_delegation::{
        concert_read::{ConcertPostReadSigner, PostConcertRootReadVerifier},
        ActorDelegationVerifier,
    },
    grpc_auth::{CallerRegistry, ServiceIdentityClientInterceptor, Workload},
    grpc_tls::TlsMaterial,
};
use std::sync::Arc;

// Private configured secrets deliberately have no Debug or serialization.
pub struct FeedRuntimeSettings {
    config: FeedProductConfig,
    tls: TlsMaterial,
    signer: ConcertPostReadSigner,
    callback_verifier: ActorDelegationVerifier,
    callers: CallerRegistry,
}
fn required(
    read: &mut impl FnMut(&str) -> anyhow::Result<Option<String>>,
    key: &str,
) -> anyhow::Result<String> {
    read(key)?
        .filter(|v| !v.is_empty())
        .ok_or_else(|| anyhow::anyhow!("{key} is required for Feed reads"))
}
fn key32(value: &str) -> anyhow::Result<[u8; 32]> {
    anyhow::ensure!(
        value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "Feed keys require 32-byte lowercase hexadecimal values"
    );
    let mut key = [0; 32];
    for (out, pair) in key.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
        let digit = |b| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
        *out = digit(pair[0]) * 16 + digit(pair[1]);
    }
    anyhow::ensure!(key != [0; 32], "Feed keys must be nonzero");
    Ok(key)
}
pub struct FeedReadKeys {
    signer: ConcertPostReadSigner,
    callback: ActorDelegationVerifier,
}
impl FeedReadKeys {
    pub fn new(signer: ConcertPostReadSigner, callback: PostConcertRootReadVerifier) -> Self {
        Self {
            signer,
            callback: callback.into_inner(),
        }
    }
}

impl FeedRuntimeSettings {
    pub fn from_values(
        mut read: impl FnMut(&str) -> anyhow::Result<Option<String>>,
        enabled: bool,
        shared_intro: bool,
        tls: Option<&TlsMaterial>,
        gateway: Option<&ActorDelegationVerifier>,
        keys: Option<FeedReadKeys>,
    ) -> anyhow::Result<Option<Self>> {
        if !enabled {
            return Ok(None);
        }
        anyhow::ensure!(
            shared_intro,
            "Feed reads require the shared Intro/Participant canonical composition"
        );
        let tls = tls
            .ok_or_else(|| anyhow::anyhow!("Feed reads require actual mutual TLS"))?
            .clone();
        tonic::transport::Server::builder().tls_config(tls.server_config()?)?;
        let gateway = gateway
            .ok_or_else(|| anyhow::anyhow!("Feed reads require the actual Gateway verifier"))?;
        let endpoint = required(&mut read, "CONCERT_POST_GRPC_ENDPOINT")?;
        tls.endpoint(&endpoint, Workload::Post)?;
        let concert_name = platform::config::service_credential_env_name(Workload::Concert);
        let concert = platform::config::resolve_service_credential(
            Some(required(&mut read, &concert_name)?),
            &concert_name,
        )?
        .ok_or_else(|| anyhow::anyhow!("Explicit Concert credential required"))?;
        let post_name = platform::config::service_credential_env_name(Workload::Post);
        let post = platform::config::resolve_service_credential(
            Some(required(&mut read, &post_name)?),
            &post_name,
        )?
        .ok_or_else(|| anyhow::anyhow!("Explicit Post credential required"))?;
        let previous_name = platform::config::previous_service_credential_env_name(Workload::Post);
        let previous =
            platform::config::resolve_service_credential(read(&previous_name)?, &previous_name)?;
        ServiceIdentityClientInterceptor::new(Workload::Concert, &concert)?;
        ServiceIdentityClientInterceptor::new(Workload::Post, &post)?;
        anyhow::ensure!(
            concert != post
                && previous
                    .as_ref()
                    .is_none_or(|v| v != &post && v != &concert),
            "Feed caller credentials must be distinct"
        );
        // The callback's dedicated registry must not reinterpret another
        // configured workload's current or retained credential as Post.
        for role in Workload::ALL {
            if role == Workload::Post {
                continue;
            }
            for name in [
                platform::config::service_credential_env_name(role),
                platform::config::previous_service_credential_env_name(role),
            ] {
                if let Some(token) =
                    platform::config::resolve_service_credential(read(&name)?, &name)?
                {
                    anyhow::ensure!(
                        token != post && previous.as_ref().is_none_or(|old| old != &token),
                        "Feed Post credentials must differ from other workload credentials"
                    );
                }
            }
        }
        let callers = CallerRegistry::new().register_with_previous(
            Workload::Post,
            &post,
            previous.as_deref(),
        )?;
        let keys = keys.ok_or_else(|| {
            anyhow::anyhow!("Feed read signing and callback verification keys are required")
        })?;
        let cursor = key32(&required(&mut read, "CONCERT_FEED_CURSOR_KEY_HEX")?)?;
        let cursor_key = ConcertPostReadSigner::from_seed(&cursor, gateway)?.public_key();
        anyhow::ensure!(
            cursor_key != keys.signer.public_key(),
            "Feed signing and cursor secrets must be independent"
        );
        anyhow::ensure!(
            !keys.callback.trusts(&keys.signer.public_key()),
            "Feed callback keys must be independent of outbound keys"
        );
        Ok(Some(Self {
            config: FeedProductConfig::new(&endpoint, &concert, &cursor)?,
            tls,
            signer: keys.signer,
            callback_verifier: keys.callback,
            callers,
        }))
    }
    pub fn load(
        enabled: bool,
        shared_intro: bool,
        tls: Option<&TlsMaterial>,
        gateway: Option<&ActorDelegationVerifier>,
    ) -> anyhow::Result<Option<Self>> {
        if !enabled {
            return Ok(None);
        }
        let gateway_keys =
            gateway.ok_or_else(|| anyhow::anyhow!("Feed reads require actual Gateway keys"))?;
        // Preserve the established standard-Base64 environment key format.
        let keys = FeedReadKeys::new(
            ConcertPostReadSigner::from_env(gateway_keys)?,
            PostConcertRootReadVerifier::configured_from_env(gateway_keys)?.ok_or_else(|| {
                anyhow::anyhow!("POST_CONCERT_ROOT_READ_VERIFYING_KEYS is required")
            })?,
        );
        Self::from_values(
            |name| match std::env::var(name) {
                Ok(value) => Ok(Some(value)),
                Err(std::env::VarError::NotPresent) => Ok(None),
                Err(_) => anyhow::bail!("{name} must contain valid Unicode"),
            },
            enabled,
            shared_intro,
            tls,
            gateway,
            Some(keys),
        )
    }
    /// No second scope registry, ledger, delivery publisher or pool is created.
    pub fn compose(
        self,
        owners: &CanonicalOwnerComposition,
        base: ProductOwners,
    ) -> anyhow::Result<(ProductOwners, FeedCallback)> {
        let producer = FeedProductProducer::new(
            self.config,
            self.tls,
            self.signer,
            owners.public_profile.clone(),
            SystemClock,
        )?;
        let delivery = owners.core.delivery();
        let feed =
            ScopedProductOwner::new(owners.fence.clone(), delivery.clone(), Arc::new(producer));
        let callback = FeedCallback {
            service: ConcertRootReadService::new(
                owners.core.registry(),
                owners.public_profile.clone(),
            ),
            callers: self.callers,
            verifier: self.callback_verifier,
        };
        Ok((
            ProductOwners {
                product: Arc::new(FeedProductOwner {
                    feed,
                    base: base.product,
                }),
                delivery,
            },
            callback,
        ))
    }
}
pub struct FeedCallback {
    pub service: ConcertRootReadService,
    pub callers: CallerRegistry,
    pub verifier: ActorDelegationVerifier,
}
struct FeedProductOwner {
    feed: ScopedProductOwner,
    base: Arc<dyn ProductOwner>,
}
#[async_trait::async_trait]
impl ProductOwner for FeedProductOwner {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        request: proto::ProductRequest,
    ) -> Result<tonic::Response<proto::ProductResponse>, tonic::Status> {
        if FeedProductProducer::<SystemClock>::handles(context.operation()) {
            self.feed.dispatch(context, request).await
        } else {
            self.base.dispatch(context, request).await
        }
    }
}
