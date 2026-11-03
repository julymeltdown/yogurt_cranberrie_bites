//! Scoped Post application / real Friend owner acceptance. Post repositories
//! and caches are recording ports; this is not the six-service process journey.
use anyhow::Result;
use platform::grpc_auth::Workload;
use post_service::adapters::out::grpc::friend_client::GrpcFriendClient;
use post_service::application::ports::FriendClientPort;
use post_service::domain::feed::FeedPage;
use std::sync::Arc;
use tokio::{net::TcpListener, sync::oneshot};
use tokio_stream::wrappers::TcpListenerStream;
use uuid::Uuid;

#[path = "../../../services/friend-service/tests/support/mod.rs"]
mod friend_support;
#[allow(dead_code)]
#[path = "../../../services/post-service/tests/unit/support.rs"]
mod post_probes;

#[tokio::test]
async fn post_cold_feed_reads_all_205_follows_through_authenticated_friend_owner() -> Result<()> {
    let app = friend_support::start_postgres_app(false).await?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = friend_support::grpc_server(app.state.clone());
    let service = platform::grpc_method_policy::guard(
        &friend_service::adapters::r#in::grpc::method_policy::FRIEND_METHOD_POLICY,
        friend_service::adapters::r#in::grpc::server::proto::friend_service_server::FriendServiceServer::new(server),
        platform::grpc_auth::CallerRegistry::new().register(Workload::Post, "test-post-credential")?,
        platform::actor_delegation::ActorDelegationVerifier::local_parity(Workload::Friend),
    )?;
    let (stop, stopped) = oneshot::channel();
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .layer(platform::correlation::CorrelationLayer)
            .layer(platform::trace_context::TraceContextLayer)
            .add_service(service)
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = stopped.await;
            })
            .await
    });
    let result = async {
        let viewer = Uuid::from_u128(10_000);
        let authors = (1..=205).map(Uuid::from_u128).collect::<Vec<_>>();
        sqlx::query("insert into follows (follower_id,followee_id) select $1, target from unnest($2::uuid[]) target")
            .bind(viewer).bind(&authors).execute(&app.pool).await?;
        let client = GrpcFriendClient::connect(format!("http://{address}"), Workload::Post, "test-post-credential", None).await?;
        // Confirm the real adapter's limit remains enforced. The application
        // must respect this contract instead of weakening the owner guard.
        assert!(client.batch_relationship_policies(viewer,&authors[..101],None).await.unwrap_err().to_string().contains("at most 100"));
        let mut harness = post_probes::Harness::new();
        Arc::get_mut(&mut harness.service).expect("new harness has one service owner").friend_client = Arc::new(client);
        let first = post_probes::post(Uuid::from_u128(20_000), authors[204], "last follow in the final owner page");
        let second = post_probes::post(Uuid::from_u128(20_001), authors[0], "first follow");
        *harness.posts.feed_page.lock().unwrap() = Some(FeedPage { posts:vec![first.clone(),second.clone()],next_cursor:Some("owner-next".into()) });
        let response = harness.service.get_feed(viewer,None,20).await?;
        assert_eq!(response.posts.iter().map(|p|p.id.clone()).collect::<Vec<_>>(),vec![first.id.to_string(),second.id.to_string()]);
        assert_eq!(response.next_cursor.as_deref(),Some("owner-next"));
        let ids = harness.posts.fetch_feed_calls.lock().unwrap()[0].author_ids.clone();
        assert_eq!(ids.len(),206);
        assert!(authors.iter().all(|id|ids.contains(id)) && ids.contains(&viewer));
        sqlx::query("insert into user_blocks(blocker_id,blocked_id,created_at) values ($1,$2,now())").bind(authors[204]).bind(viewer).execute(&app.pool).await?;
        let response = harness.service.get_feed(viewer,None,20).await?;
        assert_eq!(response.posts.iter().map(|p|p.id.clone()).collect::<Vec<_>>(),vec![second.id.to_string()]);
        assert_eq!(response.next_cursor.as_deref(),Some("owner-next"));
        assert!(!harness.posts.fetch_feed_calls.lock().unwrap()[1].author_ids.contains(&authors[204]));
        Ok::<(),anyhow::Error>(())
    }.await;
    let _ = stop.send(());
    tokio::time::timeout(std::time::Duration::from_secs(5), task).await???;
    std::net::TcpListener::bind(address)?;
    app.pool.close().await;
    result
}
