//! T-309: the like policy under the **real** visibility decision.
//!
//! WP-05's `intro_likes.rs` proves the like rules against the fail-closed default
//! policy, where every restricted post is invisible to everyone but its author.
//! That is safe but it cannot show that the rules are correct once the audience
//! actually admits someone — which is the state the product is in now that the
//! claim adapter is wired.
//!
//! This file runs the same rules through `ComposedPolicy`, the real decision:
//!
//! * a viewer whose live claim satisfies the audience may like a restricted post;
//! * a viewer whose claim does not satisfy it gets `404`, indistinguishable from
//!   a post that does not exist;
//! * two `PUT`s still produce one row and one notification;
//! * `DELETE` after the claim expires removes the like and returns
//!   `like_count: null`, so losing access never discloses the current count;
//! * the unlike path returns no restricted content at all.
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
    facts::PgIntroFacts,
    likes::{LikeError, LikeStore},
    visibility::{ComposedPolicy, VisibilityPolicy},
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

/// A claim port that answers with one fixed snapshot for every subject.
///
/// The like store asks about the viewer, and the test controls exactly what the
/// viewer's attributes are, so the audience decision is exercised without a
/// profile service.
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

/// A like store whose visibility decision is the real one.
fn store(pool: &PgPool, claims: Result<IdentityClaims, ClaimsError>) -> LikeStore {
    let policy = VisibilityPolicy::with(Arc::new(ComposedPolicy::new(
        Arc::new(Fixed(claims)),
        Arc::new(PgIntroFacts::new(pool.clone())),
    )));
    LikeStore::with_visibility(pool.clone(), policy)
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

/// The case the fail-closed default could not express: a viewer the audience
/// admits may like the post.
#[tokio::test]
async fn a_viewer_admitted_by_the_audience_can_like_a_restricted_post() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t309-author").await?;
    let viewer = profile(&pool, "t309-admitted").await?;
    let intro = published(&pool, author.profile_id, women_18_29()).await?;
    let store = store(&pool, Ok(live_woman_25()));
    let liked = store.like(intro, viewer.profile_id).await?;
    assert!(liked.liked);
    assert_eq!(liked.like_count, Some(1));
    // The row exists and the author was notified exactly once.
    let rows: i64 = sqlx::query_scalar("select count(*) from intro_likes where intro_id=$1")
        .bind(intro)
        .fetch_one(&pool)
        .await?;
    assert_eq!(rows, 1);
    let notifications: i64 = sqlx::query_scalar(
        "select count(*) from intro_like_notifications where intro_id=$1",
    )
    .bind(intro)
    .fetch_one(&pool)
    .await?;
    assert_eq!(notifications, 1);
    Ok(())
}

/// A viewer the audience excludes gets the same `404` as a missing post.
#[tokio::test]
async fn a_viewer_excluded_by_the_audience_gets_404() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t309-author2").await?;
    let viewer = profile(&pool, "t309-excluded").await?;
    let intro = published(&pool, author.profile_id, women_18_29()).await?;
    // A live claim, but the wrong gender.
    let wrong_gender = IdentityClaims {
        profile: ProfileId::from_uuid(Uuid::nil()),
        age: Attribute::Known(age_claim(25)),
        gender: Attribute::Known(Gender::Male),
        source: Some(source()),
        evaluated_at: now(),
    };
    let store = store(&pool, Ok(wrong_gender));
    let error = store
        .like(intro, viewer.profile_id)
        .await
        .expect_err("an excluded viewer cannot like the post");
    assert_eq!(error, LikeError::Unavailable);
    assert_eq!(error.status(), 404);
    // Nothing was written.
    let rows: i64 = sqlx::query_scalar("select count(*) from intro_likes where intro_id=$1")
        .bind(intro)
        .fetch_one(&pool)
        .await?;
    assert_eq!(rows, 0);
    Ok(())
}

