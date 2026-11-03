use anyhow::Result;
use async_trait::async_trait;
use chrono::Duration as ChronoDuration;
use friend_service::adapters::out::clock::SystemPeopleRecommendationClock;
use friend_service::adapters::out::metrics::recommendation_observer::RecommendationMetricsObserver;
use friend_service::adapters::out::persistence::follow_repository::PgFollowRepository;
use friend_service::adapters::out::persistence::people_recommendation_repository::PgPeopleRecommendationRepository;
use friend_service::adapters::r#in::grpc::server::{
    proto, proto::friend_service_server::FriendService, to_status, FriendGrpcServer,
};
use friend_service::adapters::r#in::messaging::mutation_receipt_retention_worker::MutationReceiptRetentionWorker;
use friend_service::adapters::r#in::messaging::recommendation_refresh_worker::RecommendationRefreshWorker;
use friend_service::adapters::r#in::messaging::recommendation_retention_worker::PeopleRecommendationRetentionWorker;
use friend_service::application::ports::{ProfileDiscoveryPort, ProfilePrivacyPort};
use friend_service::application::ranking::LinearPeopleRanker;
use friend_service::application::use_cases::accept_follow_request::AcceptFollowRequestUseCase;
use friend_service::application::use_cases::apply_user_deletion::ApplyUserDeletionUseCase;
use friend_service::application::use_cases::block_user::BlockUserUseCase;
use friend_service::application::use_cases::cancel_follow_request::CancelFollowRequestUseCase;
use friend_service::application::use_cases::check_follow::CheckFollowUseCase;
use friend_service::application::use_cases::follow::FollowUseCase;
use friend_service::application::use_cases::get_counts::GetCountsUseCase;
use friend_service::application::use_cases::get_people_recommendations::GetPeopleRecommendationsUseCase;
use friend_service::application::use_cases::get_relationship_policy::GetRelationshipPolicyUseCase;
use friend_service::application::use_cases::get_stats_batch::GetStatsBatchUseCase;
use friend_service::application::use_cases::list_follow_requests::ListFollowRequestsUseCase;
use friend_service::application::use_cases::list_followers::ListFollowersUseCase;
use friend_service::application::use_cases::list_following::ListFollowingUseCase;
use friend_service::application::use_cases::mute_user::MuteUserUseCase;
use friend_service::application::use_cases::purge_expired_mutation_outcomes::PurgeExpiredMutationOutcomesUseCase;
use friend_service::application::use_cases::record_recommendation_event::RecordRecommendationEventUseCase;
use friend_service::application::use_cases::reject_follow_request::RejectFollowRequestUseCase;
use friend_service::application::use_cases::unblock_user::UnblockUserUseCase;
use friend_service::application::use_cases::unfollow::UnfollowUseCase;
use friend_service::application::use_cases::unmute_user::UnmuteUserUseCase;
use friend_service::config::retention_metrics::MutationReceiptRetentionMetrics;
use friend_service::config::AppState;
use friend_service::domain::error::FriendError;
use friend_service::domain::recommendation::{PeopleRankWeights, ProfileDiscoverySnapshot};
use platform::grpc_auth::{VerifiedCaller, Workload};
use platform::recommendation_metrics::RecommendationMetrics;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tonic::{Code, Request};

#[test]
fn friend_status_mapping_preserves_nested_transport_status_before_typed_domain_error() {
    // Given: an unexpected domain bucket wrapping a real downstream tonic status.
    let error = anyhow::Error::new(FriendError::Unexpected(
        tonic::Status::unavailable("private profile transport refused").into(),
    ));

    // When: the Friend gRPC boundary translates the application error.
    let status = to_status(error);

    // Then: transport availability wins over the enclosing domain bucket.
    assert_eq!(status.code(), Code::Unavailable);
    assert_eq!(status.message(), "Service unavailable");
}

