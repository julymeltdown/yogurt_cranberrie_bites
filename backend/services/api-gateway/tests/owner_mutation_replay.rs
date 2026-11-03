//! Real gateway HTTP -> authenticated TCP gRPC -> post PostgreSQL/outbox.
//! Account/profile peers are fixture adapters; this is not a full-stack journey.
#[path = "../../friend-service/tests/support/mod.rs"]
mod friend_owner_support;
#[path = "../../post-service/tests/support/mod.rs"]
mod owner_support;
#[path = "support/mod.rs"]
mod support;

use api_gateway::adapters::{
    out::{friend_client::GrpcFriendClient, post_client::GrpcPostClient},
    r#in::http,
};
use api_gateway::application::service::{
    friend_gateway_service::FriendGatewayService, post_gateway_service::PostGatewayService,
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
    Router,
};
use friend_service::adapters::r#in::grpc::server::{proto as friend_proto, FriendGrpcServer};
use http_body_util::BodyExt;
use platform::grpc_auth::{CallerRegistry, RequireRegisteredCallerInterceptor, Workload};
use post_service::adapters::r#in::grpc::server::{proto, PostGrpcServer};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio_stream::wrappers::TcpListenerStream;
use tower::ServiceExt;
use uuid::Uuid;

const AUTHOR: &str = "11111111-1111-1111-1111-111111111111";
const VIEWER: &str = "22222222-2222-2222-2222-222222222222";

fn request(method: &str, path: &str, key: &str, body: &str, actor: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header(
            "authorization",
            format!("Bearer {}", support::bearer_token(actor, &["ROLE_USER"])),
        )
        .header("content-type", "application/json")
        .header("idempotency-key", key)
        // Public headers must not choose the owner operation identity.
        .header("x-owner-operation-id", Uuid::nil().to_string())
        .body(Body::from(body.to_owned()))
        .unwrap()
}

async fn body(response: Response) -> Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

async fn command(
    router: &Router,
    method: &str,
    path: &str,
    key: &str,
    input: &str,
    actor: &str,
) -> Value {
    let response = router
        .clone()
        .oneshot(request(method, path, key, input, actor))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    body(response).await
}

