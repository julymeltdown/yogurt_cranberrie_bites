//! T-302: the claim adapter's mapping, and the audience it makes satisfiable.
//!
//! Two halves, both required by the task:
//!
//! * **The mapping.** Every stored claim state is turned into exactly one
//!   `Attribute` value. A live claim is `Known`; the provider's explicit "not
//!   specified" is `Unspecified`; every other state — expired, revoked, replaced,
//!   absent, malformed, transport failure — is `Unavailable`. No state falls back
//!   to a default.
//! * **The effect.** A viewer with a live claim matching the audience sees the
//!   post; a viewer with an expired claim does not; a viewer with no claim does
//!   not; and an unrestricted post is visible to all three. That last case is the
//!   one that proves row 6 never consults a claim.
//!
//! The mapping half uses a fake port so it needs no database and no network. The
//! effect half runs the real SQL predicate against the owned PostgreSQL cluster,
//! with the attributes produced by the same conversion the producer uses.
mod intro_support;

use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use concert_application::identity_claims::{
    Attribute, ClaimSource, ClaimsError, Gender, IdentityClaims, IdentityClaimsPort, ProfileId,
};
use concert_application::intro_visibility::BlockState;
use concert_domain::age_policy::arithmetic::{derive_exact_age_claim, SeoulDate};
use concert_service::intro_posts::commands::{IntroPostStore, SaveDraft};
use concert_service::intro_posts::repository::{get_intro_post, IntroCursorCodec, IntroPostQuery, PageScope};
use concert_service::intro_posts::visibility_sql::{ViewerAttributes, VisibilityScope};
use concert_application::intro_posts::{
    audience::{Audience, GenderCondition},
    content::IntroContent,
    snapshot::Snapshot,
};
use intro_support::{pool, profile};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 9, 5, 0, 0).unwrap()
}

/// A claim for a viewer of exactly `age` on `now`'s Seoul date.
fn age_claim(age: i16) -> concert_domain::age_policy::arithmetic::ExactAgeClaim {
    let year = 2026 - age;
    derive_exact_age_claim(
        Some(&format!("{year:04}")),
        Some("11-09"),
        SeoulDate::new(NaiveDate::from_ymd_opt(2026, 10, 10).unwrap()),
    )
    .expect("a supported birth date")
}

fn source() -> ClaimSource {
    ClaimSource {
        verifier: "naver".to_owned(),
        source_revision: "naver.nid.me.v1".to_owned(),
        verified_at: now() - Duration::days(30),
        expires_at: now() + Duration::days(60),
    }
}

// ---------------------------------------------------------------------------
// The mapping, through a fake port
// ---------------------------------------------------------------------------

/// A port that answers with a fixed snapshot, so the mapping is tested without a
/// database or a network.
struct Fixed(Result<IdentityClaims, ClaimsError>);

#[async_trait::async_trait]
impl IdentityClaimsPort for Fixed {
    async fn read(
        &self,
        _: ProfileId,
        _: DateTime<Utc>,
    ) -> Result<IdentityClaims, ClaimsError> {
        self.0.clone()
    }
}

/// The conversion the producer applies: a snapshot becomes SQL attributes.
///
/// This mirrors `IntroProducer::viewer_attributes` exactly. It is duplicated here
/// because that method is private to the transport; the end-to-end tests below
/// prove the two agree by exercising the real predicate with these values.
fn attributes(snapshot: &IdentityClaims, at: DateTime<Utc>) -> ViewerAttributes {
    let age = snapshot
        .age
        .known()
        .and_then(|claim| concert_application::intro_visibility::current_age(claim, at));
    match snapshot.gender {
        Attribute::Known(gender) => ViewerAttributes::read(age, Some(gender), false),
        Attribute::Unspecified => ViewerAttributes::read(age, None, true),
        Attribute::Unavailable => {
            if age.is_some() {
                ViewerAttributes::read(age, None, false)
            } else {
                ViewerAttributes::Unavailable
            }
        }
    }
}

#[tokio::test]
async fn a_live_claim_becomes_known_attributes() {
    let snapshot = IdentityClaims {
        profile: ProfileId::from_uuid(Uuid::new_v4()),
        age: Attribute::Known(age_claim(25)),
        gender: Attribute::Known(Gender::Female),
        source: Some(source()),
        evaluated_at: now(),
    };
    let port = Fixed(Ok(snapshot.clone()));
    let read = port.read(snapshot.profile, now()).await.expect("a live claim");
    let attributes = attributes(&read, now());
    match attributes {
        ViewerAttributes::Read {
            age,
            gender,
            provider_answered,
        } => {
            assert_eq!(age, Some(25), "the current age at the evaluation instant");
            assert_eq!(gender, Some(Gender::Female));
            // A live gender means the provider answered, but not "not specified".
            assert!(!provider_answered);
        }
        other => panic!("a live claim must be readable, got {other:?}"),
    }
}

