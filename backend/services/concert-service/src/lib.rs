//! Concert composition. No product HTTP/RPC adapter is exposed before BE06 contracts.
pub mod adapters;
pub mod config;
pub mod health;
pub mod rpc;
pub mod bootstrap;

pub mod workers {
    pub mod outbox;
}

pub mod relationship;
pub mod meeting;
pub mod feed;
pub mod world;
pub mod community;
