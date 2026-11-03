//! Adapter-level retry regressions for Post's exchange-authorized Profile
//! read, over real loopback gRPC with a signed root admitted by the real
//! method guard. No database. The fake Gateway exchange and Profile author
//! authority count every call they receive.
use chrono::Utc;
use http_body_util::Full;
use platform::actor_delegation::{
    exchange::VerifiedParentAuthority, ActorDelegationSigner, ActorDelegationVerifier, HEADER,
};
use platform::grpc_auth::{CallerRegistry, Workload};
use post_service::adapters::out::grpc::authority_exchange_client::GrpcAuthorizationExchangeClient;
use post_service::adapters::out::grpc::profile_client::GrpcProfileClient;
use post_service::adapters::r#in::grpc::method_policy::POST_METHOD_POLICY;
use post_service::adapters::r#in::grpc::resource_authority::proto as authority;
use post_service::adapters::r#in::grpc::server::proto;
use post_service::application::ports::ProfileClientPort;
use prost::Message;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tonic::body::Body;
use tonic::codegen::tokio_stream::Stream;
use tonic::server::NamedService;
use tonic::{Code, Request, Response, Status};
use tower::ServiceExt;
use uuid::Uuid;

const GATEWAY_TOKEN: &str = "read-retry-gateway";

/// What the fakes answer, and how often they were called.
#[derive(Default)]
struct Plan {
    exchange_error: Mutex<Option<Code>>,
    profile_failures: Mutex<Vec<Code>>,
    exchange_calls: AtomicUsize,
    profile_calls: AtomicUsize,
}

struct FakeExchange(Arc<Plan>);

#[tonic::async_trait]
impl authority::authorization_exchange_service_server::AuthorizationExchangeService
    for FakeExchange
{
    async fn exchange_actor(
        &self,
        _request: Request<authority::ExchangeActorRequest>,
    ) -> Result<Response<authority::ExchangeActorResponse>, Status> {
        self.0.exchange_calls.fetch_add(1, Ordering::SeqCst);
        if let Some(code) = *self.0.exchange_error.lock().unwrap() {
            return Err(Status::new(code, "exchange fixture"));
        }
        Ok(Response::new(authority::ExchangeActorResponse {
            token: "child-authority-fixture".into(),
            expires_at: Utc::now().timestamp() + 5,
        }))
    }
}

struct FakeAuthor(Arc<Plan>);

#[tonic::async_trait]
impl authority::profile_author_authority_service_server::ProfileAuthorAuthorityService
    for FakeAuthor
{
    async fn check_author_profile(
        &self,
        request: Request<authority::CheckAuthorProfileRequest>,
    ) -> Result<Response<authority::CheckAuthorProfileResponse>, Status> {
        self.0.profile_calls.fetch_add(1, Ordering::SeqCst);
        let next = {
            let mut failures = self.0.profile_failures.lock().unwrap();
            (!failures.is_empty()).then(|| failures.remove(0))
        };
        if let Some(code) = next {
            return Err(Status::new(code, "author fixture"));
        }
        Ok(Response::new(authority::CheckAuthorProfileResponse {
            user_id: request.into_inner().user_id,
        }))
    }
}

struct Incoming(tokio::net::TcpListener);

impl Stream for Incoming {
    type Item = Result<tokio::net::TcpStream, std::io::Error>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.0
            .poll_accept(cx)
            .map(|accepted| Some(accepted.map(|(stream, _)| stream)))
    }
}

/// The admitted handler: it runs the adapter inside the guard's parent scope.
#[derive(Clone)]
struct AuthorCheck {
    client: Arc<GrpcProfileClient>,
    author: Uuid,
    outcome: Arc<Mutex<Option<bool>>>,
}

impl NamedService for AuthorCheck {
    const NAME: &'static str = "post.v1.PostService";
}

impl tower::Service<http::Request<Body>> for AuthorCheck {
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Infallible>> + Send>>;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, _: http::Request<Body>) -> Self::Future {
        let this = self.clone();
        Box::pin(async move {
            assert!(
                VerifiedParentAuthority::current().is_some(),
                "guard scoped a parent"
            );
            let ok = this.client.require_author(this.author).await.is_ok();
            *this.outcome.lock().unwrap() = Some(ok);
            let mut response = http::Response::new(Body::default());
            response
                .headers_mut()
                .insert("grpc-status", "0".parse().unwrap());
            Ok(response)
        })
    }
}

