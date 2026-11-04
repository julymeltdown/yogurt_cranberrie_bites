//! Filter unavailable comment authors before application projection, retaining
//! the actual owner scan boundary and every existing parent/final-state fence.
//! Ports supply current held authority; this adapter does not create a grant.
use async_trait::async_trait;
use chrono::Utc;
use concert_application::feed::{
    CommentReader, Context, FeedResources, PostReader, ScannedComments, ScannedPosts,
};
use concert_domain::feed::{Comment, ContentState, FeedError, Filter, Post, Window};
use uuid::Uuid;

pub struct AuthorFilteredReader<R, S> {
    reader: R,
    resources: S,
}
impl<R, S> AuthorFilteredReader<R, S> {
    pub fn new(reader: R, resources: S) -> Self {
        Self { reader, resources }
    }
}
#[async_trait]
impl<R: PostReader, S: FeedResources> PostReader for AuthorFilteredReader<R, S> {
    async fn page(
        &self,
        c: Context,
        w: Window,
        f: Filter,
        limit: u16,
    ) -> Result<ScannedPosts, FeedError> {
        self.reader.page(c, w, f, limit).await
    }
    async fn post(&self, c: Context, id: Uuid) -> Result<Option<Post>, FeedError> {
        self.reader.post(c, id).await
    }
}
#[async_trait]
impl<R: CommentReader, S: FeedResources> CommentReader for AuthorFilteredReader<R, S> {
    async fn comment(&self, c: Context, id: Uuid) -> Result<Option<Comment>, FeedError> {
        self.reader.comment(c, id).await
    }
    async fn comments(
        &self,
        c: Context,
        parent: Uuid,
        w: Window,
        limit: u16,
    ) -> Result<ScannedComments, FeedError> {
        c.check(Utc::now())?;
        if parent.is_nil() || !(1..=100).contains(&limit) {
            return Err(FeedError::InvalidInput);
        }
        let mut batch = self.reader.comments(c, parent, w, limit).await?;
        c.check(Utc::now())?;
        // Validate the entire unfiltered owner reply. Filtering must not hide a
        // malformed foreign-parent row, repeated tuple or bad continuation.
        if batch.items.len() > usize::from(limit)
            || batch
                .items
                .windows(2)
                .any(|rows| rows[0].position() <= rows[1].position())
            || batch.items.iter().any(|row| {
                row.event != c.event
                    || row.post != parent
                    || !w.contains(row.position())
                    || row.validate().is_err()
            })
            || (batch.has_more && batch.scanned_through.is_none())
            || batch
                .scanned_through
                .is_some_and(|position| !w.contains(position))
            || batch.items.last().is_some_and(|row| {
                batch
                    .scanned_through
                    .is_none_or(|position| position > row.position())
            })
        {
            return Err(FeedError::DependencyUnavailable);
        }
        let mut visible = Vec::with_capacity(batch.items.len());
        for row in batch.items {
            if row.state != ContentState::Published {
                continue;
            }
            match self.resources.author(c, row.author).await {
                Ok(author) => {
                    super::wire::author(author, row.author)?;
                    visible.push(row);
                }
                Err(FeedError::ResourceUnavailable | FeedError::PermissionDenied) => {}
                Err(error) => return Err(error),
            }
            c.check(Utc::now())?;
        }
        batch.items = visible;
        Ok(batch)
    }
}
