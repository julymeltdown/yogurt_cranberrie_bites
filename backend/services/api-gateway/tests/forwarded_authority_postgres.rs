//! Composed real mTLS/PG/Redis forwarding. Post runs its actual search use case;
//! Friend delegates to its actual handler and PostgreSQL policy repository.
#[path = "../../friend-service/tests/support/mod.rs"]
mod friend_support;
#[path = "../../post-service/tests/support/mod.rs"]
mod post_support;
#[path = "../../profile-service/tests/support/mod.rs"]
mod profile_support;
use api_gateway::adapters::out::authority_exchange::DelegationExchangeRuntime;
use api_gateway::adapters::out::owner_resource_client::GrpcPostResourceAuthorityClient;
use api_gateway::adapters::r#in::grpc::authority_exchange::{
    proto as authority, GatewayAuthorizationExchangeGrpcServer, AUTHORIZATION_EXCHANGE_POLICY,
};
use friend::friend_service_server::FriendService;
use friend_service::adapters::r#in::grpc::server::{proto as friend, FriendGrpcServer};
use platform::actor_delegation::exchange::{
    DelegationExchange, ResourceEvidenceSigner, ResourceEvidenceVerifier,
};
use platform::actor_delegation::{
    ActorDelegationSigner, ActorDelegationVerifier, VerifiedActor, HEADER,
};
use platform::edge::{EdgeBudget, FailurePolicy, RetryClass};
use platform::grpc_auth::{CallerRegistry, Workload};
use platform::grpc_method_policy::{
    guard, guard_with_forwarding, guard_without_delegation, ForwardedMethod,
};
use platform::grpc_tls::TlsMaterial;
use post_service::adapters::out::grpc::authority_exchange_client::GrpcAuthorizationExchangeClient;
use post_service::adapters::out::grpc::friend_client::GrpcFriendClient;
use post_service::adapters::out::persistence::authority_witness_repository::PgAuthorityWitnessRepository;
use post_service::adapters::r#in::grpc::method_policy::POST_METHOD_POLICY;
use post_service::adapters::r#in::grpc::query_witness::AdmittedQueryWitnessProducer;
use post_service::adapters::r#in::grpc::resource_authority::{
    proto as owner, PostResourceAuthorityGrpcServer, POST_RESOURCE_AUTHORITY_POLICY,
};
use post_service::adapters::r#in::grpc::server::proto as post;
use post_service::application::use_cases::PostApplicationService;
use prost::Message;
use sqlx::{PgPool, Row};
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::body::Body;
use tonic::transport::{Channel, Server};
use tonic::{Code, Request, Response, Status};
use uuid::Uuid;

const SEARCH: &str = "/post.v1.PostService/SearchPosts";
const BATCH: &str = "/friend.v1.FriendService/BatchGetRelationships";
static FRIEND_PROOF: &[ForwardedMethod] = &[
    ForwardedMethod {
        method: "ListFollowing",
        caller: Workload::Post,
        actor_fields: &[&[1]],
    },
    ForwardedMethod {
        method: "BatchGetRelationships",
        caller: Workload::Post,
        actor_fields: &[&[1]],
    },
];
#[derive(Clone, PartialEq, Message)]
struct SearchResult {
    #[prost(uint32, tag = "1")]
    count: u32,
    #[prost(string, repeated, tag = "2")]
    post_ids: Vec<String>,
}
#[derive(Clone)]
struct SearchOwner(Arc<PostApplicationService>);
impl tonic::server::NamedService for SearchOwner {
    const NAME: &'static str = "post.v1.PostService";
}
type UnaryFuture<T> = Pin<Box<dyn Future<Output = Result<Response<T>, Status>> + Send>>;
impl tonic::server::UnaryService<post::SearchPostsRequest> for SearchOwner {
    type Response = SearchResult;
    type Future = UnaryFuture<SearchResult>;
    fn call(&mut self, request: Request<post::SearchPostsRequest>) -> Self::Future {
        let app = self.0.clone();
        Box::pin(async move {
            let request = request.into_inner();
            let viewer = Uuid::parse_str(&request.viewer_id)
                .map_err(|_| Status::invalid_argument("viewer"))?;
            let page = app
                .search_posts(viewer, &request.query, None, request.limit)
                .await
                .map_err(|_| Status::unavailable("Search authority dependency unavailable"))?;
            Ok(Response::new(SearchResult {
                count: page.posts.len() as u32,
                post_ids: page.posts.iter().map(|post| post.id.clone()).collect(),
            }))
        })
    }
}
impl tower::Service<http::Request<Body>> for SearchOwner {
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Infallible>> + Send>>;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, request: http::Request<Body>) -> Self::Future {
        let handler = self.clone();
        Box::pin(async move {
            Ok(tonic::server::Grpc::new(tonic_prost::ProstCodec::<
                SearchResult,
                post::SearchPostsRequest,
            >::default())
            .unary(handler, request)
            .await)
        })
    }
}
#[derive(Clone)]
struct FeedOwner(Arc<PostApplicationService>);
impl tonic::server::NamedService for FeedOwner {
    const NAME: &'static str = "post.v1.PostService";
}
impl tonic::server::UnaryService<post::FeedRequest> for FeedOwner {
    type Response = SearchResult;
    type Future = UnaryFuture<SearchResult>;
    fn call(&mut self, request: Request<post::FeedRequest>) -> Self::Future {
        let app = self.0.clone();
        Box::pin(async move {
            let request = request.into_inner();
            let viewer = request
                .user_id
                .parse()
                .map_err(|_| Status::invalid_argument("viewer"))?;
            let cursor = (!request.cursor.is_empty()).then_some(request.cursor.as_str());
            let page = app
                .get_feed(viewer, cursor, request.limit)
                .await
                .map_err(|_| Status::unavailable("Feed authority dependency unavailable"))?;
            Ok(Response::new(SearchResult {
                count: page.posts.len() as u32,
                post_ids: page.posts.into_iter().map(|p| p.id).collect(),
            }))
        })
    }
}
impl tower::Service<http::Request<Body>> for FeedOwner {
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Infallible>> + Send>>;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, request: http::Request<Body>) -> Self::Future {
        let handler = self.clone();
        Box::pin(async move {
            Ok(tonic::server::Grpc::new(
                tonic_prost::ProstCodec::<SearchResult, post::FeedRequest>::default(),
            )
            .unary(handler, request)
            .await)
        })
    }
}

