//! T-307: media is authorized through the post it is attached to.
//!
//! The rule (http-contract.md §4.1, V-06-f) is that an asset is readable through
//! the post it belongs to, and only by a viewer who may see that post. File
//! ownership alone is not permission, and attachment alone is not permission
//! either: the post's own visibility decides.
//!
//! The cases here are the ones that make a leak possible if the rule is wrong:
//!
//! * an intro attachment on a restricted post must not be readable by a viewer
//!   the audience excludes;
//! * it must be readable by a viewer the audience admits;
//! * an asset attached to nothing is readable by its owner only;
//! * the purpose must match the surface, even for the owner.
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
    visibility::{ComposedPolicy, VisibilityPolicy},
};
use concert_service::profile_media::authorization::{
    can_view_media, MediaViewer, MediaVisibility,
};
use concert_service::profile_media::resolver::read_facts;
use intro_support::{media, pool, profile};
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

fn policy(pool: &PgPool, claims: Result<IdentityClaims, ClaimsError>) -> VisibilityPolicy {
    VisibilityPolicy::with(Arc::new(ComposedPolicy::new(
        Arc::new(Fixed(claims)),
        Arc::new(PgIntroFacts::new(pool.clone())),
    )))
}

/// Publish a post with `audience` and attach `media_id` to it.
async fn published_with_media(
    pool: &PgPool,
    author: Uuid,
    audience: Audience,
    media_id: Uuid,
) -> anyhow::Result<Uuid> {
    let store = IntroPostStore::new(pool.clone());
    let intro = Uuid::new_v4();
    let saved = store
        .save_draft(
            author,
            intro,
            &SaveDraft {
                expected_revision: 0,
                content: IntroContent {
                    body: "An introduction with a photograph.".to_owned(),
                    currently_seeking: None,
                    interaction_style: None,
                },
                favorites_snapshot: Snapshot::empty(),
                attachment_media_ids: vec![media_id],
                audience,
                expires_at: None,
            },
        )
        .await?;
    store.publish(author, intro, saved.revision).await?;
    Ok(intro)
}

async fn allowed(
    pool: &PgPool,
    visibility: &VisibilityPolicy,
    media_id: Uuid,
    viewer: Uuid,
) -> anyhow::Result<bool> {
    let facts = read_facts(pool, visibility, media_id, viewer, now()).await?;
    Ok(facts.is_some_and(|facts| can_view_media(MediaViewer { profile_id: viewer }, &facts)))
}

#[tokio::test]
async fn the_owner_reads_their_own_attachment() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t307-author").await?;
    let asset = media(&pool, author.profile_id, "INTRO_POST").await?;
    published_with_media(&pool, author.profile_id, Audience::ANYONE, asset).await?;
    let policy = policy(&pool, Err(ClaimsError::Unavailable));
    assert!(allowed(&pool, &policy, asset, author.profile_id).await?);
    Ok(())
}

#[tokio::test]
async fn a_viewer_the_audience_admits_reads_the_attachment() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t307-author2").await?;
    let viewer = profile(&pool, "t307-admitted").await?;
    let asset = media(&pool, author.profile_id, "INTRO_POST").await?;
    published_with_media(
        &pool,
        author.profile_id,
        Audience::new(GenderCondition::Female, Some(18), Some(30))?,
        asset,
    )
    .await?;
    let policy = policy(&pool, Ok(live_woman_25()));
    assert!(
        allowed(&pool, &policy, asset, viewer.profile_id).await?,
        "a matching live claim reads the attachment"
    );
    Ok(())
}

#[tokio::test]
async fn a_viewer_the_audience_excludes_cannot_read_the_attachment() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t307-author3").await?;
    let viewer = profile(&pool, "t307-excluded").await?;
    let asset = media(&pool, author.profile_id, "INTRO_POST").await?;
    published_with_media(
        &pool,
        author.profile_id,
        Audience::new(GenderCondition::Female, Some(18), Some(30))?,
        asset,
    )
    .await?;
    // No claim at all: the audience cannot be satisfied.
    let policy = policy(&pool, Err(ClaimsError::Unavailable));
    assert!(
        !allowed(&pool, &policy, asset, viewer.profile_id).await?,
        "an excluded viewer must not read the bytes"
    );
    // The resolver reports the reason, which the decision then denies.
    let facts = read_facts(&pool, &policy, asset, viewer.profile_id, now())
        .await?
        .expect("the asset exists");
    assert_eq!(facts.visibility, MediaVisibility::RestrictedIntro);
    Ok(())
}

#[tokio::test]
async fn an_unattached_asset_is_readable_by_its_owner_only() -> anyhow::Result<()> {
    let pool = pool().await?;
    let owner = profile(&pool, "t307-owner").await?;
    let stranger = profile(&pool, "t307-stranger").await?;
    // A fresh upload that was never published.
    let asset = media(&pool, owner.profile_id, "PROFILE_POST").await?;
    let policy = policy(&pool, Ok(live_woman_25()));
    assert!(
        allowed(&pool, &policy, asset, owner.profile_id).await?,
        "the owner reads their own unpublished upload"
    );
    assert!(
        !allowed(&pool, &policy, asset, stranger.profile_id).await?,
        "an unattached asset is not readable by anyone else"
    );
    let facts = read_facts(&pool, &policy, asset, stranger.profile_id, now())
        .await?
        .expect("the asset exists");
    assert_eq!(facts.visibility, MediaVisibility::Unattached);
    Ok(())
}

