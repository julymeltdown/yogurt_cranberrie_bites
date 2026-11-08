//! WP-05 T-401: intro post draft, publish and delete commands.
//!
//! The acceptance cases the task document names:
//!
//! * a draft with an empty body saves; publishing it fails;
//! * a body with contact-bypass content fails to publish;
//! * deleting a post removes its likes and comments and nothing else.
//!
//! The last one is the important one, and it is written as a *snapshot
//! comparison* over every table a post must not touch, not as a spot check.
mod intro_support;

use concert_application::intro_posts::{
    audience::{Audience, GenderCondition},
    content::IntroContent,
    snapshot::Snapshot,
};
use concert_service::intro_posts::commands::{IntroCommandError, IntroPostStore, SaveDraft};
use intro_support::{count, media, pool, profile};
use uuid::Uuid;

fn draft(body: &str) -> SaveDraft {
    SaveDraft {
        expected_revision: 0,
        content: IntroContent {
            body: body.to_owned(),
            currently_seeking: None,
            interaction_style: None,
        },
        favorites_snapshot: Snapshot::empty(),
        attachment_media_ids: Vec::new(),
        audience: Audience::ANYONE,
        expires_at: None,
    }
}

#[tokio::test]
async fn a_draft_with_an_empty_body_saves_but_does_not_publish() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "empty").await?;
    let store = IntroPostStore::new(pool.clone());
    let intro = Uuid::new_v4();
    let saved = store
        .save_draft(author.profile_id, intro, &draft(""))
        .await
        .expect("an empty draft saves");
    assert_eq!(saved.state, "DRAFT");
    assert_eq!(saved.revision, 1);
    let error = store
        .publish(author.profile_id, intro, saved.revision)
        .await
        .expect_err("an empty body must not publish");
    assert_eq!(
        error,
        IntroCommandError::InvalidContent(
            concert_application::intro_posts::content::ContentError::EmptyBody
        )
    );
    Ok(())
}

#[tokio::test]
async fn a_contact_bypass_body_fails_to_publish() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "contact").await?;
    let store = IntroPostStore::new(pool.clone());
    let intro = Uuid::new_v4();
    let saved = store
        .save_draft(author.profile_id, intro, &draft("Call me on 010-1234-5678"))
        .await
        .expect("the draft saves so the author keeps their work");
    let error = store
        .publish(author.profile_id, intro, saved.revision)
        .await
        .expect_err("contact details must not publish");
    assert_eq!(
        error,
        IntroCommandError::InvalidContent(
            concert_application::intro_posts::content::ContentError::ContactBypass
        )
    );
    // The post is still a draft, not partially published.
    let state: String = sqlx::query_scalar("select state from intro_posts where intro_id=$1")
        .bind(intro)
        .fetch_one(&pool)
        .await?;
    assert_eq!(state, "DRAFT");
    Ok(())
}

#[tokio::test]
async fn publishing_sets_the_state_and_the_instant() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "publish").await?;
    let store = IntroPostStore::new(pool.clone());
    let intro = Uuid::new_v4();
    let saved = store
        .save_draft(author.profile_id, intro, &draft("A real introduction."))
        .await?;
    let published = store.publish(author.profile_id, intro, saved.revision).await?;
    assert_eq!(published.state, "PUBLISHED");
    assert_eq!(published.revision, 2);
    assert!(published.published_at.is_some());
    Ok(())
}

