use platform::{
    grpc_auth::{VerifiedCaller, Workload},
    mutation_context::{self, MutationContext},
};
use post_service::adapters::r#in::grpc::server::proto::{self, post_service_server::PostService};
use std::time::{Duration, Instant};
use tonic::{Code, Request};
use uuid::Uuid;

fn context(actor: Uuid) -> MutationContext {
    MutationContext::new(Uuid::new_v4(), actor, Some("contract-key".into()), false)
}

async fn request<T>(context: &MutationContext, message: T, caller: Workload) -> Request<T> {
    let mut request = mutation_context::scope(Some(context.clone()), async {
        mutation_context::attach(message).unwrap()
    })
    .await;
    request.extensions_mut().insert(VerifiedCaller(caller));
    request
}

async fn create_post_with_unavailable_retry(
    grpc: &post_service::adapters::r#in::grpc::server::PostGrpcServer,
    context: &MutationContext,
    create: proto::CreatePostRequest,
) -> Result<proto::CreatePostResponse, tonic::Status> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        match grpc
            .create_post(request(context, create.clone(), Workload::Gateway).await)
            .await
        {
            Ok(response) => return Ok(response.into_inner()),
            Err(status)
                if status.code() == Code::Unavailable && tokio::time::Instant::now() < deadline =>
            {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(status) => return Err(status),
        }
    }
}

#[tokio::test]
async fn owner_rpc_requires_gateway_identity_and_binds_the_command_actor() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .unwrap();
    let grpc = crate::support::grpc_server(app.state.clone());
    let actor = Uuid::new_v4();
    let context = context(actor);
    let create = proto::CreatePostRequest {
        author_id: actor.to_string(),
        content: "text".into(),
        visibility: "PUBLIC".into(),
    };
    assert_eq!(
        grpc.create_post(Request::new(create.clone()))
            .await
            .unwrap_err()
            .code(),
        Code::Unauthenticated
    );
    assert_eq!(
        grpc.create_post(request(&context, create.clone(), Workload::Auth).await)
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    let mut forged = create.clone();
    forged.author_id = Uuid::new_v4().to_string();
    assert_eq!(
        grpc.create_post(request(&context, forged, Workload::Gateway).await)
            .await
            .unwrap_err()
            .code(),
        Code::InvalidArgument
    );
    let replay = proto::ReplayMutationRequest {
        command: Some(proto::replay_mutation_request::Command::CreatePost(create)),
    };
    assert_eq!(
        grpc.replay_mutation(request(&context, replay.clone(), Workload::Auth).await)
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    assert_eq!(
        grpc.replay_mutation(request(&context, replay, Workload::Gateway).await)
            .await
            .unwrap_err()
            .code(),
        Code::AlreadyExists
    );
    let counts: (i64, i64, i64) = sqlx::query_as("select (select count(*) from posts), (select count(*) from post_event_outbox), (select count(*) from post_mutation_receipts)")
        .fetch_one(&app.pool).await.unwrap();
    assert_eq!(
        counts,
        (0, 0, 0),
        "lookup never turns missing evidence into a write"
    );
}

#[tokio::test]
async fn database_pool_saturation_returns_sanitized_unavailable_and_safe_retry() {
    let acquire_timeout = Duration::from_millis(150);
    let app =
        crate::support::start_postgres_redis_app_with_pool_settings(false, 1, acquire_timeout)
            .await
            .unwrap();
    assert_eq!(app.pool.options().get_acquire_timeout(), acquire_timeout);
    assert_eq!(app.pool.options().get_max_connections(), 1);
    let grpc = crate::support::grpc_server(app.state.clone());
    let actor = Uuid::new_v4();
    let context = context(actor);
    let create = proto::CreatePostRequest {
        author_id: actor.to_string(),
        content: "pool recovery idempotent post".into(),
        visibility: "PUBLIC".into(),
    };

    let held_connection = app.pool.acquire().await.unwrap();
    assert_eq!(app.pool.size(), 1, "fixture must use a one-connection pool");
    assert_eq!(app.pool.num_idle(), 0, "held connection saturates the pool");

    let started = Instant::now();
    let unavailable = tokio::time::timeout(
        Duration::from_secs(3),
        grpc.create_post(request(&context, create.clone(), Workload::Gateway).await),
    )
    .await
    .expect("bounded database acquisition must return before the outer deadline")
    .expect_err("database pool saturation must not look like a successful write");
    assert_eq!(unavailable.code(), Code::Unavailable);
    assert_eq!(
        unavailable.message(),
        "Owner operation outcome unavailable",
        "database acquisition details must not leak across the service boundary"
    );
    assert!(started.elapsed() >= acquire_timeout);
    assert!(started.elapsed() < Duration::from_secs(3));

    drop(held_connection);
    let mut receipt_lookup = Request::new(proto::GetMutationReceiptStatusRequest {
        operation_id: context.operation_id.to_string(),
    });
    receipt_lookup
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Gateway));
    assert_eq!(
        grpc.get_mutation_receipt_status(receipt_lookup)
            .await
            .expect_err("a failed pool acquisition must not create a receipt")
            .code(),
        Code::NotFound
    );

    let first = create_post_with_unavailable_retry(&grpc, &context, create.clone())
        .await
        .expect("the same operation should recover after pool capacity returns");
    let replay = create_post_with_unavailable_retry(&grpc, &context, create)
        .await
        .expect("the recovered operation remains idempotent");
    assert_eq!(replay, first);

    let post_id = Uuid::parse_str(&first.post.expect("created post response").id).unwrap();
    let durable_effects: (i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from posts where id = $1), (select count(*) from post_event_outbox where aggregate_id = $1), (select count(*) from post_mutation_receipts where operation_id = $2)",
    )
    .bind(post_id)
    .bind(context.operation_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(
        durable_effects,
        (1, 1, 1),
        "retry after an unavailable result must commit one post, one outbox event, and one owner receipt"
    );
}

