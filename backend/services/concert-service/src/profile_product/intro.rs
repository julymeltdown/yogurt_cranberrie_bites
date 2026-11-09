//! WP-05 intro operations served by the concert-hosted profile transport.
//!
//! The thirteen frozen intro operations are profile-owned and event-free: their
//! tables (`intro_posts`, `intro_attachments`, `intro_likes`, `intro_comments`)
//! live in `mcr_concert`, so they are served by the concert service and routed
//! there by the Gateway, exactly like the fifteen profile-post/media/room/avatar
//! operations.
//!
//! ## What this module adds, and what it deliberately does not
//!
//! WP-05 delivered the route parser (`intro_posts::http`) and the mutation
//! stores (`IntroPostStore`, `LikeStore`, `CommentStore`). What was missing is a
//! transport home. This module is that home: it parses with WP-05's own closed
//! parser and calls WP-05's own store methods. It adds **no** product behaviour.
//!
//! ## The two operations that are not served here
//!
//! `listIntroPosts` and `getIntroPost` are **not** in [`SERVED`]. They need a
//! visibility-filtered page query and a visibility-filtered detail query over
//! `intro_posts`, joined to the author profile and the viewer's like state.
//! WP-05 did not build those queries, and WP-04 owns the visibility predicate
//! that would feed them (`can_view_intro` and the SQL form of it). Writing one
//! here would either duplicate WP-04's decision or hard-code the fail-closed
//! `DenyRestricted` default into a query that must later change — so they are
//! refused, and the coverage register records the exact follow-up.
//!
//! Serving a route that always fails would be worse than not serving it: the
//! frontend would mount a discovery feed that 403s on every load.
use super::{proto, transport::VerifiedProfile};
use crate::intro_posts::{
    commands::{IntroCommandError, IntroPostStore, SaveDraft},
    comments::{CommentError, CommentStore},
    http::{self, IntroOperation, Parsed},
    likes::{LikeError, LikeStore},
};
use concert_application::intro_posts::{
    audience::{Audience, GenderCondition},
    content::IntroContent,
    snapshot::{Snapshot, SnapshotItem},
};
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;

/// The operations this transport serves.
///
/// The two reads that need a visibility-filtered query are deliberately absent;
/// see the module documentation. [`is_served`] is the single predicate that
/// answers "does this transport answer this operation", and the coverage
/// register is asserted against it.
pub const SERVED: &[&str] = &[
    // ── own-post lifecycle (mutations + the author's own read) ──────────────
    "getOwnIntroPost",
    "listOwnIntroPosts",
    "saveIntroPostDraft",
    "publishIntroPost",
    "deleteIntroPost",
    // ── likes ──────────────────────────────────────────────────────────────
    "setIntroPostLike",
    "clearIntroPostLike",
    "listIntroPostLikers",
    // ── comments ───────────────────────────────────────────────────────────
    "listIntroPostComments",
    "createIntroPostComment",
    "deleteIntroPostComment",
];

/// The operations this transport refuses, with the reason recorded in the
/// coverage register.
pub const NOT_SERVED: &[&str] = &["listIntroPosts", "getIntroPost"];

/// Whether this transport answers `operation`.
pub fn is_served(operation: &str) -> bool {
    SERVED.contains(&operation)
}

/// The intro stores, built once per process.
pub struct IntroProducer {
    posts: Arc<IntroPostStore>,
    likes: Arc<LikeStore>,
    comments: Arc<CommentStore>,
}

