//! WP-06 T-502: profile post commands — create and delete.
//!
//! # What this module deliberately does not do
//!
//! **It performs no pixel conversion.** There is no image code in this module at
//! all: it reads `profile_media` rows and writes `profile_posts` rows. The bytes a
//! profile photograph is served from are the sanitized bytes the upload
//! lifecycle produced (a metadata-stripped re-encode of the same pixels at the
//! same dimensions), never a posterized or palette-reduced derivative. Pixel-art
//! conversion exists only for room posters (product-plan.md §2 hard rule 5), and
//! `tests/profile_posts.rs` asserts that this file contains no conversion call
//! and that the stored bytes equal the normalized photograph byte for byte.
//!
//! # Attachment rules
//!
//! A profile post carries at most four media ids. Every id must be:
//!
//! * owned by the author — otherwise the answer is `404 RESOURCE_UNAVAILABLE`,
//!   the same answer an absent id gets, so a caller cannot probe for another
//!   profile's media ids (V-06-c);
//! * `READY`;
//! * purpose `PROFILE_POST` — an `INTRO_POST` asset owned by the author is
//!   **rejected** (V-06-d). This is the rule that stops a photograph published
//!   only to a restricted intro audience from being republished to everybody by
//!   attaching it to a profile post. The database enforces it independently in
//!   `profile_post_attachments_purpose` (migration 0084), so a future caller that
//!   bypassed this module still cannot do it.
//!
//! # Why the answer for a foreign id is not `403`
//!
//! "Not yours" and "does not exist" must be indistinguishable, or the endpoint
//! becomes an existence oracle for other people's media ids. Both are therefore
//! `404 RESOURCE_UNAVAILABLE`.
use concert_application::media::Purpose;
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// The maximum number of attachments on one profile post (`CreateProfilePost`).
pub const MAX_ATTACHMENTS: usize = 4;
/// The maximum description length in Unicode code points (`CreateProfilePost`).
pub const MAX_DESCRIPTION: usize = 500;

/// The closed error set of a profile post command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfilePostError {
    /// The request body is not a well-formed `CreateProfilePost`.
    Invalid,
    /// The author has no live profile.
    NoProfile,
    /// The post, or one of its media ids, is not visible to this caller.
    ///
    /// One variant for "absent" and "not yours" on purpose: the two must not be
    /// distinguishable.
    Unavailable,
    /// The attachment is owned by the author but is not a `PROFILE_POST` asset
    /// (for example it is an `INTRO_POST` asset), or is not `READY`.
    AttachmentNotPublishable,
    /// The revision the caller expected is not the current revision.
    Conflict,
    /// The store is unavailable.
    Unavailable503,
}

impl ProfilePostError {
    pub fn status(self) -> u16 {
        match self {
            Self::Invalid | Self::AttachmentNotPublishable => 400,
            Self::NoProfile | Self::Unavailable => 404,
            Self::Conflict => 409,
            Self::Unavailable503 => 503,
        }
    }
    pub fn reason(self) -> &'static str {
        match self {
            Self::Invalid => "INVALID_INPUT",
            Self::NoProfile => "RESOURCE_UNAVAILABLE",
            Self::Unavailable => "RESOURCE_UNAVAILABLE",
            Self::AttachmentNotPublishable => "INVALID_INPUT",
            Self::Conflict => "VERSION_CONFLICT",
            Self::Unavailable503 => "DEPENDENCY_UNAVAILABLE",
        }
    }
}

impl std::fmt::Display for ProfilePostError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "profile post error: {}", self.reason())
    }
}

impl std::error::Error for ProfilePostError {}

fn store(_: sqlx::Error) -> ProfilePostError {
    ProfilePostError::Unavailable503
}



fn decode(_: sqlx::Error) -> ProfilePostError {
    ProfilePostError::Invalid
}

/// The visibility of a profile post. Closed: there is no "friends only" and no
/// "audience" — a profile post is public or private.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostVisibility {
    Public,
    Private,
}

impl PostVisibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Public => "PUBLIC",
            Self::Private => "PRIVATE",
        }
    }
    pub fn parse(value: &str) -> Result<Self, ProfilePostError> {
        match value {
            "PUBLIC" => Ok(Self::Public),
            "PRIVATE" => Ok(Self::Private),
            _ => Err(ProfilePostError::Invalid),
        }
    }
}

