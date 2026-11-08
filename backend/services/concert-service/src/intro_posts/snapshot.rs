//! T-402: the favorites snapshot column.
//!
//! The rule is a property of *where the value lives*, so this module is small on
//! purpose: it reads the stored column and never re-derives it from the profile.
//! `commands::IntroPostStore::save_draft` writes the snapshot **only on
//! creation**, so an update cannot change it even if the caller sends a new one.
//!
//! Discovery uses the profile favorites, never this snapshot, so a stale
//! snapshot cannot make a post discoverable under picks the author no longer
//! has. That is why this module has no query that joins the profile.
use concert_application::intro_posts::snapshot::Snapshot;
use sqlx::PgPool;
use uuid::Uuid;

/// Read the stored snapshot of a post.
///
/// Returns `None` when the post does not exist. A stored value that violates a
/// rule is an error rather than a repaired value: a corrupted snapshot must not
/// silently become a different post.
pub async fn read(
    pool: &PgPool,
    intro_id: Uuid,
) -> Result<Option<Snapshot>, sqlx::Error> {
    let value: Option<serde_json::Value> =
        sqlx::query_scalar("select favorites_snapshot from intro_posts where intro_id=$1")
            .bind(intro_id)
            .fetch_optional(pool)
            .await?;
    Ok(value.and_then(|value| Snapshot::from_stored(&value).ok()))
}
