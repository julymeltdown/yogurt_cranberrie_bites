//! Typed handoff from the actual supervised World listener to account deletion.
//! This retains serving objects, not a second runtime or an authority snapshot.
use crate::{
    adapters::r#in::world_ws::WorldSocketState,
    world::{
        account_deletion::DeletionFailure, account_sessions::SessionOwner,
        admission_bridge::AdmissionBridge,
    },
};
use std::sync::Arc;

/// The bridge and socket state created together by the serving World owner.
/// Receipt is not ongoing readiness or canonical Auth deletion provenance.
pub struct ServingWorldBinding {
    bridge: Arc<AdmissionBridge>,
    sockets: Arc<WorldSocketState>,
}
impl ServingWorldBinding {
    /// Reuse the Sessions owner's existing exact-object validation. A bridge
    /// reconstructed with identical IDs still cannot bind to another state.
    pub fn new(
        bridge: Arc<AdmissionBridge>,
        sockets: Arc<WorldSocketState>,
    ) -> Result<Self, DeletionFailure> {
        SessionOwner::new(bridge.clone(), sockets.as_ref())?;
        Ok(Self { bridge, sockets })
    }

    /// Borrow the retained actual serving state for the existing composer.
    /// The caller must also supervise World startup/liveness before mounting it.
    pub fn as_serving_world(&self) -> super::account_deletion_runtime::ServingWorld<'_> {
        super::account_deletion_runtime::ServingWorld {
            bridge: self.bridge.clone(),
            sockets: self.sockets.as_ref(),
        }
    }
}
