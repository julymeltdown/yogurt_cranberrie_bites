//! Concrete canonical SDK adapter for the retained Gift store. No local success grant.
use super::{
    protocol::{GiftOperation, GiftRequest},
    resources::{GiftMediaLink, PgGiftResources},
};
use crate::{
    adapters::out::relationship_authority::guarded::{GuardedAuthority, OwnerGuardScope},
    community::{
        giveaway::{GiftAuthority, GiftSettings},
        policy::SharedCommunityAuthority,
        CommunityAccess, CommunityAuthority,
    },
    relationship::{FencedRelationshipAuthority, PgRelationshipPolicy},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::commands::CommandError;
use concert_domain::{
    community::giveaway::PickupMeeting,
    relationship::{FreshAuthority, PolicyQuery, RelationshipError},
};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use std::sync::Arc;
use tonic::{Code, Status};
use uuid::Uuid;

/// Root supplies the actual acquired scope. Correlation alone cannot create one.
pub(crate) fn validate_binding(
    request: &GiftRequest,
    scope: &OwnerGuardScope,
) -> Result<(), Status> {
    request
        .identity()
        .check()
        .map_err(|_| Status::deadline_exceeded("Gift input deadline"))?;
    let i = request.identity();
    let b = scope.binding();
    b.check()?;
    if (
        b.actor(),
        b.event(),
        b.session_family(),
        b.request_id(),
        b.operation(),
        b.http_method(),
        b.http_target(),
        b.body_digest(),
    ) != (
        i.actor,
        i.event,
        i.session_family,
        i.request_id,
        request.operation().id(),
        request.method(),
        request.target(),
        Sha256::digest(request.body()).into(),
    ) || b.deadline().expires_micros() > i.deadline.expires_micros()
        || b.deadline().stop() > i.deadline.stop()
    {
        return Err(Status::permission_denied("Gift scope mismatch"));
    }
    Ok(())
}

#[derive(Clone)]
pub(crate) struct TrackedCanonical {
    scope: Arc<OwnerGuardScope>,
    effect: [u8; 32],
    reference: Arc<Vec<u8>>,
}
impl TrackedCanonical {
    pub(crate) fn new(request: &GiftRequest, scope: Arc<OwnerGuardScope>) -> Result<Self, Status> {
        validate_binding(request, &scope)?;
        // The signed parent covers exact transport payload; additionally bind the
        // typed effect to its key/content type without ever persisting code bytes.
        let mut h = Sha256::new();
        h.update(b"MCR-gift-transaction-v1\0");
        h.update(scope.binding().digest());
        h.update(request.idempotency_key().unwrap_or(Uuid::nil()).as_bytes());
        h.update((request.content_type().len() as u64).to_be_bytes());
        h.update(request.content_type().as_bytes());
        let i = request.identity();
        let reference=serde_json::to_vec(&serde_json::json!({"kind":"GIFT_REQUEST_V1","actor_id":i.actor,"event_id":i.event,"request_id":i.request_id,"operation_id":request.operation().id(),"idempotency_key":request.idempotency_key()})).map_err(|_|Status::unavailable("Gift settlement binding"))?;
        Ok(Self {
            scope,
            effect: h.finalize().into(),
            reference: Arc::new(reference),
        })
    }
    pub(crate) async fn register_in(&self, c: &mut PgConnection) -> Result<(), CommandError> {
        self.scope
            .register_transaction_in(c, self.effect, self.reference.as_ref().clone())
            .await
            .map_err(command)
    }
}
fn command(s: Status) -> CommandError {
    match s.code() {
        Code::PermissionDenied | Code::Unauthenticated | Code::NotFound => CommandError::Denied,
        Code::ResourceExhausted => CommandError::Capacity,
        _ => CommandError::Unavailable,
    }
}
#[async_trait]
impl FencedRelationshipAuthority for TrackedCanonical {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        q: PolicyQuery,
        now: DateTime<Utc>,
    ) -> Result<FreshAuthority, RelationshipError> {
        self.register_in(c).await.map_err(|e| {
            if e == CommandError::Denied {
                RelationshipError::Denied
            } else {
                RelationshipError::Unavailable
            }
        })?;
        GuardedAuthority::new(self.scope.clone())
            .check_in(c, q, now)
            .await
    }
}

pub struct ScopedGiftAuthority<M> {
    inner: Arc<SharedCommunityAuthority<TrackedCanonical, PgGiftResources<M>>>,
    safety: bool,
}
impl<M: GiftMediaLink> ScopedGiftAuthority<M> {
    pub fn new(
        pool: PgPool,
        request: &GiftRequest,
        scope: Arc<OwnerGuardScope>,
        media: M,
    ) -> Result<Self, CommandError> {
        let canonical = TrackedCanonical::new(request, scope).map_err(command)?;
        Ok(Self {
            inner: Arc::new(SharedCommunityAuthority::new(
                PgRelationshipPolicy::new(pool, canonical),
                PgGiftResources::new(request, media)?,
            )),
            safety: request.operation() == GiftOperation::Cancel,
        })
    }
    fn access(&self, access: CommunityAccess) -> CommunityAccess {
        // Retained receipt authorization asks ReadGifts first. For this exact
        // validated cancel command, apply safety policy there as well as in apply.
        if self.safety && access == CommunityAccess::ReadGifts {
            CommunityAccess::SafetyGifts
        } else {
            access
        }
    }
}
impl<M> Clone for ScopedGiftAuthority<M> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            safety: self.safety,
        }
    }
}
#[async_trait]
impl<M: GiftMediaLink> CommunityAuthority for ScopedGiftAuthority<M> {
    async fn actor_in(
        &self,
        c: &mut PgConnection,
        actor: Uuid,
        event: Uuid,
        access: CommunityAccess,
    ) -> Result<(), CommandError> {
        self.inner
            .actor_in(c, actor, event, self.access(access))
            .await
    }
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        actor: Uuid,
        event: Uuid,
        subject: Uuid,
        access: CommunityAccess,
    ) -> Result<(), CommandError> {
        self.inner
            .authorize_in(c, actor, event, subject, self.access(access))
            .await
    }
    async fn place_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        place: Uuid,
    ) -> Result<(), CommandError> {
        self.inner.place_in(c, event, place).await
    }
}
#[async_trait]
impl<M: GiftMediaLink> GiftAuthority for ScopedGiftAuthority<M> {
    async fn settings_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
    ) -> Result<GiftSettings, CommandError> {
        self.inner.settings_in(c, event).await
    }
    async fn media_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        actor: Uuid,
        giveaway: Uuid,
        media: Option<Uuid>,
    ) -> Result<(), CommandError> {
        self.inner.media_in(c, event, actor, giveaway, media).await
    }
    async fn meeting_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        meeting: &PickupMeeting,
    ) -> Result<(), CommandError> {
        self.inner.meeting_in(c, event, meeting).await
    }
}
