//! Shared canonical scope composition for live owners and system recovery.
//!
//! Construct once for the configured producer instance, then pass clones to
//! feature composition. The caller supplies the actual typed child settler and
//! native resolver; this core grants no authority and starts no background task.
use crate::{
    adapters::out::{
        public_profile::PublicProfileReader,
        relationship_authority::{
            grpc::GrpcRelationshipAuthority,
            guarded::{CanonicalOwnerFence, ScopeRegistry},
        },
    },
    gift_live::private::GiftPrivateRuntime,
    notification_live::{target_authority::TargetReadSettler, NotificationClient},
    product_rpc::{
        child_recovery::{ChildRecoverySettings, ChildRecoveryWorker},
        child_settlement::CompositeChildIntentSettler,
        delivery::DeliveryPublisher,
        scope_ledger::{
            ChildIntentSettler, ChildRecoveryKeys, PgNotificationBindingReader, PgScopeLedger,
        },
        settlement::{NativeTransactionResolver, ProducerSettlementService},
    },
    social_live::read_child::SocialReadSettler,
};
use concert_application::{bootstrap::ValidatedProductRelease, community::giveaway::PickupKeys};
use platform::{
    actor_delegation::relationship_read::RelationshipReadSigner, grpc_tls::TlsMaterial,
};
use sqlx::PgPool;
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;

/// Actual configured owner endpoints. No endpoint is inferred or optional.
pub struct CanonicalOwnerEndpoints {
    pub auth: String,
    pub friend: String,
    pub profile: String,
    pub notification: String,
}

/// Validated key material and the producer identity are supplied by the caller.
/// Deliberately has no Debug implementation: it contains private credentials.
pub struct CanonicalOwnerConfig {
    pub instance: Uuid,
    pub child_keys: Arc<ChildRecoveryKeys>,
    pub pickup_keys: Arc<PickupKeys>,
    pub endpoints: CanonicalOwnerEndpoints,
    pub credential: String,
    pub tls: TlsMaterial,
    pub relationship_signer: RelationshipReadSigner,
}

/// Typed owners share one pool and one canonical core. This is composition,
/// not route activation, target-policy authority, or worker supervision.
/// ParticipantRuntimeSettings::compose_shared constructs this directly from its
/// validated private identity/key/endpoint origin and shares this exact core;
/// callers must not independently compose a second owner core for that shell.
pub struct CanonicalOwnerComposition {
    pub core: CanonicalRuntimeCore,
    pub fence: Arc<CanonicalOwnerFence>,
    pub public_profile: Arc<PublicProfileReader>,
    pub relationship: Arc<GrpcRelationshipAuthority>,
    pub gift_private: Arc<GiftPrivateRuntime>,
    pub social_read: Arc<SocialReadSettler>,
    pub notification: Arc<NotificationClient>,
}

impl CanonicalOwnerComposition {
    pub fn compose(
        pool: PgPool,
        release: Arc<ValidatedProductRelease>,
        config: CanonicalOwnerConfig,
    ) -> anyhow::Result<Self> {
        // This reader depends only on the supplied pool and instance. Construct
        // it before the client/settler/core to avoid a ledger ownership cycle.
        let notification_binding = Arc::new(PgNotificationBindingReader::new(
            pool.clone(),
            config.instance,
        )?);
        let public_profile = Arc::new(PublicProfileReader::connect(
            pool.clone(),
            release,
            &config.endpoints.profile,
            &config.credential,
            Some(&config.tls),
            config.relationship_signer.clone(),
        )?);
        let gift_private = Arc::new(GiftPrivateRuntime::new(pool.clone(), config.pickup_keys));
        let social_read = Arc::new(SocialReadSettler::new(pool.clone()));
        let relationship = Arc::new(GrpcRelationshipAuthority::connect(
            &config.endpoints.auth,
            &config.endpoints.friend,
            &config.credential,
            Some(&config.tls),
            config.relationship_signer.clone(),
        )?);
        let notification = Arc::new(NotificationClient::connect(
            &config.endpoints.notification,
            &config.credential,
            Some(&config.tls),
            Some(notification_binding),
        )?);
        let target_read = Arc::new(TargetReadSettler::new(pool.clone(), config.instance)?);
        let children = Arc::new(
            CompositeChildIntentSettler::new(public_profile.clone(), gift_private.clone())
                .with_social(social_read.clone())
                .with_notification(notification.clone())
                .with_target(target_read),
        );
        let core = CanonicalRuntimeCore::new(pool, config.instance, config.child_keys, children)?;
        let fence = Arc::new(core.owner_fence(
            &config.endpoints.auth,
            &config.endpoints.friend,
            &config.credential,
            &config.tls,
            config.relationship_signer,
        )?);
        Ok(Self {
            core,
            fence,
            public_profile,
            relationship,
            gift_private,
            social_read,
            notification,
        })
    }
}

#[derive(Clone)]
pub struct CanonicalRuntimeCore {
    ledger: PgScopeLedger,
    registry: Arc<ScopeRegistry>,
    delivery: DeliveryPublisher,
}
impl CanonicalRuntimeCore {
    /// Preserve the caller's configured instance, retained keys and typed
    /// settler. Validation is delegated to the existing production constructors.
    pub fn new(
        pool: PgPool,
        instance: Uuid,
        keys: Arc<ChildRecoveryKeys>,
        settler: Arc<dyn ChildIntentSettler>,
    ) -> Result<Self, Status> {
        let ledger = PgScopeLedger::new(pool, instance, keys, settler)?;
        let registry = ScopeRegistry::new(instance, Arc::new(ledger.clone()))?;
        let delivery = DeliveryPublisher::new(ledger.clone(), registry.clone());
        Ok(Self {
            ledger,
            registry,
            delivery,
        })
    }

    pub fn ledger(&self) -> PgScopeLedger {
        self.ledger.clone()
    }

    pub fn registry(&self) -> Arc<ScopeRegistry> {
        self.registry.clone()
    }

    pub fn delivery(&self) -> DeliveryPublisher {
        self.delivery.clone()
    }

    /// Build the actual Auth/Friend fence against this core's existing registry.
    /// Endpoint, credential, TLS and signer validation remain with that adapter.
    pub fn owner_fence(
        &self,
        auth_endpoint: &str,
        friend_endpoint: &str,
        credential: &str,
        tls: &TlsMaterial,
        signer: RelationshipReadSigner,
    ) -> anyhow::Result<CanonicalOwnerFence> {
        CanonicalOwnerFence::connect(
            auth_endpoint,
            friend_endpoint,
            credential,
            tls,
            signer,
            self.registry(),
        )
    }

    /// The caller supplies the actual producer-specific native proof adapter.
    pub fn settlement_service(
        &self,
        resolver: Arc<dyn NativeTransactionResolver>,
    ) -> ProducerSettlementService {
        ProducerSettlementService::new(self.ledger(), resolver)
    }

    /// Keep the same retained keys and typed settler as live handler scopes.
    /// The caller owns supervision and the worker's validated system settings.
    pub fn child_recovery_worker(
        &self,
        settings: ChildRecoverySettings,
    ) -> Result<ChildRecoveryWorker, Status> {
        ChildRecoveryWorker::new(self.ledger(), settings)
    }
}
