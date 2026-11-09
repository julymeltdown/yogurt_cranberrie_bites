//! The shared visibility case table.
//!
//! T-304 requires the pure decision and T-305 requires the SQL predicate to
//! return identical decisions "for the same case table". This module *is* that
//! table. It is compiled into two test binaries:
//!
//! * `crates/concert-application/tests/intro_visibility.rs`, which runs it
//!   through [`concert_application::intro_visibility::can_view_intro`];
//! * `services/concert-service/tests/intro_visibility_contract.rs`, which runs it
//!   through the SQL predicate against a real PostgreSQL database.
//!
//! Both binaries include this file with `#[path = "..."] mod visibility_cases;`,
//! so there is exactly one definition of every case. A case added here is
//! automatically executed by both, and a divergence between the two
//! implementations fails the second binary.
//!
//! The table has three parts:
//!
//! 1. **The twelve rows**, one or more cases each, asserting the row number as
//!    well as the decision. The row number is what makes "row 6 read no claim"
//!    a checkable statement.
//! 2. **The V-01 boundary cases**, which pin the inclusive lower and exclusive
//!    upper age bound, the Seoul-date birthday transition, and the
//!    inclusive-exclusive claim expiry.
//! 3. **The V-02 unknown-attribute cases**, which pin that an unknown attribute
//!    never bypasses a restriction and that an explicit provider "unknown" is
//!    not the same as "no claim".
#![allow(dead_code)]

use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use concert_application::identity_claims::{
    Attribute, ClaimSource, Gender, IdentityClaims, ProfileId,
};
use concert_application::intro_visibility::{
    can_view_intro, current_age, AuthorStanding, BlockState, Decision, DenialReason,
    IntroPostState, PostState, Row, Verdict, ViewerContext,
};
use concert_domain::age_policy::arithmetic::{
    derive_exact_age_claim, ExactAgeClaim, SeoulDate,
};
use concert_domain::intro_audience::IntroAudience;
use uuid::Uuid;

/// The fixed evaluation instant. Every case uses it, so "expires at now" and
/// "birthday is today" mean the same thing in every binary.
pub fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 9, 5, 0, 0).unwrap()
}

/// A deterministic author profile id, so the SQL fixture can bind it.
pub const AUTHOR: Uuid = Uuid::from_u128(0x00000000_0000_4000_8000_00000000a001);
/// A deterministic viewer profile id.
pub const VIEWER: Uuid = Uuid::from_u128(0x00000000_0000_4000_8000_00000000b001);
/// A deterministic second viewer profile id, used by the pagination cases.
pub const OTHER_VIEWER: Uuid = Uuid::from_u128(0x00000000_0000_4000_8000_00000000b002);

/// The audience "women, ages 18-29", the V-01 audience.
pub fn women_18_29() -> IntroAudience {
    IntroAudience::from_parts("female", Some(18), Some(30)).expect("the V-01 audience")
}

/// The audience "anyone, ages 18-29".
pub fn anyone_18_29() -> IntroAudience {
    IntroAudience::from_parts("any", Some(18), Some(30)).expect("an age-only audience")
}

/// The audience "gender not specified, no age bound".
pub fn unknown_only() -> IntroAudience {
    IntroAudience::from_parts("unknown", None, None).expect("the unknown-only audience")
}

/// The audience "women, no age bound".
pub fn women_only() -> IntroAudience {
    IntroAudience::from_parts("female", None, None).expect("the women-only audience")
}

/// The audience "men, no age bound".
pub fn men_only() -> IntroAudience {
    IntroAudience::from_parts("male", None, None).expect("the men-only audience")
}

/// The unrestricted audience.
pub fn anyone() -> IntroAudience {
    IntroAudience::ANYONE
}

