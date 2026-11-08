//! WP-05: the mutuals-intro post producer.
//!
//! This module is the concert-service half of the intro post feature. It owns
//! the stored state and every mutation; the *rules* live in
//! `concert_application::intro_posts`, which is pure, and this module applies
//! them inside a transaction.
//!
//! What it deliberately does **not** own:
//!
//! * the visibility decision (`can_view_intro`). WP-04 owns
//!   `concert_application::intro_visibility` and
//!   `concert_service::intro_posts::visibility_sql`. This module calls a narrow
//!   [`Visibility`] port instead of re-deriving the decision, so the two cannot
//!   disagree. Until WP-04 lands, [`Visibility::deny_restricted`] is the
//!   fail-closed default: an unrestricted post is visible to everyone, and a
//!   restricted one is visible only to its author.
//! * media storage. WP-06 owns `profile_media`; this module reads the rows it
//!   needs and never writes them.
//!
//! Modules:
//!
//! * [`commands`] — draft save, publish, delete (T-401);
//! * [`snapshot`] — the favorites snapshot column (T-402);
//! * [`attachments`] — attachment validation against `profile_media` (T-403);
//! * [`comments`] — comments and replies (T-404);
//! * [`likes`] — hearts, the liker list and the notification (T-411 backend);
//! * [`http`] — the closed route parser for the 13 frozen operations (T-405).
pub mod attachments;
pub mod commands;
pub mod comments;
pub mod http;
pub mod likes;
pub mod snapshot;
pub mod visibility;

pub use commands::{IntroCommandError, IntroPostStore};
pub use likes::{LikeError, LikeStore};
pub use visibility::{Visibility, ViewerFacts};
