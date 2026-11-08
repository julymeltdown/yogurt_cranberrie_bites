//! WP-06: profile posts — ordinary selfies, photographs and artwork.
//!
//! A profile post is *not* an intro post and has nothing to do with one. It has
//! no `intro_id`, no audience, no room reference and no expiry requirement; its
//! own visibility is the only privacy control it has. That independence is the
//! product plan's hard rule 1–3 and is what the tests in
//! `tests/profile_post_lifecycle.rs` verify in both directions.
//!
//! Modules:
//!
//! * [`commands`] — create and delete (T-502);
//! * [`read`] — the visitor list, the author list and the direct read (T-503);
//! * [`http`] — the closed route parser for the three post operations and the
//!   five media operations (T-504).
pub mod commands;
pub mod http;
pub mod read;

pub use commands::{
    create_profile_post, delete_profile_post, CreateProfilePost, CreatedProfilePost,
    DeletedProfilePost, PostVisibility, ProfilePostError, MAX_ATTACHMENTS, MAX_DESCRIPTION,
};
pub use http::{
    route, BodyLimit, ProfilePostRoute, RouteRefusal, MAX_POST_BODY,
};
pub use read::{
    list_profile_posts, read_profile_post, ProfilePost, ProfilePostCursor, ProfilePostPage,
    ProfilePostQuery, DEFAULT_LIMIT, MAX_LIMIT,
};
