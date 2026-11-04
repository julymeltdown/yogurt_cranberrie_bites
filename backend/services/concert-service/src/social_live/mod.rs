//! BE28–30/37 product producer. Root owns dispatch registration and delivery.
pub mod account_deletion;
mod cursor;
pub mod invalidation;
pub mod keys;
pub mod moderation_revocation;
mod producer;
pub mod protocol;
pub mod provenance;
pub mod read_child;
pub mod witness;
pub use producer::SocialRuntime;
