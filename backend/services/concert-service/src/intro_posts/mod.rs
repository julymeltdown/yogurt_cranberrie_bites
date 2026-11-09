//! The profile product's Introduction post: the audience decision and the
//! surfaces that must all apply it.
//!
//! The domain value (`IntroAudience`), the claim port (`IdentityClaimsPort`) and
//! the pure decision (`can_view_intro`) live in the crates. This module is the
//! concert service's side of the same contract: the SQL predicate
//! ([`visibility_sql`]), the queries that embed it, the like policy, the liker
//! list, the shared-link resolution and the media authorization.
//!
//! Every submodule here is a separate enforcement point from
//! [http-contract.md §6](../../../../docs/mcr/integration/http-contract.md), and
//! each has its own test. They share one predicate and one decision function, so
//! a change to the table changes every surface at once, and a surface that stops
//! using it fails its own test.
pub mod repository;
pub mod visibility_sql;
