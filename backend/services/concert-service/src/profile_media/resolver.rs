//! T-307: resolving what a media asset is attached to, and whether the viewer
//! may see it.
//!
//! The decision itself is pure and lives in
//! [`crate::profile_media::authorization::can_view_media`]. This module is the
//! **resolver**: it reads the attachment rows and produces the
//! [`MediaVisibility`] that decision needs.
//!
//! Two rules shape the queries:
//!
//! * **Media is authorized through the post it is attached to, never through file
//!   ownership alone.** An asset with no live attachment is readable only by its
//!   owner. An asset attached to an intro post is readable only by a viewer who
//!   satisfies that post's audience, re-evaluated on every request.
//! * **The purpose must agree with the surface.** An `INTRO_POST` asset found
//!   through a profile post, or the reverse, is refused even for the owner: the
//!   association is not a permission.
//!
//! The intro audience re-evaluation is delegated to the caller's
//! [`VisibilityPolicy`], so the media read applies the *same* decision as the
//! post read and cannot drift from it.
use chrono::{DateTime, Utc};
use concert_application::media::Purpose;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::authorization::{MediaVisibility, ProfileMediaRead};
use crate::intro_posts::visibility::{PostFacts, ViewerFacts, VisibilityPolicy};

/// The stored media facts the decision needs.
pub struct StoredMedia {
    pub owner_profile_id: Uuid,
    pub purpose: Purpose,
    pub state: String,
}

/// Read the stored row for one asset.
pub async fn stored(pool: &PgPool, media_id: Uuid) -> Result<Option<StoredMedia>, sqlx::Error> {
    let row = sqlx::query("select owner_profile_id, purpose, state from profile_media where media_id=$1")
        .bind(media_id)
        .fetch_optional(pool)
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    Ok(Some(StoredMedia {
        owner_profile_id: row.try_get("owner_profile_id")?,
        purpose: Purpose::parse(&row.try_get::<String, _>("purpose")?)
            .map_err(|_| sqlx::Error::RowNotFound)?,
        state: row.try_get("state")?,
    }))
}

