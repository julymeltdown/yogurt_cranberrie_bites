use crate::support::profile_grpc::{start_profile_grpc_server, PrivacyReply, ProfileGrpcFixture};
use friend_service::adapters::out::grpc::profile_privacy_client::{
    proto, GrpcProfilePrivacyClient,
};
use friend_service::application::ports::{ProfileDiscoveryPort, ProfilePrivacyPort};
use friend_service::domain::follow_request::AccountVisibility;
use friend_service::domain::recommendation::ProfileDiscoverySnapshot;
use std::collections::HashMap;
use std::time::Duration;
use tonic::Code;
use uuid::Uuid;

const SERVICE_TOKEN: &str = "friend-profile-contract-token";
const PUBLIC_ID: &str = "11111111-1111-1111-1111-111111111111";
const PRIVATE_ID: &str = "22222222-2222-2222-2222-222222222222";
const UNAVAILABLE_ID: &str = "33333333-3333-3333-3333-333333333333";
const DELAYED_ID: &str = "44444444-4444-4444-4444-444444444444";

#[tokio::test]
async fn profile_privacy_forwards_service_token_maps_visibilities_and_decodes_discovery() {
    // Given: a token-protected profile server with public/private privacy and discovery fixtures.
    let server = start_profile_grpc_server(profile_grpc_fixture(), SERVICE_TOKEN)
        .await
        .expect("profile gRPC server");
    let client = GrpcProfilePrivacyClient::connect_lazy(
        server.endpoint(),
        platform::grpc_auth::Workload::Friend,
        SERVICE_TOKEN,
        None,
    )
    .expect("profile gRPC client");

    // When: the production adapter reads privacy and batch discovery through tonic.
    let public_visibility = client
        .account_visibility(parse_uuid(PUBLIC_ID))
        .await
        .expect("public privacy");
    let private_visibility = client
        .account_visibility(parse_uuid(PRIVATE_ID))
        .await
        .expect("private privacy");
    let profiles = client
        .batch_profiles(&[parse_uuid(PRIVATE_ID), parse_uuid(PUBLIC_ID)])
        .await
        .expect("profile discovery");

    // Then: service authentication succeeds and wire values become domain values unchanged.
    assert_eq!(public_visibility, AccountVisibility::Public);
    assert_eq!(private_visibility, AccountVisibility::Private);
    assert_eq!(profiles, expected_profiles());

    server
        .shutdown_and_verify_port_released()
        .await
        .expect("server shutdown and port release");
}

#[tokio::test]
async fn profile_privacy_rejects_an_unauthenticated_service_token() {
    // Given: the real profile service interceptor expects a different service token.
    let server = start_profile_grpc_server(profile_grpc_fixture(), SERVICE_TOKEN)
        .await
        .expect("profile gRPC server");
    let client = GrpcProfilePrivacyClient::connect_lazy(
        server.endpoint(),
        platform::grpc_auth::Workload::Friend,
        "wrong-service-token",
        None,
    )
    .expect("profile gRPC client");

    // When: the adapter invokes the protected privacy RPC.
    let error = client
        .account_visibility(parse_uuid(PUBLIC_ID))
        .await
        .expect_err("incorrect service token must be rejected");

    // Then: the gRPC boundary exposes unauthenticated, not an application-level fallback.
    assert_tonic_code(&error, Code::Unauthenticated);

    server
        .shutdown_and_verify_port_released()
        .await
        .expect("server shutdown and port release");
}

#[tokio::test]
async fn profile_privacy_preserves_server_authored_unavailable_in_the_error_chain() {
    // Given: a token-protected profile service that returns an unavailable tonic status.
    let server = start_profile_grpc_server(profile_grpc_fixture(), SERVICE_TOKEN)
        .await
        .expect("profile gRPC server");
    let client = GrpcProfilePrivacyClient::connect_lazy(
        server.endpoint(),
        platform::grpc_auth::Workload::Friend,
        SERVICE_TOKEN,
        None,
    )
    .expect("profile gRPC client");

    // When: the adapter receives the downstream status.
    let unavailable = client
        .account_visibility(parse_uuid(UNAVAILABLE_ID))
        .await
        .expect_err("unavailable profile service must fail");

    // Then: callers can downcast the original tonic status from the anyhow source chain.
    assert_tonic_code(&unavailable, Code::Unavailable);

    server
        .shutdown_and_verify_port_released()
        .await
        .expect("server shutdown and port release");
}

