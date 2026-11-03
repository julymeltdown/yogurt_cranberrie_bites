use friend_service::adapters::r#in::grpc::server::proto;
use friend_service::adapters::r#in::grpc::server::proto::friend_service_server::FriendService;
use friend_service::application::dto::mutation_dto::MutationReceiptStateDto;
use friend_service::application::ports::FollowRepositoryPort;
use platform::grpc_auth::{VerifiedCaller, Workload};
use tonic::{Code, Request};
use uuid::Uuid;

const FOLLOWER_ID: &str = "11111111-1111-1111-1111-111111111111";
const FOLLOWEE_ID: &str = "22222222-2222-2222-2222-222222222222";
const OTHER_FOLLOWEE_ID: &str = "33333333-3333-3333-3333-333333333333";

#[tokio::test]
async fn owner_receipt_replays_follow_and_unfollow_with_gateway_only_status() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");
    let grpc = crate::support::grpc_server(app.state.clone());
    let operation_id = Uuid::new_v4();
    let actor_id = Uuid::parse_str(FOLLOWER_ID).unwrap();
    let command = follow_request(FOLLOWEE_ID);

    let first = grpc
        .follow_user(
            operation_request(command.clone(), operation_id, actor_id, "friend-follow-1").await,
        )
        .await
        .expect("first follow")
        .into_inner();
    let retry = grpc
        .follow_user(
            operation_request(command.clone(), operation_id, actor_id, "friend-follow-1").await,
        )
        .await
        .expect("same operation replay")
        .into_inner();
    assert_eq!(first.edge, retry.edge);
    assert!(first.following && retry.following);

    let recovered = grpc
        .replay_mutation(
            operation_request(
                proto::ReplayMutationRequest {
                    command: Some(proto::replay_mutation_request::Command::FollowUser(command)),
                },
                operation_id,
                actor_id,
                "friend-follow-1",
            )
            .await,
        )
        .await
        .expect("lookup-only replay")
        .into_inner();
    let Some(proto::replay_mutation_response::Outcome::FollowUser(recovered)) = recovered.outcome
    else {
        panic!("lookup-only replay should return a follow outcome");
    };
    assert_eq!(first.edge, recovered.edge);

    let active_edges: i64 = sqlx::query_scalar(
        "select count(*) from follows where follower_id=$1 and followee_id=$2 and deleted_at is null",
    )
    .bind(actor_id)
    .bind(Uuid::parse_str(FOLLOWEE_ID).unwrap())
    .fetch_one(&app.pool)
    .await
    .unwrap();
    let receipt_count: i64 =
        sqlx::query_scalar("select count(*) from friend_mutation_receipts where operation_id=$1")
            .bind(operation_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(active_edges, 1);
    assert_eq!(receipt_count, 1);

    let receipt = grpc
        .get_mutation_receipt_status(gateway_request(Request::new(
            proto::GetMutationReceiptStatusRequest {
                operation_id: operation_id.to_string(),
            },
        )))
        .await
        .expect("Gateway receipt status")
        .into_inner();
    assert_eq!(receipt.actor_id, FOLLOWER_ID);
    assert_eq!(receipt.scope, "friend.v1.FollowUser");
    assert_eq!(
        proto::get_mutation_receipt_status_response::State::try_from(receipt.state).unwrap(),
        proto::get_mutation_receipt_status_response::State::Replayable
    );

    let unfollow_operation_id = Uuid::new_v4();
    let unfollow_command = proto::UnfollowUserRequest {
        follower_id: FOLLOWER_ID.into(),
        followee_id: FOLLOWEE_ID.into(),
    };
    let unfollow = grpc
        .unfollow_user(
            operation_request(
                unfollow_command.clone(),
                unfollow_operation_id,
                actor_id,
                "friend-unfollow-1",
            )
            .await,
        )
        .await
        .expect("unfollow")
        .into_inner();
    let unfollow_retry = grpc
        .unfollow_user(
            operation_request(
                unfollow_command.clone(),
                unfollow_operation_id,
                actor_id,
                "friend-unfollow-1",
            )
            .await,
        )
        .await
        .expect("same unfollow operation replay")
        .into_inner();
    assert!(!unfollow.following && !unfollow_retry.following);
    let recovered_unfollow = grpc
        .replay_mutation(
            operation_request(
                proto::ReplayMutationRequest {
                    command: Some(proto::replay_mutation_request::Command::UnfollowUser(
                        unfollow_command,
                    )),
                },
                unfollow_operation_id,
                actor_id,
                "friend-unfollow-1",
            )
            .await,
        )
        .await
        .expect("lookup-only unfollow replay")
        .into_inner();
    assert!(matches!(
        recovered_unfollow.outcome,
        Some(proto::replay_mutation_response::Outcome::UnfollowUser(response))
            if !response.following
    ));
    let unfollow_receipt = grpc
        .get_mutation_receipt_status(gateway_request(Request::new(
            proto::GetMutationReceiptStatusRequest {
                operation_id: unfollow_operation_id.to_string(),
            },
        )))
        .await
        .expect("unfollow receipt status")
        .into_inner();
    assert_eq!(unfollow_receipt.scope, "friend.v1.UnfollowUser");

    let unauthenticated = grpc
        .get_mutation_receipt_status(Request::new(proto::GetMutationReceiptStatusRequest {
            operation_id: operation_id.to_string(),
        }))
        .await
        .expect_err("receipt metadata is not public");
    assert_eq!(unauthenticated.code(), Code::Unauthenticated);
    let mut post_caller = Request::new(proto::GetMutationReceiptStatusRequest {
        operation_id: operation_id.to_string(),
    });
    post_caller
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Post));
    let denied = grpc
        .get_mutation_receipt_status(post_caller)
        .await
        .expect_err("Post is not allowed to inspect mutation receipts");
    assert_eq!(denied.code(), Code::PermissionDenied);
    app.shutdown().await.expect("fixture shutdown");
}

