//! WP-06: profile-scoped media.
//!
//! A profile outlives every event, so profile media cannot live in the
//! event-scoped `concert_media` table: that table's primary key is
//! `(event_id, media_id)`, its owner must be a concert participant, and event
//! cleanup erases it. WP-02 therefore added a separate `profile_media` table
//! (migration `0080_profile_media.sql`) and this module is its producer.
//!
//! What is deliberately shared with the concert media runtime rather than
//! forked:
//!
//! * the limits (`concert_application::media`: 10 MiB, 4096 px, 16 M pixels);
//! * the closed MIME set (JPEG, PNG, WebP — no GIF, no video);
//! * the sanitizer port, so a finalized asset is a re-encoded single frame with
//!   no metadata, exactly as a concert upload is;
//! * the init → put bytes → finalize lifecycle and the orphan retention window.
//!
//! What is different, and is the whole point of the module:
//!
//! * there is no event, no participant and no policy fence;
//! * the purpose is one of the four profile purposes, and a purpose/context
//!   mismatch is refused before any row is read or written;
//! * **no pixel conversion happens anywhere here.** A profile photograph is
//!   stored as the normalized photograph. Pixel-art conversion exists only for
//!   room posters (product-plan.md §2 hard rule 5), and the only transformation
//!   this module applies is the shared sanitizer's metadata-stripping re-encode.
//!
//! Modules:
//!
//! * [`upload`] — the upload lifecycle (T-501);
//! * [`authorization`] — who may read which profile media (T-505, E-03/E-04).
pub mod authorization;
pub mod upload;

pub use authorization::{
    can_view_media, media_read_path, MediaViewer, MediaVisibility, ProfileMediaRead,
};
pub use upload::{
    cleanup_orphans, cleanup_owner_orphans, ProfileMediaError, ProfileMediaId, ProfileMediaIntent,
    ProfileMediaState, ProfileMediaStore, ProfileMediaUpload, UploadPurpose,
};
