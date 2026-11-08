//! WP-06 T-505 / WP-04 T-307: who may read which profile media.
//!
//! The frozen rule (http-contract.md §4.1, verification V-06-f) is that a media
//! asset is readable **through the post it is attached to**, and only by a
//! viewer who may see that post. Media is not its own visibility boundary: an
//! asset that no live post references is readable by nobody but its owner.
//!
//! This module is the pure decision. [`can_view_media`] takes the facts a query
//! has already established and returns a decision; it performs no I/O and knows
//! nothing about SQL. That is deliberate: the same function is used by the
//! metadata read, the byte read and the gallery projection, so the three cannot
//! disagree, and it can be tested exhaustively without a database.
//!
//! The predicate is *deny by default*. Every fact is required; a missing fact is
//! not "probably fine".
use uuid::Uuid;

/// The frozen profile media read path (`readProfileMedia`).
///
/// It has no event segment and no participant: a profile photograph is read from
/// the profile surface, never from a concert one.
pub fn media_read_path(media_id: Uuid) -> String {
    format!("/api/media/{media_id}/content")
}

/// The viewer of a profile media read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MediaViewer {
    /// The authenticated profile. Never taken from a request body.
    pub profile_id: Uuid,
}

/// What the asset is attached to, as the query found it.
///
/// A profile photograph is attached to a profile post; an intro attachment is
/// attached to an intro post; a profile image is attached to the profile itself;
/// a room poster is attached to the room. The enum makes "attached to nothing"
/// and "attached to something the viewer cannot see" different cases, because
/// they must produce the same answer but for different reasons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaVisibility {
    /// A `READY` asset attached to a live `PUBLIC` post.
    PublicPost,
    /// A `READY` asset attached to a live `PRIVATE` post.
    PrivatePost,
    /// A `READY` asset attached to a live intro post whose audience the viewer
    /// satisfies. The audience decision is made by the intro visibility rule and
    /// is passed in as this variant; this module never re-derives it.
    VisibleIntro,
    /// A `READY` asset attached to an intro post the viewer may not see.
    RestrictedIntro,
    /// A `READY` asset attached to a `PUBLIC` profile as its profile image.
    PublicProfileImage,
    /// A `READY` asset attached to a `PRIVATE` profile as its profile image.
    PrivateProfileImage,
    /// A `READY` asset attached to a `PUBLIC` room as a poster.
    PublicRoomPoster,
    /// A `READY` asset attached to a `PRIVATE` room as a poster.
    PrivateRoomPoster,
    /// A `READY` asset attached to nothing at all: a fresh upload that was never
    /// published. Readable by its owner only.
    Unattached,
    /// The asset is not `READY`, is `DELETED`, or does not exist. Never readable.
    Absent,
}

impl MediaVisibility {
    /// Whether the asset is attached to a live, published post of some kind.
    pub fn attached(self) -> bool {
        !matches!(self, Self::Unattached | Self::Absent)
    }
}

/// The facts one media read decision needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProfileMediaRead {
    /// The profile that owns the asset.
    pub owner_profile_id: Uuid,
    /// The asset's stored purpose, as the closed profile set.
    pub purpose: concert_application::media::Purpose,
    /// What the asset is attached to and whether the viewer may see it.
    pub visibility: MediaVisibility,
}

