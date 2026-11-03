use crate::support::ProfilePrivacyResponse;
use friend_service::adapters::r#in::grpc::server::{
    proto, proto::friend_service_server::FriendService,
};
use platform::grpc_auth::{VerifiedCaller, Workload};
use tonic::{Code, Request};

const VIEWER_ID: &str = "11111111-1111-1111-1111-111111111111";
const TARGET_ID: &str = "22222222-2222-2222-2222-222222222222";
const OTHER_ID: &str = "33333333-3333-3333-3333-333333333333";

#[tokio::test]
async fn private_follow_requests_complete_reject_cancel_and_accept_lifecycles() {
    // Given: a real Friend state backed by Postgres and private profile policy.
    let app =
        crate::support::start_postgres_app_with_privacy(false, ProfilePrivacyResponse::Private)
            .await
            .expect("private friend app");
    let grpc = crate::support::grpc_server(app.state.clone());

    // When: private follow requests are listed and completed through every supported decision.
    let rejected_request_id = pending_follow(&grpc).await;
    let incoming = grpc
        .list_incoming_follow_requests(Request::new(proto::ListFollowRequestsRequest {
            user_id: TARGET_ID.into(),
            cursor: String::new(),
            limit: 10,
        }))
        .await
        .expect("incoming requests")
        .into_inner();
    let outgoing = grpc
        .list_outgoing_follow_requests(Request::new(proto::ListFollowRequestsRequest {
            user_id: VIEWER_ID.into(),
            cursor: String::new(),
            limit: 10,
        }))
        .await
        .expect("outgoing requests")
        .into_inner();
    let rejected = grpc
        .reject_follow_request(Request::new(proto::FollowRequestActionRequest {
            request_id: rejected_request_id,
            actor_id: TARGET_ID.into(),
        }))
        .await
        .expect("reject request")
        .into_inner();
    let canceled = grpc
        .cancel_follow_request(Request::new(proto::FollowRequestActionRequest {
            request_id: pending_follow(&grpc).await,
            actor_id: VIEWER_ID.into(),
        }))
        .await
        .expect("cancel request")
        .into_inner();
    let accepted = grpc
        .accept_follow_request(Request::new(proto::FollowRequestActionRequest {
            request_id: pending_follow(&grpc).await,
            actor_id: TARGET_ID.into(),
        }))
        .await
        .expect("accept request")
        .into_inner();

    // Then: lists preserve both directions and each decision exposes its stable state.
    assert_eq!(incoming.requests.len(), 1);
    assert_eq!(incoming.requests[0].follower_id, VIEWER_ID);
    assert_eq!(outgoing.requests.len(), 1);
    assert_eq!(outgoing.requests[0].followee_id, TARGET_ID);
    assert_eq!(rejected.status, "REJECTED");
    assert_eq!(
        rejected.request.expect("rejected request").status,
        "REJECTED"
    );
    assert_eq!(canceled.status, "CANCELLED");
    assert_eq!(
        canceled.request.expect("canceled request").status,
        "CANCELLED"
    );
    assert_eq!(accepted.status, "ACTIVE");
    assert!(accepted.following);
    assert_eq!(accepted.edge.expect("accepted edge").followee_id, TARGET_ID);
    app.shutdown().await.expect("fixture shutdown");
}

#[tokio::test]
async fn relationship_controls_preserve_block_mute_follow_and_batch_policy() {
    // Given: a real public-profile Friend state with an active relationship.
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("public friend app");
    let grpc = crate::support::grpc_server(app.state.clone());
    grpc.follow_user(gateway_request(Request::new(follow_request())))
        .await
        .expect("initial follow");

    // When: the relationship is blocked, inspected, unblocked, followed, muted and unmuted.
    let blocked = grpc
        .block_user(Request::new(relationship_request()))
        .await
        .expect("block")
        .into_inner();
    let blocked_lookup = grpc
        .get_relationship(Request::new(relationship_request()))
        .await
        .expect("blocked relationship")
        .into_inner();
    let blocked_batch = grpc
        .batch_get_relationships(Request::new(proto::BatchRelationshipRequest {
            viewer_id: VIEWER_ID.into(),
            target_ids: vec![TARGET_ID.into(), OTHER_ID.into()],
        }))
        .await
        .expect("batch relationship")
        .into_inner();
    let unblocked = grpc
        .unblock_user(Request::new(relationship_request()))
        .await
        .expect("unblock")
        .into_inner();
    grpc.follow_user(gateway_request(Request::new(follow_request())))
        .await
        .expect("follow after unblock");
    let followers = grpc
        .list_followers(Request::new(proto::ListFollowersRequest {
            user_id: TARGET_ID.into(),
            cursor: String::new(),
            limit: 10,
        }))
        .await
        .expect("followers")
        .into_inner();
    let muted = grpc
        .mute_user(Request::new(relationship_request()))
        .await
        .expect("mute")
        .into_inner();
    let unmuted = grpc
        .unmute_user(Request::new(relationship_request()))
        .await
        .expect("unmute")
        .into_inner();

    // Then: block removes the edge, batch returns an explicit empty policy for a target with no edges, and mute preserves refollow.
    assert!(blocked.blocked_by_viewer);
    assert!(!blocked.following);
    assert!(blocked_lookup.blocked_by_viewer);
    assert_eq!(blocked_batch.policies.len(), 2);
    assert!(
        blocked_batch.policies[0]
            .policy
            .as_ref()
            .expect("target policy")
            .blocked_by_viewer
    );
    assert!(
        !blocked_batch.policies[1]
            .policy
            .as_ref()
            .expect("explicit policy for unrelated target")
            .blocked_by_viewer
    );
    assert!(!unblocked.blocked_by_viewer);
    assert!(muted.muted_by_viewer);
    assert!(muted.following);
    assert_eq!(followers.total_count, 1);
    assert_eq!(followers.edges[0].follower_id, VIEWER_ID);
    assert!(!unmuted.muted_by_viewer);
    assert!(unmuted.following);
    app.shutdown().await.expect("fixture shutdown");
}

