use crate::application::dto::mutation_dto::{MutationContextDto, MutationError};
use crate::application::dto::post_dto::{
    ApplyContentModerationDto, ApplyUserDeletionDto, CreateCommentDto, CreatePostDto,
    DeleteCommentDto, DeletePostDto, RecordFeedRecommendationEventDto,
    ResolveDeadLetterReplayRequestDto,
};
use crate::config::AppState;
use platform::grpc_auth::{authorize_call, RpcAction};
use std::sync::Arc;
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub mod proto {
    tonic::include_proto!("post.v1");
}

#[cfg(test)]
mod mutation_failure_tests {
    #[test]
    fn a_failed_exchange_is_reported_unavailable_and_marked_not_retryable() {
        use platform::grpc_retry::{not_retried, NO_RETRY_METADATA};
        let exchange = not_retried(tonic::Status::resource_exhausted("exchange quota"))
            .context("relationship policy");
        let status = super::to_status(exchange);
        assert_eq!(status.code(), tonic::Code::Unavailable);
        assert_eq!(status.metadata().get(NO_RETRY_METADATA).unwrap(), "never");

        let owner = anyhow::Error::from(tonic::Status::unavailable("friend down"));
        let status = super::to_status(owner);
        assert_eq!(status.code(), tonic::Code::Unavailable);
        assert!(status.metadata().get(NO_RETRY_METADATA).is_none());
    }

    #[test]
    fn witness_failures_are_controlled_authority_or_dependency_errors() {
        use post_application::authority_witness::WitnessErrorDto;
        assert_eq!(
            super::to_status(WitnessErrorDto::Unavailable.into()).code(),
            tonic::Code::Unavailable
        );
        assert_eq!(
            super::to_status(WitnessErrorDto::StageConflict.into()).code(),
            tonic::Code::Unavailable
        );
        for error in [
            WitnessErrorDto::InvalidBinding,
            WitnessErrorDto::InvalidMembership,
            WitnessErrorDto::MissingOrExpired,
            WitnessErrorDto::NotAdmitted,
        ] {
            let status = super::to_status(error.into());
            assert_eq!(status.code(), tonic::Code::PermissionDenied);
            assert_eq!(status.message(), "Query authority denied");
        }
    }

    #[test]
    fn dependency_failures_are_not_reported_as_client_validation_or_disclosed() {
        let error = anyhow::Error::new(tonic::Status::unavailable("DEPENDENCY_DETAIL_CANARY"))
            .context("visibility dependency failed");
        let status = super::to_status(error);
        assert_eq!(status.code(), tonic::Code::Unavailable);
        assert!(!status.message().contains("DEPENDENCY_DETAIL_CANARY"));
    }
}

#[derive(Clone)]
pub struct PostGrpcServer {
    state: Arc<AppState>,
}

impl PostGrpcServer {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl proto::post_service_server::PostService for PostGrpcServer {
    async fn authorize_notification_posts(
        &self,
        request: Request<proto::AuthorizeNotificationPostsRequest>,
    ) -> Result<Response<proto::AuthorizeNotificationPostsResponse>, Status> {
        authorize_call(&request, RpcAction::Read)?;
        let request = request.into_inner();
        if request.post_ids.len() > 100 {
            return Err(Status::invalid_argument("At most 100 post IDs are allowed"));
        }
        let viewer_id = parse_uuid(&request.viewer_id, "Viewer ID")?;
        let post_ids = request
            .post_ids
            .iter()
            .map(|id| parse_uuid(id, "Post ID"))
            .collect::<Result<Vec<_>, _>>()?;
        let result = self
            .state
            .authorize_notification_posts
            .execute(
                crate::application::dto::post_dto::AuthorizeNotificationPostsDto {
                    viewer_id,
                    post_ids,
                },
            )
            .await
            .map_err(|_| Status::unavailable("Notification authorization unavailable"))?;
        Ok(Response::new(proto::AuthorizeNotificationPostsResponse {
            visible_post_ids: result
                .visible_post_ids
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
        }))
    }

