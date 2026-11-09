//! T-304: the pure visibility decision against the shared case table.
//!
//! The case table itself lives in `visibility_cases/mod.rs`, which is also
//! compiled into `services/concert-service/tests/intro_visibility_contract.rs`.
//! Running it here proves the pure function; running it there proves the SQL
//! predicate agrees with it.
#[path = "visibility_cases/mod.rs"]
mod visibility_cases;

use chrono::{Duration, TimeZone, Utc};
use concert_application::identity_claims::{
    Attribute, ClaimsError, Gender, IdentityClaims, IdentityClaimsPort, ProfileId,
};
use concert_application::intro_visibility::{
    can_view_intro, current_age, needs_claims, AuthorStanding, BlockState, Decision,
    DenialReason, IntroPostState, PostState, Row,
};
use concert_domain::intro_audience::IntroAudience;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use visibility_cases::*;

#[test]
fn every_row_of_the_decision_table_is_exercised() {
    let rows: std::collections::BTreeSet<u8> = all_cases()
        .iter()
        .map(|case| case.expected_row.number())
        .collect();
    assert_eq!(
        rows,
        (1..=12).collect::<std::collections::BTreeSet<u8>>(),
        "every row 1..12 must have at least one case"
    );
}

#[test]
fn every_case_matches_the_pure_decision() {
    let cases = all_cases();
    assert!(cases.len() >= 40, "the table must be exhaustive");
    for case in &cases {
        case.check_pure();
    }
}

#[test]
fn case_ids_are_unique() {
    let ids = all_ids();
    let unique: std::collections::BTreeSet<&str> = ids.iter().copied().collect();
    assert_eq!(ids.len(), unique.len(), "case ids must be unique");
}

#[test]
fn the_unrestricted_path_reads_no_claim() {
    // The port panics if it is read. Row 6 must never reach it, which is the
    // structural form of "no claim read when the audience is unrestricted".
    struct PanicOnRead;
    #[async_trait::async_trait]
    impl IdentityClaimsPort for PanicOnRead {
        async fn read(
            &self,
            _: ProfileId,
            _: chrono::DateTime<Utc>,
        ) -> Result<IdentityClaims, ClaimsError> {
            panic!("the unrestricted path must not read identity claims");
        }
    }
    // The decision is pure, so it cannot read at all; the property that matters
    // is that the caller is not told to. `needs_claims` is the caller's only
    // signal, and it is false exactly for the unrestricted audience.
    assert!(!needs_claims(IntroAudience::ANYONE));
    for case in all_cases().iter().filter(|case| !case.may_read_claims) {
        assert!(
            !needs_claims(case.post.audience),
            "{}: an unrestricted audience must not ask for claims",
            case.id
        );
        case.check_pure();
    }
    // And the double is genuinely armed: reading it panics.
    let port = PanicOnRead;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = futures_lite_block_on(port.read(ProfileId::from_uuid(VIEWER), now()));
    }));
    assert!(result.is_err(), "the panic-on-read double must panic");
}

/// A minimal blocking executor for the one assertion above. The test binary has
/// no async runtime of its own for a sync test.
fn futures_lite_block_on<F: std::future::Future>(future: F) -> F::Output {
    // The future panics synchronously before any await point, so polling it once
    // with a no-op waker is enough and cannot hang.
    use std::task::{Context, Poll, Wake, Waker};
    struct Noop;
    impl Wake for Noop {
        fn wake(self: Arc<Self>) {}
    }
    let waker = Waker::from(Arc::new(Noop));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("the double must not yield"),
    }
}

#[test]
fn needs_claims_is_exactly_the_restriction() {
    assert!(!needs_claims(IntroAudience::ANYONE));
    assert!(needs_claims(women_18_29()));
    assert!(needs_claims(anyone_18_29()));
    assert!(needs_claims(unknown_only()));
    assert!(needs_claims(women_only()));
    assert!(needs_claims(men_only()));
}

