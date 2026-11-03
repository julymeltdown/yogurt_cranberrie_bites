//! Verify W3C trace continuation over an actual loopback HTTP and gRPC socket.

use platform::grpc_client_trace::GrpcClientTraceService;
use platform::trace_context::{
    self, TraceContext, TraceContextLayer, TRACEPARENT_HEADER, TRACESTATE_HEADER,
};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::{Endpoint, Server};
use tonic_health::pb::{health_client::HealthClient, HealthCheckRequest};
use tower::{Layer, ServiceExt};

struct ServerObservation {
    traceparent: String,
    tracestate: Option<String>,
    context: Option<TraceContext>,
}

#[tokio::test]
async fn inbound_http_context_reaches_a_live_grpc_server_as_a_child_span() -> anyhow::Result<()> {
    const INBOUND_TRACEPARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    const INBOUND_TRACESTATE: &str = "vendor=opaque";

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address: SocketAddr = listener.local_addr()?;
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let (observation_tx, mut observation_rx) = tokio::sync::mpsc::unbounded_channel();

    let (_reporter, health_service) = tonic_health::server::health_reporter();
    let health_service =
        InterceptedService::new(health_service, move |request: tonic::Request<()>| {
            let traceparent = request
                .metadata()
                .get(TRACEPARENT_HEADER)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| tonic::Status::invalid_argument("missing traceparent"))?
                .to_owned();
            let tracestate = request
                .metadata()
                .get(TRACESTATE_HEADER)
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned);
            observation_tx
                .send(ServerObservation {
                    traceparent,
                    tracestate,
                    context: trace_context::current(),
                })
                .map_err(|_| tonic::Status::internal("trace observation receiver closed"))?;
            Ok(request)
        });

    let server = tokio::spawn(async move {
        Server::builder()
            .layer(TraceContextLayer)
            .add_service(health_service)
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = shutdown_rx.await;
            })
            .await
    });

    let endpoint = Endpoint::from_shared(format!("http://{address}"))?;
    let channel = endpoint.connect().await?;
    let (inbound_context_tx, mut inbound_context_rx) = tokio::sync::mpsc::unbounded_channel();
    let inbound = TraceContextLayer.layer(tower::service_fn(move |_request: http::Request<()>| {
        let channel = channel.clone();
        let inbound_context_tx = inbound_context_tx.clone();
        async move {
            inbound_context_tx
                .send(trace_context::current())
                .expect("HTTP trace context receiver is open");
            let mut client = HealthClient::new(GrpcClientTraceService::new(channel));
            let response = client
                .check(tonic::Request::new(HealthCheckRequest {
                    service: String::new(),
                }))
                .await
                .expect("live gRPC health call succeeds");
            Ok::<_, std::convert::Infallible>(http::Response::new(response.into_inner()))
        }
    }));
    let request = http::Request::builder()
        .method("GET")
        .uri("/trace-probe")
        .header(TRACEPARENT_HEADER, INBOUND_TRACEPARENT)
        .header(TRACESTATE_HEADER, INBOUND_TRACESTATE)
        .body(())?;
    let response =
        tokio::time::timeout(std::time::Duration::from_secs(3), inbound.oneshot(request)).await??;
    assert_eq!(response.body().status, 1, "health service reports serving");

    let observation =
        tokio::time::timeout(std::time::Duration::from_secs(1), observation_rx.recv())
            .await?
            .expect("server interceptor observes request metadata");
    let inbound_context = inbound_context_rx
        .recv()
        .await
        .expect("HTTP trace layer reports the request context")
        .expect("HTTP trace layer establishes a request context");
    let downstream_context = observation
        .context
        .expect("server trace layer establishes the downstream context");
    let client_span_id = &observation.traceparent[36..52];
    assert_eq!(&observation.traceparent[3..35], inbound_context.trace_id());
    assert_ne!(client_span_id, inbound_context.span_id());
    assert_eq!(downstream_context.parent_span_id(), Some(client_span_id));
    assert_eq!(downstream_context.trace_id(), inbound_context.trace_id());
    assert!(downstream_context.sampled());
    assert_eq!(observation.tracestate.as_deref(), Some(INBOUND_TRACESTATE));

    let _ = shutdown_tx.send(());
    server.await??;
    Ok(())
}