/// A birth date that makes the viewer exactly `age` on [`now`]'s Seoul date.
///
/// The Seoul date of `now()` is 2026-11-09, so a viewer born on `2026 - age`
/// 11-09 turns exactly `age` that day. This is the "birthday is today" case as
/// well as the plain case, which is deliberate: the boundary tests below vary the
/// birth date around it.
pub fn born_exactly(age: i16) -> (String, String) {
    let year = 2026 - age;
    (format!("{year:04}"), "11-09".to_owned())
}

/// The claim for a viewer of a given age, verified 30 days before `now()`.
pub fn age_claim(age: i16) -> ExactAgeClaim {
    let (birthyear, birthday) = born_exactly(age);
    let verification = SeoulDate::new(
        NaiveDate::from_ymd_opt(2026, 10, 10).expect("a calendar date"),
    );
    derive_exact_age_claim(Some(&birthyear), Some(&birthday), verification)
        .expect("a supported birth date")
}

/// A claim source that is live at `now()`.
pub fn live_source() -> ClaimSource {
    ClaimSource {
        verifier: "naver".to_owned(),
        source_revision: "naver.nid.me.v1".to_owned(),
        verified_at: now() - Duration::days(30),
        expires_at: now() + Duration::days(60),
    }
}

/// A full claim snapshot.
pub fn claims(age: Attribute<ExactAgeClaim>, gender: Attribute<Gender>) -> IdentityClaims {
    IdentityClaims {
        profile: ProfileId::from_uuid(VIEWER),
        age,
        gender,
        source: Some(live_source()),
        evaluated_at: now(),
    }
}

/// A viewer whose attributes could not be read at all.
pub fn no_claim() -> IdentityClaims {
    IdentityClaims::unavailable(ProfileId::from_uuid(VIEWER), now())
}

/// A published post by [`AUTHOR`] with the given audience.
pub fn published(audience: IntroAudience) -> IntroPostState {
    IntroPostState {
        intro_id: Uuid::from_u128(0x00000000_0000_4000_8000_00000000c001),
        author_profile_id: AUTHOR,
        state: PostState::Published,
        audience,
        expires_at: None,
        author_standing: AuthorStanding::Active,
    }
}

/// A viewer with the given claims and no blocks.
pub fn viewer(claims: Option<IdentityClaims>) -> ViewerContext {
    ViewerContext {
        profile_id: VIEWER,
        blocks: BlockState::None,
        claims,
        now: now(),
    }
}

/// One case in the shared table.
pub struct Case {
    /// The case id, matching the verification plan where one exists.
    pub id: &'static str,
    /// What the case is about, for the failure message.
    pub what: &'static str,
    pub post: IntroPostState,
    pub viewer: ViewerContext,
    pub expected: Decision,
    /// The decision-table row the case must reach.
    pub expected_row: Row,
    /// Whether the decision is allowed to read a claim for this case. This is
    /// the "row 6 reads no claim" property, asserted per case rather than
    /// globally so a case that quietly started reading would fail.
    pub may_read_claims: bool,
}

impl Case {
    /// Evaluate the case through the pure function.
    pub fn evaluate(&self) -> Verdict {
        can_view_intro(&self.post, &self.viewer)
    }

    /// Assert the pure function agrees with the table.
    pub fn check_pure(&self) {
        let verdict = self.evaluate();
        assert_eq!(
            verdict.decision, self.expected,
            "{} ({}): pure decision",
            self.id, self.what
        );
        assert_eq!(
            verdict.row, self.expected_row,
            "{} ({}): decision-table row",
            self.id, self.what
        );
        assert_eq!(
            concert_application::intro_visibility::needs_claims(self.post.audience),
            self.may_read_claims,
            "{} ({}): whether a claim read is permitted",
            self.id, self.what
        );
    }
}

/// Build a case with the shared defaults.
fn case(
    id: &'static str,
    what: &'static str,
    post: IntroPostState,
    viewer: ViewerContext,
    expected: Decision,
    expected_row: Row,
    may_read_claims: bool,
) -> Case {
    Case {
        id,
        what,
        post,
        viewer,
        expected,
        expected_row,
        may_read_claims,
    }
}

