//! Concert composition. No product HTTP/RPC adapter is exposed before BE06 contracts.
pub mod adapters;
pub mod config;
pub mod health;
pub mod rpc;
pub mod product_rpc;
pub mod media_runtime;
pub mod bootstrap;

pub mod workers {
    pub mod outbox;
    pub mod cleanup;
}

pub mod relationship;
pub mod meeting;
pub mod feed;
pub mod world;
pub mod world_rpc;
pub mod community;

pub mod event_lifecycle;

pub mod participant;
pub mod gift_live;
pub mod party_live;

pub use concert_application::age_verification;
pub mod meeting_photo;
pub mod social_live;

pub mod moderation_live;

pub mod notification_live;

pub mod event_cleanup_live;
