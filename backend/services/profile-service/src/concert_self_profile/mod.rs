//! Dedicated canonical self content; transport and coordinator wiring is additive.
pub mod fingerprint;
mod identity;
pub mod model;
mod operation;
pub mod patch;
mod store;

#[cfg(test)]
mod native;
