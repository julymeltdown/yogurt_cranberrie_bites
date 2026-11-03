#[path = "../../profile-service/tests/support/mod.rs"]
mod profile_owner_support;
#[path = "support/mod.rs"]
mod support;

use api_gateway::adapters::{out::profile_client::GrpcProfileClient, r#in::http};
use api_gateway::application::{
    ports::{FriendClientPort, PostClientPort},
    service::profile_gateway_service::ProfileGatewayService,
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use http_body_util::BodyExt;
use platform::grpc_auth::{RequireRegisteredCallerInterceptor, Workload};
use profile_service::adapters::r#in::grpc::server::proto::profile_service_server::ProfileServiceServer;
use profile_service::adapters::r#in::grpc::server::ProfileGrpcServer;
use profile_service::application::dto::profile_dto::UpdateProfileDto;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio_stream::wrappers::TcpListenerStream;
use tower::ServiceExt;
use uuid::Uuid;

fn request(key: &str, body: &str, actor: &str) -> Request<Body> {
    Request::builder()
        .method("PUT")
        .uri("/api/profile/privacy")
        .header(
            "authorization",
            format!("Bearer {}", support::bearer_token(actor, &["ROLE_USER"])),
        )
        .header("content-type", "application/json")
        .header("idempotency-key", key)
        // Public headers cannot select the owner operation identity.
        .header("x-owner-operation-id", Uuid::nil().to_string())
        .body(Body::from(body.to_owned()))
        .unwrap()
}

fn profile_edit_request(key: &str, body: &str, actor: &str) -> Request<Body> {
    Request::builder()
        .method("PUT")
        .uri(format!("/api/profile/{actor}"))
        .header(
            "authorization",
            format!("Bearer {}", support::bearer_token(actor, &["ROLE_USER"])),
        )
        .header("content-type", "application/json")
        .header("idempotency-key", key)
        // Public headers cannot select the owner operation identity.
        .header("x-owner-operation-id", Uuid::nil().to_string())
        .body(Body::from(body.to_owned()))
        .unwrap()
}

async fn body(response: Response) -> Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

#[tokio::test]
async fn gateway_recovers_profile_privacy_from_the_profile_owner_receipt() -> anyhow::Result<()> {
    let author_id = Uuid::new_v4();
    let author = author_id.to_string();
    let run_id = Uuid::new_v4().simple().to_string();
    let privacy_key = format!("privacy-{run_id}");
    let missing_key = format!("privacy-never-committed-{run_id}");
    let edit_key = format!("profile-edit-{run_id}");
    let missing_edit_key = format!("profile-edit-never-committed-{run_id}");
    let user_suffix = &run_id[..8];
    let owner_username = format!("powner_{user_suffix}");
    let edit_username = format!("pedit_{user_suffix}");
    let edit_bio = format!("Profile edit owner replay test {run_id}");

    let owner = profile_owner_support::start_postgres_app(false).await?;
    owner
        .state
        .update_profile
        .execute(UpdateProfileDto {
            user_id: author.clone(),
            username: owner_username,
            nickname: "Privacy Owner".into(),
            avatar_url: "/avatars/privacy.svg".into(),
            bio: "Profile owner replay integration test".into(),
        })
        .await?;

    let gateway = support::start_gateway_app(false).await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}", listener.local_addr()?);
    let interceptor =
        RequireRegisteredCallerInterceptor::new(profile_owner_support::test_caller_registry())?;
    let service = ProfileServiceServer::with_interceptor(
        ProfileGrpcServer::new(owner.state.clone()),
        interceptor,
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
        GrpcProfileClient::connect(endpoint, Workload::Gateway, "test-gateway-credential", None)
            .await?;
    let mut state = (*gateway.state).clone();
    state.profile_service = ProfileGatewayService::new(
        Arc::new(client),
        gateway.friend_client.clone() as Arc<dyn FriendClientPort>,
        gateway.post_client.clone() as Arc<dyn PostClientPort>,
        gateway.state.search_cursor_codec.clone(),
    );
    let router = http::server::with_transport_layers(
        http::router(Arc::new(state)),
        &gateway.config,
        http::client_identity::TrustedProxies::default(),
    );

    // The owner commits, while the Gateway cannot persist its completion state.
    let completion_constraint = format!(
        "alter table gateway_request_journal add constraint reject_profile_completion check (idempotency_key <> '{privacy_key}' or state <> 'COMPLETED')"
    );
    sqlx::query(&completion_constraint)
        .execute(&gateway.pool)
        .await?;
    let input = r#"{"accountVisibility":"PRIVATE"}"#;
    let first = router
        .clone()
        .oneshot(request(&privacy_key, input, &author))
        .await?;
    assert_eq!(first.status(), StatusCode::OK);
    let operation_id = first.headers()["x-operation-id"].to_str()?.to_owned();
    let first_body = body(first).await;
    assert_eq!(first_body["accountVisibility"], "PRIVATE");
    let journal_state: String =
        sqlx::query_scalar("select state from gateway_request_journal where id=$1")
            .bind(Uuid::parse_str(&operation_id)?)
            .fetch_one(&gateway.pool)
            .await?;
    assert_eq!(journal_state, "IN_PROGRESS");
    sqlx::query("alter table gateway_request_journal drop constraint reject_profile_completion")
        .execute(&gateway.pool)
        .await?;

    let admin_token =
        support::bearer_token("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", &["ROLE_ADMIN"]);
    let status = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/operations/{operation_id}"))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(status.status(), StatusCode::OK);
    let status = body(status).await;
    assert_eq!(status["ownerReceipt"]["lookupStatus"], "FOUND");
    assert_eq!(
        status["ownerReceipt"]["scope"],
        "profile.v1.UpdateProfilePrivacy"
    );
    assert!(status.get("actorId").is_none());

    let recovered = router
        .clone()
        .oneshot(request(&privacy_key, input, &author))
        .await?;
    assert_eq!(recovered.status(), StatusCode::OK);
    assert_eq!(recovered.headers()["x-operation-id"], operation_id);
    assert_eq!(recovered.headers()["idempotency-replayed"], "true");
    assert_eq!(body(recovered).await, first_body);

    let changed = router
        .clone()
        .oneshot(request(
            &privacy_key,
            r#"{"accountVisibility":"PUBLIC"}"#,
            &author,
        ))
        .await?;
    assert_eq!(changed.status(), StatusCode::CONFLICT);

    // A Gateway intent without owner evidence is lookup-only and cannot create
    // a receipt or change the profile on retry.
    let missing_id = Uuid::new_v4();
    let missing_input = r#"{"accountVisibility":"PUBLIC"}"#;
    let fingerprint_context = json!([
        author.as_str(),
        "PUT",
        "/api/profile/privacy",
        "application/json"
    ])
    .to_string();
    sqlx::query("insert into gateway_request_journal (id,actor_id,action,resource_path,idempotency_key,request_fingerprint,state,outcome,outcome_evidence) values ($1,$2,'PUT /api/profile/privacy','/api/profile/privacy',$3,encode(sha256(convert_to($4,'UTF8') || $5::bytea),'hex'),'UNKNOWN','UNKNOWN','LEGACY_UNRESOLVED')")
        .bind(missing_id)
        .bind(&author)
        .bind(&missing_key)
        .bind(fingerprint_context)
        .bind(missing_input.as_bytes())
        .execute(&gateway.pool)
        .await?;
    let missing = router
        .clone()
        .oneshot(request(&missing_key, missing_input, &author))
        .await?;
    assert_eq!(missing.status(), StatusCode::CONFLICT);

    // A second owner-backed Profile route receives the same lost-completion
    // treatment, but is independently bound to the profile-edit RPC scope.
    let edit_completion_constraint = format!(
        "alter table gateway_request_journal add constraint reject_profile_edit_completion check (idempotency_key <> '{edit_key}' or state <> 'COMPLETED')"
    );
    sqlx::query(&edit_completion_constraint)
        .execute(&gateway.pool)
        .await?;
    let edit_input = json!({
        "username": edit_username.clone(),
        "nickname": "Updated Owner",
        "avatarUrl": "/avatars/updated-owner.svg",
        "bio": edit_bio.clone(),
    })
    .to_string();
    let first_edit = router
        .clone()
        .oneshot(profile_edit_request(&edit_key, &edit_input, &author))
        .await?;
    let first_edit_status = first_edit.status();
    let edit_operation_id = first_edit.headers()["x-operation-id"].to_str()?.to_owned();
    let first_edit_body = body(first_edit).await;
    assert_eq!(first_edit_status, StatusCode::OK, "{first_edit_body:?}");
    assert_eq!(first_edit_body["username"], edit_username);
    let edit_journal_state: String =
        sqlx::query_scalar("select state from gateway_request_journal where id=$1")
            .bind(Uuid::parse_str(&edit_operation_id)?)
            .fetch_one(&gateway.pool)
            .await?;
    assert_eq!(edit_journal_state, "IN_PROGRESS");
    sqlx::query(
        "alter table gateway_request_journal drop constraint reject_profile_edit_completion",
    )
    .execute(&gateway.pool)
    .await?;

    let edit_status = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/operations/{edit_operation_id}"))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(edit_status.status(), StatusCode::OK);
    let edit_status = body(edit_status).await;
    assert_eq!(edit_status["ownerReceipt"]["lookupStatus"], "FOUND");
    assert_eq!(
        edit_status["ownerReceipt"]["scope"],
        "profile.v1.UpdateProfile"
    );

    let recovered_edit = router
        .clone()
        .oneshot(profile_edit_request(&edit_key, &edit_input, &author))
        .await?;
    assert_eq!(recovered_edit.status(), StatusCode::OK);
    assert_eq!(
        recovered_edit.headers()["x-operation-id"],
        edit_operation_id
    );
    assert_eq!(recovered_edit.headers()["idempotency-replayed"], "true");
    assert_eq!(body(recovered_edit).await, first_edit_body);

    let changed_edit_input = json!({
        "username": edit_username.clone(),
        "nickname": "Updated Owner",
        "avatarUrl": "/avatars/updated-owner.svg",
        "bio": format!("Changed body {run_id}"),
    })
    .to_string();
    let changed_edit = router
        .clone()
        .oneshot(profile_edit_request(
            &edit_key,
            &changed_edit_input,
            &author,
        ))
        .await?;
    assert_eq!(changed_edit.status(), StatusCode::CONFLICT);

    let missing_edit_id = Uuid::new_v4();
    let missing_edit_input = json!({
        "username": edit_username,
        "nickname": "Must Not Apply",
        "avatarUrl": "/avatars/missing.svg",
        "bio": "Lookup only",
    })
    .to_string();
    let edit_path = format!("/api/profile/{author}");
    let edit_fingerprint_context = json!([
        author.as_str(),
        "PUT",
        edit_path.as_str(),
        "application/json"
    ])
    .to_string();
    sqlx::query("insert into gateway_request_journal (id,actor_id,action,resource_path,idempotency_key,request_fingerprint,state,outcome,outcome_evidence) values ($1,$2,$3,$4,$5,encode(sha256(convert_to($6,'UTF8') || $7::bytea),'hex'),'UNKNOWN','UNKNOWN','LEGACY_UNRESOLVED')")
        .bind(missing_edit_id)
        .bind(&author)
        .bind(format!("PUT {edit_path}"))
        .bind(&edit_path)
        .bind(&missing_edit_key)
        .bind(edit_fingerprint_context)
        .bind(missing_edit_input.as_bytes())
        .execute(&gateway.pool)
        .await?;
    let missing_edit = router
        .clone()
        .oneshot(profile_edit_request(
            &missing_edit_key,
            &missing_edit_input,
            &author,
        ))
        .await?;
    assert_eq!(missing_edit.status(), StatusCode::CONFLICT);
    let unchanged_bio: String = sqlx::query_scalar("select bio from profiles where user_id=$1")
        .bind(author_id)
        .fetch_one(&owner.pool)
        .await?;
    assert_eq!(unchanged_bio, edit_bio);

    let effects: (String, i64, i64, i64) = sqlx::query_as(
        "select (select account_visibility from profiles where user_id=$1),
                (select count(*) from profile_mutation_receipts where actor_id=$1 and scope='profile.v1.UpdateProfilePrivacy'),
                (select count(*) from profile_audit_events where profile_user_id=$1 and action='UPDATE_PRIVACY'),
                (select count(*) from profile_mutation_receipts where actor_id=$1 and scope='profile.v1.UpdateProfile')",
    )
    .bind(author_id)
    .fetch_one(&owner.pool)
    .await?;
    assert_eq!(effects, ("PRIVATE".into(), 1, 1, 1));

    drop(router);
    let _ = shutdown.send(());
    server.await??;
    owner.pool.close().await;
    gateway.shutdown().await?;
    Ok(())
}