fn root_request(viewer: Uuid) -> http::Request<Body> {
    let method = "/post.v1.PostService/SearchPosts";
    let message = proto::SearchPostsRequest {
        query: "retry".into(),
        cursor: String::new(),
        limit: 1,
        viewer_id: viewer.to_string(),
    }
    .encode_to_vec();
    let bytes = [
        vec![0],
        (message.len() as u32).to_be_bytes().to_vec(),
        message,
    ]
    .concat();
    let token = ActorDelegationSigner::local_parity(Workload::Gateway).issue(
        Workload::Post,
        method,
        viewer,
        None,
        &bytes,
        Utc::now().timestamp(),
    );
    http::Request::builder()
        .uri(method)
        .header("content-type", "application/grpc")
        .header("x-service-caller", "gateway")
        .header("x-service-token", GATEWAY_TOKEN)
        .header(HEADER, token)
        .body(Body::new(Full::new(tonic::codegen::Bytes::from(bytes))))
        .unwrap()
}

async fn check(client: &Arc<GrpcProfileClient>) -> bool {
    let outcome = Arc::new(Mutex::new(None));
    let service = platform::grpc_method_policy::guard(
        &POST_METHOD_POLICY,
        AuthorCheck {
            client: client.clone(),
            author: Uuid::new_v4(),
            outcome: outcome.clone(),
        },
        CallerRegistry::new()
            .register(Workload::Gateway, GATEWAY_TOKEN)
            .unwrap(),
        ActorDelegationVerifier::local_parity(Workload::Post),
    )
    .unwrap();
    let response = service.oneshot(root_request(Uuid::new_v4())).await.unwrap();
    assert_eq!(response.headers()["grpc-status"], "0", "root admitted");
    let result = outcome.lock().unwrap().expect("handler ran");
    result
}

fn reset(plan: &Plan, exchange_error: Option<Code>, profile_failures: Vec<Code>) {
    *plan.exchange_error.lock().unwrap() = exchange_error;
    *plan.profile_failures.lock().unwrap() = profile_failures;
    plan.exchange_calls.store(0, Ordering::SeqCst);
    plan.profile_calls.store(0, Ordering::SeqCst);
}

fn calls(plan: &Plan) -> (usize, usize) {
    (
        plan.exchange_calls.load(Ordering::SeqCst),
        plan.profile_calls.load(Ordering::SeqCst),
    )
}

#[tokio::test]
async fn exchange_failures_are_never_retried_and_owner_outages_retry_with_a_fresh_child() {
    let plan = Arc::new(Plan::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(
        tonic::transport::Server::builder()
            .add_service(
                authority::authorization_exchange_service_server::AuthorizationExchangeServiceServer::new(
                    FakeExchange(plan.clone()),
                ),
            )
            .add_service(
                authority::profile_author_authority_service_server::ProfileAuthorAuthorityServiceServer::new(
                    FakeAuthor(plan.clone()),
                ),
            )
            .serve_with_incoming_shutdown(Incoming(listener), async {
                let _ = stopped.await;
            }),
    );
    let exchange = GrpcAuthorizationExchangeClient::connect(
        &endpoint,
        "post-exchange-fixture",
        None,
        platform::edge::Edge::PostGateway.budget(),
    )
    .unwrap();
    let client = Arc::new(
        GrpcProfileClient::connect(endpoint, Workload::Post, "post-profile-fixture", None)
            .await
            .unwrap()
            .with_exchange(Arc::new(exchange)),
    );

    // Exchange failures end the read after one exchange call, whatever their
    // status, and the owner is never reached.
    for code in [
        Code::ResourceExhausted,
        Code::DeadlineExceeded,
        Code::Internal,
        Code::Unavailable,
    ] {
        reset(&plan, Some(code), Vec::new());
        assert!(!check(&client).await, "{code:?}");
        assert_eq!(calls(&plan), (1, 0), "{code:?}");
    }

    // One owner outage is retried with a second, fresh exchange.
    reset(&plan, None, vec![Code::Unavailable]);
    assert!(check(&client).await);
    assert_eq!(calls(&plan), (2, 2));

    // Any other owner status ends the read after one attempt.
    reset(&plan, None, vec![Code::Internal]);
    assert!(!check(&client).await);
    assert_eq!(calls(&plan), (1, 1));

    let _ = stop.send(());
    server.await.unwrap().unwrap();
}
