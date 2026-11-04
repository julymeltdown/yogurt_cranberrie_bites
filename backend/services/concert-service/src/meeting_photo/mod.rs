//! BE09/FE20 photo producer. Root must supply qualified canonical exclusion.
mod authority;
pub mod canonical;
mod producer;
pub mod wire;
pub use producer::{PhotoBeacons, PhotoFrame, PhotoMutation, PhotoTransactionGuard};
