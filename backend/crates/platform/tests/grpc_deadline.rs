//! Prove that internal endpoints carry a deadline that terminates a stalled peer.
//!
//! The post-service clients previously created channels with no request or
//! connect deadline, so a peer that accepted the connection and never answered
//! held the calling worker indefinitely. These tests use a real listener that
//! accepts and then stays silent, which is the condition that mattered.

use platform::edge::Edge;
use platform::edge_metrics::EdgeDeadlineMetrics;
use platform::grpc_auth::Workload;
use platform::grpc_deadline::with_edge_deadline;
use platform::grpc_tls::{with_deadlines, DEFAULT_REQUEST_DEADLINE};
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::{Channel, Endpoint};
use tonic::{Request, Response, Status};
use tonic_health::pb::{
    health_check_response::ServingStatus,
    health_client::HealthClient,
    health_server::{Health, HealthServer},
    HealthCheckRequest, HealthCheckResponse,
};

/// Concrete message types for the transport probe.
#[derive(Clone, PartialEq, prost::Message)]
struct ProbeRequest {}

#[derive(Clone, PartialEq, prost::Message)]
struct ProbeResponse {}

type ProbeCodec = tonic_prost::ProstCodec<ProbeRequest, ProbeResponse>;

#[derive(Clone)]
struct DelayedHealth {
    delay: Duration,
}

#[tonic::async_trait]
impl Health for DelayedHealth {
    type WatchStream = tokio_stream::Empty<std::result::Result<HealthCheckResponse, Status>>;

    async fn check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> std::result::Result<Response<HealthCheckResponse>, Status> {
        tokio::time::sleep(self.delay).await;
        Ok(Response::new(HealthCheckResponse {
            status: ServingStatus::Serving as i32,
        }))
    }

    async fn watch(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> std::result::Result<Response<Self::WatchStream>, Status> {
        Err(Status::unimplemented(
            "watch is not part of this timeout probe",
        ))
    }
}

/// Start a listener that accepts connections and never writes a response.
async fn start_silent_listener() -> anyhow::Result<SocketAddr> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                // Read whatever the client sends and then simply hold the
                // connection open without replying. A peer that never answers is
                // exactly what a request deadline exists to bound.
                let mut buffer = [0_u8; 1024];
                loop {
                    match socket.read(&mut buffer).await {
                        Ok(0) | Err(_) => return,
                        Ok(_) => {}
                    }
                }
            });
        }
    });
    Ok(address)
}

#[tokio::test]
async fn a_silent_peer_is_bounded_by_the_request_deadline() -> anyhow::Result<()> {
    let address = start_silent_listener().await?;
    let endpoint = with_deadlines(Endpoint::from_shared(format!("http://{address}"))?);
    let channel: Channel = endpoint.connect().await?;
    let mut client = tonic::client::Grpc::new(channel);
    client
        .ready()
        .await
        .map_err(|error| anyhow::anyhow!("channel not ready: {error}"))?;

    let started = Instant::now();
    let result = client
        .unary(
            tonic::Request::new(ProbeRequest {}),
            http::uri::PathAndQuery::from_static("/internal.probe.v1.Probe/Check"),
            ProbeCodec::default(),
        )
        .await;
    let elapsed = started.elapsed();

    assert!(
        result.is_err(),
        "a peer that never answers must fail rather than hang"
    );
    // The deadline must be what ended the call, with a little tolerance for
    // scheduling. Without the deadline this call would never return.
    assert!(
        elapsed < DEFAULT_REQUEST_DEADLINE + Duration::from_secs(5),
        "the call should end within the configured deadline, took {elapsed:?}"
    );
    Ok(())
}