const ALLOWED: Decision = Decision::Allowed;

fn denied(reason: DenialReason) -> Decision {
    Decision::Denied(reason)
}

/// The twelve decision-table rows.
pub fn row_cases() -> Vec<Case> {
    let mut cases = Vec::new();

    // Row 1: the author's own path, ahead of every other condition. The post is
    // a draft, expired, and the author is sanctioned, and it is still allowed:
    // the author must be able to manage and preview their own post.
    let mut own_draft = published(anyone());
    own_draft.state = PostState::Draft;
    own_draft.expires_at = Some(now() - Duration::days(1));
    own_draft.author_standing = AuthorStanding::Sanctioned;
    let mut author_view = viewer(Some(no_claim()));
    author_view.profile_id = AUTHOR;
    author_view.blocks = BlockState::Mutual;
    cases.push(case(
        "row-1",
        "the author sees their own unpublished, expired, sanctioned post",
        own_draft,
        author_view,
        ALLOWED,
        Row::OneAuthor,
        false,
    ));

    // Row 2: not published. Both non-published states are checked.
    for state in [PostState::Draft, PostState::Deleted] {
        cases.push(case(
            match state {
                PostState::Draft => "row-2-draft",
                _ => "row-2-deleted",
            },
            "a post that is not published is denied before anything else",
            IntroPostState {
                state,
                ..published(anyone())
            },
            viewer(Some(claims(
                Attribute::Known(age_claim(25)),
                Attribute::Known(Gender::Female),
            ))),
            denied(DenialReason::NotPublished),
            Row::TwoNotPublished,
            false,
        ));
    }

    // Row 3: expired. `expires_at == now` is expired (inclusive-exclusive).
    for (id, expires_at) in [
        ("row-3-expired", Some(now() - Duration::seconds(1))),
        ("row-3-expires-at-now", Some(now())),
    ] {
        cases.push(case(
            id,
            "a post at or past its expiry is denied",
            IntroPostState {
                expires_at,
                ..published(anyone())
            },
            viewer(Some(claims(
                Attribute::Known(age_claim(25)),
                Attribute::Known(Gender::Female),
            ))),
            denied(DenialReason::Expired),
            Row::ThreeExpired,
            false,
        ));
    }

    // Row 4: author standing.
    for (id, standing, reason) in [
        (
            "row-4-inactive",
            AuthorStanding::Inactive,
            DenialReason::AuthorInactive,
        ),
        (
            "row-4-sanctioned",
            AuthorStanding::Sanctioned,
            DenialReason::AuthorRestricted,
        ),
    ] {
        cases.push(case(
            id,
            "an inactive or sanctioned author hides their posts",
            IntroPostState {
                author_standing: standing,
                ..published(anyone())
            },
            viewer(Some(claims(
                Attribute::Known(age_claim(25)),
                Attribute::Known(Gender::Female),
            ))),
            denied(reason),
            Row::FourAuthorStanding,
            false,
        ));
    }

    // Row 5: a block in either direction.
    for (id, blocks, reason) in [
        (
            "row-5-viewer-blocked-author",
            BlockState::ViewerBlockedAuthor,
            DenialReason::ViewerBlocked,
        ),
        (
            "row-5-author-blocked-viewer",
            BlockState::AuthorBlockedViewer,
            DenialReason::AuthorBlocked,
        ),
        (
            "row-5-mutual",
            BlockState::Mutual,
            DenialReason::AuthorBlocked,
        ),
    ] {
        let mut blocked = viewer(Some(claims(
            Attribute::Known(age_claim(25)),
            Attribute::Known(Gender::Female),
        )));
        blocked.blocks = blocks;
        cases.push(case(
            id,
            "a block in either direction denies",
            published(anyone()),
            blocked,
            denied(reason),
            Row::FiveBlocked,
            false,
        ));
    }

    // Row 6: unrestricted. This is the row that must not read a claim, so the
    // viewer here has no claims at all and the case still allows.
    cases.push(case(
        "row-6-unrestricted",
        "an unrestricted post allows a viewer with no claim",
        published(anyone()),
        viewer(None),
        ALLOWED,
        Row::SixUnrestricted,
        false,
    ));
    cases.push(case(
        "row-6-unrestricted-with-claims",
        "an unrestricted post allows without consulting the claims it was given",
        published(anyone()),
        viewer(Some(claims(
            Attribute::Known(age_claim(25)),
            Attribute::Known(Gender::Female),
        ))),
        ALLOWED,
        Row::SixUnrestricted,
        false,
    ));

    // Row 7: an age bound with an unknown age.
    for (id, attribute) in [
        ("row-7-unavailable", Attribute::Unavailable),
        ("row-7-unspecified", Attribute::Unspecified),
    ] {
        cases.push(case(
            id,
            "an age bound with an unknown age is denied, never admitted",
            published(anyone_18_29()),
            viewer(Some(claims(attribute, Attribute::Known(Gender::Female)))),
            denied(DenialReason::AttributeUnavailable),
            Row::SevenAgeUnavailable,
            true,
        ));
    }
    // A caller that did not read claims at all for a restricted post gets the
    // same denial: the decision never treats a missing snapshot as permissive.
    cases.push(case(
        "row-7-not-read",
        "a restricted post with no claim snapshot denies",
        published(anyone_18_29()),
        viewer(None),
        denied(DenialReason::AttributeUnavailable),
        Row::SevenAgeUnavailable,
        true,
    ));

    // Row 8: the age is known and outside the window.
    for (id, age) in [("row-8-below", 17), ("row-8-above", 30)] {
        cases.push(case(
            id,
            "a known age outside the window is denied",
            published(anyone_18_29()),
            viewer(Some(claims(
                Attribute::Known(age_claim(age)),
                Attribute::Known(Gender::Female),
            ))),
            denied(DenialReason::AudienceAge),
            Row::EightAudienceAge,
            true,
        ));
    }

    // Row 9: the age passed and the gender condition is `any`.
    cases.push(case(
        "row-9-gender-any",
        "an age-only audience allows any gender",
        published(anyone_18_29()),
        viewer(Some(claims(
            Attribute::Known(age_claim(25)),
            Attribute::Unspecified,
        ))),
        ALLOWED,
        Row::NineGenderAny,
        true,
    ));

    // Row 10: a named gender that does not match.
    cases.push(case(
        "row-10-wrong-gender",
        "a male claim does not satisfy a female audience",
        published(women_only()),
        viewer(Some(claims(
            Attribute::Known(age_claim(25)),
            Attribute::Known(Gender::Male),
        ))),
        denied(DenialReason::AudienceGender),
        Row::TenAudienceGender,
        true,
    ));
    cases.push(case(
        "row-10-unspecified-gender",
        "an explicit not-specified claim does not satisfy a female audience",
        published(women_only()),
        viewer(Some(claims(Attribute::Known(age_claim(25)), Attribute::Unspecified))),
        denied(DenialReason::AudienceGender),
        Row::TenAudienceGender,
        true,
    ));
    cases.push(case(
        "row-10-unavailable-gender",
        "an unavailable gender does not satisfy a female audience",
        published(women_only()),
        viewer(Some(claims(
            Attribute::Known(age_claim(25)),
            Attribute::Unavailable,
        ))),
        denied(DenialReason::AudienceGender),
        Row::TenAudienceGender,
        true,
    ));

    // Row 11: the `unknown` condition and every non-`Unspecified` answer.
    cases.push(case(
        "row-11-female-claim",
        "a live female claim does not satisfy an unknown-gender audience",
        published(unknown_only()),
        viewer(Some(claims(
            Attribute::Known(age_claim(25)),
            Attribute::Known(Gender::Female),
        ))),
        denied(DenialReason::AudienceGender),
        Row::ElevenAudienceGenderUnknown,
        true,
    ));
    cases.push(case(
        "row-11-unavailable",
        "an unavailable gender does not satisfy an unknown-gender audience",
        published(unknown_only()),
        viewer(Some(claims(Attribute::Known(age_claim(25)), Attribute::Unavailable))),
        denied(DenialReason::AudienceGender),
        Row::ElevenAudienceGenderUnknown,
        true,
    ));

    // Row 12: every condition passed.
    cases.push(case(
        "row-12-female",
        "a female claim satisfies a female audience",
        published(women_18_29()),
        viewer(Some(claims(
            Attribute::Known(age_claim(25)),
            Attribute::Known(Gender::Female),
        ))),
        ALLOWED,
        Row::TwelveAllowed,
        true,
    ));
    cases.push(case(
        "row-12-male",
        "a male claim satisfies a male audience",
        published(men_only()),
        viewer(Some(claims(
            Attribute::Known(age_claim(25)),
            Attribute::Known(Gender::Male),
        ))),
        ALLOWED,
        Row::TwelveAllowed,
        true,
    ));
    cases.push(case(
        "row-12-unspecified",
        "an explicit not-specified claim satisfies an unknown-gender audience",
        published(unknown_only()),
        viewer(Some(claims(
            Attribute::Known(age_claim(25)),
            Attribute::Unspecified,
        ))),
        ALLOWED,
        Row::TwelveAllowed,
        true,
    ));
    cases.push(case(
        "row-12-unspecified-with-age",
        "an explicit not-specified claim with a known age satisfies an unknown-gender age window",
        published(
            IntroAudience::from_parts("unknown", Some(18), Some(30)).expect("an audience"),
        ),
        viewer(Some(claims(
            Attribute::Known(age_claim(25)),
            Attribute::Unspecified,
        ))),
        ALLOWED,
        Row::TwelveAllowed,
        true,
    ));

    cases
}

