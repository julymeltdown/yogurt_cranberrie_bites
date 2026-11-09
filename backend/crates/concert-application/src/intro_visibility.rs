//! The one visibility decision every Introduction surface shares.
//!
//! Contract: [domain-contract.md §3](../../../../docs/mcr/integration/domain-contract.md).
//! The decision table has twelve rows, evaluated in order; the first failure
//! denies. This module implements it exactly, and the SQL predicate in
//! `concert_service::intro_posts::visibility_sql` implements the same table.
//! `visibility_case_table` is the single input both are tested against, so the
//! two cannot drift.
//!
//! Why this module is written the way it is:
//!
//! * **`can_view_intro` is pure and synchronous.** It takes a fully resolved
//!   [`IntroPostState`] and a fully resolved [`ViewerContext`]. It does not read
//!   a clock, a database or a claim store. Everything that is expensive or
//!   fallible happens before the call, so the decision itself can be tested
//!   exhaustively and cannot be made to depend on I/O ordering.
//! * **Claims are read by the caller, and only when needed.** Row 6 (no age
//!   bound, gender `any`) allows without any claim at all. The caller decides
//!   whether to read claims from [`needs_claims`], which is the same predicate
//!   the decision uses, so "did we need a claim" and "did we read one" cannot
//!   disagree.
//! * **A missing claim never bypasses a restriction.** Row 7 denies an unknown
//!   age when an age bound exists. `Unavailable` satisfies no condition, and
//!   `Unspecified` satisfies only `unknown`.
//! * **The author's own path is first.** Row 1 returns `Allowed` before any
//!   other check, because the author must be able to manage and preview their
//!   own post even after it expires or after they narrow the audience. Ownership
//!   is still verified by the caller against the session; this function only
//!   compares the two ids it was given.
use crate::identity_claims::{Attribute, Gender, IdentityClaims};
use chrono::{DateTime, Utc};
use concert_domain::age_policy::arithmetic::{ExactAgeClaim, SeoulDate};
use concert_domain::intro_audience::{GenderCondition, IntroAudience};
use uuid::Uuid;

/// The stored post state, as the decision sees it.
///
/// This mirrors `intro_posts.state`. It is a separate type from
/// `concert_domain::intro_policy::IntroState` because the profile product's post
/// has three states (`DRAFT`, `PUBLISHED`, `DELETED`) and the event-scoped
/// foundation's has three different ones (`Draft`, `Published`, `Closed`).
/// Reusing the event type would silently accept a `Closed` post as published.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostState {
    Draft,
    Published,
    Deleted,
}

impl PostState {
    /// Closed parsing of the stored column.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "DRAFT" => Some(Self::Draft),
            "PUBLISHED" => Some(Self::Published),
            "DELETED" => Some(Self::Deleted),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "DRAFT",
            Self::Published => "PUBLISHED",
            Self::Deleted => "DELETED",
        }
    }
}

/// Why a viewer may not see an Introduction.
///
/// Server-internal. The HTTP response is a uniform `404 RESOURCE_UNAVAILABLE`
/// for every denial except the author's own management path, so a client cannot
/// learn which condition failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DenialReason {
    /// Row 2: the post is not published.
    NotPublished,
    /// Row 3: the post is past its expiry.
    Expired,
    /// Row 4: the author's account is not active.
    AuthorInactive,
    /// Row 4: the author is under an active sanction.
    AuthorRestricted,
    /// Row 5: a block exists in either direction.
    AuthorBlocked,
    /// Row 5: a block exists in the other direction.
    ViewerBlocked,
    /// Row 7: an age bound exists and the viewer's age is not known.
    AttributeUnavailable,
    /// Row 8: the viewer's age is outside the window.
    AudienceAge,
    /// Rows 10 and 11: the viewer's gender does not satisfy the condition.
    AudienceGender,
}

impl DenialReason {
    /// A stable identifier for logs and evidence. It is never sent to a client.
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotPublished => "NOT_PUBLISHED",
            Self::Expired => "EXPIRED",
            Self::AuthorInactive => "AUTHOR_INACTIVE",
            Self::AuthorRestricted => "AUTHOR_RESTRICTED",
            Self::AuthorBlocked => "AUTHOR_BLOCKED",
            Self::ViewerBlocked => "VIEWER_BLOCKED",
            Self::AttributeUnavailable => "ATTRIBUTE_UNAVAILABLE",
            Self::AudienceAge => "AUDIENCE_AGE",
            Self::AudienceGender => "AUDIENCE_GENDER",
        }
    }
}

