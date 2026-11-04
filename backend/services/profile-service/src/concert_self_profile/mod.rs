//! Dedicated canonical self content; transport and coordinator wiring is additive.
mod coordinator;
pub mod fingerprint;
mod guard;
mod identity;
pub mod model;
mod operation;
pub mod patch;
mod read_reference;
mod reconcile;
mod recovery;
pub mod server;
pub mod settlement;
mod store;
pub mod wire;

#[cfg(test)]
mod native;
#[cfg(test)]
mod native_rpc;
#[cfg(test)]
mod native_prepare;
