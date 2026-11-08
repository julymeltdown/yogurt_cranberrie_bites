//! The profile producer coverage register.
//!
//! The contract freezes 43 profile-owned operations: the 41 rows the freeze
//! record enumerates (`docs/mcr/integration/contract-freeze.md` §7: 10 core + 31
//! domain), plus `createIntroPostComment` and `deleteIntroPostComment`, which
//! commit `c79ed25c1` added after that record was written. This
//! module is the single place that says, for each of them, whether a producer
//! exists and which work package owns it.
//!
//! Why a register rather than just the route parser: a closed route parser tells
//! you what it refuses, but not whether a refusal is *expected*. Without this
//! table, an operation that a later work package forgot to wire would look
//! exactly like one that is deliberately not built yet, and nothing would catch
//! it. [`COVERAGE`] makes the difference explicit, and
//! `tests/profile_producer_coverage.rs` asserts that it agrees with the frozen
//! contract in both directions.
//!
//! Three rules this module encodes:
//!
//! * **No fabricated producers.** An operation is [`Producer::Served`] only when
//!   a real module answers it. Everything else names the work package that will
//!   build it, so the gap is tracked rather than hidden.
//! * **No silent omission.** The register and the contract are compared by
//!   operation id, and a mismatch in either direction fails the test.
//! * **The event rule is not negotiable.** Every entry's path is checked to be
//!   free of an event segment; a profile operation never carries one.

/// Who answers an operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Producer {
    /// A real producer exists and the route parser accepts the operation.
    Served,
    /// No producer exists yet. The route parser refuses the operation and the
    /// gateway does not mount it. `owner` names the work package that will
    /// build it.
    NotImplemented,
}

/// One row of the register.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Coverage {
    /// The frozen `operationId`.
    pub operation: &'static str,
    /// The frozen HTTP method, upper case.
    pub method: &'static str,
    /// The frozen path template, exactly as the contract spells it.
    pub path: &'static str,
    pub producer: Producer,
    /// The module that answers the operation, or the work package that owes it.
    pub owner: &'static str,
}

const fn served(operation: &'static str, method: &'static str, path: &'static str, owner: &'static str) -> Coverage {
    Coverage {
        operation,
        method,
        path,
        producer: Producer::Served,
        owner,
    }
}

const fn owed(operation: &'static str, method: &'static str, path: &'static str, owner: &'static str) -> Coverage {
    Coverage {
        operation,
        method,
        path,
        producer: Producer::NotImplemented,
        owner,
    }
}