#[tokio::test]
async fn owner_receipt_status_is_id_only_gateway_authorized_and_keeps_expired_tombstones() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .unwrap();
    let grpc = crate::support::grpc_server(app.state.clone());
    let actor = Uuid::new_v4();
    let context = context(actor);
    let operation_id = context.operation_id;
    grpc.create_post(
        request(
            &context,
            proto::CreatePostRequest {
                author_id: actor.to_string(),
                content: "receipt metadata only".into(),
                visibility: "PUBLIC".into(),
            },
            Workload::Gateway,
        )
        .await,
    )
    .await
    .unwrap();

    let status_request = |operation_id: Uuid, caller: Option<Workload>| {
        let mut request = Request::new(proto::GetMutationReceiptStatusRequest {
            operation_id: operation_id.to_string(),
        });
        if let Some(caller) = caller {
            request.extensions_mut().insert(VerifiedCaller(caller));
        }
        request
    };
    assert_eq!(
        grpc.get_mutation_receipt_status(status_request(operation_id, None))
            .await
            .unwrap_err()
            .code(),
        Code::Unauthenticated
    );
    assert_eq!(
        grpc.get_mutation_receipt_status(status_request(operation_id, Some(Workload::Auth)))
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    let mut invalid_operation_id = Request::new(proto::GetMutationReceiptStatusRequest {
        operation_id: "not-a-uuid".into(),
    });
    invalid_operation_id
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Gateway));
    assert_eq!(
        grpc.get_mutation_receipt_status(invalid_operation_id)
            .await
            .unwrap_err()
            .code(),
        Code::InvalidArgument
    );

    let receipt = grpc
        .get_mutation_receipt_status(status_request(operation_id, Some(Workload::Gateway)))
        .await
        .unwrap()
        .into_inner();
    use proto::get_mutation_receipt_status_response::State;
    assert_eq!(receipt.operation_id, operation_id.to_string());
    assert_eq!(receipt.actor_id, actor.to_string());
    assert_eq!(receipt.scope, "post.v1.CreatePost");
    assert!(!receipt.committed_at.is_empty());
    assert!(!receipt.replay_until.is_empty());
    assert_eq!(State::try_from(receipt.state).unwrap(), State::Replayable);

    sqlx::query("update post_mutation_receipts set outcome_json = null where operation_id = $1")
        .bind(operation_id)
        .execute(&app.pool)
        .await
        .unwrap();
    let incomplete = grpc
        .get_mutation_receipt_status(status_request(operation_id, Some(Workload::Gateway)))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        State::try_from(incomplete.state).unwrap(),
        State::Incomplete
    );

    sqlx::query(
        "update post_mutation_receipts set replay_until = now() - interval '1 second' where operation_id = $1",
    )
    .bind(operation_id)
    .execute(&app.pool)
    .await
    .unwrap();
    let tombstone = grpc
        .get_mutation_receipt_status(status_request(operation_id, Some(Workload::Gateway)))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(State::try_from(tombstone.state).unwrap(), State::Expired);

    // Missing evidence is not an outcome and must never authorize a retry.
    assert_eq!(
        grpc.get_mutation_receipt_status(status_request(Uuid::new_v4(), Some(Workload::Gateway)))
            .await
            .unwrap_err()
            .code(),
        Code::NotFound
    );
}

