//! Sealed Social source proof and held SQL baselines on the incumbent World socket.
//! This adapter grants no private-read authority and creates no listener or task.
use crate::{
    adapters::{
        out::repository::outbox::DeliveryFailure,
        r#in::world_ws::{policy, SocketTransport},
    },
    social_live::invalidation::{
        BaselineGuard, PolicyReason, SocialInvalidationSink, SocialPolicyPublisher,
        VerifiedSocialInvalidation,
    },
};
use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

pub struct SocialWorldPolicy {
    pool: PgPool,
    pub transport: SocketTransport,
    pub sink: SocialInvalidationSink,
    /// The SAME sealed publisher/registry used by `sink`, for canonical deletion
    /// composition. Cloning this Arc creates no fallback transport or authority.
    pub publisher: Arc<dyn SocialPolicyPublisher>,
}
impl SocialWorldPolicy {
    /// SOCIAL-only removal leaf sharing this exact incumbent sink/publisher.
    /// The moderation aggregate remains responsible for other private owners.
    pub fn moderation_revocation(
        &self,
    ) -> crate::social_live::moderation_revocation::SocialModerationRevocation {
        crate::social_live::moderation_revocation::SocialModerationRevocation::new(
            self.pool.clone(),
            self.sink.clone(),
        )
    }
    pub fn compose(pool: PgPool) -> Self {
        let transport = SocketTransport::default();
        // The publisher retains only the shared socket registry. Create it
        // before attaching the baseline owner to avoid an ownership cycle.
        let publisher: Arc<dyn SocialPolicyPublisher> = Arc::new(SealedPublisher {
            world: transport.policy_publisher(),
            pool: pool.clone(),
        });
        let sink = SocialInvalidationSink::new(pool.clone(), publisher.clone());
        let transport = transport.with_policy_baseline(Arc::new(HeldBaseline(sink.clone())));
        Self {
            pool,
            transport,
            sink,
            publisher,
        }
    }
}

struct SealedPublisher {
    world: policy::WorldPolicyPublisher,
    pool: PgPool,
}
#[async_trait]
impl SocialPolicyPublisher for SealedPublisher {
    async fn publish(&self, signal: &VerifiedSocialInvalidation) -> Result<(), DeliveryFailure> {
        // Already connected foreign owners need their real publication leg.
        // New/reconnecting streams read the durable generation through the
        // held baseline; source generation commit precedes this verified call.
        let connections: Vec<Uuid> = sqlx::query_scalar("select connection_id from public.concert_world_memberships where event_id=$1 and user_id=$2 and not disconnected and lease_until>clock_timestamp() limit 801")
            .bind(signal.event()).bind(signal.actor()).fetch_all(&self.pool).await
            .map_err(|_| DeliveryFailure::Retryable)?;
        if connections.len() > 800
            || !self
                .world
                .covers_connections(signal.event(), signal.actor(), &connections)
                .map_err(|_| DeliveryFailure::Retryable)?
        {
            return Err(DeliveryFailure::Retryable);
        }
        self.world
            .publish_metadata(
                signal.event(),
                signal.actor(),
                signal.policy_revision(),
                reason(signal.reason()),
            )
            .map_err(|_| DeliveryFailure::Retryable)
    }
}

fn reason(value: PolicyReason) -> policy::PolicyReason {
    match value {
        PolicyReason::SourceChanged => policy::PolicyReason::SourceChanged,
        PolicyReason::ConsentChanged => policy::PolicyReason::ConsentChanged,
        PolicyReason::BlockChanged => policy::PolicyReason::BlockChanged,
        PolicyReason::Expired => policy::PolicyReason::Expired,
        PolicyReason::CanonicalPolicyChanged => policy::PolicyReason::CanonicalPolicyChanged,
    }
}

struct HeldBaseline(SocialInvalidationSink);
struct HeldGuard(BaselineGuard);
#[async_trait]
impl policy::PolicyBaselinePort for HeldBaseline {
    async fn held_baseline(
        &self,
        event: Uuid,
        actor: Uuid,
    ) -> Result<Box<dyn policy::HeldPolicyBaseline>, policy::PolicyUnavailable> {
        self.0
            .held_baseline(event, actor)
            .await
            .map(|guard| Box::new(HeldGuard(guard)) as Box<dyn policy::HeldPolicyBaseline>)
            .map_err(|_| policy::PolicyUnavailable)
    }
}
#[async_trait]
impl policy::HeldPolicyBaseline for HeldGuard {
    fn revision(&self) -> i64 {
        self.0.revision()
    }
    async fn commit(self: Box<Self>) -> Result<(), policy::PolicyUnavailable> {
        self.0.commit().await.map_err(|_| policy::PolicyUnavailable)
    }
}