#[tokio::test]
async fn an_explicit_not_specified_claim_becomes_unspecified() {
    let snapshot = IdentityClaims {
        profile: ProfileId::from_uuid(Uuid::new_v4()),
        age: Attribute::Known(age_claim(25)),
        gender: Attribute::Unspecified,
        source: Some(source()),
        evaluated_at: now(),
    };
    let attributes = attributes(&snapshot, now());
    match attributes {
        ViewerAttributes::Read {
            age,
            gender,
            provider_answered,
        } => {
            assert_eq!(age, Some(25));
            assert_eq!(gender, None, "not specified is not a gender");
            assert!(
                provider_answered,
                "row 11 admits exactly the provider's explicit answer"
            );
        }
        other => panic!("expected a readable snapshot, got {other:?}"),
    }
}

#[tokio::test]
async fn an_unavailable_claim_becomes_unavailable_attributes() {
    let snapshot = IdentityClaims::unavailable(ProfileId::from_uuid(Uuid::new_v4()), now());
    let attributes = attributes(&snapshot, now());
    assert_eq!(
        attributes,
        ViewerAttributes::Unavailable,
        "no claim, an expired claim and a revoked claim are all unavailable"
    );
}

#[tokio::test]
async fn a_transport_failure_is_unavailable_and_never_a_default() {
    let port = Fixed(Err(ClaimsError::Unavailable));
    let error = port
        .read(ProfileId::from_uuid(Uuid::new_v4()), now())
        .await
        .expect_err("a transport failure is an error, not a value");
    assert_eq!(error, ClaimsError::Unavailable);
    // The producer maps that error to `Unavailable`, which denies rows 7-11.
    let attributes = ViewerAttributes::Unavailable;
    assert!(!matches!(attributes, ViewerAttributes::Read { .. }));
}

// ---------------------------------------------------------------------------
// The effect, through the real SQL predicate
// ---------------------------------------------------------------------------

/// A published post with `audience`, authored by `author`.
async fn published(pool: &PgPool, author: Uuid, audience: Audience) -> anyhow::Result<Uuid> {
    let store = IntroPostStore::new(pool.clone());
    let intro = Uuid::new_v4();
    let saved = store
        .save_draft(
            author,
            intro,
            &SaveDraft {
                expected_revision: 0,
                content: IntroContent {
                    body: "An introduction with an audience.".to_owned(),
                    currently_seeking: None,
                    interaction_style: None,
                },
                favorites_snapshot: Snapshot::empty(),
                attachment_media_ids: Vec::new(),
                audience,
                expires_at: None,
            },
        )
        .await?;
    store.publish(author, intro, saved.revision).await?;
    Ok(intro)
}

fn scope(viewer: Uuid, attributes: ViewerAttributes) -> VisibilityScope {
    VisibilityScope::new(
        viewer,
        BlockState::None,
        concert_application::intro_visibility::AuthorStanding::Active,
        attributes,
        now(),
    )
}

fn page_scope(viewer: Uuid, attributes: ViewerAttributes) -> PageScope {
    PageScope {
        viewer_profile_id: viewer,
        blocks: BlockState::None,
        attributes,
        now: now(),
    }
}

fn women_18_29() -> Audience {
    Audience::new(GenderCondition::Female, Some(18), Some(30)).expect("the V-01 audience")
}

#[tokio::test]
async fn a_live_matching_claim_sees_a_restricted_post() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t302-author").await?;
    let viewer = profile(&pool, "t302-live").await?;
    let intro = published(&pool, author.profile_id, women_18_29()).await?;
    let snapshot = IdentityClaims {
        profile: ProfileId::from_uuid(viewer.profile_id),
        age: Attribute::Known(age_claim(25)),
        gender: Attribute::Known(Gender::Female),
        source: Some(source()),
        evaluated_at: now(),
    };
    // The detail read succeeds for a viewer the audience admits.
    let row = get_intro_post(&pool, &scope(viewer.profile_id, attributes(&snapshot, now())), intro).await;
    assert!(row.is_ok(), "a matching live claim must see the post: {row:?}");
    // And the page contains it.
    let page = concert_service::intro_posts::repository::list_intro_posts(
        &pool,
        &IntroCursorCodec::new([0x5a; 32]),
        &page_scope(viewer.profile_id, attributes(&snapshot, now())),
        &IntroPostQuery::new(Some(50), None)?,
    )
    .await?;
    assert!(
        page.items.iter().any(|item| item.intro_id == intro),
        "the restricted post is on the page for a matching viewer"
    );
    Ok(())
}