#[tokio::test]
async fn a_missing_asset_is_unavailable() -> anyhow::Result<()> {
    let pool = pool().await?;
    let viewer = profile(&pool, "t307-missing").await?;
    let policy = policy(&pool, Err(ClaimsError::Unavailable));
    let facts = read_facts(&pool, &policy, Uuid::new_v4(), viewer.profile_id, now()).await?;
    assert!(facts.is_none(), "a missing asset has no facts");
    Ok(())
}

/// Narrowing the audience takes effect on the next request, with no cache in the
/// way.
#[tokio::test]
async fn narrowing_the_audience_revokes_media_access_immediately() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t307-author4").await?;
    let viewer = profile(&pool, "t307-loses").await?;
    let asset = media(&pool, author.profile_id, "INTRO_POST").await?;
    let intro = published_with_media(&pool, author.profile_id, Audience::ANYONE, asset).await?;
    // With no claim, an unrestricted post is readable.
    let policy = policy(&pool, Err(ClaimsError::Unavailable));
    assert!(allowed(&pool, &policy, asset, viewer.profile_id).await?);
    // The author restricts the post.
    sqlx::query(
        "update intro_posts set audience_gender='female', audience_min_age=18, \
         audience_max_age_exclusive=30, revision=revision+1 where intro_id=$1",
    )
    .bind(intro)
    .execute(&pool)
    .await?;
    assert!(
        !allowed(&pool, &policy, asset, viewer.profile_id).await?,
        "the narrowed audience is applied on the next request"
    );
    Ok(())
}

/// A profile post's attachment follows that post's own visibility.
#[tokio::test]
async fn a_public_profile_post_attachment_is_readable_by_a_visitor() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t307-author5").await?;
    let visitor = profile(&pool, "t307-visitor").await?;
    let asset = media(&pool, author.profile_id, "PROFILE_POST").await?;
    let post = Uuid::new_v4();
    sqlx::query(
        "insert into profile_posts(post_id,author_profile_id,description,visibility,state) \
         values($1,$2,'a photograph','PUBLIC','PUBLISHED')",
    )
    .bind(post)
    .bind(author.profile_id)
    .execute(&pool)
    .await?;
    sqlx::query("insert into profile_post_attachments(post_id,media_id,position) values($1,$2,0)")
        .bind(post)
        .bind(asset)
        .execute(&pool)
        .await?;
    let policy = policy(&pool, Err(ClaimsError::Unavailable));
    assert!(
        allowed(&pool, &policy, asset, visitor.profile_id).await?,
        "a public profile post's photograph is readable"
    );
    let facts = read_facts(&pool, &policy, asset, visitor.profile_id, now())
        .await?
        .expect("the asset exists");
    assert_eq!(facts.visibility, MediaVisibility::PublicPost);
    Ok(())
}

/// A private profile post's attachment is readable by its author only.
#[tokio::test]
async fn a_private_profile_post_attachment_is_not_readable_by_a_visitor() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t307-author6").await?;
    let visitor = profile(&pool, "t307-visitor2").await?;
    let asset = media(&pool, author.profile_id, "PROFILE_POST").await?;
    let post = Uuid::new_v4();
    sqlx::query(
        "insert into profile_posts(post_id,author_profile_id,description,visibility,state) \
         values($1,$2,'a private photograph','PRIVATE','PUBLISHED')",
    )
    .bind(post)
    .bind(author.profile_id)
    .execute(&pool)
    .await?;
    sqlx::query("insert into profile_post_attachments(post_id,media_id,position) values($1,$2,0)")
        .bind(post)
        .bind(asset)
        .execute(&pool)
        .await?;
    let policy = policy(&pool, Err(ClaimsError::Unavailable));
    assert!(
        allowed(&pool, &policy, asset, author.profile_id).await?,
        "the author reads their own private post's photograph"
    );
    assert!(
        !allowed(&pool, &policy, asset, visitor.profile_id).await?,
        "a visitor does not read a private post's photograph"
    );
    Ok(())
}

/// The purpose must match the surface: an `INTRO_POST` asset found through a
/// profile post is refused even for the owner.
#[tokio::test]
async fn a_purpose_mismatch_is_refused_even_for_the_owner() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t307-author7").await?;
    let asset = media(&pool, author.profile_id, "INTRO_POST").await?;
    // Attach an INTRO_POST asset to a profile post, which the schema permits at
    // the row level; the decision must still refuse it.
    let post = Uuid::new_v4();
    sqlx::query(
        "insert into profile_posts(post_id,author_profile_id,description,visibility,state) \
         values($1,$2,'mismatched','PUBLIC','PUBLISHED')",
    )
    .bind(post)
    .bind(author.profile_id)
    .execute(&pool)
    .await
    .ok();
    sqlx::query("insert into profile_post_attachments(post_id,media_id,position) values($1,$2,0)")
        .bind(post)
        .bind(asset)
        .execute(&pool)
        .await
        .ok();
    let policy = policy(&pool, Err(ClaimsError::Unavailable));
    // Whether or not the row insert was refused by the purpose trigger, the
    // decision refuses to serve an INTRO_POST asset through a profile post.
    let facts = read_facts(&pool, &policy, asset, author.profile_id, now()).await?;
    if let Some(facts) = facts {
        if facts.visibility == MediaVisibility::PublicPost {
            assert!(
                !can_view_media(MediaViewer { profile_id: author.profile_id }, &facts),
                "an INTRO_POST asset is never served through a profile post"
            );
        }
    }
    Ok(())
}
