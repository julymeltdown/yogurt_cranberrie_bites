//! T-401: draft save, publish and delete.
//!
//! Every mutation runs in one transaction with `expected_revision` checked
//! inside it, so two concurrent saves cannot both win. The revision guard is
//! the database trigger `intro_posts_revision_guard`, which also refuses any
//! change to the post's scope (id, author, creation time) and any revival of a
//! deleted post.
//!
//! **Delete is a soft delete, and its cascade is bounded.** The post's state
//! becomes `DELETED`; its likes and comments are removed; and **nothing else
//! changes**. In particular the author's room, avatar, profile posts and
//! favorites are untouched — they live in other tables this module never
//! mentions. The test `deleting_a_post_removes_likes_and_comments_and_nothing_else`
//! snapshots every one of those tables before and after.
use super::visibility::VisibilityPolicy;
use crate::intro_posts::attachments;
use concert_application::intro_posts::{
    attachments::{plan, AttachmentCandidate, AttachmentError},
    audience::Audience,
    content::IntroContent,
    snapshot::Snapshot,
};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// The `social.OwnIntroPost` projection, as this store builds it.
///
/// The store returns the stored facts and the server-derived summary; the HTTP
/// layer serializes it. Keeping it a struct rather than a `serde_json::Value`
/// means a field cannot be silently dropped from the contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnIntroPost {
    pub intro_id: Uuid,
    pub state: String,
    pub body: String,
    pub favorites_snapshot: serde_json::Value,
    pub currently_seeking: Option<String>,
    pub interaction_style: Option<String>,
    pub attachment_media_ids: Vec<Uuid>,
    pub audience: Audience,
    pub like_count: i64,
    pub comment_count: i64,
    pub published_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revision: i64,
    pub updated_at: DateTime<Utc>,
}

impl OwnIntroPost {
    /// The `social.OwnIntroPost` wire projection.
    ///
    /// `audience_summary` is produced by [`Audience::summary`] here and nowhere
    /// else, which is what makes T-408's "the review step never computes the
    /// summary locally" true: the client has nothing to compute from, and the
    /// string it renders came from this function.
    pub fn to_wire(&self) -> serde_json::Value {
        serde_json::json!({
            "intro_id": self.intro_id,
            "state": self.state,
            "body": self.body,
            "favorites_snapshot": self.favorites_snapshot,
            "currently_seeking": self.currently_seeking,
            "interaction_style": self.interaction_style,
            "attachment_media_ids": self.attachment_media_ids,
            "audience": self.audience.to_wire(),
            "audience_summary": self.audience.summary(),
            "like_count": self.like_count,
            "comment_count": self.comment_count,
            "published_at": self.published_at.map(utc),
            "expires_at": self.expires_at.map(utc),
            "revision": self.revision,
            "updated_at": utc(self.updated_at),
        })
    }
}

fn utc(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntroCommandError {
    /// The caller is not the author of this post.
    PermissionDenied,
    /// The post does not exist, is deleted, or is not the caller's.
    ///
    /// Used for every "you cannot see this" case so the response never
    /// distinguishes absence from a denial.
    Unavailable,
    /// `expected_revision` did not match the stored revision.
    VersionConflict,
    /// The content failed a rule.
    InvalidContent(concert_application::intro_posts::content::ContentError),
    /// The audience failed a rule.
    InvalidAudience(concert_application::intro_posts::audience::AudienceError),
    /// An attachment could not be attached, for any reason.
    InvalidAttachment,
    /// The snapshot failed a rule.
    InvalidSnapshot,
    /// The post is not in a state that allows this transition.
    InvalidTransition,
    /// The store is unavailable.
    Unavailable2,
}

impl IntroCommandError {
    /// The HTTP status this error produces.
    pub fn status(self) -> u16 {
        match self {
            Self::PermissionDenied => 403,
            Self::Unavailable => 404,
            Self::VersionConflict => 409,
            Self::InvalidContent(_)
            | Self::InvalidAudience(_)
            | Self::InvalidAttachment
            | Self::InvalidSnapshot
            | Self::InvalidTransition => 400,
            Self::Unavailable2 => 503,
        }
    }

    /// The closed error reason the gateway renders.
    pub fn reason(self) -> &'static str {
        match self {
            Self::PermissionDenied => "PERMISSION_DENIED",
            Self::Unavailable => "RESOURCE_UNAVAILABLE",
            Self::VersionConflict => "VERSION_CONFLICT",
            Self::InvalidContent(_)
            | Self::InvalidAudience(_)
            | Self::InvalidAttachment
            | Self::InvalidSnapshot => "INVALID_INPUT",
            Self::InvalidTransition => "INVALID_TRANSITION",
            Self::Unavailable2 => "DEPENDENCY_UNAVAILABLE",
        }
    }
}

