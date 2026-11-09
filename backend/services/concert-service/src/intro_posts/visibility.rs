//! The visibility seam (WP-04 owns the decision; this module only calls it).
//!
//! `domain-contract.md` §3.1 puts the decision in one pure function,
//! `can_view_intro(post, viewer) -> Decision`. WP-04 owns that function and its
//! SQL predicate. This module defines the **port** the intro post producer calls,
//! so the producer never re-derives the decision and the two cannot drift.
//!
//! ## Why the port is async
//!
//! The decision needs the viewer's age and gender, which come from the profile
//! service through [`IdentityClaimsPort`]. That read is I/O, so a synchronous
//! `can_view` could only be implemented by either never reading claims — which is
//! the fail-closed default and is wrong once the real policy is wired — or by
//! blocking on a runtime inside a request. The port is therefore `async`, and
//! every call site is already inside an async function.
//!
//! ## The two implementations
//!
//! * [`DenyRestricted`] is the fail-closed default. It never reads a claim and
//!   denies every restricted post to everyone but its author. It is what an
//!   unconfigured deployment gets, so a missing claim source can never widen
//!   visibility.
//! * [`ComposedPolicy`] is the real one. It resolves the post's audience and the
//!   viewer's blocks through [`IntroFacts`], reads the claim through the port
//!   **only when the audience requires it**, and calls
//!   [`concert_application::intro_visibility::can_view_intro`]. It does not
//!   re-implement any row of the table.
//!
//! ## Row 6 reads no claim
//!
//! Decision-table row 6 (no age bound, gender `any`) must not read a claim at
//! all. [`ComposedPolicy`] asks
//! [`concert_application::intro_visibility::needs_claims`] before touching the
//! port, and that function is the same expression the decision uses, so "did we
//! need a claim" and "did we read one" cannot disagree.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::identity_claims::{IdentityClaimsPort, ProfileId};
use concert_application::intro_visibility::{
    can_view_intro, needs_claims, AuthorStanding, BlockState, IntroPostState, PostState,
    ViewerContext,
};
use concert_domain::intro_audience::IntroAudience;
use std::sync::Arc;
use uuid::Uuid;

/// The facts the decision needs about the post.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PostFacts {
    pub intro_id: Uuid,
    pub author_profile_id: Uuid,
    pub published: bool,
    /// Whether the post's audience constrains age or gender.
    pub restricted: bool,
    /// `expires_at <= now`, as the caller evaluated it.
    pub expired: bool,
}

/// The facts the decision needs about the viewer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewerFacts {
    pub profile_id: Uuid,
    /// Whether the caller already knows the viewer has a live claim. The default
    /// implementation never consults it; the composed policy reads the claim
    /// itself and ignores this flag.
    pub claims_known: bool,
}

/// The port. One method, one answer.
#[async_trait]
pub trait Visibility: Send + Sync {
    /// Whether `viewer` may see `post` at `now`.
    ///
    /// A denial is never distinguished from absence by the caller: the HTTP layer
    /// turns every `false` into the same `RESOURCE_UNAVAILABLE`.
    async fn can_view(&self, post: &PostFacts, viewer: &ViewerFacts, now: DateTime<Utc>) -> bool;
}

/// The fail-closed default used until WP-04's policy is composed.
pub struct DenyRestricted;

#[async_trait]
impl Visibility for DenyRestricted {
    async fn can_view(&self, post: &PostFacts, viewer: &ViewerFacts, _: DateTime<Utc>) -> bool {
        // The author's own management path always sees their post.
        if viewer.profile_id == post.author_profile_id {
            return true;
        }
        // A post that is not published, or is expired, is not visible to anyone
        // else — rows 2 and 3 of the decision table.
        if !post.published || post.expired {
            return false;
        }
        // Row 6: an unrestricted post needs no claim.
        !post.restricted
    }
}