/// The acceptance case: deleting a post removes its likes and comments and
/// nothing else.
#[tokio::test]
async fn deleting_a_post_removes_likes_and_comments_and_nothing_else() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "delete").await?;
    let liker = profile(&pool, "delete-liker").await?;
    let commenter = profile(&pool, "delete-commenter").await?;
    let store = IntroPostStore::new(pool.clone());
    let intro = Uuid::new_v4();
    let saved = store
        .save_draft(author.profile_id, intro, &draft("Delete me."))
        .await?;
    store.publish(author.profile_id, intro, saved.revision).await?;

    // The post's own children.
    sqlx::query("insert into intro_likes(intro_id,actor_profile_id) values($1,$2)")
        .bind(intro)
        .bind(liker.profile_id)
        .execute(&pool)
        .await?;
    sqlx::query(
        "insert into intro_comments(comment_id,intro_id,author_profile_id,body) \
         values($1,$2,$3,'a comment')",
    )
    .bind(Uuid::new_v4())
    .bind(intro)
    .bind(commenter.profile_id)
    .execute(&pool)
    .await?;

    // Things the delete must leave alone. Each is created *before* the
    // snapshot so a change is visible.
    let room = Uuid::new_v4();
    sqlx::query(
        "insert into profile_rooms(owner_profile_id,layout,visibility) \
         values($1,'{}'::jsonb,'PUBLIC')",
    )
    .bind(author.profile_id)
    .execute(&pool)
    .await?;
    let profile_post = Uuid::new_v4();
    sqlx::query(
        "insert into profile_posts(post_id,author_profile_id,description,visibility,state) \
         values($1,$2,'a profile post','PUBLIC','PUBLISHED')",
    )
    .bind(profile_post)
    .bind(author.profile_id)
    .execute(&pool)
    .await?;
    let attachment = media(&pool, author.profile_id, "INTRO_POST").await?;
    sqlx::query("insert into intro_attachments(intro_id,media_id,position) values($1,$2,0)")
        .bind(intro)
        .bind(attachment)
        .execute(&pool)
        .await?;
    let favorite = Uuid::new_v4();
    sqlx::query(
        "insert into discovery_favorite_items(profile_id,category,catalog_item_id,label,position) \
         values($1,'music',$2,'A Band',0)",
    )
    .bind(author.profile_id)
    .bind(favorite)
    .execute(&pool)
    .await?;

    // Snapshot every table the delete must not touch.
    let before = (
        count(&pool, "profile_rooms").await?,
        count(&pool, "profile_posts").await?,
        count(&pool, "discovery_favorite_items").await?,
        count(&pool, "profile_media").await?,
        count(&pool, "discovery_profiles").await?,
    );

    let revision: i64 = sqlx::query_scalar("select revision from intro_posts where intro_id=$1")
        .bind(intro)
        .fetch_one(&pool)
        .await?;
    store
        .delete(author.profile_id, intro, revision)
        .await
        .expect("the author may delete their own post");

    // The post's children are gone.
    let likes: i64 = sqlx::query_scalar("select count(*) from intro_likes where intro_id=$1")
        .bind(intro)
        .fetch_one(&pool)
        .await?;
    let comments: i64 =
        sqlx::query_scalar("select count(*) from intro_comments where intro_id=$1")
            .bind(intro)
            .fetch_one(&pool)
            .await?;
    let attachments: i64 =
        sqlx::query_scalar("select count(*) from intro_attachments where intro_id=$1")
            .bind(intro)
            .fetch_one(&pool)
            .await?;
    assert_eq!((likes, comments, attachments), (0, 0, 0));

    // The post is soft-deleted, not physically removed.
    let state: String = sqlx::query_scalar("select state from intro_posts where intro_id=$1")
        .bind(intro)
        .fetch_one(&pool)
        .await?;
    assert_eq!(state, "DELETED");

    // Nothing else changed.
    let after = (
        count(&pool, "profile_rooms").await?,
        count(&pool, "profile_posts").await?,
        count(&pool, "discovery_favorite_items").await?,
        count(&pool, "profile_media").await?,
        count(&pool, "discovery_profiles").await?,
    );
    assert_eq!(before, after, "the delete must touch no other table");
    // Specifically: the room, the profile post, the favorite and the media file
    // are all still there.
    let room_gone: bool =
        sqlx::query_scalar("select not exists(select 1 from profile_rooms where owner_profile_id=$1)")
            .bind(author.profile_id)
            .fetch_one(&pool)
            .await?;
    assert!(!room_gone, "the room must survive");
    let media_gone: bool =
        sqlx::query_scalar("select not exists(select 1 from profile_media where media_id=$1)")
            .bind(attachment)
            .fetch_one(&pool)
            .await?;
    assert!(!media_gone, "the media file must survive");
    Ok(())
}

#[tokio::test]
async fn a_non_author_cannot_save_publish_or_delete() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "owner").await?;
    let stranger = profile(&pool, "stranger").await?;
    let store = IntroPostStore::new(pool.clone());
    let intro = Uuid::new_v4();
    let saved = store
        .save_draft(author.profile_id, intro, &draft("Mine."))
        .await?;
    // Every foreign attempt is `Unavailable`, the same answer a missing post
    // gets, so the caller cannot tell the post exists.
    assert_eq!(
        store
            .publish(stranger.profile_id, intro, saved.revision)
            .await
            .expect_err("not the author"),
        IntroCommandError::Unavailable
    );
    assert_eq!(
        store
            .delete(stranger.profile_id, intro, saved.revision)
            .await
            .expect_err("not the author"),
        IntroCommandError::Unavailable
    );
    let mut update = draft("Hijacked.");
    update.expected_revision = saved.revision;
    assert_eq!(
        store
            .save_draft(stranger.profile_id, intro, &update)
            .await
            .expect_err("not the author"),
        IntroCommandError::Unavailable
    );
    Ok(())
}

