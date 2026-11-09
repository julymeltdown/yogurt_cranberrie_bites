//! The like and comment paths consult the viewer's claim.
//!
//! The detail and page reads build their scope through the claim adapter, but the
//! like, unlike and comment paths did not: `IntroProducer` constructed its three
//! stores with `*Store::new`, which installs the fail-closed `DenyRestricted`
//! policy, and nothing ever replaced it. The result was that a viewer who could
//! read a restricted post in the detail could not like it — the like answered
//! `404` while the detail answered `200`.
//!
//! These tests go through `IntroProducer::with_claims`, which is the composition
//! the service actually uses, so a regression in the wiring fails here rather
//! than only in the end-to-end gate.
mod intro_support;

use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use concert_application::identity_claims::{
    Attribute, ClaimSource, ClaimsError, Gender, IdentityClaims, IdentityClaimsPort, ProfileId,
};
use concert_application::intro_posts::{
    audience::{Audience, GenderCondition},
    content::IntroContent,
    snapshot::Snapshot,
};
use concert_domain::age_policy::arithmetic::{derive_exact_age_claim, SeoulDate};
use concert_service::intro_posts::{
    commands::{IntroPostStore, SaveDraft},
    likes::LikeError,
};
use concert_service::profile_product::IntroProducer;
use intro_support::{pool, profile};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 9, 5, 0, 0).unwrap()
}

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

/// A claim port that answers with one fixed snapshot for every subject.
struct Fixed(Result<IdentityClaims, ClaimsError>);

#[async_trait::async_trait]
impl IdentityClaimsPort for Fixed {
    async fn read(
        &self,
        profile: ProfileId,
        at: DateTime<Utc>,
    ) -> Result<IdentityClaims, ClaimsError> {
        match &self.0 {
            Ok(snapshot) => Ok(IdentityClaims {
                profile,
                age: snapshot.age,
                gender: snapshot.gender,
                source: snapshot.source.clone(),
                evaluated_at: at,
            }),
            Err(error) => Err(*error),
        }
    }
}

fn live_woman_25() -> IdentityClaims {
    IdentityClaims {
        profile: ProfileId::from_uuid(Uuid::nil()),
        age: Attribute::Known(age_claim(25)),
        gender: Attribute::Known(Gender::Female),
        source: Some(source()),
        evaluated_at: now(),
    }
}

/// The producer the service actually builds, with the claim port attached.
fn producer(pool: &PgPool, claims: Result<IdentityClaims, ClaimsError>) -> IntroProducer {
    IntroProducer::new(pool.clone()).with_claims(Arc::new(Fixed(claims)))
}

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
                    body: "An introduction worth a heart.".to_owned(),
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

fn women_18_29() -> Audience {
    Audience::new(GenderCondition::Female, Some(18), Some(30)).expect("the V-01 audience")
}

/// The defect: a viewer the audience admits could not like the post.
#[tokio::test]
async fn a_viewer_with_a_live_matching_claim_can_like_a_restricted_post() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "claimlike-author").await?;
    let viewer = profile(&pool, "claimlike-viewer").await?;
    let intro = published(&pool, author.profile_id, women_18_29()).await?;
    let producer = producer(&pool, Ok(live_woman_25()));
    let liked = producer
        .likes()
        .like(intro, viewer.profile_id)
        .await
        .expect("a viewer with a matching live claim may like the post");
    assert!(liked.liked);
    assert_eq!(liked.like_count, Some(1));
    Ok(())
}

/// A viewer with no claim still cannot like it.
#[tokio::test]
async fn a_viewer_with_no_claim_cannot_like_a_restricted_post() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "claimlike-author2").await?;
    let viewer = profile(&pool, "claimlike-viewer2").await?;
    let intro = published(&pool, author.profile_id, women_18_29()).await?;
    let producer = producer(&pool, Err(ClaimsError::Unavailable));
    let error = producer
        .likes()
        .like(intro, viewer.profile_id)
        .await
        .expect_err("no claim means the audience cannot be satisfied");
    assert_eq!(error, LikeError::Unavailable);
    assert_eq!(error.status(), 404);
    Ok(())
}

/// An unrestricted post is likeable by both, and reads no claim.
#[tokio::test]
async fn an_unrestricted_post_is_likeable_with_and_without_a_claim() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "claimlike-author3").await?;
    let with_claim = profile(&pool, "claimlike-with").await?;
    let without_claim = profile(&pool, "claimlike-without").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE).await?;
    // A port that fails every read: row 6 must not reach it.
    let producer = producer(&pool, Err(ClaimsError::Unavailable));
    assert!(producer.likes().like(intro, with_claim.profile_id).await?.liked);
    assert!(
        producer
            .likes()
            .like(intro, without_claim.profile_id)
            .await?
            .liked
    );
    Ok(())
}

/// The comment path consults the claim too.
#[tokio::test]
async fn a_viewer_with_a_live_matching_claim_can_comment_on_a_restricted_post(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "claimcomment-author").await?;
    let viewer = profile(&pool, "claimcomment-viewer").await?;
    let intro = published(&pool, author.profile_id, women_18_29()).await?;
    let producer = producer(&pool, Ok(live_woman_25()));
    let comment = producer
        .comments()
        .create(intro, viewer.profile_id, "A comment worth reading.", None)
        .await
        .expect("a viewer with a matching live claim may comment");
    assert_eq!(comment.intro_id, intro);
    Ok(())
}

#[tokio::test]
async fn a_viewer_with_no_claim_cannot_comment_on_a_restricted_post() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "claimcomment-author2").await?;
    let viewer = profile(&pool, "claimcomment-viewer2").await?;
    let intro = published(&pool, author.profile_id, women_18_29()).await?;
    let producer = producer(&pool, Err(ClaimsError::Unavailable));
    assert!(
        producer
            .comments()
            .create(intro, viewer.profile_id, "Should not land.", None)
            .await
            .is_err(),
        "no claim means the audience cannot be satisfied"
    );
    Ok(())
}

/// The unlike path returns the count for a viewer who still satisfies the
/// audience — which requires the same claim read.
#[tokio::test]
async fn an_admitted_viewer_sees_the_count_when_withdrawing_a_heart() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "claimunlike-author").await?;
    let viewer = profile(&pool, "claimunlike-viewer").await?;
    let intro = published(&pool, author.profile_id, women_18_29()).await?;
    let producer = producer(&pool, Ok(live_woman_25()));
    producer.likes().like(intro, viewer.profile_id).await?;
    let cleared = producer.likes().unlike(intro, viewer.profile_id).await?;
    assert!(!cleared.liked);
    assert_eq!(
        cleared.like_count,
        Some(0),
        "a viewer who still satisfies the audience sees the count"
    );
    Ok(())
}