#[tokio::test]
async fn private_follow_and_missing_lookup_preserve_pending_and_unknown_semantics() {
    let app = crate::support::start_postgres_app_with_privacy(
        false,
        crate::support::ProfilePrivacyResponse::Private,
    )
    .await
    .expect("app");
    let grpc = crate::support::grpc_server(app.state.clone());
    let operation_id = Uuid::new_v4();
    let actor_id = Uuid::parse_str(FOLLOWER_ID).unwrap();
    let command = follow_request(FOLLOWEE_ID);
    let first = grpc
        .follow_user(
            operation_request(command.clone(), operation_id, actor_id, "friend-private-1").await,
        )
        .await
        .expect("pending follow request")
        .into_inner();
    let retry = grpc
        .follow_user(operation_request(command, operation_id, actor_id, "friend-private-1").await)
        .await
        .expect("pending request replay")
        .into_inner();
    assert!(!first.following && !retry.following);
    assert_eq!(first.status, "PENDING");
    assert_eq!(first.request_id, retry.request_id);

    let missing_operation_id = Uuid::new_v4();
    let missing = grpc
        .replay_mutation(
            operation_request(
                proto::ReplayMutationRequest {
                    command: Some(proto::replay_mutation_request::Command::FollowUser(
                        follow_request(OTHER_FOLLOWEE_ID),
                    )),
                },
                missing_operation_id,
                actor_id,
                "friend-never-committed",
            )
            .await,
        )
        .await
        .expect_err("a lookup-only miss must not execute the follow");
    assert_eq!(missing.code(), Code::AlreadyExists);
    assert_eq!(
        missing
            .metadata()
            .get(platform::mutation_context::ERROR_CODE)
            .unwrap(),
        "unknown"
    );
    let no_edge: i64 = sqlx::query_scalar(
        "select count(*) from follows where follower_id=$1 and followee_id=$2 and deleted_at is null",
    )
    .bind(actor_id)
    .bind(Uuid::parse_str(OTHER_FOLLOWEE_ID).unwrap())
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(no_edge, 0);

    let status = app
        .state
        .relationship_repository
        .mutation_receipt_status(operation_id)
        .await
        .unwrap()
        .expect("pending follow receipt");
    assert_eq!(status.state, MutationReceiptStateDto::Replayable);
    app.shutdown().await.expect("fixture shutdown");
}

