//! WP-05 T-404: comments and replies.
//!
//! The acceptance cases the task document names:
//!
//! * a comment on a restricted post by an unauthorized viewer is `404`;
//! * a cross-post parent is rejected.
//!
//! Plus the documented deletion rule: deleting a comment keeps its replies.
//! That rule is stated in `intro_posts::comments` and in the contract; this test
//! is what makes it true.
mod intro_support;

use concert_application::intro_posts::{
    audience::{Audience, GenderCondition},
    content::IntroContent,
    snapshot::Snapshot,
};
use concert_service::intro_posts::{
    commands::{IntroPostStore, SaveDraft},
    comments::{CommentError, CommentStore},
};
use intro_support::{pool, profile};
use uuid::Uuid;

/// Publish a post with `audience` and return its id.
async fn published(
    pool: &sqlx::PgPool,
    author: Uuid,
    audience: Audience,
    body: &str,
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
                    body: body.to_owned(),
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

/// The acceptance case: a comment on a restricted post by an unauthorized
/// viewer is `404`.
#[tokio::test]
async fn a_comment_on_a_restricted_post_by_an_unauthorized_viewer_is_404() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "comment-author").await?;
    let viewer = profile(&pool, "comment-viewer").await?;
    // A post restricted to women. Until WP-04's claim reader is composed, the
    // fail-closed policy denies every restricted post to everyone but the
    // author, which is exactly the "unauthorized viewer" case.
    let intro = published(
        &pool,
        author.profile_id,
        Audience::new(GenderCondition::Female, None, None)?,
        "Women only.",
    )
    .await?;
    let store = CommentStore::new(pool.clone());
    let error = store
        .create(intro, viewer.profile_id, "hello", None)
        .await
        .expect_err("an unauthorized viewer must not comment");
    assert_eq!(error, CommentError::Unavailable);
    assert_eq!(error.status(), 404);
    // Reading the comments is denied the same way, so comments cannot be used
    // to probe the post.
    assert_eq!(
        store
            .list(intro, viewer.profile_id, 20, None)
            .await
            .expect_err("an unauthorized viewer must not read comments"),
        CommentError::Unavailable
    );
    // The author can comment on their own restricted post.
    assert!(store
        .create(intro, author.profile_id, "mine", None)
        .await
        .is_ok());
    Ok(())
}

/// The acceptance case: a cross-post parent is rejected.
#[tokio::test]
async fn a_cross_post_parent_is_rejected() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "cross-author").await?;
    let first = published(&pool, author.profile_id, Audience::ANYONE, "First post.").await?;
    let second = published(&pool, author.profile_id, Audience::ANYONE, "Second post.").await?;
    let store = CommentStore::new(pool.clone());
    let parent = store
        .create(first, author.profile_id, "a comment", None)
        .await?;
    let error = store
        .create(second, author.profile_id, "a reply", Some(parent.comment_id))
        .await
        .expect_err("a parent on another post must be refused");
    assert!(matches!(error, CommentError::Invalid(_)));
    assert_eq!(error.status(), 400);
    Ok(())
}

/// A reply to a reply is refused, so a thread stays one level deep.
#[tokio::test]
async fn a_reply_to_a_reply_is_refused() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "reply-depth").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE, "Post.").await?;
    let store = CommentStore::new(pool.clone());
    let parent = store.create(intro, author.profile_id, "top", None).await?;
    let reply = store
        .create(intro, author.profile_id, "reply", Some(parent.comment_id))
        .await?;
    let error = store
        .create(intro, author.profile_id, "reply to reply", Some(reply.comment_id))
        .await
        .expect_err("a reply to a reply must be refused");
    assert!(matches!(error, CommentError::Invalid(_)));
    Ok(())
}

