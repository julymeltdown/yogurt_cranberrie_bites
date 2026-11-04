//! Canonical MCR global enrollment. Event admission and verified age are separate.
mod contract;
pub mod rpc;
mod store;

pub use contract::*;
pub use store::EnrollmentStore;