    async fn get_mutation_receipt_status(
        &self,
        request: Request<proto::GetMutationReceiptStatusRequest>,
    ) -> Result<Response<proto::GetMutationReceiptStatusResponse>, Status> {
        authorize_call(&request, RpcAction::MutationReceiptLookup)?;
        let operation_id = Uuid::parse_str(&request.into_inner().operation_id)
            .map_err(|_| Status::invalid_argument("Operation ID must be a UUID"))?;
        let receipt = self
            .state
            .get_mutation_receipt_status
            .execute(operation_id)
            .await
            .map_err(to_status)?
            .ok_or_else(|| Status::not_found("OWNER_RECEIPT_NOT_FOUND"))?;
        use proto::get_mutation_receipt_status_response::State;
        let state = match receipt.state {
            crate::application::dto::mutation_dto::MutationReceiptStateDto::Replayable => {
                State::Replayable
            }
            crate::application::dto::mutation_dto::MutationReceiptStateDto::Expired => {
                State::Expired
            }
            crate::application::dto::mutation_dto::MutationReceiptStateDto::Incomplete => {
                State::Incomplete
            }
        };
        Ok(Response::new(proto::GetMutationReceiptStatusResponse {
            operation_id: receipt.operation_id.to_string(),
            actor_id: receipt.actor_id.to_string(),
            scope: receipt.scope,
            committed_at: receipt.committed_at.to_rfc3339(),
            replay_until: receipt.replay_until.to_rfc3339(),
            state: state as i32,
        }))
    }

    async fn replay_mutation(
        &self,
        request: Request<proto::ReplayMutationRequest>,
    ) -> Result<Response<proto::ReplayMutationResponse>, Status> {
        authorize_call(&request, RpcAction::UserMutation)?;
        if platform::mutation_context::from_request(&request)?.is_none() {
            return Err(Status::invalid_argument(
                "Operation context required for lookup",
            ));
        }
        let (metadata, mut extensions, command) = request.into_parts();
        // This private extension cannot be supplied through gRPC metadata. The
        // shared validation/mapping path below is forced to look up only.
        extensions.insert(LookupOnly);
        use proto::replay_mutation_request::Command;
        use proto::replay_mutation_response::Outcome;
        let outcome = match command
            .command
            .ok_or_else(|| Status::invalid_argument("Command required"))?
        {
            Command::CreatePost(command) => Outcome::CreatePost(
                self.create_post(Request::from_parts(metadata, extensions, command))
                    .await?
                    .into_inner(),
            ),
            Command::CreateComment(command) => Outcome::CreateComment(
                self.create_comment(Request::from_parts(metadata, extensions, command))
                    .await?
                    .into_inner(),
            ),
            Command::DeletePost(command) => Outcome::DeletePost(
                self.delete_post(Request::from_parts(metadata, extensions, command))
                    .await?
                    .into_inner(),
            ),
            Command::LikePost(command) => Outcome::LikePost(
                self.like_post(Request::from_parts(metadata, extensions, command))
                    .await?
                    .into_inner(),
            ),
            Command::UnlikePost(command) => Outcome::UnlikePost(
                self.unlike_post(Request::from_parts(metadata, extensions, command))
                    .await?
                    .into_inner(),
            ),
        };
        Ok(Response::new(proto::ReplayMutationResponse {
            outcome: Some(outcome),
        }))
    }

    async fn create_post(
        &self,
        request: Request<proto::CreatePostRequest>,
    ) -> Result<Response<proto::CreatePostResponse>, Status> {
        authorize_call(&request, RpcAction::UserMutation)?;
        let context = mutation_context(&request)?;
        let request = request.into_inner();
        let post = self
            .state
            .create_post
            .execute_with_operation(
                CreatePostDto {
                    author_id: request.author_id,
                    content: request.content,
                    visibility: request.visibility,
                },
                context,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::CreatePostResponse {
            post: Some(to_proto_post(post)),
        }))
    }

