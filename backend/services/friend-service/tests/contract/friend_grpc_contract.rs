use friend_service::adapters::out::grpc::profile_privacy_client::GrpcProfilePrivacyClient;
use friend_service::adapters::r#in::grpc::server::proto;
use friend_service::adapters::r#in::grpc::server::proto::friend_service_server::FriendService;
use friend_service::application::ports::{ProfileDiscoveryPort, ProfilePrivacyPort};
use friend_service::domain::recommendation::ProfileDiscoverySnapshot;
use platform::grpc_auth::{VerifiedCaller, Workload};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tonic::{Code, Request};
use uuid::Uuid;

use crate::support::profile_grpc::{start_profile_grpc_server, PrivacyReply, ProfileGrpcFixture};

const SERVICE_TOKEN: &str = "friend-profile-transport-token";
const FOLLOWER_ID: &str = "11111111-1111-1111-1111-111111111111";
const FOLLOWEE_ID: &str = "22222222-2222-2222-2222-222222222222";

#[tokio::test]
async fn grpc_friend_contract_covers_follow_counts_stats_and_status() {
    let app = crate::support::start_postgres_app(true).await.expect("app");
    let grpc = crate::support::grpc_server(app.state.clone());

    let counts = proto::friend_service_server::FriendService::get_follow_counts(
        &grpc,
        Request::new(proto::GetFollowCountsRequest {
            user_id: "11111111-1111-1111-1111-111111111111".into(),
        }),
    )
    .await
    .expect("counts")
    .into_inner();
    assert_eq!(counts.following_count, 4);

    let status = proto::friend_service_server::FriendService::check_follow(
        &grpc,
        Request::new(proto::CheckFollowRequest {
            follower_id: "11111111-1111-1111-1111-111111111111".into(),
            followee_id: "44444444-4444-4444-4444-444444444444".into(),
        }),
    )
    .await
    .expect("status")
    .into_inner();
    assert!(status.following);

    let following = proto::friend_service_server::FriendService::list_following(
        &grpc,
        Request::new(proto::ListFollowingRequest {
            user_id: "11111111-1111-1111-1111-111111111111".into(),
            cursor: String::new(),
            limit: 10,
        }),
    )
    .await
    .expect("following")
    .into_inner();
    assert_eq!(following.total_count, 4);

    let follow = proto::friend_service_server::FriendService::follow_user(
        &grpc,
        gateway_request(Request::new(proto::FollowUserRequest {
            follower_id: "11111111-1111-1111-1111-111111111111".into(),
            followee_id: "55555555-5555-5555-5555-555555555555".into(),
        })),
    )
    .await
    .expect("follow")
    .into_inner();
    assert!(follow.following);

    let stats = proto::friend_service_server::FriendService::get_follow_stats(
        &grpc,
        Request::new(proto::GetFollowStatsRequest {
            user_ids: vec![
                "11111111-1111-1111-1111-111111111111".into(),
                "55555555-5555-5555-5555-555555555555".into(),
            ],
        }),
    )
    .await
    .expect("stats")
    .into_inner();
    assert_eq!(stats.stats.len(), 2);

    let unfollow = proto::friend_service_server::FriendService::unfollow_user(
        &grpc,
        gateway_request(Request::new(proto::UnfollowUserRequest {
            follower_id: "11111111-1111-1111-1111-111111111111".into(),
            followee_id: "55555555-5555-5555-5555-555555555555".into(),
        })),
    )
    .await
    .expect("unfollow")
    .into_inner();
    assert!(!unfollow.following);
    app.shutdown().await.expect("fixture shutdown");
}

#[tokio::test]
async fn follow_mutations_require_gateway_identity_and_bind_supplied_operation_actor() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");
    let grpc = crate::support::grpc_server(app.state.clone());

    let missing_identity = grpc
        .follow_user(Request::new(follow_request()))
        .await
        .expect_err("an unauthenticated workload must not follow");
    assert_eq!(missing_identity.code(), Code::Unauthenticated);

    let mut auth_request = Request::new(follow_request());
    auth_request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Auth));
    let auth_denied = grpc
        .follow_user(auth_request)
        .await
        .expect_err("Auth must not act as the Gateway");
    assert_eq!(auth_denied.code(), Code::PermissionDenied);

    let mut post_request = Request::new(proto::UnfollowUserRequest {
        follower_id: FOLLOWER_ID.into(),
        followee_id: FOLLOWEE_ID.into(),
    });
    post_request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Post));
    let post_denied = grpc
        .unfollow_user(post_request)
        .await
        .expect_err("Post must not act as the Gateway");
    assert_eq!(post_denied.code(), Code::PermissionDenied);

    let mismatched_actor = gateway_operation_request(follow_request(), Uuid::new_v4()).await;
    let mismatch = grpc
        .follow_user(mismatched_actor)
        .await
        .expect_err("operation actor must match the follower");
    assert_eq!(mismatch.code(), Code::PermissionDenied);
    assert_eq!(
        mismatch.message(),
        "Operation actor does not match follower"
    );

    let matching_actor =
        gateway_operation_request(follow_request(), Uuid::parse_str(FOLLOWER_ID).unwrap()).await;
    let allowed = grpc
        .follow_user(matching_actor)
        .await
        .expect("a matching Gateway operation actor may follow")
        .into_inner();
    assert!(allowed.following);
    app.shutdown().await.expect("fixture shutdown");
}

