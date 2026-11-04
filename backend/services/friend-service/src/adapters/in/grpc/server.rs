use crate::application::dto::friend_dto::ApplyUserDeletionDto;
use crate::application::dto::friend_dto::{FollowEdgeDto, FollowRequestDto, RelationshipPolicyDto};
use crate::application::dto::mutation_dto::{MutationContextDto, MutationError};
use crate::application::ports::{FollowRepositoryPort, RelationshipPolicyRepositoryPort};
use crate::application::use_cases::get_people_recommendations::GetPeopleRecommendationsQuery;
use crate::application::use_cases::list_follow_requests::FollowRequestListKind;
use crate::config::AppState;
use crate::domain::error::FriendError;
use crate::domain::follow_edge::FollowCursor;
use crate::domain::relationship_policy::RelationshipPolicy;
use chrono::{DateTime, Utc};
use platform::grpc_auth::{authorize_call, RpcAction};
use platform::recommendation::{
    RecommendationAction, RecommendationContractError, RecommendationEvent,
    RecommendationSubjectType, RecommendationSurface,
};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub mod proto {
    tonic::include_proto!("friend.v1");
}

#[derive(Clone)]
pub struct FriendGrpcServer {
    state: Arc<AppState>,
}

impl FriendGrpcServer {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl proto::friend_service_server::FriendService for FriendGrpcServer {
    async fn get_mutation_receipt_status(
        &self,
        request: Request<proto::GetMutationReceiptStatusRequest>,
    ) -> Result<Response<proto::GetMutationReceiptStatusResponse>, Status> {
        authorize_call(&request, RpcAction::MutationReceiptLookup)?;
        let operation_id = Uuid::parse_str(&request.into_inner().operation_id)
            .map_err(|_| Status::invalid_argument("Operation ID must be a UUID"))?;
        let receipt = self
            .state
            .relationship_repository
            .mutation_receipt_status(operation_id)
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
        let (metadata, mut extensions, request) = request.into_parts();
        // Only this RPC can install the lookup-only marker; incoming metadata
        // is never allowed to turn an ordinary write into an unaudited lookup.
        extensions.insert(LookupOnly);
        use proto::replay_mutation_request::Command;
        use proto::replay_mutation_response::Outcome;
        let outcome = match request
            .command
            .ok_or_else(|| Status::invalid_argument("Mutation command required"))?
        {
            Command::FollowUser(command) => Outcome::FollowUser(
                self.follow_user(Request::from_parts(metadata, extensions, command))
                    .await?
                    .into_inner(),
            ),
            Command::UnfollowUser(command) => Outcome::UnfollowUser(
                self.unfollow_user(Request::from_parts(metadata, extensions, command))
                    .await?
                    .into_inner(),
            ),
        };
        Ok(Response::new(proto::ReplayMutationResponse {
            outcome: Some(outcome),
        }))
    }

