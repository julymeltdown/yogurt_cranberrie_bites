use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use post_service::application::ports::PostRepositoryPort;
use post_service::domain::comment::Comment;
use post_service::domain::feed::FeedPage;
use post_service::domain::post::{
    Post, PostAccessSnapshot, PostAuditEvent, PostDetail, RetentionPurgeResult, StoredEvent,
};
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurgeCall {
    pub actor_id: Uuid,
    pub request_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub dry_run: bool,
}

#[derive(Default)]
pub struct FakePostRepository {
    // Read-only dispatch probes; business receipt persistence still uses real PG.
    pub receipt_status:
        Mutex<Option<post_service::application::dto::mutation_dto::MutationReceiptStatusDto>>,
    pub receipt_queries: Mutex<Vec<Uuid>>,
    pub access_queries: Mutex<Vec<Vec<Uuid>>>,
    pub details: Mutex<HashMap<Uuid, PostDetail>>,
    pub comments: Mutex<HashMap<Uuid, Comment>>,
    pub posts_by_id: Mutex<HashMap<Uuid, Post>>,
    pub feed_page: Mutex<Option<FeedPage>>,
    pub fetch_feed_calls: Mutex<Vec<FeedQueryCall>>,
    pub author_page: Mutex<Option<FeedPage>>,
    pub search_page: Mutex<Option<FeedPage>>,
    /// Scripted search pages returned in order, one per call.
    ///
    /// `search_page` replays one page for every request, which cannot express a
    /// scan that advances. A queue can, so the multi-batch behaviour is testable
    /// without a database.
    pub search_pages: Mutex<VecDeque<FeedPage>>,
    pub trending_page: Mutex<Option<FeedPage>>,
    pub purge_result: Mutex<RetentionPurgeResult>,
    pub saved_comment_events: Mutex<Vec<StoredEvent>>,
    pub deleted_comment_events: Mutex<Vec<StoredEvent>>,
    pub purge_calls: Mutex<Vec<PurgeCall>>,
    pub author_calls: Mutex<Vec<(Uuid, Option<String>, i32)>>,
    pub search_calls: Mutex<Vec<(String, Option<String>, i32)>>,
    pub trending_calls: Mutex<Vec<(Option<String>, i32)>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedQueryCall {
    pub viewer_id: Uuid,
    pub author_ids: Vec<Uuid>,
    pub cursor: Option<String>,
    pub limit: i32,
}

impl FakePostRepository {
    pub fn insert_detail(&self, detail: PostDetail) {
        self.posts_by_id
            .lock()
            .unwrap()
            .insert(detail.post.id, detail.post.clone());
        self.details.lock().unwrap().insert(detail.post.id, detail);
    }

    pub fn insert_comment(&self, comment: Comment) {
        self.comments.lock().unwrap().insert(comment.id, comment);
    }
}

#[async_trait]
impl PostRepositoryPort for FakePostRepository {
    async fn mutation_receipt_status(
        &self,
        operation_id: Uuid,
    ) -> Result<Option<post_service::application::dto::mutation_dto::MutationReceiptStatusDto>>
    {
        self.receipt_queries.lock().unwrap().push(operation_id);
        Ok(self
            .receipt_status
            .lock()
            .unwrap()
            .clone()
            .filter(|receipt| receipt.operation_id == operation_id))
    }