impl std::fmt::Display for IntroCommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "intro command error: {}", self.reason())
    }
}

impl std::error::Error for IntroCommandError {}

impl From<sqlx::Error> for IntroCommandError {
    fn from(_: sqlx::Error) -> Self {
        Self::Unavailable2
    }
}

impl From<AttachmentError> for IntroCommandError {
    fn from(_: AttachmentError) -> Self {
        // Every attachment failure is the same answer on the wire.
        Self::InvalidAttachment
    }
}

/// The save request, already parsed from `social.SaveIntroPost`.
#[derive(Clone, Debug)]
pub struct SaveDraft {
    pub expected_revision: i64,
    pub content: IntroContent,
    pub favorites_snapshot: Snapshot,
    pub attachment_media_ids: Vec<Uuid>,
    pub audience: Audience,
    pub expires_at: Option<DateTime<Utc>>,
}

/// The intro post store.
pub struct IntroPostStore {
    pool: PgPool,
    visibility: VisibilityPolicy,
}

impl IntroPostStore {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            visibility: VisibilityPolicy::deny_restricted(),
        }
    }

    /// Install the composed visibility policy. WP-04's composition calls this.
    pub fn with_visibility(pool: PgPool, visibility: VisibilityPolicy) -> Self {
        Self { pool, visibility }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn visibility(&self) -> &VisibilityPolicy {
        &self.visibility
    }

    /// Save a draft.
    ///
    /// `intro_id` is supplied by the caller so creation and update are the same
    /// operation: `expected_revision = 0` creates, any other value updates.
    /// That is the `common.CreationRevision` convention the contract uses.
    ///
    /// The favorites snapshot is written **only on creation**. On an update the
    /// stored snapshot is preserved regardless of what the caller sent, which is
    /// the T-402 rule: editing the profile must not change a post's snapshot.
    pub async fn save_draft(
        &self,
        author: Uuid,
        intro_id: Uuid,
        draft: &SaveDraft,
    ) -> Result<OwnIntroPost, IntroCommandError> {
        draft
            .content
            .validate_draft()
            .map_err(IntroCommandError::InvalidContent)?;
        let mut tx = self.pool.begin().await?;
        // Serialize writers for this post so the revision check below is exact.
        sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(intro_id.to_string())
            .execute(&mut *tx)
            .await?;
        let existing = sqlx::query(
            "select author_profile_id,state,revision from intro_posts where intro_id=$1 for update",
        )
        .bind(intro_id)
        .fetch_optional(&mut *tx)
        .await?;
        let attachments = self.validate_attachments(&mut tx, author, &draft.attachment_media_ids).await?;
        let snapshot = draft.favorites_snapshot.to_stored();
        match existing {
            None => {
                if draft.expected_revision != 0 {
                    return Err(IntroCommandError::VersionConflict);
                }
                sqlx::query(
                    "insert into intro_posts(intro_id,author_profile_id,body,favorites_snapshot,\
                     currently_seeking,interaction_style,audience_gender,audience_min_age,\
                     audience_max_age_exclusive,state,revision,expires_at) \
                     values($1,$2,$3,$4,$5,$6,$7,$8,$9,'DRAFT',1,$10)",
                )
                .bind(intro_id)
                .bind(author)
                .bind(&draft.content.body)
                .bind(&snapshot)
                .bind(draft.content.currently_seeking.as_deref())
                .bind(draft.content.interaction_style.as_deref())
                .bind(draft.audience.gender.as_str())
                .bind(draft.audience.minimum_age)
                .bind(draft.audience.maximum_age_exclusive)
                .bind(draft.expires_at)
                .execute(&mut *tx)
                .await?;
            }
            Some(row) => {
                let stored_author: Uuid = row.try_get("author_profile_id")?;
                let state: String = row.try_get("state")?;
                let revision: i64 = row.try_get("revision")?;
                if stored_author != author {
                    // Not the author: the same answer as a missing post.
                    return Err(IntroCommandError::Unavailable);
                }
                if state == "DELETED" {
                    return Err(IntroCommandError::InvalidTransition);
                }
                if revision != draft.expected_revision {
                    return Err(IntroCommandError::VersionConflict);
                }
                // The stored snapshot is deliberately not touched here.
                sqlx::query(
                    "update intro_posts set body=$2,currently_seeking=$3,interaction_style=$4,\
                     audience_gender=$5,audience_min_age=$6,audience_max_age_exclusive=$7,\
                     expires_at=$8,revision=revision+1 where intro_id=$1",
                )
                .bind(intro_id)
                .bind(&draft.content.body)
                .bind(draft.content.currently_seeking.as_deref())
                .bind(draft.content.interaction_style.as_deref())
                .bind(draft.audience.gender.as_str())
                .bind(draft.audience.minimum_age)
                .bind(draft.audience.maximum_age_exclusive)
                .bind(draft.expires_at)
                .execute(&mut *tx)
                .await?;
            }
        }
        self.write_attachments(&mut tx, intro_id, &attachments).await?;
        let post = self.read_own(&mut tx, author, intro_id).await?;
        tx.commit().await?;
        Ok(post)
    }

    /// Publish a draft.
    ///
    /// The body must be non-empty and must pass the contact-bypass rule. The
    /// author needs no claim of their own: the default is that a post's
    /// restriction binds viewers, never its author.
    pub async fn publish(
        &self,
        author: Uuid,
        intro_id: Uuid,
        expected_revision: i64,
    ) -> Result<OwnIntroPost, IntroCommandError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(intro_id.to_string())
            .execute(&mut *tx)
            .await?;
        let row = sqlx::query(
            "select author_profile_id,state,revision,body,currently_seeking,interaction_style,\
             expires_at from intro_posts where intro_id=$1 for update",
        )
        .bind(intro_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            return Err(IntroCommandError::Unavailable);
        };
        let stored_author: Uuid = row.try_get("author_profile_id")?;
        if stored_author != author {
            return Err(IntroCommandError::Unavailable);
        }
        let state: String = row.try_get("state")?;
        let revision: i64 = row.try_get("revision")?;
        if state == "DELETED" {
            return Err(IntroCommandError::InvalidTransition);
        }
        if revision != expected_revision {
            return Err(IntroCommandError::VersionConflict);
        }
        let content = IntroContent {
            body: row.try_get("body")?,
            currently_seeking: row.try_get("currently_seeking")?,
            interaction_style: row.try_get("interaction_style")?,
        };
        content
            .validate_publish()
            .map_err(IntroCommandError::InvalidContent)?;
        let expires_at: Option<DateTime<Utc>> = row.try_get("expires_at")?;
        let now = Utc::now();
        if expires_at.is_some_and(|value| value <= now) {
            return Err(IntroCommandError::InvalidTransition);
        }
        sqlx::query(
            "update intro_posts set state='PUBLISHED',published_at=clock_timestamp(),\
             revision=revision+1 where intro_id=$1",
        )
        .bind(intro_id)
        .execute(&mut *tx)
        .await?;
        let post = self.read_own(&mut tx, author, intro_id).await?;
        tx.commit().await?;
        Ok(post)
    }

    /// Soft-delete a post and cascade its likes and comments.
    ///
    /// The cascade is exactly two tables: `intro_likes` and `intro_comments`.
    /// Attachments are associations and are removed too, because an association
    /// to a deleted post is meaningless; the media files themselves are not
    /// touched, so a file shared with another post survives.
    pub async fn delete(
        &self,
        author: Uuid,
        intro_id: Uuid,
        expected_revision: i64,
    ) -> Result<(), IntroCommandError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(intro_id.to_string())
            .execute(&mut *tx)
            .await?;
        let row = sqlx::query(
            "select author_profile_id,state,revision from intro_posts where intro_id=$1 for update",
        )
        .bind(intro_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            return Err(IntroCommandError::Unavailable);
        };
        let stored_author: Uuid = row.try_get("author_profile_id")?;
        if stored_author != author {
            return Err(IntroCommandError::Unavailable);
        }
        let state: String = row.try_get("state")?;
        let revision: i64 = row.try_get("revision")?;
        if state == "DELETED" {
            return Err(IntroCommandError::InvalidTransition);
        }
        if revision != expected_revision {
            return Err(IntroCommandError::VersionConflict);
        }
        // The children go first so the post's own guard sees a clean state.
        sqlx::query("delete from intro_comments where intro_id=$1")
            .bind(intro_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("delete from intro_likes where intro_id=$1")
            .bind(intro_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("delete from intro_attachments where intro_id=$1")
            .bind(intro_id)
            .execute(&mut *tx)
            .await?;
        // `intro_posts_published_shape` requires a non-published row to carry no
        // publication instant, so the delete clears it. The post's identity,
        // author and creation time are immutable and stay.
        sqlx::query(
            "update intro_posts set state='DELETED',published_at=null,revision=revision+1 \
             where intro_id=$1",
        )
        .bind(intro_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Read the author's own projection.
    pub async fn read_own(
        &self,
        executor: &mut sqlx::PgConnection,
        author: Uuid,
        intro_id: Uuid,
    ) -> Result<OwnIntroPost, IntroCommandError> {
        let row = sqlx::query(
            "select intro_id,state,body,favorites_snapshot,currently_seeking,interaction_style,\
             audience_gender,audience_min_age,audience_max_age_exclusive,published_at,expires_at,\
             revision,updated_at,author_profile_id \
             from intro_posts where intro_id=$1",
        )
        .bind(intro_id)
        .fetch_optional(&mut *executor)
        .await?;
        let Some(row) = row else {
            return Err(IntroCommandError::Unavailable);
        };
        let stored_author: Uuid = row.try_get("author_profile_id")?;
        if stored_author != author {
            return Err(IntroCommandError::Unavailable);
        }
        self.own_from_row(&mut *executor, &row).await
    }

    async fn own_from_row(
        &self,
        executor: &mut sqlx::PgConnection,
        row: &sqlx::postgres::PgRow,
    ) -> Result<OwnIntroPost, IntroCommandError> {
        let intro_id: Uuid = row.try_get("intro_id")?;
        let gender = row
            .try_get::<String, _>("audience_gender")
            .ok()
            .and_then(|value| concert_application::intro_posts::audience::GenderCondition::parse(&value))
            .ok_or(IntroCommandError::Unavailable2)?;
        let audience = Audience::new(
            gender,
            row.try_get("audience_min_age")?,
            row.try_get("audience_max_age_exclusive")?,
        )
        .map_err(IntroCommandError::InvalidAudience)?;
        let attachment_media_ids: Vec<Uuid> = sqlx::query_scalar(
            "select media_id from intro_attachments where intro_id=$1 order by position",
        )
        .bind(intro_id)
        .fetch_all(&mut *executor)
        .await?;
        let like_count: i64 =
            sqlx::query_scalar("select count(*) from intro_likes where intro_id=$1")
                .bind(intro_id)
                .fetch_one(&mut *executor)
                .await?;
        let comment_count: i64 = sqlx::query_scalar(
            "select count(*) from intro_comments where intro_id=$1 and state='PUBLISHED'",
        )
        .bind(intro_id)
        .fetch_one(&mut *executor)
        .await?;
        Ok(OwnIntroPost {
            intro_id,
            state: row.try_get("state")?,
            body: row.try_get("body")?,
            favorites_snapshot: row.try_get("favorites_snapshot")?,
            currently_seeking: row.try_get("currently_seeking")?,
            interaction_style: row.try_get("interaction_style")?,
            attachment_media_ids,
            audience,
            like_count,
            comment_count,
            published_at: row.try_get("published_at")?,
            expires_at: row.try_get("expires_at")?,
            revision: row.try_get("revision")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    /// Validate an attachment set against `profile_media`.
    ///
    /// The store reads the candidate rows and hands them to the pure rule. A
    /// foreign id and an absent id both produce `Unavailable`, so the caller
    /// cannot tell them apart.
    async fn validate_attachments(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        author: Uuid,
        requested: &[Uuid],
    ) -> Result<Vec<Uuid>, IntroCommandError> {
        if requested.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            "select media_id,owner_profile_id,purpose,state from profile_media \
             where media_id = any($1)",
        )
        .bind(requested)
        .fetch_all(&mut **tx)
        .await?;
        let found: Vec<AttachmentCandidate> = rows
            .iter()
            .map(|row| {
                let state: String = row.try_get("state").unwrap_or_else(|_| "DELETED".to_owned());
                AttachmentCandidate {
                    media_id: row.try_get("media_id").unwrap_or_else(|_| Uuid::nil()),
                    owner_profile_id: row
                        .try_get("owner_profile_id")
                        .unwrap_or_else(|_| Uuid::nil()),
                    purpose: row
                        .try_get::<String, _>("purpose")
                        .ok()
                        .and_then(|value| attachments::MediaPurpose::parse(&value))
                        .unwrap_or(attachments::MediaPurpose::ProfilePost),
                    ready: state == "READY",
                }
            })
            .collect();
        let plan = plan(requested, author, &found)?;
        Ok(plan.media_ids)
    }

    /// Replace the post's attachments with `media_ids`, in order.
    async fn write_attachments(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        intro_id: Uuid,
        media_ids: &[Uuid],
    ) -> Result<(), IntroCommandError> {
        sqlx::query("delete from intro_attachments where intro_id=$1")
            .bind(intro_id)
            .execute(&mut **tx)
            .await?;
        for (position, media_id) in media_ids.iter().enumerate() {
            sqlx::query(
                "insert into intro_attachments(intro_id,media_id,position) values($1,$2,$3)",
            )
            .bind(intro_id)
            .bind(media_id)
            .bind(position as i16)
            .execute(&mut **tx)
            .await?;
        }
        Ok(())
    }
}
