//! SDK3 Gift-private child producer. Root owns route activation and settlement.
mod locks;
mod native;
#[cfg(test)]
mod native_tests;
use super::{
    authority::validate_binding,
    protocol::{GiftIdentity, GiftInput, GiftOperation, GiftRequest},
};
use crate::{
    adapters::out::relationship_authority::guarded::{
        ChildQuery, ChildRecoveryReference, GiftPrivateKind, HeldChildGuard, OwnerGuardScope,
    },
    community::giveaway::{IssuedCode, PickupView},
};
use concert_application::community::giveaway::PickupKeys;
use sqlx::PgPool;
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;

/// Safe correlation captured only from the exact parsed request and actual scope.
/// No plaintext input, permission boolean, GID or native identity is accepted.
pub struct GiftPrivateRequest {
    identity: GiftIdentity,
    kind: GiftPrivateKind,
    reservation: Uuid,
    binding: [u8; 32],
}
impl GiftPrivateRequest {
    pub fn from_request(request: &GiftRequest, scope: &OwnerGuardScope) -> Result<Self, Status> {
        validate_binding(request, scope)?;
        let (kind, reservation) = match (request.operation(), request.input()) {
            (
                GiftOperation::Pickup,
                GiftInput::Read {
                    target: Some(id), ..
                },
            ) => (GiftPrivateKind::Pickup, *id),
            (
                GiftOperation::IssueCode,
                GiftInput::Write {
                    reservation: Some(id),
                    ..
                },
            ) => (GiftPrivateKind::Code, *id),
            _ => return Err(Status::invalid_argument("Not a Gift private request")),
        };
        let mut identity = request.identity();
        // The real response-wide ceiling may shorten the edge's original
        // budget (owner views or an earlier child). It grants no authority;
        // actual subject checks below remain mandatory.
        identity.deadline = scope.disclosure_ceiling()?;
        Ok(Self {
            identity,
            kind,
            reservation,
            binding: scope.binding().digest(),
        })
    }
    fn check(&self, scope: &OwnerGuardScope, kind: GiftPrivateKind) -> Result<(), Status> {
        scope.binding().check()?;
        self.identity
            .check()
            .map_err(|_| Status::deadline_exceeded("Gift private deadline"))?;
        if self.kind != kind
            || self.binding != scope.binding().digest()
            || self.reservation.is_nil()
        {
            return Err(Status::permission_denied("Gift private request mismatch"));
        }
        Ok(())
    }
}
/// The clip is transport-only. Business reservation/meeting/code expiries are
/// unchanged. Root MUST apply this clip before every positive byte delivery.
pub struct GiftDisclosure<T> {
    value: T,
    valid_until_unix_micros: i64,
}
impl<T> GiftDisclosure<T> {
    pub fn into_parts(self) -> (T, i64) {
        (self.value, self.valid_until_unix_micros)
    }
}
pub struct GiftPrivateRuntime {
    pool: PgPool,
    keys: Arc<PickupKeys>,
}
impl GiftPrivateRuntime {
    pub(crate) fn pickup_keys(&self) -> Arc<PickupKeys> {
        self.keys.clone()
    }
    pub fn new(pool: PgPool, keys: Arc<PickupKeys>) -> Self {
        Self { pool, keys }
    }
    pub async fn prepare_pickup(
        &self,
        scope: Arc<OwnerGuardScope>,
        request: GiftPrivateRequest,
        giveaway: Uuid,
    ) -> Result<GiftDisclosure<PickupView>, Status> {
        request.check(&scope, GiftPrivateKind::Pickup)?;
        scope
            .run(async {
                let (view, until) = self.acquire(&scope, &request, giveaway, None).await?;
                Ok(GiftDisclosure {
                    value: view,
                    valid_until_unix_micros: until,
                })
            })
            .await
    }
    pub async fn prepare_code(
        &self,
        scope: Arc<OwnerGuardScope>,
        request: GiftPrivateRequest,
        giveaway: Uuid,
        issued: IssuedCode,
    ) -> Result<GiftDisclosure<IssuedCode>, Status> {
        request.check(&scope, GiftPrivateKind::Code)?;
        if issued.reservation != request.reservation {
            return Err(Status::permission_denied("Gift code target mismatch"));
        }
        scope
            .run(async {
                let (_, until) = self
                    .acquire(&scope, &request, giveaway, Some(&issued))
                    .await?;
                Ok(GiftDisclosure {
                    value: issued,
                    valid_until_unix_micros: until,
                })
            })
            .await
    }
    async fn acquire(
        &self,
        scope: &Arc<OwnerGuardScope>,
        request: &GiftPrivateRequest,
        giveaway: Uuid,
        issued: Option<&IssuedCode>,
    ) -> Result<(PickupView, i64), Status> {
        if giveaway.is_nil() {
            return Err(Status::invalid_argument("Invalid Gift target"));
        }
        let actor = scope.public_profile_subject(request.identity.actor).await?;
        actor.check().await?;
        let mut tx = self.pool.begin().await.map_err(|_| native::unavailable())?;
        // Capture the real full native TX FIRST. It is a typed read child, NOT a
        // generic Tracked transaction. No fabricated OwnerGuardScope is possible.
        let native_identity = actor.capture_child_identity_in(&mut tx).await?;
        native::budget(
            &mut tx,
            request.identity.deadline,
            request.identity.deadline.expires_micros(),
        )
        .await?;
        let head = locks::root_in(&mut tx, request, giveaway).await?;
        let owner = scope.public_profile_subject(head.owner).await?;
        owner.check().await?;
        let header = locks::metadata_in(&mut tx, request, giveaway, head, issued).await?;
        native::reserve(&mut tx).await?;
        let reference = ChildRecoveryReference::GiftPrivate {
            query: ChildQuery::from_subject(&owner),
            child_nonce: Uuid::new_v4(),
            native: native_identity,
            private_kind: request.kind,
            giveaway_id: giveaway,
            reservation_id: request.reservation,
            valid_until_unix_micros: header.until,
        };
        // Until this durable acknowledgement returns, cancellation only drops
        // the ordinary unprepared TX. No private content has been selected.
        owner.register_child_intent(&reference).await?;
        native::budget(&mut tx, request.identity.deadline, header.until).await?;
        let view =
            locks::content_in(&mut tx, request, giveaway, &header, issued, &self.keys).await?;
        owner.check().await?;
        let child = Arc::new(
            native::prepare_after_intent(
                self.pool.clone(),
                tx,
                reference,
                request.identity.deadline,
            )
            .await?,
        );
        owner.retain_child(child.clone()).await?;
        child.check().await?;
        owner.check().await?;
        Ok((view, header.until))
    }
    /// Recovery only. Root must authenticate the exact encrypted saved intent
    /// and route only GIFT_PRIVATE here after root completion.
    pub async fn recover_one(&self, reference: &ChildRecoveryReference) -> Result<(), Status> {
        native::recover_one(&self.pool, reference).await
    }
}
#[async_trait::async_trait]
impl crate::product_rpc::child_settlement::GiftPrivateIntentSettler for GiftPrivateRuntime {
    async fn recover_gift_private(&self, reference: &ChildRecoveryReference) -> Result<(), Status> {
        self.recover_one(reference).await
    }
}
