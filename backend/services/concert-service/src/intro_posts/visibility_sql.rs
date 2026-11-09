//! The SQL form of the visibility decision.
//!
//! T-305 requires a predicate that produces the same decision as
//! [`concert_application::intro_visibility::can_view_intro`] for the same
//! `(viewer, post, now)` inputs, and that runs *inside* the page query rather
//! than over an already-fetched page. This module builds that predicate as one
//! SQL fragment plus its bind values.
//!
//! ## Why a fragment and not a view or a function
//!
//! A PostgreSQL function would be the obvious place to put the table, and it is
//! the wrong one here:
//!
//! * the predicate has to be usable inside a keyset page query with an index
//!   scan on `intro_posts_discovery`, and a function call over a row is opaque to
//!   the planner;
//! * the claim values are per-request, not per-row: the viewer's age and gender
//!   are read once by the adapter and supplied as bind values, so the predicate
//!   never joins a claim table (which lives in another database anyway).
//!
//! So the fragment is a `boolean` expression over the `intro_posts` alias `p`,
//! with the viewer's resolved facts as numbered parameters. The caller embeds it
//! in `where` and the page query filters inside the scan.
//!
//! ## The claim values are the adapter's, not the database's
//!
//! The concert database has no claim table. The adapter reads the claim from the
//! profile service and passes the *outcome* here:
//!
//! * [`ViewerAttributes::read`] carries the current age and gender;
//! * [`ViewerAttributes::unavailable`] carries neither, which denies rows 7-11;
//! * [`ViewerAttributes::not_read`] is what an unrestricted-only caller uses. It
//!   is only ever correct when the audience is unrestricted, and the fragment
//!   still evaluates rows 7-11 for a restricted row, so a caller that skipped
//!   the read gets a denial rather than an admission.
//!
//! ## Row-by-row correspondence
//!
//! | Row | Fragment |
//! | --- | --- |
//! | 1 | `p.author_profile_id = $viewer` |
//! | 2 | `p.state = 'PUBLISHED'` |
//! | 3 | `p.expires_at is null or p.expires_at > $now` |
//! | 4 | `$author_standing = 'ACTIVE'` |
//! | 5 | `not $blocked` |
//! | 6 | `p.audience_min_age is null and p.audience_gender = 'any'` |
//! | 7 | `p.audience_min_age is null or $age is not null` |
//! | 8 | `$age between p.audience_min_age and p.audience_max_age_exclusive - 1` |
//! | 9 | `p.audience_gender = 'any'` |
//! | 10 | `p.audience_gender = $gender` |
//! | 11 | `$provider_answered` |
//! | 12 | otherwise |
//!
//! Rows 1-5 are folded into one `and` chain because they are per-request facts
//! that do not depend on the row's audience: the fragment is only ever used for
//! one viewer at a fixed instant, so the author, block and standing values are
//! bind parameters rather than joins. The row *order* still matters for the pure
//! function's reported reason, and the SQL result is the same because the whole
//! chain is a conjunction.
//!
//! Rows 6-12 are written as a single `case` so the audience branch is evaluated
//! exactly once per row and the "unrestricted needs no claim" branch is visible
//! in the SQL text.
use concert_application::identity_claims::Gender;
use concert_application::intro_visibility::{
    AuthorStanding, BlockState, Decision, DenialReason, IntroPostState, PostState, Row,
    ViewerContext,
};
use concert_domain::intro_audience::IntroAudience;
use uuid::Uuid;

/// The viewer's resolved attributes, as the SQL predicate needs them.
///
/// The three states mirror [`concert_application::identity_claims::Attribute`].
/// A `read` snapshot carries the current age (already converted to the Seoul
/// calendar age by the adapter) and the gender bucket; `unavailable` carries
/// neither; `not_read` is the same as `unavailable` for evaluation purposes and
/// exists only so the caller's intent is recorded in the type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewerAttributes {
    Read {
        /// The current age in years, or `None` when the claim carries no age.
        age: Option<i16>,
        /// The provider gender. `Some` for a live `female`/`male` claim and
        /// `None` for the provider's explicit "not specified" answer.
        gender: Option<Gender>,
        /// Whether the provider answered at all. `true` for both a live gender
        /// and an explicit "not specified"; `false` only when the claim was
        /// unavailable. Row 11 admits on this flag, not on `gender.is_none()`,
        /// because an unavailable claim also has no gender and must not be
        /// admitted by an `unknown`-gender audience.
        provider_answered: bool,
    },
    Unavailable,
    NotRead,
}