/// Two `PUT`s from an admitted viewer still produce one row and one notification.
#[tokio::test]
async fn two_puts_from_an_admitted_viewer_produce_one_row_and_one_notification(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t309-author3").await?;
    let viewer = profile(&pool, "t309-twice").await?;
    let intro = published(&pool, author.profile_id, women_18_29()).await?;
    let store = store(&pool, Ok(live_woman_25()));
    let first = store.like(intro, viewer.profile_id).await?;
    let second = store.like(intro, viewer.profile_id).await?;
    assert!(first.liked && second.liked);
    assert_eq!(first.like_count, Some(1));
    assert_eq!(second.like_count, Some(1), "a second PUT is a no-op");
    let rows: i64 = sqlx::query_scalar("select count(*) from intro_likes where intro_id=$1")
        .bind(intro)
        .fetch_one(&pool)
        .await?;
    assert_eq!(rows, 1);
    let notifications: i64 = sqlx::query_scalar(
        "select count(*) from intro_like_notifications where intro_id=$1",
    )
    .bind(intro)
    .fetch_one(&pool)
    .await?;
    assert_eq!(notifications, 1, "one notification for one like");
    Ok(())
}

/// The security case: a viewer who had access, then lost it, can still withdraw
/// their heart — and the response carries no count and no content.
#[tokio::test]
async fn a_viewer_who_lost_access_can_unlike_without_learning_the_count(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t309-author4").await?;
    let viewer = profile(&pool, "t309-lost").await?;
    // Unrestricted first, so the like is created while the viewer has access.
    let intro = published(&pool, author.profile_id, Audience::ANYONE).await?;
    let admitted = store(&pool, Ok(live_woman_25()));
    let liked = admitted.like(intro, viewer.profile_id).await?;
    assert_eq!(liked.like_count, Some(1));
    // The author narrows the audience to an age window the viewer is outside.
    sqlx::query(
        "update intro_posts set audience_gender='any', audience_min_age=40, \
         audience_max_age_exclusive=50, revision=revision+1 where intro_id=$1",
    )
    .bind(intro)
    .execute(&pool)
    .await?;
    // The viewer now fails the audience, so the response must not carry a count.
    let cleared = admitted.unlike(intro, viewer.profile_id).await?;
    assert!(!cleared.liked);
    assert_eq!(
        cleared.like_count, None,
        "losing access must not disclose the current count"
    );
    let wire = cleared.to_wire();
    assert_eq!(wire["like_count"], serde_json::Value::Null);
    // The unlike response carries nothing else: no body, no attachments, no author.
    let object = wire.as_object().expect("the wire form is an object");
    let keys: std::collections::BTreeSet<&str> =
        object.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        ["like_count", "liked"].into_iter().collect(),
        "the unlike response carries only the two contract fields"
    );
    // The like really was removed.
    let rows: i64 = sqlx::query_scalar("select count(*) from intro_likes where intro_id=$1")
        .bind(intro)
        .fetch_one(&pool)
        .await?;
    assert_eq!(rows, 0);
    Ok(())
}

/// A viewer who still has access gets the current count on `DELETE`.
#[tokio::test]
async fn a_viewer_who_still_has_access_gets_the_count_on_unlike() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t309-author5").await?;
    let viewer = profile(&pool, "t309-still").await?;
    let intro = published(&pool, author.profile_id, women_18_29()).await?;
    let store = store(&pool, Ok(live_woman_25()));
    store.like(intro, viewer.profile_id).await?;
    let cleared = store.unlike(intro, viewer.profile_id).await?;
    assert!(!cleared.liked);
    assert_eq!(
        cleared.like_count,
        Some(0),
        "a viewer who still satisfies the audience sees the count"
    );
    Ok(())
}

/// A transport failure on the claim read denies the like: unknown is not allowed.
#[tokio::test]
async fn a_claim_transport_failure_denies_the_like() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t309-author6").await?;
    let viewer = profile(&pool, "t309-failure").await?;
    let intro = published(&pool, author.profile_id, women_18_29()).await?;
    let store = store(&pool, Err(ClaimsError::Unavailable));
    let error = store
        .like(intro, viewer.profile_id)
        .await
        .expect_err("an unreadable claim fails closed");
    assert_eq!(error, LikeError::Unavailable);
    Ok(())
}

/// An unrestricted post is likeable with no claim at all (row 6 reads nothing).
#[tokio::test]
async fn an_unrestricted_post_is_likeable_without_any_claim() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t309-author7").await?;
    let viewer = profile(&pool, "t309-noclaim").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE).await?;
    // A port that fails every read: row 6 must never reach it.
    let store = store(&pool, Err(ClaimsError::Unavailable));
    let liked = store.like(intro, viewer.profile_id).await?;
    assert!(liked.liked);
    assert_eq!(liked.like_count, Some(1));
    Ok(())
}
