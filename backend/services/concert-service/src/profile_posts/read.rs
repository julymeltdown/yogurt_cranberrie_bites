//! WP-06 T-503: profile post reads.
//!
//! Two questions, one predicate:
//!
//! * *May this viewer see this post?* — the post's **own** visibility, and
//!   nothing else. A profile post has no audience, no age bound and no room
//!   dependency, so there is no second decision to reconcile with this one.
//! * *Which posts are on this page?* — the same predicate, applied **inside the
//!   page query**.
//!
//! # Pagination is inside the query
//!
//! The product plan calls out the defect explicitly: "do not paginate on the
//! server and then filter the returned page on the client". Here the visibility
//! predicate is part of the `where` clause, the keyset cursor is part of the same
//! `where` clause, and `limit` is applied by the database. A page therefore
//! always contains `limit` visible posts (or fewer only when the profile has no
//! more), and `next_cursor` is derived from the last row actually returned — so a
//! visitor never receives a short page with a cursor pointing into a set they
//! cannot see.
//!
//! # The author sees their own private posts
//!
//! Privacy is a rule about other people. The author's own list includes every
//! live post of theirs, public and private, and the direct read of their own
//! private post succeeds. A visitor's list omits it and the direct read of it is
//! `404 RESOURCE_UNAVAILABLE` — the same answer an absent post id gets.
use super::commands::{PostVisibility, ProfilePostError};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

/// Default page size, from the frozen `common.PageQuery`.
pub const DEFAULT_LIMIT: i64 = 20;
/// Maximum page size, from the frozen `common.PageQuery`.
pub const MAX_LIMIT: i64 = 100;

/// A validated `common.PageQuery`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfilePostQuery {
    pub limit: i64,
    pub cursor: Option<String>,
}

impl ProfilePostQuery {
    /// Validate the query bounds. The cursor's *shape* is validated here; its
    /// content is validated when it is decoded.
    pub fn validate(self) -> Result<Self, ProfilePostError> {
        if !(1..=MAX_LIMIT).contains(&self.limit) {
            return Err(ProfilePostError::Invalid);
        }
        if let Some(cursor) = self.cursor.as_deref() {
            if cursor.is_empty()
                || cursor.len() > 1024
                || !cursor
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            {
                return Err(ProfilePostError::Invalid);
            }
        }
        Ok(self)
    }
}

/// The keyset position of a page: `(created_at, post_id)`, descending.
///
/// The pair is used rather than the timestamp alone because two posts can share
/// a `created_at` (the column default is `clock_timestamp()`, which is not
/// unique), and a timestamp-only cursor would then skip or repeat a post.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProfilePostCursor {
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub post_id: Uuid,
}

const CURSOR_PREFIX: &str = "pp1:";

fn encode_cursor(cursor: &ProfilePostCursor) -> String {
    use base64ct::{Base64UrlUnpadded, Encoding};
    Base64UrlUnpadded::encode_string(
        format!(
            "{CURSOR_PREFIX}{}\n{}",
            cursor.created_at.timestamp_micros(),
            cursor.post_id
        )
        .as_bytes(),
    )
}

fn decode_cursor(token: &str) -> Result<ProfilePostCursor, ProfilePostError> {
    use base64ct::{Base64UrlUnpadded, Encoding};
    if token.is_empty() || token.len() > 1024 {
        return Err(ProfilePostError::Invalid);
    }
    let raw = Base64UrlUnpadded::decode_vec(token).map_err(|_| ProfilePostError::Invalid)?;
    let raw = String::from_utf8(raw).map_err(|_| ProfilePostError::Invalid)?;
    let raw = raw
        .strip_prefix(CURSOR_PREFIX)
        .ok_or(ProfilePostError::Invalid)?;
    let (micros, id) = raw.split_once('\n').ok_or(ProfilePostError::Invalid)?;
    let micros: i64 = micros.parse().map_err(|_| ProfilePostError::Invalid)?;
    let post_id = Uuid::parse_str(id)
        .ok()
        .filter(|parsed| !parsed.is_nil() && parsed.to_string() == id)
        .ok_or(ProfilePostError::Invalid)?;
    let created_at = chrono::DateTime::from_timestamp_micros(micros)
        .ok_or(ProfilePostError::Invalid)?;
    Ok(ProfilePostCursor {
        created_at,
        post_id,
    })
}