/// The documented deletion rule: deleting a comment keeps its replies.
#[tokio::test]
async fn deleting_a_comment_keeps_its_replies() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "delete-parent").await?;
    let replier = profile(&pool, "delete-replier").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE, "Post.").await?;
    let store = CommentStore::new(pool.clone());
    let parent = store.create(intro, author.profile_id, "top", None).await?;
    let reply = store
        .create(intro, replier.profile_id, "a reply", Some(parent.comment_id))
        .await?;
    // The parent's author deletes it.
    store
        .delete(intro, parent.comment_id, author.profile_id)
        .await
        .expect("the comment author may delete their comment");
    let page = store.list(intro, author.profile_id, 20, None).await?;
    assert_eq!(page.items.len(), 2, "the reply must survive");
    let deleted = page
        .items
        .iter()
        .find(|item| item.comment_id == parent.comment_id)
        .expect("the deleted comment is still listed");
    assert!(deleted.deleted);
    assert_eq!(deleted.body, "", "a deleted body is never returned");
    let surviving = page
        .items
        .iter()
        .find(|item| item.comment_id == reply.comment_id)
        .expect("the reply is still listed");
    assert!(!surviving.deleted);
    assert_eq!(surviving.body, "a reply");
    assert_eq!(surviving.parent_comment_id, Some(parent.comment_id));
    Ok(())
}

/// A stranger cannot delete somebody else's comment.
#[tokio::test]
async fn a_stranger_cannot_delete_a_comment() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "del-owner").await?;
    let other = profile(&pool, "del-other").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE, "Post.").await?;
    let store = CommentStore::new(pool.clone());
    let comment = store.create(intro, author.profile_id, "mine", None).await?;
    assert_eq!(
        store
            .delete(intro, comment.comment_id, other.profile_id)
            .await
            .expect_err("not the comment author"),
        CommentError::CommentUnavailable
    );
    Ok(())
}

/// Listing is paginated, oldest first, and never post-filtered.
#[tokio::test]
async fn listing_is_paginated_and_ordered() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "page-author").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE, "Post.").await?;
    let store = CommentStore::new(pool.clone());
    let mut created = Vec::new();
    for index in 0..5 {
        created.push(
            store
                .create(intro, author.profile_id, &format!("comment {index}"), None)
                .await?
                .comment_id,
        );
    }
    let mut collected = Vec::new();
    let mut cursor = None;
    loop {
        let page = store.list(intro, author.profile_id, 2, cursor.as_deref()).await?;
        assert!(page.items.len() <= 2);
        collected.extend(page.items.iter().map(|item| item.comment_id));
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert_eq!(collected, created, "paging preserves order and loses nothing");
    Ok(())
}

/// A comment body is bounded at the database as well as in the command.
#[tokio::test]
async fn the_database_bounds_the_comment_body() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "body-bound").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE, "Post.").await?;
    let error = sqlx::query(
        "insert into intro_comments(comment_id,intro_id,author_profile_id,body) values($1,$2,$3,$4)",
    )
    .bind(Uuid::new_v4())
    .bind(intro)
    .bind(author.profile_id)
    .bind("a".repeat(501))
    .execute(&pool)
    .await
    .expect_err("501 characters must be refused");
    assert_eq!(
        intro_support::constraint(&error).as_deref(),
        Some("intro_comments_body_length")
    );
    Ok(())
}

/// A deleted comment is terminal at the database.
#[tokio::test]
async fn a_deleted_comment_is_terminal() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "terminal-comment").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE, "Post.").await?;
    let store = CommentStore::new(pool.clone());
    let comment = store.create(intro, author.profile_id, "gone", None).await?;
    store.delete(intro, comment.comment_id, author.profile_id).await?;
    let error = sqlx::query("update intro_comments set state='PUBLISHED' where comment_id=$1")
        .bind(comment.comment_id)
        .execute(&pool)
        .await
        .expect_err("a deleted comment cannot be revived");
    assert_eq!(
        intro_support::constraint(&error).as_deref(),
        Some("intro_comments_deleted_terminal")
    );
    Ok(())
}