#[tokio::test]
async fn failed_receipt_completion_rolls_back_follow_and_same_key_cannot_change_command() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");
    let grpc = crate::support::grpc_server(app.state.clone());
    let operation_id = Uuid::new_v4();
    let actor_id = Uuid::parse_str(FOLLOWER_ID).unwrap();
    sqlx::query(
        "alter table friend_mutation_receipts add constraint test_reject_receipt_outcome check (outcome_json is null)",
    )
    .execute(&app.pool)
    .await
    .unwrap();

    let error = grpc
        .follow_user(
            operation_request(
                follow_request(FOLLOWEE_ID),
                operation_id,
                actor_id,
                "friend-rollback-1",
            )
            .await,
        )
        .await
        .expect_err("receipt failure must fail the transaction");
    assert_eq!(error.code(), Code::Unavailable);
    sqlx::query("alter table friend_mutation_receipts drop constraint test_reject_receipt_outcome")
        .execute(&app.pool)
        .await
        .unwrap();

    let effects: (i64, i64) = sqlx::query_as(
        "select (select count(*) from follows where follower_id=$1 and followee_id=$2 and deleted_at is null), (select count(*) from friend_mutation_receipts where operation_id=$3)",
    )
    .bind(actor_id)
    .bind(Uuid::parse_str(FOLLOWEE_ID).unwrap())
    .bind(operation_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(effects, (0, 0));

    grpc.follow_user(
        operation_request(
            follow_request(FOLLOWEE_ID),
            operation_id,
            actor_id,
            "friend-rollback-1",
        )
        .await,
    )
    .await
    .expect("safe retry after proven rollback");
    let conflict = grpc
        .follow_user(
            operation_request(
                follow_request(OTHER_FOLLOWEE_ID),
                operation_id,
                actor_id,
                "friend-rollback-1",
            )
            .await,
        )
        .await
        .expect_err("same identity with a different command must conflict");
    assert_eq!(conflict.code(), Code::AlreadyExists);
    assert_eq!(
        conflict
            .metadata()
            .get(platform::mutation_context::ERROR_CODE)
            .unwrap(),
        "conflict"
    );
    app.shutdown().await.expect("fixture shutdown");
}

/// Forces the interleaving: one contender has inserted its receipt and is
/// parked on the edge insert, and the other is observed waiting on that
/// uncommitted receipt before the insert is released. Both return the
/// winner's outcome, with one edge and one receipt.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_duplicate_follow_waits_for_the_winner_and_commits_one_effect() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");
    let operation_id = Uuid::new_v4();
    let actor_id = Uuid::parse_str(FOLLOWER_ID).unwrap();
    let first_request = operation_request(
        follow_request(FOLLOWEE_ID),
        operation_id,
        actor_id,
        "friend-race-1",
    )
    .await;
    let second_request = operation_request(
        follow_request(FOLLOWEE_ID),
        operation_id,
        actor_id,
        "friend-race-1",
    )
    .await;
    warm_pool(&app.pool, 4).await;

    let mut gate = app.pool.begin().await.expect("controller transaction");
    // SHARE mode admits the relationship-policy reads that precede the claim
    // and blocks only the edge insert that follows it.
    sqlx::query("lock table follows in share mode")
        .execute(&mut *gate)
        .await
        .expect("hold the edge insert");
    let contender = |request| {
        let grpc = crate::support::grpc_server(app.state.clone());
        tokio::spawn(async move { grpc.follow_user(request).await })
    };
    let first = contender(first_request);
    let second = contender(second_request);
    wait_for_parked_contenders(&mut gate, "insert into follows").await;
    gate.commit().await.expect("release the winner");

    let first = first.await.unwrap().expect("first contender").into_inner();
    let second = second
        .await
        .unwrap()
        .expect("second contender")
        .into_inner();
    assert_eq!(first.edge, second.edge);
    assert!(first.following && second.following);

    let effects: (i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from follows where follower_id=$1 and followee_id=$2),
                (select count(*) from friend_mutation_receipts where actor_id=$1),
                (select count(outcome_json) from friend_mutation_receipts where operation_id=$3)",
    )
    .bind(actor_id)
    .bind(Uuid::parse_str(FOLLOWEE_ID).unwrap())
    .bind(operation_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(effects, (1, 1, 1));
    app.shutdown().await.expect("fixture shutdown");
}

