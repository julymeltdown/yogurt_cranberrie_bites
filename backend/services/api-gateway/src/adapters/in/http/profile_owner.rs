//! Which service owns a profile operation's tables.
//!
//! The integration splits profile-owned data across two databases:
//! `mcr_profile` (profiles, favorites, identity claims, catalog) and
//! `mcr_concert` (`profile_posts`, `profile_post_attachments`, `profile_media`,
//! `profile_rooms`, `profile_avatars`). `domain-contract.md` §1.1 forbids a
//! cross-database join, so the Gateway must send each profile operation to the
//! service that owns its tables rather than to one "profile" service.
//!
//! This module is that rule, and it is deliberately a **table**, not a
//! `match` arm buried in a handler. Three properties follow from making it data:
//!
//! * the routing decision is visible in one place, so a reviewer can check it
//!   against the migration that created each table;
//! * a test can enumerate every mounted profile route and assert that each one
//!   has an owner, and that the owner agrees with the operation's own coverage
//!   register;
//! * an operation with no owner is a compile-time-visible gap rather than a
//!   silent fallback to whichever client happened to be constructed.
//!
//! The rule is not "which service is named `profile`": it is **which database
//! holds the tables the operation reads**. That is why `getOwnRoom` goes to the
//! concert service even though it is a profile operation, and why
//! `getOwnProfile` stays in the profile service even though both are mounted on
//! the same `/api/me/` prefix.

/// The service that owns a profile operation's tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProfileOwner {
    /// `mcr_profile`: profiles, favorites, identity claims, catalog.
    ProfileService,
    /// `mcr_concert`: profile posts, profile media, rooms, avatars.
    ConcertService,
}

impl ProfileOwner {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProfileService => "profile-service",
            Self::ConcertService => "concert-service",
        }
    }
}

/// One routing row: the frozen operation id and the service that owns it.
///
/// Kept as `(operation, owner)` pairs rather than a derived function so that a
/// new operation cannot acquire an owner by accident. The coverage test in
/// `profile-service` asserts the same partition from the other side.
pub const PROFILE_OWNERS: &[(&str, ProfileOwner)] = &[
    // ── mcr_profile ────────────────────────────────────────────────────────
    ("getOwnProfile", ProfileOwner::ProfileService),
    ("updateOwnProfile", ProfileOwner::ProfileService),
    ("setOwnFavorites", ProfileOwner::ProfileService),
    ("getProfile", ProfileOwner::ProfileService),
    ("searchCatalog", ProfileOwner::ProfileService),
    ("discoverProfiles", ProfileOwner::ProfileService),
    ("getOwnIdentityVerification", ProfileOwner::ProfileService),
    ("startIdentityVerification", ProfileOwner::ProfileService),
    ("disconnectIdentityVerification", ProfileOwner::ProfileService),
    // ── mcr_concert ────────────────────────────────────────────────────────
    // `profile_posts` (0084), `profile_post_attachments` (0084).
    ("listProfilePosts", ProfileOwner::ConcertService),
    ("createProfilePost", ProfileOwner::ConcertService),
    ("deleteProfilePost", ProfileOwner::ConcertService),
    // `profile_media` (0080).
    ("initProfileMediaUpload", ProfileOwner::ConcertService),
    ("putProfileMediaBytes", ProfileOwner::ConcertService),
    ("finalizeProfileMedia", ProfileOwner::ConcertService),
    ("getProfileMedia", ProfileOwner::ConcertService),
    ("readProfileMedia", ProfileOwner::ConcertService),
    // `profile_rooms` (0081).
    ("getProfileRoom", ProfileOwner::ConcertService),
    ("getOwnRoom", ProfileOwner::ConcertService),
    ("saveOwnRoom", ProfileOwner::ConcertService),
    ("setOwnRoomVisibility", ProfileOwner::ConcertService),
    ("uploadRoomPoster", ProfileOwner::ConcertService),
    // `profile_avatars` (0087).
    ("getOwnProfileAvatar", ProfileOwner::ConcertService),
    ("setOwnProfileAvatar", ProfileOwner::ConcertService),
    // `intro_posts`, `intro_attachments`, `intro_likes`, `intro_comments` (0082,
    // 0083). Only the operations WP-05's producer actually serves are listed;
    // The two discovery reads, served by WP-04's visibility predicate and the
    // page/detail queries over `intro_posts`.
    ("listIntroPosts", ProfileOwner::ConcertService),
    ("getIntroPost", ProfileOwner::ConcertService),
    ("getOwnIntroPost", ProfileOwner::ConcertService),
    ("listOwnIntroPosts", ProfileOwner::ConcertService),
    ("saveIntroPostDraft", ProfileOwner::ConcertService),
    ("publishIntroPost", ProfileOwner::ConcertService),
    ("deleteIntroPost", ProfileOwner::ConcertService),
    ("setIntroPostLike", ProfileOwner::ConcertService),
    ("clearIntroPostLike", ProfileOwner::ConcertService),
    ("listIntroPostLikers", ProfileOwner::ConcertService),
    ("listIntroPostComments", ProfileOwner::ConcertService),
    ("createIntroPostComment", ProfileOwner::ConcertService),
    ("deleteIntroPostComment", ProfileOwner::ConcertService),
];