#[derive(Clone)]
struct CreateOwner(Arc<PostApplicationService>);
impl tonic::server::NamedService for CreateOwner {
    const NAME: &'static str = "post.v1.PostService";
}
impl tonic::server::UnaryService<post::CreatePostRequest> for CreateOwner {
    type Response = post::CreatePostResponse;
    type Future = UnaryFuture<Self::Response>;
    fn call(&mut self, request: Request<post::CreatePostRequest>) -> Self::Future {
        let app = self.0.clone();
        Box::pin(async move {
            let request = request.into_inner();
            let result = app
                .create_post(post_service::application::dto::post_dto::CreatePostDto {
                    author_id: request.author_id,
                    content: request.content,
                    visibility: request.visibility,
                })
                .await
                .map_err(|error| {
                    error.downcast_ref::<Status>().cloned().unwrap_or_else(|| {
                        Status::unavailable("Post author dependency unavailable")
                    })
                })?;
            Ok(Response::new(post::CreatePostResponse {
                post: Some(post::Post {
                    id: result.id,
                    author_id: result.author_id,
                    content: result.content,
                    visibility: result.visibility,
                    created_at: result.created_at,
                    comment_count: result.comment_count,
                    like_count: result.like_count,
                    liked_by_me: result.liked_by_me,
                }),
            }))
        })
    }
}
impl tower::Service<http::Request<Body>> for CreateOwner {
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Infallible>> + Send>>;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, request: http::Request<Body>) -> Self::Future {
        let handler = self.clone();
        Box::pin(async move {
            Ok(tonic::server::Grpc::new(tonic_prost::ProstCodec::<
                post::CreatePostResponse,
                post::CreatePostRequest,
            >::default())
            .unary(handler, request)
            .await)
        })
    }
}

#[derive(Clone)]
struct FriendOwner {
    entries: Arc<AtomicUsize>,
    server: FriendGrpcServer,
    mute_after_list: Option<(PgPool, Uuid, Uuid)>,
}
impl tonic::server::NamedService for FriendOwner {
    const NAME: &'static str = "friend.v1.FriendService";
}
impl tonic::server::UnaryService<friend::BatchRelationshipRequest> for FriendOwner {
    type Response = friend::BatchRelationshipResponse;
    type Future = UnaryFuture<Self::Response>;
    fn call(&mut self, request: Request<friend::BatchRelationshipRequest>) -> Self::Future {
        let entries = self.entries.clone();
        let server = self.server.clone();
        Box::pin(async move {
            let actor = request
                .extensions()
                .get::<VerifiedActor>()
                .ok_or_else(|| Status::internal("verified child required"))?
                .0;
            if actor.to_string() != request.get_ref().viewer_id {
                return Err(Status::permission_denied("viewer"));
            }
            entries.fetch_add(1, Ordering::SeqCst);
            server.batch_get_relationships(request).await
        })
    }
}
impl tonic::server::UnaryService<friend::ListFollowingRequest> for FriendOwner {
    type Response = friend::ListFollowingResponse;
    type Future = UnaryFuture<Self::Response>;
    fn call(&mut self, request: Request<friend::ListFollowingRequest>) -> Self::Future {
        let entries = self.entries.clone();
        let server = self.server.clone();
        let transition = self.mute_after_list.clone();
        Box::pin(async move {
            let actor = request
                .extensions()
                .get::<VerifiedActor>()
                .ok_or_else(|| Status::internal("verified child required"))?
                .0;
            if actor.to_string() != request.get_ref().user_id {
                return Err(Status::permission_denied("viewer"));
            }
            entries.fetch_add(1, Ordering::SeqCst);
            let response = server.list_following(request).await?;
            if let Some((pool, viewer, author)) = transition {
                sqlx::query("insert into user_mutes(muter_id,muted_id,created_at) values($1,$2,clock_timestamp()) on conflict(muter_id,muted_id) do update set deleted_at=null")
                    .bind(viewer).bind(author).execute(&pool).await.map_err(|_| Status::internal("fixture transition"))?;
            }
            Ok(response)
        })
    }
}
impl tower::Service<http::Request<Body>> for FriendOwner {
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Infallible>> + Send>>;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, request: http::Request<Body>) -> Self::Future {
        let handler = self.clone();
        Box::pin(async move {
            if request.uri().path().ends_with("/ListFollowing") {
                return Ok(tonic::server::Grpc::new(tonic_prost::ProstCodec::<
                    friend::ListFollowingResponse,
                    friend::ListFollowingRequest,
                >::default())
                .unary(handler, request)
                .await);
            }
            Ok(tonic::server::Grpc::new(tonic_prost::ProstCodec::<
                friend::BatchRelationshipResponse,
                friend::BatchRelationshipRequest,
            >::default())
            .unary(handler, request)
            .await)
        })
    }
}
struct ServerTask {
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<Result<(), tonic::transport::Error>>>,
}
impl ServerTask {
    fn spawn(
        future: impl Future<Output = Result<(), tonic::transport::Error>> + Send + 'static,
        stop: tokio::sync::oneshot::Sender<()>,
    ) -> Self {
        Self {
            stop: Some(stop),
            task: Some(tokio::spawn(future)),
        }
    }
    async fn stop(self) -> anyhow::Result<()> {
        self.stop_with_timeout(Duration::from_secs(5)).await
    }
    async fn stop_with_timeout(mut self, budget: Duration) -> anyhow::Result<()> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(mut task) = self.task.take() {
            match tokio::time::timeout(budget, &mut task).await {
                Ok(result) => result??,
                Err(error) => {
                    task.abort();
                    let _ = task.await;
                    return Err(error.into());
                }
            }
        }
        Ok(())
    }
}
impl Drop for ServerTask {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}
fn tls(workload: Workload) -> anyhow::Result<TlsMaterial> {
    let leaf = test_support::tls::issue(workload.as_str())?;
    TlsMaterial::load(&leaf.cert, &leaf.key, &leaf.ca)
}
fn registry() -> anyhow::Result<CallerRegistry> {
    CallerRegistry::new()
        .register(Workload::Gateway, "gateway-fixture")?
        .register(Workload::Post, "post-fixture")
}
fn frame<T: Message>(message: &T) -> Vec<u8> {
    let bytes = message.encode_to_vec();
    [vec![0], (bytes.len() as u32).to_be_bytes().to_vec(), bytes].concat()
}
fn identified<T>(
    message: T,
    caller: Workload,
    credential: &str,
    token: Option<&str>,
) -> Request<T> {
    let mut request = Request::new(message);
    request
        .metadata_mut()
        .insert("x-service-caller", caller.as_str().parse().unwrap());
    request
        .metadata_mut()
        .insert("x-service-token", credential.parse().unwrap());
    if let Some(token) = token {
        request
            .metadata_mut()
            .insert(HEADER, token.parse().unwrap());
    }
    request
}
async fn search(
    channel: Channel,
    message: post::SearchPostsRequest,
    token: Option<&str>,
) -> Result<Vec<String>, Code> {
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.map_err(|_| Code::Unavailable)?;
    let response: Result<Response<SearchResult>, Status> = client
        .unary(
            identified(message, Workload::Gateway, "gateway-fixture", token),
            http::uri::PathAndQuery::from_static(SEARCH),
            tonic_prost::ProstCodec::<post::SearchPostsRequest, SearchResult>::default(),
        )
        .await;
    response
        .map(|r| {
            let result = r.into_inner();
            assert_eq!(result.count as usize, result.post_ids.len());
            result.post_ids
        })
        .map_err(|s| s.code())
}
async fn friend_call(
    channel: Channel,
    request: friend::BatchRelationshipRequest,
    token: Option<&str>,
) -> Result<friend::BatchRelationshipResponse, Code> {
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.map_err(|_| Code::Unavailable)?;
    let response: Result<Response<friend::BatchRelationshipResponse>, Status> = client
        .unary(
            identified(request, Workload::Post, "post-fixture", token),
            http::uri::PathAndQuery::from_static(BATCH),
            tonic_prost::ProstCodec::<
                friend::BatchRelationshipRequest,
                friend::BatchRelationshipResponse,
            >::default(),
        )
        .await;
    response.map(Response::into_inner).map_err(|s| s.code())
}

