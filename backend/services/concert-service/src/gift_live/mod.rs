//! BE-GIFTS-LIVE production orchestration; constructing input is not owner authority.
pub mod protocol;
pub mod resources;
pub mod response;
pub use protocol::{GiftIdentity, GiftInput, GiftOperation, GiftReply, GiftRequest, InputError};
pub mod authority;
mod correlation;
pub mod cursor;
pub mod media;
pub mod private;
pub mod product;
mod profile;
mod queries;
pub mod runtime;
pub mod settlement;
mod status;
pub use runtime::GiftRuntime;
pub mod moderation;
