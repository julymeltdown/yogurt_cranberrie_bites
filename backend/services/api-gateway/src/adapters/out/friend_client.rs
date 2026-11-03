use crate::adapters::out::grpc_call::{
    delegated, owner_request, replay_request, BoundedGrpcCallExt,
};
use crate::application::dto::friend_dto::{
    FollowActionResponse, FollowCountsResponse, FollowEdgeResponse, FollowListResponse,
    FollowRequestPageResponse, FollowRequestResponse, FollowStatusResponse,
    PeopleRecommendationEventRequest, PeopleRecommendationPageResponse,
    PeopleRecommendationResponse, RecommendationEventResponse, RelationshipPolicyResponse,
};
use crate::application::dto::post_dto::{MutationReceiptStateDto, MutationReceiptStatusDto};
use crate::application::ports::FriendClientPort;
use crate::config::GatewayMetrics;
use async_trait::async_trait;
use platform::actor_delegation::{ActorDelegationClient, ActorDelegationSigner};
use platform::grpc_auth::{ServiceIdentityClientInterceptor, Workload};
use platform::grpc_client_trace::GrpcClientTraceService;
use platform::mutation_context;
use proto::replay_mutation_request::Command;
use proto::replay_mutation_response::Outcome;
use std::{collections::HashMap, future::Future};
use tonic::transport::Channel;
use tonic::{service::interceptor::InterceptedService, Status};

pub mod proto {
    tonic::include_proto!("friend.v1");
}

type FriendGrpc = proto::friend_service_client::FriendServiceClient<
    InterceptedService<
        GrpcClientTraceService<ActorDelegationClient<Channel>>,
        ServiceIdentityClientInterceptor,
    >,
>;

#[derive(Clone)]
pub struct GrpcFriendClient {
    client: FriendGrpc,
    metrics: GatewayMetrics,
}

impl GrpcFriendClient {
    /// A replay-safe Friend read without a delegated actor.
    async fn read<M, R, B, C, Fut>(
        &self,
        method: &'static str,
        build: B,
        call: C,
    ) -> anyhow::Result<tonic::Response<R>>
    where
        B: FnMut() -> M,
        C: Fn(FriendGrpc, tonic::Request<M>) -> Fut,
        Fut: Future<Output = Result<tonic::Response<R>, Status>> + Send,
        R: Send,
    {
        let mut build = build;
        crate::adapters::out::grpc_call::replay_safe_read(
            platform::edge::Edge::GatewayFriend,
            method,
            &self.metrics,
            move || Ok(tonic::Request::new(build())),
            |request| call(self.client.clone(), request),
        )
        .await
    }

    async fn bounded_call<T, F>(&self, call: F) -> Result<T, Status>
    where
        F: Future<Output = Result<T, Status>> + Send,
        T: Send,
    {
        // Bound the complete generated-client future, including channel
        // readiness. Keep the Endpoint timeout as a transport safeguard too.
        call.with_edge_deadline(platform::edge::Edge::GatewayFriend, self.metrics.clone())
            .await
    }

    async fn replay(&self, command: Command) -> anyhow::Result<Option<Outcome>> {
        let response = self
            .bounded_call(self.client.clone().replay_mutation(replay_request(
                proto::ReplayMutationRequest {
                    command: Some(command),
                },
            )?))
            .await?
            .into_inner();
        Ok(response.outcome)
    }

    pub async fn connect(
        endpoint: String,
        caller: Workload,
        service_token: &str,
        tls_material: Option<&platform::grpc_tls::TlsMaterial>,
    ) -> anyhow::Result<Self> {
        Self::connect_with_metrics(
            endpoint,
            caller,
            service_token,
            tls_material,
            ActorDelegationSigner::local_parity(caller),
            GatewayMetrics::default(),
        )
        .await
    }

    pub async fn connect_with_metrics(
        endpoint: String,
        caller: Workload,
        service_token: &str,
        tls_material: Option<&platform::grpc_tls::TlsMaterial>,
        signer: ActorDelegationSigner,
        metrics: GatewayMetrics,
    ) -> anyhow::Result<Self> {
        // Lazy connection keeps edge startup independent of one dependency, while
        // mutual TLS is configured before the first call.
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls_material,
            &endpoint,
            Workload::Friend,
            platform::edge::Edge::GatewayFriend.budget(),
        )?;
        Ok(Self {
            client: proto::friend_service_client::FriendServiceClient::with_interceptor(
                GrpcClientTraceService::new(ActorDelegationClient::new(
                    channel,
                    signer,
                    Workload::Friend,
                )),
                ServiceIdentityClientInterceptor::new(caller, service_token)?,
            ),
            metrics,
        })
    }
}

