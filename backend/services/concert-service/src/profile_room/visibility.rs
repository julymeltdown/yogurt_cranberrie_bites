//! WP-07 T-603: room visibility.
//!
//! A room is `PUBLIC` or `PRIVATE`. The rules are:
//!
//! * a profile that has never chosen starts `PUBLIC` — the frozen
//!   `getProfileRoom` operation is declared `public_read`, and product-plan.md §6
//!   says authorized visitors see the saved room while the owner is offline;
//! * a `PRIVATE` room is `404 RESOURCE_UNAVAILABLE` for every visitor, exactly as
//!   a room that does not exist, so the endpoint cannot be used to probe whether a
//!   profile has a room;
//! * the owner always reads their own room, whatever its visibility;
//! * the profile projection reports `room_available` from the same decision, so
//!   the profile and the room endpoint cannot disagree.
//!
//! # Independence from intro posts
//!
//! Room visibility and an intro post's audience are two unrelated decisions. This
//! module never reads `intro_posts` and never writes it: the visibility statement
//! touches `profile_rooms.visibility` only. The test
//! `changing_room_visibility_leaves_every_intro_audience_unchanged` and its mirror
//! `changing_an_intro_audience_leaves_room_visibility_unchanged` prove both
//! directions against a real database.
use super::repository::{RoomActor, RoomErrorKind, RoomPersistence};
use concert_domain::profile_room::{ProfileId, RoomVisibility};

/// Whether `viewer` may open `target`'s room, given the stored visibility.
///
/// This is the pure decision; [`RoomPersistence::read_for_visitor`] applies it
/// inside the read so a surface cannot forget it.
pub fn can_view_room(viewer: ProfileId, target: ProfileId, visibility: RoomVisibility) -> bool {
    viewer == target || visibility == RoomVisibility::Public
}

/// The `room_available` value the profile projection reports.
///
/// It is derived from the same decision as the room endpoint: a visitor of a
/// private room sees `false` here *and* `404` there, never one without the other.
pub fn room_available(viewer: ProfileId, target: ProfileId, visibility: RoomVisibility) -> bool {
    can_view_room(viewer, target, visibility)
}

/// The stored visibility of a profile's room, defaulting to the product default
/// for a profile that has never chosen.
pub async fn visibility_of(
    persistence: &RoomPersistence,
    target: ProfileId,
) -> Result<RoomVisibility, RoomErrorKind> {
    persistence.stored_visibility(target).await
}

/// `PATCH /api/me/room/visibility`.
///
/// The owner is the authenticated actor. The command carries the visibility and
/// the expected revision; there is no audience field, and the statement that
/// applies it names no intro post.
pub async fn set_visibility(
    persistence: &RoomPersistence,
    actor: RoomActor,
    body: &[u8],
) -> Result<serde_json::Value, RoomErrorKind> {
    persistence.set_visibility(actor, body).await
}
