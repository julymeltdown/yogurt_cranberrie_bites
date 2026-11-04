//! CF02 actual native fixtures. Reused modules stay read-only in CF01 paths.
#[path = "../canonical_owner_composition_support/capture.rs"]
pub(crate) mod capture;
pub(crate) mod environment;
#[path = "../process_child_recovery_support/fault.rs"]
pub(crate) mod fault;
pub(crate) mod fixture;
pub(crate) mod owners;
#[path = "../canonical_owner_composition_support/release.rs"]
pub(crate) mod release;
