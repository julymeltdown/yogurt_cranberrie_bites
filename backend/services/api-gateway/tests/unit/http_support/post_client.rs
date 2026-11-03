use super::{
    ContentModerationCall, DeletePostCall, FeedRecommendationEventCall, TestClients, USER_ID,
};
use api_gateway::adapters::out::post_client::classify_post_client_error;
use api_gateway::application::dto::post_dto::{
    CommentCreateRequest, CommentResponse, DeletePostResponse, FeedRecommendationEventRequest,
    FeedRecommendationEventResponse, FeedRecommendationMetadataResponse, FeedResponse,
    PostCreateRequest, PostDetailResponse, PostLikeResponse, PostResponse, RankedFeedResponse,
    ResolveDeadLetterReplayRequestDto, ResolveDeadLetterReplayResponseDto,
};
use api_gateway::application::ports::PostClientPort;
use async_trait::async_trait;

impl TestClients {
    async fn notification_posts_visible_inner(
        &self,
        viewer_id: String,
        post_ids: Vec<String>,
        max_duration: std::time::Duration,
    ) -> anyhow::Result<std::collections::HashMap<String, bool>> {
        self.notification_authorization_budgets
            .lock()
            .unwrap()
            .push(max_duration);
        self.notification_authorization_batches
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let delay = *self.notification_authorization_delay.lock().unwrap();
        if tokio::time::timeout(max_duration, tokio::time::sleep(delay))
            .await
            .is_err()
        {
            return Err(tonic::Status::deadline_exceeded("Request timed out").into());
        }
        if self
            .notification_post_unavailable
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Err(tonic::Status::unavailable("post owner unavailable").into());
        }
        let mut decisions = std::collections::HashMap::new();
        for post_id in post_ids {
            self.notification_post_checks
                .lock()
                .unwrap()
                .push((viewer_id.clone(), post_id.clone()));
            decisions.insert(
                post_id.clone(),
                self.notification_post_access
                    .lock()
                    .unwrap()
                    .get(&post_id)
                    .copied()
                    .unwrap_or(true),
            );
        }
        Ok(decisions)
    }

    async fn get_author_post_counts_inner(
        &self,
        _author_ids: Vec<String>,
    ) -> anyhow::Result<std::collections::HashMap<String, i64>> {
        Ok(std::collections::HashMap::new())
    }

    async fn get_feed_inner(
        &self,
        _user_id: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> anyhow::Result<FeedResponse> {
        Ok(feed(cursor))
    }

    async fn get_ranked_feed_inner(
        &self,
        _user_id: String,
        cursor: Option<String>,
        _limit: i32,
        request_id: String,
        _session_id: Option<String>,
    ) -> anyhow::Result<RankedFeedResponse> {
        Ok(RankedFeedResponse {
            posts: vec![post("post-1")],
            next_cursor: cursor,
            snapshot_id: Some("11111111-1111-1111-1111-111111111111".into()),
            rank_version: Some("feed-deterministic-v1".into()),
            request_id: Some(request_id),
            fallback_used: false,
            recommendation_metadata: vec![FeedRecommendationMetadataResponse {
                post_id: "post-1".into(),
                source: "MATERIALIZED".into(),
                reason_code: "FOLLOWING_POST".into(),
                score_micros: 900_000,
                rank_version: "feed-deterministic-v1".into(),
                impression_id: "22222222-2222-2222-2222-222222222222".into(),
                position: 0,
                propensity_micros: 1_000_000,
                exploration: false,
            }],
            failed_sources: Vec::new(),
        })
    }

    async fn record_feed_recommendation_event_inner(
        &self,
        viewer_id: String,
        request_id: String,
        event: FeedRecommendationEventRequest,
    ) -> anyhow::Result<FeedRecommendationEventResponse> {
        self.feed_recommendation_event_calls
            .lock()
            .unwrap()
            .push(FeedRecommendationEventCall {
                viewer_id,
                request_id,
                post_id: event.post_id,
                impression_id: event.impression_id,
                action: event.action,
            });
        Ok(FeedRecommendationEventResponse { accepted: true })
    }

    async fn get_author_posts_inner(
        &self,
        _author_id: String,
        _viewer_id: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> anyhow::Result<FeedResponse> {
        Ok(feed(cursor))
    }

    async fn search_posts_inner(
        &self,
        _viewer_id: String,
        _query: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> anyhow::Result<FeedResponse> {
        Ok(feed(cursor))
    }

    async fn get_trending_inner(
        &self,
        _viewer_id: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> anyhow::Result<FeedResponse> {
        Ok(feed(cursor))
    }

    async fn get_post_inner(
        &self,
        _viewer_id: String,
        post_id: String,
    ) -> anyhow::Result<PostDetailResponse> {
        Ok(PostDetailResponse {
            post: post(&post_id),
            comments: Vec::new(),
        })
    }

    async fn create_post_inner(
        &self,
        user_id: String,
        request: PostCreateRequest,
    ) -> anyhow::Result<PostResponse> {
        self.create_post_calls.lock().unwrap().push(user_id.clone());
        Ok(PostResponse {
            id: "post-new".into(),
            author_id: user_id,
            content: request.content,
            visibility: request.visibility,
            created_at: "2026-03-12T00:00:00Z".into(),
            comment_count: 0,
            like_count: 0,
            liked_by_me: false,
        })
    }

    async fn delete_post_inner(
        &self,
        user_id: String,
        post_id: String,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> anyhow::Result<DeletePostResponse> {
        self.delete_post_calls
            .lock()
            .expect("delete post calls")
            .push(DeletePostCall {
                user_id,
                post_id: post_id.clone(),
                request_id,
                ip_address,
                user_agent,
            });
        Ok(DeletePostResponse {
            post_id,
            deleted: true,
        })
    }

    async fn create_comment_inner(
        &self,
        user_id: String,
        post_id: String,
        request: CommentCreateRequest,
    ) -> anyhow::Result<CommentResponse> {
        Ok(CommentResponse {
            id: "comment-new".into(),
            post_id,
            author_id: user_id,
            content: request.content,
            created_at: "2026-03-12T00:00:00Z".into(),
        })
    }

    async fn like_post_inner(
        &self,
        _user_id: String,
        post_id: String,
    ) -> anyhow::Result<PostLikeResponse> {
        Ok(PostLikeResponse {
            post_id,
            like_count: 1,
            liked: true,
        })
    }

    async fn unlike_post_inner(
        &self,
        _user_id: String,
        post_id: String,
    ) -> anyhow::Result<PostLikeResponse> {
        Ok(PostLikeResponse {
            post_id,
            like_count: 0,
            liked: false,
        })
    }

    async fn apply_content_moderation_inner(
        &self,
        report_id: String,
        target_type: String,
        target_id: String,
        actor_id: String,
        action: String,
    ) -> anyhow::Result<bool> {
        self.content_moderation_calls
            .lock()
            .unwrap()
            .push(ContentModerationCall {
                report_id,
                target_type,
                target_id,
                actor_id,
                action,
            });
        Ok(self
            .content_moderation_result
            .lock()
            .unwrap()
            .unwrap_or(true))
    }

    async fn resolve_dead_letter_replay_inner(
        &self,
        request: ResolveDeadLetterReplayRequestDto,
    ) -> anyhow::Result<ResolveDeadLetterReplayResponseDto> {
        let response = ResolveDeadLetterReplayResponseDto {
            event_id: request.event_id.clone(),
            resolution_id: request.resolution_id.clone(),
            created: true,
        };
        self.dead_letter_replay_calls.lock().unwrap().push(request);
        Ok(response)
    }

    // Explicit recording behavior formerly inherited from the port.
    async fn get_mutation_receipt_status_inner(
        &self,
        _operation_id: uuid::Uuid,
    ) -> anyhow::Result<Option<api_gateway::application::dto::post_dto::MutationReceiptStatusDto>>
    {
        anyhow::bail!("Owner mutation status lookup is unavailable")
    }
}

#[async_trait]
impl PostClientPort for TestClients {
    async fn notification_posts_visible(
        &self,
        viewer_id: String,
        post_ids: Vec<String>,
        max_duration: std::time::Duration,
    ) -> api_gateway::application::post_client_error::PostClientResult<
        std::collections::HashMap<String, bool>,
    > {
        self.notification_posts_visible_inner(viewer_id, post_ids, max_duration)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_author_post_counts(
        &self,
        _author_ids: Vec<String>,
    ) -> api_gateway::application::post_client_error::PostClientResult<
        std::collections::HashMap<String, i64>,
    > {
        self.get_author_post_counts_inner(_author_ids)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_feed(
        &self,
        _user_id: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> api_gateway::application::post_client_error::PostClientResult<FeedResponse> {
        self.get_feed_inner(_user_id, cursor, _limit)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_ranked_feed(
        &self,
        _user_id: String,
        cursor: Option<String>,
        _limit: i32,
        request_id: String,
        _session_id: Option<String>,
    ) -> api_gateway::application::post_client_error::PostClientResult<RankedFeedResponse> {
        self.get_ranked_feed_inner(_user_id, cursor, _limit, request_id, _session_id)
            .await
            .map_err(classify_post_client_error)
    }
    async fn record_feed_recommendation_event(
        &self,
        viewer_id: String,
        request_id: String,
        event: FeedRecommendationEventRequest,
    ) -> api_gateway::application::post_client_error::PostClientResult<
        FeedRecommendationEventResponse,
    > {
        self.record_feed_recommendation_event_inner(viewer_id, request_id, event)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_author_posts(
        &self,
        _author_id: String,
        _viewer_id: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> api_gateway::application::post_client_error::PostClientResult<FeedResponse> {
        self.get_author_posts_inner(_author_id, _viewer_id, cursor, _limit)
            .await
            .map_err(classify_post_client_error)
    }
    async fn search_posts(
        &self,
        _viewer_id: String,
        _query: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> api_gateway::application::post_client_error::PostClientResult<FeedResponse> {
        self.search_posts_inner(_viewer_id, _query, cursor, _limit)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_trending(
        &self,
        _viewer_id: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> api_gateway::application::post_client_error::PostClientResult<FeedResponse> {
        self.get_trending_inner(_viewer_id, cursor, _limit)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_post(
        &self,
        _viewer_id: String,
        post_id: String,
    ) -> api_gateway::application::post_client_error::PostClientResult<PostDetailResponse> {
        self.get_post_inner(_viewer_id, post_id)
            .await
            .map_err(classify_post_client_error)
    }
    async fn create_post(
        &self,
        user_id: String,
        request: PostCreateRequest,
    ) -> api_gateway::application::post_client_error::PostClientResult<PostResponse> {
        self.create_post_inner(user_id, request)
            .await
            .map_err(classify_post_client_error)
    }
    async fn delete_post(
        &self,
        user_id: String,
        post_id: String,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> api_gateway::application::post_client_error::PostClientResult<DeletePostResponse> {
        self.delete_post_inner(user_id, post_id, request_id, ip_address, user_agent)
            .await
            .map_err(classify_post_client_error)
    }
    async fn create_comment(
        &self,
        user_id: String,
        post_id: String,
        request: CommentCreateRequest,
    ) -> api_gateway::application::post_client_error::PostClientResult<CommentResponse> {
        self.create_comment_inner(user_id, post_id, request)
            .await
            .map_err(classify_post_client_error)
    }
    async fn like_post(
        &self,
        _user_id: String,
        post_id: String,
    ) -> api_gateway::application::post_client_error::PostClientResult<PostLikeResponse> {
        self.like_post_inner(_user_id, post_id)
            .await
            .map_err(classify_post_client_error)
    }
    async fn unlike_post(
        &self,
        _user_id: String,
        post_id: String,
    ) -> api_gateway::application::post_client_error::PostClientResult<PostLikeResponse> {
        self.unlike_post_inner(_user_id, post_id)
            .await
            .map_err(classify_post_client_error)
    }
    async fn apply_content_moderation(
        &self,
        report_id: String,
        target_type: String,
        target_id: String,
        actor_id: String,
        action: String,
    ) -> api_gateway::application::post_client_error::PostClientResult<bool> {
        self.apply_content_moderation_inner(report_id, target_type, target_id, actor_id, action)
            .await
            .map_err(classify_post_client_error)
    }
    async fn resolve_dead_letter_replay(
        &self,
        request: ResolveDeadLetterReplayRequestDto,
    ) -> api_gateway::application::post_client_error::PostClientResult<
        ResolveDeadLetterReplayResponseDto,
    > {
        self.resolve_dead_letter_replay_inner(request)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_mutation_receipt_status(
        &self,
        _operation_id: uuid::Uuid,
    ) -> api_gateway::application::post_client_error::PostClientResult<
        Option<api_gateway::application::dto::post_dto::MutationReceiptStatusDto>,
    > {
        self.get_mutation_receipt_status_inner(_operation_id)
            .await
            .map_err(classify_post_client_error)
    }
}

fn feed(cursor: Option<String>) -> FeedResponse {
    FeedResponse {
        posts: vec![post("post-1")],
        next_cursor: cursor,
    }
}

fn post(post_id: &str) -> PostResponse {
    PostResponse {
        id: post_id.into(),
        author_id: USER_ID.into(),
        content: "hello".into(),
        visibility: "PUBLIC".into(),
        created_at: "2026-03-12T00:00:00Z".into(),
        comment_count: 0,
        like_count: 0,
        liked_by_me: false,
    }
}