impl ViewerAttributes {
    /// A live snapshot.
    ///
    /// `provider_answered` is `true` for a live `female`/`male` gender and for
    /// the explicit "not specified" answer, and `false` when the claim is
    /// unavailable.
    pub fn read(age: Option<i16>, gender: Option<Gender>, provider_answered: bool) -> Self {
        Self::Read {
            age,
            gender,
            provider_answered,
        }
    }

    fn age(self) -> Option<i16> {
        match self {
            Self::Read { age, .. } => age,
            _ => None,
        }
    }

    fn gender_bucket(self) -> Option<&'static str> {
        match self {
            Self::Read { gender, .. } => gender.map(Gender::bucket),
            _ => None,
        }
    }

    /// Whether the provider answered at all.
    ///
    /// Row 11 admits exactly this, and only for an `unknown`-gender audience.
    fn provider_answered(self) -> bool {
        match self {
            Self::Read {
                provider_answered, ..
            } => provider_answered,
            _ => false,
        }
    }
}

/// The per-request facts the fragment is bound to.
///
/// One of these is built per request, not per row. `now` is the decision instant:
/// the post expiry and the claim evaluation both use it, so a page cannot mix
/// two clocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisibilityScope {
    pub viewer_profile_id: Uuid,
    pub blocks: BlockState,
    pub author_standing: AuthorStanding,
    pub attributes: ViewerAttributes,
    pub now: chrono::DateTime<chrono::Utc>,
}

impl VisibilityScope {
    /// The scope for a viewer whose claims were read.
    pub fn new(
        viewer_profile_id: Uuid,
        blocks: BlockState,
        author_standing: AuthorStanding,
        attributes: ViewerAttributes,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        Self {
            viewer_profile_id,
            blocks,
            author_standing,
            attributes,
            now,
        }
    }

    /// The scope for a viewer whose claims could not be read.
    pub fn without_claims(
        viewer_profile_id: Uuid,
        blocks: BlockState,
        author_standing: AuthorStanding,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        Self {
            viewer_profile_id,
            blocks,
            author_standing,
            attributes: ViewerAttributes::Unavailable,
            now,
        }
    }

    /// Whether the page this scope belongs to needs any claim at all.
    ///
    /// The page is a set of posts with different audiences, so the caller reads
    /// claims when *any* row could be restricted. This method answers for the
    /// scope, and the decision for each row still fails closed when the scope
    /// carries no attributes.
    pub const fn attributes(&self) -> ViewerAttributes {
        self.attributes
    }

    /// The parameters for the fragment, in placeholder order.
    pub fn parameters(&self) -> VisibilityParameters {
        VisibilityParameters {
            viewer: self.viewer_profile_id,
            author_active: matches!(self.author_standing, AuthorStanding::Active),
            age: self.attributes.age(),
            gender: self.attributes.gender_bucket(),
            provider_answered: self.attributes.provider_answered(),
            now: self.now,
        }
    }
}

/// The bind values for [`predicate`], in placeholder order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisibilityParameters {
    pub viewer: Uuid,
    pub author_active: bool,
    pub age: Option<i16>,
    pub gender: Option<&'static str>,
    pub provider_answered: bool,
    pub now: chrono::DateTime<chrono::Utc>,
}

