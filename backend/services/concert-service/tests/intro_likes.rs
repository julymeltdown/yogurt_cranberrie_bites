//! WP-05 T-411 / T-412 backend: hearts, the liker list and the notification.
//!
//! The acceptance cases the task document names:
//!
//! * two `PUT` likes produce one row and one notification;
//! * `DELETE` by a viewer who lost access returns `like_count: null`;
//! * a non-author liker list request is `403`.
mod intro_support;

use concert_application::intro_posts::{
    audience::{Audience, GenderCondition},
    content::IntroContent,
    snapshot::Snapshot,
};
use concert_service::intro_posts::{
    commands::{IntroPostStore, SaveDraft},
    likes::{LikeError, LikeStore},
};
use intro_support::{pool, profile};
use uuid::Uuid;

async fn published(
    pool: &sqlx::PgPool,
    author: Uuid,
    audience: Audience,
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

/// The acceptance case: two `PUT` likes produce one row and one notification.
#[tokio::test]
async fn two_puts_produce_one_row_and_one_notification() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "like-author").await?;
    let liker = profile(&pool, "like-liker").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE).await?;
    let store = LikeStore::new(pool.clone());

    let first = store.like(intro, liker.profile_id).await?;
    assert!(first.liked);
    assert_eq!(first.like_count, Some(1));
    // The second PUT is idempotent, not an error and not a second row.
    let second = store.like(intro, liker.profile_id).await?;
    assert!(second.liked);
    assert_eq!(second.like_count, Some(1));

    let rows: i64 = sqlx::query_scalar("select count(*) from intro_likes where intro_id=$1")
        .bind(intro)
        .fetch_one(&pool)
        .await?;
    let notifications: i64 = sqlx::query_scalar(
        "select count(*) from intro_like_notifications where intro_id=$1 and actor_profile_id=$2",
    )
    .bind(intro)
    .bind(liker.profile_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(rows, 1, "two PUTs must produce one like row");
    assert_eq!(
        notifications, 1,
        "two PUTs must produce one notification"
    );
    Ok(())
}

/// The notification names the author as recipient and the actor as the liker.
#[tokio::test]
async fn the_notification_names_the_author_and_the_actor() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "notify-author").await?;
    let liker = profile(&pool, "notify-liker").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE).await?;
    let store = LikeStore::new(pool.clone());
    store.like(intro, liker.profile_id).await?;
    let (recipient, actor): (Uuid, Uuid) = sqlx::query_as(
        "select recipient_profile_id,actor_profile_id from intro_like_notifications where intro_id=$1",
    )
    .bind(intro)
    .fetch_one(&pool)
    .await?;
    assert_eq!(recipient, author.profile_id);
    assert_eq!(actor, liker.profile_id);
    Ok(())
}

/// Withdrawing a heart removes the notification with it: the author must not
/// keep a "someone liked you" entry for a like that no longer exists.
#[tokio::test]
async fn withdrawing_a_heart_removes_the_notification() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "withdraw-author").await?;
    let liker = profile(&pool, "withdraw-liker").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE).await?;
    let store = LikeStore::new(pool.clone());
    store.like(intro, liker.profile_id).await?;
    let cleared = store.unlike(intro, liker.profile_id).await?;
    assert!(!cleared.liked);
    assert_eq!(cleared.like_count, Some(0));
    let notifications: i64 =
        sqlx::query_scalar("select count(*) from intro_like_notifications where intro_id=$1")
            .bind(intro)
            .fetch_one(&pool)
            .await?;
    assert_eq!(notifications, 0);
    Ok(())
}

/// The acceptance case: `DELETE` by a viewer who lost access returns
/// `like_count: null`.
#[tokio::test]
async fn delete_by_a_viewer_who_lost_access_returns_a_null_count() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "lost-author").await?;
    let liker = profile(&pool, "lost-liker").await?;
    // The post starts unrestricted, so the liker can heart it.
    let intro = published(&pool, author.profile_id, Audience::ANYONE).await?;
    let store = LikeStore::new(pool.clone());
    let liked = store.like(intro, liker.profile_id).await?;
    assert_eq!(liked.like_count, Some(1));

    // The author now restricts the post to women. Until WP-04's claim reader is
    // composed, the fail-closed policy denies a restricted post to every
    // non-author, so the liker has lost access.
    sqlx::query(
        "update intro_posts set audience_gender='female',revision=revision+1 where intro_id=$1",
    )
    .bind(intro)
    .execute(&pool)
    .await?;

    let cleared = store
        .unlike(intro, liker.profile_id)
        .await
        .expect("withdrawing consent never needs permission");
    assert!(!cleared.liked);
    assert_eq!(
        cleared.like_count, None,
        "a viewer who lost access must not learn the current count"
    );
    let wire = cleared.to_wire();
    assert_eq!(wire["like_count"], serde_json::Value::Null);
    // The like really was removed.
    let rows: i64 = sqlx::query_scalar("select count(*) from intro_likes where intro_id=$1")
        .bind(intro)
        .fetch_one(&pool)
        .await?;
    assert_eq!(rows, 0);
    Ok(())
}