#[tokio::test]
async fn owner_normalizes_commands_and_never_replays_an_expired_or_hidden_receipt() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .unwrap();
    let grpc = crate::support::grpc_server(app.state.clone());
    let actor = Uuid::new_v4();
    let context = context(actor);
    let mut create = proto::CreatePostRequest {
        author_id: actor.to_string(),
        content: "  text  ".into(),
        visibility: "public".into(),
    };
    let first = grpc
        .create_post(request(&context, create.clone(), Workload::Gateway).await)
        .await
        .unwrap()
        .into_inner();
    create.content = "text".into();
    create.visibility = "PUBLIC".into();
    let replay = proto::ReplayMutationRequest {
        command: Some(proto::replay_mutation_request::Command::CreatePost(
            create.clone(),
        )),
    };
    let repeated = grpc
        .replay_mutation(request(&context, replay.clone(), Workload::Gateway).await)
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        repeated.outcome,
        Some(proto::replay_mutation_response::Outcome::CreatePost(
            first.clone()
        ))
    );
    // The same operation ID is not authority to read another actor's receipt.
    // Keep the ID and idempotency key, but make both the context and command
    // internally consistent for a different actor; only receipt ownership
    // should reject this replay.
    let mut other_actor_context = context.clone();
    other_actor_context.actor_id = Uuid::new_v4();
    let other_actor_replay = proto::ReplayMutationRequest {
        command: Some(proto::replay_mutation_request::Command::CreatePost(
            proto::CreatePostRequest {
                author_id: other_actor_context.actor_id.to_string(),
                content: "text".into(),
                visibility: "PUBLIC".into(),
            },
        )),
    };
    assert_eq!(
        grpc.replay_mutation(
            request(&other_actor_context, other_actor_replay, Workload::Gateway,).await,
        )
        .await
        .expect_err("another actor must not replay this receipt")
        .code(),
        Code::AlreadyExists
    );
    let mut mismatch = create;
    mismatch.content = "changed".into();
    assert_eq!(
        grpc.create_post(request(&context, mismatch, Workload::Gateway).await)
            .await
            .unwrap_err()
            .code(),
        Code::AlreadyExists
    );
    let id = Uuid::parse_str(&first.post.unwrap().id).unwrap();
    sqlx::query("update posts set moderation_hidden_at=now() where id=$1")
        .bind(id)
        .execute(&app.pool)
        .await
        .unwrap();
    assert_eq!(
        grpc.replay_mutation(request(&context, replay.clone(), Workload::Gateway).await)
            .await
            .unwrap_err()
            .code(),
        Code::NotFound
    );
    sqlx::query("update post_mutation_receipts set replay_until=now() - interval '1 second'")
        .execute(&app.pool)
        .await
        .unwrap();
    assert_eq!(
        grpc.replay_mutation(request(&context, replay, Workload::Gateway).await)
            .await
            .unwrap_err()
            .code(),
        Code::AlreadyExists
    );
    let count: i64 = sqlx::query_scalar("select count(*) from posts")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn fixture_connection_retry_preserves_the_runtime_saturation_deadline() -> anyhow::Result<()>
{
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let postgres = test_support::postgres::start_postgres().await?;
    let url = format!(
        "postgres://postgres:postgres@{}:{}/postgres",
        postgres.get_host().await?,
        postgres.get_host_port_ipv4(5432).await?
    );
    let attempts = Arc::new(AtomicUsize::new(0));
    let callback_attempts = attempts.clone();
    let acquire_timeout = Duration::from_millis(150);
    let options = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(acquire_timeout)
        .after_connect(move |_, _| {
            let attempts = callback_attempts.clone();
            Box::pin(async move {
                if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    // Deterministically stall the first real connection initialization.
                    // SQLx must cancel it at the unchanged 150 ms acquire deadline.
                    std::future::pending::<()>().await;
                }
                Ok(())
            })
        });
    let pool = crate::support::connect_ready_fixture_pool(options, &url).await?;
    assert!(
        attempts.load(Ordering::SeqCst) >= 2,
        "first initialization must time out and retry"
    );
    assert_eq!(pool.options().get_acquire_timeout(), acquire_timeout);
    assert_eq!(pool.options().get_max_connections(), 1);
    let held = pool.acquire().await?;
    assert_eq!((pool.size(), pool.num_idle()), (1, 0));
    let started = Instant::now();
    let result = tokio::time::timeout(Duration::from_secs(3), pool.acquire()).await?;
    assert!(matches!(result, Err(sqlx::Error::PoolTimedOut)));
    assert!(started.elapsed() >= acquire_timeout && started.elapsed() < Duration::from_secs(3));
    drop(held);
    assert_eq!(
        sqlx::query_scalar::<_, i32>("select 1")
            .fetch_one(&pool)
            .await?,
        1
    );
    pool.close().await;
    Ok(())
}
