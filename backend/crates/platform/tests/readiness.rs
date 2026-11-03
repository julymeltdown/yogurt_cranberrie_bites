use platform::edge::Edge;
use platform::grpc_auth::{CallerRegistry, Workload};
use platform::readiness::{Probe, ProbeStatus, Readiness, PROBE_CACHE_TTL};
use platform::shutdown::Shutdown;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::Barrier;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

fn constant(name: &'static str, required: bool, up: bool) -> Probe {
    Probe::new(
        name,
        required,
        Duration::from_millis(100),
        move || async move { up },
    )
}

fn local(up: bool) -> Readiness {
    Readiness::with_shutdown(constant("database", true, up), Shutdown::new())
}

struct Peer {
    endpoint: String,
    shutdown: Shutdown,
    task: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
}

impl Peer {
    async fn start(readiness: Readiness) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let shutdown = Shutdown::new();
        let stop = shutdown.cancelled();
        let service = readiness
            .grpc_service(
                CallerRegistry::new()
                    .register(Workload::Gateway, "gateway-test")
                    .unwrap(),
            )
            .unwrap();
        let task = tokio::spawn(
            Server::builder()
                .add_service(service)
                .serve_with_incoming_shutdown(TcpListenerStream::new(listener), stop),
        );
        Self {
            endpoint,
            shutdown,
            task,
        }
    }

    fn probe(&self, credential: &str) -> Probe {
        Probe::grpc(Edge::GatewayProfile, &self.endpoint, credential, None).unwrap()
    }

    async fn stop(self) {
        self.shutdown.request();
        tokio::time::timeout(Duration::from_secs(2), self.task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn required_failure_blocks_readiness_and_optional_failure_is_reported_as_degraded() {
    let optional = local(true)
        .with_dependencies(vec![constant("delivery", false, false)])
        .unwrap()
        .report()
        .await;
    assert!(optional.ready);
    assert!(optional.degraded);
    assert!(!optional.checks["delivery"].required);

    let required = local(true)
        .with_dependencies(vec![constant("authorization", true, false)])
        .unwrap()
        .report()
        .await;
    assert!(!required.ready);
    assert_eq!(required.checks["authorization"].status, ProbeStatus::Down);
    assert!(!local(false).report().await.ready);
}

#[tokio::test]
async fn probes_run_concurrently_and_a_stalled_probe_has_a_deadline() {
    let barrier = Arc::new(Barrier::new(2));
    let probes: Vec<_> = ["first", "second"]
        .into_iter()
        .map(|name| {
            let barrier = barrier.clone();
            Probe::new(name, true, Duration::from_millis(200), move || {
                let barrier = barrier.clone();
                async move {
                    barrier.wait().await;
                    true
                }
            })
        })
        .collect();
    let report = local(true)
        .with_dependencies(probes)
        .unwrap()
        .report()
        .await;
    assert!(
        report.ready,
        "serialized probes would time out at the barrier"
    );

    let stalled = Probe::new("stalled", true, Duration::from_millis(20), || {
        std::future::pending::<bool>()
    });
    let report = tokio::time::timeout(
        Duration::from_secs(1),
        local(true)
            .with_dependencies(vec![stalled])
            .unwrap()
            .report(),
    )
    .await
    .expect("stalled readiness must terminate");
    assert!(!report.ready);
    assert_eq!(report.checks["stalled"].status, ProbeStatus::TimedOut);
}

#[tokio::test]
async fn concurrent_health_requests_share_one_probe_and_recovery_is_observed_after_the_ttl() {
    let calls = Arc::new(AtomicUsize::new(0));
    let up = Arc::new(AtomicBool::new(false));
    let probe = Probe::new("database", true, Duration::from_millis(100), {
        let calls = calls.clone();
        let up = up.clone();
        move || {
            let calls = calls.clone();
            let up = up.clone();
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(5)).await;
                up.load(Ordering::SeqCst)
            }
        }
    });
    let readiness = Readiness::with_shutdown(probe, Shutdown::new());
    let results = futures::future::join_all((0..20).map(|_| readiness.report())).await;
    assert!(results.iter().all(|report| !report.ready));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    up.store(true, Ordering::SeqCst);
    tokio::time::sleep(PROBE_CACHE_TTL + Duration::from_millis(10)).await;
    assert!(readiness.report().await.ready);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn shutdown_overrides_cached_success_without_starting_another_probe() {
    let shutdown = Shutdown::new();
    let readiness = Readiness::with_shutdown(constant("database", true, true), shutdown.clone());
    assert!(readiness.report().await.ready);
    shutdown.request();
    let report = readiness.report().await;
    assert!(!report.ready);
    assert!(report.draining);
    assert_eq!(report.checks["database"].status, ProbeStatus::NotChecked);
}

#[tokio::test]
async fn a_real_authenticated_rpc_distinguishes_serving_from_not_serving() {
    for up in [true, false] {
        let peer = Peer::start(local(up)).await;
        let report = local(true)
            .with_dependencies(vec![peer.probe("gateway-test")])
            .unwrap()
            .report()
            .await;
        assert_eq!(report.ready, up);
        assert_eq!(
            report.checks["gateway->profile"].status,
            if up {
                ProbeStatus::Up
            } else {
                ProbeStatus::Down
            }
        );
        peer.stop().await;
    }
}

#[tokio::test]
async fn invalid_credentials_do_not_reach_the_health_handler() {
    let calls = Arc::new(AtomicUsize::new(0));
    let local_probe = Probe::new("database", true, Duration::from_millis(100), {
        let calls = calls.clone();
        move || {
            calls.fetch_add(1, Ordering::SeqCst);
            async { true }
        }
    });
    let peer = Peer::start(Readiness::with_shutdown(local_probe, Shutdown::new())).await;
    let report = local(true)
        .with_dependencies(vec![peer.probe("wrong-credential")])
        .unwrap()
        .report()
        .await;
    assert!(!report.ready);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    peer.stop().await;
}

#[tokio::test]
async fn grpc_health_does_not_recursively_probe_the_peers_dependencies() {
    let peer_readiness = local(true)
        .with_dependencies(vec![Probe::new(
            "peer-of-peer",
            true,
            Duration::from_millis(100),
            || async { panic!("a gRPC health check must not recursively fan out") },
        )])
        .unwrap();
    let peer = Peer::start(peer_readiness).await;
    let report = local(true)
        .with_dependencies(vec![peer.probe("gateway-test")])
        .unwrap()
        .report()
        .await;
    assert!(report.ready);
    peer.stop().await;
}

#[tokio::test]
async fn a_closed_listener_is_reported_down_without_blocking_construction() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let probe = Probe::grpc(Edge::GatewayProfile, &endpoint, "gateway-test", None).unwrap();
    let report = tokio::time::timeout(
        Duration::from_secs(2),
        local(true).with_dependencies(vec![probe]).unwrap().report(),
    )
    .await
    .unwrap();
    assert!(!report.ready);
    assert_ne!(report.checks["gateway->profile"].status, ProbeStatus::Up);
}

