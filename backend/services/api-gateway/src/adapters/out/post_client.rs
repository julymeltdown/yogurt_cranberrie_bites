use crate::adapters::out::grpc_call::{
    delegated, owner_request, replay_request, replay_safe_read, BoundedGrpcCallExt,
};
use crate::application::dto::post_dto::{
    CommentCreateRequest, CommentResponse, DeletePostResponse, FeedRecommendationEventRequest,
    FeedRecommendationEventResponse, FeedRecommendationMetadataResponse, FeedResponse,
    MutationReceiptStateDto, MutationReceiptStatusDto, PostCreateRequest, PostDetailResponse,
    PostLikeResponse, PostResponse, RankedFeedResponse, ResolveDeadLetterReplayRequestDto,
    ResolveDeadLetterReplayResponseDto,
};
use crate::application::ports::PostClientPort;
use crate::config::GatewayMetrics;
use async_trait::async_trait;
use platform::actor_delegation::{ActorDelegationClient, ActorDelegationSigner};
use platform::grpc_auth::{ServiceIdentityClientInterceptor, Workload};
use platform::grpc_client_trace::GrpcClientTraceService;
use platform::mutation_context;
use proto::replay_mutation_request::Command;
use proto::replay_mutation_response::Outcome;
use std::collections::HashMap;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::Channel;

pub mod proto {
    tonic::include_proto!("post.v1");
}

pub use super::client_error::classify_client_error as classify_post_client_error;

#[derive(Clone)]
pub struct GrpcPostClient {
    client: proto::post_service_client::PostServiceClient<
        InterceptedService<
            GrpcClientTraceService<ActorDelegationClient<Channel>>,
            ServiceIdentityClientInterceptor,
        >,
    >,
    metrics: GatewayMetrics,
}

impl GrpcPostClient {
    async fn replay(&self, command: Command) -> anyhow::Result<Option<Outcome>> {
        let response = self
            .client
            .clone()
            .replay_mutation(replay_request(proto::ReplayMutationRequest {
                command: Some(command),
            })?)
            .with_edge_deadline(platform::edge::Edge::GatewayPost, self.metrics.clone())
            .await?
            .into_inner();
        Ok(response.outcome)
    }

    /// Build the client without waiting for the peer to answer.
    ///
    /// The channel connects on first use, so a dependency that is down
    /// does not stop this service from starting. Configuration is still
    /// validated here: an invalid endpoint or TLS material fails now.
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
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls_material,
            &endpoint,
            Workload::Post,
            platform::edge::Edge::GatewayPost.budget(),
        )?;
        Ok(Self {
            client: proto::post_service_client::PostServiceClient::with_interceptor(
                GrpcClientTraceService::new(ActorDelegationClient::new(
                    channel,
                    signer,
                    Workload::Post,
                )),
                ServiceIdentityClientInterceptor::new(caller, service_token)?,
            ),
            metrics,
        })
    }
}

impl GrpcPostClient {
    async fn notification_posts_visible_inner(
        &self,
        viewer_id: String,
        post_ids: Vec<String>,
        max_duration: std::time::Duration,
    ) -> anyhow::Result<HashMap<String, bool>> {
        anyhow::ensure!(
            post_ids.len() <= 100,
            "Notification authorization exceeds its bound"
        );
        let mut client = self.client.clone();
        let message = delegated(
            proto::AuthorizeNotificationPostsRequest {
                viewer_id: viewer_id.clone(),
                post_ids: post_ids.clone(),
            },
            &viewer_id,
        )?;
        let edge = platform::edge::Edge::GatewayPost;
        let result = platform::grpc_deadline::with_deadline(
            max_duration.min(platform::grpc_deadline::local_deadline(edge)),
            edge,
            self.metrics.edge_deadline_metrics(),
            client.authorize_notification_posts(message),
        )
        .await?
        .into_inner();
        let allowed: std::collections::HashSet<_> = result.visible_post_ids.into_iter().collect();
        anyhow::ensure!(
            allowed.iter().all(|id| post_ids.contains(id)),
            "Unexpected notification authorization ID"
        );
        Ok(post_ids
            .into_iter()
            .map(|id| {
                let visible = allowed.contains(&id);
                (id, visible)
            })
            .collect())
    }