/// Same forced interleaving for UnfollowUser: the first claimant parks on the
/// soft-delete update and the duplicate waits on its uncommitted receipt.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_duplicate_unfollow_waits_for_the_winner_and_commits_one_effect() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");
    let actor_id = Uuid::parse_str(FOLLOWER_ID).unwrap();
    let followee_id = Uuid::parse_str(FOLLOWEE_ID).unwrap();
    sqlx::query(
        "insert into follows (follower_id, followee_id, created_at) values ($1, $2, now())",
    )
    .bind(actor_id)
    .bind(followee_id)
    .execute(&app.pool)
    .await
    .expect("existing edge");
    let operation_id = Uuid::new_v4();
    let command = proto::UnfollowUserRequest {
        follower_id: FOLLOWER_ID.into(),
        followee_id: FOLLOWEE_ID.into(),
    };
    let first_request = operation_request(
        command.clone(),
        operation_id,
        actor_id,
        "friend-unfollow-race",
    )
    .await;
    let second_request =
        operation_request(command, operation_id, actor_id, "friend-unfollow-race").await;
    warm_pool(&app.pool, 4).await;

    let mut gate = app.pool.begin().await.expect("controller transaction");
    sqlx::query("lock table follows in share mode")
        .execute(&mut *gate)
        .await
        .expect("hold the soft delete");
    let contender = |request| {
        let grpc = crate::support::grpc_server(app.state.clone());
        tokio::spawn(async move { grpc.unfollow_user(request).await })
    };
    let first = contender(first_request);
    let second = contender(second_request);
    wait_for_parked_contenders(&mut gate, "update follows").await;
    gate.commit().await.expect("release the winner");

    let first = first.await.unwrap().expect("first contender").into_inner();
    let second = second
        .await
        .unwrap()
        .expect("second contender")
        .into_inner();
    assert_eq!(first, second);
    assert!(!first.following);

    let effects: (i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from follows
                  where follower_id=$1 and followee_id=$2 and deleted_at is not null),
                (select count(*) from friend_mutation_receipts where actor_id=$1),
                (select count(outcome_json) from friend_mutation_receipts where operation_id=$3)",
    )
    .bind(actor_id)
    .bind(followee_id)
    .bind(operation_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(effects, (1, 1, 1));
    app.shutdown().await.expect("fixture shutdown");
}