impl IntroProducer {
    /// Build the stores over one pool.
    ///
    /// The visibility policy is WP-05's fail-closed default
    /// (`VisibilityPolicy::deny_restricted`). WP-04's composition installs the
    /// real policy through `IntroPostStore::with_visibility`; this constructor
    /// deliberately does not, so the swap stays a single owned call site rather
    /// than something a transport change could quietly pre-empt.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            posts: Arc::new(IntroPostStore::new(pool.clone())),
            likes: Arc::new(LikeStore::new(pool.clone())),
            comments: Arc::new(CommentStore::new(pool)),
        }
    }

    pub fn posts(&self) -> &Arc<IntroPostStore> {
        &self.posts
    }

    /// Serve one intro request.
    ///
    /// `actor` is the verified profile from the transport's own authority
    /// chain; it is never derived from the target or the body.
    pub async fn dispatch(
        &self,
        context: &VerifiedProfile,
        request: &proto::ProfileRequest,
    ) -> Result<super::scoped::ProfileRepresentation, Status> {
        let parsed = http::parse(&request.http_method, &request.http_target)?;
        if !is_served(parsed.contract_id()) {
            // `listIntroPosts` and `getIntroPost` land here. They are refused
            // rather than served by a query this module does not own.
            return Err(Status::permission_denied(
                "This intro operation is not served by the profile transport",
            ));
        }
        let actor = context.profile();
        match parsed.operation {
            IntroOperation::GetOwnIntroPost(intro_id) => {
                let mut tx = self
                    .posts
                    .pool()
                    .begin()
                    .await
                    .map_err(|_| Status::unavailable("Intro store unavailable"))?;
                let post = self
                    .posts
                    .read_own(&mut tx, actor, intro_id)
                    .await
                    .map_err(intro_status)?;
                tx.commit()
                    .await
                    .map_err(|_| Status::unavailable("Intro store unavailable"))?;
                json(200, post.to_wire())
            }
            IntroOperation::ListOwnIntroPosts => {
                let page = self.list_own(actor, &parsed).await?;
                json(200, page)
            }
            IntroOperation::SaveIntroPostDraft(intro_id) => {
                let draft = parse_save_draft(&request.http_body)?;
                let post = self
                    .posts
                    .save_draft(actor, intro_id, &draft)
                    .await
                    .map_err(intro_status)?;
                json(200, post.to_wire())
            }
            IntroOperation::PublishIntroPost(intro_id) => {
                let revision = parse_revision(&request.http_body)?;
                let post = self
                    .posts
                    .publish(actor, intro_id, revision)
                    .await
                    .map_err(intro_status)?;
                json(200, post.to_wire())
            }
            IntroOperation::DeleteIntroPost(intro_id) => {
                // A bodyless DELETE carries its revision in the query, which
                // WP-05's parser already extracted and validated.
                let revision = parsed
                    .query
                    .expected_revision
                    .ok_or_else(|| Status::invalid_argument("Missing expected revision"))?;
                self.posts
                    .delete(actor, intro_id, revision)
                    .await
                    .map_err(intro_status)?;
                json(
                    200,
                    serde_json::json!({
                        "resource_id": intro_id,
                        "revision": revision,
                        "state": "DELETED",
                    }),
                )
            }
            IntroOperation::SetIntroPostLike(intro_id) => {
                let state = self.likes.like(intro_id, actor).await.map_err(like_status)?;
                json(200, state.to_wire())
            }
            IntroOperation::ClearIntroPostLike(intro_id) => {
                let state = self
                    .likes
                    .unlike(intro_id, actor)
                    .await
                    .map_err(like_status)?;
                json(200, state.to_wire())
            }
            IntroOperation::ListIntroPostLikers(intro_id) => {
                let (limit, cursor) = page_of(&parsed)?;
                let page = self
                    .likes
                    .likers(intro_id, actor, limit, cursor.as_deref())
                    .await
                    .map_err(like_status)?;
                json(200, liker_page_wire(&page))
            }
            IntroOperation::ListIntroPostComments(intro_id) => {
                let (limit, cursor) = page_of(&parsed)?;
                let page = self
                    .comments
                    .list(intro_id, actor, limit, cursor.as_deref())
                    .await
                    .map_err(comment_status)?;
                json(200, comment_page_wire(&page))
            }
            IntroOperation::CreateIntroPostComment(intro_id) => {
                let (body, parent) = parse_comment_body(&request.http_body)?;
                let comment = self
                    .comments
                    .create(intro_id, actor, &body, parent)
                    .await
                    .map_err(comment_status)?;
                json(200, comment_wire(&comment))
            }
            IntroOperation::DeleteIntroPostComment(intro_id, comment_id) => {
                self.comments
                    .delete(intro_id, comment_id, actor)
                    .await
                    .map_err(comment_status)?;
                json(
                    200,
                    serde_json::json!({
                        "resource_id": comment_id,
                        "intro_id": intro_id,
                        "state": "DELETED",
                    }),
                )
            }
            // Refused above; listed so a new operation cannot be added to the
            // enum without this match failing to compile.
            IntroOperation::ListIntroPosts | IntroOperation::GetIntroPost(_) => {
                Err(Status::permission_denied(
                    "This intro operation is not served by the profile transport",
                ))
            }
        }
    }

    /// The author's own posts.
    ///
    /// The contract's `OwnIntroQuery` carries `limit`, `cursor` and `state`.
    /// `IntroPostStore` has no list method, so this reads the author's rows
    /// directly with the same closed projection `read_own` uses. It is the one
    /// query this module writes, and it is deliberately author-scoped: there is
    /// no viewer parameter, so it cannot become a discovery query by accident.
    async fn list_own(
        &self,
        author: Uuid,
        parsed: &Parsed,
    ) -> Result<serde_json::Value, Status> {
        let limit = parsed.query.limit.unwrap_or(20).clamp(1, 50);
        let state = parsed.query.state.as_deref();
        if let Some(state) = state {
            if !matches!(state, "DRAFT" | "PUBLISHED" | "DELETED") {
                return Err(Status::invalid_argument("Invalid intro state"));
            }
        }
        let rows: Vec<Uuid> = sqlx::query_scalar(
            "select intro_id from intro_posts \
             where author_profile_id = $1 \
               and ($2::text is null or state = $2) \
               and ($3::timestamptz is null or updated_at < $3) \
             order by updated_at desc, intro_id desc \
             limit $4",
        )
        .bind(author)
        .bind(state)
        .bind(parsed.query.cursor.as_deref().and_then(decode_cursor))
        .bind(limit)
        .fetch_all(self.posts.pool())
        .await
        .map_err(|_| Status::unavailable("Intro store unavailable"))?;
        let mut items = Vec::with_capacity(rows.len());
        let mut tx = self
            .posts
            .pool()
            .begin()
            .await
            .map_err(|_| Status::unavailable("Intro store unavailable"))?;
        for intro_id in &rows {
            let post = self
                .posts
                .read_own(&mut tx, author, *intro_id)
                .await
                .map_err(intro_status)?;
            items.push(post.to_wire());
        }
        tx.commit()
            .await
            .map_err(|_| Status::unavailable("Intro store unavailable"))?;
        let next_cursor = (rows.len() as i64 == limit)
            .then(|| rows.last().copied())
            .flatten()
            .map(|id| id.to_string());
        Ok(serde_json::json!({ "items": items, "next_cursor": next_cursor }))
    }
}