    async fn create_comment(
        &self,
        request: Request<proto::CreateCommentRequest>,
    ) -> Result<Response<proto::CreateCommentResponse>, Status> {
        authorize_call(&request, RpcAction::UserMutation)?;
        let context = mutation_context(&request)?;
        let request = request.into_inner();
        let comment = self
            .state
            .create_comment
            .execute_with_operation(
                CreateCommentDto {
                    post_id: request.post_id,
                    author_id: request.author_id,
                    content: request.content,
                },
                context,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::CreateCommentResponse {
            comment: Some(proto::Comment {
                id: comment.id,
                post_id: comment.post_id,
                author_id: comment.author_id,
                content: comment.content,
                created_at: comment.created_at,
            }),
        }))
    }

    async fn delete_post(
        &self,
        request: Request<proto::DeletePostRequest>,
    ) -> Result<Response<proto::DeletePostResponse>, Status> {
        // Removing content is a user-facing mutation: only the edge may request it.
        authorize_call(&request, RpcAction::UserMutation)?;
        let context = mutation_context(&request)?;
        let request = request.into_inner();
        let deleted = self
            .state
            .delete_post
            .execute_with_operation(
                DeletePostDto {
                    post_id: request.post_id,
                    actor_id: request.actor_id,
                    request_id: empty_string_to_option(request.request_id),
                    ip_address: empty_string_to_option(request.ip_address),
                    user_agent: empty_string_to_option(request.user_agent),
                },
                context,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::DeletePostResponse {
            post_id: deleted.post_id,
            deleted: deleted.deleted,
        }))
    }

    async fn delete_comment(
        &self,
        request: Request<proto::DeleteCommentRequest>,
    ) -> Result<Response<proto::DeleteCommentResponse>, Status> {
        let request = request.into_inner();
        let deleted = self
            .state
            .delete_comment
            .execute(DeleteCommentDto {
                comment_id: request.comment_id,
                actor_id: request.actor_id,
                request_id: empty_string_to_option(request.request_id),
                ip_address: empty_string_to_option(request.ip_address),
                user_agent: empty_string_to_option(request.user_agent),
            })
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::DeleteCommentResponse {
            comment_id: deleted.comment_id,
            post_id: deleted.post_id,
            deleted: deleted.deleted,
        }))
    }

    async fn get_feed(
        &self,
        request: Request<proto::FeedRequest>,
    ) -> Result<Response<proto::FeedResponse>, Status> {
        let request = request.into_inner();
        let feed = self
            .state
            .get_feed
            .execute_with_context(
                parse_uuid(&request.user_id, "User ID")?,
                empty_to_none(&request.cursor),
                request.limit,
                empty_to_none(&request.request_id),
                optional_uuid(&request.session_id, "Session ID")?,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(to_proto_feed(feed)))
    }

    async fn record_feed_recommendation_event(
        &self,
        request: Request<proto::RecordFeedRecommendationEventRequest>,
    ) -> Result<Response<proto::RecordFeedRecommendationEventResponse>, Status> {
        let request = request.into_inner();
        let response = self
            .state
            .record_feed_recommendation_event
            .execute(RecordFeedRecommendationEventDto {
                event_id: request.event_id,
                schema_version: u16::try_from(request.schema_version)
                    .map_err(|_| Status::invalid_argument("Schema version is invalid"))?,
                occurred_at: request.occurred_at,
                viewer_id: request.viewer_id,
                post_id: request.post_id,
                actor_id: empty_string_to_option(request.actor_id),
                position: request.position,
                request_id: empty_string_to_option(request.request_id),
                session_id: empty_string_to_option(request.session_id),
                action: request.action,
                source: empty_string_to_option(request.source),
                model_version: empty_string_to_option(request.model_version),
                impression_id: empty_string_to_option(request.impression_id),
                propensity_micros: request.propensity_micros,
                dwell_ms: request.dwell_ms,
                metadata_json: if request.metadata_json.trim().is_empty() {
                    "{}".into()
                } else {
                    request.metadata_json
                },
            })
            .await
            .map_err(to_status)?;
        Ok(Response::new(
            proto::RecordFeedRecommendationEventResponse {
                accepted: response.accepted,
            },
        ))
    }

    async fn get_author_posts(
        &self,
        request: Request<proto::AuthorPostsRequest>,
    ) -> Result<Response<proto::FeedResponse>, Status> {
        let request = request.into_inner();
        let feed = self
            .state
            .get_author_posts
            .execute(
                parse_uuid(&request.author_id, "Author ID")?,
                parse_uuid(&request.viewer_id, "Viewer ID")?,
                empty_to_none(&request.cursor),
                request.limit,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(to_proto_feed(feed)))
    }

    async fn get_author_post_counts(
        &self,
        request: Request<proto::AuthorPostCountsRequest>,
    ) -> Result<Response<proto::AuthorPostCountsResponse>, Status> {
        // A read path proxied from the edge. Counts are not viewer-specific, so
        // no viewer identity is required, but the caller still has to be a
        // verified workload.
        authorize_call(&request, RpcAction::Read)?;
        let request = request.into_inner();
        let mut author_ids = Vec::with_capacity(request.author_ids.len());
        for value in &request.author_ids {
            author_ids.push(parse_uuid(value, "Author ID")?);
        }
        let counts = self
            .state
            .get_author_post_counts
            .execute(author_ids)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::AuthorPostCountsResponse {
            counts: counts
                .into_iter()
                .map(|count| proto::AuthorPostCount {
                    author_id: count.author_id,
                    post_count: count.post_count,
                })
                .collect(),
        }))
    }

    async fn search_posts(
        &self,
        request: Request<proto::SearchPostsRequest>,
    ) -> Result<Response<proto::FeedResponse>, Status> {
        let request = request.into_inner();
        let feed = self
            .state
            .search_posts
            .execute(
                parse_uuid(&request.viewer_id, "Viewer ID")?,
                &request.query,
                empty_to_none(&request.cursor),
                request.limit,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(to_proto_feed(feed)))
    }

    async fn get_post(
        &self,
        request: Request<proto::GetPostRequest>,
    ) -> Result<Response<proto::GetPostResponse>, Status> {
        let request = request.into_inner();
        let detail = self
            .state
            .get_post_detail
            .execute(
                parse_uuid(&request.post_id, "Post ID")?,
                parse_uuid(&request.viewer_id, "Viewer ID")?,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::GetPostResponse {
            post: Some(to_proto_post(detail.post)),
            comments: detail
                .comments
                .into_iter()
                .map(|comment| proto::Comment {
                    id: comment.id,
                    post_id: comment.post_id,
                    author_id: comment.author_id,
                    content: comment.content,
                    created_at: comment.created_at,
                })
                .collect(),
        }))
    }

    async fn like_post(
        &self,
        request: Request<proto::LikePostRequest>,
    ) -> Result<Response<proto::LikePostResponse>, Status> {
        authorize_call(&request, RpcAction::UserMutation)?;
        let context = mutation_context(&request)?;
        let request = request.into_inner();
        let like = self
            .state
            .like_post
            .execute_with_operation(
                parse_uuid(&request.post_id, "Post ID")?,
                parse_uuid(&request.user_id, "User ID")?,
                context,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::LikePostResponse {
            post_id: like.post_id,
            like_count: like.like_count,
            liked: like.liked,
        }))
    }

    async fn unlike_post(
        &self,
        request: Request<proto::UnlikePostRequest>,
    ) -> Result<Response<proto::UnlikePostResponse>, Status> {
        authorize_call(&request, RpcAction::UserMutation)?;
        let context = mutation_context(&request)?;
        let request = request.into_inner();
        let like = self
            .state
            .unlike_post
            .execute_with_operation(
                parse_uuid(&request.post_id, "Post ID")?,
                parse_uuid(&request.user_id, "User ID")?,
                context,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::UnlikePostResponse {
            post_id: like.post_id,
            like_count: like.like_count,
            liked: like.liked,
        }))
    }

    async fn apply_content_moderation(
        &self,
        request: Request<proto::ApplyContentModerationRequest>,
    ) -> Result<Response<proto::ApplyContentModerationResponse>, Status> {
        // Moderation is an operator action delivered from the edge.
        authorize_call(&request, RpcAction::ApplyContentModeration)?;
        let request = request.into_inner();
        let response = self
            .state
            .apply_content_moderation
            .execute(ApplyContentModerationDto {
                report_id: request.report_id,
                target_type: request.target_type,
                target_id: request.target_id,
                actor_id: request.actor_id,
                action: request.action,
            })
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::ApplyContentModerationResponse {
            applied: response.applied,
        }))
    }

    async fn resolve_dead_letter_replay(
        &self,
        request: Request<proto::ResolveDeadLetterReplayRequest>,
    ) -> Result<Response<proto::ResolveDeadLetterReplayResponse>, Status> {
        authorize_call(&request, RpcAction::ResolveDeadLetterReplay)?;
        let request = request.into_inner();
        let response = self
            .state
            .resolve_dead_letter_replay
            .execute(ResolveDeadLetterReplayRequestDto {
                event_id: request.event_id,
                resolution_id: request.resolution_id,
                actor_id: request.actor_id,
                reason: request.reason,
                evidence_reference: request.evidence_reference,
            })
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::ResolveDeadLetterReplayResponse {
            event_id: response.event_id,
            resolution_id: response.resolution_id,
            created: response.created,
        }))
    }

    async fn apply_user_deletion(
        &self,
        request: Request<proto::ApplyUserDeletionRequest>,
    ) -> Result<Response<proto::ApplyUserDeletionResponse>, Status> {
        // Irreversible account cleanup belongs to the deletion saga alone.
        authorize_call(&request, RpcAction::ApplyUserDeletion)?;
        let deadline = platform::call_deadline::current().ok_or_else(|| {
            Status::failed_precondition("Deletion requires an original finite call deadline")
        })?;
        if deadline <= std::time::Instant::now() {
            return Err(Status::deadline_exceeded("Deletion deadline exceeded"));
        }
        let request = request.into_inner();
        let response = tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            self.state
                .apply_user_deletion
                .execute_with_receipt(ApplyUserDeletionDto {
                    saga_id: request.saga_id,
                    user_id: request.user_id,
                    actor_id: request.actor_id,
                    retention_until: request.retention_until,
                }),
        )
        .await
        .map_err(|_| Status::deadline_exceeded("Deletion deadline exceeded"))?
        .map_err(to_status)?;
        if deadline <= std::time::Instant::now() {
            return Err(Status::deadline_exceeded("Deletion deadline exceeded"));
        }
        Ok(Response::new(proto::ApplyUserDeletionResponse {
            applied: true,
            committed_receipt: Some(proto::CommittedDeletionReceipt {
                version: response.version,
                saga_id: response.saga_id.to_string(),
                user_id: response.user_id.to_string(),
                actor_id: response.actor_id.to_string(),
                retention_seconds: Some(response.retention_until.timestamp()),
                retention_nanos: Some(response.retention_until.timestamp_subsec_nanos()),
                fence_revision: response.fence_revision,
                effect_xid: response.effect_xid,
                recorded_at: response.recorded_at.to_rfc3339(),
                owner: response.owner,
            }),
        }))
    }

    async fn get_trending(
        &self,
        request: Request<proto::TrendingRequest>,
    ) -> Result<Response<proto::FeedResponse>, Status> {
        let request = request.into_inner();
        let feed = self
            .state
            .get_trending
            .execute(
                parse_uuid(&request.viewer_id, "Viewer ID")?,
                empty_to_none(&request.cursor),
                request.limit,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(to_proto_feed(feed)))
    }
}

