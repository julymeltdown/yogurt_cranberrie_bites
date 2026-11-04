//! Authority admission seam for the Post concert-content owner.
//!
//! A transport-authenticated caller/actor is necessary but never sufficient.
//! A real implementation must hold relationship, eligibility, linked-object and
//! READY media revisions stable until the returned fence is dropped. Observe-only
//! relationship snapshots cannot implement this contract. The public owner
//! constructor remains DenyAuthority. The dedicated read runtime installs only
//! its concrete scoped authority after actual durable owner registrations.
//! Fixture authorities are compiled only into owner unit tests.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use post_domain::concert_feed::{Command, Comment, Error, Post};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub actor_id: Uuid,
    pub event_id: Uuid,
    pub session_family_id: Uuid,
    pub request_id: Uuid,
    /// Digest of the original admitted authorization, not a bearer credential.
    pub original_authority: [u8; 32],
    pub expires_at: DateTime<Utc>,
}
impl Context {
    pub fn validate(&self, now: DateTime<Utc>) -> Result<(), Error> {
        if [
            self.actor_id,
            self.event_id,
            self.session_family_id,
            self.request_id,
        ]
        .iter()
        .any(Uuid::is_nil)
            || self.original_authority == [0; 32]
            || self.expires_at <= now
            || self.expires_at - now > chrono::Duration::seconds(65)
        {
            Err(Error::Authority)
        } else {
            Ok(())
        }
    }
    pub fn binds(&self, command: &Command) -> Result<(), Error> {
        if self.actor_id != command.actor_id || self.event_id != command.event_id {
            Err(Error::Authority)
        } else {
            Ok(())
        }
    }
}

/// Exact request and actual owner snapshots. Constructors cannot attest policy.
/// The authority must verify current subject eligibility/blocks, public event
/// participation, and every link/media against its owning service, including
/// event, media uploader/purpose/READY state and current related-object visibility.
#[derive(Clone, Serialize)]
#[serde(tag = "access")]
pub enum Access {
    ListPosts {
        window: Window,
    },
    Write {
        command: Command,
        parent: Option<Post>,
        comment: Option<Comment>,
    },
    ReadPost {
        post: Post,
    },
    ReadComment {
        parent: Post,
        comment: Comment,
    },
    Receipt {
        command: Command,
        parent: Option<Post>,
        comment: Option<Comment>,
    },
}

/// The binding is SHA-256(context + exact access), computed by the owner adapter.
/// Implementations must retain all authority locks/leases through transaction
/// completion. A boolean or caller-supplied digest is not a fence.
pub trait Fence: Send + Sync {
    fn check(&self, context: &Context, binding: &[u8; 32], now: DateTime<Utc>)
        -> Result<(), Error>;
}
#[async_trait]
pub trait Authority: Send + Sync {
    async fn acquire(
        &self,
        context: &Context,
        access: &Access,
        binding: [u8; 32],
    ) -> Result<Box<dyn Fence>, Error>;
}
pub struct DenyAuthority;
#[async_trait]
impl Authority for DenyAuthority {
    async fn acquire(&self, _: &Context, _: &Access, _: [u8; 32]) -> Result<Box<dyn Fence>, Error> {
        Err(Error::Authority)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Filter {
    All,
    WithMedia,
    PartyLink,
    GiveawayLink,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub created_at: DateTime<Utc>,
    pub id: Uuid,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Window {
    pub upper: DateTime<Utc>,
    pub after: Option<Position>,
    pub filter: Filter,
    pub limit: u32,
}
impl Window {
    pub fn validate(&self) -> Result<(), Error> {
        if !(1..=100).contains(&self.limit)
            || self
                .after
                .is_some_and(|p| p.id.is_nil() || p.created_at > self.upper)
        {
            Err(Error::Invalid)
        } else {
            Ok(())
        }
    }
}
#[derive(Clone)]
pub struct PostView {
    pub post: Post,
    pub reacted_by_me: bool,
}
/// Internal continuation is a typed scan boundary. Concert owns opaque cursor
/// signing and actor/event/filter binding. No inaccessible row IDs go to browser.
pub struct Page<T> {
    pub items: Vec<T>,
    pub scanned_through: Option<Position>,
    pub has_more: bool,
}