/// One attachment of a profile post, as the page projects it (`feed.Media`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfilePostMedia {
    pub media_id: Uuid,
    pub revision: i64,
    pub width: u32,
    pub height: u32,
    pub content_type: String,
    pub purpose: String,
}

impl ProfilePostMedia {
    pub fn to_wire(&self) -> Value {
        json!({
            "media_id": self.media_id,
            "state": "READY",
            "revision": self.revision,
            "width": self.width,
            "height": self.height,
            "content_type": self.content_type,
            "purpose": self.purpose,
            "read_path": super::super::profile_media::media_read_path(self.media_id),
        })
    }
}

/// The frozen `social.ProfilePost` projection.
///
/// `author` is the `core.PublicProfileV2` subset the concert service can build
/// without a cross-service call: the profile id and the facts the concert service
/// mirrors. The profile service owns the rest and fills it in when it composes a
/// response; the concert service never invents a nickname it does not have.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfilePost {
    pub post_id: Uuid,
    pub author_profile_id: Uuid,
    pub description: String,
    pub visibility: PostVisibility,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub revision: i64,
    pub media: Vec<ProfilePostMedia>,
}

impl ProfilePost {
    pub fn to_wire(&self) -> Value {
        json!({
            "post_id": self.post_id,
            "author": {"profile_id": self.author_profile_id},
            "description": self.description,
            "media": self.media.iter().map(ProfilePostMedia::to_wire).collect::<Vec<_>>(),
            "visibility": self.visibility.as_str(),
            "created_at": self.created_at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "expires_at": self.expires_at.map(|value| {
                value.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
            }),
            "revision": self.revision,
        })
    }
}

/// The frozen `social.ProfilePostPage`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfilePostPage {
    pub items: Vec<ProfilePost>,
    pub next_cursor: Option<String>,
}

impl ProfilePostPage {
    pub fn to_wire(&self) -> Value {
        json!({
            "items": self.items.iter().map(ProfilePost::to_wire).collect::<Vec<_>>(),
            "next_cursor": self.next_cursor,
        })
    }
}

/// The visibility predicate, as SQL.
///
/// `$2` is the viewer's profile id and `$3` is the author whose posts are being
/// listed. A post is visible when it is live, unexpired, and either public or
/// authored by the viewer. This is the *only* place the rule is written for the
/// list; the direct read uses the same expression through [`visible_post`].
const VISIBLE: &str = "p.state = 'PUBLISHED' \
     and (p.expires_at is null or p.expires_at > clock_timestamp()) \
     and (p.visibility = 'PUBLIC' or p.author_profile_id = $2)";

type PostRow = (
    Uuid,
    Uuid,
    String,
    String,
    chrono::DateTime<chrono::Utc>,
    Option<chrono::DateTime<chrono::Utc>>,
    i64,
);