impl VisibilityParameters {
    /// Bind this parameter set to a query, in order.
    ///
    /// The placeholder numbers in [`predicate`] start at `first` and are
    /// contiguous, so a caller composing the fragment into a larger query can
    /// place it after its own parameters.
    pub fn bind<'q, O>(
        self,
        query: sqlx::query::QueryAs<'q, sqlx::Postgres, O, sqlx::postgres::PgArguments>,
    ) -> sqlx::query::QueryAs<'q, sqlx::Postgres, O, sqlx::postgres::PgArguments> {
        query
            .bind(self.viewer)
            .bind(self.author_active)
            .bind(self.age)
            .bind(self.gender)
            .bind(self.provider_answered)
            .bind(self.now)
    }

    /// Bind this parameter set to a scalar query, in order.
    pub fn bind_scalar<'q, O>(
        self,
        query: sqlx::query::QueryScalar<'q, sqlx::Postgres, O, sqlx::postgres::PgArguments>,
    ) -> sqlx::query::QueryScalar<'q, sqlx::Postgres, O, sqlx::postgres::PgArguments> {
        query
            .bind(self.viewer)
            .bind(self.author_active)
            .bind(self.age)
            .bind(self.gender)
            .bind(self.provider_answered)
            .bind(self.now)
    }

    /// The number of placeholders the fragment uses.
    ///
    /// The block term is a correlated subquery rather than a bound value, so it
    /// adds no placeholder; the viewer id it reads is the one already bound.
    pub const PLACEHOLDERS: usize = 6;
}

/// The per-row block term: true when this viewer and this post's author are
/// blocked from each other, or when the pair cannot be resolved.
///
/// `viewer` is the placeholder number holding the viewer's profile id. The term
/// is a correlated subquery over the post alias `p`, so it is evaluated once per
/// row against that row's author.
///
/// ## An unresolved pair is *not* a block
///
/// Each side's account id comes from a scalar subquery over
/// `discovery_profiles`, which is a **projection**: a background worker mirrors
/// profiles into it. The subquery yields `NULL` when a profile has never been
/// mirrored, and comparing against `NULL` makes both `exists` arms false.
///
/// That is the correct answer, and it is the one this term now gives. An earlier
/// version wrapped the whole subquery in `coalesce(..., true)`, reasoning that
/// "cannot resolve" should fail closed. It did not: the cross join produced *no
/// row at all* when either profile was unprojected, so the `coalesce` fired and
/// the term evaluated to `true` — "blocked" — for every pair the projection had
/// not yet caught up on. Since `discovery_profiles` is empty until the worker
/// runs, that denied **every** restricted intro post to **every** viewer,
/// including the author's intended audience, on a freshly seeded database.
///
/// A missing projection row means "no block is recorded", which is what an
/// absent block is. The block is a real fact stored in `discovery_blocks`; the
/// projection only supplies the account ids used to compare against it, and an
/// unresolvable pair matches no block. Authorization is not weakened by this:
/// the audience branch still denies a viewer with no live claim, and a block
/// that *is* recorded for two projected profiles is still enforced.
///
/// ## A nil viewer
///
/// A nil viewer is an anonymous resolver, not a profile. No block can name it —
/// `discovery_blocks_not_self` and the account mirror both require a real
/// account — so the term is `false` for it. An anonymous viewer is still unable
/// to satisfy any restriction, because the audience branch denies a viewer with
/// no claim.
pub fn block_term(viewer: usize) -> String {
    format!(
        "(cast(${viewer} as uuid) <> '00000000-0000-0000-0000-000000000000'::uuid \
          and exists (\
              select 1 from discovery_blocks b \
              where b.deleted_at is null \
                and (\
                    (b.blocker_account_id = (select account_id from discovery_profiles \
                                              where profile_id = cast(${viewer} as uuid)) \
                     and b.blocked_account_id = (select account_id from discovery_profiles \
                                                  where profile_id = p.author_profile_id)) \
                    or \
                    (b.blocker_account_id = (select account_id from discovery_profiles \
                                              where profile_id = p.author_profile_id) \
                     and b.blocked_account_id = (select account_id from discovery_profiles \
                                                  where profile_id = cast(${viewer} as uuid)))\
                )\
          ))"
    )
}