/// The 41 frozen profile-owned operations, in contract order.
///
/// "Profile-owned" means the path does not start with `/api/concert/events/`.
/// The 8 `/api/auth/` operations in the core register are **not** profile-owned:
/// they belong to the Auth service and are served by its own routes, so they are
/// outside this register by construction rather than by omission. The
/// distinction is asserted in the coverage test, which recomputes the
/// profile-owned set from the contract and compares it with this table.
pub const COVERAGE: &[Coverage] = &[
    // ── §2.1 profile (core) ────────────────────────────────────────────────
    served("getOwnProfile", "GET", "/api/me/profile", "profile_product::targets"),
    owed("updateOwnProfile", "PATCH", "/api/me/profile", "WP-04 self-profile pipeline"),
    owed("setOwnFavorites", "PUT", "/api/me/profile/favorites", "WP-03 favorites command"),
    served("getProfile", "GET", "/api/profiles/{profileId}", "profile_product::targets"),
    served("searchCatalog", "GET", "/api/catalog/{category}/search", "profile_service::catalog"),
    // The discovery tables are a concert-service projection (migration 0086).
    owed("discoverProfiles", "GET", "/api/discover/people", "WP-03 discovery (concert-service projection)"),
    // ── §2.1b identity verification (core) ─────────────────────────────────
    served("getOwnIdentityVerification", "GET", "/api/me/identity/{provider}", "profile_service::identity"),
    served("startIdentityVerification", "POST", "/api/me/identity/{provider}/verifications", "profile_service::identity"),
    owed("completeIdentityVerification", "GET", "/api/identity/{provider}/callback", "WP-04b callback transport"),
    served("disconnectIdentityVerification", "DELETE", "/api/me/identity/{provider}", "profile_service::identity"),
    // ── §2.2 intro posts (domain) ──────────────────────────────────────────
    owed("listIntroPosts", "GET", "/api/discover/intro-posts", "WP-05 intro posts"),
    owed("getIntroPost", "GET", "/api/intro-posts/{introId}", "WP-05 intro posts"),
    owed("getOwnIntroPost", "GET", "/api/me/intro-posts/{introId}", "WP-05 intro posts"),
    owed("listOwnIntroPosts", "GET", "/api/me/intro-posts", "WP-05 intro posts"),
    owed("saveIntroPostDraft", "PUT", "/api/me/intro-posts/{introId}", "WP-05 intro posts"),
    owed("publishIntroPost", "POST", "/api/me/intro-posts/{introId}/publish", "WP-05 intro posts"),
    owed("deleteIntroPost", "DELETE", "/api/me/intro-posts/{introId}", "WP-05 intro posts"),
    owed("setIntroPostLike", "PUT", "/api/intro-posts/{introId}/like", "WP-05 intro posts"),
    owed("clearIntroPostLike", "DELETE", "/api/intro-posts/{introId}/like", "WP-05 intro posts"),
    owed("listIntroPostLikers", "GET", "/api/intro-posts/{introId}/likes", "WP-05 intro posts"),
    owed("listIntroPostComments", "GET", "/api/intro-posts/{introId}/comments", "WP-05 intro posts"),
    // Added by commit c79ed25c1 after the freeze record was written; the freeze
    // document's own reconciliation section notes they were "subsequently
    // added". They are profile-owned by path and are tracked here so the
    // register stays complete.
    owed("createIntroPostComment", "POST", "/api/intro-posts/{introId}/comments", "WP-05 intro posts"),
    owed("deleteIntroPostComment", "DELETE", "/api/intro-posts/{introId}/comments/{commentId}", "WP-05 intro posts"),
    // ── §2.3 profile posts and media (domain) ──────────────────────────────
    // WP-06 built these producers, but they read `profile_posts`,
    // `profile_post_attachments` and `profile_media`, which live in the CONCERT
    // database. The profile producer runs against `mcr_profile` and cannot
    // reach them. See `CROSS_SERVICE` below.
    owed("listProfilePosts", "GET", "/api/profiles/{profileId}/posts", "WP-06 profile_posts (concert-service)"),
    owed("createProfilePost", "POST", "/api/me/posts", "WP-06 profile_posts (concert-service)"),
    owed("deleteProfilePost", "DELETE", "/api/me/posts/{postId}", "WP-06 profile_posts (concert-service)"),
    owed("initProfileMediaUpload", "POST", "/api/me/media/uploads", "WP-06 profile_media (concert-service)"),
    owed("putProfileMediaBytes", "PUT", "/api/me/media/{mediaId}/bytes", "WP-06 profile_media (concert-service)"),
    owed("finalizeProfileMedia", "POST", "/api/me/media/{mediaId}/finalize", "WP-06 profile_media (concert-service)"),
    owed("getProfileMedia", "GET", "/api/media/{mediaId}", "WP-06 profile_media (concert-service)"),
    owed("readProfileMedia", "GET", "/api/media/{mediaId}/content", "WP-06 profile_media (concert-service)"),
    // ── §2.4 room (domain) ─────────────────────────────────────────────────
    owed("getProfileRoom", "GET", "/api/profiles/{profileId}/room", "WP-07 profile_room (concert-service)"),
    owed("getOwnRoom", "GET", "/api/me/room", "WP-07 profile_room (concert-service)"),
    owed("saveOwnRoom", "PUT", "/api/me/room", "WP-07 profile_room (concert-service)"),
    owed("setOwnRoomVisibility", "PATCH", "/api/me/room/visibility", "WP-07 profile_room (concert-service)"),
    owed("uploadRoomPoster", "POST", "/api/me/room/posters", "WP-07 profile_room (concert-service)"),
    // ── §2.5 avatar (domain) ───────────────────────────────────────────────
    owed("getOwnProfileAvatar", "GET", "/api/me/avatar", "WP-08 profile_avatar (concert-service)"),
    owed("setOwnProfileAvatar", "PUT", "/api/me/avatar", "WP-08 profile_avatar (concert-service)"),
    // ── §2.6 activity and notifications (domain) ───────────────────────────
    owed("listActivity", "GET", "/api/me/activity", "WP-05 activity feed"),
    owed("listNotifications", "GET", "/api/me/notifications", "WP-05 notification inbox"),
    owed("markNotificationRead", "PATCH", "/api/me/notifications/{notificationId}", "WP-05 notification inbox"),
    // ── §2.7 follow (domain) ───────────────────────────────────────────────
    owed("followProfile", "PUT", "/api/profiles/{profileId}/follow", "WP-03 follow"),
    owed("unfollowProfile", "DELETE", "/api/profiles/{profileId}/follow", "WP-03 follow"),
];

