//! The profile product's Introduction post: stored state, the audience
//! decision, and the surfaces that must all apply it.
//!
//! Two packages own different halves of this module and neither may re-derive
//! the other's decision:
//!
//! * **WP-05** owns the stored state and every mutation. The *rules* live in
//!   `concert_application::intro_posts`, which is pure, and the modules below
//!   apply them inside a transaction.
//! * **WP-04** owns the visibility decision. The domain value
//!   (`IntroAudience`), the claim port (`IdentityClaimsPort`) and the pure
//!   decision (`can_view_intro`) live in the crates; this module holds the SQL
//!   predicate ([`visibility_sql`]) and the queries that embed it.
//!
//! The two meet at the [`Visibility`] port in [`visibility`]: WP-05 calls it
//! rather than re-deriving the decision, so the pure function and the SQL
//! predicate cannot disagree with the callers. [`Visibility::deny_restricted`]
//! remains the fail-closed default for any caller that has not yet been wired to
//! the real policy.
//!
//! What this module deliberately does **not** own: media storage. WP-06 owns
//! `profile_media`; these modules read the rows they need and never write them.
//!
//! Every submodule is a separate enforcement point from
//! [http-contract.md §6](../../../../docs/mcr/integration/http-contract.md), and
//! each has its own test. They share one predicate and one decision function, so
//! a change to the table changes every surface at once, and a surface that stops
//! using it fails its own test.
//!
//! Modules:
//!
//! * [`commands`] — draft save, publish, delete (T-401);
//! * [`snapshot`] — the favorites snapshot column (T-402);
//! * [`attachments`] — attachment validation against `profile_media` (T-403);
//! * [`comments`] — comments and replies (T-404);
//! * [`likes`] — hearts, the liker list and the notification (T-411 backend);
//! * [`http`] — the closed route parser for the 13 frozen operations (T-405);
//! * [`visibility`] — the port WP-05 calls instead of re-deriving the decision;
//! * [`visibility_sql`] — the SQL form of the decision (T-305);
//! * [`repository`] — the page and detail queries that embed it (T-306).
pub mod attachments;
pub mod claims_adapter;
pub mod facts;
pub mod commands;
pub mod comments;
pub mod http;
pub mod likes;
pub mod repository;
pub mod snapshot;
pub mod visibility;
pub mod visibility_sql;
