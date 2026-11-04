//! BE-GIFTS-LIVE production orchestration; constructing input is not owner authority.
pub mod protocol;
pub mod resources;
pub mod response;
pub use protocol::{GiftIdentity, GiftInput, GiftOperation, GiftReply, GiftRequest, InputError};
pub mod authority;
mod correlation;
pub mod cursor;
pub mod media;
mod queries;
pub mod runtime;
mod status;
pub use runtime::GiftRuntime;