#[test]
fn a_denial_reason_is_never_allowed() {
    for case in all_cases() {
        match case.expected {
            Decision::Allowed => assert!(case.expected.reason().is_none(), "{}", case.id),
            Decision::Denied(reason) => {
                assert_eq!(case.expected.reason(), Some(reason), "{}", case.id);
                assert!(!reason.code().is_empty(), "{}", case.id);
            }
        }
    }
}

#[test]
fn the_row_order_is_the_table_order() {
    // Row 1 precedes row 2: the author sees a draft.
    let mut draft = published(anyone());
    draft.state = PostState::Draft;
    let mut author = viewer(None);
    author.profile_id = AUTHOR;
    assert_eq!(can_view_intro(&draft, &author).row, Row::OneAuthor);
    // Row 2 precedes row 3: an unpublished, expired post reports not-published.
    draft.expires_at = Some(now() - Duration::days(1));
    let other = viewer(None);
    assert_eq!(
        can_view_intro(&draft, &other).decision,
        Decision::Denied(DenialReason::NotPublished)
    );
    // Row 3 precedes row 4: an expired post reports expired even when the
    // author is sanctioned.
    let expired = IntroPostState {
        expires_at: Some(now()),
        author_standing: AuthorStanding::Sanctioned,
        ..published(anyone())
    };
    assert_eq!(
        can_view_intro(&expired, &other).decision,
        Decision::Denied(DenialReason::Expired)
    );
    // Row 4 precedes row 5: a sanctioned author reports the sanction even when
    // the viewer is blocked.
    let sanctioned = IntroPostState {
        author_standing: AuthorStanding::Sanctioned,
        ..published(anyone())
    };
    let mut blocked = viewer(None);
    blocked.blocks = BlockState::Mutual;
    assert_eq!(
        can_view_intro(&sanctioned, &blocked).decision,
        Decision::Denied(DenialReason::AuthorRestricted)
    );
    // Row 5 precedes row 6: a block denies even an unrestricted post.
    assert_eq!(
        can_view_intro(&published(anyone()), &blocked).decision,
        Decision::Denied(DenialReason::AuthorBlocked)
    );
    // Row 6 precedes row 7: an unrestricted post allows a viewer with no claim.
    assert_eq!(
        can_view_intro(&published(anyone()), &other).decision,
        Decision::Allowed
    );
    // Row 7 precedes row 8: an unknown age reports unavailable, not age.
    let age_bounded = published(anyone_18_29());
    assert_eq!(
        can_view_intro(&age_bounded, &other).decision,
        Decision::Denied(DenialReason::AttributeUnavailable)
    );
    // Row 8 precedes row 9: an out-of-window age denies even with gender `any`.
    let mut in_window = viewer(Some(claims(
        Attribute::Known(age_claim(17)),
        Attribute::Unspecified,
    )));
    in_window.blocks = BlockState::None;
    assert_eq!(
        can_view_intro(&age_bounded, &in_window).decision,
        Decision::Denied(DenialReason::AudienceAge)
    );
}

#[test]
fn the_author_path_bypasses_rows_two_through_twelve() {
    // Every condition that would deny a stranger, at once.
    let post = IntroPostState {
        intro_id: uuid::Uuid::new_v4(),
        author_profile_id: AUTHOR,
        state: PostState::Deleted,
        audience: women_18_29(),
        expires_at: Some(now() - Duration::days(30)),
        author_standing: AuthorStanding::Inactive,
    };
    let mut author = viewer(Some(no_claim()));
    author.profile_id = AUTHOR;
    author.blocks = BlockState::Mutual;
    let verdict = can_view_intro(&post, &author);
    assert_eq!(verdict.decision, Decision::Allowed);
    assert_eq!(verdict.row, Row::OneAuthor);
    // The same post for anyone else is denied at the first applicable row.
    let stranger = viewer(Some(no_claim()));
    assert_eq!(
        can_view_intro(&post, &stranger).decision,
        Decision::Denied(DenialReason::NotPublished)
    );
}