/// Why a producer that exists is still not reachable from this service.
///
/// This is the finding that makes the register honest. WP-06, WP-07 and WP-08
/// each built a real producer, and each reported the same gap: the profile
/// producer parses only the operations it can answer. The reason is not a
/// missing module — it is the deployment topology.
///
/// The profile service and the concert service are separate processes with
/// separate databases (`mcr_profile` and `mcr_concert`). The tables those
/// producers read — `profile_posts`, `profile_post_attachments`,
/// `profile_media`, `profile_rooms`, `profile_avatars` — are created by
/// CONCERT-service migrations and exist only in `mcr_concert`. A producer
/// running inside the profile service cannot query them, and a cross-database
/// join is exactly what the domain contract forbids
/// (`domain-contract.md` §1.1: "No cross-database join").
///
/// Serving these operations therefore needs one of two things, and neither is a
/// change to this file:
///
/// 1. a concert-service product transport that dispatches these routes to the
///    modules that already implement them, with the gateway routing them to the
///    concert service; or
/// 2. a documented cross-service call from the profile producer to the concert
///    service's gRPC surface, with its own authority scope.
///
/// Both are composition decisions with an owner outside this work package. This
/// register records the gap rather than papering over it with a producer that
/// would answer an operation it cannot actually serve.
pub const CROSS_SERVICE_NOTE: &str = "producer exists in the concert service; the tables it reads live in mcr_concert, not mcr_profile";

/// The register row for an operation id, if the register knows it.
pub fn find(operation: &str) -> Option<&'static Coverage> {
    COVERAGE.iter().find(|row| row.operation == operation)
}

/// Every operation the producer serves.
pub fn served_operations() -> impl Iterator<Item = &'static Coverage> {
    COVERAGE
        .iter()
        .filter(|row| row.producer == Producer::Served)
}

/// Every operation with no producer yet.
pub fn not_implemented() -> impl Iterator<Item = &'static Coverage> {
    COVERAGE
        .iter()
        .filter(|row| row.producer == Producer::NotImplemented)
}

/// Counts for reporting: `(served, not_implemented)`.
pub fn counts() -> (usize, usize) {
    (
        served_operations().count(),
        not_implemented().count(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_register_covers_exactly_forty_one_operations() {
        assert_eq!(COVERAGE.len(), 41, "the freeze declares 41 profile operations");
    }

    #[test]
    fn every_register_path_is_event_free() {
        for row in COVERAGE {
            assert!(
                !row.path.contains("/events/"),
                "{} must not carry an event path",
                row.operation
            );
            assert!(
                row.path.starts_with("/api/"),
                "{} must be rooted at /api",
                row.operation
            );
        }
    }

    #[test]
    fn operation_ids_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for row in COVERAGE {
            assert!(seen.insert(row.operation), "duplicate {}", row.operation);
        }
    }

    #[test]
    fn every_row_names_an_owner() {
        for row in COVERAGE {
            assert!(!row.owner.is_empty(), "{} has no owner", row.operation);
        }
    }
}
