use super::{
    PeopleRecommendationCall, PeopleRecommendationEventCall, RelationshipCall, TestClients,
};
use api_gateway::application::dto::friend_dto::{
    FollowActionResponse, FollowCountsResponse, FollowEdgeResponse, FollowListResponse,
    FollowRequestPageResponse, FollowRequestResponse, FollowStatusResponse,
    PeopleRecommendationEventRequest, PeopleRecommendationPageResponse,
    PeopleRecommendationResponse, RecommendationEventResponse, RelationshipPolicyResponse,
};
use api_gateway::application::dto::notification_dto::{
    NotificationFeedResponse, NotificationMarkAllReadResponse, NotificationResponse,
};
use api_gateway::application::ports::{FriendClientPort, NotificationClientPort};
use async_trait::async_trait;
use std::collections::HashMap;

impl TestClients {
    async fn follow_friend_client_port_inner(
        &self,
        _follower_id: String,
        _followee_id: String,
    ) -> anyhow::Result<FollowActionResponse> {
        Ok(FollowActionResponse {
            following: true,
            followed_at: Some("2026-03-12T00:00:00Z".into()),
            status: "ACTIVE".into(),
            request_id: None,
        })
    }

    async fn unfollow_friend_client_port_inner(
        &self,
        _follower_id: String,
        _followee_id: String,
    ) -> anyhow::Result<FollowActionResponse> {
        Ok(FollowActionResponse {
            following: false,
            followed_at: None,
            status: "INACTIVE".into(),
            request_id: None,
        })
    }

    async fn list_followers_friend_client_port_inner(
        &self,
        _user_id: String,
        cursor: Option<String>,
        _limit: Option<i32>,
    ) -> anyhow::Result<FollowListResponse> {
        Ok(follow_list(cursor))
    }

    async fn list_following_friend_client_port_inner(
        &self,
        _user_id: String,
        cursor: Option<String>,
        _limit: Option<i32>,
    ) -> anyhow::Result<FollowListResponse> {
        Ok(follow_list(cursor))
    }

    async fn get_counts_friend_client_port_inner(
        &self,
        _user_id: String,
    ) -> anyhow::Result<FollowCountsResponse> {
        Ok(FollowCountsResponse {
            follower_count: 12,
            following_count: 7,
        })
    }

    async fn get_counts_batch_friend_client_port_inner(
        &self,
        user_ids: Vec<String>,
    ) -> anyhow::Result<HashMap<String, FollowCountsResponse>> {
        Ok(user_ids
            .into_iter()
            .map(|user_id| {
                (
                    user_id,
                    FollowCountsResponse {
                        follower_count: 12,
                        following_count: 7,
                    },
                )
            })
            .collect())
    }

    async fn check_following_friend_client_port_inner(
        &self,
        _follower_id: String,
        _followee_id: String,
    ) -> anyhow::Result<FollowStatusResponse> {
        Ok(FollowStatusResponse { following: true })
    }

    async fn accept_follow_request_friend_client_port_inner(
        &self,
        request_id: String,
        _actor_id: String,
    ) -> anyhow::Result<FollowActionResponse> {
        Ok(FollowActionResponse {
            following: true,
            followed_at: Some("2026-03-12T00:00:00Z".into()),
            status: "ACTIVE".into(),
            request_id: Some(request_id),
        })
    }

    async fn reject_follow_request_friend_client_port_inner(
        &self,
        request_id: String,
        _actor_id: String,
    ) -> anyhow::Result<FollowRequestResponse> {
        Ok(request(&request_id, "REJECTED"))
    }

    async fn cancel_follow_request_friend_client_port_inner(
        &self,
        request_id: String,
        _actor_id: String,
    ) -> anyhow::Result<FollowRequestResponse> {
        Ok(request(&request_id, "CANCELLED"))
    }

