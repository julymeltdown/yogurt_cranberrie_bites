//! BE31 event-scoped adapter. Post remains the only content owner.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_domain::feed::*;
use uuid::Uuid;

/// Trusted boundary binding, never deserialize an actor from the content body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Context {
    pub actor: Uuid,
    pub event: Uuid,
    pub session_family: Uuid,
    pub request: Uuid,
    pub expires_at: DateTime<Utc>,
    pub authority_digest: [u8; 32],
}
impl Context {
    pub fn check(self, now: DateTime<Utc>) -> Result<(), FeedError> {
        if [self.actor, self.event, self.session_family, self.request]
            .iter()
            .any(Uuid::is_nil)
            || self.authority_digest == [0; 32]
            || now >= self.expires_at
        {
            return Err(FeedError::PermissionDenied);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Read,
    Write,
    SafetySelf,
}
/// A bounded observation, never a transaction commit grant. Final fences must fetch
/// current local/owner revisions, not echo this value. Do not collapse revision vectors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Observation {
    pub context: Context,
    pub subject: Uuid,
    pub action: Action,
    pub event_revision: i64,
    pub relationship_revision: i64,
    pub auth_actor_revision: i64,
    pub auth_subject_revision: i64,
    pub friend_revision: i64,
    pub valid_until: DateTime<Utc>,
}
impl Observation {
    pub fn check(
        self,
        context: Context,
        subject: Uuid,
        action: Action,
        now: DateTime<Utc>,
    ) -> Result<(), FeedError> {
        context.check(now)?;
        if self.context != context
            || self.subject != subject
            || self.action != action
            || now >= self.valid_until
            || self.valid_until > context.expires_at
            || [
                self.event_revision,
                self.relationship_revision,
                self.auth_actor_revision,
                self.auth_subject_revision,
                self.friend_revision,
            ]
            .iter()
            .any(|v| revision(*v).is_err())
        {
            return Err(FeedError::PermissionDenied);
        }
        Ok(())
    }
}
#[async_trait]
pub trait FeedPolicy: Send + Sync {
    async fn observe(
        &self,
        context: Context,
        subject: Uuid,
        action: Action,
    ) -> Result<Observation, FeedError>;
    /// Re-query current authority and compare every revision; called after enrichment.
    async fn fence(&self, observation: Observation) -> Result<(), FeedError>;
}
/// Exact owner content snapshot. Results must be current and viewer-authorized. No
/// user feed/search fallback; all rows bind event and upper/last created_at,id tuple.
pub struct ScannedPosts {
    pub items: Vec<Post>,
    pub scanned_through: Option<Position>,
    pub has_more: bool,
}
pub struct ScannedComments {
    pub items: Vec<Comment>,
    pub scanned_through: Option<Position>,
    pub has_more: bool,
}
pub(crate) fn continuation(
    window: Window,
    positions: &[Position],
    scanned: Option<Position>,
    more: bool,
) -> Result<Option<Position>, FeedError> {
    if more && scanned.is_none()
        || scanned.is_some_and(|p| !window.contains(p))
        || positions
            .last()
            .is_some_and(|last| scanned.is_none_or(|p| p > *last))
    {
        return Err(FeedError::DependencyUnavailable);
    }
    Ok(if more { scanned } else { None })
}
#[async_trait]
pub trait PostReader: Send + Sync {
    async fn page(
        &self,
        context: Context,
        window: Window,
        filter: Filter,
        limit: u16,
    ) -> Result<ScannedPosts, FeedError>;
    async fn post(&self, context: Context, id: Uuid) -> Result<Option<Post>, FeedError>;
}
/// Public profile bytes must match frozen C03 PublicProfile and exact user_id.
/// The service schema boundary validates the allowlist; no author fallback is invented.
#[derive(Clone)]
pub struct PublicAuthor {
    pub user_id: Uuid,
    pub json: Vec<u8>,
}
#[async_trait]
pub trait FeedResources: Send + Sync {
    async fn author(&self, context: Context, user: Uuid) -> Result<PublicAuthor, FeedError>;
    /// Must query actual target owner, current event/audience/membership/moderation.
    /// Public means safe for the permitted event audience, not just a viewer's private grant.
    async fn public_link(&self, context: Context, link: ResourceLink) -> Result<bool, FeedError>;
    /// READY, owned by author, POST purpose, currently publishable. No URL or fake upload.
    async fn public_media(
        &self,
        context: Context,
        author: Uuid,
        media: Uuid,
    ) -> Result<bool, FeedError>;
}
pub struct ProjectedPost {
    pub post: Post,
    pub author: PublicAuthor,
}
pub struct FeedPage {
    pub items: Vec<ProjectedPost>,
    pub next: Option<Position>,
}

pub struct Feed<R, P, S, C> {
    pub reader: R,
    pub policy: P,
    pub resources: S,
    pub clock: C,
}
impl<R: PostReader, P: FeedPolicy, S: FeedResources, C: crate::Clock> Feed<R, P, S, C> {
    pub(crate) async fn observe(
        &self,
        context: Context,
        subject: Uuid,
        action: Action,
    ) -> Result<Observation, FeedError> {
        context.check(self.clock.now())?;
        let observation = self.policy.observe(context, subject, action).await?;
        observation.check(context, subject, action, self.clock.now())?;
        Ok(observation)
    }
    pub(crate) async fn fence(&self, observation: Observation) -> Result<(), FeedError> {
        self.policy.fence(observation).await?;
        observation.check(
            observation.context,
            observation.subject,
            observation.action,
            self.clock.now(),
        )
    }
    async fn project(
        &self,
        context: Context,
        mut post: Post,
    ) -> Result<Option<ProjectedPost>, FeedError> {
        if !post.visible_in(context.event) {
            return Ok(None);
        }
        post.validate()
            .map_err(|_| FeedError::DependencyUnavailable)?;
        let observed = match self.observe(context, post.author, Action::Read).await {
            Ok(v) => v,
            Err(FeedError::PermissionDenied | FeedError::ResourceUnavailable) => return Ok(None),
            Err(e) => return Err(e),
        };
        let author = self.resources.author(context, post.author).await?;
        if author.user_id != post.author {
            return Err(FeedError::DependencyUnavailable);
        }
        let mut links = Vec::new();
        for link in &post.content.links {
            if self.resources.public_link(context, *link).await? {
                links.push(*link);
            }
        }
        let mut media = Vec::new();
        for id in &post.content.media_ids {
            if self
                .resources
                .public_media(context, post.author, *id)
                .await?
            {
                media.push(*id);
            }
        }
        // Recheck owner state after potentially slow profile/link/media dependencies.
        let current = self
            .reader
            .post(context, post.id)
            .await?
            .ok_or(FeedError::ResourceUnavailable)?;
        if current != post {
            return Err(FeedError::ResourceUnavailable);
        }
        // Target authorities are queried again. A changed reference is removed; the
        // final profile/relationship fence must not use cached allow decisions.
        post.content.links.clear();
        for link in links {
            if self.resources.public_link(context, link).await? {
                post.content.links.push(link);
            }
        }
        post.content.media_ids.clear();
        for id in media {
            if self
                .resources
                .public_media(context, post.author, id)
                .await?
            {
                post.content.media_ids.push(id);
            }
        }
        self.fence(observed).await?;
        Ok(Some(ProjectedPost { post, author }))
    }
    pub async fn page(
        &self,
        context: Context,
        window: Window,
        filter: Filter,
        limit: u16,
    ) -> Result<FeedPage, FeedError> {
        context.check(self.clock.now())?;
        if !(1..=100).contains(&limit)
            || window.upper > self.clock.now()
            || window.after.is_some_and(|p| p.created_at > window.upper)
        {
            return Err(FeedError::InvalidInput);
        }
        let member = self.observe(context, context.actor, Action::Read).await?;
        let batch = self.reader.page(context, window, filter, limit).await?;
        let rows = batch.items;
        if rows.len() > usize::from(limit)
            || rows.windows(2).any(|w| w[0].position() <= w[1].position())
            || rows
                .iter()
                .any(|p| p.event != context.event || !window.contains(p.position()))
        {
            return Err(FeedError::DependencyUnavailable);
        }
        // One bounded scan; empty pages may have a continuation when rows are revoked.
        let positions = rows.iter().map(Post::position).collect::<Vec<_>>();
        let next = continuation(window, &positions, batch.scanned_through, batch.has_more)?;
        let mut items = Vec::new();
        let mut observations = Vec::new();
        for row in rows {
            match self.project(context, row).await {
                Ok(Some(value)) if value.post.content.matches(filter) => {
                    observations.push(
                        self.observe(context, value.post.author, Action::Read)
                            .await?,
                    );
                    items.push(value);
                }
                Ok(_) | Err(FeedError::ResourceUnavailable | FeedError::PermissionDenied) => {}
                Err(e) => return Err(e),
            }
        }
        // Earlier rows can be revoked while later rows are enriched.
        for item in &items {
            let current = self
                .reader
                .post(context, item.post.id)
                .await?
                .ok_or(FeedError::ResourceUnavailable)?;
            if !current.visible_in(context.event) || current.revision != item.post.revision {
                return Err(FeedError::ResourceUnavailable);
            }
        }
        for observed in observations {
            self.fence(observed).await?;
        }
        self.fence(member).await?;
        Ok(FeedPage { items, next })
    }
    pub async fn post(&self, context: Context, id: Uuid) -> Result<ProjectedPost, FeedError> {
        self.observe(context, context.actor, Action::Read).await?;
        let post = self
            .reader
            .post(context, id)
            .await?
            .ok_or(FeedError::ResourceUnavailable)?;
        if post.id != id {
            return Err(FeedError::DependencyUnavailable);
        }
        self.project(context, post)
            .await?
            .ok_or(FeedError::ResourceUnavailable)
    }
}
/// Production defaults remain fail closed until owner adapters are explicitly composed.
pub struct UnavailableFeed;
#[async_trait]
impl FeedPolicy for UnavailableFeed {
    async fn observe(&self, _: Context, _: Uuid, _: Action) -> Result<Observation, FeedError> {
        Err(FeedError::DependencyUnavailable)
    }
    async fn fence(&self, _: Observation) -> Result<(), FeedError> {
        Err(FeedError::DependencyUnavailable)
    }
}
#[async_trait]
impl PostReader for UnavailableFeed {
    async fn page(
        &self,
        _: Context,
        _: Window,
        _: Filter,
        _: u16,
    ) -> Result<ScannedPosts, FeedError> {
        Err(FeedError::DependencyUnavailable)
    }
    async fn post(&self, _: Context, _: Uuid) -> Result<Option<Post>, FeedError> {
        Err(FeedError::DependencyUnavailable)
    }
}
#[async_trait]
impl FeedResources for UnavailableFeed {
    async fn author(&self, _: Context, _: Uuid) -> Result<PublicAuthor, FeedError> {
        Err(FeedError::DependencyUnavailable)
    }
    async fn public_link(&self, _: Context, _: ResourceLink) -> Result<bool, FeedError> {
        Err(FeedError::DependencyUnavailable)
    }
    async fn public_media(&self, _: Context, _: Uuid, _: Uuid) -> Result<bool, FeedError> {
        Err(FeedError::DependencyUnavailable)
    }
}

pub mod content;
pub use content::{CommentPage, CommentReader, PreparedCommand, ProjectedComment, Thread};