/// Everything the composed policy needs to build a decision.
///
/// The caller resolves the post's stored audience and the viewer's block state
/// from its own database; the policy resolves the claim through the port. This
/// split keeps the policy free of SQL and the SQL free of the decision.
#[async_trait]
pub trait IntroFacts: Send + Sync {
    /// The stored audience of a post, or `None` when the post does not exist or
    /// its stored audience does not parse.
    async fn audience(&self, intro_id: Uuid) -> Option<IntroAudience>;
    /// The block relationship between the viewer and the author.
    async fn blocks(&self, viewer: Uuid, author: Uuid) -> BlockState;
}

/// The real policy: the decision table, with a claim read only when needed.
pub struct ComposedPolicy {
    claims: Arc<dyn IdentityClaimsPort>,
    facts: Arc<dyn IntroFacts>,
}

impl ComposedPolicy {
    pub fn new(claims: Arc<dyn IdentityClaimsPort>, facts: Arc<dyn IntroFacts>) -> Self {
        Self { claims, facts }
    }
}

#[async_trait]
impl Visibility for ComposedPolicy {
    async fn can_view(&self, post: &PostFacts, viewer: &ViewerFacts, now: DateTime<Utc>) -> bool {
        // Row 1 is answered without touching the database or the claim port: the
        // author always sees their own post. This is also why an author can still
        // manage a post after narrowing its audience.
        if viewer.profile_id == post.author_profile_id {
            return true;
        }
        // An unpublished or expired post is denied before any expensive work.
        if !post.published || post.expired {
            return false;
        }
        // The audience is needed for every remaining row.
        let Some(audience) = self.facts.audience(post.intro_id).await else {
            return false;
        };
        // Row 6 needs no claim. Asking `needs_claims` here — rather than
        // re-deriving "is it restricted" — keeps the read and the decision on the
        // same expression.
        let claims = if needs_claims(audience) {
            self.claims
                .read(ProfileId::from_uuid(viewer.profile_id), now)
                .await
                .ok()
        } else {
            None
        };
        let blocks = self
            .facts
            .blocks(viewer.profile_id, post.author_profile_id)
            .await;
        let state = IntroPostState {
            intro_id: post.intro_id,
            author_profile_id: post.author_profile_id,
            state: PostState::Published,
            audience,
            // The caller already decided expiry; the decision sees a post that is
            // live at `now`, so row 3 does not fire a second time.
            expires_at: None,
            // A post being evaluated for visibility is by construction authored by
            // an active account; the caller resolves standing before this port.
            author_standing: AuthorStanding::Active,
        };
        let context = ViewerContext {
            profile_id: viewer.profile_id,
            blocks,
            claims,
            now,
        };
        can_view_intro(&state, &context).decision.is_allowed()
    }
}

/// The composed policy. `visibility` is the port the producer calls.
#[derive(Clone)]
pub struct VisibilityPolicy {
    inner: Arc<dyn Visibility>,
    /// Whether `inner` is the fail-closed default. Recorded at construction so
    /// the composition root can report an unwired policy without downcasting.
    deny_restricted: bool,
}

impl VisibilityPolicy {
    /// The fail-closed default.
    pub fn deny_restricted() -> Self {
        Self {
            inner: Arc::new(DenyRestricted),
            deny_restricted: true,
        }
    }

    /// Install the real policy.
    pub fn with(inner: Arc<dyn Visibility>) -> Self {
        Self {
            inner,
            deny_restricted: false,
        }
    }

    pub async fn can_view(
        &self,
        post: &PostFacts,
        viewer: &ViewerFacts,
        now: DateTime<Utc>,
    ) -> bool {
        self.inner.can_view(post, viewer, now).await
    }

    /// Whether this policy is the fail-closed default. Used by the composition
    /// root to report that the real policy is not wired.
    pub const fn is_deny_restricted(&self) -> bool {
        self.deny_restricted
    }
}

impl Default for VisibilityPolicy {
    fn default() -> Self {
        Self::deny_restricted()
    }
}
