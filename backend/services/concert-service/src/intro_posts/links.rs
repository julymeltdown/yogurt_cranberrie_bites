//! T-308: shared link resolution.
//!
//! A shared Introduction link is resolved through the **same** visibility
//! predicate as every other surface. The contract's requirement is narrow and
//! absolute: a resolution the viewer may not see must be indistinguishable from a
//! link that does not exist, and no restricted content may appear in the preview.
//!
//! ## One answer, one shape
//!
//! [`resolve`] returns a [`LinkResolution`]. An unavailable resolution is the
//! same value whether the post is missing, unpublished, expired, blocked, or
//! simply outside the viewer's audience. The type has no field that could
//! distinguish them, so a caller cannot accidentally leak the difference through
//! a status code, an error variant or a null-vs-absent distinction.
//!
//! ## No preview bytes
//!
//! The available resolution carries the post's body and its attachment **ids**,
//! never the bytes and never a thumbnail. Fetching a byte still goes through
//! [`crate::profile_media`], which authorizes through the attached post again, so
//! a link preview cannot become a second, weaker path to a photograph.
//!
//! ## Anonymous resolution
//!
//! An anonymous resolver is a viewer with no profile. It is treated as a viewer
//! that satisfies no restriction, so an unrestricted post resolves and a
//! restricted one does not — exactly as the decision table says for a viewer with
//! no claim.
use chrono::{DateTime, Utc};
use concert_application::identity_claims::ProfileId;
use concert_application::intro_visibility::BlockState;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::repository::{get_intro_post, IntroPageError};
use super::visibility_sql::VisibilityScope;

/// What a shared link resolves to.
///
/// There is deliberately no `NotFound` variant separate from `Unavailable`: the
/// two must be the same value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkResolution {
    /// The viewer may see the post. The preview carries the body and attachment
    /// ids, and never any bytes.
    Available(LinkPreview),
    /// The link does not resolve for this viewer. This is the answer for a
    /// missing post, an unpublished post, an expired post, a blocked author and
    /// an audience the viewer does not satisfy, alike.
    Unavailable,
}

/// The metadata a shared link may show.
///
/// No author profile, no author nickname, no audience, no expiry and no bytes:
/// a preview of a post the viewer can see is still not a reason to disclose
/// anything the post itself does not show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkPreview {
    pub intro_id: Uuid,
    pub body: String,
    /// The attachment ids, in display order. Never the bytes.
    pub attachment_media_ids: Vec<Uuid>,
    pub published_at: DateTime<Utc>,
}

/// The viewer of a shared link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkViewer {
    /// No session. Treated as a viewer that satisfies no restriction.
    Anonymous,
    /// An authenticated profile.
    Profile(ProfileId),
}

impl LinkViewer {
    /// The profile id this viewer is evaluated as.
    ///
    /// An anonymous viewer is evaluated as the nil profile, which is never the
    /// author and therefore never takes the author's own path.
    pub const fn profile_id(self) -> Uuid {
        match self {
            Self::Anonymous => Uuid::nil(),
            Self::Profile(profile) => profile.uuid(),
        }
    }
}

/// Resolve a shared link.
///
/// `scope_for` builds the viewer's scope. It is a closure rather than a value
/// because the claim read is per-request and must happen at the resolution
/// instant, not at some earlier point.
pub async fn resolve<F>(
    pool: &PgPool,
    intro_id: Uuid,
    viewer: LinkViewer,
    now: DateTime<Utc>,
    scope_for: F,
) -> Result<LinkResolution, IntroPageError>
where
    F: FnOnce(Uuid, DateTime<Utc>) -> VisibilityScope,
{
    if intro_id.is_nil() {
        return Ok(LinkResolution::Unavailable);
    }
    let scope = scope_for(viewer.profile_id(), now);
    // The same detail query every other surface uses. Its predicate is the
    // decision table, so a link cannot be a weaker path than the post page.
    let detail = match get_intro_post(pool, &scope, intro_id).await {
        Ok(detail) => detail,
        // Every refusal is the same answer.
        Err(IntroPageError::Unavailable) => return Ok(LinkResolution::Unavailable),
        Err(other) => return Err(other),
    };
    // The attachment ids come from the same query that resolved the post, so a
    // refusal never touches the attachment table and a preview never needs a
    // second read that could disagree.
    Ok(LinkResolution::Available(LinkPreview {
        intro_id: detail.post.intro_id,
        body: detail.post.body,
        attachment_media_ids: detail.attachment_media_ids,
        published_at: detail
            .post
            .published_at
            .ok_or(IntroPageError::Unavailable)?,
    }))
}

/// The wire form of a resolution.
///
/// Both cases are objects with a `status` field, so a client cannot tell them
/// apart by shape. An unavailable resolution carries no other field at all.
pub fn to_wire(resolution: &LinkResolution) -> serde_json::Value {
    match resolution {
        LinkResolution::Available(preview) => serde_json::json!({
            "status": "AVAILABLE",
            "intro_id": preview.intro_id,
            "body": preview.body,
            "attachment_media_ids": preview.attachment_media_ids,
            "published_at": preview
                .published_at
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        }),
        LinkResolution::Unavailable => serde_json::json!({ "status": "UNAVAILABLE" }),
    }
}

/// A scope whose claims are unavailable, for an anonymous resolver.
pub fn anonymous_scope(viewer: Uuid, now: DateTime<Utc>) -> VisibilityScope {
    VisibilityScope::new(
        viewer,
        BlockState::None,
        concert_application::intro_visibility::AuthorStanding::Active,
        super::visibility_sql::ViewerAttributes::Unavailable,
        now,
    )
}

/// The attachment ids of a post, for a caller that has already resolved it.
pub async fn attachment_ids(pool: &PgPool, intro_id: Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar("select media_id from intro_attachments where intro_id = $1 order by position")
        .bind(intro_id)
        .fetch_all(pool)
        .await
}

/// Whether a row is the nil profile, which is what an anonymous viewer resolves
/// to. Kept as a named function so the intent is explicit at the call site.
pub const fn is_anonymous(profile: Uuid) -> bool {
    profile.is_nil()
}

/// Read a post's author, for a caller that needs it after a successful
/// resolution. Never called on a refused resolution.
pub async fn author_of(pool: &PgPool, intro_id: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
    let row = sqlx::query("select author_profile_id from intro_posts where intro_id = $1")
        .bind(intro_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| row.get("author_profile_id")))
}
