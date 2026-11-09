//! T-311: the `INTRO_LIKED` notification, under the real policy and under
//! concurrency.
//!
//! WP-05 built the mechanism (migration `0087_intro_like_notifications`): the
//! table's primary key is `(recipient, intro, actor)`, the like command inserts
//! with `on conflict do nothing`, and a trigger removes the notification when the
//! like is withdrawn. This file does **not** add a second mechanism. It proves
//! the existing one holds in the two situations the acceptance criteria name and
//! that WP-05's tests do not cover:
//!
//! * a like on a **restricted** post, which is only reachable now that the claim
//!   adapter is wired, notifies exactly once;
//! * two **concurrent** `PUT`s — not two sequential ones — produce one row and
//!   one notification, because the dedupe is a database constraint and not a
//!   read-then-write check;
//! * a like that is withdrawn and repeated notifies again, because the second
//!   like is a genuinely new like;
//! * the notification carries no attribute of the actor: not the age, not the
//!   gender, not the claim.
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
    likes::LikeStore,
    visibility::{ComposedPolicy, VisibilityPolicy},
};
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

fn store(pool: &PgPool) -> LikeStore {
    let policy = VisibilityPolicy::with(Arc::new(ComposedPolicy::new(
        Arc::new(Fixed(Ok(live_woman_25()))),
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
                    body: "An introduction that will be liked.".to_owned(),
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

async fn notifications(pool: &PgPool, intro: Uuid) -> anyhow::Result<i64> {
    Ok(
        sqlx::query_scalar("select count(*) from intro_like_notifications where intro_id=$1")
            .bind(intro)
            .fetch_one(pool)
            .await?,
    )
}

/// A like on a restricted post notifies exactly once. Before the claim adapter
/// was wired this path was unreachable, so WP-05's tests could not cover it.
#[tokio::test]
async fn a_like_on_a_restricted_post_notifies_exactly_once() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t311-author").await?;
    let liker = profile(&pool, "t311-liker").await?;
    let intro = published(
        &pool,
        author.profile_id,
        Audience::new(GenderCondition::Female, Some(18), Some(30))?,
    )
    .await?;
    let store = store(&pool);
    store.like(intro, liker.profile_id).await?;
    assert_eq!(notifications(&pool, intro).await?, 1);
    // The recipient is the author and the actor is the liker.
    let (recipient, actor): (Uuid, Uuid) = sqlx::query_as(
        "select recipient_profile_id, actor_profile_id from intro_like_notifications \
         where intro_id=$1",
    )
    .bind(intro)
    .fetch_one(&pool)
    .await?;
    assert_eq!(recipient, author.profile_id);
    assert_eq!(actor, liker.profile_id);
    Ok(())
}

/// Two **concurrent** `PUT`s produce one row and one notification.
///
/// This is the case that distinguishes a database constraint from a
/// read-then-write check: both requests observe "no like" and both attempt the
/// insert, and only the primary key decides.
#[tokio::test]
async fn concurrent_likes_produce_one_row_and_one_notification() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t311-author2").await?;
    let liker = profile(&pool, "t311-concurrent").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE).await?;
    let store = Arc::new(store(&pool));
    let mut handles = Vec::new();
    for _ in 0..8 {
        let store = store.clone();
        let liker = liker.profile_id;
        handles.push(tokio::spawn(async move { store.like(intro, liker).await }));
    }
    let mut succeeded = 0;
    for handle in handles {
        match handle.await.expect("the task did not panic") {
            Ok(state) => {
                assert!(state.liked);
                succeeded += 1;
            }
            Err(error) => panic!("a concurrent like failed: {error:?}"),
        }
    }
    assert_eq!(succeeded, 8, "every PUT answers success");
    let rows: i64 = sqlx::query_scalar("select count(*) from intro_likes where intro_id=$1")
        .bind(intro)
        .fetch_one(&pool)
        .await?;
    assert_eq!(rows, 1, "eight concurrent PUTs produce one row");
    assert_eq!(
        notifications(&pool, intro).await?,
        1,
        "eight concurrent PUTs produce one notification"
    );
    Ok(())
}

/// Withdrawing and liking again notifies again: the second like is a new like,
/// and the trigger removed the first notification with the first like.
#[tokio::test]
async fn a_withdrawn_and_repeated_like_notifies_again() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t311-author3").await?;
    let liker = profile(&pool, "t311-repeat").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE).await?;
    let store = store(&pool);
    store.like(intro, liker.profile_id).await?;
    assert_eq!(notifications(&pool, intro).await?, 1);
    store.unlike(intro, liker.profile_id).await?;
    assert_eq!(
        notifications(&pool, intro).await?,
        0,
        "the notification follows the like out"
    );
    store.like(intro, liker.profile_id).await?;
    assert_eq!(
        notifications(&pool, intro).await?,
        1,
        "the new like notifies once"
    );
    Ok(())
}

/// The notification carries no attribute of the actor.
#[tokio::test]
async fn the_notification_carries_no_actor_attributes() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t311-author4").await?;
    let liker = profile(&pool, "t311-private").await?;
    let intro = published(
        &pool,
        author.profile_id,
        Audience::new(GenderCondition::Female, Some(18), Some(30))?,
    )
    .await?;
    store(&pool).like(intro, liker.profile_id).await?;
    // The table's columns are exactly the four the contract allows. A column
    // holding an age or a gender would be a privacy defect; the assertion is on
    // the schema, so adding one later fails here.
    let columns: Vec<String> = sqlx::query_scalar(
        "select column_name::text from information_schema.columns \
         where table_schema='public' and table_name='intro_like_notifications' \
         order by ordinal_position",
    )
    .fetch_all(&pool)
    .await?;
    assert_eq!(
        columns,
        vec![
            "recipient_profile_id".to_owned(),
            "intro_id".to_owned(),
            "actor_profile_id".to_owned(),
            "created_at".to_owned(),
        ],
        "the notification carries only identity and time"
    );
    Ok(())
}

/// A like that the audience does not admit writes no notification either.
#[tokio::test]
async fn a_refused_like_writes_no_notification() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t311-author5").await?;
    let liker = profile(&pool, "t311-refused").await?;
    let intro = published(
        &pool,
        author.profile_id,
        Audience::new(GenderCondition::Male, None, None)?,
    )
    .await?;
    // The fixed claim is female, so a male-only audience excludes the liker.
    let error = store(&pool)
        .like(intro, liker.profile_id)
        .await
        .expect_err("the audience excludes this liker");
    assert_eq!(error.status(), 404);
    assert_eq!(notifications(&pool, intro).await?, 0);
    Ok(())
}