impl GrpcFriendClient {
    async fn get_mutation_receipt_status_friend_client_port_inner(
        &self,
        operation_id: uuid::Uuid,
    ) -> anyhow::Result<Option<MutationReceiptStatusDto>> {
        let mut client = self.client.clone();
        let response = match self
            .bounded_call(client.get_mutation_receipt_status(
                proto::GetMutationReceiptStatusRequest {
                    operation_id: operation_id.to_string(),
                },
            ))
            .await
        {
            Ok(response) => response.into_inner(),
            Err(status) if status.code() == tonic::Code::NotFound => return Ok(None),
            Err(status) => return Err(status.into()),
        };
        let response_operation_id = uuid::Uuid::parse_str(&response.operation_id)?;
        let actor_id = uuid::Uuid::parse_str(&response.actor_id)?;
        let state =
            match proto::get_mutation_receipt_status_response::State::try_from(response.state)? {
                proto::get_mutation_receipt_status_response::State::Replayable => {
                    MutationReceiptStateDto::Replayable
                }
                proto::get_mutation_receipt_status_response::State::Expired => {
                    MutationReceiptStateDto::Expired
                }
                proto::get_mutation_receipt_status_response::State::Incomplete => {
                    MutationReceiptStateDto::Incomplete
                }
                proto::get_mutation_receipt_status_response::State::Unspecified => {
                    anyhow::bail!("Friend service returned an unspecified mutation receipt state")
                }
            };
        Ok(Some(MutationReceiptStatusDto {
            operation_id: response_operation_id,
            actor_id,
            scope: response.scope,
            committed_at: chrono::DateTime::parse_from_rfc3339(&response.committed_at)?
                .with_timezone(&chrono::Utc),
            replay_until: chrono::DateTime::parse_from_rfc3339(&response.replay_until)?
                .with_timezone(&chrono::Utc),
            state,
        }))
    }

    async fn follow_friend_client_port_inner(
        &self,
        follower_id: String,
        followee_id: String,
    ) -> anyhow::Result<FollowActionResponse> {
        let mut client = self.client.clone();
        let actor = follower_id.clone();
        let command = proto::FollowUserRequest {
            follower_id,
            followee_id,
        };
        let response = if lookup_only() {
            match self.replay(Command::FollowUser(command)).await? {
                Some(Outcome::FollowUser(response)) => response,
                _ => anyhow::bail!("Friend service returned an incompatible replay outcome"),
            }
        } else {
            self.bounded_call(client.follow_user(owner_request(command, &actor)?))
                .await?
                .into_inner()
        };
        Ok(FollowActionResponse {
            following: response.following,
            followed_at: response.edge.map(|edge| edge.created_at),
            status: response.status,
            request_id: empty_to_none(response.request_id),
        })
    }

    async fn unfollow_friend_client_port_inner(
        &self,
        follower_id: String,
        followee_id: String,
    ) -> anyhow::Result<FollowActionResponse> {
        let mut client = self.client.clone();
        let actor = follower_id.clone();
        let command = proto::UnfollowUserRequest {
            follower_id,
            followee_id,
        };
        let response = if lookup_only() {
            match self.replay(Command::UnfollowUser(command)).await? {
                Some(Outcome::UnfollowUser(response)) => response,
                _ => anyhow::bail!("Friend service returned an incompatible replay outcome"),
            }
        } else {
            self.bounded_call(client.unfollow_user(owner_request(command, &actor)?))
                .await?
                .into_inner()
        };
        Ok(FollowActionResponse {
            following: response.following,
            followed_at: None,
            status: "INACTIVE".to_owned(),
            request_id: None,
        })
    }

    async fn list_followers_friend_client_port_inner(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: Option<i32>,
    ) -> anyhow::Result<FollowListResponse> {
        let cursor = cursor.unwrap_or_default();
        let response = self
            .read(
                "/friend.v1.FriendService/ListFollowers",
                || proto::ListFollowersRequest {
                    user_id: user_id.clone(),
                    cursor: cursor.clone(),
                    limit: limit.unwrap_or(20),
                },
                |mut client, request| async move { client.list_followers(request).await },
            )
            .await?
            .into_inner();
        Ok(FollowListResponse {
            edges: response
                .edges
                .into_iter()
                .map(|edge| FollowEdgeResponse {
                    user_id: edge.follower_id,
                    followed_at: edge.created_at,
                })
                .collect(),
            next_cursor: empty_to_none(response.next_cursor),
            total_count: response.total_count,
        })
    }

