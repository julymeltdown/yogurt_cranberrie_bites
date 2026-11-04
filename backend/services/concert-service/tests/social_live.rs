//! New bridge modules are compiled here until Root owns their public export.
//! Synthetic native checks do not manufacture an OwnerGuardScope.
pub use concert_service::{adapters, product_rpc, relationship, world};
pub use concert_service::workers;
#[path = "social_live_support/hooks.rs"]
mod hook_tests;
#[path = "social_live_support/intro_edit.rs"]
mod intro_edit_tests;
#[path = "social_live_support/invalidation.rs"]
mod invalidation_tests;
#[path = "social_remaining_support/owned.rs"]
mod owned;
#[path = "social_live_support/protocol.rs"]
mod protocol_tests;
#[path = "../src/social_live/mod.rs"]
mod social_live;
#[path = "social_live_support/witness.rs"]
mod witness_tests;