    async fn get_mutation_receipt_status_inner(
        &self,
        operation_id: uuid::Uuid,
    ) -> anyhow::Result<Option<MutationReceiptStatusDto>> {
        let mut client = self.client.clone();
        let response = match client
            .get_mutation_receipt_status(proto::GetMutationReceiptStatusRequest {
                operation_id: operation_id.to_string(),
            })
            .with_edge_deadline(platform::edge::Edge::GatewayPost, self.metrics.clone())
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
                    anyhow::bail!("Post service returned an unspecified mutation receipt state")
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

    async fn get_feed_inner(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> anyhow::Result<FeedResponse> {
        let mut client = self.client.clone();
        let actor = user_id.clone();
        Ok(map_feed(
            client
                .get_feed(delegated(
                    proto::FeedRequest {
                        user_id,
                        cursor: cursor.unwrap_or_default(),
                        limit,
                        request_id: String::new(),
                        session_id: String::new(),
                    },
                    &actor,
                )?)
                .with_edge_deadline(platform::edge::Edge::GatewayPost, self.metrics.clone())
                .await?
                .into_inner(),
        ))
    }

    async fn get_ranked_feed_inner(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
        request_id: String,
        session_id: Option<String>,
    ) -> anyhow::Result<RankedFeedResponse> {
        let mut client = self.client.clone();
        let actor = user_id.clone();
        Ok(map_ranked_feed(
            client
                .get_feed(delegated(
                    proto::FeedRequest {
                        user_id,
                        cursor: cursor.unwrap_or_default(),
                        limit,
                        request_id,
                        session_id: session_id.unwrap_or_default(),
                    },
                    &actor,
                )?)
                .with_edge_deadline(platform::edge::Edge::GatewayPost, self.metrics.clone())
                .await?
                .into_inner(),
        ))
    }

    async fn record_feed_recommendation_event_inner(
        &self,
        viewer_id: String,
        request_id: String,
        event: FeedRecommendationEventRequest,
    ) -> anyhow::Result<FeedRecommendationEventResponse> {
        let event_id = uuid::Uuid::new_v4();
        let mut client = self.client.clone();
        let actor = viewer_id.clone();
        let response = client
            .record_feed_recommendation_event(delegated(
                proto::RecordFeedRecommendationEventRequest {
                    event_id: event_id.to_string(),
                    schema_version: u32::from(
                        platform::recommendation::RECOMMENDATION_SCHEMA_VERSION,
                    ),
                    occurred_at: chrono::Utc::now().to_rfc3339(),
                    viewer_id: viewer_id.clone(),
                    post_id: event.post_id,
                    actor_id: viewer_id,
                    position: event.position,
                    request_id,
                    session_id: String::new(),
                    action: event.action,
                    source: event.source,
                    model_version: event.model_version,
                    impression_id: event.impression_id,
                    propensity_micros: event.propensity_micros,
                    dwell_ms: event.dwell_ms,
                    metadata_json: if event.metadata.is_null() {
                        "{}".into()
                    } else {
                        serde_json::to_string(&event.metadata)?
                    },
                },
                &actor,
            )?)
            .with_edge_deadline(platform::edge::Edge::GatewayPost, self.metrics.clone())
            .await?
            .into_inner();
        Ok(FeedRecommendationEventResponse {
            accepted: response.accepted,
        })
    }

    async fn get_author_posts_inner(
        &self,
        author_id: String,
        viewer_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> anyhow::Result<FeedResponse> {
        let mut client = self.client.clone();
        let actor = viewer_id.clone();
        Ok(map_feed(
            client
                .get_author_posts(delegated(
                    proto::AuthorPostsRequest {
                        author_id,
                        viewer_id,
                        cursor: cursor.unwrap_or_default(),
                        limit,
                    },
                    &actor,
                )?)
                .with_edge_deadline(platform::edge::Edge::GatewayPost, self.metrics.clone())
                .await?
                .into_inner(),
        ))
    }

    async fn get_author_post_counts_inner(
        &self,
        author_ids: Vec<String>,
    ) -> anyhow::Result<HashMap<String, i64>> {
        if author_ids.is_empty() {
            return Ok(HashMap::new());
        }
        // Counts need no actor and no authority exchange, so this is the one
        // Post read the Gateway retries. Post's actor-bound reads call the
        // exchange and some publish witness rows; they run once.
        let response = replay_safe_read(
            platform::edge::Edge::GatewayPost,
            "/post.v1.PostService/GetAuthorPostCounts",
            &self.metrics,
            || {
                Ok(tonic::Request::new(proto::AuthorPostCountsRequest {
                    author_ids: author_ids.clone(),
                }))
            },
            |request| {
                let mut client = self.client.clone();
                async move { client.get_author_post_counts(request).await }
            },
        )
        .await?
        .into_inner();
        Ok(response
            .counts
            .into_iter()
            .map(|count| (count.author_id, count.post_count))
            .collect())
    }

    async fn search_posts_inner(
        &self,
        viewer_id: String,
        query: String,
        cursor: Option<String>,
        limit: i32,
    ) -> anyhow::Result<FeedResponse> {
        let mut client = self.client.clone();
        let actor = viewer_id.clone();
        Ok(map_feed(
            client
                .search_posts(delegated(
                    proto::SearchPostsRequest {
                        query,
                        viewer_id,
                        cursor: cursor.unwrap_or_default(),
                        limit,
                    },
                    &actor,
                )?)
                .with_edge_deadline(platform::edge::Edge::GatewayPost, self.metrics.clone())
                .await?
                .into_inner(),
        ))
    }

    async fn get_trending_inner(
        &self,
        viewer_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> anyhow::Result<FeedResponse> {
        let mut client = self.client.clone();
        let actor = viewer_id.clone();
        Ok(map_feed(
            client
                .get_trending(delegated(
                    proto::TrendingRequest {
                        viewer_id,
                        cursor: cursor.unwrap_or_default(),
                        limit,
                    },
                    &actor,
                )?)
                .with_edge_deadline(platform::edge::Edge::GatewayPost, self.metrics.clone())
                .await?
                .into_inner(),
        ))
    }

    async fn get_post_inner(
        &self,
        viewer_id: String,
        post_id: String,
    ) -> anyhow::Result<PostDetailResponse> {
        let mut client = self.client.clone();
        let response = client
            .get_post(delegated(
                proto::GetPostRequest {
                    post_id,
                    viewer_id: viewer_id.clone(),
                },
                &viewer_id,
            )?)
            .with_edge_deadline(platform::edge::Edge::GatewayPost, self.metrics.clone())
            .await?
            .into_inner();
        Ok(PostDetailResponse {
            post: map_post(response.post.expect("post")),
            comments: response.comments.into_iter().map(map_comment).collect(),
        })
    }

    async fn create_post_inner(
        &self,
        user_id: String,
        request: PostCreateRequest,
    ) -> anyhow::Result<PostResponse> {
        let mut client = self.client.clone();
        let actor = user_id.clone();
        let command = proto::CreatePostRequest {
            author_id: user_id,
            content: request.content,
            visibility: request.visibility,
        };
        let response = if lookup_only() {
            match self.replay(Command::CreatePost(command)).await? {
                Some(Outcome::CreatePost(response)) => response,
                _ => anyhow::bail!("Owner returned an incompatible replay outcome"),
            }
        } else {
            client
                .create_post(owner_request(command, &actor)?)
                .with_edge_deadline(platform::edge::Edge::GatewayPost, self.metrics.clone())
                .await?
                .into_inner()
        };
        Ok(map_post(response.post.ok_or_else(|| {
            anyhow::anyhow!("Owner response omitted post")
        })?))
    }

    async fn delete_post_inner(
        &self,
        user_id: String,
        post_id: String,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> anyhow::Result<DeletePostResponse> {
        let mut client = self.client.clone();
        let actor = user_id.clone();
        let command = proto::DeletePostRequest {
            post_id,
            actor_id: user_id,
            request_id: request_id.unwrap_or_default(),
            ip_address: ip_address.unwrap_or_default(),
            user_agent: user_agent.unwrap_or_default(),
        };
        let response = if lookup_only() {
            match self.replay(Command::DeletePost(command)).await? {
                Some(Outcome::DeletePost(response)) => response,
                _ => anyhow::bail!("Owner returned an incompatible replay outcome"),
            }
        } else {
            client
                .delete_post(owner_request(command, &actor)?)
                .with_edge_deadline(platform::edge::Edge::GatewayPost, self.metrics.clone())
                .await?
                .into_inner()
        };
        Ok(DeletePostResponse {
            post_id: response.post_id,
            deleted: response.deleted,
        })
    }

    async fn create_comment_inner(
        &self,
        user_id: String,
        post_id: String,
        request: CommentCreateRequest,
    ) -> anyhow::Result<CommentResponse> {
        let mut client = self.client.clone();
        let actor = user_id.clone();
        let command = proto::CreateCommentRequest {
            post_id,
            author_id: user_id,
            content: request.content,
        };
        let response = if lookup_only() {
            match self.replay(Command::CreateComment(command)).await? {
                Some(Outcome::CreateComment(response)) => response,
                _ => anyhow::bail!("Owner returned an incompatible replay outcome"),
            }
        } else {
            client
                .create_comment(owner_request(command, &actor)?)
                .with_edge_deadline(platform::edge::Edge::GatewayPost, self.metrics.clone())
                .await?
                .into_inner()
        };
        Ok(map_comment(response.comment.ok_or_else(|| {
            anyhow::anyhow!("Owner response omitted comment")
        })?))
    }

    async fn like_post_inner(
        &self,
        user_id: String,
        post_id: String,
    ) -> anyhow::Result<PostLikeResponse> {
        let mut client = self.client.clone();
        let actor = user_id.clone();
        let command = proto::LikePostRequest { post_id, user_id };
        let response = if lookup_only() {
            match self.replay(Command::LikePost(command)).await? {
                Some(Outcome::LikePost(response)) => response,
                _ => anyhow::bail!("Owner returned an incompatible replay outcome"),
            }
        } else {
            client
                .like_post(owner_request(command, &actor)?)
                .with_edge_deadline(platform::edge::Edge::GatewayPost, self.metrics.clone())
                .await?
                .into_inner()
        };
        Ok(PostLikeResponse {
            post_id: response.post_id,
            like_count: response.like_count,
            liked: response.liked,
        })
    }

    async fn unlike_post_inner(
        &self,
        user_id: String,
        post_id: String,
    ) -> anyhow::Result<PostLikeResponse> {
        let mut client = self.client.clone();
        let actor = user_id.clone();
        let command = proto::UnlikePostRequest { post_id, user_id };
        let response = if lookup_only() {
            match self.replay(Command::UnlikePost(command)).await? {
                Some(Outcome::UnlikePost(response)) => response,
                _ => anyhow::bail!("Owner returned an incompatible replay outcome"),
            }
        } else {
            client
                .unlike_post(owner_request(command, &actor)?)
                .with_edge_deadline(platform::edge::Edge::GatewayPost, self.metrics.clone())
                .await?
                .into_inner()
        };
        Ok(PostLikeResponse {
            post_id: response.post_id,
            like_count: response.like_count,
            liked: response.liked,
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
        let mut client = self.client.clone();
        let actor = actor_id.clone();
        let response = client
            .apply_content_moderation(delegated(
                proto::ApplyContentModerationRequest {
                    report_id,
                    target_type,
                    target_id,
                    actor_id,
                    action,
                },
                &actor,
            )?)
            .with_edge_deadline(platform::edge::Edge::GatewayPost, self.metrics.clone())
            .await?
            .into_inner();
        Ok(response.applied)
    }

    async fn resolve_dead_letter_replay_inner(
        &self,
        request: ResolveDeadLetterReplayRequestDto,
    ) -> anyhow::Result<ResolveDeadLetterReplayResponseDto> {
        let actor = request.actor_id.clone();
        let response = self
            .client
            .clone()
            .resolve_dead_letter_replay(delegated(
                proto::ResolveDeadLetterReplayRequest {
                    event_id: request.event_id,
                    resolution_id: request.resolution_id,
                    actor_id: request.actor_id,
                    reason: request.reason,
                    evidence_reference: request.evidence_reference,
                },
                &actor,
            )?)
            .with_edge_deadline(platform::edge::Edge::GatewayPost, self.metrics.clone())
            .await?
            .into_inner();
        Ok(ResolveDeadLetterReplayResponseDto {
            event_id: response.event_id,
            resolution_id: response.resolution_id,
            created: response.created,
        })
    }
}

#[async_trait]
impl PostClientPort for GrpcPostClient {
    async fn notification_posts_visible(
        &self,
        viewer_id: String,
        post_ids: Vec<String>,
        max_duration: std::time::Duration,
    ) -> crate::application::post_client_error::PostClientResult<HashMap<String, bool>> {
        self.notification_posts_visible_inner(viewer_id, post_ids, max_duration)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_mutation_receipt_status(
        &self,
        operation_id: uuid::Uuid,
    ) -> crate::application::post_client_error::PostClientResult<Option<MutationReceiptStatusDto>>
    {
        self.get_mutation_receipt_status_inner(operation_id)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_feed(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::application::post_client_error::PostClientResult<FeedResponse> {
        self.get_feed_inner(user_id, cursor, limit)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_ranked_feed(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
        request_id: String,
        session_id: Option<String>,
    ) -> crate::application::post_client_error::PostClientResult<RankedFeedResponse> {
        self.get_ranked_feed_inner(user_id, cursor, limit, request_id, session_id)
            .await
            .map_err(classify_post_client_error)
    }
    async fn record_feed_recommendation_event(
        &self,
        viewer_id: String,
        request_id: String,
        event: FeedRecommendationEventRequest,
    ) -> crate::application::post_client_error::PostClientResult<FeedRecommendationEventResponse>
    {
        self.record_feed_recommendation_event_inner(viewer_id, request_id, event)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_author_posts(
        &self,
        author_id: String,
        viewer_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::application::post_client_error::PostClientResult<FeedResponse> {
        self.get_author_posts_inner(author_id, viewer_id, cursor, limit)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_author_post_counts(
        &self,
        author_ids: Vec<String>,
    ) -> crate::application::post_client_error::PostClientResult<HashMap<String, i64>> {
        self.get_author_post_counts_inner(author_ids)
            .await
            .map_err(classify_post_client_error)
    }
    async fn search_posts(
        &self,
        viewer_id: String,
        query: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::application::post_client_error::PostClientResult<FeedResponse> {
        self.search_posts_inner(viewer_id, query, cursor, limit)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_trending(
        &self,
        viewer_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::application::post_client_error::PostClientResult<FeedResponse> {
        self.get_trending_inner(viewer_id, cursor, limit)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_post(
        &self,
        viewer_id: String,
        post_id: String,
    ) -> crate::application::post_client_error::PostClientResult<PostDetailResponse> {
        self.get_post_inner(viewer_id, post_id)
            .await
            .map_err(classify_post_client_error)
    }
    async fn create_post(
        &self,
        user_id: String,
        request: PostCreateRequest,
    ) -> crate::application::post_client_error::PostClientResult<PostResponse> {
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
    ) -> crate::application::post_client_error::PostClientResult<DeletePostResponse> {
        self.delete_post_inner(user_id, post_id, request_id, ip_address, user_agent)
            .await
            .map_err(classify_post_client_error)
    }
    async fn create_comment(
        &self,
        user_id: String,
        post_id: String,
        request: CommentCreateRequest,
    ) -> crate::application::post_client_error::PostClientResult<CommentResponse> {
        self.create_comment_inner(user_id, post_id, request)
            .await
            .map_err(classify_post_client_error)
    }
    async fn like_post(
        &self,
        user_id: String,
        post_id: String,
    ) -> crate::application::post_client_error::PostClientResult<PostLikeResponse> {
        self.like_post_inner(user_id, post_id)
            .await
            .map_err(classify_post_client_error)
    }
    async fn unlike_post(
        &self,
        user_id: String,
        post_id: String,
    ) -> crate::application::post_client_error::PostClientResult<PostLikeResponse> {
        self.unlike_post_inner(user_id, post_id)
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
    ) -> crate::application::post_client_error::PostClientResult<bool> {
        self.apply_content_moderation_inner(report_id, target_type, target_id, actor_id, action)
            .await
            .map_err(classify_post_client_error)
    }
    async fn resolve_dead_letter_replay(
        &self,
        request: ResolveDeadLetterReplayRequestDto,
    ) -> crate::application::post_client_error::PostClientResult<ResolveDeadLetterReplayResponseDto>
    {
        self.resolve_dead_letter_replay_inner(request)
            .await
            .map_err(classify_post_client_error)
    }
}

fn lookup_only() -> bool {
    mutation_context::current().is_some_and(|context| context.lookup_only)
}

fn map_feed(value: proto::FeedResponse) -> FeedResponse {
    FeedResponse {
        posts: value.posts.into_iter().map(map_post).collect(),
        next_cursor: empty_to_none(value.next_cursor),
    }
}

fn map_ranked_feed(value: proto::FeedResponse) -> RankedFeedResponse {
    let rank_version = empty_to_none(value.rank_version);
    RankedFeedResponse {
        posts: value.posts.into_iter().map(map_post).collect(),
        next_cursor: empty_to_none(value.next_cursor),
        snapshot_id: empty_to_none(value.snapshot_id),
        rank_version: rank_version.clone(),
        request_id: empty_to_none(value.request_id),
        fallback_used: value.fallback_used,
        recommendation_metadata: value
            .items
            .into_iter()
            .map(|item| FeedRecommendationMetadataResponse {
                post_id: item.post_id,
                source: item.source,
                reason_code: item.reason_code,
                score_micros: item.score_micros,
                rank_version: rank_version.clone().unwrap_or_default(),
                impression_id: item.impression_id,
                position: item.position,
                propensity_micros: item.propensity_micros,
                exploration: item.exploration,
            })
            .collect(),
        failed_sources: value.failed_sources,
    }
}

fn map_post(value: proto::Post) -> PostResponse {
    PostResponse {
        id: value.id,
        author_id: value.author_id,
        content: value.content,
        visibility: value.visibility,
        created_at: value.created_at,
        comment_count: value.comment_count,
        like_count: value.like_count,
        liked_by_me: value.liked_by_me,
    }
}

fn map_comment(value: proto::Comment) -> CommentResponse {
    CommentResponse {
        id: value.id,
        post_id: value.post_id,
        author_id: value.author_id,
        content: value.content,
        created_at: value.created_at,
    }
}

fn empty_to_none(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}
