//! Concrete canonical Auth/Friend durable-guard consumer. There is no positive
//! observation-only constructor. Production requires actual owner RPCs and a
//! durable producer-owned lifecycle implementation.
mod child;
mod contract;
mod network;
mod registry;
mod scope;
mod wire;
pub use child::{ChildNativeIdentity, ChildQuery, ChildRecoveryReference};
pub use contract::*;
pub use registry::{RootReadPermit, ScopeLifecycle, ScopeRegistry};
pub use scope::{CanonicalOwnerFence, GuardedAuthority, OwnerGuardScope};
pub use wire::wire_binding_digest;
pub mod proto {
    tonic::include_proto!("authority.lease.v1");
}