/// Decode the opaque cursor of the own-post list. The cursor is the last
/// `updated_at` of the previous page, rendered as RFC 3339; a value that is not
/// a timestamp is a client error rather than a silently ignored one.
fn decode_cursor(cursor: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(cursor)
        .ok()
        .map(|value| value.with_timezone(&chrono::Utc))
}

/// Parse `social.SaveIntroPost` into WP-05's `SaveDraft`.
///
/// The schema is closed (`additionalProperties: false`) and every field is
/// required, so an unknown or missing field is `400` rather than a silently
/// defaulted value. The audience is validated by `Audience::new`, which enforces
/// the paired-bounds and ordering rules the database also enforces.
fn parse_save_draft(body: &[u8]) -> Result<SaveDraft, Status> {
    let value: serde_json::Value = serde_json::from_slice(body)
        .map_err(|_| Status::invalid_argument("Invalid intro draft"))?;
    let object = value
        .as_object()
        .ok_or_else(|| Status::invalid_argument("Invalid intro draft"))?;
    const FIELDS: [&str; 8] = [
        "expected_revision",
        "body",
        "favorites_snapshot",
        "currently_seeking",
        "interaction_style",
        "attachment_media_ids",
        "audience",
        "expires_at",
    ];
    if object.len() != FIELDS.len() || FIELDS.iter().any(|field| !object.contains_key(*field)) {
        return Err(Status::invalid_argument("Invalid intro draft"));
    }
    let expected_revision = object["expected_revision"]
        .as_i64()
        .filter(|value| (0..=9007199254740991).contains(value))
        .ok_or_else(|| Status::invalid_argument("Invalid intro draft"))?;
    let content = IntroContent {
        body: object["body"]
            .as_str()
            .ok_or_else(|| Status::invalid_argument("Invalid intro draft"))?
            .to_owned(),
        currently_seeking: optional_text(&object["currently_seeking"])?,
        interaction_style: optional_text(&object["interaction_style"])?,
    };
    let snapshot = parse_snapshot(&object["favorites_snapshot"])?;
    let attachment_media_ids = parse_media_ids(&object["attachment_media_ids"])?;
    let audience = parse_audience(&object["audience"])?;
    let expires_at = match &object["expires_at"] {
        serde_json::Value::Null => None,
        serde_json::Value::String(text) => Some(
            chrono::DateTime::parse_from_rfc3339(text)
                .map_err(|_| Status::invalid_argument("Invalid intro draft"))?
                .with_timezone(&chrono::Utc),
        ),
        _ => return Err(Status::invalid_argument("Invalid intro draft")),
    };
    Ok(SaveDraft {
        expected_revision,
        content,
        favorites_snapshot: snapshot,
        attachment_media_ids,
        audience,
        expires_at,
    })
}

fn optional_text(value: &serde_json::Value) -> Result<Option<String>, Status> {
    match value {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::String(text) if text.chars().count() <= 120 => Ok(Some(text.clone())),
        _ => Err(Status::invalid_argument("Invalid intro draft")),
    }
}

