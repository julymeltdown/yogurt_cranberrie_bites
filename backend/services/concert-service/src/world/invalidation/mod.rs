//! Removal-only local application of verified metadata from the retained Concert outbox.
//! Remote source authentication and complete deployment fanout remain unavailable.
mod checkpoint;
pub(crate) mod remote;
pub(crate) mod runtime;
mod store;
use super::{admission_bridge::AdmissionBridge, simulation::Binding};
use crate::adapters::out::repository::outbox::{Delivery, DeliveryFailure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Scope {
    Pair { event: Uuid, a: Uuid, b: Uuid },
    Event(Uuid),
}
impl Scope {
    pub(crate) fn event(&self) -> Uuid {
        match self {
            Self::Pair { event, .. } => *event,
            Self::Event(event) => *event,
        }
    }
    pub(crate) fn matches(&self, event: Uuid, user: Uuid) -> bool {
        self.event() == event
            && match self {
                Self::Pair { a, b, .. } => user == *a || user == *b,
                Self::Event(_) => true,
            }
    }
    fn parts(&self) -> (&'static str, Uuid, Uuid) {
        match self {
            Self::Pair { a, b, .. } => ("PAIR", (*a).min(*b), (*a).max(*b)),
            Self::Event(e) => ("EVENT", *e, *e),
        }
    }
}
/// Exact persisted victims; a duplicate cannot revoke a newly authorized connection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Victim {
    pub user: Uuid,
    pub room: Uuid,
    pub family: Uuid,
    pub connection: Uuid,
    pub owner: String,
    pub epoch: Uuid,
}
impl Victim {
    pub(crate) fn matches(&self, b: &Binding, owner_epoch: Uuid) -> bool {
        self.user == b.key.user_id.uuid()
            && self.room == b.room_id
            && self.family == b.session_id
            && self.connection == b.connection_id
            && self.owner == b.owner_id.to_string()
            && self.epoch == owner_epoch
    }
}
#[derive(Clone)]
pub(crate) enum Removal {
    Scope(Scope),
    Victims { event: Uuid, victims: Vec<Victim> },
    RemoteScope(remote::RemoteScope),
    RemoteVictims(Vec<remote::RemoteVictim>),
}
impl Removal {
    pub(crate) fn matches(&self, b: &Binding, epoch: Uuid) -> bool {
        match self {
            Self::Scope(s) => s.matches(b.key.event_id.uuid(), b.key.user_id.uuid()),
            Self::Victims { event, victims } => {
                *event == b.key.event_id.uuid() && victims.iter().any(|v| v.matches(b, epoch))
            }
            Self::RemoteScope(scope) => scope.matches(b.key.user_id.uuid(), b.session_id),
            Self::RemoteVictims(victims) => victims
                .iter()
                .any(|v| v.event == b.key.event_id.uuid() && v.victim.matches(b, epoch)),
        }
    }
    pub(crate) fn masks_user(&self, event: Uuid, user: Uuid) -> bool {
        match self {
            Self::Scope(scope) => scope.matches(event, user),
            Self::RemoteScope(scope) => scope.public_users().contains(&user),
            _ => false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyError {
    UnqualifiedSource,
    Unbound,
    StaleLease,
    Unavailable,
}
impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "World invalidation failed: {self:?}")
    }
}
impl std::error::Error for ApplyError {}
impl From<ApplyError> for DeliveryFailure {
    fn from(_: ApplyError) -> Self {
        Self::Retryable
    }
}
/// Untrusted remote hints have no conversion to an authenticated local signal.
/// A later transport owner must supply its own sealed verified source contract.
#[derive(Clone, Debug)]
pub struct RemoteInvalidationHint {
    pub owner: RemoteOwner,
    pub owner_epoch: Uuid,
    pub signal_id: Uuid,
    pub user: Option<Uuid>,
    pub family: Option<Uuid>,
    pub pair: Option<(Uuid, Uuid)>,
    pub event: Option<Uuid>,
    pub revision: Option<i64>,
    pub cursor: Option<i64>,
}
#[derive(Clone, Copy, Debug)]
pub enum RemoteOwner {
    Auth,
    Friend,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationAck {
    pub signal_id: Uuid,
    pub duplicate: bool,
    pub superseded: bool,
    pub revoked: usize,
}
/// Deliberately NOT an IdempotentInvalidationSink: local World effects cannot
/// acknowledge the general queue on behalf of other required consumers.
pub struct WorldInvalidationConsumer {
    bridge: Arc<AdmissionBridge>,
}
impl WorldInvalidationConsumer {
    pub fn new(bridge: Arc<AdmissionBridge>) -> Self {
        Self { bridge }
    }
    pub fn reject_remote(_: &RemoteInvalidationHint) -> Result<ApplicationAck, ApplyError> {
        Err(ApplyError::UnqualifiedSource)
    }
    pub async fn apply(&self, delivery: &Delivery) -> Result<ApplicationAck, ApplyError> {
        // One coherent order: delivery gate -> short native fence -> admission
        // gate -> SQL transaction. No native mutex is held across SQL/await.
        let _serial = self.bridge.invalidation_delivery_gate().lock().await;
        let (pool, consumer, epoch, event) = self.bridge.invalidation_context();
        let signal = store::resolve(&pool, delivery, consumer, epoch, event).await?;
        let previous = store::previous(&pool, consumer, &signal).await?;
        let removal = match &previous {
            Some(old) => Removal::Victims {
                event,
                victims: old.victims.clone(),
            },
            None if signal.superseded => Removal::Victims {
                event,
                victims: vec![],
            },
            None => Removal::Scope(signal.scope.clone()),
        };
        self.bridge
            .begin_invalidation(delivery.id, removal.clone())
            .map_err(|_| ApplyError::Unavailable)?;
        // A cancelled/failed operation leaves the removal fence installed. A
        // retry completes it; it never rolls authority back into the live cache.
        let _gate = self.bridge.invalidation_admission_gate().lock().await;
        self.bridge
            .pause_invalidation()
            .await
            .map_err(|_| ApplyError::Unavailable)?;
        self.bridge
            .begin_invalidation(delivery.id, removal)
            .map_err(|_| ApplyError::Unavailable)?;
        let result = store::apply(&pool, delivery, consumer, epoch, event, &signal).await?;
        // Durable commit (including victim identities) has completed before ack.
        self.bridge
            .finish_invalidation(delivery.id)
            .map_err(|_| ApplyError::Unavailable)?;
        Ok(ApplicationAck {
            signal_id: delivery.id,
            duplicate: result.duplicate,
            superseded: result.superseded,
            revoked: result.victims.len(),
        })
    }
}