#[tokio::test]
async fn actual_search_crosses_independent_exchange_resource_and_friend_mtls_with_pg(
) -> anyhow::Result<()> {
    use anyhow::Context;

    let app = post_support::start_postgres_redis_app(false)
        .await
        .context("search authority: initialize Post PostgreSQL/Redis fixture")?;
    let friend_app = friend_support::start_postgres_app(false)
        .await
        .context("search authority: initialize Friend PostgreSQL fixture")?;
    let viewer = Uuid::new_v4();
    // Persisted owner policy must affect Post visibility, not fixture defaults.
    sqlx::query("insert into follows(follower_id,followee_id) values ($1,$2)")
        .bind(viewer)
        .bind(Uuid::from_u128(26))
        .execute(&friend_app.pool)
        .await
        .with_context(|| {
            format!(
                "search authority: seed Friend follow (pool_size={}, idle={})",
                friend_app.pool.size(),
                friend_app.pool.num_idle()
            )
        })?;
    sqlx::query("insert into user_blocks(blocker_id,blocked_id,created_at) values ($1,$2,clock_timestamp())")
        .bind(Uuid::from_u128(27)).bind(viewer).execute(&friend_app.pool).await
        .with_context(|| format!("search authority: seed Friend block (pool_size={}, idle={})", friend_app.pool.size(), friend_app.pool.num_idle()))?;
    for (id, author) in [(2u128, 26u128), (4, 27)] {
        sqlx::query("insert into posts(id,author_id,content,visibility,created_at) values ($1,$2,'witness query','PUBLIC',clock_timestamp())")
            .bind(Uuid::from_u128(id)).bind(Uuid::from_u128(author)).execute(&app.pool).await
            .with_context(|| format!("search authority: seed Post (pool_size={}, idle={})", app.pool.size(), app.pool.num_idle()))?;
    }
    let post_tls = tls(Workload::Post)?;
    let gateway_tls = tls(Workload::Gateway)?;
    let friend_tls = tls(Workload::Friend)?;
    let effects = Arc::new(AtomicUsize::new(0));
    let friend_listener = TcpListener::bind("127.0.0.1:0").await?;
    let friend_url = format!("https://{}", friend_listener.local_addr()?);
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let friend_server = ServerTask::spawn(
        Server::builder()
            .tls_config(friend_tls.server_config()?)?
            .add_service(guard_with_forwarding(
                &friend_service::adapters::r#in::grpc::method_policy::FRIEND_METHOD_POLICY,
                FRIEND_PROOF,
                FriendOwner {
                    entries: effects.clone(),
                    server: friend_support::grpc_server(friend_app.state.clone()),
                    mute_after_list: None,
                },
                registry()?,
                ActorDelegationVerifier::local_parity(Workload::Friend),
            )?)
            .serve_with_incoming_shutdown(TcpListenerStream::new(friend_listener), async {
                let _ = stopped.await;
            }),
        stop,
    );
    let lookup_pool = PgPool::connect_with(app.pool.connect_options().as_ref().clone())
        .await
        .context("search authority: connect independent Post resource lookup pool")?;
    let owner_key = ResourceEvidenceSigner::from_seed(&[11; 32])?;
    let resource_calls = Arc::new(resource_lookup::Calls::default());
    let resource = PostResourceAuthorityGrpcServer::new(
        Arc::new(resource_lookup::CountedRepository {
            repository: PgAuthorityWitnessRepository::new(lookup_pool.clone()),
            calls: resource_calls.clone(),
        }),
        ActorDelegationVerifier::local_parity(Workload::Post),
        owner_key.clone(),
    )?;
    let post_listener = TcpListener::bind("127.0.0.1:0").await?;
    let post_url = format!("https://{}", post_listener.local_addr()?);
    let gateway_listener = TcpListener::bind("127.0.0.1:0").await?;
    let gateway_url = format!("https://{}", gateway_listener.local_addr()?);
    let owner_client = GrpcPostResourceAuthorityClient::connect(
        &post_url,
        "gateway-fixture",
        Some(&gateway_tls),
        Default::default(),
    )?;
    let issuer = DelegationExchange::new(
        ActorDelegationSigner::local_parity(Workload::Gateway),
        ActorDelegationVerifier::local_parity(Workload::Post),
        ResourceEvidenceVerifier::new(&[owner_key.public_key()])?,
    )?;
    let broker = GatewayAuthorizationExchangeGrpcServer::new(Arc::new(
        DelegationExchangeRuntime::new(issuer, Arc::new(owner_client)),
    ));
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let gateway_server = ServerTask::spawn(
        Server::builder()
            .tls_config(gateway_tls.server_config()?)?
            .add_service(guard_without_delegation(
                &AUTHORIZATION_EXCHANGE_POLICY,
                authority::authorization_exchange_service_server::AuthorizationExchangeServiceServer::new(broker),
                registry()?,
            )?)
            .serve_with_incoming_shutdown(TcpListenerStream::new(gateway_listener), async {
                let _ = stopped.await;
            }),
        stop,
    );
    // Fixture-only injection. No operational edge default or SLO is introduced.
    let budget = EdgeBudget {
        edge: "fixture-post-exchange",
        expected_latency: Duration::from_millis(20),
        deadline: Duration::from_secs(3),
        connect_deadline: Duration::from_secs(1),
        retry: RetryClass::Never,
        failure_policy: FailurePolicy::FailRequest,
    };
    let exchange = GrpcAuthorizationExchangeClient::connect(
        &gateway_url,
        "post-fixture",
        Some(&post_tls),
        budget,
    )?;
    let friend_client = GrpcFriendClient::connect(
        friend_url.clone(),
        Workload::Post,
        "post-fixture",
        Some(&post_tls),
    )
    .await?
    .with_exchange(Arc::new(exchange));
    let application = app.application_with_query_witnesses(
        Arc::new(AdmittedQueryWitnessProducer::new(Arc::new(
            PgAuthorityWitnessRepository::new(app.pool.clone()),
        ))),
        Arc::new(friend_client),
    );
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let post_server = ServerTask::spawn(
        Server::builder()
            .tls_config(post_tls.server_config()?)?
            .add_service(guard(
                &POST_METHOD_POLICY,
                SearchOwner(application),
                registry()?,
                ActorDelegationVerifier::local_parity(Workload::Post),
            )?)
            .add_service(guard_without_delegation(
                &POST_RESOURCE_AUTHORITY_POLICY,
                owner::post_resource_authority_service_server::PostResourceAuthorityServiceServer::new(resource),
                registry()?,
            )?)
            .serve_with_incoming_shutdown(TcpListenerStream::new(post_listener), async {
                let _ = stopped.await;
            }),
        stop,
    );
    let channel = gateway_tls.connect(&post_url, Workload::Post).await?;
    let request = post::SearchPostsRequest {
        query: "witness query".into(),
        cursor: String::new(),
        limit: 1,
        viewer_id: viewer.to_string(),
    };
    let root_body = frame(&request);
    let root = ActorDelegationSigner::local_parity(Workload::Gateway).issue(
        Workload::Post,
        SEARCH,
        viewer,
        None,
        &root_body,
        chrono::Utc::now().timestamp(),
    );
    assert_eq!(
        search(channel.clone(), request.clone(), None).await,
        Err(Code::Unauthenticated)
    );
    let mut changed = request.clone();
    changed.query = "other query".into();
    assert_eq!(
        search(channel.clone(), changed, Some(&root)).await,
        Err(Code::PermissionDenied)
    );
    assert_eq!(effects.load(Ordering::SeqCst), 0);
    assert_eq!(
        search(channel.clone(), request.clone(), Some(&root)).await,
        Ok(vec![Uuid::from_u128(2).to_string()])
    );
    assert_eq!(effects.load(Ordering::SeqCst), 1);
    assert_eq!(
        search(channel, request, Some(&root)).await,
        Err(Code::Unauthenticated)
    );
    let stages =
        sqlx::query("select stage_id,actor_id,post_ids from post_authority_resource_witnesses")
            .fetch_all(&app.pool)
            .await
            .with_context(|| {
                format!(
                    "search authority: inspect persisted witnesses (pool_size={}, idle={})",
                    app.pool.size(),
                    app.pool.num_idle()
                )
            })?;
    assert_eq!(stages.len(), 1);
    assert_eq!(stages[0].try_get::<Uuid, _>("actor_id")?, viewer);
    assert_eq!(
        stages[0].try_get::<Vec<Uuid>, _>("post_ids")?,
        vec![Uuid::from_u128(2), Uuid::from_u128(4)]
    );
    let stage = stages[0].try_get::<Uuid, _>("stage_id")?;
    let child = friend::BatchRelationshipRequest {
        viewer_id: viewer.to_string(),
        target_ids: vec![Uuid::from_u128(26).to_string()],
    };
    let exchange_request = authority::ExchangeActorRequest {
        parent: Some(authority::ParentAuthority {
            token: root.clone(),
            method: SEARCH.into(),
            framed_body: root_body,
            operation_id: None,
        }),
        audience: "friend".into(),
        method: BATCH.into(),
        framed_body: frame(&child),
        operation_id: None,
        witness_id: Some(stage.to_string()),
    };
    let resource_parent = exchange_request.parent.as_ref().unwrap();
    resource_lookup::assert_resolve_post_resources_denials(
        &app.pool,
        &resource_calls,
        gateway_tls.connect(&post_url, Workload::Post).await?,
        post_tls.connect(&post_url, Workload::Post).await?,
        owner::ParentAuthority {
            token: resource_parent.token.clone(),
            method: resource_parent.method.clone(),
            framed_body: resource_parent.framed_body.clone(),
            operation_id: resource_parent.operation_id.clone(),
        },
        stage,
        Uuid::from_u128(26),
    )
    .await?;
    let mut issuer_client =
        authority::authorization_exchange_service_client::AuthorizationExchangeServiceClient::new(
            post_tls.connect(&gateway_url, Workload::Gateway).await?,
        );
    for mutation in [0, 1, 2] {
        let mut denied = exchange_request.clone();
        match mutation {
            0 => denied.witness_id = Some(Uuid::new_v4().to_string()),
            1 => {
                denied.framed_body = frame(&friend::BatchRelationshipRequest {
                    viewer_id: viewer.to_string(),
                    target_ids: vec![Uuid::from_u128(99).to_string()],
                })
            }
            _ => denied.parent.as_mut().unwrap().framed_body.push(0),
        }
        assert_eq!(
            issuer_client
                .exchange_actor(identified(denied, Workload::Post, "post-fixture", None))
                .await
                .unwrap_err()
                .code(),
            Code::PermissionDenied
        );
    }
    assert_eq!(
        issuer_client
            .exchange_actor(identified(
                exchange_request.clone(),
                Workload::Gateway,
                "gateway-fixture",
                None
            ))
            .await
            .unwrap_err()
            .code(),
        Code::Unauthenticated,
        "certificate/credential mismatch"
    );
    let mut wrong_presenter =
        authority::authorization_exchange_service_client::AuthorizationExchangeServiceClient::new(
            gateway_tls.connect(&gateway_url, Workload::Gateway).await?,
        );
    assert_eq!(
        wrong_presenter
            .exchange_actor(identified(
                exchange_request.clone(),
                Workload::Gateway,
                "gateway-fixture",
                None
            ))
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    let issued = issuer_client
        .exchange_actor(identified(
            exchange_request.clone(),
            Workload::Post,
            "post-fixture",
            None,
        ))
        .await?
        .into_inner();
    let friend_channel = post_tls.connect(&friend_url, Workload::Friend).await?;
    assert_eq!(
        friend_call(friend_channel.clone(), child.clone(), None).await,
        Err(Code::Unauthenticated)
    );
    assert_eq!(
        friend_call(friend_channel.clone(), child.clone(), Some(&root)).await,
        Err(Code::Unauthenticated),
        "root cannot act as a child"
    );
    let altered = friend::BatchRelationshipRequest {
        viewer_id: viewer.to_string(),
        target_ids: vec![Uuid::from_u128(27).to_string()],
    };
    assert_eq!(
        friend_call(friend_channel.clone(), altered, Some(&issued.token)).await,
        Err(Code::PermissionDenied)
    );
    assert_eq!(effects.load(Ordering::SeqCst), 1);
    let policies = friend_call(friend_channel.clone(), child.clone(), Some(&issued.token))
        .await
        .map_err(|code| anyhow::anyhow!("Friend call failed: {code:?}"))?;
    assert_eq!(policies.policies.len(), 1);
    let policy = policies.policies[0].policy.as_ref().unwrap();
    assert!(policy.following);
    assert!(!policy.blocked_by_viewer && !policy.blocked_viewer && !policy.muted_by_viewer);
    assert_eq!(
        friend_call(friend_channel, child, Some(&issued.token)).await,
        Err(Code::Unauthenticated)
    );
    assert_eq!(effects.load(Ordering::SeqCst), 2);
    let mut owner_client =
        owner::post_resource_authority_service_client::PostResourceAuthorityServiceClient::new(
            post_tls.connect(&post_url, Workload::Post).await?,
        );
    let parent = exchange_request.parent.as_ref().unwrap();
    let lookup = owner::ResolvePostResourcesRequest {
        parent: Some(owner::ParentAuthority {
            token: parent.token.clone(),
            method: parent.method.clone(),
            framed_body: parent.framed_body.clone(),
            operation_id: None,
        }),
        witness_id: stage.to_string(),
        target_author_ids: vec![Uuid::from_u128(26).to_string()],
    };
    assert_eq!(
        owner_client
            .resolve_post_resources(identified(lookup, Workload::Post, "post-fixture", None))
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied,
        "presenter cannot resolve its own evidence"
    );
    let mut later_tokens = Vec::new();
    for _ in 0..2 {
        later_tokens.push(
            issuer_client
                .exchange_actor(identified(
                    exchange_request.clone(),
                    Workload::Post,
                    "post-fixture",
                    None,
                ))
                .await?
                .into_inner()
                .token,
        );
    }
    sqlx::query("update posts set moderation_hidden_at=clock_timestamp() where id=$1")
        .bind(Uuid::from_u128(2))
        .execute(&app.pool)
        .await
        .with_context(|| {
            format!(
                "search authority: hide Post before lifecycle recheck (pool_size={}, idle={})",
                app.pool.size(),
                app.pool.num_idle()
            )
        })?;
    assert_eq!(
        issuer_client
            .exchange_actor(identified(
                exchange_request.clone(),
                Workload::Post,
                "post-fixture",
                None
            ))
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied,
        "fresh owner lifecycle check"
    );
    lookup_pool.close().await;
    assert_eq!(
        issuer_client
            .exchange_actor(identified(
                exchange_request.clone(),
                Workload::Post,
                "post-fixture",
                None
            ))
            .await
            .unwrap_err()
            .code(),
        Code::Unavailable,
        "owner storage unavailable cannot issue"
    );
    assert_eq!(effects.load(Ordering::SeqCst), 2);
    // A valid child authorizes a current read, not a frozen Friend policy.
    sqlx::query(
        "insert into user_mutes(muter_id,muted_id,created_at) values ($1,$2,clock_timestamp())",
    )
    .bind(viewer)
    .bind(Uuid::from_u128(26))
    .execute(&friend_app.pool)
    .await
    .with_context(|| {
        format!(
            "search authority: mute Friend before freshness recheck (pool_size={}, idle={})",
            friend_app.pool.size(),
            friend_app.pool.num_idle()
        )
    })?;
    let friend_channel = post_tls.connect(&friend_url, Workload::Friend).await?;
    let policy_request = friend::BatchRelationshipRequest {
        viewer_id: viewer.to_string(),
        target_ids: vec![Uuid::from_u128(26).to_string()],
    };
    let policies = friend_call(
        friend_channel.clone(),
        policy_request.clone(),
        Some(&later_tokens[0]),
    )
    .await
    .map_err(|code| anyhow::anyhow!("Friend freshness read failed: {code:?}"))?;
    assert!(
        policies.policies[0]
            .policy
            .as_ref()
            .unwrap()
            .muted_by_viewer
    );
    assert!(policies.policies[0].policy.as_ref().unwrap().following);
    assert_eq!(effects.load(Ordering::SeqCst), 3);
    friend_app.pool.close().await;
    assert_eq!(
        friend_call(
            friend_channel.clone(),
            policy_request.clone(),
            Some(&later_tokens[1])
        )
        .await,
        Err(Code::Internal),
        "closed Friend storage cannot synthesize an allowing policy"
    );
    assert_eq!(effects.load(Ordering::SeqCst), 4);
    assert_eq!(
        friend_call(friend_channel, policy_request, Some(&later_tokens[1])).await,
        Err(Code::Unauthenticated),
        "owner storage failure does not restore consumed child proof"
    );
    assert_eq!(effects.load(Ordering::SeqCst), 4);
    post_server.stop().await?;
    gateway_server.stop().await?;
    friend_server.stop().await?;
    friend_app
        .shutdown()
        .await
        .context("search authority: shut down Friend fixture")?;
    println!("COMPOSED_AUTHORITY_MTLS_PG: actual Post search and Friend PG policy handler; independent Gateway issuer/Post lookup; persisted block filters Post; fresh mute observed; unavailable Friend fails closed and consumes proof; denial entries unchanged; all listeners stopped; Friend fixture dropped");
    Ok(())
}

#[derive(Clone)]
struct NoPostResources;
#[async_trait::async_trait]
impl gateway_application::authority_exchange::OwnerResourceAuthorityPort for NoPostResources {
    async fn resolve(
        &self,
        _: &gateway_application::authority_exchange::OwnerResourceLookupDto,
    ) -> Result<
        gateway_application::authority_exchange::OwnerResourceEvidenceDto,
        gateway_application::authority_exchange::AuthorityExchangeErrorDto,
    > {
        panic!("Author existence must not resolve unrelated Post resources")
    }
}

#[tokio::test]
async fn create_post_uses_minimal_exact_author_profile_child_and_real_pg_existence(
) -> anyhow::Result<()> {
    use post_service::adapters::out::grpc::profile_client::GrpcProfileClient;
    use profile_service::adapters::out::persistence::profile_repository::PgProfileRepository;
    use profile_service::adapters::r#in::grpc::author_authority::{
        proto as projection, ProfileAuthorAuthorityGrpcServer, PROFILE_AUTHOR_FORWARDING,
        PROFILE_AUTHOR_POLICY,
    };
    const CREATE: &str = "/post.v1.PostService/CreatePost";
    let app = post_support::start_postgres_redis_app(false).await?;
    let profile = profile_support::start_postgres_app(false).await?;
    let actor = Uuid::new_v4();
    sqlx::query("insert into profiles(user_id,username,nickname,avatar_url,bio,account_visibility) values ($1,$2,'private nickname','private avatar','private biography','PRIVATE')")
        .bind(actor).bind(format!("author_{}", actor.simple())).execute(&profile.pool).await?;
    let post_tls = tls(Workload::Post)?;
    let profile_tls = tls(Workload::Profile)?;
    let gateway_tls = tls(Workload::Gateway)?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let profile_url = format!("https://{}", listener.local_addr()?);
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let profile_server = ServerTask::spawn(Server::builder().tls_config(profile_tls.server_config()?)?
        .add_service(guard_with_forwarding(&PROFILE_AUTHOR_POLICY, PROFILE_AUTHOR_FORWARDING,
            projection::profile_author_authority_service_server::ProfileAuthorAuthorityServiceServer::new(
                ProfileAuthorAuthorityGrpcServer::new(Arc::new(PgProfileRepository::new(profile.pool.clone())))),
            registry()?, ActorDelegationVerifier::local_parity(Workload::Profile))?)
        .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async { let _=stopped.await; }),stop);
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let gateway_url = format!("https://{}", listener.local_addr()?);
    let issuer = DelegationExchange::new(
        ActorDelegationSigner::local_parity(Workload::Gateway),
        ActorDelegationVerifier::local_parity(Workload::Post),
        ResourceEvidenceVerifier::new(&[
            ResourceEvidenceSigner::from_seed(&[11; 32])?.public_key()
        ])?,
    )?;
    let broker = GatewayAuthorizationExchangeGrpcServer::new(Arc::new(
        DelegationExchangeRuntime::new(issuer, Arc::new(NoPostResources)),
    ));
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let gateway_server = ServerTask::spawn(Server::builder().tls_config(gateway_tls.server_config()?)?
        .add_service(guard_without_delegation(&AUTHORIZATION_EXCHANGE_POLICY,
            authority::authorization_exchange_service_server::AuthorizationExchangeServiceServer::new(broker),registry()?)?)
        .serve_with_incoming_shutdown(TcpListenerStream::new(listener),async{let _=stopped.await;}),stop);
    let exchange = GrpcAuthorizationExchangeClient::connect(
        &gateway_url,
        "post-fixture",
        Some(&post_tls),
        EdgeBudget {
            edge: "fixture-post-profile-exchange",
            expected_latency: Duration::from_millis(20),
            deadline: Duration::from_secs(3),
            connect_deadline: Duration::from_secs(1),
            retry: RetryClass::Never,
            failure_policy: FailurePolicy::FailRequest,
        },
    )?;
    let client =
        GrpcProfileClient::connect(profile_url, Workload::Post, "post-fixture", Some(&post_tls))
            .await?
            .with_exchange(Arc::new(exchange));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let post_url = format!("https://{}", listener.local_addr()?);
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let post_server = ServerTask::spawn(
        Server::builder()
            .tls_config(post_tls.server_config()?)?
            .add_service(guard(
                &POST_METHOD_POLICY,
                CreateOwner(app.application_with_profile_author(Arc::new(client))),
                registry()?,
                ActorDelegationVerifier::local_parity(Workload::Post),
            )?)
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = stopped.await;
            }),
        stop,
    );
    let channel = gateway_tls.connect(&post_url, Workload::Post).await?;
    let mut grpc = tonic::client::Grpc::new(channel);
    for mode in 0..4 {
        if mode == 1 {
            sqlx::query("update profiles set deleted_at=clock_timestamp() where user_id=$1")
                .bind(actor)
                .execute(&profile.pool)
                .await?;
        }
        if mode == 3 {
            profile.pool.close().await;
        }
        let author = if mode == 2 { Uuid::new_v4() } else { actor };
        let request = post::CreatePostRequest {
            author_id: author.to_string(),
            content: format!("minimal author existence {mode}"),
            visibility: "PUBLIC".into(),
        };
        let root = ActorDelegationSigner::local_parity(Workload::Gateway).issue(
            Workload::Post,
            CREATE,
            author,
            None,
            &frame(&request),
            chrono::Utc::now().timestamp(),
        );
        grpc.ready().await?;
        let response: Result<Response<post::CreatePostResponse>, Status> = grpc
            .unary(
                identified(request, Workload::Gateway, "gateway-fixture", Some(&root)),
                http::uri::PathAndQuery::from_static(CREATE),
                tonic_prost::ProstCodec::default(),
            )
            .await;
        match mode {
            0 => assert_eq!(
                response?.into_inner().post.unwrap().author_id,
                actor.to_string(),
                "private profile still exists"
            ),
            1 | 2 => assert_eq!(
                response.unwrap_err().code(),
                Code::NotFound,
                "deleted/missing profile does not exist"
            ),
            _ => assert_eq!(
                response.unwrap_err().code(),
                Code::Internal,
                "storage failure cannot synthesize existence"
            ),
        }
        let posts: i64 = sqlx::query_scalar("select count(*) from posts")
            .fetch_one(&app.pool)
            .await?;
        assert_eq!(posts, 1, "failed existence must precede Post transaction");
    }
    post_server.stop().await?;
    gateway_server.stop().await?;
    profile_server.stop().await?;
    profile.pool.close().await;
    println!("MINIMAL_PROFILE_AUTHORITY_PG: actual Post CreatePost; Gateway-issued exact-author child; real Profile PG; private accepted; deleted/missing/storage failure create no Post; all listeners stopped");
    Ok(())
}

