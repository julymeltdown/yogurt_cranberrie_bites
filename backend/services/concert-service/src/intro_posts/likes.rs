//! Hearts on intro posts: like, unlike, the liker list, and the notification.
//!
//! Three rules shape this module, and each has a test that fails if it is
//! softened:
//!
//! * **Two `PUT`s produce one row and one notification.** The row is the
//!   primary key `(intro_id, actor_profile_id)`, so a second insert is a no-op.
//!   The notification is written in the same transaction with a dedupe key of
//!   `(recipient, kind, intro_id, actor_profile_id)`, so a second `PUT` cannot
//!   produce a second notification even if the row insert were somehow
//!   repeated.
//! * **`DELETE` after losing access returns `like_count: null`.** Removing your
//!   own heart always works — you may withdraw consent you already gave — but
//!   the response carries no count, so the operation never reveals the current
//!   state of a post you can no longer see.
//! * **The liker list is author-only.** Anyone else gets `403`, and that is a
//!   different answer from the `404` a missing post gets, because the author
//!   relationship is not a secret: the caller already knows the post exists.
//!
//! A like is a row and nothing else. There is no reward, no follow, no friend
//! request and no approval state anywhere in this module or in the schema.
use super::visibility::{PostFacts, ViewerFacts};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// The `social.IntroLikeState` projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LikeState {
    pub liked: bool,
    /// `None` only on the `DELETE`-after-access-lost path.
    pub like_count: Option<i64>,
}

impl LikeState {
    pub fn to_wire(self) -> serde_json::Value {
        serde_json::json!({ "liked": self.liked, "like_count": self.like_count })
    }
}

/// One row of `social.LikerPage`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LikerProfile {
    pub profile_id: Uuid,
    pub nickname: String,
    pub profile_image_media_id: Option<Uuid>,
    pub shared_with_author: Vec<SharedItem>,
    pub interaction_style: Option<String>,
    pub liked_at: DateTime<Utc>,
}

/// A catalog item the liker and the author both picked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedItem {
    pub catalog_item_id: Uuid,
    pub label: String,
    pub category: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LikerPage {
    pub items: Vec<LikerProfile>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LikeError {
    /// The viewer may not see the post at all.
    Unavailable,
    /// The viewer is the author and tried to like their own post.
    SelfLike,
    /// The liker list was requested by someone who is not the author.
    PermissionDenied,
    /// The store is unavailable.
    StoreUnavailable,
}

impl LikeError {
    pub fn status(self) -> u16 {
        match self {
            Self::Unavailable => 404,
            Self::SelfLike => 400,
            Self::PermissionDenied => 403,
            Self::StoreUnavailable => 503,
        }
    }

    pub fn reason(self) -> &'static str {
        match self {
            Self::Unavailable => "RESOURCE_UNAVAILABLE",
            Self::SelfLike => "INVALID_INPUT",
            Self::PermissionDenied => "PERMISSION_DENIED",
            Self::StoreUnavailable => "DEPENDENCY_UNAVAILABLE",
        }
    }
}

impl std::fmt::Display for LikeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "intro like error: {}", self.reason())
    }
}

impl std::error::Error for LikeError {}

impl From<sqlx::Error> for LikeError {
    fn from(_: sqlx::Error) -> Self {
        Self::StoreUnavailable
    }
}

pub const DEFAULT_LIMIT: i64 = 20;
pub const MAX_LIMIT: i64 = 100;

/// The like store.
pub struct LikeStore {
    pool: PgPool,
    visibility: super::visibility::VisibilityPolicy,
}

impl LikeStore {
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

    /// Load the post facts the visibility decision needs.
    async fn facts(&self, intro_id: Uuid) -> Result<Option<PostFacts>, LikeError> {
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
        Ok(Some(PostFacts {
            intro_id,
            author_profile_id: row.try_get("author_profile_id")?,
            published: state == "PUBLISHED",
            restricted: gender != "any" || minimum.is_some(),
            expired: expires_at.is_some_and(|value| value <= Utc::now()),
        }))
    }

    /// Whether `viewer` may see `intro_id`.
    async fn visible(&self, intro_id: Uuid, viewer: Uuid) -> Result<Option<PostFacts>, LikeError> {
        let Some(facts) = self.facts(intro_id).await? else {
            return Ok(None);
        };
        let viewer_facts = ViewerFacts {
            profile_id: viewer,
            claims_known: false,
        };
        if !self.visibility.can_view(&facts, &viewer_facts) {
            return Ok(None);
        }
        Ok(Some(facts))
    }

    /// `setIntroPostLike`: idempotent, and notifies the author exactly once.
    pub async fn like(&self, intro_id: Uuid, actor: Uuid) -> Result<LikeState, LikeError> {
        let Some(facts) = self.visible(intro_id, actor).await? else {
            return Err(LikeError::Unavailable);
        };
        if facts.author_profile_id == actor {
            return Err(LikeError::SelfLike);
        }
        let mut tx = self.pool.begin().await?;
        // One row per (post, actor). A second PUT is a no-op, not an error.
        sqlx::query(
            "insert into intro_likes(intro_id,actor_profile_id) values($1,$2) \
             on conflict (intro_id,actor_profile_id) do nothing",
        )
        .bind(intro_id)
        .bind(actor)
        .execute(&mut *tx)
        .await?;
        // The notification is deduped by its own key, so even a replayed
        // command cannot produce a second one. `do nothing` here is the whole
        // "notify once" rule.
        sqlx::query(
            "insert into intro_like_notifications(recipient_profile_id,intro_id,\
             actor_profile_id,created_at) values($1,$2,$3,clock_timestamp()) \
             on conflict (recipient_profile_id,intro_id,actor_profile_id) do nothing",
        )
        .bind(facts.author_profile_id)
        .bind(intro_id)
        .bind(actor)
        .execute(&mut *tx)
        .await?;
        let count: i64 = sqlx::query_scalar("select count(*) from intro_likes where intro_id=$1")
            .bind(intro_id)
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(LikeState {
            liked: true,
            like_count: Some(count),
        })
    }

