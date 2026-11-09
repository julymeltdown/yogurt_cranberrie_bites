//! T-404: comments and replies.
//!
//! Every read and write is gated by the parent post's audience. A viewer who may
//! not see the post gets the same `404` for creating a comment as for reading
//! one, so comments cannot be used to probe a restricted post's existence.
//!
//! **Deletion rule (documented here and in the contract, tested in
//! `tests/intro_comments.rs`).** Deleting a comment is a soft delete: the row is
//! kept with `state = 'DELETED'`, its body is no longer returned, and its
//! replies keep their parent. Replies are **not** deleted with their parent,
//! because a reply is a different author's contribution and removing one
//! person's comment must not silently erase somebody else's. The projection
//! renders a deleted comment as `deleted: true` with an empty body, so a reader
//! sees that something was removed without learning what it said.
//!
//! Deleting the *post* removes everything: that is the separate cascade in
//! `commands::IntroPostStore::delete`.
use super::visibility::{PostFacts, ViewerFacts};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// The `social.IntroComment` projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntroComment {
    pub comment_id: Uuid,
    pub intro_id: Uuid,
    pub author_profile_id: Uuid,
    pub author_nickname: String,
    pub author_profile_image_media_id: Option<Uuid>,
    pub parent_comment_id: Option<Uuid>,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub deleted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntroCommentPage {
    pub items: Vec<IntroComment>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommentError {
    /// The viewer may not see the post.
    Unavailable,
    /// The comment does not exist, is on another post, or is not the caller's.
    CommentUnavailable,
    /// The body, parent or thread depth failed a rule.
    Invalid(concert_application::intro_posts::comments::CommentError),
    /// The store is unavailable.
    StoreUnavailable,
}

impl CommentError {
    pub fn status(self) -> u16 {
        match self {
            Self::Unavailable | Self::CommentUnavailable => 404,
            Self::Invalid(_) => 400,
            Self::StoreUnavailable => 503,
        }
    }

    pub fn reason(self) -> &'static str {
        match self {
            Self::Unavailable | Self::CommentUnavailable => "RESOURCE_UNAVAILABLE",
            Self::Invalid(_) => "INVALID_INPUT",
            Self::StoreUnavailable => "DEPENDENCY_UNAVAILABLE",
        }
    }
}

impl std::fmt::Display for CommentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "intro comment error: {}", self.reason())
    }
}

impl std::error::Error for CommentError {}

impl From<sqlx::Error> for CommentError {
    fn from(_: sqlx::Error) -> Self {
        Self::StoreUnavailable
    }
}

pub const DEFAULT_LIMIT: i64 = 20;
pub const MAX_LIMIT: i64 = 100;

pub struct CommentStore {
    pool: PgPool,
    visibility: super::visibility::VisibilityPolicy,
}