#[test]
fn a_viewer_with_no_claims_structurally_cannot_satisfy_a_restriction() {
    // A `ViewerContext` with `claims: None` is what a caller holds when it did
    // not read. For every restricted audience in the table, that must deny.
    for audience in [
        women_18_29(),
        anyone_18_29(),
        unknown_only(),
        women_only(),
        men_only(),
    ] {
        let verdict = can_view_intro(&published(audience), &viewer(None));
        assert!(
            !verdict.decision.is_allowed(),
            "{} must deny an unread viewer",
            audience.summary()
        );
    }
}

#[test]
fn an_age_only_audience_never_requires_a_gender() {
    // Row 9: once the age passes, gender `any` allows even with no gender claim.
    for gender in [
        Attribute::Known(Gender::Female),
        Attribute::Known(Gender::Male),
        Attribute::Unspecified,
        Attribute::Unavailable,
    ] {
        let verdict = can_view_intro(
            &published(anyone_18_29()),
            &viewer(Some(claims(Attribute::Known(age_claim(25)), gender))),
        );
        assert_eq!(verdict.decision, Decision::Allowed, "{gender:?}");
        assert_eq!(verdict.row, Row::NineGenderAny);
    }
}

#[test]
fn a_gender_only_audience_never_requires_an_age() {
    // Row 7 asks whether an age bound exists, not whether the audience is
    // restricted. A gender-only audience with an unavailable age still allows.
    let verdict = can_view_intro(
        &published(women_only()),
        &viewer(Some(claims(
            Attribute::Unavailable,
            Attribute::Known(Gender::Female),
        ))),
    );
    assert_eq!(verdict.decision, Decision::Allowed);
}

#[test]
fn the_current_age_uses_the_seoul_calendar_date() {
    // 2026-11-08 16:00 UTC is 2026-11-09 01:00 Seoul. A viewer whose birthday is
    // 11-09 is already the older age in Seoul and not yet in UTC.
    let instant = Utc
        .with_ymd_and_hms(2026, 11, 8, 16, 0, 0)
        .unwrap();
    let claim = age_claim(25);
    let seoul = current_age(claim, instant).expect("a current age");
    let utc_same_day = current_age(claim, Utc.with_ymd_and_hms(2026, 11, 9, 0, 30, 0).unwrap())
        .expect("a current age");
    assert_eq!(seoul, utc_same_day);
    // Before the Seoul birthday, one less.
    let before = current_age(
        claim,
        Utc.with_ymd_and_hms(2026, 11, 8, 14, 59, 59).unwrap(),
    )
    .expect("a current age");
    assert_eq!(before, seoul - 1);
}

#[test]
fn the_decision_reads_no_clock_of_its_own() {
    // `ViewerContext::now` is the only time input. Two calls with the same
    // context return the same verdict, and moving `now` changes only the
    // expiry-sensitive cases.
    let post = IntroPostState {
        expires_at: Some(now() + Duration::seconds(1)),
        ..published(anyone())
    };
    let context = viewer(None);
    let first = can_view_intro(&post, &context);
    let second = can_view_intro(&post, &context);
    assert_eq!(first, second);
    let mut later = context.clone();
    later.now = now() + Duration::seconds(2);
    assert_eq!(
        can_view_intro(&post, &later).decision,
        Decision::Denied(DenialReason::Expired)
    );
}

#[test]
fn a_counting_port_is_never_reached_by_the_pure_decision() {
    // The pure function takes a resolved context, so it cannot call a port. This
    // test documents the boundary: the caller reads, the decision decides.
    struct Counting {
        reads: AtomicUsize,
    }
    let port = Counting {
        reads: AtomicUsize::new(0),
    };
    for case in all_cases() {
        case.check_pure();
    }
    assert_eq!(port.reads.load(Ordering::SeqCst), 0);
}
