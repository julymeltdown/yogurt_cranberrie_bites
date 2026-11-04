//! Bounded BE39 engine. Public dispatch and actual providers remain Root-owned.
mod authority;
mod cursor;
pub mod ports;
mod runtime;
pub mod wire;
pub use authority::CanonicalModerationAuthority;
pub use runtime::{ModerationReply, ModerationRuntime};