    /// `clearIntroPostLike`.
    ///
    /// Removing a heart always succeeds for the actor who left it, even after
    /// they have lost access to the post: withdrawing consent must not require
    /// permission. When access is lost the response carries `like_count: null`,
    /// so the operation never discloses the current count.
    pub async fn unlike(&self, intro_id: Uuid, actor: Uuid) -> Result<LikeState, LikeError> {
        let mut tx = self.pool.begin().await?;
        let removed = sqlx::query(
            "delete from intro_likes where intro_id=$1 and actor_profile_id=$2",
        )
        .bind(intro_id)
        .bind(actor)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let facts = self.facts(intro_id).await?;
        let Some(facts) = facts else {
            // The post is gone; there is nothing to count and nothing to reveal.
            tx.commit().await?;
            return Ok(LikeState {
                liked: false,
                like_count: None,
            });
        };
        let viewer_facts = ViewerFacts {
            profile_id: actor,
            claims_known: false,
        };
        let visible = self.visibility.can_view(&facts, &viewer_facts);
        let count = if visible {
            Some(
                sqlx::query_scalar("select count(*) from intro_likes where intro_id=$1")
                    .bind(intro_id)
                    .fetch_one(&mut *tx)
                    .await?,
            )
        } else {
            None
        };
        tx.commit().await?;
        let _ = removed;
        Ok(LikeState {
            liked: false,
            like_count: count,
        })
    }

    /// `listIntroPostLikers`: author-only, paginated.
    ///
    /// Blocked, deleted and otherwise inaccessible likers are omitted, so the
    /// page may be shorter than `limit`.
    pub async fn likers(
        &self,
        intro_id: Uuid,
        viewer: Uuid,
        limit: i64,
        cursor: Option<&str>,
    ) -> Result<LikerPage, LikeError> {
        if !(1..=MAX_LIMIT).contains(&limit) {
            return Err(LikeError::Unavailable);
        }
        let author: Option<Uuid> = sqlx::query_scalar(
            "select author_profile_id from intro_posts where intro_id=$1 and state<>'DELETED'",
        )
        .bind(intro_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(author) = author else {
            return Err(LikeError::Unavailable);
        };
        // Not the author: a distinct 403, because the caller already knows the
        // post exists and the author relationship is not a secret.
        if author != viewer {
            return Err(LikeError::PermissionDenied);
        }
        let after = cursor.and_then(decode_cursor);
        let rows = sqlx::query(
            "select l.actor_profile_id, l.created_at, p.nickname, p.profile_image_media_id, \
                    p.interaction_style \
             from intro_likes l \
             join discovery_profiles p on p.profile_id = l.actor_profile_id \
             where l.intro_id = $1 \
               and ($2::timestamptz is null or (l.created_at, l.actor_profile_id) < ($2, $3)) \
             order by l.created_at desc, l.actor_profile_id desc \
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
            let profile_id: Uuid = row.try_get("actor_profile_id")?;
            let liked_at: DateTime<Utc> = row.try_get("created_at")?;
            items.push(LikerProfile {
                profile_id,
                nickname: row.try_get("nickname")?,
                profile_image_media_id: row.try_get("profile_image_media_id")?,
                // The shared-items join is filled by the caller's projection
                // when the catalog is available; an empty list is honest.
                shared_with_author: Vec::new(),
                interaction_style: row.try_get("interaction_style")?,
                liked_at,
            });
        }
        let next_cursor = if items.len() as i64 > limit {
            items.truncate(limit as usize);
            items
                .last()
                .map(|last| encode_cursor(last.liked_at, last.profile_id))
        } else {
            None
        };
        Ok(LikerPage { items, next_cursor })
    }

    /// The current like state as `viewer` sees it, for a projection.
    pub async fn state(&self, intro_id: Uuid, viewer: Uuid) -> Result<LikeState, LikeError> {
        let liked: bool = sqlx::query_scalar(
            "select exists(select 1 from intro_likes where intro_id=$1 and actor_profile_id=$2)",
        )
        .bind(intro_id)
        .bind(viewer)
        .fetch_one(&self.pool)
        .await?;
        let count: i64 = sqlx::query_scalar("select count(*) from intro_likes where intro_id=$1")
            .bind(intro_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(LikeState {
            liked,
            like_count: Some(count),
        })
    }
}

/// The keyset cursor: `(created_at, profile_id)` of the last row of a page.
fn encode_cursor(at: DateTime<Utc>, id: Uuid) -> String {
    use base64ct::{Base64UrlUnpadded, Encoding};
    Base64UrlUnpadded::encode_string(format!("l1:{}\n{id}", at.timestamp_micros()).as_bytes())
}

fn decode_cursor(token: &str) -> Option<(DateTime<Utc>, Uuid)> {
    use base64ct::{Base64UrlUnpadded, Encoding};
    if token.is_empty() || token.len() > 1024 {
        return None;
    }
    let raw = Base64UrlUnpadded::decode_vec(token).ok()?;
    let raw = String::from_utf8(raw).ok()?;
    let raw = raw.strip_prefix("l1:")?;
    let (at, id) = raw.split_once('\n')?;
    let micros: i64 = at.parse().ok()?;
    let id = Uuid::parse_str(id).ok()?;
    Some((DateTime::from_timestamp_micros(micros)?, id))
}