    async fn follow_user(
        &self,
        request: Request<proto::FollowUserRequest>,
    ) -> Result<Response<proto::FollowUserResponse>, Status> {
        authorize_call(&request, RpcAction::UserMutation)?;
        let operation = mutation_context(&request)?;
        let request = request.into_inner();
        let follower_id = parse_uuid(&request.follower_id, "Follower ID")?;
        let followee_id = parse_uuid(&request.followee_id, "Followee ID")?;
        if operation
            .as_ref()
            .is_some_and(|context| context.actor_id != follower_id)
        {
            return Err(Status::permission_denied(
                "Operation actor does not match follower",
            ));
        }
        let response = self
            .state
            .follow
            .execute_with_operation(follower_id, followee_id, operation)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::FollowUserResponse {
            edge: response.edge.map(to_proto_edge),
            following: response.following,
            status: response.status,
            request_id: response.request_id.unwrap_or_default(),
        }))
    }

    async fn unfollow_user(
        &self,
        request: Request<proto::UnfollowUserRequest>,
    ) -> Result<Response<proto::UnfollowUserResponse>, Status> {
        authorize_call(&request, RpcAction::UserMutation)?;
        let operation = mutation_context(&request)?;
        let request = request.into_inner();
        let follower_id = parse_uuid(&request.follower_id, "Follower ID")?;
        let followee_id = parse_uuid(&request.followee_id, "Followee ID")?;
        if operation
            .as_ref()
            .is_some_and(|context| context.actor_id != follower_id)
        {
            return Err(Status::permission_denied(
                "Operation actor does not match follower",
            ));
        }
        let response = self
            .state
            .unfollow
            .execute_with_operation(follower_id, followee_id, operation)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::UnfollowUserResponse {
            following: response.following,
        }))
    }

    async fn list_followers(
        &self,
        request: Request<proto::ListFollowersRequest>,
    ) -> Result<Response<proto::ListFollowersResponse>, Status> {
        let request = request.into_inner();
        let page = self
            .state
            .list_followers
            .execute(
                parse_uuid(&request.user_id, "User ID")?,
                parse_cursor(&request.cursor)?,
                request.limit,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::ListFollowersResponse {
            edges: page.edges.into_iter().map(to_proto_edge).collect(),
            next_cursor: page.next_cursor.unwrap_or_default(),
            total_count: page.total_count,
        }))
    }

    async fn list_following(
        &self,
        request: Request<proto::ListFollowingRequest>,
    ) -> Result<Response<proto::ListFollowingResponse>, Status> {
        let request = request.into_inner();
        let page = self
            .state
            .list_following
            .execute(
                parse_uuid(&request.user_id, "User ID")?,
                parse_cursor(&request.cursor)?,
                request.limit,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::ListFollowingResponse {
            edges: page.edges.into_iter().map(to_proto_edge).collect(),
            next_cursor: page.next_cursor.unwrap_or_default(),
            total_count: page.total_count,
        }))
    }

    async fn get_follow_counts(
        &self,
        request: Request<proto::GetFollowCountsRequest>,
    ) -> Result<Response<proto::GetFollowCountsResponse>, Status> {
        let counts = self
            .state
            .get_counts
            .execute(parse_uuid(&request.into_inner().user_id, "User ID")?)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::GetFollowCountsResponse {
            follower_count: counts.follower_count,
            following_count: counts.following_count,
        }))
    }

    async fn get_follow_stats(
        &self,
        request: Request<proto::GetFollowStatsRequest>,
    ) -> Result<Response<proto::GetFollowStatsResponse>, Status> {
        let request = request.into_inner();
        if request.user_ids.len() > 100 {
            return Err(Status::invalid_argument(
                "Follow stats support at most 100 user IDs",
            ));
        }
        let user_ids = request
            .user_ids
            .into_iter()
            .map(|value| parse_uuid(&value, "User ID"))
            .collect::<Result<Vec<_>, _>>()?;
        let stats = self
            .state
            .get_stats_batch
            .execute(&user_ids)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::GetFollowStatsResponse {
            stats: stats
                .into_iter()
                .map(|stat| proto::FollowStats {
                    user_id: stat.user_id,
                    follower_count: stat.follower_count,
                    following_count: stat.following_count,
                })
                .collect(),
        }))
    }

    async fn check_follow(
        &self,
        request: Request<proto::CheckFollowRequest>,
    ) -> Result<Response<proto::CheckFollowResponse>, Status> {
        let request = request.into_inner();
        let following = self
            .state
            .check_follow
            .execute(
                parse_uuid(&request.follower_id, "Follower ID")?,
                parse_uuid(&request.followee_id, "Followee ID")?,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::CheckFollowResponse { following }))
    }

    async fn accept_follow_request(
        &self,
        request: Request<proto::FollowRequestActionRequest>,
    ) -> Result<Response<proto::FollowRequestActionResponse>, Status> {
        let request = request.into_inner();
        let response = self
            .state
            .accept_follow_request
            .execute(
                parse_uuid(&request.request_id, "Request ID")?,
                parse_uuid(&request.actor_id, "Actor ID")?,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::FollowRequestActionResponse {
            request: None,
            edge: response.edge.map(to_proto_edge),
            following: response.following,
            status: response.status,
        }))
    }

    async fn reject_follow_request(
        &self,
        request: Request<proto::FollowRequestActionRequest>,
    ) -> Result<Response<proto::FollowRequestActionResponse>, Status> {
        let request = request.into_inner();
        let response = self
            .state
            .reject_follow_request
            .execute(
                parse_uuid(&request.request_id, "Request ID")?,
                parse_uuid(&request.actor_id, "Actor ID")?,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::FollowRequestActionResponse {
            status: response.status.clone(),
            request: Some(to_proto_request(response)),
            edge: None,
            following: false,
        }))
    }

    async fn cancel_follow_request(
        &self,
        request: Request<proto::FollowRequestActionRequest>,
    ) -> Result<Response<proto::FollowRequestActionResponse>, Status> {
        let request = request.into_inner();
        let response = self
            .state
            .cancel_follow_request
            .execute(
                parse_uuid(&request.request_id, "Request ID")?,
                parse_uuid(&request.actor_id, "Actor ID")?,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::FollowRequestActionResponse {
            status: response.status.clone(),
            request: Some(to_proto_request(response)),
            edge: None,
            following: false,
        }))
    }

    async fn list_incoming_follow_requests(
        &self,
        request: Request<proto::ListFollowRequestsRequest>,
    ) -> Result<Response<proto::ListFollowRequestsResponse>, Status> {
        self.list_follow_requests(request.into_inner(), FollowRequestListKind::Incoming)
            .await
    }

    async fn list_outgoing_follow_requests(
        &self,
        request: Request<proto::ListFollowRequestsRequest>,
    ) -> Result<Response<proto::ListFollowRequestsResponse>, Status> {
        self.list_follow_requests(request.into_inner(), FollowRequestListKind::Outgoing)
            .await
    }

    async fn block_user(
        &self,
        request: Request<proto::RelationshipRequest>,
    ) -> Result<Response<proto::RelationshipPolicy>, Status> {
        run_relationship(request.into_inner(), |viewer, target| {
            self.state.block_user.execute(viewer, target)
        })
        .await
    }

    async fn unblock_user(
        &self,
        request: Request<proto::RelationshipRequest>,
    ) -> Result<Response<proto::RelationshipPolicy>, Status> {
        run_relationship(request.into_inner(), |viewer, target| {
            self.state.unblock_user.execute(viewer, target)
        })
        .await
    }

    async fn mute_user(
        &self,
        request: Request<proto::RelationshipRequest>,
    ) -> Result<Response<proto::RelationshipPolicy>, Status> {
        run_relationship(request.into_inner(), |viewer, target| {
            self.state.mute_user.execute(viewer, target)
        })
        .await
    }

    async fn unmute_user(
        &self,
        request: Request<proto::RelationshipRequest>,
    ) -> Result<Response<proto::RelationshipPolicy>, Status> {
        run_relationship(request.into_inner(), |viewer, target| {
            self.state.unmute_user.execute(viewer, target)
        })
        .await
    }

    async fn get_relationship(
        &self,
        request: Request<proto::RelationshipRequest>,
    ) -> Result<Response<proto::RelationshipPolicy>, Status> {
        let request = request.into_inner();
        let response = self
            .state
            .get_relationship
            .execute(
                parse_uuid(&request.viewer_id, "Viewer ID")?,
                parse_uuid(&request.target_id, "Target ID")?,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(to_proto_policy(response)))
    }

    async fn get_people_recommendations(
        &self,
        request: Request<proto::GetPeopleRecommendationsRequest>,
    ) -> Result<Response<proto::GetPeopleRecommendationsResponse>, Status> {
        let request = request.into_inner();
        let page = self
            .state
            .get_people_recommendations
            .execute(GetPeopleRecommendationsQuery {
                viewer_id: parse_uuid(&request.viewer_id, "Viewer ID")?,
                cursor: non_empty(&request.cursor).map(ToOwned::to_owned),
                limit: request.limit,
                request_id: required_text(&request.request_id, "Request ID")?,
                session_id: parse_optional_uuid(&request.session_id, "Session ID")?,
            })
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::GetPeopleRecommendationsResponse {
            items: page
                .items
                .into_iter()
                .map(|item| proto::PeopleRecommendation {
                    user_id: item.candidate_user_id.to_string(),
                    username: item.profile.username,
                    nickname: item.profile.nickname,
                    bio: item.profile.bio,
                    avatar_url: item.profile.avatar_url,
                    source: item.source,
                    reason_code: item.reason_code,
                    score_micros: item.score_micros,
                    rank_version: item.rank_version,
                    impression_id: item.impression_id.to_string(),
                })
                .collect(),
            next_cursor: page.next_cursor.unwrap_or_default(),
            snapshot_id: page.snapshot_id.to_string(),
            rank_version: page.rank_version,
            request_id: page.request_id,
            fallback_used: page.fallback_used,
        }))
    }

    async fn record_people_recommendation_event(
        &self,
        request: Request<proto::RecordRecommendationEventRequest>,
    ) -> Result<Response<proto::RecordRecommendationEventResponse>, Status> {
        let request = request.into_inner();
        let schema_version = u16::try_from(request.schema_version)
            .map_err(|_| Status::invalid_argument("Schema version is invalid"))?;
        let position = optional_u32(request.position, "Position")?;
        let propensity_micros = optional_positive_u32(request.propensity_micros, "Propensity")?;
        let dwell_ms = optional_positive_u32(request.dwell_ms, "Dwell time")?;
        let occurred_at = DateTime::parse_from_rfc3339(request.occurred_at.trim())
            .map_err(|_| Status::invalid_argument("Occurred at is invalid"))?
            .with_timezone(&Utc);
        let event = RecommendationEvent {
            event_id: parse_uuid(&request.event_id, "Event ID")?,
            schema_version,
            occurred_at,
            viewer_id: parse_uuid(&request.viewer_id, "Viewer ID")?,
            subject_type: RecommendationSubjectType::parse(&request.subject_type)
                .map_err(|error| Status::invalid_argument(error.to_string()))?,
            subject_id: parse_uuid(&request.subject_id, "Subject ID")?,
            actor_id: parse_optional_uuid(&request.actor_id, "Actor ID")?,
            surface: RecommendationSurface::parse(&request.surface)
                .map_err(|error| Status::invalid_argument(error.to_string()))?,
            position,
            request_id: non_empty(&request.request_id).map(ToOwned::to_owned),
            session_id: parse_optional_uuid(&request.session_id, "Session ID")?,
            action: RecommendationAction::parse(&request.action)
                .map_err(|error| Status::invalid_argument(error.to_string()))?,
            source: request.source,
            model_version: request.model_version,
            impression_id: parse_optional_uuid(&request.impression_id, "Impression ID")?,
            propensity_micros,
            dwell_ms,
            metadata_json: if request.metadata_json.trim().is_empty() {
                "{}".into()
            } else {
                request.metadata_json
            },
        };
        let accepted = self
            .state
            .record_recommendation_event
            .execute(event)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::RecordRecommendationEventResponse {
            accepted,
        }))
    }

    async fn apply_user_deletion(
        &self,
        request: Request<proto::ApplyUserDeletionRequest>,
    ) -> Result<Response<proto::ApplyUserDeletionResponse>, Status> {
        // Irreversible account cleanup belongs to the deletion saga alone.
        platform::grpc_auth::authorize_call(
            &request,
            platform::grpc_auth::RpcAction::ApplyUserDeletion,
        )?;
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

    async fn batch_get_relationships(
        &self,
        request: Request<proto::BatchRelationshipRequest>,
    ) -> Result<Response<proto::BatchRelationshipResponse>, Status> {
        let request = request.into_inner();
        if request.target_ids.len() > 100 {
            return Err(Status::invalid_argument(
                "Relationship batch supports at most 100 target IDs",
            ));
        }
        let viewer_id = parse_uuid(&request.viewer_id, "Viewer ID")?;
        let target_ids = request
            .target_ids
            .into_iter()
            .map(|value| parse_uuid(&value, "Target ID"))
            .collect::<Result<Vec<_>, _>>()?;
        #[cfg(feature = "process-stack-test-hooks")]
        await_process_stack_relationship_gate().await?;
        let policies = self
            .state
            .relationship_repository
            .batch_policy(viewer_id, &target_ids)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::BatchRelationshipResponse {
            policies: relationship_policy_entries(&target_ids, &policies)?,
        }))
    }
}

