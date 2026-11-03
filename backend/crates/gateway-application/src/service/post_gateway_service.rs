use crate::dto::post_dto::{
    CommentCreateRequest, CommentResponse, DeletePostResponse, FeedRecommendationEventRequest,
    FeedRecommendationEventResponse, FeedResponse, MutationReceiptStatusDto, PostCreateRequest,
    PostDetailResponse, PostLikeResponse, PostResponse, RankedFeedResponse,
    ResolveDeadLetterReplayRequestDto, ResolveDeadLetterReplayResponseDto,
};
use crate::ports::PostClientPort;
use crate::search_cursor::{SearchCursorCodec, SearchSort};
use std::sync::Arc;

#[derive(Clone)]
pub struct PostGatewayService {
    post_client: Arc<dyn PostClientPort>,
    search_cursor_codec: SearchCursorCodec,
}

impl PostGatewayService {
    pub fn new(
        post_client: Arc<dyn PostClientPort>,
        search_cursor_codec: SearchCursorCodec,
    ) -> Self {
        Self {
            post_client,
            search_cursor_codec,
        }
    }

    pub async fn get_mutation_receipt_status(
        &self,
        operation_id: uuid::Uuid,
    ) -> crate::service_error::GatewayServiceResult<Option<MutationReceiptStatusDto>> {
        self.post_client
            .as_ref()
            .get_mutation_receipt_status(operation_id)
            .await
            .map_err(Into::into)
    }

    pub async fn resolve_dead_letter_replay(
        &self,
        request: ResolveDeadLetterReplayRequestDto,
    ) -> crate::service_error::GatewayServiceResult<ResolveDeadLetterReplayResponseDto> {
        self.post_client
            .resolve_dead_letter_replay(request)
            .await
            .map_err(Into::into)
    }

    pub async fn get_feed(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::service_error::GatewayServiceResult<FeedResponse> {
        self.post_client
            .get_feed(user_id, cursor, limit)
            .await
            .map_err(Into::into)
    }

    pub async fn get_ranked_feed(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
        request_id: String,
        session_id: Option<String>,
    ) -> crate::service_error::GatewayServiceResult<RankedFeedResponse> {
        self.post_client
            .get_ranked_feed(user_id, cursor, limit, request_id, session_id)
            .await
            .map_err(Into::into)
    }

    pub async fn record_feed_recommendation_event(
        &self,
        viewer_id: String,
        request_id: String,
        event: FeedRecommendationEventRequest,
    ) -> crate::service_error::GatewayServiceResult<FeedRecommendationEventResponse> {
        self.post_client
            .record_feed_recommendation_event(viewer_id, request_id, event)
            .await
            .map_err(Into::into)
    }

    pub async fn get_author_posts(
        &self,
        author_id: String,
        viewer_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::service_error::GatewayServiceResult<FeedResponse> {
        self.post_client
            .get_author_posts(author_id, viewer_id, cursor, limit)
            .await
            .map_err(Into::into)
    }

    pub async fn search_posts(
        &self,
        viewer_id: String,
        query: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::service_error::GatewayServiceResult<FeedResponse> {
        let sort = SearchSort::PostsCreatedAtDescIdDescV1;
        let position = self
            .search_cursor_codec
            .decode(cursor.as_deref(), &query, Some(&viewer_id), sort)
            .map_err(crate::service_error::GatewayServiceError::from)?;
        let mut response = self
            .post_client
            .search_posts(viewer_id.clone(), query.clone(), position, limit)
            .await?;
        response.next_cursor = self.search_cursor_codec.encode(
            response.next_cursor.as_deref(),
            &query,
            Some(&viewer_id),
            sort,
        )?;
        Ok(response)
    }

    pub async fn get_trending(
        &self,
        viewer_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::service_error::GatewayServiceResult<FeedResponse> {
        self.post_client
            .get_trending(viewer_id, cursor, limit)
            .await
            .map_err(Into::into)
    }

    pub async fn get_post(
        &self,
        viewer_id: String,
        post_id: String,
    ) -> crate::service_error::GatewayServiceResult<PostDetailResponse> {
        self.post_client
            .get_post(viewer_id, post_id)
            .await
            .map_err(Into::into)
    }

    pub async fn create_post(
        &self,
        user_id: String,
        request: PostCreateRequest,
    ) -> crate::service_error::GatewayServiceResult<PostResponse> {
        self.post_client
            .create_post(user_id, request)
            .await
            .map_err(Into::into)
    }

    pub async fn delete_post(
        &self,
        user_id: String,
        post_id: String,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> crate::service_error::GatewayServiceResult<DeletePostResponse> {
        self.post_client
            .delete_post(user_id, post_id, request_id, ip_address, user_agent)
            .await
            .map_err(Into::into)
    }

    pub async fn create_comment(
        &self,
        user_id: String,
        post_id: String,
        request: CommentCreateRequest,
    ) -> crate::service_error::GatewayServiceResult<CommentResponse> {
        self.post_client
            .create_comment(user_id, post_id, request)
            .await
            .map_err(Into::into)
    }

    pub async fn like_post(
        &self,
        user_id: String,
        post_id: String,
    ) -> crate::service_error::GatewayServiceResult<PostLikeResponse> {
        self.post_client
            .like_post(user_id, post_id)
            .await
            .map_err(Into::into)
    }

    pub async fn unlike_post(
        &self,
        user_id: String,
        post_id: String,
    ) -> crate::service_error::GatewayServiceResult<PostLikeResponse> {
        self.post_client
            .unlike_post(user_id, post_id)
            .await
            .map_err(Into::into)
    }
}