#[tokio::test]
async fn accepting_pending_request_denies_when_relationship_became_blocked() {
    // Given: a pending private follow request whose relationship is blocked before acceptance.
    let app =
        crate::support::start_postgres_app_with_privacy(false, ProfilePrivacyResponse::Private)
            .await
            .expect("private friend app");
    let grpc = crate::support::grpc_server(app.state.clone());
    let request_id = pending_follow(&grpc).await;
    sqlx::query(
        "insert into user_blocks (blocker_id, blocked_id, created_at) values ($1, $2, now())",
    )
    .bind(TARGET_ID.parse::<uuid::Uuid>().expect("target UUID"))
    .bind(VIEWER_ID.parse::<uuid::Uuid>().expect("viewer UUID"))
    .execute(&app.pool)
    .await
    .expect("concurrent block state");

    // When: the target accepts through the real repository transaction.
    let error = grpc
        .accept_follow_request(Request::new(proto::FollowRequestActionRequest {
            request_id,
            actor_id: TARGET_ID.into(),
        }))
        .await
        .expect_err("blocked pending request must be denied");

    // Then: the public gRPC contract reports permission denial with stable safe text.
    assert_eq!(error.code(), Code::PermissionDenied);
    assert_eq!(error.message(), "Blocked relationship");
    app.shutdown().await.expect("fixture shutdown");
}

/// T01.1 ownership: each follow-request decision binds the actor to the side
/// that may take it in the same UPDATE (accept/reject: followee; cancel:
/// follower). A stranger or the wrong side sees NOT_FOUND and the request
/// stays pending.
#[tokio::test]
async fn follow_request_decisions_are_scoped_to_the_deciding_side() {
    let app =
        crate::support::start_postgres_app_with_privacy(false, ProfilePrivacyResponse::Private)
            .await
            .expect("private friend app");
    let grpc = crate::support::grpc_server(app.state.clone());
    let request_id = pending_follow(&grpc).await;
    let action = |actor: &str| proto::FollowRequestActionRequest {
        request_id: request_id.clone(),
        actor_id: actor.into(),
    };

    for actor in [OTHER_ID, VIEWER_ID] {
        let accept = grpc
            .accept_follow_request(Request::new(action(actor)))
            .await
            .expect_err("only the followee may accept");
        assert_eq!(accept.code(), Code::NotFound, "accept by {actor}");
        let reject = grpc
            .reject_follow_request(Request::new(action(actor)))
            .await
            .expect_err("only the followee may reject");
        assert_eq!(reject.code(), Code::NotFound, "reject by {actor}");
    }
    for actor in [OTHER_ID, TARGET_ID] {
        let cancel = grpc
            .cancel_follow_request(Request::new(action(actor)))
            .await
            .expect_err("only the follower may cancel");
        assert_eq!(cancel.code(), Code::NotFound, "cancel by {actor}");
    }
    let incoming = grpc
        .list_incoming_follow_requests(Request::new(proto::ListFollowRequestsRequest {
            user_id: OTHER_ID.into(),
            cursor: String::new(),
            limit: 100,
        }))
        .await
        .expect("stranger incoming list")
        .into_inner();
    let outgoing = grpc
        .list_outgoing_follow_requests(Request::new(proto::ListFollowRequestsRequest {
            user_id: OTHER_ID.into(),
            cursor: String::new(),
            limit: 100,
        }))
        .await
        .expect("stranger outgoing list")
        .into_inner();
    assert!(incoming.requests.is_empty() && outgoing.requests.is_empty());

    let (status, edges): (String, i64) = sqlx::query_as(
        "select status, (select count(*) from follows where deleted_at is null) from follow_requests where id = $1",
    )
    .bind(request_id.parse::<uuid::Uuid>().expect("request UUID"))
    .fetch_one(&app.pool)
    .await
    .expect("request row");
    assert_eq!(status, "PENDING");
    assert_eq!(edges, 0);

    // The deciding side still can.
    let accepted = grpc
        .accept_follow_request(Request::new(action(TARGET_ID)))
        .await
        .expect("followee accepts")
        .into_inner();
    assert_eq!(accepted.status, "ACTIVE");
    app.shutdown().await.expect("fixture shutdown");
}

async fn pending_follow(
    grpc: &friend_service::adapters::r#in::grpc::server::FriendGrpcServer,
) -> String {
    let response = grpc
        .follow_user(gateway_request(Request::new(follow_request())))
        .await
        .expect("pending follow")
        .into_inner();
    assert_eq!(response.status, "PENDING");
    assert!(!response.following);
    response.request_id
}

fn follow_request() -> proto::FollowUserRequest {
    proto::FollowUserRequest {
        follower_id: VIEWER_ID.into(),
        followee_id: TARGET_ID.into(),
    }
}

fn gateway_request<T>(mut request: Request<T>) -> Request<T> {
    request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Gateway));
    request
}

fn relationship_request() -> proto::RelationshipRequest {
    proto::RelationshipRequest {
        viewer_id: VIEWER_ID.into(),
        target_id: TARGET_ID.into(),
    }
}