#[tokio::test]
async fn profile_privacy_preserves_transport_unavailable_after_server_shutdown() {
    // Given: an ephemeral profile server that has released its listener before client use.
    let server = start_profile_grpc_server(profile_grpc_fixture(), SERVICE_TOKEN)
        .await
        .expect("profile gRPC server");
    let endpoint = server.endpoint();
    server
        .shutdown_and_verify_port_released()
        .await
        .expect("server shutdown and port release");
    let client = GrpcProfilePrivacyClient::connect_lazy(
        endpoint,
        platform::grpc_auth::Workload::Friend,
        SERVICE_TOKEN,
        None,
    )
    .expect("profile gRPC client");

    // When: the production adapter calls the stopped listener.
    let error = client
        .account_visibility(parse_uuid(PUBLIC_ID))
        .await
        .expect_err("stopped profile listener must be unavailable");

    // Then: the client exposes the transport-generated unavailable tonic status.
    assert_tonic_code(&error, Code::Unavailable);
}

#[tokio::test]
async fn profile_privacy_preserves_deadline_exceeded_after_a_real_delayed_rpc() {
    // Given: a profile RPC whose response arrives after the production request deadline.
    let mut fixture = profile_grpc_fixture();
    fixture.privacy_replies.insert(
        DELAYED_ID.into(),
        PrivacyReply::DelayedVisibility {
            account_visibility: "PUBLIC",
            delay: Duration::from_secs(3),
        },
    );
    let server = start_profile_grpc_server(fixture, SERVICE_TOKEN)
        .await
        .expect("profile gRPC server");
    let client = GrpcProfilePrivacyClient::connect_lazy(
        server.endpoint(),
        platform::grpc_auth::Workload::Friend,
        SERVICE_TOKEN,
        None,
    )
    .expect("profile gRPC client");

    // When: the production adapter waits for the slow RPC.
    let error = client
        .account_visibility(parse_uuid(DELAYED_ID))
        .await
        .expect_err("slow profile RPC must exceed the production deadline");

    // Then: callers can downcast the client-generated deadline status from anyhow.
    assert_tonic_code(&error, Code::DeadlineExceeded);

    server
        .shutdown_and_verify_port_released()
        .await
        .expect("server shutdown and port release");
}

fn profile_grpc_fixture() -> ProfileGrpcFixture {
    ProfileGrpcFixture {
        privacy_replies: HashMap::from([
            (PUBLIC_ID.into(), PrivacyReply::Visibility("PUBLIC")),
            (PRIVATE_ID.into(), PrivacyReply::Visibility("PRIVATE")),
            (
                UNAVAILABLE_ID.into(),
                PrivacyReply::Status(Code::Unavailable),
            ),
        ]),
        profiles: vec![
            proto::Profile {
                user_id: PRIVATE_ID.into(),
                username: "private-user".into(),
                nickname: "Private User".into(),
                bio: "private bio".into(),
                avatar_url: "https://cdn.example/private.png".into(),
                account_visibility: "PRIVATE".into(),
                lifecycle_status: "ACTIVE".into(),
                follower_count: 0,
                post_count: 0,
                following_count: 0,
            },
            proto::Profile {
                user_id: PUBLIC_ID.into(),
                username: "public-user".into(),
                nickname: "Public User".into(),
                bio: "public bio".into(),
                avatar_url: "https://cdn.example/public.png".into(),
                account_visibility: "PUBLIC".into(),
                lifecycle_status: "ACTIVE".into(),
                follower_count: 0,
                post_count: 0,
                following_count: 0,
            },
        ],
    }
}