#[test]
fn friend_status_mapping_is_exhaustive_for_typed_domain_errors() {
    // Given: each public Friend application error bucket.
    let cases = [
        (
            FriendError::InvalidInput("invalid follow input".into()),
            Code::InvalidArgument,
            "invalid follow input",
        ),
        (
            FriendError::NotFound("follow request not found".into()),
            Code::NotFound,
            "follow request not found",
        ),
        (
            FriendError::PermissionDenied("blocked relationship".into()),
            Code::PermissionDenied,
            "blocked relationship",
        ),
        (
            FriendError::Unexpected(anyhow::anyhow!("database password leaked")),
            Code::Internal,
            "Internal error",
        ),
    ];

    for (error, expected_code, expected_message) in cases {
        // When: the Friend gRPC boundary translates the typed error.
        let status = to_status(error);

        // Then: the code is stable and unexpected sources are redacted.
        assert_eq!(status.code(), expected_code);
        assert_eq!(status.message(), expected_message);
    }
}

#[test]
fn friend_status_mapping_redacts_untyped_repository_failures_as_internal() {
    // Given: a repository error that is neither a typed Friend error nor a tonic status.
    let error =
        anyhow::Error::new(sqlx::Error::PoolClosed).context("friend repository password leaked");

    // When: the Friend gRPC boundary translates the unchanged repository failure.
    let status = to_status(error);

    // Then: the boundary returns a stable internal status without exposing source text.
    assert_eq!(status.code(), Code::Internal);
    assert_eq!(status.message(), "Internal error");
}

#[test]
fn friend_status_mapping_reports_pool_acquire_timeout_as_sanitized_unavailable() {
    let error = FriendError::Unexpected(
        anyhow::Error::new(sqlx::Error::PoolTimedOut)
            .context("friend repository details must stay private"),
    );

    let status = to_status(error);

    assert_eq!(status.code(), Code::Unavailable);
    assert_eq!(status.message(), "Service unavailable");
}

#[tokio::test]
async fn follow_user_returns_invalid_argument_when_follower_id_is_blank() {
    let grpc = grpc_server_without_db();

    let error = grpc
        .follow_user(gateway_request(Request::new(proto::FollowUserRequest {
            follower_id: "   ".into(),
            followee_id: uuid(2),
        })))
        .await
        .expect_err("blank follower id must fail");

    assert_eq!(error.code(), Code::InvalidArgument);
    assert_eq!(error.message(), "Follower ID is required");
}

#[tokio::test]
async fn follow_user_maps_domain_rejection_to_invalid_argument() {
    let grpc = grpc_server_without_db();
    let user_id = uuid(3);

    let error = grpc
        .follow_user(gateway_request(Request::new(proto::FollowUserRequest {
            follower_id: user_id.clone(),
            followee_id: user_id,
        })))
        .await
        .expect_err("self follow must fail");

    assert_eq!(error.code(), Code::InvalidArgument);
    assert_eq!(error.message(), "Cannot follow yourself");
}

#[tokio::test]
async fn unfollow_user_maps_self_unfollow_to_invalid_argument() {
    let grpc = grpc_server_without_db();
    let user_id = uuid(4);

    let error = grpc
        .unfollow_user(gateway_request(Request::new(proto::UnfollowUserRequest {
            follower_id: user_id.clone(),
            followee_id: user_id,
        })))
        .await
        .expect_err("self unfollow must fail");

    assert_eq!(error.code(), Code::InvalidArgument);
    assert_eq!(error.message(), "Cannot unfollow yourself");
}

#[tokio::test]
async fn list_followers_rejects_invalid_cursor_before_repository_access() {
    let grpc = grpc_server_without_db();

    let error = grpc
        .list_followers(Request::new(proto::ListFollowersRequest {
            user_id: uuid(5),
            cursor: "not-a-date".into(),
            limit: 10,
        }))
        .await
        .expect_err("invalid cursor must fail");

    assert_eq!(error.code(), Code::InvalidArgument);
    assert_eq!(error.message(), "Invalid cursor");
}

