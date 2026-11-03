use crate::authority_witness::WitnessMemberDto;
use crate::query_witness::{
    OwnedQueryInputDto, OwnedQueryObservationDto, OwnedQueryWitnessProducerPort,
    PublishedWitnessStageDto,
};
pub mod apply_content_moderation;
pub mod authorize_notification_posts;
pub mod create_comment;
pub mod create_post;
pub mod delete_comment;
pub mod delete_post;
pub mod get_author_post_counts;
pub mod get_author_posts;
pub mod get_feed;
pub mod get_mutation_receipt_status;
pub mod get_post_detail;
pub mod get_ranked_feed;
pub mod get_trending;
pub mod like_post;
mod mutation_replay;
pub mod process_timeline_fanout;
pub mod purge_expired_posts;
pub mod record_feed_recommendation_event;
pub mod replay_events;
pub mod resolve_dead_letter_replay;
pub mod resolve_resource_witness;
pub mod search_posts;
pub mod unlike_post;

use crate::dto::mutation_dto::{
    MutationContextDto, MutationKind, MutationOperationDto, MutationOutcomeDto,
};
use crate::dto::post_dto::{
    AuthorPostCountDto, CommentDto, CreateCommentDto, CreatePostDto, DeleteCommentDto,
    DeleteCommentResponseDto, DeletePostDto, DeletePostResponseDto, FeedDto, PostDetailDto,
    PostDto, PostLikeDto, PurgeExpiredPostsDto, PurgeExpiredPostsResponseDto, ReplayRequestDto,
    ReplayResponseDto,
};
use crate::ports::{
    EventOutboxPort, ExternalEventPublisherPort, FeedCachePort, FriendClientPort,
    InternalEventDispatcherPort, PostCachePort, PostLikeRepositoryPort, PostRepositoryPort,
    ProfileClientPort,
};
use anyhow::{anyhow, bail, Result};
use chrono::{Duration, Utc};
use post_domain::comment::Comment;
use post_domain::feed::{
    FeedCachePage, FeedView, PostCursor, PostView, TrendingCursor, MIN_SEARCH_BATCH,
    SEARCH_SCAN_ROWS,
};
use post_domain::post::{
    Post, PostAuditEvent, PostLikeStatus, RelationshipPolicySnapshot, StoredEvent,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

const RELATIONSHIP_POLICY_BATCH_SIZE: usize = 100;

#[derive(Clone)]
pub struct PostApplicationService {
    pub posts: Arc<dyn PostRepositoryPort>,
    pub query_witnesses: Arc<dyn OwnedQueryWitnessProducerPort>,
    pub likes: Arc<dyn PostLikeRepositoryPort>,
    pub feed_cache: Arc<dyn FeedCachePort>,
    pub post_cache: Arc<dyn PostCachePort>,
    pub friend_client: Arc<dyn FriendClientPort>,
    pub profile_client: Arc<dyn ProfileClientPort>,
    pub outbox: Arc<dyn EventOutboxPort>,
    pub replay_delivery: Arc<dyn crate::ports::ReplayEventDeliveryPort>,
    pub publisher: Arc<dyn ExternalEventPublisherPort>,
    pub internal_dispatcher: Arc<dyn InternalEventDispatcherPort>,
}

impl PostApplicationService {
    pub async fn create_post(&self, request: CreatePostDto) -> Result<PostDto> {
        self.create_post_with_operation(request, None).await
    }

    pub async fn create_post_with_operation(
        &self,
        request: CreatePostDto,
        context: Option<MutationContextDto>,
    ) -> Result<PostDto> {
        let author_id = parse_uuid(&request.author_id, "Author ID is required")?;
        let content = required_content(&request.content)?;
        let visibility = normalize_visibility(&request.visibility)?;
        let operation = context
            .map(|context| {
                MutationOperationDto::new(
                    context,
                    author_id,
                    MutationKind::CreatePost,
                    json!([content, visibility]),
                )
            })
            .transpose()?;
        if let Some(MutationOutcomeDto::Post(post, _)) =
            self.replay_mutation(operation.as_ref()).await?
        {
            return Ok(post.into());
        }
        let profile = self.profile_client.require_author(author_id).await?;
        anyhow::ensure!(
            profile.user_id() == author_id,
            "Profile author binding denied"
        );
        let post = Post {
            id: Uuid::new_v4(),
            author_id,
            content,
            visibility: visibility.clone(),
            created_at: Utc::now(),
            comment_count: 0,
            like_count: 0,
        };
        let event = build_event(
            "POST_CREATED",
            post.id,
            Some(author_id),
            vec![visibility],
            json!({
                "postId": post.id,
                "authorId": author_id,
                "content": &post.content
            })
            .to_string(),
        );
        let (post, _) = self
            .posts
            .save_post_with_event(&post, &event, operation.as_ref())
            .await?;
        if operation.is_some() {
            self.ensure_replay_visible(post.id, author_id).await?;
        }
        Ok(post.into())
    }

    pub async fn create_comment(&self, request: CreateCommentDto) -> Result<CommentDto> {
        self.create_comment_with_operation(request, None).await
    }

    pub async fn create_comment_with_operation(
        &self,
        request: CreateCommentDto,
        context: Option<MutationContextDto>,
    ) -> Result<CommentDto> {
        let post_id = parse_uuid(&request.post_id, "Post ID is required")?;
        let author_id = parse_uuid(&request.author_id, "Author ID is required")?;
        let content = required_content(&request.content)?;
        let operation = context
            .map(|context| {
                MutationOperationDto::new(
                    context,
                    author_id,
                    MutationKind::CreateComment,
                    json!([post_id, content]),
                )
            })
            .transpose()?;
        if let Some(MutationOutcomeDto::Comment(comment, _)) =
            self.replay_mutation(operation.as_ref()).await?
        {
            return Ok(comment.into());
        }
        let existing = self
            .posts
            .fetch_post_header(post_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Post not found"))?;
        if !self.can_view(&existing, author_id).await? {
            bail!("Post not found");
        }
        let comment = Comment {
            id: Uuid::new_v4(),
            post_id,
            author_id,
            content,
            created_at: Utc::now(),
        };
        let event = build_event(
            "POST_COMMENTED",
            post_id,
            Some(author_id),
            vec!["comment".into()],
            json!({
                "postId": post_id,
                "authorId": author_id,
                "notifyUserId": existing.author_id,
                "content": &comment.content
            })
            .to_string(),
        );
        let (comment, _) = self
            .posts
            .save_comment_with_counter_event(&comment, &event, operation.as_ref())
            .await?;
        if operation.is_some() {
            self.ensure_replay_visible(post_id, author_id).await?;
            if self.posts.fetch_comment(comment.id).await?.is_none() {
                bail!("Comment not found");
            }
        }
        Ok(comment.into())
    }

    pub async fn delete_post(&self, request: DeletePostDto) -> Result<DeletePostResponseDto> {
        self.delete_post_with_operation(request, None).await
    }

    pub async fn delete_post_with_operation(
        &self,
        request: DeletePostDto,
        context: Option<MutationContextDto>,
    ) -> Result<DeletePostResponseDto> {
        let post_id = parse_uuid(&request.post_id, "Post ID is required")?;
        let actor_id = parse_uuid(&request.actor_id, "Actor ID is required")?;
        let operation = context
            .map(|context| {
                MutationOperationDto::new(
                    context,
                    actor_id,
                    MutationKind::DeletePost,
                    json!([post_id]),
                )
            })
            .transpose()?;
        if let Some(MutationOutcomeDto::Deleted { post_id }) =
            self.replay_mutation(operation.as_ref()).await?
        {
            return Ok(DeletePostResponseDto {
                post_id: post_id.to_string(),
                deleted: true,
            });
        }
        let post = match self.posts.fetch_post_header(post_id).await? {
            Some(post) => post,
            None => {
                // Another attempt may have committed between our receipt and
                // content reads. Consult the owner again, never repeat deletion.
                if let Some(MutationOutcomeDto::Deleted { post_id }) =
                    self.replay_mutation(operation.as_ref()).await?
                {
                    return Ok(DeletePostResponseDto {
                        post_id: post_id.to_string(),
                        deleted: true,
                    });
                }
                bail!("Post not found");
            }
        };

        if post.author_id != actor_id {
            self.record_post_audit(
                Some(actor_id),
                Some(post_id),
                "POST_DELETED",
                "FORBIDDEN",
                &request,
                json!({"authorId": post.author_id}).to_string(),
            )
            .await?;
            bail!("Forbidden");
        }

        let retention_until = Utc::now() + Duration::days(30);
        let event = build_event(
            "POST_DELETED",
            post_id,
            Some(actor_id),
            vec!["delete".into(), "soft_delete".into()],
            json!({
                "postId": post_id,
                "authorId": post.author_id,
                "actorId": actor_id,
                "retentionUntil": retention_until
            })
            .to_string(),
        );
        let audit_event = self.build_post_audit_event(
            Some(actor_id),
            Some(post_id),
            "POST_DELETED",
            "SUCCESS",
            &request,
            json!({"retentionUntil": retention_until}).to_string(),
        );
        let deleted = self
            .posts
            .soft_delete_post_with_event_audit(
                post_id,
                actor_id,
                "user_request",
                retention_until,
                &event,
                &audit_event,
                operation.as_ref(),
            )
            .await?;
        if !deleted {
            bail!("Post not found");
        }

        Ok(DeletePostResponseDto {
            post_id: post_id.to_string(),
            deleted: true,
        })
    }

    pub async fn delete_comment(
        &self,
        request: DeleteCommentDto,
    ) -> Result<DeleteCommentResponseDto> {
        let comment_id = parse_uuid(&request.comment_id, "Comment ID is required")?;
        let actor_id = parse_uuid(&request.actor_id, "Actor ID is required")?;
        let comment = self
            .posts
            .fetch_comment(comment_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Comment not found"))?;
        let post = self
            .posts
            .fetch_post_header(comment.post_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Comment not found"))?;

        if comment.author_id != actor_id {
            self.record_comment_audit(
                Some(actor_id),
                Some(comment.post_id),
                Some(comment_id),
                "COMMENT_DELETED",
                "FORBIDDEN",
                &request,
                json!({
                    "authorId": comment.author_id,
                    "postAuthorId": post.author_id
                })
                .to_string(),
            )
            .await?;
            bail!("Forbidden");
        }

        let retention_until = Utc::now() + Duration::days(30);
        let event = build_event(
            "POST_COMMENT_DELETED",
            comment.post_id,
            Some(actor_id),
            vec!["comment".into(), "delete".into(), "soft_delete".into()],
            json!({
                "postId": comment.post_id,
                "commentId": comment_id,
                "authorId": comment.author_id,
                "postAuthorId": post.author_id,
                "actorId": actor_id,
                "retentionUntil": retention_until
            })
            .to_string(),
        );
        let audit_event = self.build_comment_audit_event(
            Some(actor_id),
            Some(comment.post_id),
            Some(comment_id),
            "COMMENT_DELETED",
            "SUCCESS",
            &request,
            json!({"retentionUntil": retention_until}).to_string(),
        );
        let deleted = self
            .posts
            .soft_delete_comment_with_event_audit(
                comment_id,
                comment.post_id,
                actor_id,
                "user_request",
                retention_until,
                &event,
                &audit_event,
            )
            .await?;
        if !deleted {
            bail!("Comment not found");
        }

        Ok(DeleteCommentResponseDto {
            comment_id: comment_id.to_string(),
            post_id: comment.post_id.to_string(),
            deleted: true,
        })
    }

    pub async fn get_feed(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedDto> {
        let limit = normalize_limit(limit);
        let cursor = canonical_post_cursor(cursor)?;
        let cursor = cursor.as_deref();
        let cached = match self.feed_cache.get_feed(user_id, cursor, limit).await {
            Ok(cached) => cached,
            Err(error) => {
                tracing::warn!(error = ?error, user_id = %user_id, "feed cache read failed");
                None
            }
        };
        if let Some(cached) = cached {
            let posts = self.resolve_posts(&cached.post_ids).await?;
            let stage = self
                .observe_owned_posts(
                    user_id,
                    OwnedQueryInputDto::CachedChronological {
                        cursor: cursor.map(ToOwned::to_owned),
                        limit,
                        page_post_ids: cached.post_ids.clone(),
                    },
                    &posts,
                )
                .await?;
            let visible = self
                .filter_followed_feed(posts, user_id, stage.as_ref())
                .await?;
            let view = self
                .build_feed_view(visible, cached.next_cursor, user_id)
                .await?;
            return Ok(view.into());
        }

        // Read the generation before computing the page, not after. A mutation
        // that arrives while this page is being built bumps the generation, and
        // storing against the pre-read value is what makes the fill safe to
        // discard instead of silently serving pre-mutation content.
        let generation = match self.feed_cache.current_generation(user_id).await {
            Ok(generation) => Some(generation),
            Err(error) => {
                tracing::warn!(error = ?error, user_id = %user_id, "feed cache generation read failed");
                None
            }
        };

        let following = self.friend_client.fetch_feed_authors(user_id).await?;
        anyhow::ensure!(
            following.viewer_id() == user_id,
            "Feed author viewer binding denied"
        );
        let mut author_ids = following.author_ids().to_vec();
        if !author_ids.contains(&user_id) {
            author_ids.push(user_id);
        }
        let page = self
            .posts
            .fetch_feed(user_id, &author_ids, cursor, limit)
            .await?;
        let stage = self
            .observe_owned_posts(
                user_id,
                OwnedQueryInputDto::Chronological {
                    cursor: cursor.map(ToOwned::to_owned),
                    limit,
                    author_ids,
                },
                &page.posts,
            )
            .await?;
        let visible = self
            .filter_followed_feed(page.posts, user_id, stage.as_ref())
            .await?;
        if let Err(error) = self.post_cache.set_posts(&visible).await {
            tracing::warn!(error = ?error, user_id = %user_id, "post cache write failed");
        }
        if let Some(generation) = generation {
            if let Err(error) = self
                .feed_cache
                .set_feed(
                    user_id,
                    cursor,
                    limit,
                    &FeedCachePage {
                        post_ids: visible.iter().map(|post| post.id).collect(),
                        next_cursor: page.next_cursor.clone(),
                    },
                    generation,
                )
                .await
            {
                tracing::warn!(error = ?error, user_id = %user_id, "feed cache write failed");
            }
        }
        Ok(self
            .build_feed_view(visible, page.next_cursor, user_id)
            .await?
            .into())
    }

    pub(crate) async fn hydrate_recommendation_posts(
        &self,
        post_ids: &[Uuid],
        viewer_id: Uuid,
        input: OwnedQueryInputDto,
    ) -> Result<FeedDto> {
        let posts = self.resolve_posts(post_ids).await?;
        let stage = self.observe_owned_posts(viewer_id, input, &posts).await?;
        let visible = self
            .filter_discovery(posts, viewer_id, stage.as_ref())
            .await?;
        Ok(self.build_feed_view(visible, None, viewer_id).await?.into())
    }

    pub async fn get_author_posts(
        &self,
        author_id: Uuid,
        viewer_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedDto> {
        let cursor = canonical_post_cursor(cursor)?;
        let page = self
            .posts
            .fetch_by_author(author_id, cursor.as_deref(), normalize_limit(limit))
            .await?;
        let visible = self.filter_direct(page.posts, viewer_id).await?;
        Ok(self
            .build_feed_view(visible, page.next_cursor, viewer_id)
            .await?
            .into())
    }

    /// Count the existing posts for each requested author.
    ///
    /// Requested order is preserved and duplicates are collapsed, so the caller
    /// can line the result up with its request.
    ///
    /// An author with no posts gets an explicit zero. That is a real observation
    /// rather than a missing value: the caller asks about authors it already
    /// knows exist, so "no rows in the posts table" means "has not posted", not
    /// "unknown author". Encoding it as zero is what lets the caller distinguish
    /// an empty author from a failed lookup, which is a dependency error and is
    /// returned as one rather than as a count.
    pub async fn author_post_counts(
        &self,
        author_ids: Vec<Uuid>,
    ) -> Result<Vec<AuthorPostCountDto>> {
        let mut ordered = Vec::with_capacity(author_ids.len());
        for author_id in author_ids {
            if !ordered.contains(&author_id) {
                ordered.push(author_id);
            }
        }
        let counts = self.posts.count_posts_by_author(&ordered).await?;
        Ok(ordered
            .into_iter()
            .map(|author_id| AuthorPostCountDto {
                author_id: author_id.to_string(),
                post_count: counts.get(&author_id).copied().unwrap_or(0),
            })
            .collect())
    }

    pub async fn search_posts(
        &self,
        viewer_id: Uuid,
        query: &str,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedDto> {
        // Reject a term that is empty or too short before touching the database.
        // A one-character term matches a large fraction of the table, so it is a
        // client error rather than a legitimate query.
        let query = search_policy::normalize_search_term(query)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let cursor = canonical_post_cursor(cursor)?;
        let limit = normalize_limit(limit);
        // Visibility is decided here rather than in the query, so a database batch
        // is not a response page: a batch can be entirely filtered out while matching
        // rows still remain behind it. Answering from one batch would then return a
        // short page and a cursor, which reads exactly like a page that is short
        // because the results ran out. That is the shape that loses rows, because a
        // client paginating until an empty page stops early and never learns the
        // hidden prefix was not the end. Keep asking until the page is full, the
        // rows run out, or the scan budget is spent.
        let mut visible: Vec<Post> = Vec::new();
        let mut scan_from = cursor;
        let mut scanned = 0usize;
        // Whether the scan read every matching row. Set on the way out of the loop,
        // because that is the only place the answer is final.
        let exhausted;
        let mut sequence = 0u32;
        loop {
            let remaining = limit as usize - visible.len();
            let batch = remaining.max(MIN_SEARCH_BATCH) as i32;
            let requested_from = scan_from.clone();
            let page = self
                .posts
                .search_by_content(&query, requested_from.as_deref(), batch)
                .await?;
            let returned_cursor = page.next_cursor;
            scanned += page.posts.len();
            // Filtering runs against the whole batch, so the policy lookup is
            // amortized over every row fetched rather than one call per row.
            let stage = self
                .observe_owned_posts(
                    viewer_id,
                    OwnedQueryInputDto::Search {
                        term: query.clone(),
                        cursor: requested_from.clone(),
                        batch_limit: batch,
                        sequence,
                    },
                    &page.posts,
                )
                .await?;
            sequence = sequence
                .checked_add(1)
                .ok_or_else(|| anyhow!("Search witness sequence exhausted"))?;
            let admitted = self
                .filter_discovery(page.posts, viewer_id, stage.as_ref())
                .await?;
            // The repository offers a cursor only when rows remain beyond the batch
            // it returned, so an absent cursor means the scan reached the end of the
            // matching set. A cursor equal to the one just used means the batch would
            // be repeated, so its rows are already in hand and are not added twice.
            // That leaves the scan exhausted either way, and it is the case a stalled
            // or stubbed repository would otherwise turn into a duplicate page.
            let repeated = requested_from.is_some() && returned_cursor == requested_from;
            let advanced = returned_cursor.is_some() && !repeated;
            // On the way out this holds the furthest point the repository confirmed,
            // which is the only way to name a continuation when the scan stopped
            // between page boundaries.
            scan_from = returned_cursor;
            if !repeated {
                visible.extend(admitted);
            }
            if visible.len() >= limit as usize || !advanced || scanned >= SEARCH_SCAN_ROWS {
                exhausted = !advanced;
                break;
            }
        }
        // Anything past the page is left for the next request rather than dropped.
        let overflowed = visible.len() > limit as usize;
        visible.truncate(limit as usize);
        // The continuation is derived from the page boundary, not from the batch
        // cursor. The batch cursor names the last row *fetched*, and one batch can
        // return more admitted rows than fit on the page, so reusing it would skip
        // the rows the page left behind.
        //
        // An empty repository cursor means the scan read every matching row, but that
        // is not the same as an empty page: rows the viewer may not see can still
        // follow, and `overflowed` is what says visible rows remain. Ordering the two
        // checks the other way answers a full page with no continuation and strands
        // the rest of the results.
        let next_cursor = if overflowed {
            visible.last().map(|post| PostCursor::from(post).encode())
        } else if exhausted {
            None
        } else {
            // The row budget stopped the scan before the matching set was read, so
            // the continuation is the furthest point the repository confirmed.
            scan_from
        };
        Ok(self
            .build_feed_view(visible, next_cursor, viewer_id)
            .await?
            .into())
    }

    pub async fn get_post_detail(&self, post_id: Uuid, viewer_id: Uuid) -> Result<PostDetailDto> {
        let detail = self
            .posts
            .fetch_post(post_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Post not found"))?;
        if !self.can_view(&detail.post, viewer_id).await? {
            bail!("Post not found");
        }
        let like_counts = self.likes.count_likes(&[post_id]).await?;
        let liked_ids = self.likes.liked_post_ids(viewer_id, &[post_id]).await?;
        let post = PostView {
            id: detail.post.id,
            author_id: detail.post.author_id,
            content: detail.post.content,
            visibility: detail.post.visibility,
            created_at: detail.post.created_at,
            comment_count: detail.post.comment_count,
            like_count: like_counts
                .get(&post_id)
                .copied()
                .unwrap_or(detail.post.like_count),
            liked_by_viewer: liked_ids.contains(&post_id),
        };
        Ok(PostDetailDto {
            post: post.into(),
            comments: detail.comments.into_iter().map(Into::into).collect(),
        })
    }

    pub async fn like_post(&self, post_id: Uuid, user_id: Uuid) -> Result<PostLikeDto> {
        self.like_post_with_operation(post_id, user_id, None).await
    }

    pub async fn like_post_with_operation(
        &self,
        post_id: Uuid,
        user_id: Uuid,
        context: Option<MutationContextDto>,
    ) -> Result<PostLikeDto> {
        let operation = context
            .map(|context| {
                MutationOperationDto::new(
                    context,
                    user_id,
                    MutationKind::LikePost,
                    json!([post_id]),
                )
            })
            .transpose()?;
        if let Some(MutationOutcomeDto::Like {
            post_id,
            like_count,
            liked,
            ..
        }) = self.replay_mutation(operation.as_ref()).await?
        {
            return Ok(PostLikeStatus {
                post_id,
                like_count,
                liked,
            }
            .into());
        }
        let post = self
            .posts
            .fetch_post_header(post_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Post not found"))?;
        if !self.can_view(&post, user_id).await? {
            bail!("Post not found");
        }
        let event = build_event(
            "POST_LIKED",
            post_id,
            Some(user_id),
            vec!["like".into()],
            json!({
                "postId": post_id,
                "actorId": user_id,
                "authorId": post.author_id,
                "notifyUserId": post.author_id,
                "likeCount": post.like_count + 1
            })
            .to_string(),
        );
        let (_added, like_count) = self
            .likes
            .add_like_with_event(post_id, user_id, &event, operation.as_ref())
            .await?;
        if operation.is_some() {
            self.ensure_replay_visible(post_id, user_id).await?;
        }
        Ok(PostLikeStatus {
            post_id,
            like_count,
            liked: true,
        }
        .into())
    }

    pub async fn unlike_post(&self, post_id: Uuid, user_id: Uuid) -> Result<PostLikeDto> {
        self.unlike_post_with_operation(post_id, user_id, None)
            .await
    }

    pub async fn unlike_post_with_operation(
        &self,
        post_id: Uuid,
        user_id: Uuid,
        context: Option<MutationContextDto>,
    ) -> Result<PostLikeDto> {
        let operation = context
            .map(|context| {
                MutationOperationDto::new(
                    context,
                    user_id,
                    MutationKind::UnlikePost,
                    json!([post_id]),
                )
            })
            .transpose()?;
        if let Some(MutationOutcomeDto::Like {
            post_id,
            like_count,
            liked,
            ..
        }) = self.replay_mutation(operation.as_ref()).await?
        {
            return Ok(PostLikeStatus {
                post_id,
                like_count,
                liked,
            }
            .into());
        }
        let post = self
            .posts
            .fetch_post_header(post_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Post not found"))?;
        if !self.can_view(&post, user_id).await? {
            bail!("Post not found");
        }
        let event = build_event(
            "POST_UNLIKED",
            post_id,
            Some(user_id),
            vec!["unlike".into()],
            json!({
                "postId": post_id,
                "actorId": user_id,
                "authorId": post.author_id,
                "notifyUserId": post.author_id,
                "likeCount": post.like_count.saturating_sub(1)
            })
            .to_string(),
        );
        let (_removed, like_count) = self
            .likes
            .remove_like_with_event(post_id, user_id, &event, operation.as_ref())
            .await?;
        if operation.is_some() {
            self.ensure_replay_visible(post_id, user_id).await?;
        }
        Ok(PostLikeStatus {
            post_id,
            like_count,
            liked: false,
        }
        .into())
    }

    pub async fn get_trending(
        &self,
        viewer_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedDto> {
        let cursor = canonical_trending_cursor(cursor)?;
        let page = self
            .posts
            .fetch_trending(cursor.as_deref(), normalize_limit(limit))
            .await?;
        let stage = self
            .observe_owned_posts(
                viewer_id,
                OwnedQueryInputDto::Trending {
                    cursor: cursor.clone(),
                    limit: normalize_limit(limit),
                },
                &page.posts,
            )
            .await?;
        let visible = self
            .filter_discovery(page.posts, viewer_id, stage.as_ref())
            .await?;
        Ok(self
            .build_feed_view(visible, page.next_cursor, viewer_id)
            .await?
            .into())
    }

    pub async fn replay_events(&self, request: ReplayRequestDto) -> Result<ReplayResponseDto> {
        replay_events::ReplayEventsUseCase::new(self.outbox.clone(), self.replay_delivery.clone())
            .execute(request)
            .await
    }

    pub async fn purge_expired_posts(
        &self,
        request: PurgeExpiredPostsDto,
    ) -> Result<PurgeExpiredPostsResponseDto> {
        let actor_id = parse_uuid(&request.actor_id, "Actor ID is required")?;
        let purged = self
            .posts
            .purge_expired_posts(
                Utc::now(),
                actor_id,
                optional_text(request.request_id),
                optional_text(request.ip_address),
                optional_text(request.user_agent),
                request.dry_run,
            )
            .await?;
        Ok(PurgeExpiredPostsResponseDto {
            purged_count: purged.purged_count(),
            purged_post_count: purged.purged_post_count(),
            purged_comment_count: purged.purged_comment_count(),
        })
    }

    pub(crate) async fn observe_owned_posts(
        &self,
        viewer_id: Uuid,
        input: OwnedQueryInputDto,
        posts: &[Post],
    ) -> Result<Option<PublishedWitnessStageDto>> {
        let members = posts
            .iter()
            .map(|post| WitnessMemberDto::from_owned_query(post.id, post.author_id))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self
            .query_witnesses
            .observe(OwnedQueryObservationDto::new(
                viewer_id,
                input,
                members,
                vec![],
            )?)
            .await?)
    }

    async fn filter_direct(&self, posts: Vec<Post>, viewer_id: Uuid) -> Result<Vec<Post>> {
        self.filter_by_relationship(posts, viewer_id, false, false, None)
            .await
    }

    pub(crate) async fn filter_discovery(
        &self,
        posts: Vec<Post>,
        viewer_id: Uuid,
        stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<Vec<Post>> {
        self.filter_by_relationship(posts, viewer_id, true, false, stage)
            .await
    }

    /// The chronological home-feed fallback is limited to the viewer's current
    /// follows; a public post does not remain in that feed after an unfollow.
    async fn filter_followed_feed(
        &self,
        posts: Vec<Post>,
        viewer_id: Uuid,
        stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<Vec<Post>> {
        self.filter_by_relationship(posts, viewer_id, true, true, stage)
            .await
    }

    async fn filter_by_relationship(
        &self,
        posts: Vec<Post>,
        viewer_id: Uuid,
        exclude_muted: bool,
        require_following: bool,
        stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<Vec<Post>> {
        let mut author_ids = posts
            .iter()
            .map(|post| post.author_id)
            .filter(|author_id| *author_id != viewer_id)
            .collect::<Vec<_>>();
        author_ids.sort_unstable();
        author_ids.dedup();
        let policies = self
            .relationship_policies(viewer_id, &author_ids, stage)
            .await?;
        let mut visible = Vec::with_capacity(posts.len());
        for post in posts {
            if post.author_id == viewer_id {
                visible.push(post);
                continue;
            }
            let policy = policies
                .get(&post.author_id)
                .copied()
                .ok_or_else(|| anyhow!("Friend service omitted a requested relationship policy"))?;
            if !policy.is_blocked()
                && (!exclude_muted || !policy.muted_by_viewer)
                && (!require_following || policy.following)
                && visibility_allows(&post, viewer_id, policy)
            {
                visible.push(post);
            }
        }
        Ok(visible)
    }

    /// Every target needs a current policy from its own bounded request. An
    /// unsolicited response entry must not mask an omission in a later batch.
    async fn relationship_policies(
        &self,
        viewer_id: Uuid,
        target_ids: &[Uuid],
        stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<HashMap<Uuid, RelationshipPolicySnapshot>> {
        let mut policies = HashMap::with_capacity(target_ids.len());
        for chunk in target_ids.chunks(RELATIONSHIP_POLICY_BATCH_SIZE) {
            let mut batch = self
                .friend_client
                .batch_relationship_policies(viewer_id, chunk, stage)
                .await?;
            for target in chunk {
                let policy = batch.remove(target).ok_or_else(|| {
                    anyhow!("Friend service omitted a requested relationship policy")
                })?;
                policies.insert(*target, policy);
            }
        }
        Ok(policies)
    }

    async fn can_view(&self, post: &Post, viewer_id: Uuid) -> Result<bool> {
        if post.author_id == viewer_id {
            return Ok(true);
        }
        let stage = self
            .observe_owned_posts(
                viewer_id,
                OwnedQueryInputDto::Fixed {
                    post_ids: vec![post.id],
                },
                std::slice::from_ref(post),
            )
            .await?;
        let policy = self
            .friend_client
            .relationship_policy(viewer_id, post.author_id, stage.as_ref())
            .await?;
        Ok(!policy.is_blocked() && visibility_allows(post, viewer_id, policy))
    }

    pub(crate) async fn resolve_posts(&self, post_ids: &[Uuid]) -> Result<Vec<Post>> {
        let (cached, cache_available) = match self.post_cache.get_posts(post_ids).await {
            Ok(cached) => (cached, true),
            Err(error) => {
                tracing::warn!(error = ?error, "post cache read failed");
                (HashMap::new(), false)
            }
        };
        // Cache invalidation is asynchronous. Consult authoritative lifecycle,
        // moderation, author, and visibility state before reusing a cached body
        // so a stale cache entry cannot resurrect hidden or deleted content.
        let access = self.posts.fetch_post_access_snapshots(post_ids).await?;
        let missing: Vec<Uuid> = post_ids
            .iter()
            .copied()
            .filter(|id| access.contains_key(id) && !cached.contains_key(id))
            .collect();
        let mut fetched = if missing.is_empty() {
            Vec::new()
        } else {
            let posts = self.posts.fetch_posts_by_ids(&missing).await?;
            if cache_available {
                if let Err(error) = self.post_cache.set_posts(&posts).await {
                    tracing::warn!(error = ?error, "post cache backfill failed");
                }
            }
            posts
        };
        let mut merged = HashMap::with_capacity(post_ids.len());
        for post_id in post_ids {
            let Some(snapshot) = access.get(post_id) else {
                continue;
            };
            let Some(mut post) = cached.get(post_id).cloned() else {
                continue;
            };
            post.author_id = snapshot.author_id;
            post.visibility.clone_from(&snapshot.visibility);
            merged.insert(*post_id, post);
        }
        for post in fetched.drain(..) {
            merged.insert(post.id, post);
        }
        Ok(post_ids
            .iter()
            .filter_map(|id| merged.get(id).cloned())
            .collect())
    }

    pub(crate) async fn build_feed_view(
        &self,
        posts: Vec<Post>,
        next_cursor: Option<String>,
        viewer_id: Uuid,
    ) -> Result<FeedView> {
        let post_ids: Vec<Uuid> = posts.iter().map(|post| post.id).collect();
        let like_counts = self.likes.count_likes(&post_ids).await?;
        let liked_ids = self.likes.liked_post_ids(viewer_id, &post_ids).await?;
        Ok(FeedView {
            posts: posts
                .into_iter()
                .map(|post| PostView {
                    id: post.id,
                    author_id: post.author_id,
                    content: post.content,
                    visibility: post.visibility,
                    created_at: post.created_at,
                    comment_count: post.comment_count,
                    like_count: like_counts
                        .get(&post.id)
                        .copied()
                        .unwrap_or(post.like_count),
                    liked_by_viewer: liked_ids.contains(&post.id),
                })
                .collect(),
            next_cursor,
        })
    }

    async fn record_post_audit(
        &self,
        actor_id: Option<Uuid>,
        post_id: Option<Uuid>,
        action: &str,
        outcome: &str,
        request: &DeletePostDto,
        metadata_json: String,
    ) -> Result<()> {
        self.posts
            .record_audit_event(&self.build_post_audit_event(
                actor_id,
                post_id,
                action,
                outcome,
                request,
                metadata_json,
            ))
            .await
    }

    fn build_post_audit_event(
        &self,
        actor_id: Option<Uuid>,
        post_id: Option<Uuid>,
        action: &str,
        outcome: &str,
        request: &DeletePostDto,
        metadata_json: String,
    ) -> PostAuditEvent {
        PostAuditEvent {
            id: Uuid::new_v4(),
            actor_id,
            post_id,
            comment_id: None,
            action: action.to_owned(),
            outcome: outcome.to_owned(),
            request_id: optional_text(request.request_id.clone()),
            ip_address: optional_text(request.ip_address.clone()),
            user_agent: optional_text(request.user_agent.clone()),
            metadata_json,
            occurred_at: Utc::now(),
        }
    }

    async fn record_comment_audit(
        &self,
        actor_id: Option<Uuid>,
        post_id: Option<Uuid>,
        comment_id: Option<Uuid>,
        action: &str,
        outcome: &str,
        request: &DeleteCommentDto,
        metadata_json: String,
    ) -> Result<()> {
        self.posts
            .record_audit_event(&self.build_comment_audit_event(
                actor_id,
                post_id,
                comment_id,
                action,
                outcome,
                request,
                metadata_json,
            ))
            .await
    }

    fn build_comment_audit_event(
        &self,
        actor_id: Option<Uuid>,
        post_id: Option<Uuid>,
        comment_id: Option<Uuid>,
        action: &str,
        outcome: &str,
        request: &DeleteCommentDto,
        metadata_json: String,
    ) -> PostAuditEvent {
        PostAuditEvent {
            id: Uuid::new_v4(),
            actor_id,
            post_id,
            comment_id,
            action: action.to_owned(),
            outcome: outcome.to_owned(),
            request_id: optional_text(request.request_id.clone()),
            ip_address: optional_text(request.ip_address.clone()),
            user_agent: optional_text(request.user_agent.clone()),
            metadata_json,
            occurred_at: Utc::now(),
        }
    }
}

fn parse_uuid(value: &str, error: &str) -> Result<Uuid> {
    if value.trim().is_empty() {
        bail!(error.to_owned());
    }
    Uuid::parse_str(value.trim()).map_err(|_| anyhow::anyhow!(error.to_owned()))
}

fn required_content(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("Content is required");
    }
    Ok(trimmed.to_owned())
}

fn normalize_visibility(value: &str) -> Result<String> {
    let normalized = value.trim().to_uppercase();
    match normalized.as_str() {
        "PUBLIC" | "FRIENDS" | "PRIVATE" => Ok(normalized),
        _ => bail!("Visibility must be PUBLIC, FRIENDS, or PRIVATE"),
    }
}

fn canonical_post_cursor(cursor: Option<&str>) -> Result<Option<String>> {
    Ok(PostCursor::parse(cursor.unwrap_or_default())?.map(|cursor| cursor.encode()))
}

fn canonical_trending_cursor(cursor: Option<&str>) -> Result<Option<String>> {
    Ok(TrendingCursor::parse(cursor.unwrap_or_default())?.map(|cursor| cursor.encode()))
}

fn visibility_allows(post: &Post, viewer_id: Uuid, policy: RelationshipPolicySnapshot) -> bool {
    match post.visibility.as_str() {
        "PUBLIC" => true,
        "PRIVATE" => post.author_id == viewer_id,
        "FRIENDS" => post.author_id == viewer_id || policy.following,
        _ => false,
    }
}

fn normalize_limit(limit: i32) -> i32 {
    if limit > 0 {
        limit.min(100)
    } else {
        20
    }
}

fn optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn build_event(
    event_type: &str,
    aggregate_id: Uuid,
    actor_id: Option<Uuid>,
    attributes: Vec<String>,
    payload: String,
) -> StoredEvent {
    let now = Utc::now();
    StoredEvent {
        event_id: Uuid::new_v4(),
        event_type: event_type.to_owned(),
        aggregate_id,
        aggregate_version: None,
        origin_traceparent: None,
        actor_id,
        attributes,
        payload,
        occurred_at: now,
        created_at: now,
        published: false,
        published_at: None,
        replay_resolution_id: None,
    }
}
pub mod apply_user_deletion;
