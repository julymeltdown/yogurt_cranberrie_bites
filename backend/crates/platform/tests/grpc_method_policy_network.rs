//! Default-deny method policy over a live loopback gRPC socket, registered
//! through the same guard() used by every service binary.

use platform::actor_delegation::{
    request_for, ActorDelegationClient, ActorDelegationSigner, ActorDelegationVerifier,
};
use platform::grpc_auth::{CallerRegistry, ServiceIdentityClientInterceptor, Workload};
use platform::grpc_method_policy::{guard, MethodPolicy};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::{Channel, Endpoint, Server};
use tonic::Code;
use tonic_health::pb::{health_client::HealthClient, HealthCheckRequest};
use tonic_health::ServingStatus;
use uuid::Uuid;

static HEALTH_POLICY: MethodPolicy = MethodPolicy {
    service: "grpc.health.v1.Health",
    workload: Some(Workload::Profile),
    methods: &[("Check", &[Workload::Gateway]), ("Watch", &[])],
    actor_fields: &[],
};

/// Check acts for the user named in its `service` field (field 1), standing
/// in for an owner write whose request names its actor.
static DELEGATED_POLICY: MethodPolicy = MethodPolicy {
    service: "grpc.health.v1.Health",
    workload: Some(Workload::Profile),
    methods: &[("Check", &[Workload::Gateway]), ("Watch", &[])],
    actor_fields: &[("Check", &[&[1]])],
};

async fn call(
    address: std::net::SocketAddr,
    workload: Workload,
    token: &str,
    watch: bool,
) -> anyhow::Result<Result<(), Code>> {
    let channel = Endpoint::from_shared(format!("http://{address}"))?
        .connect()
        .await?;
    let mut client = HealthClient::with_interceptor(
        channel,
        ServiceIdentityClientInterceptor::new(workload, token)?,
    );
    let request = HealthCheckRequest {
        service: String::new(),
    };
    let outcome = if watch {
        client.watch(request).await.map(|_| ())
    } else {
        client.check(request).await.map(|_| ())
    };
    Ok(outcome.map_err(|status| status.code()))
}

#[tokio::test]
async fn only_listed_callers_reach_a_live_guarded_service() -> anyhow::Result<()> {
    let registry = CallerRegistry::new()
        .register(Workload::Gateway, "gateway-credential")?
        .register(Workload::Post, "post-credential")?;
    let (_reporter, health) = tonic_health::server::health_reporter();
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let (shutdown, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(
        Server::builder()
            .add_service(guard(
                &HEALTH_POLICY,
                health,
                registry,
                ActorDelegationVerifier::local_parity(Workload::Profile),
            )?)
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = stopped.await;
            }),
    );

    let gateway_check = call(address, Workload::Gateway, "gateway-credential", false).await?;
    let sibling_check = call(address, Workload::Post, "post-credential", false).await?;
    let gateway_watch = call(address, Workload::Gateway, "gateway-credential", true).await?;
    let stolen_name = call(address, Workload::Gateway, "post-credential", false).await?;

    let _ = shutdown.send(());
    server.await??;
    assert_eq!(
        gateway_check,
        Ok(()),
        "listed caller must reach the handler"
    );
    assert_eq!(
        sibling_check,
        Err(Code::PermissionDenied),
        "an authenticated sibling not listed for the method must be denied"
    );
    assert_eq!(
        gateway_watch,
        Err(Code::PermissionDenied),
        "a method with no listed caller must be denied"
    );
    assert_eq!(
        stolen_name,
        Err(Code::Unauthenticated),
        "authentication still runs before the method policy"
    );
    Ok(())
}

#[derive(Clone)]
struct Counted<S> {
    inner: S,
    effects: Arc<AtomicUsize>,
}

impl<S: tonic::server::NamedService> tonic::server::NamedService for Counted<S> {
    const NAME: &'static str = S::NAME;
}

impl<S: tower::Service<R>, R> tower::Service<R> for Counted<S> {
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(
        &mut self,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, request: R) -> Self::Future {
        self.effects.fetch_add(1, Ordering::SeqCst);
        self.inner.call(request)
    }
}

async fn delegated_check(
    channel: Channel,
    signer: Option<(ActorDelegationSigner, Workload)>,
    actor: Uuid,
    named: Uuid,
) -> anyhow::Result<Result<(), Code>> {
    let interceptor =
        ServiceIdentityClientInterceptor::new(Workload::Gateway, "gateway-credential")?;
    let message = HealthCheckRequest {
        service: named.to_string(),
    };
    let outcome = match signer {
        Some((signer, audience)) => {
            HealthClient::with_interceptor(
                ActorDelegationClient::new(channel, signer, audience),
                interceptor,
            )
            .check(request_for(message, &actor.to_string())?)
            .await
        }
        None => {
            HealthClient::with_interceptor(channel, interceptor)
                .check(message)
                .await
        }
    };
    Ok(outcome.map(|_| ()).map_err(|status| status.code()))
}

/// T01.4 over a live socket: the Gateway-side signing client and the
/// owner-side guard. Only a delegation from the trusted key, addressed to this
/// owner, for the user the request names, reaches the service.
#[tokio::test]
async fn only_a_matching_actor_delegation_reaches_an_actor_bound_method() -> anyhow::Result<()> {
    let registry = CallerRegistry::new().register(Workload::Gateway, "gateway-credential")?;
    let (reporter, health) = tonic_health::server::health_reporter();
    let actor = Uuid::new_v4();
    let victim = Uuid::new_v4();
    for user in [actor, victim] {
        reporter
            .set_service_status(user.to_string(), ServingStatus::Serving)
            .await;
    }
    let effects = Arc::new(AtomicUsize::new(0));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let (shutdown, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(
        Server::builder()
            .add_service(guard(
                &DELEGATED_POLICY,
                Counted {
                    inner: health,
                    effects: effects.clone(),
                },
                registry,
                ActorDelegationVerifier::local_parity(Workload::Profile),
            )?)
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = stopped.await;
            }),
    );
    let channel = Endpoint::from_shared(format!("http://{address}"))?
        .connect()
        .await?;
    let gateway = ActorDelegationSigner::local_parity(Workload::Gateway);
    let forger = ActorDelegationSigner::from_seed(Workload::Gateway, &[42; 32])?;

    let cases = [
        ("no delegation", None, actor, Err(Code::Unauthenticated)),
        (
            "delegation for another user's request",
            Some((gateway.clone(), Workload::Profile)),
            victim,
            Err(Code::PermissionDenied),
        ),
        (
            "delegation addressed to another owner",
            Some((gateway.clone(), Workload::Friend)),
            actor,
            Err(Code::PermissionDenied),
        ),
        (
            "delegation signed by an untrusted key",
            Some((forger, Workload::Profile)),
            actor,
            Err(Code::Unauthenticated),
        ),
    ];
    for (label, signer, named, expected) in cases {
        let before = effects.load(Ordering::SeqCst);
        let outcome = delegated_check(channel.clone(), signer, actor, named).await?;
        assert_eq!(outcome, expected, "{label}");
        assert_eq!(
            effects.load(Ordering::SeqCst),
            before,
            "{label}: service effects"
        );
    }
    let allowed = delegated_check(
        channel.clone(),
        Some((gateway, Workload::Profile)),
        actor,
        actor,
    )
    .await?;
    assert_eq!(allowed, Ok(()), "matching delegation");
    assert_eq!(effects.load(Ordering::SeqCst), 1);

    let _ = shutdown.send(());
    server.await??;
    Ok(())
}