#[test]
fn background_deletion_and_enrichment_do_not_gate_request_readiness() {
    for edge in [
        Edge::AuthProfile,
        Edge::AuthFriend,
        Edge::AuthPost,
        Edge::AuthNotification,
        Edge::PostProfile,
        Edge::PostNotification,
    ] {
        assert!(!edge.required_for_readiness(), "{edge:?}");
    }
    for edge in [
        Edge::GatewayAuth,
        Edge::GatewayProfile,
        Edge::GatewayPost,
        Edge::GatewayFriend,
        Edge::GatewayNotification,
        Edge::PostFriend,
        Edge::PostGateway,
        Edge::FriendProfile,
    ] {
        assert!(edge.required_for_readiness(), "{edge:?}");
    }
    let exchange = Edge::PostGateway.budget();
    assert_eq!(
        platform::edge::EdgeBudget {
            edge: Edge::GatewayPost.budget().edge,
            ..exchange
        },
        Edge::GatewayPost.budget(),
        "the exchange edge must carry exactly the Gateway to Post budget values"
    );
    for edge in Edge::ALL {
        assert_eq!(
            edge.budget().edge,
            format!("{}->{}", edge.caller().as_str(), edge.peer().as_str())
        );
    }
}

#[test]
fn duplicate_dependency_names_are_rejected_instead_of_overwriting_a_result() {
    assert!(local(true)
        .with_dependencies(vec![constant("database", false, true)])
        .is_err());
}
