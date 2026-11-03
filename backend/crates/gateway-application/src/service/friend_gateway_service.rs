use crate::dto::friend_dto::{
    FollowActionResponse, FollowCountsResponse, FollowListResponse, FollowRequestPageResponse,
    FollowRequestResponse, FollowStatusResponse, PeopleRecommendationEventRequest,
    PeopleRecommendationPageResponse, RecommendationEventResponse, RelationshipPolicyResponse,
};
use crate::dto::post_dto::MutationReceiptStatusDto;
use crate::ports::FriendClientPort;
use std::sync::Arc;

#[derive(Clone)]
pub struct FriendGatewayService {
    friend_client: Arc<dyn FriendClientPort>,
}

impl FriendGatewayService {
    pub fn new(friend_client: Arc<dyn FriendClientPort>) -> Self {
        Self { friend_client }
    }

    pub async fn get_mutation_receipt_status(
        &self,
        operation_id: uuid::Uuid,
    ) -> crate::service_error::GatewayServiceResult<Option<MutationReceiptStatusDto>> {
        self.friend_client
            .get_mutation_receipt_status(operation_id)
            .await
            .map_err(Into::into)
    }

    pub async fn follow(
        &self,
        follower_id: String,
        followee_id: String,
    ) -> crate::service_error::GatewayServiceResult<FollowActionResponse> {
        self.friend_client
            .follow(follower_id, followee_id)
            .await
            .map_err(Into::into)
    }

    pub async fn unfollow(
        &self,
        follower_id: String,
        followee_id: String,
    ) -> crate::service_error::GatewayServiceResult<FollowActionResponse> {
        self.friend_client
            .unfollow(follower_id, followee_id)
            .await
            .map_err(Into::into)
    }

    pub async fn list_followers(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: Option<i32>,
    ) -> crate::service_error::GatewayServiceResult<FollowListResponse> {
        self.friend_client
            .list_followers(user_id, cursor, limit)
            .await
            .map_err(Into::into)
    }

    pub async fn list_following(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: Option<i32>,
    ) -> crate::service_error::GatewayServiceResult<FollowListResponse> {
        self.friend_client
            .list_following(user_id, cursor, limit)
            .await
            .map_err(Into::into)
    }

    pub async fn counts(
        &self,
        user_id: String,
    ) -> crate::service_error::GatewayServiceResult<FollowCountsResponse> {
        self.friend_client
            .get_counts(user_id)
            .await
            .map_err(Into::into)
    }

    pub async fn status(
        &self,
        follower_id: String,
        followee_id: String,
    ) -> crate::service_error::GatewayServiceResult<FollowStatusResponse> {
        self.friend_client
            .check_following(follower_id, followee_id)
            .await
            .map_err(Into::into)
    }

    pub async fn accept_follow_request(
        &self,
        request_id: String,
        actor_id: String,
    ) -> crate::service_error::GatewayServiceResult<FollowActionResponse> {
        self.friend_client
            .accept_follow_request(request_id, actor_id)
            .await
            .map_err(Into::into)
    }

    pub async fn reject_follow_request(
        &self,
        request_id: String,
        actor_id: String,
    ) -> crate::service_error::GatewayServiceResult<FollowRequestResponse> {
        self.friend_client
            .reject_follow_request(request_id, actor_id)
            .await
            .map_err(Into::into)
    }

    pub async fn cancel_follow_request(
        &self,
        request_id: String,
        actor_id: String,
    ) -> crate::service_error::GatewayServiceResult<FollowRequestResponse> {
        self.friend_client
            .cancel_follow_request(request_id, actor_id)
            .await
            .map_err(Into::into)
    }

    pub async fn list_incoming_follow_requests(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::service_error::GatewayServiceResult<FollowRequestPageResponse> {
        self.friend_client
            .list_incoming_follow_requests(user_id, cursor, limit)
            .await
            .map_err(Into::into)
    }

    pub async fn list_outgoing_follow_requests(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::service_error::GatewayServiceResult<FollowRequestPageResponse> {
        self.friend_client
            .list_outgoing_follow_requests(user_id, cursor, limit)
            .await
            .map_err(Into::into)
    }

    pub async fn block(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> crate::service_error::GatewayServiceResult<RelationshipPolicyResponse> {
        self.friend_client
            .block_user(viewer_id, target_id)
            .await
            .map_err(Into::into)
    }

    pub async fn unblock(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> crate::service_error::GatewayServiceResult<RelationshipPolicyResponse> {
        self.friend_client
            .unblock_user(viewer_id, target_id)
            .await
            .map_err(Into::into)
    }

    pub async fn mute(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> crate::service_error::GatewayServiceResult<RelationshipPolicyResponse> {
        self.friend_client
            .mute_user(viewer_id, target_id)
            .await
            .map_err(Into::into)
    }

    pub async fn unmute(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> crate::service_error::GatewayServiceResult<RelationshipPolicyResponse> {
        self.friend_client
            .unmute_user(viewer_id, target_id)
            .await
            .map_err(Into::into)
    }

    pub async fn people_recommendations(
        &self,
        viewer_id: String,
        cursor: Option<String>,
        limit: i32,
        request_id: String,
    ) -> crate::service_error::GatewayServiceResult<PeopleRecommendationPageResponse> {
        self.friend_client
            .get_people_recommendations(viewer_id, cursor, limit, request_id)
            .await
            .map_err(Into::into)
    }

    pub async fn record_people_recommendation_event(
        &self,
        viewer_id: String,
        request_id: String,
        event: PeopleRecommendationEventRequest,
    ) -> crate::service_error::GatewayServiceResult<RecommendationEventResponse> {
        self.friend_client
            .record_people_recommendation_event(viewer_id, request_id, event)
            .await
            .map_err(Into::into)
    }

    pub async fn relationship(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> crate::service_error::GatewayServiceResult<RelationshipPolicyResponse> {
        self.friend_client
            .get_relationship(viewer_id, target_id)
            .await
            .map_err(Into::into)
    }
}
