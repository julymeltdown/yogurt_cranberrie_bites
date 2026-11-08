//! WP-07: the personal room.
//!
//! Every profile has exactly one personal room: a fixed pixel-art space with
//! furniture and up to four pixel-art posters converted from user images. The
//! room is **subordinate to the profile** and completely independent of every
//! mutuals-intro post. Nothing in this module can name an intro post or a concert
//! event; the owner is the authenticated profile.
//!
//! * [`repository`] — persistence, revision compare-and-set, poster storage (T-602,
//!   T-604);
//! * [`visibility`] — the public/private decision and the `room_available`
//!   projection (T-603);
//! * [`http`] — the closed five-operation route table (T-605).
//!
//! The frozen operations are exactly the five of http-contract.md §2.4. The room
//! has no event path, no participant key and no intro reference, and it has no
//! presence or "online" notion: a character standing in a room is a decoration of
//! the room, never an observation that the owner is connected.
pub mod http;
pub mod repository;
pub mod visibility;

pub use http::{
    handle, names_event_path, names_intro_path, route, RoomRefusal, RoomResponse, RoomRoute,
    OWN_ROOM_PATH, POSTERS_PATH, PROFILE_ROOM_PREFIX, PROFILE_ROOM_SUFFIX, ROUTES, VISIBILITY_PATH,
};
pub use repository::{
    conflict_body, furniture_id, furniture_ids, http_reason, http_status, validate_png, RoomActor,
    RoomErrorKind, RoomPersistence,
};
pub use visibility::{can_view_room, room_available, set_visibility, visibility_of};