/// UnfollowUser completes its receipt after the soft delete. A completion
/// failure must leave the edge active and no receipt; the same operation then
/// commits once.
#[tokio::test]
async fn failed_unfollow_receipt_completion_keeps_the_edge_and_allows_safe_retry() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");
    let grpc = crate::support::grpc_server(app.state.clone());
    let actor_id = Uuid::parse_str(FOLLOWER_ID).unwrap();
    let followee_id = Uuid::parse_str(FOLLOWEE_ID).unwrap();
    sqlx::query(
        "insert into follows (follower_id, followee_id, created_at) values ($1, $2, now())",
    )
    .bind(actor_id)
    .bind(followee_id)
    .execute(&app.pool)
    .await
    .expect("existing edge");
    let operation_id = Uuid::new_v4();
    let command = proto::UnfollowUserRequest {
        follower_id: FOLLOWER_ID.into(),
        followee_id: FOLLOWEE_ID.into(),
    };
    let state = || async {
        sqlx::query_as::<_, (i64, i64)>(
            "select (select count(*) from follows
                      where follower_id=$1 and followee_id=$2 and deleted_at is null),
                    (select count(*) from friend_mutation_receipts where operation_id=$3)",
        )
        .bind(actor_id)
        .bind(followee_id)
        .bind(operation_id)
        .fetch_one(&app.pool)
        .await
        .unwrap()
    };
    sqlx::query(
        "alter table friend_mutation_receipts add constraint test_reject_unfollow_outcome check (outcome_json is null) not valid",
    )
    .execute(&app.pool)
    .await
    .unwrap();
    let error = grpc
        .unfollow_user(
            operation_request(
                command.clone(),
                operation_id,
                actor_id,
                "friend-unfollow-fault",
            )
            .await,
        )
        .await
        .expect_err("receipt failure must fail the transaction");
    assert_eq!(error.code(), Code::Unavailable);
    assert_eq!(state().await, (1, 0));
    sqlx::query(
        "alter table friend_mutation_receipts drop constraint test_reject_unfollow_outcome",
    )
    .execute(&app.pool)
    .await
    .unwrap();
    let retried = grpc
        .unfollow_user(
            operation_request(command, operation_id, actor_id, "friend-unfollow-fault").await,
        )
        .await
        .expect("safe retry after proven rollback")
        .into_inner();
    assert!(!retried.following);
    assert_eq!(state().await, (0, 1));
    app.shutdown().await.expect("fixture shutdown");
}

/// Holds pool connections open together once, so connection setup does not
/// separate the contenders' arrival at their receipt inserts.
async fn warm_pool(pool: &sqlx::PgPool, connections: usize) {
    let mut held = Vec::with_capacity(connections);
    for _ in 0..connections {
        held.push(pool.acquire().await.expect("warm connection"));
    }
}

/// Waits until exactly one contender is parked on the effect statement after
/// inserting its receipt and the other is waiting on that uncommitted receipt.
/// The parked transaction has a 500 ms lock timeout, which bounds how long the
/// second contender may take to arrive.
async fn wait_for_parked_contenders(connection: &mut sqlx::PgConnection, effect_statement: &str) {
    let receipt_insert = "insert into friend_mutation_receipts";
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        // Backend activity is otherwise snapshotted once per transaction.
        sqlx::query("select pg_stat_clear_snapshot()")
            .execute(&mut *connection)
            .await
            .expect("fresh activity snapshot");
        let waiting: Vec<String> = sqlx::query_scalar(
            "select regexp_replace(query, '^\\s+', '') from pg_stat_activity
             where datname = current_database() and wait_event_type = 'Lock'
               and pid <> pg_backend_pid()",
        )
        .fetch_all(&mut *connection)
        .await
        .expect("lock waiters");
        let receipts = waiting
            .iter()
            .filter(|query| query.starts_with(receipt_insert))
            .count();
        let effects = waiting
            .iter()
            .filter(|query| query.starts_with(effect_statement))
            .count();
        if waiting.len() == 2 && receipts == 1 && effects == 1 {
            return;
        }
        assert!(
            waiting.len() == receipts + effects && receipts <= 1 && effects <= 1,
            "unexpected lock waiters: {waiting:#?}"
        );
        assert!(
            std::time::Instant::now() < deadline,
            "contenders never parked: {waiting:#?}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }
}

fn follow_request(followee_id: &str) -> proto::FollowUserRequest {
    proto::FollowUserRequest {
        follower_id: FOLLOWER_ID.into(),
        followee_id: followee_id.into(),
    }
}

fn gateway_request<T>(mut request: Request<T>) -> Request<T> {
    request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Gateway));
    request
}

async fn operation_request<T>(
    message: T,
    operation_id: Uuid,
    actor_id: Uuid,
    idempotency_key: &str,
) -> Request<T> {
    let context = platform::mutation_context::MutationContext::new(
        operation_id,
        actor_id,
        Some(idempotency_key.to_owned()),
        false,
    );
    platform::mutation_context::scope(Some(context), async {
        gateway_request(platform::mutation_context::attach(message).unwrap())
    })
    .await
}