/// The decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Allowed,
    Denied(DenialReason),
}

impl Decision {
    pub const fn is_allowed(self) -> bool {
        matches!(self, Self::Allowed)
    }

    /// The denial reason, or `None` when allowed.
    pub const fn reason(self) -> Option<DenialReason> {
        match self {
            Self::Allowed => None,
            Self::Denied(reason) => Some(reason),
        }
    }
}

/// The author's account standing, as row 4 needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorStanding {
    /// The account is active and not sanctioned.
    Active,
    /// The account is not active (deleted, disabled, locked).
    Inactive,
    /// The account is active but under a sanction that hides its content.
    Sanctioned,
}

/// The block relationship, as row 5 needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockState {
    /// No block in either direction.
    None,
    /// The viewer has blocked the author.
    ViewerBlockedAuthor,
    /// The author has blocked the viewer.
    AuthorBlockedViewer,
    /// Both directions.
    Mutual,
}

impl BlockState {
    pub const fn blocks(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// The resolved post. Every field is a stored fact; none is derived here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntroPostState {
    pub intro_id: Uuid,
    pub author_profile_id: Uuid,
    pub state: PostState,
    pub audience: IntroAudience,
    pub expires_at: Option<DateTime<Utc>>,
    pub author_standing: AuthorStanding,
}

/// The resolved viewer.
///
/// `claims` is `None` when the caller did not read them. That is only correct
/// when [`needs_claims`] is false; when it is true, a `None` here is treated as
/// "every attribute unavailable", which denies. The decision never assumes that
/// a missing snapshot means "no restriction".
#[derive(Clone, Debug)]
pub struct ViewerContext {
    pub profile_id: Uuid,
    pub blocks: BlockState,
    pub claims: Option<IdentityClaims>,
    pub now: DateTime<Utc>,
}

/// Whether the decision needs an identity claim at all.
///
/// True exactly when the audience is restricted. This is decision-table row 6
/// inverted, written once so the caller's read and the decision's use of it are
/// the same expression.
pub const fn needs_claims(audience: IntroAudience) -> bool {
    audience.is_restricted()
}

/// The decision-table row that produced a result, for the shared case table.
///
/// The row number is a property of the table, not of the code path: it is
/// reported so a test can assert that a case exercised the row it claims to.
/// The pure function and the SQL predicate both report it, which is what makes
/// "row 6 read no claim" a checkable statement rather than a comment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    OneAuthor,
    TwoNotPublished,
    ThreeExpired,
    FourAuthorStanding,
    FiveBlocked,
    SixUnrestricted,
    SevenAgeUnavailable,
    EightAudienceAge,
    NineGenderAny,
    TenAudienceGender,
    ElevenAudienceGenderUnknown,
    TwelveAllowed,
}

impl Row {
    pub const fn number(self) -> u8 {
        match self {
            Self::OneAuthor => 1,
            Self::TwoNotPublished => 2,
            Self::ThreeExpired => 3,
            Self::FourAuthorStanding => 4,
            Self::FiveBlocked => 5,
            Self::SixUnrestricted => 6,
            Self::SevenAgeUnavailable => 7,
            Self::EightAudienceAge => 8,
            Self::NineGenderAny => 9,
            Self::TenAudienceGender => 10,
            Self::ElevenAudienceGenderUnknown => 11,
            Self::TwelveAllowed => 12,
        }
    }
}

/// The decision and the row that produced it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub decision: Decision,
    pub row: Row,
}

impl Verdict {
    const fn allowed(row: Row) -> Self {
        Self {
            decision: Decision::Allowed,
            row,
        }
    }

    const fn denied(row: Row, reason: DenialReason) -> Self {
        Self {
            decision: Decision::Denied(reason),
            row,
        }
    }
}

