//! WP-05 T-402: the favorites snapshot import.
//!
//! The acceptance case the task document names is the one that matters:
//! **changing favorites after publishing leaves the post snapshot unchanged.**
//! This test edits the author's profile favorites for real, between the publish
//! and the re-read, and requires the snapshot to be byte-identical.
//!
//! The second case is the other half of the same rule: an *update* to the post
//! cannot change the snapshot either, even if the caller sends a different one.
//! A snapshot is taken once, at creation.
mod intro_support;

use concert_application::intro_posts::{
    audience::Audience,
    content::IntroContent,
    snapshot::{Snapshot, SnapshotItem},
};
use concert_service::intro_posts::commands::{IntroPostStore, SaveDraft};
use intro_support::{pool, profile};
use uuid::Uuid;

fn item(label: &str) -> SnapshotItem {
    SnapshotItem {
        catalog_item_id: Uuid::new_v4(),
        label: label.to_owned(),
        category: "MUSIC",
    }
}

fn draft(snapshot: Snapshot) -> SaveDraft {
    SaveDraft {
        expected_revision: 0,
        content: IntroContent {
            body: "My introduction.".to_owned(),
            currently_seeking: None,
            interaction_style: None,
        },
        favorites_snapshot: snapshot,
        attachment_media_ids: Vec::new(),
        audience: Audience::ANYONE,
        expires_at: None,
    }
}

/// The acceptance case: changing favorites after publishing leaves the post
/// snapshot unchanged.
#[tokio::test]
async fn changing_favorites_after_publishing_leaves_the_snapshot_unchanged() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "snapshot").await?;
    let store = IntroPostStore::new(pool.clone());
    let intro = Uuid::new_v4();
    let original = Snapshot::from_favorites(vec![item("My Chemical Romance")])?;
    let saved = store
        .save_draft(author.profile_id, intro, &draft(original.clone()))
        .await?;
    store.publish(author.profile_id, intro, saved.revision).await?;

    // The author now changes their profile favorites completely.
    sqlx::query(
        "insert into discovery_favorite_items(profile_id,category,catalog_item_id,label,position) \
         values($1,'music',$2,'Paramore',0)",
    )
    .bind(author.profile_id)
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await?;

    // The published post's snapshot is exactly what it was.
    let stored: serde_json::Value =
        sqlx::query_scalar("select favorites_snapshot from intro_posts where intro_id=$1")
            .bind(intro)
            .fetch_one(&pool)
            .await?;
    assert_eq!(stored, original.to_stored());
    assert_eq!(Snapshot::from_stored(&stored)?, original);
    Ok(())
}

/// An update cannot change the snapshot, even when the caller sends a new one.
#[tokio::test]
async fn an_update_cannot_change_the_snapshot() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "snapshot-update").await?;
    let store = IntroPostStore::new(pool.clone());
    let intro = Uuid::new_v4();
    let original = Snapshot::from_favorites(vec![item("Original")])?;
    let saved = store
        .save_draft(author.profile_id, intro, &draft(original.clone()))
        .await?;

    let mut update = draft(Snapshot::from_favorites(vec![item("Replacement")])?);
    update.expected_revision = saved.revision;
    update.content.body = "Edited body.".to_owned();
    let updated = store.save_draft(author.profile_id, intro, &update).await?;

    // The body changed; the snapshot did not.
    assert_eq!(updated.body, "Edited body.");
    assert_eq!(updated.favorites_snapshot, original.to_stored());
    Ok(())
}

/// The snapshot carries catalog item ids and labels only.
#[tokio::test]
async fn the_snapshot_contains_ids_and_labels_only() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "snapshot-shape").await?;
    let store = IntroPostStore::new(pool.clone());
    let intro = Uuid::new_v4();
    let snapshot = Snapshot::from_favorites(vec![item("NANA"), item("Omori")])?;
    let saved = store
        .save_draft(author.profile_id, intro, &draft(snapshot))
        .await?;
    let entries = saved
        .favorites_snapshot
        .as_array()
        .expect("the snapshot is an array");
    assert_eq!(entries.len(), 2);
    for entry in entries {
        let mut keys: Vec<&str> = entry
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort();
        // No weight, intensity, importance or preference field exists.
        assert_eq!(keys, ["catalog_item_id", "category", "external_url", "label"]);
    }
    Ok(())
}

/// A profile with no picks still publishes, with an empty snapshot.
#[tokio::test]
async fn an_empty_snapshot_publishes() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "snapshot-empty").await?;
    let store = IntroPostStore::new(pool.clone());
    let intro = Uuid::new_v4();
    let saved = store
        .save_draft(author.profile_id, intro, &draft(Snapshot::empty()))
        .await?;
    let published = store.publish(author.profile_id, intro, saved.revision).await?;
    assert_eq!(published.state, "PUBLISHED");
    assert_eq!(published.favorites_snapshot, serde_json::json!([]));
    Ok(())
}

/// Discovery uses the profile favorites, never the snapshot: a post whose
/// snapshot is stale is still found by the *current* picks.
#[tokio::test]
async fn discovery_reads_the_profile_not_the_snapshot() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "snapshot-discovery").await?;
    let viewer = profile(&pool, "snapshot-viewer").await?;
    let store = IntroPostStore::new(pool.clone());
    let intro = Uuid::new_v4();
    // The post's snapshot names an item the author no longer has.
    let stale = Snapshot::from_favorites(vec![item("Stale Band")])?;
    let saved = store
        .save_draft(author.profile_id, intro, &draft(stale))
        .await?;
    store.publish(author.profile_id, intro, saved.revision).await?;
    // The author's *current* favorites and the viewer's overlap.
    let shared = Uuid::new_v4();
    for (who, position) in [(author.profile_id, 0i16), (viewer.profile_id, 0i16)] {
        sqlx::query(
            "insert into discovery_favorite_items(profile_id,category,catalog_item_id,label,position) \
             values($1,'music',$2,'Shared Band',$3)",
        )
        .bind(who)
        .bind(shared)
        .bind(position)
        .execute(&pool)
        .await?;
    }
    let page = concert_service::discovery::discover(
        &pool,
        viewer.profile_id,
        &concert_service::discovery::validate(concert_service::discovery::PeopleQuery {
            category: concert_service::discovery::Category::Music,
            cursor: None,
            limit: 100,
        })?,
    )
    .await?;
    let card = page
        .items
        .iter()
        .find(|card| card.profile_id == author.profile_id)
        .expect("the author is discoverable by their current picks");
    assert_eq!(card.shared_items[0].label, "Shared Band");
    Ok(())
}
