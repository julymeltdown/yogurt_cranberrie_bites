use super::mutation_receipts;
use super::subject_locks::{lock_post_subjects, lock_posts_subjects};
use crate::application::dto::mutation_dto::{
    MutationError, MutationOperationDto, MutationOutcomeDto, MutationReceiptStateDto,
    MutationReceiptStatusDto,
};
use crate::application::ports::{PostLikeRepositoryPort, PostRepositoryPort};
use crate::domain::comment::Comment;
use crate::domain::feed::{FeedPage, PostCursor, TrendingCursor};
use crate::domain::post::{
    Post, PostAccessSnapshot, PostAuditEvent, PostDetail, RetentionPurgeResult, StoredEvent,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use std::collections::{HashMap, HashSet};
use tracing::Instrument;
use uuid::Uuid;

#[derive(Clone)]
pub struct PgPostRepository {
    pool: PgPool,
}

#[derive(Clone)]
pub struct PgPostLikeRepository {
    pool: PgPool,
}

impl PgPostRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl PgPostLikeRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn event_origin_traceparent(event: &StoredEvent) -> Option<String> {
    event
        .origin_traceparent
        .clone()
        .or_else(|| platform::trace_context::current().map(|context| context.traceparent()))
}

fn map_post(row: sqlx::postgres::PgRow) -> Post {
    Post {
        id: row.get("id"),
        author_id: row.get("author_id"),
        content: row.get("content"),
        visibility: row.get("visibility"),
        created_at: row.get("created_at"),
        comment_count: row.get("comment_count"),
        like_count: row.get("like_count"),
    }
}

fn map_comment(row: sqlx::postgres::PgRow) -> Comment {
    Comment {
        id: row.get("id"),
        post_id: row.get("post_id"),
        author_id: row.get("author_id"),
        content: row.get("content"),
        created_at: row.get("created_at"),
    }
}

fn map_stored_event(row: sqlx::postgres::PgRow) -> StoredEvent {
    StoredEvent {
        event_id: row.get("event_id"),
        event_type: row.get("event_type"),
        aggregate_id: row.get("aggregate_id"),
        aggregate_version: row.get("aggregate_version"),
        origin_traceparent: row.get("origin_traceparent"),
        actor_id: row.get("actor_id"),
        attributes: row.get("attributes"),
        payload: row.get("payload"),
        occurred_at: row.get("occurred_at"),
        created_at: row.get("created_at"),
        published: row.get("published"),
        published_at: row.get("published_at"),
        replay_resolution_id: None,
    }
}

#[async_trait]
impl PostRepositoryPort for PgPostRepository {
    async fn mutation_receipt_status(
        &self,
        operation_id: Uuid,
    ) -> Result<Option<MutationReceiptStatusDto>> {
        let row = sqlx::query(
            "select operation_id, actor_id, scope, committed_at, replay_until,
                    case
                      when replay_until <= now() then 'EXPIRED'
                      when outcome_json is null then 'INCOMPLETE'
                      else 'REPLAYABLE'
                    end as receipt_state
             from post_mutation_receipts where operation_id = $1",
        )
        .bind(operation_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let state = match row.get::<String, _>("receipt_state").as_str() {
            "REPLAYABLE" => MutationReceiptStateDto::Replayable,
            "EXPIRED" => MutationReceiptStateDto::Expired,
            "INCOMPLETE" => MutationReceiptStateDto::Incomplete,
            _ => return Err(MutationError::Unavailable.into()),
        };
        Ok(Some(MutationReceiptStatusDto {
            operation_id: row.get("operation_id"),
            actor_id: row.get("actor_id"),
            scope: row.get("scope"),
            committed_at: row.get("committed_at"),
            replay_until: row.get("replay_until"),
            state,
        }))
    }

    async fn lookup_mutation(
        &self,
        operation: &MutationOperationDto,
    ) -> Result<Option<MutationOutcomeDto>> {
        mutation_receipts::lookup(&mut *self.pool.acquire().await?, operation).await
    }
    async fn save_post(&self, post: &Post) -> Result<Post> {
        let row = sqlx::query(
            r#"
            insert into posts (id, author_id, content, visibility, created_at, comment_count, like_count)
            values ($1, $2, $3, $4, $5, $6, $7)
            returning id, author_id, content, visibility, created_at, comment_count, like_count
            "#,
        )
        .bind(post.id)
        .bind(post.author_id)
        .bind(&post.content)
        .bind(&post.visibility)
        .bind(post.created_at)
        .bind(post.comment_count)
        .bind(post.like_count)
        .fetch_one(&self.pool)
        .await?;
        Ok(map_post(row))
    }

    async fn save_post_with_event(
        &self,
        post: &Post,
        event: &StoredEvent,
        operation: Option<&MutationOperationDto>,
    ) -> Result<(Post, StoredEvent)> {
        let span = tracing::info_span!(
            target: "post_service::adapters::out::persistence::post_repository",
            "post_write_transaction",
            db_system = "postgresql",
            db_operation = "post.create_with_outbox",
            trace_id = tracing::field::Empty,
            span_id = tracing::field::Empty,
            parent_span_id = tracing::field::Empty,
            sampled = tracing::field::Empty,
            outcome = tracing::field::Empty,
        );
        platform::trace_context::bind_current_child_to_span(&span);

        let result: Result<(Post, StoredEvent)> = async {
            let mut tx = self.pool.begin().await?;
            if let Some(outcome) = mutation_receipts::claim(&mut tx, operation).await? {
                return match outcome {
                    MutationOutcomeDto::Post(post, event) => Ok((post, event)),
                    _ => Err(MutationError::Conflict.into()),
                };
            }
            let post_row = sqlx::query(
                r#"
                insert into posts (id, author_id, content, visibility, created_at, comment_count, like_count)
                values ($1, $2, $3, $4, $5, $6, $7)
                returning id, author_id, content, visibility, created_at, comment_count, like_count
                "#,
            )
            .bind(post.id)
            .bind(post.author_id)
            .bind(&post.content)
            .bind(&post.visibility)
            .bind(post.created_at)
            .bind(post.comment_count)
            .bind(post.like_count)
            .fetch_one(&mut *tx)
            .await?;
            let event_row = sqlx::query(
                r#"
                insert into post_event_outbox (
                    event_id, event_type, aggregate_id, actor_id, attributes, payload,
                    occurred_at, created_at, published, published_at, origin_traceparent
                )
                values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                returning event_id, event_type, aggregate_id, actor_id, attributes, payload,
                    occurred_at, created_at, published, published_at, aggregate_version,
                    origin_traceparent
                "#,
            )
            .bind(event.event_id)
            .bind(&event.event_type)
            .bind(event.aggregate_id)
            .bind(event.actor_id)
            .bind(&event.attributes)
            .bind(&event.payload)
            .bind(event.occurred_at)
            .bind(event.created_at)
            .bind(event.published)
            .bind(event.published_at)
            .bind(event_origin_traceparent(event))
            .fetch_one(&mut *tx)
            .await?;
            let result = (map_post(post_row), map_stored_event(event_row));
            mutation_receipts::complete(
                &mut tx,
                operation,
                MutationOutcomeDto::Post(result.0.clone(), result.1.clone()),
            )
            .await?;
            tx.commit().await?;
            Ok(result)
        }
        .instrument(span.clone())
        .await;

        span.record("outcome", if result.is_ok() { "success" } else { "error" });
        result
    }

    async fn save_comment(&self, comment: &Comment) -> Result<Comment> {
        let row = sqlx::query(
            r#"
            insert into comments (id, post_id, author_id, content, created_at)
            values ($1, $2, $3, $4, $5)
            returning id, post_id, author_id, content, created_at
            "#,
        )
        .bind(comment.id)
        .bind(comment.post_id)
        .bind(comment.author_id)
        .bind(&comment.content)
        .bind(comment.created_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(map_comment(row))
    }

    async fn save_comment_with_counter_event(
        &self,
        comment: &Comment,
        event: &StoredEvent,
        operation: Option<&MutationOperationDto>,
    ) -> Result<(Comment, StoredEvent)> {
        let mut tx = self.pool.begin().await?;
        lock_post_subjects(&mut tx, comment.post_id, &[comment.author_id]).await?;
        if let Some(outcome) = mutation_receipts::claim(&mut tx, operation).await? {
            return match outcome {
                MutationOutcomeDto::Comment(comment, event) => Ok((comment, event)),
                _ => Err(MutationError::Conflict.into()),
            };
        }
        // Serialize with deletion/moderation and refuse a comment on hidden content.
        let parent: Option<Uuid> = sqlx::query_scalar("select id from posts where id = $1 and deleted_at is null and moderation_hidden_at is null for update")
            .bind(comment.post_id).fetch_optional(&mut *tx).await?;
        anyhow::ensure!(parent.is_some(), "Post not found");
        let comment_row = sqlx::query(
            r#"
            insert into comments (id, post_id, author_id, content, created_at)
            values ($1, $2, $3, $4, $5)
            returning id, post_id, author_id, content, created_at
            "#,
        )
        .bind(comment.id)
        .bind(comment.post_id)
        .bind(comment.author_id)
        .bind(&comment.content)
        .bind(comment.created_at)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("update posts set comment_count = comment_count + 1 where id = $1")
            .bind(comment.post_id)
            .execute(&mut *tx)
            .await?;
        let event_row = sqlx::query(
            r#"
            insert into post_event_outbox (
                event_id, event_type, aggregate_id, actor_id, attributes, payload,
                occurred_at, created_at, published, published_at, origin_traceparent
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            returning event_id, event_type, aggregate_id, actor_id, attributes, payload,
                occurred_at, created_at, published, published_at, aggregate_version,
                origin_traceparent
            "#,
        )
        .bind(event.event_id)
        .bind(&event.event_type)
        .bind(event.aggregate_id)
        .bind(event.actor_id)
        .bind(&event.attributes)
        .bind(&event.payload)
        .bind(event.occurred_at)
        .bind(event.created_at)
        .bind(event.published)
        .bind(event.published_at)
        .bind(event_origin_traceparent(event))
        .fetch_one(&mut *tx)
        .await?;
        let result = (map_comment(comment_row), map_stored_event(event_row));
        mutation_receipts::complete(
            &mut tx,
            operation,
            MutationOutcomeDto::Comment(result.0.clone(), result.1.clone()),
        )
        .await?;
        tx.commit().await?;
        Ok(result)
    }

    async fn fetch_comment(&self, comment_id: Uuid) -> Result<Option<Comment>> {
        let row = sqlx::query(
            r#"
            select id, post_id, author_id, content, created_at
            from comments
            where id = $1
              and deleted_at is null
              and moderation_hidden_at is null
            "#,
        )
        .bind(comment_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(map_comment))
    }

    async fn fetch_feed(
        &self,
        viewer_id: Uuid,
        author_ids: &[Uuid],
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedPage> {
        if author_ids.is_empty() {
            return Ok(FeedPage {
                posts: Vec::new(),
                next_cursor: None,
            });
        }

        let parsed_cursor = PostCursor::parse(cursor.unwrap_or_default())?;
        let mut qb = QueryBuilder::<Postgres>::new(
            "select id, author_id, content, visibility, created_at, comment_count, like_count from posts where author_id in (",
        );
        {
            let mut separated = qb.separated(", ");
            for author_id in author_ids {
                separated.push_bind(author_id);
            }
        }
        qb.push(") and (visibility <> 'PRIVATE' or author_id = ");
        qb.push_bind(viewer_id);
        qb.push(") and deleted_at is null and moderation_hidden_at is null");
        if let Some(cursor) = &parsed_cursor {
            qb.push(" and (created_at, id) < (");
            qb.push_bind(cursor.created_at);
            qb.push(", ");
            qb.push_bind(cursor.post_id);
            qb.push(")");
        }
        qb.push(" order by created_at desc, id desc limit ");
        qb.push_bind(i64::from(limit.max(1)) + 1);
        let rows = qb.build().fetch_all(&self.pool).await?;
        let mut posts: Vec<Post> = rows.into_iter().map(map_post).collect();
        let next_cursor = if posts.len() > limit as usize {
            let cursor = posts
                .get(limit as usize - 1)
                .map(PostCursor::from)
                .map(|cursor| cursor.encode());
            posts.truncate(limit as usize);
            cursor
        } else {
            None
        };
        Ok(FeedPage { posts, next_cursor })
    }

    async fn fetch_by_author(
        &self,
        author_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedPage> {
        let parsed_cursor = PostCursor::parse(cursor.unwrap_or_default())?;
        let rows = sqlx::query(
            r#"
            select id, author_id, content, visibility, created_at, comment_count, like_count
            from posts
            where author_id = $1
              and deleted_at is null
              and moderation_hidden_at is null
              and ($2::timestamptz is null or (created_at, id) < ($2, $3))
            order by created_at desc, id desc
            limit $4
            "#,
        )
        .bind(author_id)
        .bind(parsed_cursor.as_ref().map(|value| value.created_at))
        .bind(parsed_cursor.as_ref().map(|value| value.post_id))
        .bind(i64::from(limit.max(1)) + 1)
        .fetch_all(&self.pool)
        .await?;
        let mut posts: Vec<Post> = rows.into_iter().map(map_post).collect();
        let next_cursor = if posts.len() > limit as usize {
            let cursor = posts
                .get(limit as usize - 1)
                .map(PostCursor::from)
                .map(|cursor| cursor.encode());
            posts.truncate(limit as usize);
            cursor
        } else {
            None
        };
        Ok(FeedPage { posts, next_cursor })
    }

    async fn search_by_content(
        &self,
        query: &str,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedPage> {
        let parsed_cursor = PostCursor::parse(cursor.unwrap_or_default())?;
        let rows = sqlx::query(
            r#"
            select id, author_id, content, visibility, created_at, comment_count, like_count
            from posts
            where lower(content) like $1 escape '\'
              and deleted_at is null
              and moderation_hidden_at is null
              and ($2::timestamptz is null or (created_at, id) < ($2, $3))
            order by created_at desc, id desc
            limit $4
            "#,
        )
        // Escaped so a literal % or _ matches literally rather than every row.
        .bind(platform::search::contains_pattern(query))
        .bind(parsed_cursor.as_ref().map(|value| value.created_at))
        .bind(parsed_cursor.as_ref().map(|value| value.post_id))
        .bind(i64::from(limit.max(1)) + 10)
        .fetch_all(&self.pool)
        .await?;
        let mut posts: Vec<Post> = rows.into_iter().map(map_post).collect();
        let next_cursor = if posts.len() > limit as usize {
            let cursor = posts
                .get(limit as usize - 1)
                .map(PostCursor::from)
                .map(|cursor| cursor.encode());
            posts.truncate(limit as usize);
            cursor
        } else {
            None
        };
        Ok(FeedPage { posts, next_cursor })
    }

    async fn count_posts_by_author(&self, author_ids: &[Uuid]) -> Result<HashMap<Uuid, i64>> {
        if author_ids.is_empty() {
            return Ok(HashMap::new());
        }
        // The aggregate is computed in one statement rather than per author, so
        // a batch costs the same number of round trips regardless of size. This
        // is a profile-level total: per-post PUBLIC/FRIENDS/PRIVATE visibility
        // does not change the count. Soft-deleted and moderation-hidden posts
        // are excluded.
        let mut query = QueryBuilder::<Postgres>::new(
            "select author_id, count(*) as post_count from posts where author_id in (",
        );
        {
            let mut separated = query.separated(", ");
            for author_id in author_ids {
                separated.push_bind(author_id);
            }
        }
        query.push(") and deleted_at is null and moderation_hidden_at is null group by author_id");
        let rows = query.build().fetch_all(&self.pool).await?;
        let mut counts = HashMap::with_capacity(rows.len());
        for row in rows {
            counts.insert(
                row.get::<Uuid, _>("author_id"),
                row.get::<i64, _>("post_count"),
            );
        }
        Ok(counts)
    }

    async fn fetch_trending(&self, cursor: Option<&str>, limit: i32) -> Result<FeedPage> {
        let parsed_cursor = TrendingCursor::parse(cursor.unwrap_or_default())?;
        let rows = sqlx::query(
            r#"
            select id, author_id, content, visibility, created_at, comment_count, like_count
            from posts
            where deleted_at is null
              and moderation_hidden_at is null
              and (
                $1::bigint is null
                or (like_count, created_at, id) < ($1, $2, $3)
            )
            order by like_count desc, created_at desc, id desc
            limit $4
            "#,
        )
        .bind(parsed_cursor.as_ref().map(|value| value.like_count))
        .bind(parsed_cursor.as_ref().map(|value| value.created_at))
        .bind(parsed_cursor.as_ref().map(|value| value.post_id))
        .bind(i64::from(limit.max(1)) + 10)
        .fetch_all(&self.pool)
        .await?;
        let mut posts: Vec<Post> = rows.into_iter().map(map_post).collect();
        let next_cursor = if posts.len() > limit as usize {
            let cursor = posts
                .get(limit as usize - 1)
                .map(TrendingCursor::from)
                .map(|cursor| cursor.encode());
            posts.truncate(limit as usize);
            cursor
        } else {
            None
        };
        Ok(FeedPage { posts, next_cursor })
    }

    async fn fetch_post_access_snapshots(
        &self,
        post_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, PostAccessSnapshot>> {
        if post_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut qb = QueryBuilder::<Postgres>::new(
            "select id, author_id, visibility from posts where id in (",
        );
        {
            let mut separated = qb.separated(", ");
            for post_id in post_ids {
                separated.push_bind(post_id);
            }
        }
        qb.push(" ) and deleted_at is null and moderation_hidden_at is null");
        let rows = qb.build().fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    row.get("id"),
                    PostAccessSnapshot {
                        author_id: row.get("author_id"),
                        visibility: row.get("visibility"),
                    },
                )
            })
            .collect())
    }

    async fn fetch_posts_by_ids(&self, post_ids: &[Uuid]) -> Result<Vec<Post>> {
        if post_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut qb = QueryBuilder::<Postgres>::new(
            "select id, author_id, content, visibility, created_at, comment_count, like_count from posts where id in (",
        );
        {
            let mut separated = qb.separated(", ");
            for post_id in post_ids {
                separated.push_bind(post_id);
            }
        }
        qb.push(") and deleted_at is null and moderation_hidden_at is null");
        let rows = qb.build().fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(map_post).collect())
    }

    async fn fetch_post_header(&self, post_id: Uuid) -> Result<Option<Post>> {
        let row = sqlx::query(
            "select id, author_id, content, visibility, created_at, comment_count, like_count from posts where id = $1 and deleted_at is null and moderation_hidden_at is null",
        )
        .bind(post_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(map_post))
    }

    async fn fetch_post(&self, post_id: Uuid) -> Result<Option<PostDetail>> {
        let post = sqlx::query(
            "select id, author_id, content, visibility, created_at, comment_count, like_count from posts where id = $1 and deleted_at is null and moderation_hidden_at is null",
        )
        .bind(post_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(post) = post else {
            return Ok(None);
        };
        let comments = sqlx::query(
            "select id, post_id, author_id, content, created_at from comments where post_id = $1 and deleted_at is null and moderation_hidden_at is null order by created_at asc, id asc",
        )
        .bind(post_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(Some(PostDetail {
            post: map_post(post),
            comments: comments.into_iter().map(map_comment).collect(),
        }))
    }

    async fn soft_delete_post(
        &self,
        post_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            update posts
            set updated_at = now(),
                deleted_at = now(),
                deleted_by = $2,
                delete_reason = $3,
                retention_until = $4
            where id = $1
              and deleted_at is null
            "#,
        )
        .bind(post_id)
        .bind(actor_id)
        .bind(reason)
        .bind(retention_until)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn soft_delete_post_with_event_audit(
        &self,
        post_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
        event: &StoredEvent,
        audit_event: &PostAuditEvent,
        operation: Option<&MutationOperationDto>,
    ) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        lock_post_subjects(&mut tx, post_id, &[actor_id]).await?;
        if let Some(outcome) = mutation_receipts::claim(&mut tx, operation).await? {
            return match outcome {
                MutationOutcomeDto::Deleted { post_id: stored } if stored == post_id => Ok(true),
                _ => Err(MutationError::Conflict.into()),
            };
        }
        let result = sqlx::query(
            r#"
            update posts
            set updated_at = now(),
                deleted_at = now(),
                deleted_by = $2,
                delete_reason = $3,
                retention_until = $4
            where id = $1
              and deleted_at is null
            "#,
        )
        .bind(post_id)
        .bind(actor_id)
        .bind(reason)
        .bind(retention_until)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Ok(false);
        }

        sqlx::query(
            r#"
            insert into post_event_outbox (
                event_id, event_type, aggregate_id, actor_id, attributes, payload,
                occurred_at, created_at, published, published_at, origin_traceparent
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(event.event_id)
        .bind(&event.event_type)
        .bind(event.aggregate_id)
        .bind(event.actor_id)
        .bind(&event.attributes)
        .bind(&event.payload)
        .bind(event.occurred_at)
        .bind(event.created_at)
        .bind(event.published)
        .bind(event.published_at)
        .bind(event_origin_traceparent(event))
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            insert into post_audit_events (
                id, actor_id, post_id, comment_id, action, outcome,
                request_id, ip_address, user_agent, metadata_json, occurred_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(audit_event.id)
        .bind(audit_event.actor_id)
        .bind(audit_event.post_id)
        .bind(audit_event.comment_id)
        .bind(&audit_event.action)
        .bind(&audit_event.outcome)
        .bind(&audit_event.request_id)
        .bind(&audit_event.ip_address)
        .bind(&audit_event.user_agent)
        .bind(&audit_event.metadata_json)
        .bind(audit_event.occurred_at)
        .execute(&mut *tx)
        .await?;

        mutation_receipts::complete(&mut tx, operation, MutationOutcomeDto::Deleted { post_id })
            .await?;
        tx.commit().await?;
        Ok(true)
    }

    async fn soft_delete_comment_with_event_audit(
        &self,
        comment_id: Uuid,
        post_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
        event: &StoredEvent,
        audit_event: &PostAuditEvent,
    ) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        lock_post_subjects(&mut tx, post_id, &[actor_id]).await?;
        let result = sqlx::query(
            r#"
            update comments
            set updated_at = now(),
                deleted_at = now(),
                deleted_by = $2,
                delete_reason = $3,
                retention_until = $4
            where id = $1
              and post_id = $5
              and deleted_at is null
              and moderation_hidden_at is null
            "#,
        )
        .bind(comment_id)
        .bind(actor_id)
        .bind(reason)
        .bind(retention_until)
        .bind(post_id)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Ok(false);
        }

        sqlx::query(
            "update posts set comment_count = greatest(comment_count - 1, 0) where id = $1",
        )
        .bind(post_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            insert into post_event_outbox (
                event_id, event_type, aggregate_id, actor_id, attributes, payload,
                occurred_at, created_at, published, published_at, origin_traceparent
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(event.event_id)
        .bind(&event.event_type)
        .bind(event.aggregate_id)
        .bind(event.actor_id)
        .bind(&event.attributes)
        .bind(&event.payload)
        .bind(event.occurred_at)
        .bind(event.created_at)
        .bind(event.published)
        .bind(event.published_at)
        .bind(event_origin_traceparent(event))
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            insert into post_audit_events (
                id, actor_id, post_id, comment_id, action, outcome,
                request_id, ip_address, user_agent, metadata_json, occurred_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(audit_event.id)
        .bind(audit_event.actor_id)
        .bind(audit_event.post_id)
        .bind(audit_event.comment_id)
        .bind(&audit_event.action)
        .bind(&audit_event.outcome)
        .bind(&audit_event.request_id)
        .bind(&audit_event.ip_address)
        .bind(&audit_event.user_agent)
        .bind(&audit_event.metadata_json)
        .bind(audit_event.occurred_at)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(true)
    }

    async fn record_audit_event(&self, event: &PostAuditEvent) -> Result<()> {
        sqlx::query(
            r#"
            insert into post_audit_events (
                id, actor_id, post_id, comment_id, action, outcome,
                request_id, ip_address, user_agent, metadata_json, occurred_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(event.id)
        .bind(event.actor_id)
        .bind(event.post_id)
        .bind(event.comment_id)
        .bind(&event.action)
        .bind(&event.outcome)
        .bind(&event.request_id)
        .bind(&event.ip_address)
        .bind(&event.user_agent)
        .bind(&event.metadata_json)
        .bind(event.occurred_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn purge_expired_posts(
        &self,
        cutoff: DateTime<Utc>,
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
    ) -> Result<RetentionPurgeResult> {
        let mut tx = self.pool.begin().await?;
        let post_rows = sqlx::query(
            r#"
            select id
            from posts
            where deleted_at is not null
              and retention_until is not null
              and retention_until <= $1
            order by retention_until asc, id asc
            "#,
        )
        .bind(cutoff)
        .fetch_all(&mut *tx)
        .await?;
        let post_ids: Vec<Uuid> = post_rows.into_iter().map(|row| row.get("id")).collect();

        let comment_rows = sqlx::query(
            r#"
            select id, post_id
            from comments
            where deleted_at is not null
              and retention_until is not null
              and retention_until <= $1
              and not (post_id = any($2))
            order by retention_until asc, id asc
            "#,
        )
        .bind(cutoff)
        .bind(&post_ids)
        .fetch_all(&mut *tx)
        .await?;
        let comment_rows: Vec<(Uuid, Uuid)> = comment_rows
            .into_iter()
            .map(|row| (row.get("id"), row.get("post_id")))
            .collect();
        let comment_ids: Vec<Uuid> = comment_rows
            .iter()
            .map(|(comment_id, _post_id)| *comment_id)
            .collect();

        // Lifecycle deletion can lock a comment before updating its parent.
        // Serialize on the complete owner set before acquiring any post row or
        // destructive cascade, including parents of independently expired comments.
        let mut affected_posts = post_ids.clone();
        affected_posts.extend(comment_rows.iter().map(|(_, post)| *post));
        lock_posts_subjects(&mut tx, &affected_posts, &[]).await?;
        // Account deletion may have extended retention while we waited. Audit
        // and delete only rows still eligible under the acquired owner/row locks.
        let post_ids: Vec<Uuid> = sqlx::query_scalar(
            "select id from posts where id=any($1) and deleted_at is not null and retention_until<=$2 order by retention_until,id",
        ).bind(&post_ids).bind(cutoff).fetch_all(&mut *tx).await?;
        let comment_rows: Vec<(Uuid, Uuid)> = sqlx::query_as(
            "select id,post_id from comments where id=any($1) and deleted_at is not null and retention_until<=$2 and not(post_id=any($3)) order by retention_until,id for update",
        ).bind(&comment_ids).bind(cutoff).bind(&post_ids).fetch_all(&mut *tx).await?;
        let comment_ids: Vec<Uuid> = comment_rows.iter().map(|(id, _)| *id).collect();

        if !dry_run {
            // Prune payloads, never identities. Even a quiet service must clear
            // expired bodies; retained tombstones prevent late redispatch.
            sqlx::query(
                "update post_mutation_receipts set outcome_json = null
                 where operation_id in (
                     select operation_id from post_mutation_receipts
                     where replay_until <= now() and outcome_json is not null
                     order by replay_until limit 1000 for update skip locked
                 )",
            )
            .execute(&mut *tx)
            .await?;
        }
        if post_ids.is_empty() && comment_ids.is_empty() {
            tx.commit().await?;
            return Ok(RetentionPurgeResult {
                post_ids,
                comment_ids,
            });
        }

        let outcome = if dry_run { "DRY_RUN" } else { "SUCCESS" };
        for post_id in &post_ids {
            sqlx::query(
                r#"
                insert into post_audit_events (
                    id, actor_id, post_id, comment_id, action, outcome,
                    request_id, ip_address, user_agent, metadata_json, occurred_at
                )
                values ($1, $2, $3, null, 'POST_PURGED', $4, $5, $6, $7, $8, now())
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(actor_id)
            .bind(*post_id)
            .bind(outcome)
            .bind(&request_id)
            .bind(&ip_address)
            .bind(&user_agent)
            .bind(
                json!({
                    "cutoff": cutoff,
                    "dryRun": dry_run
                })
                .to_string(),
            )
            .execute(&mut *tx)
            .await?;
        }

        for (comment_id, post_id) in &comment_rows {
            sqlx::query(
                r#"
                insert into post_audit_events (
                    id, actor_id, post_id, comment_id, action, outcome,
                    request_id, ip_address, user_agent, metadata_json, occurred_at
                )
                values ($1, $2, $3, $4, 'COMMENT_PURGED', $5, $6, $7, $8, $9, now())
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(actor_id)
            .bind(*post_id)
            .bind(*comment_id)
            .bind(outcome)
            .bind(&request_id)
            .bind(&ip_address)
            .bind(&user_agent)
            .bind(
                json!({
                    "cutoff": cutoff,
                    "dryRun": dry_run
                })
                .to_string(),
            )
            .execute(&mut *tx)
            .await?;
        }

        if dry_run {
            tx.commit().await?;
            return Ok(RetentionPurgeResult {
                post_ids,
                comment_ids,
            });
        }

        if !comment_ids.is_empty() {
            sqlx::query("delete from comments where id = any($1)")
                .bind(&comment_ids)
                .execute(&mut *tx)
                .await?;
        }

        sqlx::query("delete from posts where id = any($1)")
            .bind(&post_ids)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(RetentionPurgeResult {
            post_ids,
            comment_ids,
        })
    }

    async fn adjust_like_count(&self, post_id: Uuid, delta: i64) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        lock_post_subjects(&mut tx, post_id, &[]).await?;
        sqlx::query("update posts set like_count = greatest(like_count + $2, 0) where id = $1")
            .bind(post_id)
            .bind(delta)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn adjust_comment_count(&self, post_id: Uuid, delta: i64) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        lock_post_subjects(&mut tx, post_id, &[]).await?;
        sqlx::query(
            "update posts set comment_count = greatest(comment_count + $2, 0) where id = $1",
        )
        .bind(post_id)
        .bind(delta)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
}

#[async_trait]
impl PostLikeRepositoryPort for PgPostLikeRepository {
    async fn add_like(&self, post_id: Uuid, user_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            "insert into post_likes (post_id, user_id, created_at) values ($1, $2, now()) on conflict do nothing",
        )
        .bind(post_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn add_like_with_event(
        &self,
        post_id: Uuid,
        user_id: Uuid,
        event: &StoredEvent,
        operation: Option<&MutationOperationDto>,
    ) -> Result<(bool, i64)> {
        let mut tx = self.pool.begin().await?;
        lock_post_subjects(&mut tx, post_id, &[user_id]).await?;
        if let Some(outcome) = mutation_receipts::claim(&mut tx, operation).await? {
            return match outcome {
                MutationOutcomeDto::Like {
                    post_id: stored,
                    changed,
                    like_count,
                    liked: true,
                } if stored == post_id => Ok((changed, like_count)),
                _ => Err(MutationError::Conflict.into()),
            };
        }
        let result = sqlx::query(
            "insert into post_likes (post_id, user_id, created_at) values ($1, $2, now()) on conflict do nothing",
        )
        .bind(post_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        let added = result.rows_affected() > 0;
        let like_count: i64 = if added {
            sqlx::query_scalar(
                "update posts set like_count = like_count + 1 where id = $1 and deleted_at is null and moderation_hidden_at is null returning like_count",
            )
            .bind(post_id)
            .fetch_one(&mut *tx)
            .await?
        } else {
            sqlx::query_scalar("select like_count from posts where id = $1 and deleted_at is null and moderation_hidden_at is null")
                .bind(post_id)
                .fetch_one(&mut *tx)
                .await?
        };
        if added {
            let mut payload: serde_json::Value = serde_json::from_str(&event.payload)
                .context("like event payload is not valid JSON")?;
            payload
                .as_object_mut()
                .context("like event payload must be a JSON object")?
                .insert("likeCount".to_owned(), json!(like_count));
            sqlx::query(
                r#"
                insert into post_event_outbox (
                    event_id, event_type, aggregate_id, actor_id, attributes, payload,
                    occurred_at, created_at, published, published_at, origin_traceparent
                )
                values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                "#,
            )
            .bind(event.event_id)
            .bind(&event.event_type)
            .bind(event.aggregate_id)
            .bind(event.actor_id)
            .bind(&event.attributes)
            .bind(payload.to_string())
            .bind(event.occurred_at)
            .bind(event.created_at)
            .bind(event.published)
            .bind(event.published_at)
            .bind(event_origin_traceparent(event))
            .execute(&mut *tx)
            .await?;
        }
        mutation_receipts::complete(
            &mut tx,
            operation,
            MutationOutcomeDto::Like {
                post_id,
                changed: added,
                like_count,
                liked: true,
            },
        )
        .await?;
        tx.commit().await?;
        Ok((added, like_count))
    }

    async fn remove_like(&self, post_id: Uuid, user_id: Uuid) -> Result<bool> {
        let result = sqlx::query("delete from post_likes where post_id = $1 and user_id = $2")
            .bind(post_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn remove_like_with_event(
        &self,
        post_id: Uuid,
        user_id: Uuid,
        event: &StoredEvent,
        operation: Option<&MutationOperationDto>,
    ) -> Result<(bool, i64)> {
        let mut tx = self.pool.begin().await?;
        lock_post_subjects(&mut tx, post_id, &[user_id]).await?;
        if let Some(outcome) = mutation_receipts::claim(&mut tx, operation).await? {
            return match outcome {
                MutationOutcomeDto::Like {
                    post_id: stored,
                    changed,
                    like_count,
                    liked: false,
                } if stored == post_id => Ok((changed, like_count)),
                _ => Err(MutationError::Conflict.into()),
            };
        }
        let result = sqlx::query("delete from post_likes where post_id = $1 and user_id = $2")
            .bind(post_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        let removed = result.rows_affected() > 0;
        let like_count: i64 = if removed {
            sqlx::query_scalar(
                "update posts set like_count = greatest(like_count - 1, 0) where id = $1 and deleted_at is null and moderation_hidden_at is null returning like_count",
            )
            .bind(post_id)
            .fetch_one(&mut *tx)
            .await?
        } else {
            sqlx::query_scalar("select like_count from posts where id = $1 and deleted_at is null and moderation_hidden_at is null")
                .bind(post_id)
                .fetch_one(&mut *tx)
                .await?
        };
        if removed {
            sqlx::query(
                r#"
                insert into post_event_outbox (
                    event_id, event_type, aggregate_id, actor_id, attributes, payload,
                    occurred_at, created_at, published, published_at, origin_traceparent
                )
                values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                "#,
            )
            .bind(event.event_id)
            .bind(&event.event_type)
            .bind(event.aggregate_id)
            .bind(event.actor_id)
            .bind(&event.attributes)
            .bind(
                json!({
                    "postId": post_id,
                    "authorId": user_id,
                    "likeCount": like_count
                })
                .to_string(),
            )
            .bind(event.occurred_at)
            .bind(event.created_at)
            .bind(event.published)
            .bind(event.published_at)
            .bind(event_origin_traceparent(event))
            .execute(&mut *tx)
            .await?;
        }
        mutation_receipts::complete(
            &mut tx,
            operation,
            MutationOutcomeDto::Like {
                post_id,
                changed: removed,
                like_count,
                liked: false,
            },
        )
        .await?;
        tx.commit().await?;
        Ok((removed, like_count))
    }

    async fn count_likes(&self, post_ids: &[Uuid]) -> Result<HashMap<Uuid, i64>> {
        if post_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut qb = QueryBuilder::<Postgres>::new(
            "select post_id, count(*) as like_count from post_likes where post_id in (",
        );
        {
            let mut separated = qb.separated(", ");
            for post_id in post_ids {
                separated.push_bind(post_id);
            }
        }
        qb.push(") group by post_id");
        let rows = qb.build().fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|row| (row.get("post_id"), row.get("like_count")))
            .collect())
    }

    async fn liked_post_ids(&self, user_id: Uuid, post_ids: &[Uuid]) -> Result<HashSet<Uuid>> {
        if post_ids.is_empty() {
            return Ok(HashSet::new());
        }
        let mut qb =
            QueryBuilder::<Postgres>::new("select post_id from post_likes where user_id = ");
        qb.push_bind(user_id);
        qb.push(" and post_id in (");
        {
            let mut separated = qb.separated(", ");
            for post_id in post_ids {
                separated.push_bind(post_id);
            }
        }
        qb.push(")");
        let rows = qb.build().fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|row| row.get("post_id")).collect())
    }
}