#[cfg(feature = "process-stack-test-hooks")]
async fn await_process_stack_relationship_gate() -> Result<(), Status> {
    let Some(directory) = std::env::var_os("FRIEND_PROCESS_STACK_RELATIONSHIP_GATE_DIR") else {
        return Ok(());
    };
    await_process_stack_relationship_gate_in(std::path::PathBuf::from(directory)).await
}

#[cfg(feature = "process-stack-test-hooks")]
async fn await_process_stack_relationship_gate_in(
    directory: impl AsRef<std::path::Path>,
) -> Result<(), Status> {
    let directory = directory.as_ref();
    let armed = directory.join("armed");
    let entered = directory.join("entered");
    let released = directory.join("released");

    match std::fs::rename(&armed, &entered) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => {
            return Err(Status::internal(
                "Process-stack relationship gate could not be armed",
            ));
        }
    }
    std::fs::write(&entered, b"paused").map_err(|_| {
        Status::internal("Process-stack relationship gate could not announce readiness")
    })?;

    let wait_started = std::time::Instant::now();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while !released.exists() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| Status::deadline_exceeded("Process-stack relationship gate timed out"))?;
    tracing::info!(
        wait_ms = wait_started.elapsed().as_millis(),
        "process-stack relationship gate released"
    );
    Ok(())
}