#[tokio::test]
async fn gateway_recovers_committed_owner_outcome_without_redispatch_and_rechecks_privacy(
) -> anyhow::Result<()> {
    let owner = owner_support::start_postgres_redis_app(false).await?;
    let gateway = support::start_gateway_app(false).await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}", listener.local_addr()?);
    let registry = CallerRegistry::new().register(Workload::Gateway, "test-gateway-credential")?;
    let service = proto::post_service_server::PostServiceServer::with_interceptor(
        PostGrpcServer::new(owner.state.clone()),
        RequireRegisteredCallerInterceptor::new(registry)?,
    );
    let (shutdown, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(
        tonic::transport::Server::builder()
            .add_service(service)
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = stopped.await;
            }),
    );
    let client =
        GrpcPostClient::connect(endpoint, Workload::Gateway, "test-gateway-credential", None)
            .await?;
    let mut state = (*gateway.state).clone();
    state.post_service =
        PostGatewayService::new(Arc::new(client), state.search_cursor_codec.clone());
    let router = http::server::with_transport_layers(
        http::router(Arc::new(state)),
        &gateway.config,
        http::client_identity::TrustedProxies::default(),
    );

    // Fault injection: owner COMMIT succeeds, but gateway completion cannot commit.
    sqlx::query("alter table gateway_request_journal add constraint reject_completion check (state <> 'COMPLETED')")
        .execute(&gateway.pool).await?;
    let input = r#"{"content":"atomic owner write","visibility":"PUBLIC"}"#;
    let first = router
        .clone()
        .oneshot(request("POST", "/api/posts", "create-1", input, AUTHOR))
        .await?;
    assert_eq!(first.status(), StatusCode::OK);
    let operation_id = first.headers()["x-operation-id"].to_str()?.to_owned();
    drop(first); // Deliberately discard the reply as a disconnected client would.
    let journal_state: String =
        sqlx::query_scalar("select state from gateway_request_journal where id=$1")
            .bind(Uuid::parse_str(&operation_id)?)
            .fetch_one(&gateway.pool)
            .await?;
    assert_eq!(journal_state, "IN_PROGRESS");
    sqlx::query("alter table gateway_request_journal drop constraint reject_completion")
        .execute(&gateway.pool)
        .await?;

    // Admin status enriches the unresolved Gateway intent with read-only owner
    // evidence over the real authenticated PostService gRPC client.
    let admin_token =
        support::bearer_token("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", &["ROLE_ADMIN"]);
    let owner_status = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/operations/{operation_id}"))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(owner_status.status(), StatusCode::OK);
    let status = body(owner_status).await;
    assert_eq!(status["state"], "IN_PROGRESS");
    assert_eq!(status["outcome"], "COMMITTED");
    assert_eq!(status["ownerReceipt"]["lookupStatus"], "FOUND");
    assert_eq!(status["ownerReceipt"]["receiptState"], "REPLAYABLE");
    assert_eq!(status["ownerReceipt"]["scope"], "post.v1.CreatePost");
    assert!(status.get("actorId").is_none());

    let recovered = router
        .clone()
        .oneshot(request("POST", "/api/posts", "create-1", input, AUTHOR))
        .await?;
    assert_eq!(recovered.status(), StatusCode::OK);
    assert_eq!(recovered.headers()["x-operation-id"], operation_id);
    assert_eq!(recovered.headers()["idempotency-replayed"], "true");
    assert_eq!(recovered.headers()["cache-control"], "no-store");
    let created = body(recovered).await;
    let post_id = created["id"].as_str().unwrap();
    let counts: (i64, i64, i64) = sqlx::query_as("select (select count(*) from posts), (select count(*) from post_event_outbox), (select count(*) from post_mutation_receipts)")
        .fetch_one(&owner.pool).await?;
    assert_eq!(counts, (1, 1, 1));
    let state: String = sqlx::query_scalar("select state from gateway_request_journal where id=$1")
        .bind(Uuid::parse_str(&operation_id)?)
        .fetch_one(&gateway.pool)
        .await?;
    assert_eq!(state, "COMPLETED");
    let reconciliations: i64 = sqlx::query_scalar(
        "select count(*) from admin_audit_events where action='OPERATION_OWNER_RECONCILED'",
    )
    .fetch_one(&gateway.pool)
    .await?;
    assert_eq!(reconciliations, 1);
    // An older Gateway can have recorded a 4xx after the owner committed (for
    // example, a visibility check on the response path). Positive owner evidence
    // must replace that classification without rewriting the prior audit.
    sqlx::query("update gateway_request_journal set response_status=400 where id=$1")
        .bind(Uuid::parse_str(&operation_id)?)
        .execute(&gateway.pool)
        .await?;
    assert_eq!(
        command(&router, "POST", "/api/posts", "create-1", input, AUTHOR).await,
        created
    );
    let reconciled_status: i32 =
        sqlx::query_scalar("select response_status from gateway_request_journal where id=$1")
            .bind(Uuid::parse_str(&operation_id)?)
            .fetch_one(&gateway.pool)
            .await?;
    assert_eq!(
        reconciled_status, 200,
        "owner commit evidence supersedes a legacy 4xx reply"
    );
    let reconciliations: i64 = sqlx::query_scalar(
        "select count(*) from admin_audit_events where action='OPERATION_OWNER_RECONCILED'",
    )
    .fetch_one(&gateway.pool)
    .await?;
    assert_eq!(
        reconciliations, 2,
        "the first reconciliation audit remains intact"
    );

    let changed = router
        .clone()
        .oneshot(request(
            "POST",
            "/api/posts",
            "create-1",
            r#"{"content":"changed","visibility":"PUBLIC"}"#,
            AUTHOR,
        ))
        .await?;
    assert_eq!(changed.status(), StatusCode::CONFLICT);
    let comments_path = format!("/api/posts/{post_id}/comments");
    let likes_path = format!("/api/posts/{post_id}/likes");
    let comment = command(
        &router,
        "POST",
        &comments_path,
        "comment-1",
        r#"{"content":"one comment"}"#,
        VIEWER,
    )
    .await;
    assert_eq!(
        command(
            &router,
            "POST",
            &comments_path,
            "comment-1",
            r#"{"content":"one comment"}"#,
            VIEWER
        )
        .await,
        comment
    );
    let like = command(&router, "POST", &likes_path, "like-1", "", VIEWER).await;
    let unlike = command(&router, "DELETE", &likes_path, "unlike-1", "", VIEWER).await;
    assert_eq!(
        command(&router, "POST", &likes_path, "like-1", "", VIEWER).await,
        like
    );
    assert_eq!(
        command(&router, "DELETE", &likes_path, "unlike-1", "", VIEWER).await,
        unlike
    );
    let counters: (i64, i64) =
        sqlx::query_as("select comment_count, like_count from posts where id=$1")
            .bind(Uuid::parse_str(post_id)?)
            .fetch_one(&owner.pool)
            .await?;
    assert_eq!(
        counters,
        (1, 0),
        "replayed LIKE must not undo a later UNLIKE"
    );

    let restriction = Uuid::new_v4();
    sqlx::query("insert into moderation_reports (id,reporter_id,target_type,target_id,category,reason) values ($1,$2,'USER',$3,'OTHER','replay restriction test')")
        .bind(restriction).bind(Uuid::parse_str(AUTHOR)?).bind(Uuid::parse_str(VIEWER)?).execute(&gateway.pool).await?;
    sqlx::query("insert into user_restrictions (user_id,report_id,reason,restricted_by) values ($1,$2,'replay restriction test',$3)")
        .bind(Uuid::parse_str(VIEWER)?).bind(restriction).bind(Uuid::parse_str(AUTHOR)?).execute(&gateway.pool).await?;
    let restricted = router
        .clone()
        .oneshot(request(
            "POST",
            &comments_path,
            "comment-1",
            r#"{"content":"one comment"}"#,
            VIEWER,
        ))
        .await?;
    assert_eq!(
        restricted.status(),
        StatusCode::FORBIDDEN,
        "replay reapplies current user restrictions"
    );
    sqlx::query("update user_restrictions set lifted_at=now() where user_id=$1")
        .bind(Uuid::parse_str(VIEWER)?)
        .execute(&gateway.pool)
        .await?;

    sqlx::query("update posts set visibility='PRIVATE' where id=$1")
        .bind(Uuid::parse_str(post_id)?)
        .execute(&owner.pool)
        .await?;
    for (method, path, key, content) in [
        (
            "POST",
            comments_path.as_str(),
            "comment-1",
            r#"{"content":"one comment"}"#,
        ),
        ("POST", likes_path.as_str(), "like-1", ""),
    ] {
        let response = router
            .clone()
            .oneshot(request(method, path, key, content, VIEWER))
            .await?;
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "cached gateway body must not bypass owner privacy"
        );
    }
    let post_path = format!("/api/posts/{post_id}");
    let deleted = command(&router, "DELETE", &post_path, "delete-1", "", AUTHOR).await;
    assert_eq!(
        command(&router, "DELETE", &post_path, "delete-1", "", AUTHOR).await,
        deleted
    );
    let hidden = router
        .clone()
        .oneshot(request("POST", "/api/posts", "create-1", input, AUTHOR))
        .await?;
    assert_eq!(
        hidden.status(),
        StatusCode::NOT_FOUND,
        "deleted content is not replayable"
    );
    let effects: (i64, i64, i64) = sqlx::query_as("select (select count(*) from comments), (select count(*) from post_event_outbox), (select count(*) from post_mutation_receipts)")
        .fetch_one(&owner.pool).await?;
    assert_eq!(effects, (1, 5, 5));

    // The gateway has an intent, but the owner has no receipt. A retry is lookup
    // only and cannot create the apparently missing post.
    let missing_id = Uuid::new_v4();
    let fingerprint_context = json!([AUTHOR, "POST", "/api/posts", "application/json"]).to_string();
    sqlx::query("insert into gateway_request_journal (id,actor_id,action,resource_path,idempotency_key,request_fingerprint,state,outcome,outcome_evidence) values ($1,$2,'POST /api/posts','/api/posts','never-committed',encode(sha256(convert_to($3,'UTF8') || $4::bytea),'hex'),'UNKNOWN','UNKNOWN','LEGACY_UNRESOLVED')")
        .bind(missing_id).bind(AUTHOR).bind(fingerprint_context).bind(input.as_bytes()).execute(&gateway.pool).await?;
    let missing = router
        .clone()
        .oneshot(request(
            "POST",
            "/api/posts",
            "never-committed",
            input,
            AUTHOR,
        ))
        .await?;
    assert_eq!(missing.status(), StatusCode::CONFLICT);
    assert!(body(missing).await["message"]
        .as_str()
        .unwrap()
        .contains("outcome is unknown"));
    let total: i64 = sqlx::query_scalar("select count(*) from posts")
        .fetch_one(&owner.pool)
        .await?;
    assert_eq!(total, 1);
    let state: String = sqlx::query_scalar("select state from gateway_request_journal where id=$1")
        .bind(missing_id)
        .fetch_one(&gateway.pool)
        .await?;
    assert_eq!(state, "UNKNOWN");

    drop(router);
    let _ = shutdown.send(());
    server.await??;
    owner.pool.close().await;
    gateway.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn gateway_recovers_follow_and_unfollow_from_friend_owner_receipts() -> anyhow::Result<()> {
    let follower_id = Uuid::new_v4();
    let followee_id = Uuid::new_v4();
    let follower = follower_id.to_string();
    let followee = followee_id.to_string();
    let run_id = Uuid::new_v4().simple().to_string();
    let follow_key = format!("friend-follow-{run_id}");
    let unfollow_key = format!("friend-unfollow-{run_id}");
    let missing_key = format!("friend-never-committed-{run_id}");

    let owner = friend_owner_support::start_postgres_app(false).await?;
    let gateway = support::start_gateway_app(false).await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}", listener.local_addr()?);
    let registry = CallerRegistry::new().register(Workload::Gateway, "test-gateway-credential")?;
    let service = friend_proto::friend_service_server::FriendServiceServer::with_interceptor(
        FriendGrpcServer::new(owner.state.clone()),
        RequireRegisteredCallerInterceptor::new(registry)?,
    );
    let (shutdown, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(
        tonic::transport::Server::builder()
            .add_service(service)
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = stopped.await;
            }),
    );
    let client =
        GrpcFriendClient::connect(endpoint, Workload::Gateway, "test-gateway-credential", None)
            .await?;
    let mut state = (*gateway.state).clone();
    state.friend_service = FriendGatewayService::new(Arc::new(client));
    let router = http::server::with_transport_layers(
        http::router(Arc::new(state)),
        &gateway.config,
        http::client_identity::TrustedProxies::default(),
    );

    // The owner commits the follow, while the Gateway journal is prevented
    // from recording completion. A same-key retry must only query FriendService.
    let completion_constraint = format!(
        "alter table gateway_request_journal add constraint reject_friend_completion check (idempotency_key <> '{follow_key}' or state <> 'COMPLETED')"
    );
    sqlx::query(&completion_constraint)
        .execute(&gateway.pool)
        .await?;
    let follow_body = format!(r#"{{"targetUserId":"{followee}"}}"#);
    let first = router
        .clone()
        .oneshot(request(
            "POST",
            "/api/follows",
            &follow_key,
            &follow_body,
            &follower,
        ))
        .await?;
    assert_eq!(first.status(), StatusCode::OK);
    let operation_id = first.headers()["x-operation-id"].to_str()?.to_owned();
    let first_body = body(first).await;
    sqlx::query("alter table gateway_request_journal drop constraint reject_friend_completion")
        .execute(&gateway.pool)
        .await?;

    let status = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/operations/{operation_id}"))
                .header(
                    "authorization",
                    format!(
                        "Bearer {}",
                        support::bearer_token(
                            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                            &["ROLE_ADMIN"]
                        )
                    ),
                )
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(status.status(), StatusCode::OK);
    let status = body(status).await;
    assert_eq!(status["ownerReceipt"]["lookupStatus"], "FOUND");
    assert_eq!(status["ownerReceipt"]["scope"], "friend.v1.FollowUser");
    assert!(status.get("actorId").is_none());

    let recovered = router
        .clone()
        .oneshot(request(
            "POST",
            "/api/follows",
            &follow_key,
            &follow_body,
            &follower,
        ))
        .await?;
    assert_eq!(recovered.status(), StatusCode::OK);
    assert_eq!(recovered.headers()["x-operation-id"], operation_id);
    assert_eq!(recovered.headers()["idempotency-replayed"], "true");
    assert_eq!(body(recovered).await, first_body);
    let count: (i64, i64) = sqlx::query_as(
        "select (select count(*) from follows where follower_id=$1 and followee_id=$2 and deleted_at is null), (select count(*) from friend_mutation_receipts where actor_id=$1)",
    )
    .bind(follower_id)
    .bind(followee_id)
    .fetch_one(&owner.pool)
    .await?;
    assert_eq!(count, (1, 1));
    let audit_owner: Option<String> = sqlx::query_scalar(
        "select metadata_json::jsonb->>'owner' from admin_audit_events where action='OPERATION_OWNER_RECONCILED' and resource_id=$1",
    )
    .bind(&operation_id)
    .fetch_optional(&gateway.pool)
    .await?;
    assert_eq!(audit_owner.as_deref(), Some("friend-service"));

    let unfollow_path = format!("/api/follows/{followee}");
    let first_unfollow = router
        .clone()
        .oneshot(request(
            "DELETE",
            &unfollow_path,
            &unfollow_key,
            "",
            &follower,
        ))
        .await?;
    assert_eq!(first_unfollow.status(), StatusCode::OK);
    let first_unfollow_body = body(first_unfollow).await;
    let replayed_unfollow = router
        .clone()
        .oneshot(request(
            "DELETE",
            &unfollow_path,
            &unfollow_key,
            "",
            &follower,
        ))
        .await?;
    assert_eq!(replayed_unfollow.status(), StatusCode::OK);
    assert_eq!(replayed_unfollow.headers()["idempotency-replayed"], "true");
    assert_eq!(body(replayed_unfollow).await, first_unfollow_body);
    let unfollow_receipt: (i64, i64) = sqlx::query_as(
        "select (select count(*) from follows where follower_id=$1 and followee_id=$2 and deleted_at is null), (select count(*) from friend_mutation_receipts where actor_id=$1)",
    )
    .bind(follower_id)
    .bind(followee_id)
    .fetch_one(&owner.pool)
    .await?;
    assert_eq!(unfollow_receipt, (0, 2));

    let changed = router
        .clone()
        .oneshot(request(
            "POST",
            "/api/follows",
            &follow_key,
            r#"{"targetUserId":"55555555-5555-5555-5555-555555555555"}"#,
            &follower,
        ))
        .await?;
    assert_eq!(changed.status(), StatusCode::CONFLICT);
    let edges: i64 = sqlx::query_scalar(
        "select count(*) from follows where follower_id=$1 and deleted_at is null",
    )
    .bind(follower_id)
    .fetch_one(&owner.pool)
    .await?;
    assert_eq!(edges, 0, "changed-key replay must not redispatch follow");

    // A Gateway intent can outlive an owner receipt after a process/network
    // failure. Retrying that key invokes lookup only and must not invent a follow.
    let missing_id = Uuid::new_v4();
    let missing_body = format!(r#"{{"targetUserId":"{followee}"}}"#);
    let fingerprint_context = json!([
        follower.as_str(),
        "POST",
        "/api/follows",
        "application/json"
    ])
    .to_string();
    sqlx::query("insert into gateway_request_journal (id,actor_id,action,resource_path,idempotency_key,request_fingerprint,state,outcome,outcome_evidence) values ($1,$2,'POST /api/follows','/api/follows',$3,encode(sha256(convert_to($4,'UTF8') || $5::bytea),'hex'),'UNKNOWN','UNKNOWN','LEGACY_UNRESOLVED')")
        .bind(missing_id)
        .bind(follower_id)
        .bind(&missing_key)
        .bind(fingerprint_context)
        .bind(missing_body.as_bytes())
        .execute(&gateway.pool)
        .await?;
    let missing = router
        .clone()
        .oneshot(request(
            "POST",
            "/api/follows",
            &missing_key,
            &missing_body,
            &follower,
        ))
        .await?;
    assert_eq!(missing.status(), StatusCode::CONFLICT);
    assert!(body(missing).await["message"]
        .as_str()
        .unwrap()
        .contains("outcome is unknown"));
    let unresolved: String =
        sqlx::query_scalar("select state from gateway_request_journal where id=$1")
            .bind(missing_id)
            .fetch_one(&gateway.pool)
            .await?;
    let no_extra_receipt: i64 =
        sqlx::query_scalar("select count(*) from friend_mutation_receipts where actor_id=$1")
            .bind(follower_id)
            .fetch_one(&owner.pool)
            .await?;
    assert_eq!(unresolved, "UNKNOWN");
    assert_eq!(no_extra_receipt, 2);

    drop(router);
    let _ = shutdown.send(());
    server.await??;
    owner.shutdown().await?;
    gateway.shutdown().await?;
    Ok(())
}