impl CommentStore {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            visibility: super::visibility::VisibilityPolicy::deny_restricted(),
        }
    }

    pub fn with_visibility(
        pool: PgPool,
        visibility: super::visibility::VisibilityPolicy,
    ) -> Self {
        Self { pool, visibility }
    }

    /// Whether `viewer` may see `intro_id`, and the post facts if so.
    async fn visible(&self, intro_id: Uuid, viewer: Uuid) -> Result<Option<PostFacts>, CommentError> {
        let row = sqlx::query(
            "select author_profile_id,state,audience_gender,audience_min_age,expires_at \
             from intro_posts where intro_id=$1",
        )
        .bind(intro_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let state: String = row.try_get("state")?;
        let gender: String = row.try_get("audience_gender")?;
        let minimum: Option<i16> = row.try_get("audience_min_age")?;
        let expires_at: Option<DateTime<Utc>> = row.try_get("expires_at")?;
        let facts = PostFacts {
            intro_id,
            author_profile_id: row.try_get("author_profile_id")?,
            published: state == "PUBLISHED",
            restricted: gender != "any" || minimum.is_some(),
            expired: expires_at.is_some_and(|value| value <= Utc::now()),
        };
        let viewer_facts = ViewerFacts {
            profile_id: viewer,
            claims_known: false,
        };
        if !self.visibility.can_view(&facts, &viewer_facts, Utc::now()).await {
            return Ok(None);
        }
        Ok(Some(facts))
    }

    /// Create a comment or a reply.
    pub async fn create(
        &self,
        intro_id: Uuid,
        author: Uuid,
        body: &str,
        parent_comment_id: Option<Uuid>,
    ) -> Result<IntroComment, CommentError> {
        // The audience gate comes first: a viewer who cannot see the post must
        // not learn anything about its comments.
        if self.visible(intro_id, author).await?.is_none() {
            return Err(CommentError::Unavailable);
        }
        let parent = match parent_comment_id {
            None => None,
            Some(id) => {
                let row = sqlx::query(
                    "select comment_id,intro_id,parent_comment_id,state from intro_comments \
                     where comment_id=$1",
                )
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
                row.map(|row| {
                    Ok::<_, sqlx::Error>(concert_application::intro_posts::comments::ParentComment {
                        comment_id: row.try_get("comment_id")?,
                        intro_id: row.try_get("intro_id")?,
                        parent_comment_id: row.try_get("parent_comment_id")?,
                        deleted: row.try_get::<String, _>("state")? == "DELETED",
                    })
                })
                .transpose()?
            }
        };
        let input = concert_application::intro_posts::comments::validate(
            intro_id,
            body,
            parent_comment_id,
            parent,
        )
        .map_err(CommentError::Invalid)?;
        let comment_id = Uuid::new_v4();
        sqlx::query(
            "insert into intro_comments(comment_id,intro_id,author_profile_id,parent_comment_id,body) \
             values($1,$2,$3,$4,$5)",
        )
        .bind(comment_id)
        .bind(intro_id)
        .bind(author)
        .bind(input.parent_comment_id)
        .bind(&input.body)
        .execute(&self.pool)
        .await?;
        self.read_one(comment_id).await
    }

    async fn read_one(&self, comment_id: Uuid) -> Result<IntroComment, CommentError> {
        let row = sqlx::query(
            "select c.comment_id,c.intro_id,c.author_profile_id,c.parent_comment_id,c.body,\
                    c.created_at,c.state,p.nickname,p.profile_image_media_id \
             from intro_comments c \
             left join discovery_profiles p on p.profile_id = c.author_profile_id \
             where c.comment_id=$1",
        )
        .bind(comment_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Err(CommentError::CommentUnavailable);
        };
        let deleted: String = row.try_get("state")?;
        let deleted = deleted == "DELETED";
        Ok(IntroComment {
            comment_id: row.try_get("comment_id")?,
            intro_id: row.try_get("intro_id")?,
            author_profile_id: row.try_get("author_profile_id")?,
            author_nickname: row
                .try_get::<Option<String>, _>("nickname")?
                .unwrap_or_default(),
            author_profile_image_media_id: row.try_get("profile_image_media_id")?,
            parent_comment_id: row.try_get("parent_comment_id")?,
            // A deleted comment's body is never returned.
            body: if deleted {
                String::new()
            } else {
                row.try_get("body")?
            },
            created_at: row.try_get("created_at")?,
            deleted,
        })
    }

    /// List a post's comments, oldest first, paginated.
    ///
    /// The list is never post-filtered: the audience predicate is part of the
    /// query's gate, not a filter applied to a page afterwards, so a page of 20
    /// really is 20 comments the viewer may see.
    pub async fn list(
        &self,
        intro_id: Uuid,
        viewer: Uuid,
        limit: i64,
        cursor: Option<&str>,
    ) -> Result<IntroCommentPage, CommentError> {
        if !(1..=MAX_LIMIT).contains(&limit) {
            return Err(CommentError::Unavailable);
        }
        if self.visible(intro_id, viewer).await?.is_none() {
            return Err(CommentError::Unavailable);
        }
        let after = cursor.and_then(decode_cursor);
        let rows = sqlx::query(
            "select c.comment_id,c.intro_id,c.author_profile_id,c.parent_comment_id,c.body,\
                    c.created_at,c.state,p.nickname,p.profile_image_media_id \
             from intro_comments c \
             left join discovery_profiles p on p.profile_id = c.author_profile_id \
             where c.intro_id=$1 \
               and ($2::timestamptz is null or (c.created_at, c.comment_id) > ($2, $3)) \
             order by c.created_at, c.comment_id \
             limit $4",
        )
        .bind(intro_id)
        .bind(after.as_ref().map(|(at, _)| *at))
        .bind(after.as_ref().map(|(_, id)| *id))
        .bind(limit + 1)
        .fetch_all(&self.pool)
        .await?;
        let mut items = Vec::with_capacity(rows.len());
        for row in &rows {
            let deleted: String = row.try_get("state")?;
            let deleted = deleted == "DELETED";
            items.push(IntroComment {
                comment_id: row.try_get("comment_id")?,
                intro_id: row.try_get("intro_id")?,
                author_profile_id: row.try_get("author_profile_id")?,
                author_nickname: row
                    .try_get::<Option<String>, _>("nickname")?
                    .unwrap_or_default(),
                author_profile_image_media_id: row.try_get("profile_image_media_id")?,
                parent_comment_id: row.try_get("parent_comment_id")?,
                body: if deleted {
                    String::new()
                } else {
                    row.try_get("body")?
                },
                created_at: row.try_get("created_at")?,
                deleted,
            });
        }
        let next_cursor = if items.len() as i64 > limit {
            items.truncate(limit as usize);
            items
                .last()
                .map(|last| encode_cursor(last.created_at, last.comment_id))
        } else {
            None
        };
        Ok(IntroCommentPage { items, next_cursor })
    }

    /// Soft-delete a comment.
    ///
    /// Only the comment's author may delete it. The post's author may also
    /// delete a comment on their own post, because a post's author is
    /// responsible for the thread beneath it. Replies survive.
    pub async fn delete(
        &self,
        intro_id: Uuid,
        comment_id: Uuid,
        viewer: Uuid,
    ) -> Result<(), CommentError> {
        let row = sqlx::query(
            "select c.author_profile_id,c.state,p.author_profile_id as post_author \
             from intro_comments c join intro_posts p on p.intro_id = c.intro_id \
             where c.comment_id=$1 and c.intro_id=$2",
        )
        .bind(comment_id)
        .bind(intro_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Err(CommentError::CommentUnavailable);
        };
        let comment_author: Uuid = row.try_get("author_profile_id")?;
        let post_author: Uuid = row.try_get("post_author")?;
        if viewer != comment_author && viewer != post_author {
            return Err(CommentError::CommentUnavailable);
        }
        let state: String = row.try_get("state")?;
        if state == "DELETED" {
            return Err(CommentError::CommentUnavailable);
        }
        sqlx::query("update intro_comments set state='DELETED' where comment_id=$1 and intro_id=$2")
            .bind(comment_id)
            .bind(intro_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The number of live comments on a post, for a projection.
    pub async fn count(&self, intro_id: Uuid) -> Result<i64, CommentError> {
        Ok(sqlx::query_scalar(
            "select count(*) from intro_comments where intro_id=$1 and state='PUBLISHED'",
        )
        .bind(intro_id)
        .fetch_one(&self.pool)
        .await?)
    }
}

fn encode_cursor(at: DateTime<Utc>, id: Uuid) -> String {
    use base64ct::{Base64UrlUnpadded, Encoding};
    Base64UrlUnpadded::encode_string(format!("c1:{}\n{id}", at.timestamp_micros()).as_bytes())
}

fn decode_cursor(token: &str) -> Option<(DateTime<Utc>, Uuid)> {
    use base64ct::{Base64UrlUnpadded, Encoding};
    if token.is_empty() || token.len() > 1024 {
        return None;
    }
    let raw = Base64UrlUnpadded::decode_vec(token).ok()?;
    let raw = String::from_utf8(raw).ok()?;
    let raw = raw.strip_prefix("c1:")?;
    let (at, id) = raw.split_once('\n')?;
    let micros: i64 = at.parse().ok()?;
    let id = Uuid::parse_str(id).ok()?;
    Some((DateTime::from_timestamp_micros(micros)?, id))
}
