//! Notification-owned native evidence. No constructor here grants target access.
mod durable;
mod inbox;
pub use inbox::{NotificationInboxService, RootTargetSnapshotOwner, INBOX_POLICY};
mod owners;
mod settlement;
pub(crate) use durable::{AuthorityStore, Intent, NativeIdentity, NativeOutcome};
pub(crate) use owners::{CheckedScope, Owners};
pub use settlement::NotificationSettlement;
pub use settlement::SETTLEMENT_POLICY;

use super::transport::lease;
use platform::actor_delegation::owner_binding::OwnerBinding;
use tonic::Status;
use uuid::Uuid;

pub(crate) fn canonical(value: &str) -> Result<Uuid, Status> {
    let id = Uuid::parse_str(value).map_err(|_| denied())?;
    if id.is_nil() || id.to_string() != value {
        return Err(denied());
    }
    Ok(id)
}
pub(crate) fn binding_digest(binding: &lease::GuardBinding) -> Result<[u8; 32], Status> {
    let subjects = binding
        .subject_ids
        .iter()
        .map(|v| canonical(v))
        .collect::<Result<Vec<_>, _>>()?;
    let unique: std::collections::BTreeSet<_> = subjects.iter().copied().collect();
    let actor = canonical(&binding.actor_id)?;
    if subjects.len() > 101
        || unique.len() != subjects.len()
        || unique.iter().filter(|id| **id != actor).count() > 100
    {
        return Err(denied());
    }
    OwnerBinding {
        actor_id: canonical(&binding.actor_id)?,
        event_id: canonical(&binding.event_id)?,
        session_family_id: canonical(&binding.session_family_id)?,
        request_id: canonical(&binding.request_id)?,
        operation: binding.operation.clone(),
        expires_unix_micros: binding.expires_unix_micros,
        parent_digest: binding
            .parent_digest
            .clone()
            .try_into()
            .map_err(|_| denied())?,
        parent_body_digest: binding
            .parent_body_digest
            .clone()
            .try_into()
            .map_err(|_| denied())?,
        parent_method: binding.parent_method.clone(),
        http_method: binding.http_method.clone(),
        http_target: binding.http_target.clone(),
        body_digest: binding
            .body_digest
            .clone()
            .try_into()
            .map_err(|_| denied())?,
    }
    .digest()
}
fn denied() -> Status {
    Status::permission_denied("Notification authority binding denied")
}