async fn attachments(
    pool: &PgPool,
    post_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Vec<ProfilePostMedia>>, ProfilePostError> {
    use std::collections::HashMap;
    if post_ids.is_empty() {
        return Ok(HashMap::new());
    }
    type AttachmentRow = (Uuid, Uuid, i64, i32, i32, String, String);
    let rows: Vec<AttachmentRow> = sqlx::query_as(
        "select a.post_id, m.media_id, m.revision, m.width, m.height, m.content_type, m.purpose \
         from public.profile_post_attachments a \
         join public.profile_media m on m.media_id = a.media_id \
         where a.post_id = any($1) and m.state = 'READY' and m.purpose = 'PROFILE_POST' \
         order by a.post_id, a.position",
    )
    .bind(post_ids)
    .fetch_all(pool)
    .await
    .map_err(|_| ProfilePostError::Unavailable503)?;
    let mut grouped: HashMap<Uuid, Vec<ProfilePostMedia>> = HashMap::new();
    for (post_id, media_id, revision, width, height, content_type, purpose) in rows {
        grouped.entry(post_id).or_default().push(ProfilePostMedia {
            media_id,
            revision,
            width: width as u32,
            height: height as u32,
            content_type,
            purpose,
        });
    }
    Ok(grouped)
}

fn post_from_row(row: PostRow, media: Vec<ProfilePostMedia>) -> Result<ProfilePost, ProfilePostError> {
    Ok(ProfilePost {
        post_id: row.0,
        author_profile_id: row.1,
        description: row.2,
        visibility: PostVisibility::parse(&row.3)?,
        created_at: row.4,
        expires_at: row.5,
        revision: row.6,
        media,
    })
}

/// List the posts of `author_profile_id` that `viewer_profile_id` may see.
///
/// The visibility predicate and the keyset cursor are both inside the query, so
/// the page is cut from the visible set and never post-filtered.
pub async fn list_profile_posts(
    pool: &PgPool,
    viewer_profile_id: Uuid,
    author_profile_id: Uuid,
    query: &ProfilePostQuery,
) -> Result<ProfilePostPage, ProfilePostError> {
    if viewer_profile_id.is_nil() || author_profile_id.is_nil() {
        return Err(ProfilePostError::Unavailable);
    }
    let query = query.clone().validate()?;
    let cursor = match query.cursor.as_deref() {
        Some(token) => Some(decode_cursor(token)?),
        None => None,
    };
    // One row more than the page proves whether a next page exists without a
    // second count query and without ever returning the extra row.
    let sql = format!(
        "select p.post_id, p.author_profile_id, p.description, p.visibility, p.created_at, \
                p.expires_at, p.revision \
         from public.profile_posts p \
         where p.author_profile_id = $3 and {VISIBLE} \
           and ($4::timestamptz is null \
                or (p.created_at, p.post_id) < ($4::timestamptz, $5::uuid)) \
         order by p.created_at desc, p.post_id desc \
         limit $1"
    );
    let rows: Vec<PostRow> = sqlx::query_as(&sql)
        .bind(query.limit + 1)
        .bind(viewer_profile_id)
        .bind(author_profile_id)
        .bind(cursor.map(|cursor| cursor.created_at))
        .bind(cursor.map(|cursor| cursor.post_id))
        .fetch_all(pool)
        .await
        .map_err(|_| ProfilePostError::Unavailable503)?;
    let mut rows = rows;
    let next_cursor = if rows.len() as i64 > query.limit {
        rows.truncate(query.limit as usize);
        rows.last().map(|row| {
            encode_cursor(&ProfilePostCursor {
                created_at: row.4,
                post_id: row.0,
            })
        })
    } else {
        None
    };
    let ids: Vec<Uuid> = rows.iter().map(|row| row.0).collect();
    let mut media = attachments(pool, &ids).await?;
    let items = rows
        .into_iter()
        .map(|row| {
            let attached = media.remove(&row.0).unwrap_or_default();
            post_from_row(row, attached)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ProfilePostPage { items, next_cursor })
}

/// Read one post directly.
///
/// A post the viewer may not see is `Unavailable`, which the route renders as
/// `404 RESOURCE_UNAVAILABLE` — the same answer an absent id gets.
pub async fn read_profile_post(
    pool: &PgPool,
    viewer_profile_id: Uuid,
    post_id: Uuid,
) -> Result<ProfilePost, ProfilePostError> {
    if viewer_profile_id.is_nil() || post_id.is_nil() {
        return Err(ProfilePostError::Unavailable);
    }
    // The same `VISIBLE` predicate as the list, with the post id as the only
    // additional filter. `$3` is bound to the post id because the shared
    // predicate does not use it; binding a real value keeps the two statements
    // textually identical in the predicate they apply.
    let sql = format!(
        "select p.post_id, p.author_profile_id, p.description, p.visibility, p.created_at, \
                p.expires_at, p.revision \
         from public.profile_posts p \
         where p.post_id = $1 and {VISIBLE}"
    );
    let row: Option<PostRow> = sqlx::query_as(&sql)
        .bind(post_id)
        .bind(viewer_profile_id)
        .bind(post_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| ProfilePostError::Unavailable503)?;
    let Some(row) = row else {
        return Err(ProfilePostError::Unavailable);
    };
    let mut media = attachments(pool, &[post_id]).await?;
    post_from_row(row, media.remove(&post_id).unwrap_or_default())
}