/// Resolve what the asset is attached to, from the viewer's point of view.
///
/// The order is deliberate: an intro attachment is checked first because it is
/// the surface with an audience, then a profile post, then the profile image and
/// the room poster. The first live attachment decides, and an asset attached to
/// several posts is visible if **any** of them is visible to the viewer.
pub async fn resolve(
    pool: &PgPool,
    visibility: &VisibilityPolicy,
    media_id: Uuid,
    stored: &StoredMedia,
    viewer: Uuid,
    now: DateTime<Utc>,
) -> Result<MediaVisibility, sqlx::Error> {
    // An asset that is not READY is absent, whatever it is attached to.
    if stored.state != "READY" {
        return Ok(MediaVisibility::Absent);
    }
    // Intro attachments: re-evaluate the post's audience on every request.
    let intro_attachments = sqlx::query(
        "select p.intro_id, p.author_profile_id, p.state, p.expires_at \
         from intro_attachments a join intro_posts p on p.intro_id = a.intro_id \
         where a.media_id = $1",
    )
    .bind(media_id)
    .fetch_all(pool)
    .await?;
    let mut saw_restricted_intro = false;
    for row in &intro_attachments {
        let intro_id: Uuid = row.try_get("intro_id")?;
        let author: Uuid = row.try_get("author_profile_id")?;
        let state: String = row.try_get("state")?;
        let expires_at: Option<DateTime<Utc>> = row.try_get("expires_at")?;
        let facts = PostFacts {
            intro_id,
            author_profile_id: author,
            published: state == "PUBLISHED",
            // The predicate re-reads the audience itself; this flag is only the
            // cheap pre-check the port documents.
            restricted: true,
            expired: expires_at.is_some_and(|value| value <= now),
        };
        let viewer_facts = ViewerFacts {
            profile_id: viewer,
            claims_known: false,
        };
        if visibility.can_view(&facts, &viewer_facts, now).await {
            return Ok(MediaVisibility::VisibleIntro);
        }
        saw_restricted_intro = true;
    }
    if saw_restricted_intro {
        return Ok(MediaVisibility::RestrictedIntro);
    }

    // Profile post attachments.
    let post_attachments = sqlx::query(
        "select p.post_id, p.author_profile_id, p.visibility, p.state, p.expires_at, p.deleted_at \
         from profile_post_attachments a join profile_posts p on p.post_id = a.post_id \
         where a.media_id = $1",
    )
    .bind(media_id)
    .fetch_all(pool)
    .await?;
    let mut saw_private_post = false;
    for row in &post_attachments {
        let author: Uuid = row.try_get("author_profile_id")?;
        let post_visibility: String = row.try_get("visibility")?;
        let state: String = row.try_get("state")?;
        let expires_at: Option<DateTime<Utc>> = row.try_get("expires_at")?;
        let deleted_at: Option<DateTime<Utc>> = row.try_get("deleted_at")?;
        let live = state == "PUBLISHED"
            && deleted_at.is_none()
            && expires_at.is_none_or(|value| value > now);
        if !live {
            continue;
        }
        if post_visibility == "PUBLIC" {
            return Ok(MediaVisibility::PublicPost);
        }
        // A private post is readable by its author, which the pure decision
        // handles through the owner comparison.
        saw_private_post = true;
        let _ = author;
    }
    if saw_private_post {
        return Ok(MediaVisibility::PrivatePost);
    }

    // A profile image, when the asset is the profile's current image.
    //
    // `profile_identities` lives in the profile service's database, so this reads
    // the concert service's own `discovery_profiles` mirror instead. A cross-
    // database join is forbidden, and a mirror that has not been written yet
    // simply means the asset is not yet a profile image.
    let profile_visibility: Option<String> = sqlx::query_scalar(
        "select visibility from discovery_profiles \
         where profile_image_media_id = $1 limit 1",
    )
    .bind(media_id)
    .fetch_optional(pool)
    .await?;
    if let Some(profile_visibility) = profile_visibility {
        return Ok(if profile_visibility == "PUBLIC" {
            MediaVisibility::PublicProfileImage
        } else {
            MediaVisibility::PrivateProfileImage
        });
    }

    // A room poster. A poster is a placement inside the room's layout, not a
    // column, so the asset is matched against the layout's poster list.
    let room_visibility: Option<String> = sqlx::query_scalar(
        "select visibility from profile_rooms \
         where layout->'posters' @> jsonb_build_array(jsonb_build_object('poster_media_id', $1::text)) \
            or layout->'posters' @> jsonb_build_array(jsonb_build_object('poster_media_id', $1::uuid)) \
         limit 1",
    )
    .bind(media_id)
    .fetch_optional(pool)
    .await?;
    if let Some(room_visibility) = room_visibility {
        return Ok(if room_visibility == "PUBLIC" {
            MediaVisibility::PublicRoomPoster
        } else {
            MediaVisibility::PrivateRoomPoster
        });
    }

    // Attached to nothing: a fresh upload that was never published.
    Ok(MediaVisibility::Unattached)
}

/// The full read decision for one asset.
///
/// Returns the [`ProfileMediaRead`] the pure decision needs, or `None` when the
/// asset does not exist at all.
pub async fn read_facts(
    pool: &PgPool,
    visibility: &VisibilityPolicy,
    media_id: Uuid,
    viewer: Uuid,
    now: DateTime<Utc>,
) -> Result<Option<ProfileMediaRead>, sqlx::Error> {
    let Some(stored) = stored(pool, media_id).await? else {
        return Ok(None);
    };
    let resolved = resolve(pool, visibility, media_id, &stored, viewer, now).await?;
    Ok(Some(ProfileMediaRead {
        owner_profile_id: stored.owner_profile_id,
        purpose: stored.purpose,
        visibility: resolved,
    }))
}
