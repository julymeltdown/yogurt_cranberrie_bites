//! WP-08: profile-owned avatar.
//!
//! The emo character used to be stored per concert participant. A personal
//! profile outlives any concert, so the avatar is re-keyed to profile ownership
//! here. Nothing in this module accepts a concert identifier: the owner is the
//! authenticated profile, and every operation is reachable only through the two
//! frozen profile routes `GET /api/me/avatar` and `PUT /api/me/avatar`.
pub mod http;
pub mod repository;

pub use http::{
    handle, names_event_path, route, ProfileAvatarRefusal, ProfileAvatarResponse,
    ProfileAvatarRoute, AVATAR_PATH, ROUTES,
};
pub use repository::{
    http_reason, http_status, ProfileAvatarActor, ProfileAvatarError, ProfileAvatarPersistence,
    ProfileAvatarState,
};