#[allow(clippy::result_large_err)]
fn optional_uuid(value: &str, label: &str) -> Result<Option<Uuid>, Status> {
    if value.trim().is_empty() {
        Ok(None)
    } else {
        parse_uuid(value, label).map(Some)
    }
}

#[allow(clippy::result_large_err)]
fn parse_uuid(value: &str, label: &str) -> Result<Uuid, Status> {
    if value.trim().is_empty() {
        return Err(Status::invalid_argument(format!("{label} is required")));
    }
    Uuid::parse_str(value.trim())
        .map_err(|_| Status::invalid_argument(format!("{label} is required")))
}

fn empty_to_none(value: &str) -> Option<&str> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

/// The status a handler returns for `error`. A failed authority exchange is
/// also marked so the caller does not retry the whole read: Post reports it as
/// UNAVAILABLE, which a caller would otherwise treat as retryable.
fn to_status(error: anyhow::Error) -> Status {
    // anyhow reaches context values, including nested ones, by downcast.
    let not_retried = error
        .downcast_ref::<platform::grpc_retry::NotRetried>()
        .is_some();
    let mut status = owner_status(error);
    if not_retried {
        platform::grpc_retry::mark_not_retryable(&mut status);
    }
    status
}

fn owner_status(error: anyhow::Error) -> Status {
    if let Some(error) = error.chain().find_map(|source| {
        source.downcast_ref::<post_application::authority_witness::WitnessErrorDto>()
    }) {
        use post_application::authority_witness::WitnessErrorDto;
        return match error {
            WitnessErrorDto::Unavailable | WitnessErrorDto::StageConflict => {
                Status::unavailable("Query authority unavailable")
            }
            _ => Status::permission_denied("Query authority denied"),
        };
    }
    if error.chain().any(|source| {
        matches!(
            source.downcast_ref::<sqlx::Error>(),
            Some(sqlx::Error::Database(database_error))
                if database_error.constraint() == Some("post_user_deletion_fence")
        )
    }) {
        return Status::permission_denied("Account is deleted");
    }
    if let Some(error) = error
        .chain()
        .find_map(|source| source.downcast_ref::<crate::domain::post::DeadLetterReplayError>())
    {
        return match error {
            crate::domain::post::DeadLetterReplayError::EventNotFound => {
                Status::not_found("POST_EVENT_NOT_FOUND")
            }
            crate::domain::post::DeadLetterReplayError::NotDeadLettered
            | crate::domain::post::DeadLetterReplayError::Conflict => {
                Status::already_exists("POST_EVENT_DEAD_LETTER_CONFLICT")
            }
            crate::domain::post::DeadLetterReplayError::InvalidReason
            | crate::domain::post::DeadLetterReplayError::InvalidEvidenceReference => {
                Status::invalid_argument(error.to_string())
            }
        };
    }
    if let Some(error) = error.downcast_ref::<MutationError>() {
        return match error {
            MutationError::InvalidContext => Status::invalid_argument(error.to_string()),
            MutationError::Conflict | MutationError::Expired | MutationError::NotCommitted => {
                let mut status = Status::already_exists(error.to_string());
                let code = match error {
                    MutationError::Conflict => "conflict",
                    MutationError::Expired => "expired",
                    _ => "unknown",
                };
                status.metadata_mut().insert(
                    platform::mutation_context::ERROR_CODE,
                    code.parse().expect("static operation error"),
                );
                status
            }
            MutationError::Unavailable => Status::unavailable(error.to_string()),
        };
    }
    if error.is::<sqlx::Error>()
        || error.is::<serde_json::Error>()
        || error.chain().any(|cause| cause.is::<tonic::Status>())
    {
        // A storage, serialization, or dependency failure is not validation and
        // does not prove that a commit failed. Never expose database details.
        return Status::unavailable("Owner operation outcome unavailable");
    }
    if error.to_string() == "Post not found" || error.to_string() == "Comment not found" {
        Status::not_found(error.to_string())
    } else if error.to_string() == "Forbidden" {
        Status::permission_denied(error.to_string())
    } else {
        Status::invalid_argument(error.to_string())
    }
}