    async fn count_posts_by_author(
        &self,
        _author_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, i64>> {
        Ok(std::collections::HashMap::new())
    }

    async fn save_post(&self, post: &Post) -> Result<Post> {
        Ok(post.clone())
    }

    async fn save_post_with_event(
        &self,
        post: &Post,
        event: &StoredEvent,
        operation: Option<&post_service::application::dto::mutation_dto::MutationOperationDto>,
    ) -> Result<(Post, StoredEvent)> {
        assert!(
            operation.is_none(),
            "Use real PostgreSQL to test owner receipts"
        );
        Ok((post.clone(), event.clone()))
    }

    async fn save_comment(&self, comment: &Comment) -> Result<Comment> {
        Ok(comment.clone())
    }

    async fn save_comment_with_counter_event(
        &self,
        comment: &Comment,
        event: &StoredEvent,
        operation: Option<&post_service::application::dto::mutation_dto::MutationOperationDto>,
    ) -> Result<(Comment, StoredEvent)> {
        assert!(
            operation.is_none(),
            "Use real PostgreSQL to test owner receipts"
        );
        self.saved_comment_events
            .lock()
            .unwrap()
            .push(event.clone());
        Ok((comment.clone(), event.clone()))
    }

    async fn fetch_comment(&self, comment_id: Uuid) -> Result<Option<Comment>> {
        Ok(self.comments.lock().unwrap().get(&comment_id).cloned())
    }

    async fn fetch_feed(
        &self,
        viewer_id: Uuid,
        author_ids: &[Uuid],
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedPage> {
        self.fetch_feed_calls.lock().unwrap().push(FeedQueryCall {
            viewer_id,
            author_ids: author_ids.to_vec(),
            cursor: cursor.map(str::to_owned),
            limit,
        });
        Ok(self
            .feed_page
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(empty_feed_page))
    }

    async fn fetch_by_author(
        &self,
        author_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedPage> {
        self.author_calls
            .lock()
            .unwrap()
            .push((author_id, cursor.map(str::to_owned), limit));
        Ok(self
            .author_page
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(empty_feed_page))
    }

    async fn search_by_content(
        &self,
        query: &str,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedPage> {
        self.search_calls.lock().unwrap().push((
            query.to_owned(),
            cursor.map(str::to_owned),
            limit,
        ));
        // A scripted queue takes precedence: it is the only fixture that can show
        // a scan advancing, because a single stored page answers every request.
        if let Some(page) = self.search_pages.lock().unwrap().pop_front() {
            return Ok(page);
        }
        Ok(self
            .search_page
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(empty_feed_page))
    }

    async fn fetch_trending(&self, cursor: Option<&str>, limit: i32) -> Result<FeedPage> {
        self.trending_calls
            .lock()
            .unwrap()
            .push((cursor.map(str::to_owned), limit));
        Ok(self
            .trending_page
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(empty_feed_page))
    }

    async fn fetch_posts_by_ids(&self, post_ids: &[Uuid]) -> Result<Vec<Post>> {
        let posts = self.posts_by_id.lock().unwrap();
        Ok(post_ids
            .iter()
            .filter_map(|id| posts.get(id).cloned())
            .collect())
    }

    async fn fetch_post_access_snapshots(
        &self,
        post_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, PostAccessSnapshot>> {
        self.access_queries.lock().unwrap().push(post_ids.to_vec());
        let posts = self.posts_by_id.lock().unwrap();
        Ok(post_ids
            .iter()
            .filter_map(|id| {
                posts.get(id).map(|post| {
                    (
                        *id,
                        PostAccessSnapshot {
                            author_id: post.author_id,
                            visibility: post.visibility.clone(),
                        },
                    )
                })
            })
            .collect())
    }

    async fn fetch_post_header(&self, post_id: Uuid) -> Result<Option<Post>> {
        Ok(self.posts_by_id.lock().unwrap().get(&post_id).cloned())
    }

    async fn fetch_post(&self, post_id: Uuid) -> Result<Option<PostDetail>> {
        Ok(self.details.lock().unwrap().get(&post_id).cloned())
    }

    async fn soft_delete_post(&self, _: Uuid, _: Uuid, _: &str, _: DateTime<Utc>) -> Result<bool> {
        Ok(true)
    }

    async fn soft_delete_post_with_event_audit(
        &self,
        _: Uuid,
        _: Uuid,
        _: &str,
        _: DateTime<Utc>,
        _: &StoredEvent,
        _: &PostAuditEvent,
        operation: Option<&post_service::application::dto::mutation_dto::MutationOperationDto>,
    ) -> Result<bool> {
        assert!(
            operation.is_none(),
            "Use real PostgreSQL to test owner receipts"
        );
        Ok(true)
    }

    async fn soft_delete_comment_with_event_audit(
        &self,
        _: Uuid,
        _: Uuid,
        _: Uuid,
        _: &str,
        _: DateTime<Utc>,
        event: &StoredEvent,
        _: &PostAuditEvent,
    ) -> Result<bool> {
        self.deleted_comment_events
            .lock()
            .unwrap()
            .push(event.clone());
        Ok(true)
    }

    async fn purge_expired_posts(
        &self,
        _: DateTime<Utc>,
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
    ) -> Result<RetentionPurgeResult> {
        self.purge_calls.lock().unwrap().push(PurgeCall {
            actor_id,
            request_id,
            ip_address,
            user_agent,
            dry_run,
        });
        Ok(self.purge_result.lock().unwrap().clone())
    }

    async fn record_audit_event(&self, _: &PostAuditEvent) -> Result<()> {
        Ok(())
    }

    async fn adjust_like_count(&self, _: Uuid, _: i64) -> Result<()> {
        Ok(())
    }

    async fn adjust_comment_count(&self, _: Uuid, _: i64) -> Result<()> {
        Ok(())
    }
}

fn empty_feed_page() -> FeedPage {
    FeedPage {
        posts: Vec::new(),
        next_cursor: None,
    }
}