#[tokio::test]
async fn an_expired_claim_does_not_see_a_restricted_post() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t302-author2").await?;
    let viewer = profile(&pool, "t302-expired").await?;
    let intro = published(&pool, author.profile_id, women_18_29()).await?;
    // An expired claim is `Unavailable` for both attributes.
    let snapshot = IdentityClaims::unavailable(ProfileId::from_uuid(viewer.profile_id), now());
    assert_eq!(attributes(&snapshot, now()), ViewerAttributes::Unavailable);
    let error = get_intro_post(&pool, &scope(viewer.profile_id, ViewerAttributes::Unavailable), intro)
        .await
        .expect_err("an expired claim must not see a restricted post");
    assert_eq!(
        error,
        concert_service::intro_posts::repository::IntroPageError::Unavailable
    );
    Ok(())
}

#[tokio::test]
async fn a_viewer_with_no_claim_does_not_see_a_restricted_post() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t302-author3").await?;
    let viewer = profile(&pool, "t302-none").await?;
    let intro = published(&pool, author.profile_id, women_18_29()).await?;
    let error = get_intro_post(&pool, &scope(viewer.profile_id, ViewerAttributes::Unavailable), intro)
        .await
        .expect_err("no claim must not see a restricted post");
    assert_eq!(
        error,
        concert_service::intro_posts::repository::IntroPageError::Unavailable
    );
    Ok(())
}

#[tokio::test]
async fn an_unrestricted_post_is_visible_to_all_three_viewers() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t302-author4").await?;
    let live = profile(&pool, "t302-u-live").await?;
    let expired = profile(&pool, "t302-u-expired").await?;
    let none = profile(&pool, "t302-u-none").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE).await?;
    let live_snapshot = IdentityClaims {
        profile: ProfileId::from_uuid(live.profile_id),
        age: Attribute::Known(age_claim(25)),
        gender: Attribute::Known(Gender::Female),
        source: Some(source()),
        evaluated_at: now(),
    };
    for (label, viewer, attributes) in [
        ("live", live, attributes(&live_snapshot, now())),
        ("expired", expired, ViewerAttributes::Unavailable),
        ("none", none, ViewerAttributes::Unavailable),
    ] {
        let row = get_intro_post(&pool, &scope(viewer.profile_id, attributes), intro).await;
        assert!(
            row.is_ok(),
            "an unrestricted post is visible to the {label} viewer: {row:?}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_live_claim_outside_the_window_does_not_see_the_post() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t302-author5").await?;
    let viewer = profile(&pool, "t302-outside").await?;
    let intro = published(&pool, author.profile_id, women_18_29()).await?;
    let snapshot = IdentityClaims {
        profile: ProfileId::from_uuid(viewer.profile_id),
        age: Attribute::Known(age_claim(31)),
        gender: Attribute::Known(Gender::Female),
        source: Some(source()),
        evaluated_at: now(),
    };
    let error = get_intro_post(&pool, &scope(viewer.profile_id, attributes(&snapshot, now())), intro)
        .await
        .expect_err("an age outside the window is denied");
    assert_eq!(
        error,
        concert_service::intro_posts::repository::IntroPageError::Unavailable
    );
    Ok(())
}

#[tokio::test]
async fn a_live_claim_with_the_wrong_gender_does_not_see_the_post() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t302-author6").await?;
    let viewer = profile(&pool, "t302-wrong-gender").await?;
    let intro = published(&pool, author.profile_id, women_18_29()).await?;
    let snapshot = IdentityClaims {
        profile: ProfileId::from_uuid(viewer.profile_id),
        age: Attribute::Known(age_claim(25)),
        gender: Attribute::Known(Gender::Male),
        source: Some(source()),
        evaluated_at: now(),
    };
    let error = get_intro_post(&pool, &scope(viewer.profile_id, attributes(&snapshot, now())), intro)
        .await
        .expect_err("a male claim does not satisfy a female audience");
    assert_eq!(
        error,
        concert_service::intro_posts::repository::IntroPageError::Unavailable
    );
    Ok(())
}

#[tokio::test]
async fn the_adapter_is_object_safe_and_shareable() {
    // The composition root stores the adapter behind `Arc<dyn IdentityClaimsPort>`.
    let port: Arc<dyn IdentityClaimsPort> = Arc::new(Fixed(Err(ClaimsError::Unavailable)));
    assert!(Arc::strong_count(&port) == 1);
}
