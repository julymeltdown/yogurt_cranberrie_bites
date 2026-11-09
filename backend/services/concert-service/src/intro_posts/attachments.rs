//! T-403: attachment validation against `profile_media`.
//!
//! The rule itself is pure (`concert_application::intro_posts::attachments`);
//! this module is the store half: it reads the candidate rows and hands them to
//! the rule. It deliberately exposes the **count** and the **ids** of a post's
//! attachments, and nothing about the media rows themselves, so a caller cannot
//! use it to learn anything about an asset they were not allowed to attach.
pub use concert_application::intro_posts::attachments::MediaPurpose;
use concert_application::intro_posts::attachments::{
    plan, AttachmentCandidate, AttachmentError, MAX_ATTACHMENTS,
};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Read the candidate rows for a requested set.
///
/// An id that is not in the result is either absent or unreadable; the caller
/// cannot tell which, and neither can the pure rule. The row decoding itself is
/// shared with the profile-post command, so the two surfaces cannot disagree
/// about what a media row means.
pub async fn candidates(
    pool: &PgPool,
    requested: &[Uuid],
) -> Result<Vec<AttachmentCandidate>, sqlx::Error> {
    let mut connection = pool.acquire().await?;
    crate::profile_media::candidates_in(&mut connection, requested).await
}

/// Validate a requested set for `author`.
pub async fn validate(
    pool: &PgPool,
    author: Uuid,
    requested: &[Uuid],
) -> Result<Vec<Uuid>, AttachmentError> {
    let found = candidates(pool, requested)
        .await
        .map_err(|_| AttachmentError::Unavailable)?;
    plan(requested, author, &found).map(|plan| plan.media_ids)
}

/// The attachment ids of a post, in display order.
pub async fn list(pool: &PgPool, intro_id: Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "select media_id from intro_attachments where intro_id=$1 order by position",
    )
    .bind(intro_id)
    .fetch_all(pool)
    .await
}

/// The `feed.Media` projection of a post's attachments.
///
/// Only `READY` assets are returned, so a post whose asset was deleted shows
/// fewer images rather than a broken one.
pub async fn media(pool: &PgPool, intro_id: Uuid) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    let rows = sqlx::query(
        "select a.media_id, m.content_type, m.width, m.height, m.revision \
         from intro_attachments a join profile_media m on m.media_id = a.media_id \
         where a.intro_id=$1 and m.state='READY' order by a.position",
    )
    .bind(intro_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|row| {
            let media_id: Uuid = row.try_get("media_id").unwrap_or_else(|_| Uuid::nil());
            serde_json::json!({
                "media_id": media_id,
                "content_type": row.try_get::<String, _>("content_type").unwrap_or_default(),
                "width": row.try_get::<Option<i32>, _>("width").ok().flatten().unwrap_or(0),
                "height": row.try_get::<Option<i32>, _>("height").ok().flatten().unwrap_or(0),
                "revision": row.try_get::<i64, _>("revision").unwrap_or(1),
                "read_path": format!("/api/media/{media_id}/content"),
            })
        })
        .collect())
}

/// The frozen bound, re-exported for callers that do not depend on the crate.
pub const LIMIT: usize = MAX_ATTACHMENTS;