#[derive(Clone)]
struct LookupOnly;

#[allow(clippy::result_large_err)]
fn mutation_context<T>(request: &Request<T>) -> Result<Option<MutationContextDto>, Status> {
    Ok(
        platform::mutation_context::from_request(request)?.map(|context| MutationContextDto {
            operation_id: context.operation_id,
            actor_id: context.actor_id,
            idempotency_key: context.idempotency_key,
            lookup_only: request.extensions().get::<LookupOnly>().is_some(),
        }),
    )
}

fn empty_string_to_option(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn to_proto_post(post: crate::application::dto::post_dto::PostDto) -> proto::Post {
    proto::Post {
        id: post.id,
        author_id: post.author_id,
        content: post.content,
        visibility: post.visibility,
        created_at: post.created_at,
        comment_count: post.comment_count,
        like_count: post.like_count,
        liked_by_me: post.liked_by_me,
    }
}

fn to_proto_feed(feed: crate::application::dto::post_dto::FeedDto) -> proto::FeedResponse {
    proto::FeedResponse {
        posts: feed.posts.into_iter().map(to_proto_post).collect(),
        next_cursor: feed.next_cursor.unwrap_or_default(),
        request_id: feed.request_id.unwrap_or_default(),
        snapshot_id: feed.snapshot_id.unwrap_or_default(),
        rank_version: feed.rank_version.unwrap_or_default(),
        fallback_used: feed.fallback_used,
        items: feed
            .items
            .into_iter()
            .map(|item| proto::FeedItemMetadata {
                post_id: item.post_id,
                source: item.source,
                reason_code: item.reason_code,
                score_micros: item.score_micros,
                position: item.position,
                impression_id: item.impression_id,
                propensity_micros: item.propensity_micros,
                exploration: item.exploration,
            })
            .collect(),
        failed_sources: feed.failed_sources,
    }
}
