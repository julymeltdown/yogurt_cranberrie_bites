//! T-308: shared link resolution.
//!
//! The requirement is absolute: a resolution the viewer may not see must be
//! indistinguishable from a link that does not exist, and no restricted content
//! may appear in the preview. These tests assert exactly that, and nothing about
//! *why* a link failed is observable.
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
    links::{anonymous_scope, resolve, to_wire, LinkResolution, LinkViewer},
    visibility_sql::VisibilityScope,
};
use intro_support::{media, pool, profile};
use sqlx::PgPool;
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

/// A scope builder that always answers with the anonymous scope. The link tests
/// exercise the SQL predicate; the claim mapping is proven in `claims_adapter`.
fn anonymous(viewer: Uuid, at: DateTime<Utc>) -> VisibilityScope {
    anonymous_scope(viewer, at)
}

/// A scope builder for a viewer with a live matching claim.
fn with_claim(viewer: Uuid, at: DateTime<Utc>) -> VisibilityScope {
    use concert_service::intro_posts::visibility_sql::ViewerAttributes;
    let snapshot = live_woman_25();
    let age = snapshot
        .age
        .known()
        .and_then(|claim| concert_application::intro_visibility::current_age(claim, at));
    VisibilityScope::new(
        viewer,
        concert_application::intro_visibility::BlockState::None,
        concert_application::intro_visibility::AuthorStanding::Active,
        ViewerAttributes::read(age, Some(Gender::Female), false),
        at,
    )
}

async fn published(
    pool: &PgPool,
    author: Uuid,
    audience: Audience,
    attachments: Vec<Uuid>,
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
                    body: "A shared introduction.".to_owned(),
                    currently_seeking: None,
                    interaction_style: None,
                },
                favorites_snapshot: Snapshot::empty(),
                attachment_media_ids: attachments,
                audience,
                expires_at: None,
            },
        )
        .await?;
    store.publish(author, intro, saved.revision).await?;
    Ok(intro)
}

/// The core requirement: a missing link and an unauthorized link produce the
/// same value, and the same wire shape.
#[tokio::test]
async fn a_missing_link_and_an_unauthorized_link_are_indistinguishable() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t308-author").await?;
    // A restricted post the anonymous viewer cannot see.
    let restricted = published(
        &pool,
        author.profile_id,
        Audience::new(GenderCondition::Female, Some(18), Some(30))?,
        Vec::new(),
    )
    .await?;
    let missing = Uuid::new_v4();
    let unauthorized = resolve(&pool, restricted, LinkViewer::Anonymous, now(), anonymous).await?;
    let absent = resolve(&pool, missing, LinkViewer::Anonymous, now(), anonymous).await?;
    assert_eq!(
        unauthorized, absent,
        "the two cases must be the same value"
    );
    assert_eq!(unauthorized, LinkResolution::Unavailable);
    // And the same wire shape.
    assert_eq!(to_wire(&unauthorized), to_wire(&absent));
    // The wire form carries nothing but the status.
    let wire = to_wire(&unauthorized);
    let object = wire.as_object().expect("an object");
    assert_eq!(object.len(), 1, "an unavailable link carries only its status");
    assert_eq!(wire["status"], "UNAVAILABLE");
    Ok(())
}

/// An unpublished post is the same answer as a missing one.
#[tokio::test]
async fn an_unpublished_link_is_the_same_answer_as_missing() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t308-author2").await?;
    // A draft: saved, never published.
    let store = IntroPostStore::new(pool.clone());
    let draft = Uuid::new_v4();
    store
        .save_draft(
            author.profile_id,
            draft,
            &SaveDraft {
                expected_revision: 0,
                content: IntroContent {
                    body: "A draft nobody should see.".to_owned(),
                    currently_seeking: None,
                    interaction_style: None,
                },
                favorites_snapshot: Snapshot::empty(),
                attachment_media_ids: Vec::new(),
                audience: Audience::ANYONE,
                expires_at: None,
            },
        )
        .await?;
    let unavailable = resolve(&pool, draft, LinkViewer::Anonymous, now(), anonymous).await?;
    let missing = resolve(&pool, Uuid::new_v4(), LinkViewer::Anonymous, now(), anonymous).await?;
    assert_eq!(unavailable, missing);
    assert_eq!(unavailable, LinkResolution::Unavailable);
    Ok(())
}