#[tokio::test]
async fn friend_grpc_preserves_unavailable_from_an_actually_stopped_profile_listener() {
    // Given: a production profile client whose ephemeral listener has actually stopped.
    let profile = start_profile_grpc_server(
        profile_fixture(PrivacyReply::Visibility("PUBLIC")),
        SERVICE_TOKEN,
    )
    .await
    .expect("profile server");
    let endpoint = profile.endpoint();
    profile
        .shutdown_and_verify_port_released()
        .await
        .expect("stopped profile listener");
    let privacy: Arc<dyn ProfilePrivacyPort> = Arc::new(
        GrpcProfilePrivacyClient::connect_lazy(
            endpoint,
            platform::grpc_auth::Workload::Friend,
            SERVICE_TOKEN,
            None,
        )
        .expect("profile client"),
    );
    let app = crate::support::start_postgres_app_with_dependencies(
        false,
        privacy,
        Arc::new(EmptyProfileDiscovery),
    )
    .await
    .expect("friend app");

    // When: Friend follows through the real tonic transport to the refused listener.
    let error = proto::friend_service_server::FriendService::follow_user(
        &crate::support::grpc_server(app.state.clone()),
        gateway_request(Request::new(follow_request())),
    )
    .await
    .expect_err("stopped profile listener must fail");

    // Then: Friend exposes availability, never the old invalid-argument flattening.
    assert_eq!(error.code(), Code::Unavailable);
    assert_eq!(error.message(), "Service unavailable");
    app.shutdown().await.expect("fixture shutdown");
}

#[tokio::test]
async fn friend_grpc_preserves_deadline_exceeded_from_an_actually_delayed_profile_rpc() {
    // Given: a real profile RPC delayed beyond the production two-second client timeout.
    let profile = start_profile_grpc_server(
        profile_fixture(PrivacyReply::DelayedVisibility {
            account_visibility: "PUBLIC",
            delay: Duration::from_secs(3),
        }),
        SERVICE_TOKEN,
    )
    .await
    .expect("profile server");
    let privacy: Arc<dyn ProfilePrivacyPort> = Arc::new(
        GrpcProfilePrivacyClient::connect_lazy(
            profile.endpoint(),
            platform::grpc_auth::Workload::Friend,
            SERVICE_TOKEN,
            None,
        )
        .expect("profile client"),
    );
    let app = crate::support::start_postgres_app_with_dependencies(
        false,
        privacy,
        Arc::new(EmptyProfileDiscovery),
    )
    .await
    .expect("friend app");

    // When: Friend waits for the delayed response through the actual tonic transport.
    let error = proto::friend_service_server::FriendService::follow_user(
        &crate::support::grpc_server(app.state.clone()),
        gateway_request(Request::new(follow_request())),
    )
    .await
    .expect_err("delayed profile RPC must time out");

    // Then: Friend exposes deadline exceeded and a safe boundary message.
    assert_eq!(error.code(), Code::DeadlineExceeded);
    assert_eq!(error.message(), "Request timed out");

    profile
        .shutdown_and_verify_port_released()
        .await
        .expect("profile shutdown");
    app.shutdown().await.expect("fixture shutdown");
}

fn profile_fixture(reply: PrivacyReply) -> ProfileGrpcFixture {
    ProfileGrpcFixture {
        privacy_replies: HashMap::from([(FOLLOWEE_ID.into(), reply)]),
        profiles: Vec::new(),
    }
}

fn follow_request() -> proto::FollowUserRequest {
    proto::FollowUserRequest {
        follower_id: FOLLOWER_ID.into(),
        followee_id: FOLLOWEE_ID.into(),
    }
}

fn gateway_request<T>(mut request: Request<T>) -> Request<T> {
    request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Gateway));
    request
}

async fn gateway_operation_request<T>(message: T, actor_id: Uuid) -> Request<T> {
    let context =
        platform::mutation_context::MutationContext::new(Uuid::new_v4(), actor_id, None, false);
    platform::mutation_context::scope(Some(context), async {
        let mut request = platform::mutation_context::attach(message).unwrap();
        request
            .extensions_mut()
            .insert(VerifiedCaller(Workload::Gateway));
        request
    })
    .await
}

#[derive(Clone, Copy)]
struct EmptyProfileDiscovery;

#[async_trait::async_trait]
impl ProfileDiscoveryPort for EmptyProfileDiscovery {
    async fn batch_profiles(
        &self,
        _user_ids: &[Uuid],
    ) -> anyhow::Result<Vec<ProfileDiscoverySnapshot>> {
        Ok(Vec::new())
    }
}