#[tokio::test]
async fn a_stale_revision_is_a_conflict() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "revision").await?;
    let store = IntroPostStore::new(pool.clone());
    let intro = Uuid::new_v4();
    let saved = store
        .save_draft(author.profile_id, intro, &draft("First."))
        .await?;
    let mut second = draft("Second.");
    second.expected_revision = saved.revision;
    let updated = store.save_draft(author.profile_id, intro, &second).await?;
    assert_eq!(updated.revision, saved.revision + 1);
    // Replaying the first revision now conflicts.
    let mut stale = draft("Stale.");
    stale.expected_revision = saved.revision;
    assert_eq!(
        store
            .save_draft(author.profile_id, intro, &stale)
            .await
            .expect_err("stale"),
        IntroCommandError::VersionConflict
    );
    Ok(())
}

#[tokio::test]
async fn a_created_post_requires_revision_zero() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "creation").await?;
    let store = IntroPostStore::new(pool.clone());
    let mut create = draft("New.");
    create.expected_revision = 3;
    assert_eq!(
        store
            .save_draft(author.profile_id, Uuid::new_v4(), &create)
            .await
            .expect_err("creation must start at revision 0"),
        IntroCommandError::VersionConflict
    );
    Ok(())
}

#[tokio::test]
async fn a_deleted_post_is_terminal() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "terminal").await?;
    let store = IntroPostStore::new(pool.clone());
    let intro = Uuid::new_v4();
    let saved = store
        .save_draft(author.profile_id, intro, &draft("Gone soon."))
        .await?;
    store.delete(author.profile_id, intro, saved.revision).await?;
    let mut again = draft("Back.");
    again.expected_revision = saved.revision + 1;
    assert_eq!(
        store
            .save_draft(author.profile_id, intro, &again)
            .await
            .expect_err("deleted is terminal"),
        IntroCommandError::InvalidTransition
    );
    Ok(())
}

#[tokio::test]
async fn the_audience_round_trips_through_the_store() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "audience").await?;
    let store = IntroPostStore::new(pool.clone());
    let intro = Uuid::new_v4();
    let mut create = draft("Ages 18 to 29.");
    create.audience =
        Audience::new(GenderCondition::Female, Some(18), Some(30)).expect("valid audience");
    let saved = store.save_draft(author.profile_id, intro, &create).await?;
    assert_eq!(saved.audience, create.audience);
    // The summary is server-derived and matches the contract's example.
    let wire = saved.to_wire();
    assert_eq!(wire["audience_summary"], "Women \u{00b7} Ages 18\u{2013}29");
    assert_eq!(wire["audience"]["minimum_age"], 18);
    assert_eq!(wire["audience"]["maximum_age_exclusive"], 30);
    // The viewer projection never carries the audience: `OwnIntroPost` does,
    // and that is the author-only type.
    assert!(wire.get("audience").is_some());
    Ok(())
}

#[tokio::test]
async fn the_database_refuses_an_unpaired_audience() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "paired").await?;
    let error = sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,audience_min_age,state) \
         values($1,$2,'body',18,'DRAFT')",
    )
    .bind(Uuid::new_v4())
    .bind(author.profile_id)
    .execute(&pool)
    .await
    .expect_err("an unpaired bound must be refused");
    assert_eq!(
        intro_support::constraint(&error).as_deref(),
        Some("intro_posts_audience_bounds_paired")
    );
    Ok(())
}

#[tokio::test]
async fn a_published_post_requires_a_body_at_the_database() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "published-shape").await?;
    let error = sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,state,published_at) \
         values($1,$2,'   ','PUBLISHED',clock_timestamp())",
    )
    .bind(Uuid::new_v4())
    .bind(author.profile_id)
    .execute(&pool)
    .await
    .expect_err("a published post needs a body");
    assert_eq!(
        intro_support::constraint(&error).as_deref(),
        Some("intro_posts_published_shape")
    );
    Ok(())
}