/// The owner of one operation, or `None` when the operation is not mounted.
///
/// `None` is not a fallback: a caller that receives it must refuse the request.
/// Returning a default owner here would route an unknown operation to a service
/// that cannot answer it, which is exactly the failure this module prevents.
pub fn owner_of(operation: &str) -> Option<ProfileOwner> {
    PROFILE_OWNERS
        .iter()
        .find(|(id, _)| *id == operation)
        .map(|(_, owner)| *owner)
}

/// Every operation routed to one service.
pub fn operations_for(owner: ProfileOwner) -> impl Iterator<Item = &'static str> {
    PROFILE_OWNERS
        .iter()
        .filter(move |(_, routed)| *routed == owner)
        .map(|(operation, _)| *operation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_operation_has_exactly_one_owner() {
        let mut seen = std::collections::BTreeSet::new();
        for (operation, _) in PROFILE_OWNERS {
            assert!(
                seen.insert(*operation),
                "{operation} has more than one owner"
            );
        }
    }

    #[test]
    fn the_concert_owned_set_is_the_cross_database_set() {
        // The fifteen operations whose tables live in `mcr_concert`. Pinned so a
        // later change has to update both this list and the migrations that
        // justify it.
        let concert: std::collections::BTreeSet<&str> = operations_for(ProfileOwner::ConcertService).collect();
        assert_eq!(
            concert,
            std::collections::BTreeSet::from([
                "listProfilePosts",
                "createProfilePost",
                "deleteProfilePost",
                "initProfileMediaUpload",
                "putProfileMediaBytes",
                "finalizeProfileMedia",
                "getProfileMedia",
                "readProfileMedia",
                "getProfileRoom",
                "getOwnRoom",
                "saveOwnRoom",
                "setOwnRoomVisibility",
                "uploadRoomPoster",
                "getOwnProfileAvatar",
                "setOwnProfileAvatar",
                // WP-05 intro operations served by the concert transport.
                "listIntroPosts",
                "getIntroPost",
                "getOwnIntroPost",
                "listOwnIntroPosts",
                "saveIntroPostDraft",
                "publishIntroPost",
                "deleteIntroPost",
                "setIntroPostLike",
                "clearIntroPostLike",
                "listIntroPostLikers",
                "listIntroPostComments",
                "createIntroPostComment",
                "deleteIntroPostComment",
            ])
        );
        assert_eq!(concert.len(), 28);
    }

    #[test]
    fn an_unknown_operation_has_no_owner() {
        assert_eq!(owner_of("notAnOperation"), None);
        assert_eq!(owner_of(""), None);
        // And a concert operation is not accidentally profile-owned.
        assert_eq!(owner_of("getOwnRoom"), Some(ProfileOwner::ConcertService));
        assert_eq!(owner_of("getOwnProfile"), Some(ProfileOwner::ProfileService));
    }
}