#[tokio::test]
async fn list_following_rejects_invalid_user_id_before_repository_access() {
    let grpc = grpc_server_without_db();

    let error = grpc
        .list_following(Request::new(proto::ListFollowingRequest {
            user_id: "not-a-uuid".into(),
            cursor: String::new(),
            limit: 10,
        }))
        .await
        .expect_err("invalid user id must fail");

    assert_eq!(error.code(), Code::InvalidArgument);
    assert_eq!(error.message(), "User ID is required");
}

#[tokio::test]
async fn incoming_follow_requests_reject_malformed_cursor_as_invalid_argument() {
    let grpc = grpc_server_without_db();

    let error = grpc
        .list_incoming_follow_requests(Request::new(proto::ListFollowRequestsRequest {
            user_id: uuid(51),
            cursor: "malformed-cursor".into(),
            limit: 10,
        }))
        .await
        .expect_err("malformed incoming cursor must fail");

    assert_eq!(error.code(), Code::InvalidArgument);
    assert_eq!(error.message(), "Invalid cursor");
}

#[tokio::test]
async fn outgoing_follow_requests_reject_malformed_cursor_as_invalid_argument() {
    let grpc = grpc_server_without_db();

    let error = grpc
        .list_outgoing_follow_requests(Request::new(proto::ListFollowRequestsRequest {
            user_id: uuid(52),
            cursor: "malformed-cursor".into(),
            limit: 10,
        }))
        .await
        .expect_err("malformed outgoing cursor must fail");

    assert_eq!(error.code(), Code::InvalidArgument);
    assert_eq!(error.message(), "Invalid cursor");
}

#[tokio::test]
async fn get_follow_counts_rejects_blank_user_id() {
    let grpc = grpc_server_without_db();

    let error = grpc
        .get_follow_counts(Request::new(proto::GetFollowCountsRequest {
            user_id: "\t".into(),
        }))
        .await
        .expect_err("blank user id must fail");

    assert_eq!(error.code(), Code::InvalidArgument);
    assert_eq!(error.message(), "User ID is required");
}

#[tokio::test]
async fn get_follow_stats_rejects_invalid_user_id_in_batch() {
    let grpc = grpc_server_without_db();

    let error = grpc
        .get_follow_stats(Request::new(proto::GetFollowStatsRequest {
            user_ids: vec![uuid(6), "invalid".into()],
        }))
        .await
        .expect_err("invalid batch user id must fail");

    assert_eq!(error.code(), Code::InvalidArgument);
    assert_eq!(error.message(), "User ID is required");
}

#[tokio::test]
async fn get_follow_stats_returns_empty_stats_for_empty_batch() {
    let grpc = grpc_server_without_db();

    let response = grpc
        .get_follow_stats(Request::new(proto::GetFollowStatsRequest {
            user_ids: Vec::new(),
        }))
        .await
        .expect("empty batch stats")
        .into_inner();

    assert!(response.stats.is_empty());
}

#[tokio::test]
async fn check_follow_rejects_invalid_followee_id() {
    let grpc = grpc_server_without_db();

    let error = grpc
        .check_follow(Request::new(proto::CheckFollowRequest {
            follower_id: uuid(7),
            followee_id: "invalid".into(),
        }))
        .await
        .expect_err("invalid followee id must fail");

    assert_eq!(error.code(), Code::InvalidArgument);
    assert_eq!(error.message(), "Followee ID is required");
}