/// A validated `CreateProfilePost`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateProfilePost {
    pub description: String,
    pub media_ids: Vec<Uuid>,
    pub visibility: PostVisibility,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl CreateProfilePost {
    /// Parse the frozen `CreateProfilePost` body.
    ///
    /// The schema is closed (`additionalProperties: false`) and every field is
    /// required, so an unknown field or a missing field is `400`, never a
    /// silently ignored value. The description limit is counted in Unicode code
    /// points, which is what `char_length` counts in PostgreSQL and what the
    /// client counts, so the two agree on an emoji or a combining mark.
    pub fn parse(bytes: &[u8]) -> Result<Self, ProfilePostError> {
        let value: Value = serde_json::from_slice(bytes).map_err(|_| ProfilePostError::Invalid)?;
        let object = value.as_object().ok_or(ProfilePostError::Invalid)?;
        const FIELDS: [&str; 4] = ["description", "media_ids", "visibility", "expires_at"];
        if object.len() != FIELDS.len() || FIELDS.iter().any(|field| !object.contains_key(*field)) {
            return Err(ProfilePostError::Invalid);
        }
        let description = object["description"]
            .as_str()
            .ok_or(ProfilePostError::Invalid)?
            .to_owned();
        if description.chars().count() > MAX_DESCRIPTION || description.contains('\0') {
            return Err(ProfilePostError::Invalid);
        }
        let media_ids = object["media_ids"]
            .as_array()
            .ok_or(ProfilePostError::Invalid)?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .and_then(|text| Uuid::parse_str(text).ok())
                    .filter(|id| !id.is_nil() && id.to_string() == value.as_str().unwrap_or_default())
                    .ok_or(ProfilePostError::Invalid)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if media_ids.len() > MAX_ATTACHMENTS {
            return Err(ProfilePostError::Invalid);
        }
        if media_ids.iter().enumerate().any(|(index, id)| {
            media_ids[..index].contains(id)
        }) {
            return Err(ProfilePostError::Invalid);
        }
        let visibility = PostVisibility::parse(
            object["visibility"]
                .as_str()
                .ok_or(ProfilePostError::Invalid)?,
        )?;
        let expires_at = match &object["expires_at"] {
            Value::Null => None,
            Value::String(text) => Some(
                chrono::DateTime::parse_from_rfc3339(text)
                    .map_err(|_| ProfilePostError::Invalid)?
                    .with_timezone(&chrono::Utc),
            ),
            _ => return Err(ProfilePostError::Invalid),
        };
        Ok(Self {
            description,
            media_ids,
            visibility,
            expires_at,
        })
    }
}

/// The committed projection of a created post.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatedProfilePost {
    pub post_id: Uuid,
    pub author_profile_id: Uuid,
    pub description: String,
    pub visibility: PostVisibility,
    pub revision: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// The media ids in attachment position order.
    pub media_ids: Vec<Uuid>,
}

/// The `common.Deleted` projection of a deleted post.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeletedProfilePost {
    pub post_id: Uuid,
    pub revision: i64,
}

impl DeletedProfilePost {
    pub fn to_wire(&self) -> Value {
        json!({"resource_id": self.post_id, "revision": self.revision, "state": "DELETED"})
    }
}

/// Resolve the author's live profile id from the authenticated account.
///
/// The concert service keeps no profile table: the profile id is the canonical
/// profile identity the profile service owns. The caller passes the already
/// verified profile id in, so this function only refuses a nil one.
fn author(profile_id: Uuid) -> Result<Uuid, ProfilePostError> {
    if profile_id.is_nil() {
        Err(ProfilePostError::NoProfile)
    } else {
        Ok(profile_id)
    }
}