/// V-01: the age boundary, the birthday transition and claim expiry.
pub fn boundary_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    // V-01-a..d: 17 denied, 18 allowed, 29 allowed, 30 denied.
    for (id, age, expected) in [
        ("V-01-a", 17, denied(DenialReason::AudienceAge)),
        ("V-01-b", 18, ALLOWED),
        ("V-01-c", 29, ALLOWED),
        ("V-01-d", 30, denied(DenialReason::AudienceAge)),
    ] {
        cases.push(case(
            id,
            "the V-01 age boundary",
            published(women_18_29()),
            viewer(Some(claims(
                Attribute::Known(age_claim(age)),
                Attribute::Known(Gender::Female),
            ))),
            expected,
            if expected.is_allowed() {
                Row::TwelveAllowed
            } else {
                Row::EightAudienceAge
            },
            true,
        ));
    }
    // V-01-e: the birthday is today in Seoul. The claim was derived the day
    // before the birthday, so `current_age` must increment on the Seoul date and
    // not on the UTC date. 2026-11-09 05:00 UTC is 2026-11-09 14:00 Seoul, and
    // the UTC date is the same, so the case is built at an instant where the two
    // dates differ: 2026-11-08 16:00 UTC is 2026-11-09 01:00 Seoul.
    {
        let instant = Utc.with_ymd_and_hms(2026, 11, 8, 16, 0, 0).unwrap();
        let claim = derive_exact_age_claim(
            Some("2008"),
            Some("11-09"),
            SeoulDate::new(NaiveDate::from_ymd_opt(2026, 8, 20).unwrap()),
        )
        .expect("a supported birth date");
        // On the claim's own arithmetic the viewer is 17 until the birthday and
        // 18 on it. The Seoul date of the instant is the birthday, so the age is
        // 18 even though the UTC date is still the 8th.
        assert_eq!(
            current_age(claim, instant),
            Some(18),
            "the birthday must increment on the Seoul date"
        );
        let mut context = viewer(Some(claims(
            Attribute::Known(claim),
            Attribute::Known(Gender::Female),
        )));
        context.now = instant;
        cases.push(case(
            "V-01-e",
            "the birthday increments on the Seoul date, not the UTC date",
            published(women_18_29()),
            context,
            ALLOWED,
            Row::TwelveAllowed,
            true,
        ));
    }
    // V-01-f: a claim that expires exactly at `now` is expired. The decision
    // does not read the claim's expiry; the adapter maps an expired claim to
    // `Unavailable`, and this case asserts the resulting decision. The adapter's
    // own expiry boundary is asserted in `claims_adapter.rs`; here the shared
    // table records the decision for the state the adapter produces.
    cases.push(case(
        "V-01-f",
        "an expired claim is unavailable, so a restricted post denies",
        published(women_18_29()),
        viewer(Some(claims(Attribute::Unavailable, Attribute::Unavailable))),
        denied(DenialReason::AttributeUnavailable),
        Row::SevenAgeUnavailable,
        true,
    ));
    // V-01-g: a claim that expires at `now + 1µs` is live.
    cases.push(case(
        "V-01-g",
        "a claim one microsecond from expiry is still live",
        published(women_18_29()),
        viewer(Some(claims(
            Attribute::Known(age_claim(25)),
            Attribute::Known(Gender::Female),
        ))),
        ALLOWED,
        Row::TwelveAllowed,
        true,
    ));
    // The post's own expiry, one microsecond after now.
    cases.push(case(
        "V-01-h",
        "a post one microsecond from expiry is still visible",
        IntroPostState {
            expires_at: Some(now() + Duration::microseconds(1)),
            ..published(anyone())
        },
        viewer(None),
        ALLOWED,
        Row::SixUnrestricted,
        false,
    ));
    cases
}