fn parse_snapshot(value: &serde_json::Value) -> Result<Snapshot, Status> {
    let array = value
        .as_array()
        .filter(|items| items.len() <= 15)
        .ok_or_else(|| Status::invalid_argument("Invalid intro draft"))?;
    let mut items = Vec::with_capacity(array.len());
    for entry in array {
        let object = entry
            .as_object()
            .filter(|object| object.len() == 3)
            .ok_or_else(|| Status::invalid_argument("Invalid intro draft"))?;
        let catalog_item_id = object["catalog_item_id"]
            .as_str()
            .and_then(|text| Uuid::parse_str(text).ok())
            .ok_or_else(|| Status::invalid_argument("Invalid intro draft"))?;
        let label = object["label"]
            .as_str()
            .ok_or_else(|| Status::invalid_argument("Invalid intro draft"))?
            .to_owned();
        let category = match object["category"].as_str() {
            Some("music") => "music",
            Some("anime") => "anime",
            Some("game") => "game",
            _ => return Err(Status::invalid_argument("Invalid intro draft")),
        };
        items.push(SnapshotItem {
            catalog_item_id,
            label,
            category,
        });
    }
    Snapshot::from_favorites(items).map_err(|_| Status::invalid_argument("Invalid intro draft"))
}

fn parse_media_ids(value: &serde_json::Value) -> Result<Vec<Uuid>, Status> {
    let array = value
        .as_array()
        .filter(|items| items.len() <= 4)
        .ok_or_else(|| Status::invalid_argument("Invalid intro draft"))?;
    let mut ids = Vec::with_capacity(array.len());
    for entry in array {
        let id = entry
            .as_str()
            .and_then(|text| Uuid::parse_str(text).ok())
            .ok_or_else(|| Status::invalid_argument("Invalid intro draft"))?;
        if ids.contains(&id) {
            // `uniqueItems: true`. A repeated id is a client error, not a
            // silently deduplicated list.
            return Err(Status::invalid_argument("Invalid intro draft"));
        }
        ids.push(id);
    }
    Ok(ids)
}

fn parse_audience(value: &serde_json::Value) -> Result<Audience, Status> {
    let object = value
        .as_object()
        .filter(|object| object.len() == 3)
        .ok_or_else(|| Status::invalid_argument("Invalid intro draft"))?;
    let gender = match object["allowed_gender"].as_str() {
        Some("any") => GenderCondition::Any,
        Some("female") => GenderCondition::Female,
        Some("male") => GenderCondition::Male,
        Some("unknown") => GenderCondition::Unknown,
        _ => return Err(Status::invalid_argument("Invalid intro draft")),
    };
    let minimum = match &object["minimum_age"] {
        serde_json::Value::Null => None,
        value => Some(
            value
                .as_i64()
                .and_then(|age| i16::try_from(age).ok())
                .ok_or_else(|| Status::invalid_argument("Invalid intro draft"))?,
        ),
    };
    let maximum = match &object["maximum_age_exclusive"] {
        serde_json::Value::Null => None,
        value => Some(
            value
                .as_i64()
                .and_then(|age| i16::try_from(age).ok())
                .ok_or_else(|| Status::invalid_argument("Invalid intro draft"))?,
        ),
    };
    Audience::new(gender, minimum, maximum)
        .map_err(|_| Status::invalid_argument("Invalid intro draft"))
}

/// `common.RevisionRequest`: `{"expected_revision": n}` and nothing else.
fn parse_revision(body: &[u8]) -> Result<i64, Status> {
    let value: serde_json::Value = serde_json::from_slice(body)
        .map_err(|_| Status::invalid_argument("Invalid revision request"))?;
    let object = value
        .as_object()
        .filter(|object| object.len() == 1)
        .ok_or_else(|| Status::invalid_argument("Invalid revision request"))?;
    object["expected_revision"]
        .as_i64()
        .filter(|value| (1..=9007199254740991).contains(value))
        .ok_or_else(|| Status::invalid_argument("Invalid revision request"))
}