/// An unrestricted post resolves for an anonymous viewer, with ids and no bytes.
#[tokio::test]
async fn an_unrestricted_link_resolves_without_any_bytes() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t308-author3").await?;
    let asset = media(&pool, author.profile_id, "INTRO_POST").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE, vec![asset]).await?;
    let resolution = resolve(&pool, intro, LinkViewer::Anonymous, now(), anonymous).await?;
    let LinkResolution::Available(preview) = resolution else {
        panic!("an unrestricted post resolves for an anonymous viewer");
    };
    assert_eq!(preview.intro_id, intro);
    assert_eq!(preview.attachment_media_ids, vec![asset]);
    // The preview carries no bytes and no thumbnail: the wire form has no field
    // that could hold them.
    let wire = to_wire(&LinkResolution::Available(preview));
    let object = wire.as_object().expect("an object");
    for forbidden in ["bytes", "thumbnail", "content", "author", "audience", "expires_at"] {
        assert!(
            !object.contains_key(forbidden),
            "a preview must not carry {forbidden}"
        );
    }
    assert_eq!(object["status"], "AVAILABLE");
    Ok(())
}

/// A viewer the audience admits resolves the link.
#[tokio::test]
async fn a_viewer_the_audience_admits_resolves_the_link() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t308-author4").await?;
    let viewer = profile(&pool, "t308-admitted").await?;
    let intro = published(
        &pool,
        author.profile_id,
        Audience::new(GenderCondition::Female, Some(18), Some(30))?,
        Vec::new(),
    )
    .await?;
    let resolution = resolve(
        &pool,
        intro,
        LinkViewer::Profile(ProfileId::from_uuid(viewer.profile_id)),
        now(),
        with_claim,
    )
    .await?;
    assert!(matches!(resolution, LinkResolution::Available(_)));
    Ok(())
}

/// A viewer the audience excludes gets the same answer as a missing link.
#[tokio::test]
async fn a_viewer_the_audience_excludes_gets_the_same_answer_as_missing() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t308-author5").await?;
    let viewer = profile(&pool, "t308-excluded").await?;
    let intro = published(
        &pool,
        author.profile_id,
        Audience::new(GenderCondition::Female, Some(18), Some(30))?,
        Vec::new(),
    )
    .await?;
    // The viewer has no claim, so the audience cannot be satisfied.
    let excluded = resolve(
        &pool,
        intro,
        LinkViewer::Profile(ProfileId::from_uuid(viewer.profile_id)),
        now(),
        anonymous,
    )
    .await?;
    let missing = resolve(
        &pool,
        Uuid::new_v4(),
        LinkViewer::Profile(ProfileId::from_uuid(viewer.profile_id)),
        now(),
        anonymous,
    )
    .await?;
    assert_eq!(excluded, missing);
    assert_eq!(excluded, LinkResolution::Unavailable);
    Ok(())
}

/// The author's own link always resolves.
#[tokio::test]
async fn the_author_resolves_their_own_link() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t308-author6").await?;
    let intro = published(
        &pool,
        author.profile_id,
        Audience::new(GenderCondition::Female, Some(18), Some(30))?,
        Vec::new(),
    )
    .await?;
    let resolution = resolve(
        &pool,
        intro,
        LinkViewer::Profile(ProfileId::from_uuid(author.profile_id)),
        now(),
        anonymous,
    )
    .await?;
    assert!(matches!(resolution, LinkResolution::Available(_)));
    Ok(())
}

/// A nil intro id never resolves.
#[tokio::test]
async fn a_nil_link_never_resolves() -> anyhow::Result<()> {
    let pool = pool().await?;
    let resolution = resolve(&pool, Uuid::nil(), LinkViewer::Anonymous, now(), anonymous).await?;
    assert_eq!(resolution, LinkResolution::Unavailable);
    Ok(())
}
