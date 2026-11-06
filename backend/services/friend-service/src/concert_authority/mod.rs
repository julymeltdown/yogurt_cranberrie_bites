//! Owner-local durable exclusion. Capabilities are private; uncertain remote
//! outcomes quarantine prepared locks and never become an expiry-based release.
pub mod proto {
    tonic::include_proto!("authority.lease.v1");
}
mod budget;
mod model;
mod owner;
mod resolver;
mod store;
pub use budget::{OperationBudget, MAX_COMPLETION, MAX_OPERATION};
pub use resolver::{RpcSettlementResolver, SettlementResolver};
pub use store::{PgOwnerAuthority, MAX_GUARDS};
fn unavailable(error: sqlx::Error) -> tonic::Status {
    if matches!(
        error.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("57014" | "55P03")
    ) {
        tonic::Status::deadline_exceeded("Owner operation expired")
    } else {
        model::unavailable()
    }
}

mod runtime;
pub use runtime::{configured_store, owner_caller_registry, readiness_probe, run_recovery, verify_ready_bounded};
