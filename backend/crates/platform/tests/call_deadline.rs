//! The caller's remaining time crosses service hops without cancelling the
//! receiver.
use platform::call_deadline::{
    parse_grpc_timeout, CallDeadlineLayer, GRPC_TIMEOUT_HEADER, REMAINING_HEADER,
};
use platform::grpc_client_trace::GrpcClientTraceService;
use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio_stream::wrappers::TcpListenerStream;
use tonic_health::pb::health_client::HealthClient;
use tonic_health::pb::health_server::{Health, HealthServer};
use tonic_health::pb::{HealthCheckRequest, HealthCheckResponse};
use tower::{Layer, Service, ServiceExt};

/// Remaining-time and grpc-timeout headers seen by the next hop.
type SeenHeaders = Arc<Mutex<Vec<(Option<String>, Option<String>)>>>;

#[tokio::test]
async fn an_inbound_deadline_reaches_the_next_hop_as_remaining_time() {
    let seen: SeenHeaders = Arc::default();
    let recorded = seen.clone();
    let downstream = tower::service_fn(move |request: http::Request<tonic::body::Body>| {
        let header = |name| {
            request
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        };
        recorded
            .lock()
            .unwrap()
            .push((header(REMAINING_HEADER), header(GRPC_TIMEOUT_HEADER)));
        async { Ok::<_, Infallible>(http::Response::new(tonic::body::Body::empty())) }
    });
    let client = GrpcClientTraceService::new(downstream);
    let mut owner =
        CallDeadlineLayer.layer(tower::service_fn(move |_request: http::Request<()>| {
            let mut client = client.clone();
            async move {
                // Work done before the next hop consumes part of the caller's time.
                tokio::time::sleep(Duration::from_millis(100)).await;
                let mut outbound = http::Request::new(tonic::body::Body::empty());
                *outbound.uri_mut() = "/friend.v1.FriendService/ListFollowing".parse().unwrap();
                client.ready().await?.call(outbound).await.map(|_| ())
            }
        }));

    let inbound = http::Request::builder()
        .header(REMAINING_HEADER, "400m")
        .body(())
        .unwrap();
    owner.ready().await.unwrap().call(inbound).await.unwrap();
    owner
        .ready()
        .await
        .unwrap()
        .call(http::Request::new(()))
        .await
        .unwrap();

    let seen = seen.lock().unwrap().clone();
    let forwarded = parse_grpc_timeout(seen[0].0.as_deref().expect("propagated deadline")).unwrap();
    assert!(
        forwarded <= Duration::from_millis(300) && forwarded > Duration::from_millis(150),
        "next hop received {forwarded:?} of a 400 ms budget after 100 ms of work"
    );
    assert_eq!(
        seen[0].1, None,
        "propagation never adds an enforced grpc-timeout"
    );
    assert_eq!(
        seen[1],
        (None, None),
        "a call without a caller deadline adds none"
    );
}

#[derive(Clone, Default)]
struct SlowOwner {
    observed: Arc<Mutex<Option<Duration>>>,
    finished: Arc<AtomicBool>,
}

#[tonic::async_trait]
impl Health for SlowOwner {
    async fn check(
        &self,
        _request: tonic::Request<HealthCheckRequest>,
    ) -> Result<tonic::Response<HealthCheckResponse>, tonic::Status> {
        *self.observed.lock().unwrap() = platform::call_deadline::remaining();
        // Finishing durable work outlasts the caller's remaining time.
        tokio::time::sleep(Duration::from_millis(400)).await;
        self.finished.store(true, Ordering::SeqCst);
        Ok(tonic::Response::new(HealthCheckResponse { status: 1 }))
    }

    type WatchStream = std::pin::Pin<
        Box<dyn tokio_stream::Stream<Item = Result<HealthCheckResponse, tonic::Status>> + Send>,
    >;

    async fn watch(
        &self,
        _request: tonic::Request<HealthCheckRequest>,
    ) -> Result<tonic::Response<Self::WatchStream>, tonic::Status> {
        Err(tonic::Status::unimplemented("not used"))
    }
}

async fn serve(owner: SlowOwner) -> (String, tokio::sync::oneshot::Sender<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(
        tonic::transport::Server::builder()
            .layer(CallDeadlineLayer)
            .add_service(HealthServer::new(owner))
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = stopped.await;
            }),
    );
    (address, stop)
}

#[tokio::test]
async fn real_transport_propagation_bounds_the_receiver_without_cancelling_it() {
    let owner = SlowOwner::default();
    let (address, stop) = serve(owner.clone()).await;
    let channel = tonic::transport::Endpoint::from_shared(address)
        .unwrap()
        .connect()
        .await
        .unwrap();
    let mut client = HealthClient::new(GrpcClientTraceService::new(channel));

    let started = Instant::now();
    let response = platform::call_deadline::scope(started + Duration::from_millis(150), async {
        client
            .check(HealthCheckRequest {
                service: String::new(),
            })
            .await
    })
    .await
    .expect("propagated time must not make the receiver's transport cancel it");
    assert_eq!(response.get_ref().status, 1);
    assert!(owner.finished.load(Ordering::SeqCst));
    let observed = owner
        .observed
        .lock()
        .unwrap()
        .expect("receiver saw a deadline");
    assert!(
        observed <= Duration::from_millis(150) && observed > Duration::from_millis(50),
        "receiver saw {observed:?} of a 150 ms budget"
    );
    assert!(started.elapsed() >= Duration::from_millis(400));
    let _ = stop.send(());
}

#[tokio::test]
async fn an_explicit_grpc_timeout_is_still_enforced_by_the_receiver() {
    // This is why propagation uses its own header: Tonic enforces grpc-timeout
    // on the server and drops the handler when it expires.
    let owner = SlowOwner::default();
    let (address, stop) = serve(owner.clone()).await;
    let channel = tonic::transport::Endpoint::from_shared(address)
        .unwrap()
        .connect()
        .await
        .unwrap();
    let mut client = HealthClient::new(GrpcClientTraceService::new(channel));
    let mut request = tonic::Request::new(HealthCheckRequest {
        service: String::new(),
    });
    request.set_timeout(Duration::from_millis(150));
    let status = client.check(request).await.expect_err("enforced timeout");
    assert_eq!(status.code(), tonic::Code::Cancelled, "{status:?}");
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        !owner.finished.load(Ordering::SeqCst),
        "an enforced grpc-timeout drops the receiver's handler"
    );
    let _ = stop.send(());
}