    async fn list_incoming_follow_requests_friend_client_port_inner(
        &self,
        _user_id: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> anyhow::Result<FollowRequestPageResponse> {
        Ok(FollowRequestPageResponse {
            requests: vec![request("incoming-1", "PENDING")],
            next_cursor: cursor,
        })
    }

    async fn list_outgoing_follow_requests_friend_client_port_inner(
        &self,
        _user_id: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> anyhow::Result<FollowRequestPageResponse> {
        Ok(FollowRequestPageResponse {
            requests: vec![request("outgoing-1", "PENDING")],
            next_cursor: cursor,
        })
    }

    async fn block_user_friend_client_port_inner(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> anyhow::Result<RelationshipPolicyResponse> {
        self.relationship_calls
            .lock()
            .unwrap()
            .push(RelationshipCall {
                operation: "block".into(),
                viewer_id,
                target_id,
            });
        Ok(RelationshipPolicyResponse {
            blocked_by_viewer: true,
            ..RelationshipPolicyResponse::default()
        })
    }

    async fn unblock_user_friend_client_port_inner(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> anyhow::Result<RelationshipPolicyResponse> {
        self.relationship_calls
            .lock()
            .unwrap()
            .push(RelationshipCall {
                operation: "unblock".into(),
                viewer_id,
                target_id,
            });
        Ok(RelationshipPolicyResponse::default())
    }

    async fn mute_user_friend_client_port_inner(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> anyhow::Result<RelationshipPolicyResponse> {
        self.relationship_calls
            .lock()
            .unwrap()
            .push(RelationshipCall {
                operation: "mute".into(),
                viewer_id,
                target_id,
            });
        Ok(RelationshipPolicyResponse {
            muted_by_viewer: true,
            ..RelationshipPolicyResponse::default()
        })
    }

    async fn unmute_user_friend_client_port_inner(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> anyhow::Result<RelationshipPolicyResponse> {
        self.relationship_calls
            .lock()
            .unwrap()
            .push(RelationshipCall {
                operation: "unmute".into(),
                viewer_id,
                target_id,
            });
        Ok(RelationshipPolicyResponse::default())
    }

    async fn get_relationship_friend_client_port_inner(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> anyhow::Result<RelationshipPolicyResponse> {
        self.relationship_calls
            .lock()
            .unwrap()
            .push(RelationshipCall {
                operation: "get".into(),
                viewer_id,
                target_id,
            });
        Ok(RelationshipPolicyResponse {
            following: true,
            ..RelationshipPolicyResponse::default()
        })
    }

    async fn get_people_recommendations_friend_client_port_inner(
        &self,
        viewer_id: String,
        cursor: Option<String>,
        limit: i32,
        request_id: String,
    ) -> anyhow::Result<PeopleRecommendationPageResponse> {
        self.people_recommendation_calls
            .lock()
            .unwrap()
            .push(PeopleRecommendationCall {
                viewer_id,
                cursor: cursor.clone(),
                limit,
                request_id: request_id.clone(),
            });
        Ok(PeopleRecommendationPageResponse {
            items: vec![PeopleRecommendationResponse {
                user_id: "22222222-2222-2222-2222-222222222222".into(),
                username: "neighbor".into(),
                nickname: "Neighbor".into(),
                bio: "Suggested from the social graph".into(),
                avatar_url: "/avatars/neighbor.png".into(),
                source: "SOCIAL_GRAPH".into(),
                reason_code: "MUTUAL_FOLLOWS".into(),
                score_micros: 900_000,
                rank_version: "people-v1".into(),
                impression_id: "33333333-3333-3333-3333-333333333333".into(),
            }],
            next_cursor: cursor,
            snapshot_id: "people-snapshot-1".into(),
            rank_version: "people-v1".into(),
            request_id,
            fallback_used: false,
        })
    }

    async fn record_people_recommendation_event_friend_client_port_inner(
        &self,
        viewer_id: String,
        request_id: String,
        event: PeopleRecommendationEventRequest,
    ) -> anyhow::Result<RecommendationEventResponse> {
        self.people_recommendation_event_calls.lock().unwrap().push(
            PeopleRecommendationEventCall {
                viewer_id,
                request_id,
                candidate_user_id: event.candidate_user_id,
                action: event.action,
            },
        );
        Ok(RecommendationEventResponse { accepted: true })
    }

    // Explicit recording behavior formerly inherited from the port.
    async fn get_mutation_receipt_status_friend_client_port_inner(
        &self,
        _operation_id: uuid::Uuid,
    ) -> anyhow::Result<Option<api_gateway::application::dto::post_dto::MutationReceiptStatusDto>>
    {
        anyhow::bail!("Friend mutation status lookup is unavailable")
    }

    async fn get_relationships_batch_friend_client_port_inner(
        &self,
        viewer_id: String,
        target_ids: Vec<String>,
    ) -> anyhow::Result<
        std::collections::HashMap<
            String,
            api_gateway::application::dto::friend_dto::RelationshipPolicyResponse,
        >,
    > {
        if target_ids.len() > 100 {
            anyhow::bail!("Relationship batch supports at most 100 user IDs");
        }
        let mut policies = std::collections::HashMap::new();
        for target_id in target_ids {
            let policy = self
                .get_relationship(viewer_id.clone(), target_id.clone())
                .await?;
            policies.insert(target_id, policy);
        }
        Ok(policies)
    }
}

#[async_trait]
impl FriendClientPort for TestClients {
    async fn follow(
        &self,
        _follower_id: String,
        _followee_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowActionResponse> {
        self.follow_friend_client_port_inner(_follower_id, _followee_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn unfollow(
        &self,
        _follower_id: String,
        _followee_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowActionResponse> {
        self.unfollow_friend_client_port_inner(_follower_id, _followee_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn list_followers(
        &self,
        _user_id: String,
        cursor: Option<String>,
        _limit: Option<i32>,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowListResponse> {
        self.list_followers_friend_client_port_inner(_user_id, cursor, _limit)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn list_following(
        &self,
        _user_id: String,
        cursor: Option<String>,
        _limit: Option<i32>,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowListResponse> {
        self.list_following_friend_client_port_inner(_user_id, cursor, _limit)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_counts(
        &self,
        _user_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowCountsResponse> {
        self.get_counts_friend_client_port_inner(_user_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_counts_batch(
        &self,
        user_ids: Vec<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        HashMap<String, FollowCountsResponse>,
    > {
        self.get_counts_batch_friend_client_port_inner(user_ids)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn check_following(
        &self,
        _follower_id: String,
        _followee_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowStatusResponse> {
        self.check_following_friend_client_port_inner(_follower_id, _followee_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn accept_follow_request(
        &self,
        request_id: String,
        _actor_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowActionResponse> {
        self.accept_follow_request_friend_client_port_inner(request_id, _actor_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn reject_follow_request(
        &self,
        request_id: String,
        _actor_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowRequestResponse> {
        self.reject_follow_request_friend_client_port_inner(request_id, _actor_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn cancel_follow_request(
        &self,
        request_id: String,
        _actor_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowRequestResponse> {
        self.cancel_follow_request_friend_client_port_inner(request_id, _actor_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn list_incoming_follow_requests(
        &self,
        _user_id: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowRequestPageResponse>
    {
        self.list_incoming_follow_requests_friend_client_port_inner(_user_id, cursor, _limit)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn list_outgoing_follow_requests(
        &self,
        _user_id: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowRequestPageResponse>
    {
        self.list_outgoing_follow_requests_friend_client_port_inner(_user_id, cursor, _limit)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn block_user(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<RelationshipPolicyResponse>
    {
        self.block_user_friend_client_port_inner(viewer_id, target_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn unblock_user(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<RelationshipPolicyResponse>
    {
        self.unblock_user_friend_client_port_inner(viewer_id, target_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn mute_user(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<RelationshipPolicyResponse>
    {
        self.mute_user_friend_client_port_inner(viewer_id, target_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn unmute_user(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<RelationshipPolicyResponse>
    {
        self.unmute_user_friend_client_port_inner(viewer_id, target_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_relationship(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<RelationshipPolicyResponse>
    {
        self.get_relationship_friend_client_port_inner(viewer_id, target_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_people_recommendations(
        &self,
        viewer_id: String,
        cursor: Option<String>,
        limit: i32,
        request_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<PeopleRecommendationPageResponse>
    {
        self.get_people_recommendations_friend_client_port_inner(
            viewer_id, cursor, limit, request_id,
        )
        .await
        .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn record_people_recommendation_event(
        &self,
        viewer_id: String,
        request_id: String,
        event: PeopleRecommendationEventRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<RecommendationEventResponse>
    {
        self.record_people_recommendation_event_friend_client_port_inner(
            viewer_id, request_id, event,
        )
        .await
        .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_mutation_receipt_status(
        &self,
        _operation_id: uuid::Uuid,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        Option<api_gateway::application::dto::post_dto::MutationReceiptStatusDto>,
    > {
        self.get_mutation_receipt_status_friend_client_port_inner(_operation_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_relationships_batch(
        &self,
        viewer_id: String,
        target_ids: Vec<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        std::collections::HashMap<
            String,
            api_gateway::application::dto::friend_dto::RelationshipPolicyResponse,
        >,
    > {
        self.get_relationships_batch_friend_client_port_inner(viewer_id, target_ids)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
}

impl TestClients {
    async fn list_notifications_notification_client_port_inner(
        &self,
        user_id: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> anyhow::Result<NotificationFeedResponse> {
        Ok(NotificationFeedResponse {
            items: self
                .notification_items
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| vec![notification(&user_id, "notification-1")]),
            next_cursor: cursor,
        })
    }

    async fn mark_read_notification_client_port_inner(
        &self,
        user_id: String,
        notification_id: String,
    ) -> anyhow::Result<NotificationResponse> {
        let delay = *self.notification_mark_read_delay.lock().unwrap();
        tokio::time::sleep(delay).await;
        self.notification_mark_read_calls
            .lock()
            .unwrap()
            .push((user_id.clone(), notification_id.clone()));
        let mut response = self
            .notification_items
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|items| items.iter().find(|item| item.id == notification_id))
            .cloned()
            .unwrap_or_else(|| notification(&user_id, &notification_id));
        response.read_at = "2026-10-11T00:00:00Z".into();
        Ok(response)
    }

    async fn mark_all_read_notification_client_port_inner(
        &self,
        _user_id: String,
    ) -> anyhow::Result<NotificationMarkAllReadResponse> {
        Ok(NotificationMarkAllReadResponse { updated_count: 1 })
    }
}

#[async_trait]
impl NotificationClientPort for TestClients {
    async fn list_notifications(
        &self,
        user_id: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> api_gateway::application::client_error::GatewayClientResult<NotificationFeedResponse> {
        self.list_notifications_notification_client_port_inner(user_id, cursor, _limit)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn mark_read(
        &self,
        user_id: String,
        notification_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<NotificationResponse> {
        self.mark_read_notification_client_port_inner(user_id, notification_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn mark_all_read(
        &self,
        _user_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<NotificationMarkAllReadResponse>
    {
        self.mark_all_read_notification_client_port_inner(_user_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
}

fn follow_list(cursor: Option<String>) -> FollowListResponse {
    FollowListResponse {
        edges: vec![FollowEdgeResponse {
            user_id: "22222222-2222-2222-2222-222222222222".into(),
            followed_at: "2026-03-12T00:00:00Z".into(),
        }],
        next_cursor: cursor,
        total_count: 1,
    }
}

fn notification(user_id: &str, id: &str) -> NotificationResponse {
    NotificationResponse {
        id: id.into(),
        user_id: user_id.into(),
        notification_type: "POST_CREATED".into(),
        payload_json: r#"{"postId":"post-1"}"#.into(),
        created_at: "2026-03-12T00:00:00Z".into(),
        read_at: String::new(),
    }
}

fn request(id: &str, status: &str) -> FollowRequestResponse {
    FollowRequestResponse {
        id: id.into(),
        follower_id: "11111111-1111-1111-1111-111111111111".into(),
        followee_id: "22222222-2222-2222-2222-222222222222".into(),
        status: status.into(),
        created_at: "2026-03-12T00:00:00Z".into(),
        decided_at: (status != "PENDING").then(|| "2026-03-12T00:01:00Z".into()),
    }
}