/// `social.CreateIntroPostComment`: `{"body": "...", "parent_comment_id": ...}`.
fn parse_comment_body(body: &[u8]) -> Result<(String, Option<Uuid>), Status> {
    let value: serde_json::Value = serde_json::from_slice(body)
        .map_err(|_| Status::invalid_argument("Invalid intro comment"))?;
    let object = value
        .as_object()
        .ok_or_else(|| Status::invalid_argument("Invalid intro comment"))?;
    if object.is_empty() || object.len() > 2 || !object.contains_key("body") {
        return Err(Status::invalid_argument("Invalid intro comment"));
    }
    if object.keys().any(|key| !matches!(key.as_str(), "body" | "parent_comment_id")) {
        return Err(Status::invalid_argument("Invalid intro comment"));
    }
    let text = object["body"]
        .as_str()
        .filter(|text| !text.trim().is_empty() && text.chars().count() <= 1000)
        .ok_or_else(|| Status::invalid_argument("Invalid intro comment"))?
        .to_owned();
    let parent = match object.get("parent_comment_id") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(text)) => Some(
            Uuid::parse_str(text).map_err(|_| Status::invalid_argument("Invalid intro comment"))?,
        ),
        Some(_) => return Err(Status::invalid_argument("Invalid intro comment")),
    };
    Ok((text, parent))
}

/// The `limit`/`cursor` pair of a page operation, with WP-05's bounds.
fn page_of(parsed: &Parsed) -> Result<(i64, Option<String>), Status> {
    let limit = parsed.query.limit.unwrap_or(20);
    if !(1..=50).contains(&limit) {
        return Err(Status::invalid_argument("Invalid intro page"));
    }
    Ok((limit, parsed.query.cursor.clone()))
}

fn liker_page_wire(page: &crate::intro_posts::likes::LikerPage) -> serde_json::Value {
    serde_json::json!({
        "items": page.items.iter().map(|liker| serde_json::json!({
            "profile_id": liker.profile_id,
            "nickname": liker.nickname,
            "profile_image_media_id": liker.profile_image_media_id,
            "shared_items": liker.shared_with_author.iter().map(|item| serde_json::json!({
                "catalog_item_id": item.catalog_item_id,
                "label": item.label,
            })).collect::<Vec<_>>(),
            "interaction_style": liker.interaction_style,
            "liked_at": liker.liked_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        })).collect::<Vec<_>>(),
        "next_cursor": page.next_cursor,
    })
}

fn comment_page_wire(page: &crate::intro_posts::comments::IntroCommentPage) -> serde_json::Value {
    serde_json::json!({
        "items": page.items.iter().map(comment_wire).collect::<Vec<_>>(),
        "next_cursor": page.next_cursor,
    })
}

fn comment_wire(comment: &crate::intro_posts::comments::IntroComment) -> serde_json::Value {
    serde_json::json!({
        "comment_id": comment.comment_id,
        "intro_id": comment.intro_id,
        "author_profile_id": comment.author_profile_id,
        "author_nickname": comment.author_nickname,
        "author_profile_image_media_id": comment.author_profile_image_media_id,
        "parent_comment_id": comment.parent_comment_id,
        "body": comment.body,
        "created_at": comment.created_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "deleted": comment.deleted,
    })
}

fn json(
    status: u32,
    body: serde_json::Value,
) -> Result<super::scoped::ProfileRepresentation, Status> {
    Ok(super::scoped::ProfileRepresentation {
        status,
        content_type: "application/json".into(),
        body: serde_json::to_vec(&body).map_err(|_| Status::internal("Intro projection"))?,
    })
}

fn intro_status(error: IntroCommandError) -> Status {
    match error {
        IntroCommandError::InvalidContent(_) | IntroCommandError::InvalidAudience(_) => {
            Status::invalid_argument("Invalid intro content")
        }
        IntroCommandError::VersionConflict => Status::already_exists("Version conflict"),
        IntroCommandError::InvalidTransition => {
            Status::failed_precondition("Invalid intro state transition")
        }
        IntroCommandError::Unavailable | IntroCommandError::Unavailable2 => {
            Status::not_found("Intro unavailable")
        }
        _ => Status::unavailable("Intro store unavailable"),
    }
}

/// Map a like error to a closed gRPC status.
///
/// WP-05's own `LikeError::status()` is the authority for the HTTP projection,
/// so this maps from that value rather than re-deciding it. The one message that
/// is not fixed by the status is the self-like case, which the client can act on.
fn like_status(error: LikeError) -> Status {
    match error.status() {
        400 => Status::invalid_argument("You cannot like your own introduction"),
        403 => Status::permission_denied("Intro likers are author-only"),
        404 => Status::not_found("Intro unavailable"),
        _ => Status::unavailable("Intro store unavailable"),
    }
}

/// Map a comment error to a closed gRPC status, from WP-05's own projection.
fn comment_status(error: CommentError) -> Status {
    match error.status() {
        400 => Status::invalid_argument("Invalid intro comment"),
        404 => Status::not_found("Comment unavailable"),
        _ => Status::unavailable("Intro store unavailable"),
    }
}