/// Create a profile post.
///
/// The whole command is one transaction: the attachment check, the post insert
/// and the association insert either all commit or none do. A rejected
/// attachment therefore leaves no post behind (V-06-c).
///
/// There is no intro post and no room involved at any point. A profile that has
/// never published an intro post and whose room is in its default state can
/// publish here (V-07-a).
pub async fn create_profile_post(
    pool: &PgPool,
    author_profile_id: Uuid,
    command: &CreateProfilePost,
) -> Result<CreatedProfilePost, ProfilePostError> {
    let author = author(author_profile_id)?;
    if command.description.chars().count() > MAX_DESCRIPTION {
        return Err(ProfilePostError::Invalid);
    }
    if command.media_ids.len() > MAX_ATTACHMENTS {
        return Err(ProfilePostError::Invalid);
    }
    let mut tx = pool.begin().await.map_err(store)?;
    // Lock the author's account row so two concurrent creates cannot interleave
    // the attachment check with the association write.
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("mcr-profile-post-create:{author}"))
        .execute(&mut *tx)
        .await
        .map_err(store)?;
    // Every attachment is checked before anything is written. The row decoding
    // is shared with the intro store, and the decision is the same pure rule the
    // intro surface uses, parameterised by the purpose this surface accepts.
    //
    // The order of the checks is deliberate: ownership first, so a foreign id is
    // `Unavailable` (404) and never reveals whether the asset exists or what its
    // purpose is; the purpose is checked only once the asset is known to be the
    // author's own.
    let found = crate::profile_media::candidates_in(&mut tx, &command.media_ids)
        .await
        .map_err(store)?;
    for media_id in &command.media_ids {
        let Some(candidate) = found.iter().find(|candidate| candidate.media_id == *media_id) else {
            // Absent and unreadable are the same answer, exactly as a foreign id
            // is: the caller cannot tell them apart.
            return Err(ProfilePostError::Unavailable);
        };
        if candidate.owner_profile_id != author {
            return Err(ProfilePostError::Unavailable);
        }
        // An INTRO_POST asset is rejected here, before the insert. The database
        // trigger refuses it too; this check exists so the refusal carries the
        // right status and so no post row is ever created for it.
        if candidate.purpose != Purpose::ProfilePost || !candidate.ready {
            return Err(ProfilePostError::AttachmentNotPublishable);
        }
    }
    let post_id = Uuid::new_v4();
    let created = sqlx::query(
        "insert into public.profile_posts(post_id, author_profile_id, description, visibility, \
         state, expires_at) values($1,$2,$3,$4,'PUBLISHED',$5) \
         returning revision, created_at, expires_at",
    )
    .bind(post_id)
    .bind(author)
    .bind(&command.description)
    .bind(command.visibility.as_str())
    .bind(command.expires_at)
    .fetch_one(&mut *tx)
    .await
    .map_err(store)?;
    for (position, media_id) in command.media_ids.iter().enumerate() {
        sqlx::query(
            "insert into public.profile_post_attachments(post_id, media_id, position) \
             values($1,$2,$3)",
        )
        .bind(post_id)
        .bind(media_id)
        .bind(position as i16)
        .execute(&mut *tx)
        .await
        .map_err(store)?;
    }
    let created = CreatedProfilePost {
        post_id,
        author_profile_id: author,
        description: command.description.clone(),
        visibility: command.visibility,
        revision: created.try_get("revision").map_err(decode)?,
        created_at: created.try_get("created_at").map_err(decode)?,
        expires_at: created.try_get("expires_at").map_err(decode)?,
        media_ids: command.media_ids.clone(),
    };
    tx.commit().await.map_err(store)?;
    Ok(created)
}

/// Delete a profile post.
///
/// Deletion is a state transition, not a row removal: the post becomes
/// `DELETED`, its attachments are removed as *associations only*, and the media
/// files survive because another post may still use them
/// (`domain-contract.md` §4.4).
///
/// Nothing else is touched. The room, the avatar, the intro posts and the
/// favorites are not referenced by this statement at all, and
/// `tests/profile_post_lifecycle.rs` asserts their row counts and contents before
/// and after.
pub async fn delete_profile_post(
    pool: &PgPool,
    author_profile_id: Uuid,
    post_id: Uuid,
    expected_revision: i64,
) -> Result<DeletedProfilePost, ProfilePostError> {
    let author = author(author_profile_id)?;
    if post_id.is_nil() || !(1..=9007199254740991).contains(&expected_revision) {
        return Err(ProfilePostError::Invalid);
    }
    let mut tx = pool.begin().await.map_err(store)?;
    let row = sqlx::query(
        "select author_profile_id, state, revision from public.profile_posts \
         where post_id=$1 for update",
    )
    .bind(post_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(store)?;
    let Some(row) = row else {
        return Err(ProfilePostError::Unavailable);
    };
    let owner: Uuid = row.try_get("author_profile_id").map_err(decode)?;
    // A post belonging to someone else is reported as unavailable, exactly as an
    // absent one is.
    if owner != author {
        return Err(ProfilePostError::Unavailable);
    }
    let state: String = row.try_get("state").map_err(decode)?;
    if state == "DELETED" {
        return Err(ProfilePostError::Unavailable);
    }
    let revision: i64 = row.try_get("revision").map_err(decode)?;
    if revision != expected_revision {
        return Err(ProfilePostError::Conflict);
    }
    // The revision advances exactly once, which the table guard requires.
    sqlx::query(
        "update public.profile_posts set state='DELETED', deleted_at=clock_timestamp(), \
         revision=revision+1 where post_id=$1",
    )
    .bind(post_id)
    .execute(&mut *tx)
    .await
    .map_err(store)?;
    // Associations only. The media rows are untouched: a file shared by two
    // posts must survive until both references are gone.
    sqlx::query("delete from public.profile_post_attachments where post_id=$1")
        .bind(post_id)
        .execute(&mut *tx)
        .await
        .map_err(store)?;
    let revision: i64 =
        sqlx::query_scalar("select revision from public.profile_posts where post_id=$1")
            .bind(post_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(store)?;
    tx.commit().await.map_err(store)?;
    Ok(DeletedProfilePost { post_id, revision })
}
