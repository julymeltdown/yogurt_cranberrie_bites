//! Concert cleanup ports for the canonical Auth deletion saga.
//!
//! Owners must atomically persist the operation key and their effects, including
//! a permanent deleted-user fence. Retries (including after an uncertain reply)
//! use exactly the same key. An acknowledgement means all owner effects are
//! durable; accepting a job is insufficient. No identity or saga store lives here.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConcertDeletionTarget {
    Tickets,
    Sessions,
    Sharing,
    PrivateData,
}

/// Identity is stable across worker attempts, leases, and operator retries.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConcertDeletionKey {
    pub saga_id: Uuid,
    pub user_id: Uuid,
    pub target: ConcertDeletionTarget,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ConcertDeletionCommand {
    pub key: ConcertDeletionKey,
    pub actor_id: Uuid,
    pub retention_until: DateTime<Utc>,
}

impl ConcertDeletionCommand {
    pub fn validate(&self) -> Result<(), ConcertDeletionFailure> {
        if self.key.saga_id.is_nil() || self.key.user_id.is_nil() || self.actor_id.is_nil() {
            return Err(ConcertDeletionFailure::InvalidCommand);
        }
        Ok(())
    }
}

/// An owner assertion of durable completion, bound to one exact operation.
/// The revision identifies the owner's committed deleted-user fence. Transport
/// adapters must authenticate the owner; this value alone is not proof of trust.
#[derive(Clone, PartialEq, Eq)]
pub struct ConcertDeletionAcknowledgement {
    pub command: ConcertDeletionCommand,
    pub fence_revision: u64,
}

impl ConcertDeletionAcknowledgement {
    pub fn verifies(&self, command: &ConcertDeletionCommand) -> bool {
        self.fence_revision > 0 && &self.command == command
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConcertDeletionFailure {
    #[error("Concert deletion owner unavailable")]
    Unavailable,
    #[error("Concert deletion not durably completed")]
    Incomplete,
    #[error("Invalid Concert deletion command")]
    InvalidCommand,
}

macro_rules! owner_port {
    ($name:ident, $contract:literal) => {
        #[doc = $contract]
        #[async_trait]
        pub trait $name: Send + Sync {
            async fn apply(
                &self,
                command: ConcertDeletionCommand,
            ) -> Result<ConcertDeletionAcknowledgement, ConcertDeletionFailure>;
        }
    };
}

owner_port!(ConcertTicketDeletionPort,
    "Revoke every outstanding world ticket and fence all future issuance/consumption for this Auth user.");
owner_port!(ConcertSessionDeletionPort,
    "Disconnect active world sessions, remove presence/subscriptions, invalidate private caches, and fence reconnect.");
owner_port!(ConcertSharingDeletionPort,
    "Revoke incoming/outgoing requests and grants, remove encrypted handles, invalidate reveal caches, and fence future reveal/writes.");
owner_port!(ConcertPrivateDataDeletionPort,
    "Scrub retained personal fields across Concert features under the reviewed retention policy and fence recreation. Retain required aggregate/receipt/audit integrity.");

// Deliberately suppress identifiers and retention details in diagnostics.
macro_rules! redacted_debug {
    ($($name:ty),+ $(,)?) => {$(
        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(concat!(stringify!($name), "([REDACTED])"))
            }
        }
    )+};
}
redacted_debug!(
    ConcertDeletionKey,
    ConcertDeletionCommand,
    ConcertDeletionAcknowledgement
);