#[tokio::test]
async fn cold_feed_uses_friend_owned_membership_then_post_stage_policy_and_rechecks_cache(
) -> anyhow::Result<()> {
    const FEED: &str = "/post.v1.PostService/GetFeed";
    for race in [false, true] {
        let app = post_support::start_postgres_redis_app(false).await?;
        let friend_app = friend_support::start_postgres_app(false).await?;
        let viewer = Uuid::new_v4();
        // More than one 100-row owner page; these followed authors have no Post.
        for author in 1000u128..=1100 {
            sqlx::query("insert into follows(follower_id,followee_id) values($1,$2)")
                .bind(viewer)
                .bind(Uuid::from_u128(author))
                .execute(&friend_app.pool)
                .await?;
        }
        for author in 26u128..=30 {
            sqlx::query("insert into follows(follower_id,followee_id,deleted_at) values($1,$2,case when $3 then clock_timestamp() else null end)")
                .bind(viewer).bind(Uuid::from_u128(author)).bind(author==30).execute(&friend_app.pool).await?;
        }
        for (blocker, blocked) in [(Uuid::from_u128(27), viewer), (viewer, Uuid::from_u128(28))] {
            sqlx::query("insert into user_blocks(blocker_id,blocked_id,created_at) values($1,$2,clock_timestamp())")
                .bind(blocker).bind(blocked).execute(&friend_app.pool).await?;
        }
        sqlx::query(
            "insert into user_mutes(muter_id,muted_id,created_at) values($1,$2,clock_timestamp())",
        )
        .bind(viewer)
        .bind(Uuid::from_u128(29))
        .execute(&friend_app.pool)
        .await?;
        for (id, author, visibility) in [
            (2, Uuid::from_u128(26), "FRIENDS"),
            (4, Uuid::from_u128(27), "PUBLIC"),
            (6, Uuid::from_u128(28), "PUBLIC"),
            (8, Uuid::from_u128(29), "PUBLIC"),
            (10, Uuid::from_u128(30), "PUBLIC"),
            (12, Uuid::from_u128(31), "PUBLIC"),
            (14, viewer, "PRIVATE"),
        ] {
            sqlx::query("insert into posts(id,author_id,content,visibility,created_at) values($1,$2,'cold membership',$3,clock_timestamp())")
                .bind(Uuid::from_u128(id)).bind(author).bind(visibility).execute(&app.pool).await?;
        }
        let post_tls = tls(Workload::Post)?;
        let gateway_tls = tls(Workload::Gateway)?;
        let friend_tls = tls(Workload::Friend)?;
        let effects = Arc::new(AtomicUsize::new(0));
        let friend_listener = TcpListener::bind("127.0.0.1:0").await?;
        let friend_url = format!("https://{}", friend_listener.local_addr()?);
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let friend_server = ServerTask::spawn(
            Server::builder()
                .tls_config(friend_tls.server_config()?)?
                .add_service(guard_with_forwarding(
                    &friend_service::adapters::r#in::grpc::method_policy::FRIEND_METHOD_POLICY,
                    FRIEND_PROOF,
                    FriendOwner {
                        entries: effects.clone(),
                        server: friend_support::grpc_server(friend_app.state.clone()),
                        mute_after_list: race
                            .then(|| (friend_app.pool.clone(), viewer, Uuid::from_u128(26))),
                    },
                    registry()?,
                    ActorDelegationVerifier::local_parity(Workload::Friend),
                )?)
                .serve_with_incoming_shutdown(TcpListenerStream::new(friend_listener), async {
                    let _ = stopped.await;
                }),
            stop,
        );
        let lookup_pool = PgPool::connect_with(app.pool.connect_options().as_ref().clone()).await?;
        let owner_key = ResourceEvidenceSigner::from_seed(&[11; 32])?;
        let resource = PostResourceAuthorityGrpcServer::new(
            Arc::new(PgAuthorityWitnessRepository::new(lookup_pool.clone())),
            ActorDelegationVerifier::local_parity(Workload::Post),
            owner_key.clone(),
        )?;
        let post_listener = TcpListener::bind("127.0.0.1:0").await?;
        let post_url = format!("https://{}", post_listener.local_addr()?);
        let gateway_listener = TcpListener::bind("127.0.0.1:0").await?;
        let gateway_url = format!("https://{}", gateway_listener.local_addr()?);
        let owner_client = GrpcPostResourceAuthorityClient::connect(
            &post_url,
            "gateway-fixture",
            Some(&gateway_tls),
            Default::default(),
        )?;
        let issuer = DelegationExchange::new(
            ActorDelegationSigner::local_parity(Workload::Gateway),
            ActorDelegationVerifier::local_parity(Workload::Post),
            ResourceEvidenceVerifier::new(&[owner_key.public_key()])?,
        )?;
        let broker = GatewayAuthorizationExchangeGrpcServer::new(Arc::new(
            DelegationExchangeRuntime::new(issuer, Arc::new(owner_client)),
        ));
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let gateway_server = ServerTask::spawn(
        Server::builder()
            .tls_config(gateway_tls.server_config()?)?
            .add_service(guard_without_delegation(
                &AUTHORIZATION_EXCHANGE_POLICY,
                authority::authorization_exchange_service_server::AuthorizationExchangeServiceServer::new(broker),
                registry()?,
            )?)
            .serve_with_incoming_shutdown(TcpListenerStream::new(gateway_listener), async {
                let _ = stopped.await;
            }),
        stop,
    );
        // Fixture-only injection. No operational edge default or SLO is introduced.
        let budget = EdgeBudget {
            edge: "fixture-post-exchange",
            expected_latency: Duration::from_millis(20),
            deadline: Duration::from_secs(3),
            connect_deadline: Duration::from_secs(1),
            retry: RetryClass::Never,
            failure_policy: FailurePolicy::FailRequest,
        };
        let exchange = GrpcAuthorizationExchangeClient::connect(
            &gateway_url,
            "post-fixture",
            Some(&post_tls),
            budget,
        )?;
        let friend_client = GrpcFriendClient::connect(
            friend_url.clone(),
            Workload::Post,
            "post-fixture",
            Some(&post_tls),
        )
        .await?
        .with_exchange(Arc::new(exchange));
        let application = app.application_with_query_witnesses(
            Arc::new(AdmittedQueryWitnessProducer::new(Arc::new(
                PgAuthorityWitnessRepository::new(app.pool.clone()),
            ))),
            Arc::new(friend_client),
        );
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let post_server = ServerTask::spawn(
        Server::builder()
            .tls_config(post_tls.server_config()?)?
            .add_service(guard(
                &POST_METHOD_POLICY,
                FeedOwner(application),
                registry()?,
                ActorDelegationVerifier::local_parity(Workload::Post),
            )?)
            .add_service(guard_without_delegation(
                &POST_RESOURCE_AUTHORITY_POLICY,
                owner::post_resource_authority_service_server::PostResourceAuthorityServiceServer::new(resource),
                registry()?,
            )?)
            .serve_with_incoming_shutdown(TcpListenerStream::new(post_listener), async {
                let _ = stopped.await;
            }),
        stop,
    );

        let channel = gateway_tls.connect(&post_url, Workload::Post).await?;
        let request = post::FeedRequest {
            user_id: viewer.to_string(),
            cursor: String::new(),
            limit: 10,
            request_id: String::new(),
            session_id: String::new(),
        };
        let mut grpc = tonic::client::Grpc::new(channel);
        grpc.ready().await?;
        let denied: Result<Response<SearchResult>, Status> = grpc
            .unary(
                identified(request.clone(), Workload::Gateway, "gateway-fixture", None),
                http::uri::PathAndQuery::from_static(FEED),
                tonic_prost::ProstCodec::default(),
            )
            .await;
        assert_eq!(denied.unwrap_err().code(), Code::Unauthenticated);
        assert_eq!(effects.load(Ordering::SeqCst), 0);
        for warm in [false, true] {
            if warm {
                sqlx::query("insert into user_mutes(muter_id,muted_id,created_at) values($1,$2,clock_timestamp()) on conflict(muter_id,muted_id) do update set deleted_at=null")
                    .bind(viewer).bind(Uuid::from_u128(26)).execute(&friend_app.pool).await?;
            }
            let root = ActorDelegationSigner::local_parity(Workload::Gateway).issue(
                Workload::Post,
                FEED,
                viewer,
                None,
                &frame(&request),
                chrono::Utc::now().timestamp(),
            );
            grpc.ready().await?;
            let response: Response<SearchResult> = grpc
                .unary(
                    identified(
                        request.clone(),
                        Workload::Gateway,
                        "gateway-fixture",
                        Some(&root),
                    ),
                    http::uri::PathAndQuery::from_static(FEED),
                    tonic_prost::ProstCodec::default(),
                )
                .await?;
            let mut ids = response.into_inner().post_ids;
            ids.sort();
            let mut expected = vec![Uuid::from_u128(14).to_string()];
            if !race && !warm {
                expected.push(Uuid::from_u128(2).to_string());
            }
            expected.sort();
            assert_eq!(
                ids, expected,
                "owner-filtered membership/current policy race={race} warm={warm}"
            );
            assert_eq!(
                effects.load(Ordering::SeqCst),
                if warm && !race { 4 } else { 3 },
                "cold ListFollowing + witnessed policy; warm only witnessed policy"
            );
        }
        let rows = sqlx::query("select post_ids from post_authority_resource_witnesses")
            .fetch_all(&app.pool)
            .await?;
        assert_eq!(
            rows.len(),
            2,
            "one separate stage per admitted cold/warm query"
        );
        for row in rows {
            let ids: Vec<Uuid> = row.try_get("post_ids")?;
            assert!(
                ids.iter()
                    .all(|id| [Uuid::from_u128(2), Uuid::from_u128(14)].contains(id)),
                "ineligible/non-followed posts cannot enter the query stage"
            );
        }
        let before_unavailable = effects.load(Ordering::SeqCst);
        friend_app.pool.close().await;
        let unavailable = post::FeedRequest {
            user_id: Uuid::new_v4().to_string(),
            ..request.clone()
        };
        let actor = Uuid::parse_str(&unavailable.user_id)?;
        let root = ActorDelegationSigner::local_parity(Workload::Gateway).issue(
            Workload::Post,
            FEED,
            actor,
            None,
            &frame(&unavailable),
            chrono::Utc::now().timestamp(),
        );
        grpc.ready().await?;
        let unavailable: Result<Response<SearchResult>, Status> = grpc
            .unary(
                identified(
                    unavailable,
                    Workload::Gateway,
                    "gateway-fixture",
                    Some(&root),
                ),
                http::uri::PathAndQuery::from_static(FEED),
                tonic_prost::ProstCodec::default(),
            )
            .await;
        assert_eq!(unavailable.unwrap_err().code(), Code::Unavailable);
        assert_eq!(
            effects.load(Ordering::SeqCst),
            before_unavailable + 1,
            "one failing owner ListFollowing, no target policy call"
        );
        let stages: i64 =
            sqlx::query_scalar("select count(*) from post_authority_resource_witnesses")
                .fetch_one(&app.pool)
                .await?;
        assert_eq!(stages, 2, "owner failure cannot publish a Post observation");
        lookup_pool.close().await;
        post_server.stop().await?;
        gateway_server.stop().await?;
        friend_server.stop().await?;
        friend_app.shutdown().await?;
        println!("COLD_FEED_AUTHORITY_PG: actual Friend filtered membership; actual Post cold/warm feed; exact Post stage; latest policy retained; race={race}; all listeners stopped");
    }
    Ok(())
}

