//! Loopback transport double proves HTTP-body completion ordering, not real
//! canonical-owner/mTLS or producer acceptance. Capabilities never enter JSON.
use api_gateway::adapters::out::concert_product::{proto, GrpcConcertProductClient};
use axum::{body::Body, http::Request, response::Response, routing::get, Router};
use http_body_util::BodyExt;
use platform::{
    actor_delegation::{response_scope::RetainedScopeLayer, ActorDelegationSigner},
    grpc_auth::{CallerRegistry, RequireRegisteredCallerInterceptor, Workload},
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Request as RpcRequest, Response as RpcResponse, Status};
use tower::ServiceExt;
use uuid::Uuid;

struct Completion {
    calls: Arc<AtomicUsize>,
    fail: bool,
}
#[tonic::async_trait]
impl proto::product_delivery_service_server::ProductDeliveryService for Completion {
    async fn complete_response(
        &self,
        request: RpcRequest<proto::ResponseDelivery>,
    ) -> Result<RpcResponse<proto::CompleteResponseResult>, Status> {
        assert_eq!(request.get_ref().completion_secret, [7; 32]);
        assert!(request
            .metadata()
            .get(platform::actor_delegation::HEADER)
            .is_none());
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err(Status::unavailable("Synthetic unresolved settlement"));
        }
        Ok(RpcResponse::new(proto::CompleteResponseResult {
            completed: true,
        }))
    }
}
fn capability() -> proto::ResponseDelivery {
    proto::ResponseDelivery {
        instance_id: Uuid::new_v4().to_string(),
        scope_id: Uuid::new_v4().to_string(),
        binding_digest: vec![6; 32],
        completion_secret: vec![7; 32],
    }
}
async fn fixture(
    fail: bool,
) -> (
    GrpcConcertProductClient,
    Arc<AtomicUsize>,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    let service =
        proto::product_delivery_service_server::ProductDeliveryServiceServer::with_interceptor(
            Completion {
                calls: calls.clone(),
                fail,
            },
            RequireRegisteredCallerInterceptor::new(
                CallerRegistry::new()
                    .register(Workload::Gateway, "delivery-transport-token")
                    .unwrap(),
            )
            .unwrap(),
        );
    let (shutdown, stop) = oneshot::channel();
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(service)
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = stop.await;
            })
            .await
            .unwrap();
    });
    let client = GrpcConcertProductClient::connect(
        &endpoint,
        "delivery-transport-token",
        None,
        ActorDelegationSigner::from_seed(Workload::Gateway, &[8; 32]).unwrap(),
    )
    .unwrap();
    (client, calls, shutdown, task)
}
fn router(client: &GrpcConcertProductClient) -> Router {
    let token = client.delivery_token(capability()).unwrap();
    Router::new()
        .route(
            "/representation",
            get(move || {
                let token = token.clone();
                async move {
                    let mut response = Response::new(Body::from("{\"value\":1}"));
                    response.extensions_mut().insert(token);
                    response
                }
            }),
        )
        .layer(RetainedScopeLayer)
}

#[tokio::test]
async fn completion_occurs_only_after_actual_http_body_eof_with_fresh_system_budget() {
    let (client, calls, shutdown, task) = fixture(false).await;
    let response = router(&client)
        .oneshot(
            Request::builder()
                .uri("/representation")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let mut body = response.into_body();
    let data = body.frame().await.unwrap().unwrap().into_data().unwrap();
    assert_eq!(data.as_ref(), b"{\"value\":1}");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    // Expired browser/content scope must not become renewed user authority.
    // Completion is a separate system request with no actor delegation.
    let end = platform::call_deadline::scope(
        std::time::Instant::now() - std::time::Duration::from_secs(1),
        body.frame(),
    )
    .await;
    assert!(end.is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let _ = shutdown.send(());
    task.await.unwrap();
}
#[tokio::test]
async fn dropping_http_body_before_eof_never_reports_completion() {
    let (client, calls, shutdown, task) = fixture(false).await;
    let response = router(&client)
        .oneshot(
            Request::builder()
                .uri("/representation")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let mut body = response.into_body();
    assert!(body.frame().await.unwrap().unwrap().is_data());
    drop(body);
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let _ = shutdown.send(());
    task.await.unwrap();
}
#[tokio::test]
async fn unresolved_completion_errors_body_and_does_not_retry() {
    let (client, calls, shutdown, task) = fixture(true).await;
    let response = router(&client)
        .oneshot(
            Request::builder()
                .uri("/representation")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let mut body = response.into_body();
    assert!(body.frame().await.unwrap().unwrap().is_data());
    assert!(body.frame().await.unwrap().is_err());
    assert!(body.frame().await.is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let _ = shutdown.send(());
    task.await.unwrap();
}
#[tokio::test]
async fn malformed_delivery_capability_cannot_install_a_completion_token() {
    let (client, calls, shutdown, task) = fixture(false).await;
    let mut wrong = capability();
    wrong.scope_id = Uuid::nil().to_string();
    assert!(client.delivery_token(wrong).is_err());
    let mut wrong = capability();
    wrong.binding_digest.fill(0);
    assert!(client.delivery_token(wrong).is_err());
    let mut wrong = capability();
    wrong.completion_secret.pop();
    assert!(client.delivery_token(wrong).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let _ = shutdown.send(());
    task.await.unwrap();
}
