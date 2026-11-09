//! Profile-owned product transport hosted by the CONCERT service.
//!
//! ## Why this module exists
//!
//! Fifteen frozen profile operations read tables that live in the concert
//! database: `profile_posts`, `profile_post_attachments`, `profile_media`,
//! `profile_rooms` and `profile_avatars`. Those tables are concert-service
//! migrations, the profile service runs against a different database
//! (`mcr_profile`), and `domain-contract.md` §1.1 forbids a cross-database join.
//!
//! The producers for those operations already exist here, with closed route
//! parsers and `handle` entry points. This module is their transport home: it
//! mounts them under the same authority discipline the profile product transport
//! uses, and the Gateway routes each profile operation to the service that owns
//! its tables.
//!
//! ## Why not `concert.product.v1`
//!
//! The concert product transport is event-scoped by construction: `event_id`
//! must be non-nil and every target must start with
//! `/api/concert/events/{event}/`. A profile operation has no event. Reusing that
//! transport would require inventing one, which is exactly the "silent product
//! decision" the integration forbids. This transport is the profile analogue:
//! the same message shape and the same delivery/settlement discipline, keyed by
//! `profile_id` and refusing any event path.
//!
//! ## The authority discipline, preserved
//!
//! Calling a persistence function with a UUID supplies none of these, so all of
//! them are enforced here before the producer runs:
//!
//! * the caller is the verified Gateway workload presenting a verified actor
//!   delegation;
//! * the signed parent frame is re-decoded and compared byte for byte with the
//!   message received, so a substituted body cannot reach a producer;
//! * the actor in the message must equal the delegated actor and the actor the
//!   original deadline proof names;
//! * the profile identity must be a canonical non-nil UUID;
//! * the target must be rooted at `/api/` and must never name an event;
//! * a mutation must carry a canonical idempotency key, and a read must carry no
//!   body, no content type and no key;
//! * observations must be fresh and the authority window bounded;
//! * one owner scope is acquired and a delivery capability published *before*
//!   any producer work, and the response is only released with that capability;
//! * the scope is settled by a separate system operation after the Gateway's
//!   actual HTTP body terminal, never by returning from the producer.
pub mod delivery;
pub mod guard;
pub mod intro;
pub mod ledger;
pub mod retained;
pub mod scoped;
pub mod targets;
pub mod transport;

pub mod proto {
    tonic::include_proto!("concert.profile_product.v1");
}

pub use guard::{ProfileOwnerFence, ProfileOwnerScope, ProfileScopeRegistry};
pub use intro::{IntroProducer, NOT_SERVED as INTRO_NOT_SERVED, SERVED as INTRO_SERVED};
pub use ledger::ProfileScopeLedger;
pub use scoped::{ProfileRepresentation, ScopedProfileOwner, ScopedProfileProducer};
pub use targets::{parse, ProfileOperation, ProfileProductProducer};
pub use transport::{
    authorize, retain_scopes, ProfileProductOwner, ProfileProductRpcService,
    ProfileProductRuntime, VerifiedProfile, BUDGET, DELIVERY_POLICY, MAX_BODY, MAX_FRAME, POLICY,
};

/// Canonical lowercase non-nil UUID. Same rule as every other product producer.
pub fn canonical_uuid(value: &str) -> Result<uuid::Uuid, tonic::Status> {
    transport::canonical_uuid(value)
}

/// The event-path guard. A profile operation never carries an event path, so any
/// target naming one is a caller error, not an authorization decision.
pub fn rejects_event_path(target: &str) -> bool {
    transport::rejects_event_path(target)
}

/// Build the concert-hosted profile product transport over a real database pool.
pub fn runtime(
    pool: sqlx::PgPool,
    callers: platform::grpc_auth::CallerRegistry,
    verifier: platform::actor_delegation::ActorDelegationVerifier,
    media: Option<std::sync::Arc<crate::profile_media::ProfileMediaStore>>,
) -> anyhow::Result<ProfileProductRuntime> {
    transport::runtime(pool, callers, verifier, media)
}
