//! Typed Post-owner adapter pinned to source e975500. Root composition supplies a
//! qualified authenticated transport; no raw Channel or actor-forwarding shortcut.
use super::queue::OwnerCommit;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{
    commands::{ReceiptState, SafeReceipt},
    feed::*,
};
use concert_domain::feed::*;
use tonic::{Code, Status};
use uuid::Uuid;
pub mod proto {
    tonic::include_proto!("post.concert.v1");
}

/// Trusted transport seam. Implementations must authenticate Post, apply qualified
/// workload/delegated-actor metadata and preserve the original absolute deadline.
/// Production uses qualified_transport::QualifiedPostTransport and its dedicated
/// whole-envelope signer plus actual retained canonical scope views.
#[async_trait]
pub trait PostTransport: Send + Sync {
    async fn post(&self, _: proto::GetPostRequest) -> Result<proto::GetPostResponse, Status> {
        Err(Status::unavailable("Post authority unavailable"))
    }
    async fn posts(&self, _: proto::ListPostsRequest) -> Result<proto::ListPostsResponse, Status> {
        Err(Status::unavailable("Post authority unavailable"))
    }
    async fn comment(
        &self,
        _: proto::GetCommentRequest,
    ) -> Result<proto::GetCommentResponse, Status> {
        Err(Status::unavailable("Post authority unavailable"))
    }
    async fn comments(
        &self,
        _: proto::ListCommentsRequest,
    ) -> Result<proto::ListCommentsResponse, Status> {
        Err(Status::unavailable("Post authority unavailable"))
    }
    async fn execute(&self, _: proto::ExecuteCommandRequest) -> Result<proto::Receipt, Status> {
        Err(Status::unavailable("Post authority unavailable"))
    }
    async fn receipt(
        &self,
        _: proto::GetReceiptRequest,
    ) -> Result<proto::GetReceiptResponse, Status> {
        Err(Status::unavailable("Post authority unavailable"))
    }
}
pub struct UnavailablePostTransport;
#[async_trait]
impl PostTransport for UnavailablePostTransport {}
pub struct PostOwner<T> {
    pub transport: T,
}
fn wire_context(c: Context) -> Result<proto::Context, FeedError> {
    c.check(Utc::now())?;
    Ok(proto::Context {
        actor_id: c.actor.to_string(),
        event_id: c.event.to_string(),
        session_family_id: c.session_family.to_string(),
        request_id: c.request.to_string(),
        original_authority_digest: c.authority_digest.to_vec(),
        expires_at: c.expires_at.to_rfc3339(),
    })
}
fn position(p: Position) -> proto::Position {
    proto::Position {
        created_at: p.created_at.to_rfc3339(),
        id: p.id.to_string(),
    }
}
fn parse_position(p: proto::Position) -> Result<Position, FeedError> {
    Ok(Position {
        created_at: time(&p.created_at)?,
        id: id(&p.id)?,
    })
}
fn id(s: &str) -> Result<Uuid, FeedError> {
    Uuid::parse_str(s)
        .ok()
        .filter(|v| !v.is_nil() && v.to_string() == s)
        .ok_or(FeedError::DependencyUnavailable)
}
fn time(s: &str) -> Result<DateTime<Utc>, FeedError> {
    DateTime::parse_from_rfc3339(s)
        .map(|v| v.with_timezone(&Utc))
        .map_err(|_| FeedError::DependencyUnavailable)
}
fn state(s: &str) -> Result<ContentState, FeedError> {
    match s {
        "PUBLISHED" => Ok(ContentState::Published),
        "HIDDEN" => Ok(ContentState::Hidden),
        "DELETED" => Ok(ContentState::Deleted),
        _ => Err(FeedError::DependencyUnavailable),
    }
}
fn link(l: proto::Link) -> Result<ResourceLink, FeedError> {
    Ok(ResourceLink {
        target_type: match l.target_type.as_str() {
            "PARTY" => TargetType::Party,
            "GIVEAWAY" => TargetType::Giveaway,
            "POST" => TargetType::Post,
            "INTRO" => TargetType::Intro,
            "MURAL" => TargetType::Mural,
            "NOTICE" => TargetType::Notice,
            _ => return Err(FeedError::DependencyUnavailable),
        },
        target_id: id(&l.target_id)?,
    })
}
fn content(c: proto::Content) -> Result<Content, FeedError> {
    let value = Content {
        text: c.text,
        media_ids: c
            .media_ids
            .iter()
            .map(|v| id(v))
            .collect::<Result<_, _>>()?,
        links: c.links.into_iter().map(link).collect::<Result<_, _>>()?,
    };
    value
        .validate()
        .map_err(|_| FeedError::DependencyUnavailable)?;
    Ok(value)
}
fn post(c: Context, p: proto::PostRecord) -> Result<Post, FeedError> {
    let value = Post {
        id: id(&p.post_id)?,
        event: id(&p.event_id)?,
        author: id(&p.author_id)?,
        content: content(p.content.ok_or(FeedError::DependencyUnavailable)?)?,
        state: state(&p.state)?,
        public: p.is_public,
        revision: p.revision,
        created_at: time(&p.created_at)?,
        edited_at: p.edited_at.as_deref().map(time).transpose()?,
        comment_count: p
            .comment_count
            .try_into()
            .map_err(|_| FeedError::DependencyUnavailable)?,
        reaction_count: p
            .reaction_count
            .try_into()
            .map_err(|_| FeedError::DependencyUnavailable)?,
        reacted_by_me: p.reacted_by_me,
    };
    value
        .validate()
        .map_err(|_| FeedError::DependencyUnavailable)?;
    if value.event != c.event {
        return Err(FeedError::DependencyUnavailable);
    }
    Ok(value)
}
fn comment(c: Context, p: proto::CommentRecord) -> Result<Comment, FeedError> {
    let value = Comment {
        id: id(&p.comment_id)?,
        event: id(&p.event_id)?,
        post: id(&p.post_id)?,
        author: id(&p.author_id)?,
        text: p.text,
        state: state(&p.state)?,
        revision: p.revision,
        created_at: time(&p.created_at)?,
    };
    value
        .validate()
        .map_err(|_| FeedError::DependencyUnavailable)?;
    if value.event != c.event {
        return Err(FeedError::DependencyUnavailable);
    }
    Ok(value)
}
fn failure(s: Status) -> FeedError {
    match s.code() {
        Code::NotFound | Code::PermissionDenied => FeedError::ResourceUnavailable,
        Code::Aborted | Code::AlreadyExists => FeedError::VersionConflict,
        _ => FeedError::DependencyUnavailable,
    }
}
#[async_trait]
impl<T: PostTransport> PostReader for PostOwner<T> {
    async fn post(&self, c: Context, n: Uuid) -> Result<Option<Post>, FeedError> {
        let response = self
            .transport
            .post(proto::GetPostRequest {
                context: Some(wire_context(c)?),
                post_id: n.to_string(),
            })
            .await;
        c.check(Utc::now())?;
        match response {
            Ok(r) => r.post.map(|p| post(c, p)).transpose().and_then(|p| {
                if p.as_ref().is_some_and(|p| p.id != n) {
                    Err(FeedError::DependencyUnavailable)
                } else {
                    Ok(p)
                }
            }),
            Err(s) if s.code() == Code::NotFound => Ok(None),
            Err(s) => Err(failure(s)),
        }
    }
    async fn page(
        &self,
        c: Context,
        w: Window,
        f: Filter,
        limit: u16,
    ) -> Result<ScannedPosts, FeedError> {
        let response = self
            .transport
            .posts(proto::ListPostsRequest {
                context: Some(wire_context(c)?),
                upper: w.upper.to_rfc3339(),
                after: w.after.map(position),
                filter: match f {
                    Filter::All => "ALL",
                    Filter::WithMedia => "WITH_MEDIA",
                    Filter::PartyLink => "PARTY_LINK",
                    Filter::GiveawayLink => "GIVEAWAY_LINK",
                }
                .into(),
                limit: limit.into(),
            })
            .await;
        c.check(Utc::now())?;
        let r = response.map_err(failure)?;
        Ok(ScannedPosts {
            items: r
                .posts
                .into_iter()
                .map(|p| post(c, p))
                .collect::<Result<_, _>>()?,
            scanned_through: r.scanned_through.map(parse_position).transpose()?,
            has_more: r.has_more,
        })
    }
}
#[async_trait]
impl<T: PostTransport> CommentReader for PostOwner<T> {
    async fn comment(&self, c: Context, n: Uuid) -> Result<Option<Comment>, FeedError> {
        let response = self
            .transport
            .comment(proto::GetCommentRequest {
                context: Some(wire_context(c)?),
                comment_id: n.to_string(),
            })
            .await;
        c.check(Utc::now())?;
        match response {
            Ok(r) => r.comment.map(|p| comment(c, p)).transpose().and_then(|p| {
                if p.as_ref().is_some_and(|p| p.id != n) {
                    Err(FeedError::DependencyUnavailable)
                } else {
                    Ok(p)
                }
            }),
            Err(s) if s.code() == Code::NotFound => Ok(None),
            Err(s) => Err(failure(s)),
        }
    }
    async fn comments(
        &self,
        c: Context,
        parent: Uuid,
        w: Window,
        limit: u16,
    ) -> Result<ScannedComments, FeedError> {
        let response = self
            .transport
            .comments(proto::ListCommentsRequest {
                context: Some(wire_context(c)?),
                post_id: parent.to_string(),
                upper: w.upper.to_rfc3339(),
                after: w.after.map(position),
                limit: limit.into(),
            })
            .await;
        c.check(Utc::now())?;
        let r = response.map_err(failure)?;
        let items = r
            .comments
            .into_iter()
            .map(|p| comment(c, p))
            .collect::<Result<Vec<_>, _>>()?;
        if items.iter().any(|p| p.post != parent) {
            return Err(FeedError::DependencyUnavailable);
        }
        Ok(ScannedComments {
            items,
            scanned_through: r.scanned_through.map(parse_position).transpose()?,
            has_more: r.has_more,
        })
    }
}
fn wire_content(c: &Content) -> proto::Content {
    proto::Content {
        text: c.text.clone(),
        media_ids: c.media_ids.iter().map(ToString::to_string).collect(),
        links: c
            .links
            .iter()
            .map(|v| proto::Link {
                target_type: match v.target_type {
                    TargetType::Party => "PARTY",
                    TargetType::Giveaway => "GIVEAWAY",
                    TargetType::Post => "POST",
                    TargetType::Intro => "INTRO",
                    TargetType::Mural => "MURAL",
                    TargetType::Notice => "NOTICE",
                }
                .into(),
                target_id: v.target_id.to_string(),
            })
            .collect(),
    }
}
fn wire_command(p: &PreparedCommand, r: &SafeReceipt) -> Result<proto::Command, FeedError> {
    use proto::command::Mutation;
    if p.scope()? != r.scope
        || !matches!(r.state, ReceiptState::Pending | ReceiptState::Succeeded)
        || r.id.is_nil()
    {
        return Err(FeedError::PermissionDenied);
    }
    let mut post_id = p.target();
    let mutation = match p.command() {
        FeedCommand::CreatePost { content } => Mutation::CreatePost(proto::CreatePost {
            content: Some(wire_content(content)),
        }),
        FeedCommand::EditPost {
            content,
            expected_revision,
            ..
        } => Mutation::EditPost(proto::EditPost {
            content: Some(wire_content(content)),
            expected_revision: *expected_revision,
        }),
        FeedCommand::DeletePost {
            expected_revision, ..
        } => Mutation::DeletePost(proto::DeletePost {
            expected_revision: *expected_revision,
        }),
        FeedCommand::CreateComment {
            post,
            post_revision,
            text,
        } => {
            post_id = *post;
            Mutation::CreateComment(proto::CreateComment {
                comment_id: p.target().to_string(),
                post_revision: *post_revision,
                text: text.clone(),
            })
        }
        FeedCommand::EditComment {
            expected_revision,
            text,
            ..
        } => {
            post_id = p.parent().ok_or(FeedError::DependencyUnavailable)?;
            Mutation::EditComment(proto::EditComment {
                comment_id: p.target().to_string(),
                expected_revision: *expected_revision,
                text: text.clone(),
            })
        }
        FeedCommand::DeleteComment {
            expected_revision, ..
        } => {
            post_id = p.parent().ok_or(FeedError::DependencyUnavailable)?;
            Mutation::DeleteComment(proto::DeleteComment {
                comment_id: p.target().to_string(),
                expected_revision: *expected_revision,
            })
        }
        FeedCommand::SetReaction { present, .. } => Mutation::SetReaction(proto::SetReaction {
            expected_revision: p
                .parent_revision()
                .ok_or(FeedError::DependencyUnavailable)?,
            active: *present,
        }),
    };
    // Post's internal operation UUID is BE15 receipt identity, distinct from public
    // operation name. The name remains in receipt scope and canonical fingerprint.
    Ok(proto::Command {
        operation_id: r.id.to_string(),
        actor_id: r.scope.actor.to_string(),
        event_id: r.scope.event.to_string(),
        post_id: post_id.to_string(),
        idempotency_key: r.scope.key.to_string(),
        mutation: Some(mutation),
    })
}
fn committed(
    p: &PreparedCommand,
    r: &SafeReceipt,
    result: proto::Receipt,
) -> Result<OwnerCommit, FeedError> {
    let expected = wire_command(p, r)?;
    if result.operation_id != expected.operation_id
        || result.event_id != expected.event_id
        || result.actor_id != expected.actor_id
        || result.idempotency_key != expected.idempotency_key
        || result.post_id != expected.post_id
    {
        return Err(FeedError::DependencyUnavailable);
    }
    let is_comment = matches!(
        p.command(),
        FeedCommand::CreateComment { .. }
            | FeedCommand::EditComment { .. }
            | FeedCommand::DeleteComment { .. }
    );
    if is_comment {
        if result.comment_id.as_deref() != Some(p.target().to_string().as_str()) {
            return Err(FeedError::DependencyUnavailable);
        }
    } else if result.comment_id.is_some() {
        return Err(FeedError::DependencyUnavailable);
    }
    revision(result.revision).map_err(|_| FeedError::DependencyUnavailable)?;
    revision(result.post_revision).map_err(|_| FeedError::DependencyUnavailable)?;
    let at = time(&result.committed_at)?;
    if at > Utc::now() || result.reaction_count < 0 || result.reaction_count > i64::from(i32::MAX) {
        return Err(FeedError::DependencyUnavailable);
    }
    let deleted = matches!(
        p.command(),
        FeedCommand::DeletePost { .. } | FeedCommand::DeleteComment { .. }
    );
    if result.deleted != deleted {
        return Err(FeedError::DependencyUnavailable);
    }
    if let FeedCommand::SetReaction { present, .. } = p.command() {
        if result.reacted_by_me != *present {
            return Err(FeedError::DependencyUnavailable);
        }
    }
    Ok(OwnerCommit {
        scope: r.scope,
        resource: p.target(),
        revision: result.revision,
    })
}
impl<T: PostTransport> PostOwner<T> {
    pub async fn execute(
        &self,
        p: &PreparedCommand,
        r: &SafeReceipt,
    ) -> Result<OwnerCommit, FeedError> {
        let response = self
            .transport
            .execute(proto::ExecuteCommandRequest {
                context: Some(wire_context(p.context())?),
                command: Some(wire_command(p, r)?),
            })
            .await;
        p.context().check(Utc::now())?;
        committed(p, r, response.map_err(failure)?)
    }
    pub async fn receipt(
        &self,
        p: &PreparedCommand,
        r: &SafeReceipt,
    ) -> Result<Option<OwnerCommit>, FeedError> {
        let response = self
            .transport
            .receipt(proto::GetReceiptRequest {
                context: Some(wire_context(p.context())?),
                command: Some(wire_command(p, r)?),
            })
            .await;
        p.context().check(Utc::now())?;
        response
            .map_err(failure)?
            .receipt
            .map(|v| committed(p, r, v))
            .transpose()
    }
}
