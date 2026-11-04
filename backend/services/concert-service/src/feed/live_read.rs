//! Canonical read orchestration. The caller must retain qualified owner guards
//! across this future and response assembly. Context is a binding, not a grant.
use super::{
    cursor::{CursorBinding, CursorCodec},
    wire,
};
use chrono::{DateTime, Utc};
use concert_application::{
    feed::{CommentReader, Context, Feed, FeedPolicy, FeedResources, PostReader},
    Clock,
};
use concert_domain::feed::{FeedError, Filter, Position, Window};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedQuery {
    pub limit: Option<u16>,
    pub cursor: Option<String>,
    pub filter: Option<Filter>,
    pub before: Option<DateTime<Utc>>,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommentsQuery {
    pub limit: Option<u16>,
    pub cursor: Option<String>,
}

/// No unavailable fallback or write capability is installed by this facade.
/// Generic ports remain the application contracts; startup must supply their
/// concrete production owners and keep authority guards alive outside the call.
pub struct ReadFeed<R, P, S, C> {
    feed: Feed<R, P, S, C>,
    cursors: CursorCodec,
}
impl<R: PostReader + CommentReader, P: FeedPolicy, S: FeedResources, C: Clock>
    ReadFeed<R, P, S, C>
{
    pub fn new(feed: Feed<R, P, S, C>, cursors: CursorCodec) -> Self {
        Self { feed, cursors }
    }

    pub async fn page(&self, context: Context, query: FeedQuery) -> Result<Value, FeedError> {
        let now = self.feed.clock.now();
        context.check(now)?;
        let binding = CursorBinding {
            actor: context.actor,
            event: context.event,
            operation: "listConcertFeed".into(),
            collection: "feed".into(),
            filter: query.filter.unwrap_or_default(),
            before: query.before,
            limit: limit(query.limit)?,
        };
        let window = self.window(&binding, query.cursor.as_deref(), now)?;
        self.bounded(context, async {
            let page = self
                .feed
                .page(context, window, binding.filter, binding.limit)
                .await?;
            let cursor = self.next(&binding, window, page.next)?;
            wire::page(page, cursor)
        })
        .await
    }

    /// Frozen getConcertPost has no query. Its continuation is a normal comment
    /// cursor, bound to this exact post and default limit, never a feed cursor.
    pub async fn detail(&self, context: Context, post: Uuid) -> Result<Value, FeedError> {
        let now = self.feed.clock.now();
        context.check(now)?;
        let binding = comments_binding(context, post, 20)?;
        let window = Window {
            upper: now,
            after: None,
        };
        self.bounded(context, async {
            let thread = self.feed.thread(context, post, window, 20).await?;
            let cursor = self.next(&binding, window, thread.comments.next)?;
            wire::thread(thread, cursor)
        })
        .await
    }

    pub async fn comments(
        &self,
        context: Context,
        post: Uuid,
        query: CommentsQuery,
    ) -> Result<Value, FeedError> {
        let now = self.feed.clock.now();
        context.check(now)?;
        let binding = comments_binding(context, post, limit(query.limit)?)?;
        let window = self.window(&binding, query.cursor.as_deref(), now)?;
        self.bounded(context, async {
            let comments = self
                .feed
                .comments(context, post, window, binding.limit)
                .await?;
            let cursor = self.next(&binding, window, comments.next)?;
            wire::comments(comments, cursor)
        })
        .await
    }

    fn window(
        &self,
        binding: &CursorBinding,
        cursor: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<Window, FeedError> {
        match cursor {
            Some(cursor) => self.cursors.open(binding, cursor, now),
            None => Ok(Window {
                upper: binding.before.unwrap_or(now).min(now),
                after: None,
            }),
        }
    }
    fn next(
        &self,
        binding: &CursorBinding,
        window: Window,
        position: Option<Position>,
    ) -> Result<Option<String>, FeedError> {
        position
            .map(|after| {
                self.cursors.seal(
                    binding,
                    Window {
                        upper: window.upper,
                        after: Some(after),
                    },
                    self.feed.clock.now(),
                )
            })
            .transpose()
    }
    async fn bounded(
        &self,
        context: Context,
        future: impl std::future::Future<Output = Result<Value, FeedError>>,
    ) -> Result<Value, FeedError> {
        // The absolute deadline was captured at ingress, never reconstructed from
        // a fresh per-hop timeout. Dropping this future drops all nested reads.
        let remaining = (context.expires_at - Utc::now())
            .to_std()
            .map_err(|_| FeedError::PermissionDenied)?;
        let result = tokio::time::timeout(remaining, future)
            .await
            .map_err(|_| FeedError::DependencyUnavailable)??;
        context.check(Utc::now())?;
        context.check(self.feed.clock.now())?;
        Ok(result)
    }
}
fn limit(value: Option<u16>) -> Result<u16, FeedError> {
    let limit = value.unwrap_or(20);
    if !(1..=100).contains(&limit) {
        return Err(FeedError::InvalidInput);
    }
    Ok(limit)
}
fn comments_binding(context: Context, post: Uuid, limit: u16) -> Result<CursorBinding, FeedError> {
    if post.is_nil() {
        return Err(FeedError::InvalidInput);
    }
    Ok(CursorBinding {
        actor: context.actor,
        event: context.event,
        operation: "listConcertComments".into(),
        collection: post.to_string(),
        filter: Filter::All,
        before: None,
        limit,
    })
}