#[path = "forwarded_authority_postgres/claimed_cache.rs"]
mod claimed_cache;
#[path = "forwarded_authority_postgres/claimed_fanout.rs"]
mod claimed_fanout;
#[path = "forwarded_authority_postgres/ranked_snapshot.rs"]
mod ranked_snapshot;

#[path = "forwarded_authority_postgres/claimed_notification.rs"]
mod claimed_notification;
#[path = "../../notification-service/tests/support/mod.rs"]
mod notification_support;
#[path = "forwarded_authority_postgres/profile_author.rs"]
mod profile_author;
#[path = "forwarded_authority_postgres/resource_lookup.rs"]
mod resource_lookup;

#[tokio::test(start_paused = true)]
async fn timed_out_server_shutdown_aborts_and_awaits_the_owned_task() {
    struct Dropped(Arc<std::sync::atomic::AtomicBool>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let captured = dropped.clone();
    let (stop, _stopped) = tokio::sync::oneshot::channel();
    let (started, ready) = tokio::sync::oneshot::channel();
    let server = ServerTask::spawn(
        async move {
            let _on_drop = Dropped(captured);
            let _ = started.send(());
            std::future::pending::<()>().await;
            Ok(())
        },
        stop,
    );
    ready.await.unwrap();
    assert!(server
        .stop_with_timeout(Duration::from_millis(10))
        .await
        .is_err());
    assert!(
        dropped.load(Ordering::SeqCst),
        "timeout detached a live server task"
    );
}