/// Evaluate the decision table.
///
/// The order is the table's order. Every early return is a row, and no row is
/// merged with another: the reason a viewer was denied is what the evidence
/// records, and two conditions that happen to produce the same HTTP response are
/// still different rows.
pub fn can_view_intro(post: &IntroPostState, viewer: &ViewerContext) -> Verdict {
    // Row 1. The author's own path. Ownership is compared, not inferred, and the
    // caller has already authenticated the session.
    if viewer.profile_id == post.author_profile_id {
        return Verdict::allowed(Row::OneAuthor);
    }
    // Row 2.
    if post.state != PostState::Published {
        return Verdict::denied(Row::TwoNotPublished, DenialReason::NotPublished);
    }
    // Row 3. Expiry is inclusive-exclusive: `expires_at <= now` is expired, so a
    // claim or a post that expires exactly at `now` is already gone.
    if post.expires_at.is_some_and(|expires_at| expires_at <= viewer.now) {
        return Verdict::denied(Row::ThreeExpired, DenialReason::Expired);
    }
    // Row 4.
    match post.author_standing {
        AuthorStanding::Active => {}
        AuthorStanding::Inactive => {
            return Verdict::denied(Row::FourAuthorStanding, DenialReason::AuthorInactive)
        }
        AuthorStanding::Sanctioned => {
            return Verdict::denied(Row::FourAuthorStanding, DenialReason::AuthorRestricted)
        }
    }
    // Row 5.
    match viewer.blocks {
        BlockState::None => {}
        BlockState::ViewerBlockedAuthor => {
            return Verdict::denied(Row::FiveBlocked, DenialReason::ViewerBlocked)
        }
        BlockState::AuthorBlockedViewer | BlockState::Mutual => {
            return Verdict::denied(Row::FiveBlocked, DenialReason::AuthorBlocked)
        }
    }
    let audience = post.audience;
    // Row 6. No age bound and gender `any`: allowed, and no claim is needed.
    // This is the row that removes the donor's global verification gate.
    if !audience.is_restricted() {
        return Verdict::allowed(Row::SixUnrestricted);
    }
    // Rows 7 onward need the viewer's attributes. A caller that skipped the read
    // for a restricted audience gets the same answer as a caller whose read
    // failed: every attribute is unavailable, and row 7 denies.
    let claims = viewer.claims.as_ref();
    let age = claims.map_or(Attribute::Unavailable, |claims| claims.age);
    let gender = claims.map_or(Attribute::Unavailable, |claims| claims.gender);
    // Row 7. An age bound exists and the viewer's age is not known.
    if audience.has_age_bound() {
        let Some(claim) = age.known() else {
            return Verdict::denied(
                Row::SevenAgeUnavailable,
                DenialReason::AttributeUnavailable,
            );
        };
        // Row 8. The half-open window.
        let current = current_age(claim, viewer.now);
        let Some(current) = current else {
            // The Seoul-date conversion cannot fail for a representable instant.
            // If it ever did, the age is not known and row 7 is the honest
            // answer rather than an admitted viewer.
            return Verdict::denied(
                Row::SevenAgeUnavailable,
                DenialReason::AttributeUnavailable,
            );
        };
        if !audience.admits_age(current) {
            return Verdict::denied(Row::EightAudienceAge, DenialReason::AudienceAge);
        }
    }
    // Row 9.
    match audience.gender() {
        GenderCondition::Any => Verdict::allowed(Row::NineGenderAny),
        // Row 10.
        GenderCondition::Female => {
            if gender == Attribute::Known(Gender::Female) {
                Verdict::allowed(Row::TwelveAllowed)
            } else {
                Verdict::denied(Row::TenAudienceGender, DenialReason::AudienceGender)
            }
        }
        GenderCondition::Male => {
            if gender == Attribute::Known(Gender::Male) {
                Verdict::allowed(Row::TwelveAllowed)
            } else {
                Verdict::denied(Row::TenAudienceGender, DenialReason::AudienceGender)
            }
        }
        // Row 11. Only the provider's explicit "not specified" answer matches.
        // `Unavailable` does not, and neither does a live `female` or `male`.
        GenderCondition::Unknown => {
            if gender == Attribute::Unspecified {
                Verdict::allowed(Row::TwelveAllowed)
            } else {
                Verdict::denied(
                    Row::ElevenAudienceGenderUnknown,
                    DenialReason::AudienceGender,
                )
            }
        }
    }
}

/// The viewer's current age on the Seoul calendar date at `now`.
///
/// Delegates to the adapted arithmetic in
/// `concert_domain::age_policy::arithmetic`; this module does not re-implement
/// the birthday rule. `ExactAgeClaim::current_age` takes a `SeoulDate`, and the
/// conversion from a UTC instant to the Seoul date is the one in
/// `concert_domain::age_policy::seoul_date`, so a claim verified at 23:30 UTC on
/// the 31st is evaluated on the next Seoul day.
pub fn current_age(claim: ExactAgeClaim, now: DateTime<Utc>) -> Option<i16> {
    let date = concert_domain::age_policy::seoul_date(now).ok()?;
    Some(i16::from(claim.current_age(SeoulDate::new(date))))
}

/// The decision as the application layer reports it, without the row.
pub fn can_view(post: &IntroPostState, viewer: &ViewerContext) -> Decision {
    can_view_intro(post, viewer).decision
}
