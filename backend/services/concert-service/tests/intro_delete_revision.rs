//! `deleteIntroPost`: the revision the publish returns is the one the delete wants.
//!
//! The end-to-end gate reported `INVALID_INPUT` on the intro delete. The product
//! side of that question is two claims, and this file pins both:
//!
//! 1. the route parser accepts the contract's shape — a bodyless `DELETE` whose
//!    `expected_revision` is in the query;
//! 2. the revision `publish` returns is the post-publish revision, and a delete
//!    naming it succeeds.
//!
//! If both hold, the `INVALID_INPUT` came from somewhere the product does not
//! control, and the gate's request is the thing to look at.
mod intro_support;

use concert_application::intro_posts::{
    audience::Audience, content::IntroContent, snapshot::Snapshot,
};
use concert_service::intro_posts::{
    commands::{IntroCommandError, IntroPostStore, SaveDraft},
    http::{self, IntroOperation},
};
use intro_support::{pool, profile};
use uuid::Uuid;

/// The parser accepts exactly the target the gate builds.
#[test]
fn the_delete_route_parses_the_contract_shape() {
    let intro = Uuid::new_v4();
    let target = format!("/api/me/intro-posts/{intro}?expected_revision=2");
    let parsed = http::parse("DELETE", &target).expect("the delete route parses");
    assert_eq!(parsed.operation, IntroOperation::DeleteIntroPost(intro));
    assert_eq!(
        parsed.query.expected_revision,
        Some(2),
        "the revision is read from the query"
    );
}

/// A bodyless delete with no revision is refused by the parser, so the transport
/// can answer a precise error rather than a version conflict.
#[test]
fn a_delete_without_a_revision_is_refused() {
    let intro = Uuid::new_v4();
    let parsed = http::parse("DELETE", &format!("/api/me/intro-posts/{intro}"))
        .expect("the route parses");
    assert_eq!(parsed.query.expected_revision, None);
}

/// The revision `publish` returns is the one the delete must name.
#[tokio::test]
async fn the_published_revision_is_the_one_the_delete_accepts() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "delete-author").await?;
    let store = IntroPostStore::new(pool.clone());
    let intro = Uuid::new_v4();
    let draft = store
        .save_draft(
            author.profile_id,
            intro,
            &SaveDraft {
                expected_revision: 0,
                content: IntroContent {
                    body: "A draft that will be published and deleted.".to_owned(),
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
    assert_eq!(draft.revision, 1, "a new draft is revision 1");
    let published = store.publish(author.profile_id, intro, draft.revision).await?;
    assert_eq!(
        published.revision, 2,
        "publish advances the revision and returns the post-publish value"
    );
    // The delete naming the published revision succeeds.
    store
        .delete(author.profile_id, intro, published.revision)
        .await
        .expect("the revision publish returned is the one delete accepts");
    // A second delete is no longer a valid transition.
    assert_eq!(
        store
            .delete(author.profile_id, intro, published.revision)
            .await
            .expect_err("already deleted"),
        IntroCommandError::InvalidTransition
    );
    Ok(())
}

/// A delete naming a stale revision is a version conflict, not invalid input.
#[tokio::test]
async fn a_stale_revision_is_a_version_conflict() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "delete-author2").await?;
    let store = IntroPostStore::new(pool.clone());
    let intro = Uuid::new_v4();
    let draft = store
        .save_draft(
            author.profile_id,
            intro,
            &SaveDraft {
                expected_revision: 0,
                content: IntroContent {
                    body: "A draft with a stale delete.".to_owned(),
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
    let published = store.publish(author.profile_id, intro, draft.revision).await?;
    assert_eq!(
        store
            .delete(author.profile_id, intro, draft.revision)
            .await
            .expect_err("the draft revision is stale after publish"),
        IntroCommandError::VersionConflict,
        "a stale revision is a conflict, which is a different answer from invalid input"
    );
    // The correct revision still works afterwards.
    store.delete(author.profile_id, intro, published.revision).await?;
    Ok(())
}
