//! The visibility seam (WP-04 owns the decision; this module only calls it).
//!
//! `domain-contract.md` §3.1 puts the decision in one pure function,
//! `can_view_intro(post, viewer) -> Decision`. WP-04 is writing that function
//! and its SQL predicate. This module defines the **port** the intro post
//! producer calls, so the producer never re-derives the decision and the two
//! cannot drift.
//!
//! Until WP-04 lands, [`Visibility::deny_restricted`] is the wired
//! implementation. It is deliberately the *safe* one:
//!
//! * an unrestricted post (no age bound, gender `any`) is visible to every
//!   viewer, which is the row-6 case of the decision table and requires no
//!   claim at all;
//! * a restricted post is visible to its author only, so a restriction can never
//!   be bypassed while the real policy is missing.
//!
//! When WP-04 lands, the composition swaps in its implementation and nothing in
//! this module changes.
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
///
/// `claims_known` is `false` until WP-04's claim reader is wired; the default
/// implementation never consults it, because it denies every restricted post
/// anyway.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewerFacts {
    pub profile_id: Uuid,
    pub claims_known: bool,
}

/// The port. One method, one answer.
pub trait Visibility: Send + Sync {
    /// Whether `viewer` may see `post`.
    ///
    /// A denial is never distinguished from absence by the caller: the HTTP
    /// layer turns every `false` into the same `RESOURCE_UNAVAILABLE`.
    fn can_view(&self, post: &PostFacts, viewer: &ViewerFacts) -> bool;
}

/// The fail-closed default used until WP-04's policy is composed.
pub struct DenyRestricted;

impl Visibility for DenyRestricted {
    fn can_view(&self, post: &PostFacts, viewer: &ViewerFacts) -> bool {
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

/// The composed policy. `visibility` is the port the producer calls.
#[derive(Clone)]
pub struct VisibilityPolicy {
    inner: std::sync::Arc<dyn Visibility>,
}

impl VisibilityPolicy {
    /// The fail-closed default.
    pub fn deny_restricted() -> Self {
        Self {
            inner: std::sync::Arc::new(DenyRestricted),
        }
    }

    /// Install a real implementation (WP-04's composed policy).
    pub fn with(inner: std::sync::Arc<dyn Visibility>) -> Self {
        Self { inner }
    }

    pub fn can_view(&self, post: &PostFacts, viewer: &ViewerFacts) -> bool {
        self.inner.can_view(post, viewer)
    }
}

impl Default for VisibilityPolicy {
    fn default() -> Self {
        Self::deny_restricted()
    }
}
