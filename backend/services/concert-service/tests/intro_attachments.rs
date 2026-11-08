//! WP-05 T-403: attachment rules against a real cluster.
//!
//! The acceptance cases the task document names:
//!
//! * a foreign media id is rejected without revealing whether it exists;
//! * a fifth attachment is rejected.
//!
//! The first is proven by *comparing the two responses*: a real asset owned by
//! somebody else and an id that does not exist must produce the same error
//! value, so no caller can tell them apart.
mod intro_support;

use concert_application::intro_posts::{
    audience::Audience,
    content::IntroContent,
    snapshot::Snapshot,
};
use concert_service::intro_posts::commands::{IntroCommandError, IntroPostStore, SaveDraft};
use intro_support::{media, pool, profile};
use uuid::Uuid;

fn draft(attachments: Vec<Uuid>) -> SaveDraft {
    SaveDraft {
        expected_revision: 0,
        content: IntroContent {
            body: "With photographs.".to_owned(),
            currently_seeking: None,
            interaction_style: None,
        },
        favorites_snapshot: Snapshot::empty(),
        attachment_media_ids: attachments,
        audience: Audience::ANYONE,
        expires_at: None,
    }
}

/// The acceptance case: a foreign media id is rejected without revealing
/// whether it exists.
#[tokio::test]
async fn a_foreign_media_id_is_indistinguishable_from_a_missing_one() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "attach-author").await?;
    let stranger = profile(&pool, "attach-stranger").await?;
    let store = IntroPostStore::new(pool.clone());
    // A real, READY, correctly-purposed asset owned by somebody else.
    let foreign = media(&pool, stranger.profile_id, "INTRO_POST").await?;
    // An id that does not exist at all.
    let absent = Uuid::new_v4();

    let foreign_error = store
        .save_draft(author.profile_id, Uuid::new_v4(), &draft(vec![foreign]))
        .await
        .expect_err("a foreign asset must not attach");
    let absent_error = store
        .save_draft(author.profile_id, Uuid::new_v4(), &draft(vec![absent]))
        .await
        .expect_err("an absent asset must not attach");
    // The two answers are the same value: nothing distinguishes them.
    assert_eq!(foreign_error, IntroCommandError::InvalidAttachment);
    assert_eq!(absent_error, IntroCommandError::InvalidAttachment);
    assert_eq!(foreign_error, absent_error);
    // And the wire answer is the same too.
    assert_eq!(foreign_error.status(), absent_error.status());
    assert_eq!(foreign_error.reason(), absent_error.reason());
    Ok(())
}

/// The acceptance case: a fifth attachment is rejected.
#[tokio::test]
async fn a_fifth_attachment_is_rejected() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "attach-five").await?;
    let store = IntroPostStore::new(pool.clone());
    let mut assets = Vec::new();
    for _ in 0..5 {
        assets.push(media(&pool, author.profile_id, "INTRO_POST").await?);
    }
    // Four is fine.
    let saved = store
        .save_draft(author.profile_id, Uuid::new_v4(), &draft(assets[..4].to_vec()))
        .await
        .expect("four attachments are allowed");
    assert_eq!(saved.attachment_media_ids.len(), 4);
    // Five is not.
    let error = store
        .save_draft(author.profile_id, Uuid::new_v4(), &draft(assets.clone()))
        .await
        .expect_err("a fifth attachment must be refused");
    assert_eq!(error, IntroCommandError::InvalidAttachment);
    Ok(())
}

/// A wrong-purpose asset is refused, with the same undifferentiated answer.
#[tokio::test]
async fn a_wrong_purpose_asset_is_refused() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "attach-purpose").await?;
    let store = IntroPostStore::new(pool.clone());
    for purpose in ["PROFILE_POST", "ROOM_POSTER", "PROFILE_IMAGE"] {
        let asset = media(&pool, author.profile_id, purpose).await?;
        let error = store
            .save_draft(author.profile_id, Uuid::new_v4(), &draft(vec![asset]))
            .await
            .expect_err("a wrong-purpose asset must not attach");
        assert_eq!(error, IntroCommandError::InvalidAttachment, "{purpose}");
    }
    Ok(())
}

/// An asset that is not `READY` is refused.
#[tokio::test]
async fn an_unready_asset_is_refused() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "attach-pending").await?;
    let store = IntroPostStore::new(pool.clone());
    let pending = Uuid::new_v4();
    sqlx::query(
        "insert into profile_media(media_id,owner_profile_id,purpose,content_type,state,\
         declared_bytes,input_sha256) \
         values($1,$2,'INTRO_POST','image/png','PENDING',4,decode(repeat('cd',32),'hex'))",
    )
    .bind(pending)
    .bind(author.profile_id)
    .execute(&pool)
    .await?;
    let error = store
        .save_draft(author.profile_id, Uuid::new_v4(), &draft(vec![pending]))
        .await
        .expect_err("a pending asset must not attach");
    assert_eq!(error, IntroCommandError::InvalidAttachment);
    Ok(())
}

/// Attachment order is the author's order, and it survives a re-read.
#[tokio::test]
async fn attachment_order_is_preserved() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "attach-order").await?;
    let store = IntroPostStore::new(pool.clone());
    let first = media(&pool, author.profile_id, "INTRO_POST").await?;
    let second = media(&pool, author.profile_id, "INTRO_POST").await?;
    let intro = Uuid::new_v4();
    let saved = store
        .save_draft(author.profile_id, intro, &draft(vec![second, first]))
        .await?;
    assert_eq!(saved.attachment_media_ids, vec![second, first]);
    // Replacing the set replaces the order.
    let mut update = draft(vec![first]);
    update.expected_revision = saved.revision;
    let updated = store.save_draft(author.profile_id, intro, &update).await?;
    assert_eq!(updated.attachment_media_ids, vec![first]);
    Ok(())
}

/// A duplicate id is refused rather than silently deduplicated, so the caller
/// learns its request was wrong.
#[tokio::test]
async fn a_duplicate_attachment_is_refused() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "attach-dup").await?;
    let store = IntroPostStore::new(pool.clone());
    let asset = media(&pool, author.profile_id, "INTRO_POST").await?;
    let error = store
        .save_draft(author.profile_id, Uuid::new_v4(), &draft(vec![asset, asset]))
        .await
        .expect_err("a duplicate must be refused");
    assert_eq!(error, IntroCommandError::InvalidAttachment);
    Ok(())
}

/// The database refuses a fifth attachment row even if a writer bypasses the
/// command, so the cap holds for every writer.
#[tokio::test]
async fn the_database_refuses_a_fifth_attachment_row() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "attach-db").await?;
    let intro = Uuid::new_v4();
    sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,state) values($1,$2,'body','DRAFT')",
    )
    .bind(intro)
    .bind(author.profile_id)
    .execute(&pool)
    .await?;
    for position in 0..4i16 {
        let asset = media(&pool, author.profile_id, "INTRO_POST").await?;
        sqlx::query("insert into intro_attachments(intro_id,media_id,position) values($1,$2,$3)")
            .bind(intro)
            .bind(asset)
            .bind(position)
            .execute(&pool)
            .await?;
    }
    let fifth = media(&pool, author.profile_id, "INTRO_POST").await?;
    let error = sqlx::query(
        "insert into intro_attachments(intro_id,media_id,position) values($1,$2,4)",
    )
    .bind(intro)
    .bind(fifth)
    .execute(&pool)
    .await
    .expect_err("position 4 is out of range");
    assert_eq!(
        intro_support::constraint(&error).as_deref(),
        Some("intro_attachments_position")
    );
    Ok(())
}
