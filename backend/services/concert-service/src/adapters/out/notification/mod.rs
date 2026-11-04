//! BE15 at-least-once consumer. Ownership transport is injected by Delivery.
//! No database access across service boundaries and no raw payload forwarding.
pub mod source;
use crate::{
    adapters::out::repository::outbox::{Delivery, DeliveryFailure},
    workers::outbox::IdempotentInvalidationSink,
};
use async_trait::async_trait;
use concert_application::{
    commands::maintenance::InvalidationKind,
    notification::{DurableNotificationOwner, NotificationProjectionOwner},
};
use concert_domain::notification::{Envelope, NotificationError};

pub struct NotificationSink<P, O> {
    pub projection: P,
    pub owner: O,
}
#[async_trait]
impl<P: NotificationProjectionOwner, O: DurableNotificationOwner> IdempotentInvalidationSink
    for NotificationSink<P, O>
{
    async fn deliver(&self, delivery: &Delivery) -> Result<(), DeliveryFailure> {
        let receipt = match (delivery.kind, delivery.receipt) {
            (InvalidationKind::ResourceInvalidated, Some(receipt)) => receipt,
            // Maintenance has no actor command receipt. Its Notification policy
            // is not bound by this command-only projection; never invent one or
            // acknowledge an unqualified fanout leg.
            (InvalidationKind::MaintenanceInvalidated, None) => {
                return Err(DeliveryFailure::Retryable);
            }
            _ => return Err(DeliveryFailure::Rejected),
        };
        let envelope = Envelope {
            outbox_id: delivery.id,
            receipt_id: receipt,
            event_id: delivery.event,
            resource_id: delivery.reference.resource,
            resource_revision: delivery.reference.revision,
        };
        envelope.validate().map_err(failure)?;
        let recipients = self.projection.resolve(envelope).await.map_err(failure)?;
        self.owner
            .deliver(envelope, recipients)
            .await
            .map_err(failure)
    }
}
fn failure(error: NotificationError) -> DeliveryFailure {
    match error {
        NotificationError::InvalidInput | NotificationError::IdempotencyConflict => {
            DeliveryFailure::Rejected
        }
        // Current policy/auth may recover before BE15's bounded retry exhaustion.
        _ => DeliveryFailure::Retryable,
    }
}
pub struct UnavailableNotificationOwner;
#[async_trait]
impl DurableNotificationOwner for UnavailableNotificationOwner {
    async fn deliver(
        &self,
        _: Envelope,
        _: Vec<concert_domain::notification::Recipient>,
    ) -> concert_domain::notification::Result<()> {
        Err(NotificationError::Unavailable)
    }
}
pub struct UnavailableProjectionOwner;
#[async_trait]
impl NotificationProjectionOwner for UnavailableProjectionOwner {
    async fn resolve(
        &self,
        _: Envelope,
    ) -> concert_domain::notification::Result<Vec<concert_domain::notification::Recipient>> {
        Err(NotificationError::Unavailable)
    }
}
