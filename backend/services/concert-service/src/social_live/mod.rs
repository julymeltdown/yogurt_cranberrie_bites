//! BE28–30/37 product producer. Root owns dispatch registration and delivery.
mod cursor;
pub mod keys;
mod producer;
pub mod protocol;
pub mod provenance;
pub mod read_child;
pub mod witness;
pub use producer::SocialRuntime;