    async fn list_following_friend_client_port_inner(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: Option<i32>,
    ) -> anyhow::Result<FollowListResponse> {
        let cursor = cursor.unwrap_or_default();
        let response = self
            .read(
                "/friend.v1.FriendService/ListFollowing",
                || proto::ListFollowingRequest {
                    user_id: user_id.clone(),
                    cursor: cursor.clone(),
                    limit: limit.unwrap_or(20),
                },
                |mut client, request| async move { client.list_following(request).await },
            )
            .await?
            .into_inner();
        Ok(FollowListResponse {
            edges: response
                .edges
                .into_iter()
                .map(|edge| FollowEdgeResponse {
                    user_id: edge.followee_id,
                    followed_at: edge.created_at,
                })
                .collect(),
            next_cursor: empty_to_none(response.next_cursor),
            total_count: response.total_count,
        })
    }

    async fn get_counts_friend_client_port_inner(
        &self,
        user_id: String,
    ) -> anyhow::Result<FollowCountsResponse> {
        let response = self
            .read(
                "/friend.v1.FriendService/GetFollowCounts",
                || proto::GetFollowCountsRequest {
                    user_id: user_id.clone(),
                },
                |mut client, request| async move { client.get_follow_counts(request).await },
            )
            .await?
            .into_inner();
        Ok(FollowCountsResponse {
            follower_count: response.follower_count,
            following_count: response.following_count,
        })
    }

    async fn get_counts_batch_friend_client_port_inner(
        &self,
        user_ids: Vec<String>,
    ) -> anyhow::Result<std::collections::HashMap<String, FollowCountsResponse>> {
        let response = self
            .read(
                "/friend.v1.FriendService/GetFollowStats",
                || proto::GetFollowStatsRequest {
                    user_ids: user_ids.clone(),
                },
                |mut client, request| async move { client.get_follow_stats(request).await },
            )
            .await?
            .into_inner();
        Ok(response
            .stats
            .into_iter()
            .map(|stat| {
                (
                    stat.user_id,
                    FollowCountsResponse {
                        follower_count: stat.follower_count,
                        following_count: stat.following_count,
                    },
                )
            })
            .collect())
    }

    async fn check_following_friend_client_port_inner(
        &self,
        follower_id: String,
        followee_id: String,
    ) -> anyhow::Result<FollowStatusResponse> {
        let response = self
            .read(
                "/friend.v1.FriendService/CheckFollow",
                || proto::CheckFollowRequest {
                    follower_id: follower_id.clone(),
                    followee_id: followee_id.clone(),
                },
                |mut client, request| async move { client.check_follow(request).await },
            )
            .await?
            .into_inner();
        Ok(FollowStatusResponse {
            following: response.following,
        })
    }

    async fn accept_follow_request_friend_client_port_inner(
        &self,
        request_id: String,
        actor_id: String,
    ) -> anyhow::Result<FollowActionResponse> {
        let mut client = self.client.clone();
        let response = self
            .bounded_call(client.accept_follow_request(delegated(
                proto::FollowRequestActionRequest {
                    request_id,
                    actor_id: actor_id.clone(),
                },
                &actor_id,
            )?))
            .await?
            .into_inner();
        Ok(FollowActionResponse {
            following: response.following,
            followed_at: response.edge.map(|edge| edge.created_at),
            status: response.status,
            request_id: response.request.map(|request| request.id),
        })
    }

    async fn reject_follow_request_friend_client_port_inner(
        &self,
        request_id: String,
        actor_id: String,
    ) -> anyhow::Result<FollowRequestResponse> {
        self.follow_request_action(request_id, actor_id, FollowRequestAction::Reject)
            .await
    }

    async fn cancel_follow_request_friend_client_port_inner(
        &self,
        request_id: String,
        actor_id: String,
    ) -> anyhow::Result<FollowRequestResponse> {
        self.follow_request_action(request_id, actor_id, FollowRequestAction::Cancel)
            .await
    }

