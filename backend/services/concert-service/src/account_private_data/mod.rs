//! BE42 PrivateData core owner.
//!
//! This leaf erases participant preferences and avatar selection only. It is
//! intentionally not a complete PrivateData deletion port: other Concert
//! categories remain with their actual owners and are represented durably by
//! the companion inventory in the backend documentation.

pub mod core;
pub use core::*;