/// Build the predicate for a scope whose parameters start at `first`.
///
/// The returned string is a parenthesized `boolean` expression over the
/// `intro_posts` alias `p`, so it composes into a `where` clause, a
/// `join ... on`, or an `exists (...)`.
///
/// The fragment has no string interpolation of values: every value is a
/// placeholder. The only interpolated text is the placeholder numbers, plus the
/// block term below, which is a fixed correlated subquery rather than a value.
///
/// ## The block term is per row, not per request
///
/// Rows 4 and 5 must be evaluated against the **actual pair** — this viewer and
/// *this post's author* — not against the viewer's block state in general. A
/// per-request boolean would have to mean "this viewer has blocked somebody",
/// which is true for anyone with one active block and would then deny every post
/// on the page. The term is therefore a correlated `not exists` over
/// `discovery_blocks`, resolved through the local `discovery_profiles` mirror of
/// account ids. That is the same shape the discovery projection query uses, so
/// the two surfaces cannot disagree about what a block means.
///
/// It is **fail-closed in both directions**: the subquery is satisfied (that is,
/// the row is denied) when an active block exists in either direction, *and*
/// when the pair cannot be resolved at all. A viewer or author with no mirrored
/// account row cannot be proven unblocked, so the post is not shown. That is the
/// same answer the per-request boolean gave for the unresolvable case, so this
/// change removes the over-denial without introducing over-admission.
pub fn predicate(first: usize) -> String {
    let viewer = first;
    // The block term is a correlated subquery over the post alias, not a bound
    // value, so it consumes no placeholder of its own. It reads the viewer's
    // profile id, which is the same `$viewer` the author comparison uses.
    let blocked = block_term(viewer);
    let author_active = first + 1;
    let age = first + 2;
    let gender = first + 3;
    let provider_answered = first + 4;
    let now = first + 5;
    // Rows 1-5 are a conjunction of per-request facts. Rows 6-12 are the
    // audience branch, written once as a `case` so the unrestricted arm is
    // visibly claim-free.
    // Every placeholder carries an explicit cast. SQLx sends parameters as
    // `unknown`-typed values, and PostgreSQL cannot infer a type from an
    // expression such as `$1 is null` on its own; the cast makes the statement
    // preparable without depending on a context the planner does not have.
    format!(
        "(\
             (p.author_profile_id = cast(${viewer} as uuid) \
              or (\
                  p.state = 'PUBLISHED' \
                  and (p.expires_at is null or p.expires_at > cast(${now} as timestamptz)) \
                  and cast(${author_active} as boolean) \
                  and not ({blocked}) \
                  and case \
                      when p.audience_min_age is null and p.audience_gender = 'any' \
                          then true \
                      when p.audience_min_age is not null and cast(${age} as smallint) is null \
                          then false \
                      when p.audience_min_age is not null \
                           and not (cast(${age} as smallint) >= p.audience_min_age \
                                    and cast(${age} as smallint) < p.audience_max_age_exclusive) \
                          then false \
                      when p.audience_gender = 'any' \
                          then true \
                      when p.audience_gender in ('female','male') \
                          then cast(${gender} as text) is not null \
                               and p.audience_gender = cast(${gender} as text) \
                      when p.audience_gender = 'unknown' \
                          then cast(${provider_answered} as boolean) \
                      else false \
                  end\
              )\
             )\
         )"
    )
}

/// Evaluate the decision table in Rust for one row, from the same inputs the SQL
/// predicate receives.
///
/// This is not a second implementation of the table. It builds the
/// [`IntroPostState`] and [`ViewerContext`] the pure function expects and calls
/// it, so a test can compare "what the SQL returned" with "what the pure
/// function says" for exactly the same scope and row.
pub fn expected_decision(
    scope: &VisibilityScope,
    intro_id: Uuid,
    author_profile_id: Uuid,
    state: PostState,
    audience: IntroAudience,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
) -> Decision {
    let post = IntroPostState {
        intro_id,
        author_profile_id,
        state,
        audience,
        expires_at,
        author_standing: scope.author_standing,
    };
    let claims = match scope.attributes {
        ViewerAttributes::Read {
            age,
            gender,
            provider_answered,
        } => Some(claims_from(scope, age, gender, provider_answered)),
        // `NotRead` and `Unavailable` are both "no claim snapshot": the pure
        // function treats a missing snapshot as every attribute unavailable.
        _ => None,
    };
    let viewer = ViewerContext {
        profile_id: scope.viewer_profile_id,
        blocks: scope.blocks,
        claims,
        now: scope.now,
    };
    concert_application::intro_visibility::can_view_intro(&post, &viewer).decision
}