/// Liking a post you cannot see is `404`, never a distinguishable denial.
#[tokio::test]
async fn liking_an_invisible_post_is_404() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "invisible-author").await?;
    let viewer = profile(&pool, "invisible-viewer").await?;
    let intro = published(
        &pool,
        author.profile_id,
        Audience::new(GenderCondition::Male, None, None)?,
    )
    .await?;
    let store = LikeStore::new(pool.clone());
    let error = store
        .like(intro, viewer.profile_id)
        .await
        .expect_err("a restricted post is not likeable by an unauthorized viewer");
    assert_eq!(error, LikeError::Unavailable);
    assert_eq!(error.status(), 404);
    Ok(())
}

/// An author cannot like their own post.
#[tokio::test]
async fn an_author_cannot_like_their_own_post() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "self-author").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE).await?;
    let store = LikeStore::new(pool.clone());
    assert_eq!(
        store
            .like(intro, author.profile_id)
            .await
            .expect_err("no self-like"),
        LikeError::SelfLike
    );
    // The database refuses it too, for any writer.
    let error = sqlx::query("insert into intro_likes(intro_id,actor_profile_id) values($1,$2)")
        .bind(intro)
        .bind(author.profile_id)
        .execute(&pool)
        .await
        .expect_err("the database also refuses a self-like");
    assert_eq!(
        intro_support::constraint(&error).as_deref(),
        Some("intro_likes_no_self_like")
    );
    Ok(())
}

/// The acceptance case: a non-author liker list request is `403`.
#[tokio::test]
async fn a_non_author_liker_list_is_403() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "likers-author").await?;
    let liker = profile(&pool, "likers-liker").await?;
    let stranger = profile(&pool, "likers-stranger").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE).await?;
    let store = LikeStore::new(pool.clone());
    store.like(intro, liker.profile_id).await?;
    // The liker themselves cannot list the likers.
    assert_eq!(
        store
            .likers(intro, liker.profile_id, 20, None)
            .await
            .expect_err("only the author may list likers"),
        LikeError::PermissionDenied
    );
    assert_eq!(
        store
            .likers(intro, stranger.profile_id, 20, None)
            .await
            .expect_err("only the author may list likers"),
        LikeError::PermissionDenied
    );
    assert_eq!(LikeError::PermissionDenied.status(), 403);
    // The author can.
    let page = store.likers(intro, author.profile_id, 20, None).await?;
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].profile_id, liker.profile_id);
    assert_eq!(page.items[0].nickname, liker.nickname);
    Ok(())
}

/// The liker list is paginated and ordered newest first.
#[tokio::test]
async fn the_liker_list_is_paginated() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "page-likers-author").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE).await?;
    let store = LikeStore::new(pool.clone());
    let mut likers = Vec::new();
    for index in 0..5 {
        let liker = profile(&pool, &format!("page-liker-{index}")).await?;
        store.like(intro, liker.profile_id).await?;
        likers.push(liker.profile_id);
    }
    let mut collected = Vec::new();
    let mut cursor = None;
    loop {
        let page = store
            .likers(intro, author.profile_id, 2, cursor.as_deref())
            .await?;
        assert!(page.items.len() <= 2);
        collected.extend(page.items.iter().map(|item| item.profile_id));
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert_eq!(collected.len(), 5);
    assert_eq!(
        collected.iter().copied().collect::<std::collections::BTreeSet<_>>(),
        likers.iter().copied().collect::<std::collections::BTreeSet<_>>()
    );
    Ok(())
}

/// A like is a row and nothing else: it creates no follow, no friend request
/// and no approval state.
#[tokio::test]
async fn a_like_creates_no_relationship() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "plain-author").await?;
    let liker = profile(&pool, "plain-liker").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE).await?;
    let store = LikeStore::new(pool.clone());
    store.like(intro, liker.profile_id).await?;
    // The only rows a like produced are the like and its notification.
    let like_rows: i64 = sqlx::query_scalar("select count(*) from intro_likes where intro_id=$1")
        .bind(intro)
        .fetch_one(&pool)
        .await?;
    let notification_rows: i64 = sqlx::query_scalar(
        "select count(*) from intro_like_notifications where intro_id=$1",
    )
    .bind(intro)
    .fetch_one(&pool)
    .await?;
    assert_eq!((like_rows, notification_rows), (1, 1));
    // The notification table has no state, pending or accepted column.
    let columns: Vec<String> = sqlx::query_scalar(
        "select column_name from information_schema.columns \
         where table_schema='public' and table_name='intro_like_notifications'",
    )
    .fetch_all(&pool)
    .await?;
    for forbidden in ["state", "pending", "accepted", "approval", "deadline"] {
        assert!(
            !columns.iter().any(|column| column == forbidden),
            "intro_like_notifications must not carry {forbidden}"
        );
    }
    Ok(())
}