/// The single media read decision (E-03 metadata, E-04 bytes).
///
/// The asset is readable when, and only when:
///
/// 1. it is `READY` and attached to something (`Absent` is never readable);
/// 2. the viewer is the owner, **or** the attachment is visible to the viewer;
/// 3. a `PROFILE_POST` asset is never readable through an intro attachment and an
///    `INTRO_POST` asset is never readable through a profile post. The purpose is
///    part of the decision so a mis-associated row cannot leak across surfaces.
///
/// The owner always sees their own asset, including a private post's photograph:
/// privacy is a rule about other people.
pub fn can_view_media(viewer: MediaViewer, media: &ProfileMediaRead) -> bool {
    use concert_application::media::Purpose;
    if viewer.profile_id.is_nil() || media.owner_profile_id.is_nil() {
        return false;
    }
    if media.visibility == MediaVisibility::Absent {
        return false;
    }
    // The purpose must agree with the surface the asset was found through.
    let consistent = match media.visibility {
        MediaVisibility::PublicPost | MediaVisibility::PrivatePost => {
            media.purpose == Purpose::ProfilePost
        }
        MediaVisibility::VisibleIntro | MediaVisibility::RestrictedIntro => {
            media.purpose == Purpose::IntroPost
        }
        MediaVisibility::PublicProfileImage | MediaVisibility::PrivateProfileImage => {
            media.purpose == Purpose::ProfileImage
        }
        MediaVisibility::PublicRoomPoster | MediaVisibility::PrivateRoomPoster => {
            media.purpose == Purpose::RoomPoster
        }
        MediaVisibility::Unattached => media.purpose.is_profile(),
        MediaVisibility::Absent => false,
    };
    if !consistent {
        return false;
    }
    if viewer.profile_id == media.owner_profile_id {
        return true;
    }
    matches!(
        media.visibility,
        MediaVisibility::PublicPost
            | MediaVisibility::VisibleIntro
            | MediaVisibility::PublicProfileImage
            | MediaVisibility::PublicRoomPoster
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use concert_application::media::Purpose;

    fn owner() -> Uuid {
        Uuid::from_u128(0x1111_1111_1111_4111_8111_1111_1111_1111)
    }

    fn viewer() -> Uuid {
        Uuid::from_u128(0x2222_2222_2222_4222_8222_2222_2222_2222)
    }

    fn read(purpose: Purpose, visibility: MediaVisibility) -> ProfileMediaRead {
        ProfileMediaRead {
            owner_profile_id: owner(),
            purpose,
            visibility,
        }
    }

    #[test]
    fn an_owner_sees_every_attached_asset_of_their_own() {
        for (purpose, visibility) in [
            (Purpose::ProfilePost, MediaVisibility::PublicPost),
            (Purpose::ProfilePost, MediaVisibility::PrivatePost),
            (Purpose::IntroPost, MediaVisibility::RestrictedIntro),
            (Purpose::IntroPost, MediaVisibility::VisibleIntro),
            (Purpose::ProfileImage, MediaVisibility::PublicProfileImage),
            (Purpose::ProfileImage, MediaVisibility::PrivateProfileImage),
            (Purpose::RoomPoster, MediaVisibility::PublicRoomPoster),
            (Purpose::RoomPoster, MediaVisibility::PrivateRoomPoster),
            (Purpose::ProfilePost, MediaVisibility::Unattached),
        ] {
            assert!(
                can_view_media(MediaViewer { profile_id: owner() }, &read(purpose, visibility)),
                "{purpose:?} {visibility:?}"
            );
        }
    }

    #[test]
    fn a_visitor_sees_only_public_attachments() {
        let visitor = MediaViewer {
            profile_id: viewer(),
        };
        for (purpose, visibility, expected) in [
            (Purpose::ProfilePost, MediaVisibility::PublicPost, true),
            (Purpose::ProfilePost, MediaVisibility::PrivatePost, false),
            (Purpose::IntroPost, MediaVisibility::VisibleIntro, true),
            (Purpose::IntroPost, MediaVisibility::RestrictedIntro, false),
            (Purpose::ProfileImage, MediaVisibility::PublicProfileImage, true),
            (Purpose::ProfileImage, MediaVisibility::PrivateProfileImage, false),
            (Purpose::RoomPoster, MediaVisibility::PublicRoomPoster, true),
            (Purpose::RoomPoster, MediaVisibility::PrivateRoomPoster, false),
            (Purpose::ProfilePost, MediaVisibility::Unattached, false),
        ] {
            assert_eq!(
                can_view_media(visitor, &read(purpose, visibility)),
                expected,
                "{purpose:?} {visibility:?}"
            );
        }
    }

    #[test]
    fn nothing_absent_is_readable_by_anyone() {
        for purpose in Purpose::PROFILE {
            for profile_id in [owner(), viewer()] {
                assert!(!can_view_media(
                    MediaViewer { profile_id },
                    &read(purpose, MediaVisibility::Absent)
                ));
            }
        }
    }

    #[test]
    fn a_purpose_never_crosses_a_surface() {
        // An intro asset found through a profile post, and the reverse, is
        // refused even for the owner: the association is not a permission.
        assert!(!can_view_media(
            MediaViewer { profile_id: owner() },
            &read(Purpose::IntroPost, MediaVisibility::PublicPost)
        ));
        assert!(!can_view_media(
            MediaViewer { profile_id: owner() },
            &read(Purpose::ProfilePost, MediaVisibility::VisibleIntro)
        ));
        assert!(!can_view_media(
            MediaViewer { profile_id: owner() },
            &read(Purpose::RoomPoster, MediaVisibility::PublicPost)
        ));
    }

    #[test]
    fn a_nil_viewer_or_owner_is_never_authorized() {
        let mut media = read(Purpose::ProfilePost, MediaVisibility::PublicPost);
        assert!(!can_view_media(
            MediaViewer {
                profile_id: Uuid::nil()
            },
            &media
        ));
        media.owner_profile_id = Uuid::nil();
        assert!(!can_view_media(
            MediaViewer {
                profile_id: Uuid::nil()
            },
            &media
        ));
    }
}