fn expected_profiles() -> Vec<ProfileDiscoverySnapshot> {
    vec![
        ProfileDiscoverySnapshot {
            user_id: parse_uuid(PRIVATE_ID),
            username: "private-user".into(),
            nickname: "Private User".into(),
            bio: "private bio".into(),
            avatar_url: "https://cdn.example/private.png".into(),
            account_visibility: "PRIVATE".into(),
            lifecycle_status: "ACTIVE".into(),
        },
        ProfileDiscoverySnapshot {
            user_id: parse_uuid(PUBLIC_ID),
            username: "public-user".into(),
            nickname: "Public User".into(),
            bio: "public bio".into(),
            avatar_url: "https://cdn.example/public.png".into(),
            account_visibility: "PUBLIC".into(),
            lifecycle_status: "ACTIVE".into(),
        },
    ]
}

fn parse_uuid(value: &str) -> Uuid {
    value.parse().expect("contract UUID")
}

fn assert_tonic_code(error: &anyhow::Error, expected: Code) {
    let status = error
        .chain()
        .find_map(|source| source.downcast_ref::<tonic::Status>())
        .expect("tonic status must remain downcastable in the source chain");
    assert_eq!(status.code(), expected);
}
#[tokio::test]
async fn profile_privacy_retries_one_unavailable_read_and_never_other_statuses() {
    // One test, run in order: the retry tokens are per edge and per process,
    // so separate tests could drain them for each other.
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    const RECOVERS_ID: &str = "55555555-5555-5555-5555-555555555555";
    const DOWN_ID: &str = "66666666-6666-6666-6666-666666666666";
    const INTERNAL_ID: &str = "77777777-7777-7777-7777-777777777777";
    let recovers = Arc::new(AtomicUsize::new(0));
    let down = Arc::new(AtomicUsize::new(0));
    let internal = Arc::new(AtomicUsize::new(0));
    let fixture = ProfileGrpcFixture {
        privacy_replies: HashMap::from([
            (
                RECOVERS_ID.to_owned(),
                PrivacyReply::Flaky {
                    account_visibility: "PRIVATE",
                    failures: 1,
                    calls: recovers.clone(),
                },
            ),
            (
                DOWN_ID.to_owned(),
                PrivacyReply::Flaky {
                    account_visibility: "PUBLIC",
                    failures: usize::MAX,
                    calls: down.clone(),
                },
            ),
            (
                INTERNAL_ID.to_owned(),
                PrivacyReply::CountedStatus {
                    code: Code::Internal,
                    calls: internal.clone(),
                },
            ),
        ]),
        profiles: Vec::new(),
    };
    let server = start_profile_grpc_server(fixture, SERVICE_TOKEN)
        .await
        .expect("profile gRPC server");
    let metrics = platform::edge_metrics::EdgeDeadlineMetrics::default();
    let client = GrpcProfilePrivacyClient::connect_lazy_with_metrics(
        server.endpoint(),
        platform::grpc_auth::Workload::Friend,
        SERVICE_TOKEN,
        None,
        metrics.clone(),
    )
    .expect("profile gRPC client");

    // A single outage is absorbed by one retry over the real transport.
    assert_eq!(
        client
            .account_visibility(parse_uuid(RECOVERS_ID))
            .await
            .expect("second attempt answers"),
        AccountVisibility::Private
    );
    assert_eq!(recovers.load(Ordering::SeqCst), 2);

    // A persistent outage stops at the policy's two attempts.
    let error = client
        .account_visibility(parse_uuid(DOWN_ID))
        .await
        .expect_err("still unavailable");
    assert_tonic_code(&error, Code::Unavailable);
    assert_eq!(
        down.load(Ordering::SeqCst),
        platform::grpc_retry::READ_RETRY.max_attempts as usize
    );

    // Any other status is returned after exactly one call.
    let error = client
        .account_visibility(parse_uuid(INTERNAL_ID))
        .await
        .expect_err("internal");
    assert_tonic_code(&error, Code::Internal);
    assert_eq!(internal.load(Ordering::SeqCst), 1);

    assert!(metrics
        .render_prometheus()
        .contains("grpc_edge_read_retries_total{edge=\"friend->profile\"} 2"));
    server
        .shutdown_and_verify_port_released()
        .await
        .expect("server shutdown and port release");
}