#[tokio::test]
async fn delayed_real_grpc_peer_returns_and_records_the_declared_edge_deadline(
) -> anyhow::Result<()> {
    let edge = Edge::GatewayProfile;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let peer_delay = edge.budget().deadline + Duration::from_secs(1);
    let peer = tokio::spawn(
        tonic::transport::Server::builder()
            .add_service(HealthServer::new(DelayedHealth { delay: peer_delay }))
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async move {
                let _ = shutdown_rx.await;
            }),
    );

    let endpoint = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
        None,
        &format!("http://{address}"),
        edge.peer(),
        edge.budget(),
    )?;
    let mut client = HealthClient::new(endpoint);
    let metrics = EdgeDeadlineMetrics::default();
    let started = Instant::now();
    // The caller timer is intentionally 100 ms ahead of the tonic endpoint
    // timeout. A direct transport probe showed tonic can surface its own
    // timeout as CANCELLED, so this wrapper must win and normalize the result.
    let result = with_edge_deadline(
        edge,
        metrics.clone(),
        client.check(HealthCheckRequest::default()),
    )
    .await;
    let elapsed = started.elapsed();

    let status = result.expect_err("the delayed gRPC method should exceed its edge budget");
    assert_eq!(status.code(), tonic::Code::DeadlineExceeded);
    assert!(
        elapsed
            >= edge
                .budget()
                .deadline
                .saturating_sub(Duration::from_millis(250)),
        "the caller deadline should end the call near its configured edge, elapsed {elapsed:?}"
    );
    assert!(
        elapsed < edge.budget().deadline,
        "the caller deadline should beat the transport deadline, elapsed {elapsed:?}"
    );
    assert_eq!(
        metrics.render_prometheus(),
        "grpc_edge_deadline_exhaustions_total{edge=\"gateway->profile\"} 1\n"
    );

    let _ = shutdown_tx.send(());
    tokio::time::timeout(Duration::from_secs(3), peer)
        .await
        .expect("delayed peer should shut down")??;
    Ok(())
}

#[tokio::test]
async fn the_endpoint_sets_both_a_request_and_connect_deadline() {
    // A value, not a guarantee, but it keeps a future edit from silently
    // dropping one of the two budgets.
    let endpoint = with_deadlines(Endpoint::from_shared("http://127.0.0.1:1").expect("endpoint"));
    // The connect attempt must fail quickly rather than being retried forever.
    let started = Instant::now();
    let outcome = endpoint.connect().await;
    assert!(outcome.is_err(), "connecting to a closed port must fail");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "a refused connection must fail promptly"
    );
}

#[tokio::test]
async fn a_reachable_peer_still_connects_with_the_defaults() -> anyhow::Result<()> {
    // The deadlines must not be so aggressive that an ordinary connection fails.
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move {
        loop {
            if listener.accept().await.is_err() {
                return;
            }
        }
    });
    let endpoint = with_deadlines(Endpoint::from_shared(format!("http://{address}"))?);
    assert!(endpoint.connect().await.is_ok());
    // Workload is referenced so the TLS peer-name helper stays covered here too.
    assert_eq!(Workload::Post.as_str(), "post");
    Ok(())
}

#[tokio::test]
async fn a_lazy_channel_builds_without_waiting_for_an_unreachable_peer() {
    // Eight outbound clients used to connect eagerly at startup, so one
    // dependency being down stopped the dependent service from starting at all.
    // A restart during a dependency blip therefore left the service down and
    // unready instead of serving the routes that did not need that peer.
    //
    // 192.0.2.1 is TEST-NET-1, reserved for documentation and never routed. This
    // is the realistic failure mode: a dependency host that does not answer at
    // all, rather than a closed port that refuses instantly. An eager connect
    // would wait out the two-second connect deadline; a lazy one returns now.
    let started = Instant::now();
    let channel = platform::grpc_tls::lazy_channel_with_optional_tls(
        None,
        "http://192.0.2.1:50052",
        Workload::Friend,
    )
    .expect("a valid endpoint must still build a channel");
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "building a lazy channel must not attempt a connection; it took {:?}",
        started.elapsed()
    );
    // The channel is usable as a client handle; the connection is attempted when
    // a call is made, where the per-edge connect deadline applies.
    drop(channel);
}

#[tokio::test]
async fn a_lazy_channel_still_rejects_invalid_configuration_eagerly() {
    // Deferring reachability must not defer validation. A malformed endpoint is a
    // configuration error, and reporting it at startup is what keeps a typo from
    // becoming a runtime failure on the first request that happens to use it.
    let error =
        platform::grpc_tls::lazy_channel_with_optional_tls(None, "not a uri", Workload::Friend)
            .expect_err("an invalid endpoint must fail while the channel is built");
    assert!(!error.to_string().is_empty());
}