#[cfg(all(test, feature = "process-stack-test-hooks"))]
mod process_stack_relationship_gate_tests {
    use super::await_process_stack_relationship_gate_in;
    use anyhow::{Context, Result};
    use std::{path::PathBuf, time::Duration};
    use tokio::time::{sleep, timeout};
    use uuid::Uuid;

    struct TemporaryGateDirectory(PathBuf);

    impl Drop for TemporaryGateDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn process_stack_relationship_gate_waits_for_release_once() -> Result<()> {
        let directory = TemporaryGateDirectory(std::env::temp_dir().join(format!(
            "port-rust-friend-relationship-gate-test-{}",
            Uuid::new_v4().simple()
        )));
        std::fs::create_dir(&directory.0)?;
        std::fs::write(directory.0.join("armed"), b"arm")?;

        let gate_directory = directory.0.clone();
        let gate =
            tokio::spawn(
                async move { await_process_stack_relationship_gate_in(gate_directory).await },
            );
        timeout(Duration::from_secs(1), async {
            loop {
                if directory.0.join("entered").exists() {
                    return;
                }
                sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .context("test gate did not signal entry")?;
        assert!(
            !gate.is_finished(),
            "gate returned before its release marker"
        );

        std::fs::write(directory.0.join("released"), b"release")?;
        gate.await
            .context("gate task panicked")?
            .expect("release marker should allow the gate to complete");

        timeout(
            Duration::from_millis(100),
            await_process_stack_relationship_gate_in(&directory.0),
        )
        .await
        .context("one-shot gate blocked a later relationship request")?
        .expect("a consumed gate must not pause a later request");
        Ok(())
    }
}

fn relationship_policy_entries(
    target_ids: &[Uuid],
    policies: &HashMap<Uuid, RelationshipPolicy>,
) -> Result<Vec<proto::RelationshipPolicyEntry>, Status> {
    let requested: HashSet<Uuid> = target_ids.iter().copied().collect();
    if policies.len() != requested.len() || policies.keys().any(|id| !requested.contains(id)) {
        return Err(Status::internal(
            "Relationship policy lookup returned an inconsistent result",
        ));
    }

    target_ids
        .iter()
        .map(|target_id| {
            let policy = policies.get(target_id).copied().ok_or_else(|| {
                Status::internal("Relationship policy lookup returned an inconsistent result")
            })?;
            Ok(proto::RelationshipPolicyEntry {
                target_id: target_id.to_string(),
                policy: Some(to_proto_policy(policy.into())),
            })
        })
        .collect()
}

#[cfg(test)]
mod relationship_policy_batch_tests {
    use super::*;
    use tonic::Code;

    #[test]
    fn missing_policy_is_not_replaced_by_an_all_false_policy() {
        let present = Uuid::from_u128(1);
        let missing = Uuid::from_u128(2);
        let policies = HashMap::from([(
            present,
            RelationshipPolicy {
                blocked_by_viewer: true,
                ..RelationshipPolicy::default()
            },
        )]);

        let error = relationship_policy_entries(&[present, missing], &policies)
            .expect_err("every requested target must have an authoritative policy");

        assert_eq!(error.code(), Code::Internal);
        assert_eq!(
            error.message(),
            "Relationship policy lookup returned an inconsistent result"
        );
    }

    #[test]
    fn unexpected_policy_is_not_silently_ignored() {
        let requested = Uuid::from_u128(3);
        let unexpected = Uuid::from_u128(4);
        let policies = HashMap::from([
            (requested, RelationshipPolicy::default()),
            (
                unexpected,
                RelationshipPolicy {
                    blocked_viewer: true,
                    ..RelationshipPolicy::default()
                },
            ),
        ]);

        let error = relationship_policy_entries(&[requested], &policies)
            .expect_err("the repository result must exactly match requested targets");

        assert_eq!(error.code(), Code::Internal);
        assert_eq!(
            error.message(),
            "Relationship policy lookup returned an inconsistent result"
        );
    }
}

impl FriendGrpcServer {
    async fn list_follow_requests(
        &self,
        request: proto::ListFollowRequestsRequest,
        kind: FollowRequestListKind,
    ) -> Result<Response<proto::ListFollowRequestsResponse>, Status> {
        let page = self
            .state
            .list_follow_requests
            .execute(
                parse_uuid(&request.user_id, "User ID")?,
                non_empty(&request.cursor),
                request.limit,
                kind,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::ListFollowRequestsResponse {
            requests: page.requests.into_iter().map(to_proto_request).collect(),
            next_cursor: page.next_cursor.unwrap_or_default(),
        }))
    }
}

async fn run_relationship<F, Fut>(
    request: proto::RelationshipRequest,
    operation: F,
) -> Result<Response<proto::RelationshipPolicy>, Status>
where
    F: FnOnce(Uuid, Uuid) -> Fut,
    Fut: Future<Output = Result<RelationshipPolicyDto, FriendError>>,
{
    let response = operation(
        parse_uuid(&request.viewer_id, "Viewer ID")?,
        parse_uuid(&request.target_id, "Target ID")?,
    )
    .await
    .map_err(to_status)?;
    Ok(Response::new(to_proto_policy(response)))
}

fn non_empty(value: &str) -> Option<&str> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn to_proto_request(value: FollowRequestDto) -> proto::FollowRequest {
    proto::FollowRequest {
        id: value.id,
        follower_id: value.follower_id,
        followee_id: value.followee_id,
        status: value.status,
        created_at: value.created_at,
        decided_at: value.decided_at.unwrap_or_default(),
    }
}

fn to_proto_policy(value: RelationshipPolicyDto) -> proto::RelationshipPolicy {
    proto::RelationshipPolicy {
        blocked_by_viewer: value.blocked_by_viewer,
        blocked_viewer: value.blocked_viewer,
        muted_by_viewer: value.muted_by_viewer,
        following: value.following,
    }
}

#[allow(clippy::result_large_err)]
fn required_text(value: &str, label: &str) -> Result<String, Status> {
    let value = value.trim();
    if value.is_empty() {
        return Err(Status::invalid_argument(format!("{label} is required")));
    }
    Ok(value.to_owned())
}

#[allow(clippy::result_large_err)]
fn parse_optional_uuid(value: &str, label: &str) -> Result<Option<Uuid>, Status> {
    if value.trim().is_empty() {
        Ok(None)
    } else {
        parse_uuid(value, label).map(Some)
    }
}

#[allow(clippy::result_large_err)]
fn optional_u32(value: i32, label: &str) -> Result<Option<u32>, Status> {
    if value < 0 {
        Ok(None)
    } else {
        u32::try_from(value)
            .map(Some)
            .map_err(|_| Status::invalid_argument(format!("{label} is invalid")))
    }
}

#[allow(clippy::result_large_err)]
fn optional_positive_u32(value: i32, label: &str) -> Result<Option<u32>, Status> {
    if value <= 0 {
        Ok(None)
    } else {
        u32::try_from(value)
            .map(Some)
            .map_err(|_| Status::invalid_argument(format!("{label} is invalid")))
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

#[allow(clippy::result_large_err)]
fn parse_cursor(value: &str) -> Result<Option<FollowCursor>, Status> {
    FollowCursor::parse(value).map_err(|_| Status::invalid_argument("Invalid cursor"))
}

pub fn to_status(error: impl Into<anyhow::Error>) -> Status {
    let error = error.into();
    if let Some(status) = error
        .chain()
        .find_map(|source| source.downcast_ref::<Status>())
    {
        return match status.code() {
            tonic::Code::Unavailable => Status::unavailable("Service unavailable"),
            tonic::Code::DeadlineExceeded => Status::deadline_exceeded("Request timed out"),
            tonic::Code::Internal => Status::internal("Internal error"),
            _ => status.clone(),
        };
    }
    if error.chain().any(|source| {
        matches!(
            source.downcast_ref::<sqlx::Error>(),
            Some(sqlx::Error::PoolTimedOut)
        )
    }) {
        return Status::unavailable("Service unavailable");
    }
    if error.chain().any(|source| {
        matches!(
            source.downcast_ref::<sqlx::Error>(),
            Some(sqlx::Error::Database(database_error))
                if database_error.constraint() == Some("friend_user_deletion_fence")
        )
    }) {
        return Status::permission_denied("Account is deleted");
    }
    if let Some(mutation_error) = error
        .chain()
        .find_map(|source| source.downcast_ref::<MutationError>())
    {
        return match mutation_error {
            MutationError::InvalidContext => Status::invalid_argument(mutation_error.to_string()),
            MutationError::Conflict | MutationError::Expired | MutationError::NotCommitted => {
                let mut status = Status::already_exists(mutation_error.to_string());
                let code = match mutation_error {
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
            MutationError::Unavailable => Status::unavailable(mutation_error.to_string()),
        };
    }
    if let Some(friend_error) = error
        .chain()
        .find_map(|source| source.downcast_ref::<FriendError>())
    {
        return match friend_error {
            FriendError::InvalidInput(message) => Status::invalid_argument(message.clone()),
            FriendError::NotFound(message) => Status::not_found(message.clone()),
            FriendError::PermissionDenied(message) => Status::permission_denied(message.clone()),
            FriendError::Unexpected(_) => Status::internal("Internal error"),
        };
    }
    if let Some(contract_error) = error
        .chain()
        .find_map(|source| source.downcast_ref::<RecommendationContractError>())
    {
        return Status::invalid_argument(contract_error.to_string());
    }
    Status::internal("Internal error")
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

fn to_proto_edge(edge: FollowEdgeDto) -> proto::FollowEdge {
    proto::FollowEdge {
        follower_id: edge.follower_id,
        followee_id: edge.followee_id,
        created_at: edge.created_at,
    }
}