/// The denial reason the SQL row corresponds to, for the evidence table.
///
/// The SQL predicate answers allow/deny; the reason is a property of the pure
/// function's row order. When the two disagree about the *decision*, the parity
/// test fails; when they agree, the reason is informational and is recorded so
/// the evidence can name the row.
pub fn expected_row(
    scope: &VisibilityScope,
    intro_id: Uuid,
    author_profile_id: Uuid,
    state: PostState,
    audience: IntroAudience,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
) -> (Decision, Row) {
    let post = IntroPostState {
        intro_id,
        author_profile_id,
        state,
        audience,
        expires_at,
        author_standing: scope.author_standing,
    };
    let claims = match scope.attributes {
        ViewerAttributes::Read {
            age,
            gender,
            provider_answered,
        } => Some(claims_from(scope, age, gender, provider_answered)),
        _ => None,
    };
    let viewer = ViewerContext {
        profile_id: scope.viewer_profile_id,
        blocks: scope.blocks,
        claims,
        now: scope.now,
    };
    let verdict = concert_application::intro_visibility::can_view_intro(&post, &viewer);
    (verdict.decision, verdict.row)
}

/// Build the claim snapshot the pure function needs from the SQL scope's values.
///
/// The age is supplied as the *current* age, so the snapshot is reconstructed as
/// a claim whose `age_at_verification` is that age and whose next birthday is in
/// the future. That is exact for the decision, which only asks for the current
/// age; the adapter's real claim is a date pair and is converted by
/// `current_age` before it reaches this module.
fn claims_from(
    scope: &VisibilityScope,
    age: Option<i16>,
    gender: Option<Gender>,
    provider_answered: bool,
) -> concert_application::identity_claims::IdentityClaims {
    use concert_application::identity_claims::{Attribute, IdentityClaims, ProfileId};
    use concert_domain::age_policy::arithmetic::{ExactAgeClaim, SeoulDate};
    let age_attribute = match age {
        Some(age) => {
            let seoul = concert_domain::age_policy::seoul_date(scope.now)
                .expect("a representable instant has a Seoul date");
            // A claim whose age at verification is the current age and whose
            // next birthday is tomorrow: `current_age` returns it unchanged.
            let next_birthday = seoul.succ_opt().expect("the next Seoul day is representable");
            Attribute::Known(
                ExactAgeClaim::from_parts(age, SeoulDate::new(next_birthday))
                    .expect("a supported current age"),
            )
        }
        None => Attribute::Unavailable,
    };
    let gender_attribute = match (gender, provider_answered) {
        (Some(gender), _) => Attribute::Known(gender),
        (None, true) => Attribute::Unspecified,
        (None, false) => Attribute::Unavailable,
    };
    IdentityClaims {
        profile: ProfileId::from_uuid(scope.viewer_profile_id),
        age: age_attribute,
        gender: gender_attribute,
        source: None,
        evaluated_at: scope.now,
    }
}

/// The denial reasons a SQL denial can correspond to, for the evidence record.
pub const DENIAL_REASONS: [DenialReason; 9] = [
    DenialReason::NotPublished,
    DenialReason::Expired,
    DenialReason::AuthorInactive,
    DenialReason::AuthorRestricted,
    DenialReason::AuthorBlocked,
    DenialReason::ViewerBlocked,
    DenialReason::AttributeUnavailable,
    DenialReason::AudienceAge,
    DenialReason::AudienceGender,
];