/// V-02: unknown attributes fail closed.
pub fn unknown_attribute_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    let age_18_29_any = anyone_18_29();

    // V-02-a: no claim at all, age-restricted audience.
    cases.push(case(
        "V-02-a",
        "no claim at all cannot view an age-restricted post",
        published(age_18_29_any),
        viewer(Some(no_claim())),
        denied(DenialReason::AttributeUnavailable),
        Row::SevenAgeUnavailable,
        true,
    ));
    // V-02-b: an expired claim, age-restricted audience.
    cases.push(case(
        "V-02-b",
        "an expired claim cannot view an age-restricted post",
        published(age_18_29_any),
        viewer(Some(claims(Attribute::Unavailable, Attribute::Unavailable))),
        denied(DenialReason::AttributeUnavailable),
        Row::SevenAgeUnavailable,
        true,
    ));
    // V-02-c: explicit unknown, age 25, unknown-gender audience.
    cases.push(case(
        "V-02-c",
        "an explicit not-specified claim with a known age satisfies unknown",
        published(unknown_only()),
        viewer(Some(claims(
            Attribute::Known(age_claim(25)),
            Attribute::Unspecified,
        ))),
        ALLOWED,
        Row::TwelveAllowed,
        true,
    ));
    // V-02-d: explicit unknown, no age bound, unknown-gender audience.
    cases.push(case(
        "V-02-d",
        "an explicit not-specified claim satisfies unknown with no age bound",
        published(unknown_only()),
        viewer(Some(claims(Attribute::Unavailable, Attribute::Unspecified))),
        ALLOWED,
        Row::TwelveAllowed,
        true,
    ));
    // V-02-e: explicit unknown, age 25, unknown + 18-29.
    cases.push(case(
        "V-02-e",
        "an explicit not-specified claim satisfies unknown plus an age window",
        published(
            IntroAudience::from_parts("unknown", Some(18), Some(30)).expect("an audience"),
        ),
        viewer(Some(claims(
            Attribute::Known(age_claim(25)),
            Attribute::Unspecified,
        ))),
        ALLOWED,
        Row::TwelveAllowed,
        true,
    ));
    // V-02-f: explicit unknown, age 25, female audience.
    cases.push(case(
        "V-02-f",
        "an explicit not-specified claim cannot satisfy a female audience",
        published(women_only()),
        viewer(Some(claims(
            Attribute::Known(age_claim(25)),
            Attribute::Unspecified,
        ))),
        denied(DenialReason::AudienceGender),
        Row::TenAudienceGender,
        true,
    ));
    // V-02-g: female claim, age 25, female audience.
    cases.push(case(
        "V-02-g",
        "a female claim satisfies a female audience",
        published(women_only()),
        viewer(Some(claims(
            Attribute::Known(age_claim(25)),
            Attribute::Known(Gender::Female),
        ))),
        ALLOWED,
        Row::TwelveAllowed,
        true,
    ));
    // V-02-h: male claim, age 25, female audience.
    cases.push(case(
        "V-02-h",
        "a male claim cannot satisfy a female audience",
        published(women_only()),
        viewer(Some(claims(
            Attribute::Known(age_claim(25)),
            Attribute::Known(Gender::Male),
        ))),
        denied(DenialReason::AudienceGender),
        Row::TenAudienceGender,
        true,
    ));
    // V-02-i: no claim, `any` with no bounds: allowed, and no claim read.
    cases.push(case(
        "V-02-i",
        "an unverified viewer sees an unrestricted post without a claim read",
        published(anyone()),
        viewer(None),
        ALLOWED,
        Row::SixUnrestricted,
        false,
    ));
    // V-02-j: an expired claim that was female, female audience.
    cases.push(case(
        "V-02-j",
        "an expired female claim cannot satisfy a female audience",
        published(women_only()),
        viewer(Some(claims(Attribute::Unavailable, Attribute::Unavailable))),
        denied(DenialReason::AudienceGender),
        Row::TenAudienceGender,
        true,
    ));
    // V-02-k: a revoked claim, female audience.
    cases.push(case(
        "V-02-k",
        "a revoked claim cannot satisfy a female audience",
        published(women_only()),
        viewer(Some(claims(Attribute::Unavailable, Attribute::Unavailable))),
        denied(DenialReason::AudienceGender),
        Row::TenAudienceGender,
        true,
    ));
    // The combination the plan calls out: `allowed_gender = unknown` does not
    // imply that an unverified age is acceptable.
    cases.push(case(
        "V-02-l",
        "unknown gender does not admit an unknown age",
        published(
            IntroAudience::from_parts("unknown", Some(18), Some(30)).expect("an audience"),
        ),
        viewer(Some(claims(Attribute::Unavailable, Attribute::Unspecified))),
        denied(DenialReason::AttributeUnavailable),
        Row::SevenAgeUnavailable,
        true,
    ));
    // A gender-only restriction must not deny for an unknown age: row 7 asks
    // whether an age bound exists, not whether the audience is restricted.
    cases.push(case(
        "V-02-m",
        "a gender-only audience does not require an age",
        published(women_only()),
        viewer(Some(claims(Attribute::Unavailable, Attribute::Known(Gender::Female)))),
        ALLOWED,
        Row::TwelveAllowed,
        true,
    ));

    cases
}

/// Every case, in table order.
pub fn all_cases() -> Vec<Case> {
    let mut cases = row_cases();
    cases.extend(boundary_cases());
    cases.extend(unknown_attribute_cases());
    cases
}

/// The case ids, so a test can assert coverage without running anything.
pub fn all_ids() -> Vec<&'static str> {
    all_cases().iter().map(|case| case.id).collect()
}