    async fn list_incoming_follow_requests_friend_client_port_inner(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> anyhow::Result<FollowRequestPageResponse> {
        self.list_follow_requests(user_id, cursor, limit, true)
            .await
    }

    async fn list_outgoing_follow_requests_friend_client_port_inner(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> anyhow::Result<FollowRequestPageResponse> {
        self.list_follow_requests(user_id, cursor, limit, false)
            .await
    }

    async fn block_user_friend_client_port_inner(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> anyhow::Result<RelationshipPolicyResponse> {
        self.relationship_action(viewer_id, target_id, RelationshipAction::Block)
            .await
    }

    async fn unblock_user_friend_client_port_inner(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> anyhow::Result<RelationshipPolicyResponse> {
        self.relationship_action(viewer_id, target_id, RelationshipAction::Unblock)
            .await
    }

    async fn mute_user_friend_client_port_inner(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> anyhow::Result<RelationshipPolicyResponse> {
        self.relationship_action(viewer_id, target_id, RelationshipAction::Mute)
            .await
    }

    async fn unmute_user_friend_client_port_inner(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> anyhow::Result<RelationshipPolicyResponse> {
        self.relationship_action(viewer_id, target_id, RelationshipAction::Unmute)
            .await
    }

    async fn get_relationship_friend_client_port_inner(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> anyhow::Result<RelationshipPolicyResponse> {
        self.relationship_action(viewer_id, target_id, RelationshipAction::Get)
            .await
    }

    async fn get_people_recommendations_friend_client_port_inner(
        &self,
        viewer_id: String,
        cursor: Option<String>,
        limit: i32,
        request_id: String,
    ) -> anyhow::Result<PeopleRecommendationPageResponse> {
        let mut client = self.client.clone();
        let response = self
            .bounded_call(client.get_people_recommendations(delegated(
                proto::GetPeopleRecommendationsRequest {
                    viewer_id: viewer_id.clone(),
                    cursor: cursor.unwrap_or_default(),
                    limit,
                    request_id,
                    session_id: String::new(),
                },
                &viewer_id,
            )?))
            .await?
            .into_inner();
        Ok(PeopleRecommendationPageResponse {
            items: response
                .items
                .into_iter()
                .map(|item| PeopleRecommendationResponse {
                    user_id: item.user_id,
                    username: item.username,
                    nickname: item.nickname,
                    bio: item.bio,
                    avatar_url: item.avatar_url,
                    source: item.source,
                    reason_code: item.reason_code,
                    score_micros: item.score_micros,
                    rank_version: item.rank_version,
                    impression_id: item.impression_id,
                })
                .collect(),
            next_cursor: empty_to_none(response.next_cursor),
            snapshot_id: response.snapshot_id,
            rank_version: response.rank_version,
            request_id: response.request_id,
            fallback_used: response.fallback_used,
        })
    }

    async fn record_people_recommendation_event_friend_client_port_inner(
        &self,
        viewer_id: String,
        request_id: String,
        event: PeopleRecommendationEventRequest,
    ) -> anyhow::Result<RecommendationEventResponse> {
        let event_id = uuid::Uuid::new_v4();
        let mut client = self.client.clone();
        let actor = viewer_id.clone();
        let response = self
            .bounded_call(client.record_people_recommendation_event(delegated(
                proto::RecordRecommendationEventRequest {
                    event_id: event_id.to_string(),
                    schema_version: i32::from(
                        platform::recommendation::RECOMMENDATION_SCHEMA_VERSION,
                    ),
                    occurred_at: chrono::Utc::now().to_rfc3339(),
                    viewer_id: viewer_id.clone(),
                    subject_type: "USER".into(),
                    subject_id: event.candidate_user_id,
                    actor_id: viewer_id,
                    surface: "PEOPLE_RECOMMENDATION".into(),
                    position: event.position.unwrap_or(-1),
                    request_id,
                    session_id: String::new(),
                    action: event.action,
                    source: event.source,
                    model_version: event.model_version,
                    impression_id: event.impression_id,
                    propensity_micros: event.propensity_micros.unwrap_or(0),
                    dwell_ms: event.dwell_ms.unwrap_or(0),
                    metadata_json: if event.metadata.is_null() {
                        "{}".into()
                    } else {
                        serde_json::to_string(&event.metadata)?
                    },
                },
                &actor,
            )?))
            .await?
            .into_inner();
        Ok(RecommendationEventResponse {
            accepted: response.accepted,
        })
    }

    async fn get_relationships_batch_friend_client_port_inner(
        &self,
        viewer_id: String,
        target_ids: Vec<String>,
    ) -> anyhow::Result<HashMap<String, RelationshipPolicyResponse>> {
        if target_ids.len() > 100 {
            anyhow::bail!("Relationship batch supports at most 100 user IDs");
        }
        let response = self
            .read(
                "/friend.v1.FriendService/BatchGetRelationships",
                || proto::BatchRelationshipRequest {
                    viewer_id: viewer_id.clone(),
                    target_ids: target_ids.clone(),
                },
                |mut client, request| async move { client.batch_get_relationships(request).await },
            )
            .await?
            .into_inner();
        Ok(response
            .policies
            .into_iter()
            .filter_map(|entry| {
                entry
                    .policy
                    .map(|policy| (entry.target_id, to_relationship_policy(policy)))
            })
            .collect())
    }
}

#[async_trait]
impl FriendClientPort for GrpcFriendClient {
    async fn get_mutation_receipt_status(
        &self,
        operation_id: uuid::Uuid,
    ) -> crate::application::client_error::GatewayClientResult<Option<MutationReceiptStatusDto>>
    {
        self.get_mutation_receipt_status_friend_client_port_inner(operation_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn follow(
        &self,
        follower_id: String,
        followee_id: String,
    ) -> crate::application::client_error::GatewayClientResult<FollowActionResponse> {
        self.follow_friend_client_port_inner(follower_id, followee_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn unfollow(
        &self,
        follower_id: String,
        followee_id: String,
    ) -> crate::application::client_error::GatewayClientResult<FollowActionResponse> {
        self.unfollow_friend_client_port_inner(follower_id, followee_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn list_followers(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: Option<i32>,
    ) -> crate::application::client_error::GatewayClientResult<FollowListResponse> {
        self.list_followers_friend_client_port_inner(user_id, cursor, limit)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn list_following(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: Option<i32>,
    ) -> crate::application::client_error::GatewayClientResult<FollowListResponse> {
        self.list_following_friend_client_port_inner(user_id, cursor, limit)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn get_counts(
        &self,
        user_id: String,
    ) -> crate::application::client_error::GatewayClientResult<FollowCountsResponse> {
        self.get_counts_friend_client_port_inner(user_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn get_counts_batch(
        &self,
        user_ids: Vec<String>,
    ) -> crate::application::client_error::GatewayClientResult<
        std::collections::HashMap<String, FollowCountsResponse>,
    > {
        self.get_counts_batch_friend_client_port_inner(user_ids)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn check_following(
        &self,
        follower_id: String,
        followee_id: String,
    ) -> crate::application::client_error::GatewayClientResult<FollowStatusResponse> {
        self.check_following_friend_client_port_inner(follower_id, followee_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn accept_follow_request(
        &self,
        request_id: String,
        actor_id: String,
    ) -> crate::application::client_error::GatewayClientResult<FollowActionResponse> {
        self.accept_follow_request_friend_client_port_inner(request_id, actor_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn reject_follow_request(
        &self,
        request_id: String,
        actor_id: String,
    ) -> crate::application::client_error::GatewayClientResult<FollowRequestResponse> {
        self.reject_follow_request_friend_client_port_inner(request_id, actor_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn cancel_follow_request(
        &self,
        request_id: String,
        actor_id: String,
    ) -> crate::application::client_error::GatewayClientResult<FollowRequestResponse> {
        self.cancel_follow_request_friend_client_port_inner(request_id, actor_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn list_incoming_follow_requests(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::application::client_error::GatewayClientResult<FollowRequestPageResponse> {
        self.list_incoming_follow_requests_friend_client_port_inner(user_id, cursor, limit)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn list_outgoing_follow_requests(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::application::client_error::GatewayClientResult<FollowRequestPageResponse> {
        self.list_outgoing_follow_requests_friend_client_port_inner(user_id, cursor, limit)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn block_user(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> crate::application::client_error::GatewayClientResult<RelationshipPolicyResponse> {
        self.block_user_friend_client_port_inner(viewer_id, target_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn unblock_user(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> crate::application::client_error::GatewayClientResult<RelationshipPolicyResponse> {
        self.unblock_user_friend_client_port_inner(viewer_id, target_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn mute_user(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> crate::application::client_error::GatewayClientResult<RelationshipPolicyResponse> {
        self.mute_user_friend_client_port_inner(viewer_id, target_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn unmute_user(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> crate::application::client_error::GatewayClientResult<RelationshipPolicyResponse> {
        self.unmute_user_friend_client_port_inner(viewer_id, target_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn get_relationship(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> crate::application::client_error::GatewayClientResult<RelationshipPolicyResponse> {
        self.get_relationship_friend_client_port_inner(viewer_id, target_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn get_people_recommendations(
        &self,
        viewer_id: String,
        cursor: Option<String>,
        limit: i32,
        request_id: String,
    ) -> crate::application::client_error::GatewayClientResult<PeopleRecommendationPageResponse>
    {
        self.get_people_recommendations_friend_client_port_inner(
            viewer_id, cursor, limit, request_id,
        )
        .await
        .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn record_people_recommendation_event(
        &self,
        viewer_id: String,
        request_id: String,
        event: PeopleRecommendationEventRequest,
    ) -> crate::application::client_error::GatewayClientResult<RecommendationEventResponse> {
        self.record_people_recommendation_event_friend_client_port_inner(
            viewer_id, request_id, event,
        )
        .await
        .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn get_relationships_batch(
        &self,
        viewer_id: String,
        target_ids: Vec<String>,
    ) -> crate::application::client_error::GatewayClientResult<
        HashMap<String, RelationshipPolicyResponse>,
    > {
        self.get_relationships_batch_friend_client_port_inner(viewer_id, target_ids)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
}

#[derive(Clone, Copy)]
enum FollowRequestAction {
    Reject,
    Cancel,
}

#[derive(Clone, Copy)]
enum RelationshipAction {
    Block,
    Unblock,
    Mute,
    Unmute,
    Get,
}

impl GrpcFriendClient {
    async fn follow_request_action(
        &self,
        request_id: String,
        actor_id: String,
        action: FollowRequestAction,
    ) -> anyhow::Result<FollowRequestResponse> {
        let request = delegated(
            proto::FollowRequestActionRequest {
                request_id,
                actor_id: actor_id.clone(),
            },
            &actor_id,
        )?;
        let mut client = self.client.clone();
        let response = self
            .bounded_call(async {
                match action {
                    FollowRequestAction::Reject => client.reject_follow_request(request).await,
                    FollowRequestAction::Cancel => client.cancel_follow_request(request).await,
                }
            })
            .await?
            .into_inner();
        response
            .request
            .map(to_follow_request)
            .ok_or_else(|| anyhow::anyhow!("Follow request response is missing"))
    }

    async fn list_follow_requests(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
        incoming: bool,
    ) -> anyhow::Result<FollowRequestPageResponse> {
        let request = delegated(
            proto::ListFollowRequestsRequest {
                user_id: user_id.clone(),
                cursor: cursor.unwrap_or_default(),
                limit,
            },
            &user_id,
        )?;
        let mut client = self.client.clone();
        let response = self
            .bounded_call(async {
                if incoming {
                    client.list_incoming_follow_requests(request).await
                } else {
                    client.list_outgoing_follow_requests(request).await
                }
            })
            .await?
            .into_inner();
        Ok(FollowRequestPageResponse {
            requests: response
                .requests
                .into_iter()
                .map(to_follow_request)
                .collect(),
            next_cursor: empty_to_none(response.next_cursor),
        })
    }

    async fn relationship_action(
        &self,
        viewer_id: String,
        target_id: String,
        action: RelationshipAction,
    ) -> anyhow::Result<RelationshipPolicyResponse> {
        let request = delegated(
            proto::RelationshipRequest {
                viewer_id: viewer_id.clone(),
                target_id,
            },
            &viewer_id,
        )?;
        let mut client = self.client.clone();
        let response = self
            .bounded_call(async {
                match action {
                    RelationshipAction::Block => client.block_user(request).await,
                    RelationshipAction::Unblock => client.unblock_user(request).await,
                    RelationshipAction::Mute => client.mute_user(request).await,
                    RelationshipAction::Unmute => client.unmute_user(request).await,
                    RelationshipAction::Get => client.get_relationship(request).await,
                }
            })
            .await?
            .into_inner();
        Ok(to_relationship_policy(response))
    }
}

fn to_follow_request(value: proto::FollowRequest) -> FollowRequestResponse {
    FollowRequestResponse {
        id: value.id,
        follower_id: value.follower_id,
        followee_id: value.followee_id,
        status: value.status,
        created_at: value.created_at,
        decided_at: empty_to_none(value.decided_at),
    }
}

fn to_relationship_policy(value: proto::RelationshipPolicy) -> RelationshipPolicyResponse {
    RelationshipPolicyResponse {
        blocked_by_viewer: value.blocked_by_viewer,
        blocked_viewer: value.blocked_viewer,
        muted_by_viewer: value.muted_by_viewer,
        following: value.following,
    }
}

fn lookup_only() -> bool {
    mutation_context::current().is_some_and(|context| context.lookup_only)
}

fn empty_to_none(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}
