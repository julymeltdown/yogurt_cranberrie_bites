//! BE32 parent-protected comment projection and BE31/32 command preparation.
use super::*;
use crate::commands::{CanonicalRequest, CanonicalValue, RegisteredOperation, Scope};
use sha2::{Digest, Sha256};

#[async_trait]
pub trait CommentReader: Send + Sync {
    async fn comment(&self, context: Context, id: Uuid) -> Result<Option<Comment>, FeedError>;
    async fn comments(
        &self,
        context: Context,
        parent: Uuid,
        window: Window,
        limit: u16,
    ) -> Result<ScannedComments, FeedError>;
}
pub struct ProjectedComment {
    pub comment: Comment,
    pub author: PublicAuthor,
}
pub struct CommentPage {
    pub items: Vec<ProjectedComment>,
    pub next: Option<Position>,
}
pub struct Thread {
    pub post: ProjectedPost,
    pub comments: CommentPage,
}
impl<R: PostReader + CommentReader, P: FeedPolicy, S: FeedResources, C: crate::Clock>
    Feed<R, P, S, C>
{
    /// The comment route always resolves and reauthorizes the real parent, including
    /// direct comment-ID routes whose URL contains no post ID.
    pub async fn comments(
        &self,
        context: Context,
        parent: Uuid,
        window: Window,
        limit: u16,
    ) -> Result<CommentPage, FeedError> {
        if !(1..=100).contains(&limit) || window.upper > self.clock.now() {
            return Err(FeedError::InvalidInput);
        }
        let before = self.post(context, parent).await?;
        let batch = self.reader.comments(context, parent, window, limit).await?;
        let rows = batch.items;
        if rows.len() > usize::from(limit)
            || rows.windows(2).any(|w| w[0].position() <= w[1].position())
            || rows.iter().any(|c| {
                c.event != context.event || c.post != parent || !window.contains(c.position())
            })
        {
            return Err(FeedError::DependencyUnavailable);
        }
        let positions = rows.iter().map(Comment::position).collect::<Vec<_>>();
        let next = continuation(window, &positions, batch.scanned_through, batch.has_more)?;
        let mut items = Vec::new();
        let mut fences = Vec::new();
        for comment in rows {
            if comment.state != ContentState::Published {
                continue;
            }
            comment
                .validate()
                .map_err(|_| FeedError::DependencyUnavailable)?;
            let observed = match self.observe(context, comment.author, Action::Read).await {
                Ok(v) => v,
                Err(FeedError::PermissionDenied | FeedError::ResourceUnavailable) => continue,
                Err(e) => return Err(e),
            };
            let author = self.resources.author(context, comment.author).await?;
            if author.user_id != comment.author {
                return Err(FeedError::DependencyUnavailable);
            }
            fences.push(observed);
            items.push(ProjectedComment { comment, author });
        }
        for item in &items {
            let current = self
                .reader
                .comment(context, item.comment.id)
                .await?
                .ok_or(FeedError::ResourceUnavailable)?;
            if current != item.comment {
                return Err(FeedError::ResourceUnavailable);
            }
        }
        let after = self.post(context, parent).await?;
        if after.post.revision != before.post.revision {
            return Err(FeedError::ResourceUnavailable);
        }
        for f in fences {
            self.fence(f).await?;
        }
        Ok(CommentPage { items, next })
    }
    pub async fn thread(
        &self,
        context: Context,
        parent: Uuid,
        window: Window,
        limit: u16,
    ) -> Result<Thread, FeedError> {
        let comments = self.comments(context, parent, window, limit).await?;
        let post = self.post(context, parent).await?;
        Ok(Thread { post, comments })
    }
    pub async fn prepare(
        &self,
        context: Context,
        key: Uuid,
        command: FeedCommand,
    ) -> Result<PreparedCommand, FeedError> {
        context.check(self.clock.now())?;
        command.validate()?;
        if key.is_nil() {
            return Err(FeedError::InvalidInput);
        }
        let mut subject = context.actor;
        let mut parent = None;
        let mut parent_revision = None;
        match &command {
            FeedCommand::CreatePost { content } => self.validate_content(context, content).await?,
            FeedCommand::EditPost {
                post,
                expected_revision,
                content,
            } => {
                let current = self.post(context, *post).await?.post;
                require_owner(
                    context,
                    current.author,
                    current.revision,
                    *expected_revision,
                )?;
                self.validate_content(context, content).await?;
            }
            FeedCommand::DeletePost {
                post,
                expected_revision,
            } => {
                let current = self.post(context, *post).await?.post;
                require_owner(
                    context,
                    current.author,
                    current.revision,
                    *expected_revision,
                )?;
            }
            FeedCommand::CreateComment {
                post: _,
                post_revision,
                ..
            } => {
                let current = self.post(context, *post).await?.post;
                if current.revision != *post_revision {
                    return Err(FeedError::VersionConflict);
                }
                subject = current.author;
                parent = Some(current.id);
                parent_revision = Some(current.revision);
            }
            FeedCommand::EditComment {
                comment,
                expected_revision,
                ..
            }
            | FeedCommand::DeleteComment {
                comment,
                expected_revision,
            } => {
                let current = self
                    .reader
                    .comment(context, *comment)
                    .await?
                    .ok_or(FeedError::ResourceUnavailable)?;
                if current.id != *comment
                    || current.event != context.event
                    || current.state != ContentState::Published
                {
                    return Err(FeedError::ResourceUnavailable);
                }
                current
                    .validate()
                    .map_err(|_| FeedError::DependencyUnavailable)?;
                let post = self.post(context, current.post).await?.post;
                parent = Some(post.id);
                parent_revision = Some(post.revision);
                require_owner(
                    context,
                    current.author,
                    current.revision,
                    *expected_revision,
                )?;
            }
            FeedCommand::SetReaction { post, .. } => {
                let current = self.post(context, *post).await?.post;
                subject = current.author;
                parent = Some(current.id);
                parent_revision = Some(current.revision);
            }
        }
        let action = if command.is_safety() {
            Action::SafetySelf
        } else {
            Action::Write
        };
        let observed = self.observe(context, subject, action).await?;
        self.fence(observed).await?;
        PreparedCommand::new(context, key, command, parent, parent_revision)
    }
    async fn validate_content(&self, context: Context, content: &Content) -> Result<(), FeedError> {
        for link in &content.links {
            if !self.resources.public_link(context, *link).await? {
                return Err(FeedError::ResourceUnavailable);
            }
        }
        for media in &content.media_ids {
            if !self
                .resources
                .public_media(context, context.actor, *media)
                .await?
            {
                return Err(FeedError::MediaNotReady);
            }
        }
        Ok(())
    }
}
fn require_owner(c: Context, author: Uuid, actual: i64, expected: i64) -> Result<(), FeedError> {
    if c.actor != author {
        return Err(FeedError::PermissionDenied);
    }
    if actual != expected {
        return Err(FeedError::VersionConflict);
    }
    Ok(())
}
/// Validated intent only, NOT a grant. Any sink must recheck current phase, policy,
/// content, media/linked visibility and CAS inside its qualified commit fence.
/// Fields are private to prevent a caller substituting command/scope after hashing.
pub struct PreparedCommand {
    context: Context,
    key: Uuid,
    target: Uuid,
    command: FeedCommand,
    parent: Option<Uuid>,
    parent_revision: Option<i64>,
}
impl PreparedCommand {
    fn new(
        context: Context,
        key: Uuid,
        command: FeedCommand,
        parent: Option<Uuid>,
        parent_revision: Option<i64>,
    ) -> Result<Self, FeedError> {
        let target = match command {
            FeedCommand::CreatePost { .. } | FeedCommand::CreateComment { .. } => {
                let mut hash = Sha256::new();
                hash.update(b"mcr/feed/target/v1");
                for id in [context.actor, context.event, key] {
                    hash.update(id.as_bytes());
                }
                hash.update(command.operation().as_bytes());
                let digest = hash.finalize();
                let mut bytes = [0; 16];
                bytes.copy_from_slice(&digest[..16]);
                bytes[6] = (bytes[6] & 0x0f) | 0x80;
                bytes[8] = (bytes[8] & 0x3f) | 0x80;
                Uuid::from_bytes(bytes)
            }
            FeedCommand::EditPost { post, .. }
            | FeedCommand::DeletePost { post, .. }
            | FeedCommand::SetReaction { post, .. } => post,
            FeedCommand::EditComment { comment, .. }
            | FeedCommand::DeleteComment { comment, .. } => comment,
        };
        Ok(Self {
            context,
            key,
            target,
            command,
            parent,
            parent_revision,
        })
    }
    pub fn parent(&self) -> Option<Uuid> {
        self.parent
    }
    pub fn parent_revision(&self) -> Option<i64> {
        self.parent_revision
    }
    pub fn context(&self) -> Context {
        self.context
    }
    pub fn target(&self) -> Uuid {
        self.target
    }
    pub fn command(&self) -> &FeedCommand {
        &self.command
    }
    pub fn scope(&self) -> Result<Scope, FeedError> {
        Ok(Scope {
            actor: self.context.actor,
            event: self.context.event,
            key: self.key,
            operation: RegisteredOperation::find(self.command.operation())
                .map_err(|_| FeedError::DependencyUnavailable)?,
        })
    }
    pub fn canonical(&self) -> Result<CanonicalRequest, FeedError> {
        let scope = self.scope()?;
        let event = scope.event.to_string();
        let target = self.target.to_string();
        let mut params = vec![("eventId", event.as_str())];
        let mut query = Vec::new();
        let mut revision_value = String::new();
        let body = match &self.command {
            FeedCommand::CreatePost { content } => Some(content_value(content)),
            FeedCommand::EditPost {
                expected_revision,
                content,
                ..
            } => {
                params.push(("postId", &target));
                let mut values = content_fields(content);
                values.push((
                    "expected_revision".into(),
                    CanonicalValue::Integer(*expected_revision),
                ));
                Some(CanonicalValue::Object(values))
            }
            FeedCommand::DeletePost {
                expected_revision, ..
            } => {
                params.push(("postId", &target));
                revision_value.push_str(&expected_revision.to_string());
                query.push(("expected_revision", revision_value.as_str()));
                None
            }
            FeedCommand::CreateComment {
                post: _,
                post_revision,
                text,
            } => {
                params.push(("postId", "")); // replaced below with the actual parent, not the new comment UUID
                Some(CanonicalValue::Object(vec![
                    ("text".into(), CanonicalValue::String(text.clone())),
                    (
                        "post_revision".into(),
                        CanonicalValue::Integer(*post_revision),
                    ),
                ]))
            }
            FeedCommand::EditComment {
                expected_revision,
                text,
                ..
            } => {
                params.push(("commentId", &target));
                Some(CanonicalValue::Object(vec![
                    ("text".into(), CanonicalValue::String(text.clone())),
                    (
                        "expected_revision".into(),
                        CanonicalValue::Integer(*expected_revision),
                    ),
                ]))
            }
            FeedCommand::DeleteComment {
                expected_revision, ..
            } => {
                params.push(("commentId", &target));
                revision_value.push_str(&expected_revision.to_string());
                query.push(("expected_revision", revision_value.as_str()));
                None
            }
            FeedCommand::SetReaction { present, .. } => {
                params.push(("postId", &target));
                if *present {
                    Some(CanonicalValue::Object(vec![]))
                } else {
                    None
                }
            }
        };
        let parent = match &self.command {
            FeedCommand::CreateComment { post, .. } => Some(post.to_string()),
            _ => None,
        };
        if let Some(parent) = &parent {
            params[1] = ("postId", parent.as_str());
        }
        CanonicalRequest::json(scope, &params, &query, body.as_ref())
            .map_err(|_| FeedError::InvalidInput)
    }
}
fn content_fields(c: &Content) -> Vec<(String, CanonicalValue)> {
    vec![
        ("text".into(), CanonicalValue::String(c.text.clone())),
        (
            "media_ids".into(),
            CanonicalValue::Array(
                c.media_ids
                    .iter()
                    .map(|v| CanonicalValue::String(v.to_string()))
                    .collect(),
            ),
        ),
        (
            "links".into(),
            CanonicalValue::Array(
                c.links
                    .iter()
                    .map(|v| {
                        CanonicalValue::Object(vec![
                            (
                                "target_type".into(),
                                CanonicalValue::String(
                                    match v.target_type {
                                        TargetType::Party => "PARTY",
                                        TargetType::Giveaway => "GIVEAWAY",
                                        TargetType::Post => "POST",
                                        TargetType::Intro => "INTRO",
                                        TargetType::Mural => "MURAL",
                                        TargetType::Notice => "NOTICE",
                                    }
                                    .into(),
                                ),
                            ),
                            (
                                "target_id".into(),
                                CanonicalValue::String(v.target_id.to_string()),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
    ]
}
fn content_value(c: &Content) -> CanonicalValue {
    CanonicalValue::Object(content_fields(c))
}
#[async_trait]
impl CommentReader for UnavailableFeed {
    async fn comment(&self, _: Context, _: Uuid) -> Result<Option<Comment>, FeedError> {
        Err(FeedError::DependencyUnavailable)
    }
    async fn comments(
        &self,
        _: Context,
        _: Uuid,
        _: Window,
        _: u16,
    ) -> Result<ScannedComments, FeedError> {
        Err(FeedError::DependencyUnavailable)
    }
}