fn grpc_server_without_db() -> FriendGrpcServer {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://postgres:postgres@localhost:5432/friend_unit")
        .expect("lazy pool");
    let repository = Arc::new(PgFollowRepository::new(pool.clone()));
    let recommendation_repository = Arc::new(PgPeopleRecommendationRepository::new(pool));
    let metrics = Arc::new(RecommendationMetrics::default());
    let receipt_metrics = Arc::new(MutationReceiptRetentionMetrics::default());
    let privacy: Arc<dyn ProfilePrivacyPort> =
        Arc::new(crate::public_profile_privacy::PublicProfilePrivacy);
    let discovery: Arc<dyn ProfileDiscoveryPort> = Arc::new(EmptyProfileDiscovery);
    FriendGrpcServer::new(Arc::new(AppState {
        apply_user_deletion: ApplyUserDeletionUseCase::new(Arc::clone(&repository)),
        follow: FollowUseCase::new(Arc::clone(&repository), privacy),
        unfollow: UnfollowUseCase::new(Arc::clone(&repository)),
        list_followers: ListFollowersUseCase::new(Arc::clone(&repository)),
        list_following: ListFollowingUseCase::new(Arc::clone(&repository)),
        get_counts: GetCountsUseCase::new(Arc::clone(&repository)),
        get_stats_batch: GetStatsBatchUseCase::new(Arc::clone(&repository)),
        check_follow: CheckFollowUseCase::new(Arc::clone(&repository)),
        accept_follow_request: AcceptFollowRequestUseCase::new(Arc::clone(&repository)),
        reject_follow_request: RejectFollowRequestUseCase::new(Arc::clone(&repository)),
        cancel_follow_request: CancelFollowRequestUseCase::new(Arc::clone(&repository)),
        list_follow_requests: ListFollowRequestsUseCase::new(Arc::clone(&repository)),
        block_user: BlockUserUseCase::new(Arc::clone(&repository)),
        unblock_user: UnblockUserUseCase::new(Arc::clone(&repository)),
        mute_user: MuteUserUseCase::new(Arc::clone(&repository)),
        unmute_user: UnmuteUserUseCase::new(Arc::clone(&repository)),
        get_relationship: GetRelationshipPolicyUseCase::new(Arc::clone(&repository)),
        relationship_repository: Arc::clone(&repository),
        get_people_recommendations: GetPeopleRecommendationsUseCase::new(
            Arc::clone(&recommendation_repository),
            discovery,
            repository.clone(),
            Arc::new(LinearPeopleRanker::new(PeopleRankWeights::default())),
            Arc::new(SystemPeopleRecommendationClock),
            Arc::new(RecommendationMetricsObserver::new(metrics.clone())),
        ),
        record_recommendation_event: RecordRecommendationEventUseCase::new(Arc::clone(
            &recommendation_repository,
        )),
        recommendation_refresh_worker: RecommendationRefreshWorker::new(
            recommendation_repository.clone(),
            "people-linear-v1".into(),
            ChronoDuration::minutes(5),
            100,
            5,
            ChronoDuration::seconds(1),
        ),
        recommendation_retention_worker: PeopleRecommendationRetentionWorker::new(
            recommendation_repository,
            metrics.clone(),
            ChronoDuration::days(90),
            500,
        ),
        recommendation_metrics: metrics,
        mutation_receipt_retention_worker: MutationReceiptRetentionWorker::new(
            PurgeExpiredMutationOutcomesUseCase::new(repository),
            receipt_metrics.clone(),
            1_000,
        ),
        mutation_receipt_retention_metrics: receipt_metrics,
        edge_deadline_metrics: Default::default(),
    }))
}

#[derive(Clone)]
struct EmptyProfileDiscovery;

#[async_trait]
impl ProfileDiscoveryPort for EmptyProfileDiscovery {
    async fn batch_profiles(
        &self,
        _user_ids: &[uuid::Uuid],
    ) -> Result<Vec<ProfileDiscoverySnapshot>> {
        Ok(Vec::new())
    }
}

fn uuid(suffix: u128) -> String {
    uuid::Uuid::from_u128(suffix).to_string()
}

fn gateway_request<T>(mut request: Request<T>) -> Request<T> {
    request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Gateway));
    request
}
