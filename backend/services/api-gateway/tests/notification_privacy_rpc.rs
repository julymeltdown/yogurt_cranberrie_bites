//! Real TCP between the production Gateway batch adapter and guarded Post RPC.
//! The owner application ports are doubles; real persistence is tested separately.
#[allow(dead_code)]
#[path = "../../post-service/tests/unit/support.rs"]
mod post_support;

use api_gateway::adapters::out::post_client::GrpcPostClient;
use api_gateway::application::ports::PostClientPort;
use platform::actor_delegation::{
    request_for, ActorDelegationClient, ActorDelegationSigner, ActorDelegationVerifier,
};
use platform::grpc_auth::{CallerRegistry, ServiceIdentityClientInterceptor, Workload};
use post_service::adapters::r#in::grpc::{
    method_policy::POST_METHOD_POLICY,
    server::{proto, PostGrpcServer},
};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::{Endpoint, Server};
use tower::Service;
use uuid::Uuid;

/// Controlled wire-error peer behind the production method/actor guard.
/// This supplies errors over TCP; it is not a production owner or persistence proof.
#[derive(Clone)]
struct PostFailureWirePeer {
    status: Arc<std::sync::Mutex<tonic::Status>>,
    calls: Arc<AtomicU64>,
}

impl tonic::server::NamedService for PostFailureWirePeer {
    const NAME: &'static str = "post.v1.PostService";
}

impl Service<http::Request<tonic::body::Body>> for PostFailureWirePeer {
    type Response = http::Response<tonic::body::Body>;
    type Error = std::convert::Infallible;
    type Future = std::future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: http::Request<tonic::body::Body>) -> Self::Future {
        self.calls.fetch_add(1, Ordering::SeqCst);
        std::future::ready(Ok(self.status.lock().unwrap().clone().into_http()))
    }
}

#[tokio::test]
async fn post_failure_http_transport_projection_preserves_codes_metadata_and_bounds(
) -> anyhow::Result<()> {
    use api_gateway::adapters::r#in::http::map_service_error;
    use axum::response::IntoResponse;
    use serde_json::{json, Value};
    use tonic::Code;
    let peer = PostFailureWirePeer {
        status: Arc::new(std::sync::Mutex::new(tonic::Status::internal(
            "private-post-diagnostic",
        ))),
        calls: Arc::new(AtomicU64::new(0)),
    };
    let registry = CallerRegistry::new().register(Workload::Gateway, "gateway-post-errors")?;
    let guarded = platform::grpc_method_policy::guard(
        &POST_METHOD_POLICY,
        peer.clone(),
        registry,
        ActorDelegationVerifier::local_parity(Workload::Post),
    )?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}", listener.local_addr()?);
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(
        Server::builder()
            .add_service(guarded)
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = stopped.await;
            }),
    );
    let result = async {
        let client = GrpcPostClient::connect(endpoint, Workload::Gateway, "gateway-post-errors", None).await?;
        let actor = "11111111-1111-4111-8111-111111111111";
        let rows = [
            (Code::InvalidArgument,400,"Invalid input"),
            (Code::Unauthenticated,401,"Invalid credentials"),
            (Code::PermissionDenied,403,"Forbidden"),
            (Code::NotFound,404,"Not found"),
            (Code::AlreadyExists,409,"Resource already exists"),
            (Code::Aborted,409,"Operation conflict; retry"),
            (Code::ResourceExhausted,429,"Too many requests"),
            (Code::FailedPrecondition,400,"Request is not valid in the current state"),
            (Code::Unavailable,503,"Service unavailable"),
            (Code::DeadlineExceeded,504,"Downstream request timed out"),
            (Code::Ok,500,"Internal error"),
            (Code::Cancelled,500,"Internal error"),
            (Code::Unknown,500,"Internal error"),
            (Code::OutOfRange,500,"Internal error"),
            (Code::Unimplemented,500,"Internal error"),
            (Code::Internal,500,"Internal error"),
            (Code::DataLoss,500,"Internal error"),
        ];
        let mut cases = Vec::new();
        for (code,status,message) in rows {
            // A grpc OK with no reply becomes a client decode fault, not an error-code projection.
            *peer.status.lock().unwrap() = tonic::Status::new(code, if code==Code::InvalidArgument {"Invalid input"} else {"private-post-diagnostic"});
            let error = client.get_feed(actor.into(),None,10).await.expect_err("controlled wire failure");
            let response = map_service_error(error).into_response();
            assert_eq!(response.status().as_u16(),status,"{code:?}");
            assert!(response.headers().get(http::header::RETRY_AFTER).is_none());
            let bytes=axum::body::to_bytes(response.into_body(),1<<20).await?;
            let body:Value=serde_json::from_slice(&bytes)?;
            assert_eq!(body["message"],message);
            assert!(!body.to_string().contains("private-post-diagnostic"));
            cases.push(json!({"case":format!("code-{code:?}"),"status":status,"message":message}));
        }
        for (value,message) in [
            ("unknown","Operation outcome is unknown; reconcile using the original operation ID and key"),
            ("expired","Operation replay window expired; reconcile before issuing another command"),
            ("conflict","Operation identity or request conflicts with its durable receipt"),
            ("UNKNOWN","Resource already exists"),
        ] {
            let mut status=tonic::Status::already_exists("private-owner-diagnostic");
            status.metadata_mut().insert(platform::mutation_context::ERROR_CODE,value.parse()?);
            *peer.status.lock().unwrap()=status;
            let error=client.get_feed(actor.into(),None,10).await.expect_err("owner conflict");
            let response=map_service_error(error).into_response();
            assert_eq!(response.status().as_u16(),409);
            let body:Value=serde_json::from_slice(&axum::body::to_bytes(response.into_body(),1<<20).await?)?;
            assert_eq!(body["message"],message);
            assert!(!body.to_string().contains("private-owner-diagnostic"));
            cases.push(json!({"case":format!("owner-{value}"),"status":409,"message":message}));
        }
        for message in ["AUTH_BUSY","not AUTH_BUSY","auth_busy","AUTH_DEPENDENCY_UNAVAILABLE"] {
            *peer.status.lock().unwrap()=tonic::Status::unavailable(message);
            let error=client.get_feed(actor.into(),None,10).await.expect_err("unavailable");
            let response=map_service_error(error).into_response();
            assert_eq!(response.status().as_u16(),503);
            let busy=message=="AUTH_BUSY";
            assert_eq!(response.headers().get(http::header::RETRY_AFTER).map(|v|v.to_str().unwrap()),if busy {Some("1")} else {None});
            let body:Value=serde_json::from_slice(&axum::body::to_bytes(response.into_body(),1<<20).await?)?;
            let projected=if busy {"Service busy; retry later"} else {"Service unavailable"};
            assert_eq!(body["message"],projected);
            cases.push(json!({"case":format!("busy-{message}"),"status":503,"message":projected,"retryAfter":if busy {Some("1")} else {None}}));
        }
        for (name,message,expected) in [("bounded",format!("  public\n{}  ","x".repeat(400)),format!("public{}","x".repeat(248))),("empty","\n\r\t  ".into(),"Invalid request".into())] {
            *peer.status.lock().unwrap()=tonic::Status::invalid_argument(message);
            let error=client.get_feed(actor.into(),None,10).await.expect_err("invalid argument");
            let response=map_service_error(error).into_response();
            assert_eq!(response.status().as_u16(),400);
            let body:Value=serde_json::from_slice(&axum::body::to_bytes(response.into_body(),1<<20).await?)?;
            assert_eq!(body["message"],expected);
            assert!(body["message"].as_str().unwrap().chars().count()<=256);
            cases.push(json!({"case":format!("input-{name}"),"status":400,"message":expected}));
        }
        *peer.status.lock().unwrap()=tonic::Status::not_found("private-receipt-diagnostic");
        assert!(client.get_mutation_receipt_status(Uuid::from_u128(19)).await?.is_none());
        assert_eq!(peer.calls.load(Ordering::SeqCst),28);
        for case in cases {println!("POST_FAILURE_HTTP {case}");}
        anyhow::Ok(())
    }.await;
    let _ = stop.send(());
    server.await??;
    result
}

#[derive(Clone)]
struct Delayed<S> {
    inner: S,
    delay_ms: Arc<AtomicU64>,
}
impl<S: tonic::server::NamedService> tonic::server::NamedService for Delayed<S> {
    const NAME: &'static str = S::NAME;
}
impl<S, R> Service<R> for Delayed<S>
where
    S: Service<R> + Clone + Send + 'static,
    R: Send + 'static,
    S::Future: Send,
    S::Response: Send + 'static,
    S::Error: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<S::Response, S::Error>> + Send>>;
    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, request: R) -> Self::Future {
        let replacement = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, replacement);
        let delay = self.delay_ms.load(Ordering::SeqCst);
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(delay)).await;
            inner.call(request).await
        })
    }
}

#[tokio::test]
async fn batch_authorization_is_actor_bound_content_free_and_deadline_bounded() -> anyhow::Result<()>
{
    let harness = post_support::Harness::new();
    let viewer = Uuid::new_v4();
    let author = Uuid::new_v4();
    let public_id = Uuid::new_v4();
    let private_id = Uuid::new_v4();
    harness.posts.posts_by_id.lock().unwrap().insert(
        public_id,
        post_support::post(
            public_id,
            author,
            "CONTENT_MUST_NOT_CROSS_AUTHORIZATION_RPC",
        ),
    );
    let mut private = post_support::post(private_id, author, "PRIVATE_CONTENT");
    private.visibility = "PRIVATE".into();
    harness
        .posts
        .posts_by_id
        .lock()
        .unwrap()
        .insert(private_id, private);
    let registry = CallerRegistry::new()
        .register(Workload::Gateway, "gateway-batch")?
        .register(Workload::Friend, "friend-batch")?;
    let guarded = platform::grpc_method_policy::guard(
        &POST_METHOD_POLICY,
        proto::post_service_server::PostServiceServer::new(PostGrpcServer::new(harness.state())),
        registry,
        ActorDelegationVerifier::local_parity(Workload::Post),
    )?;
    let delay_ms = Arc::new(AtomicU64::new(0));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}", listener.local_addr()?);
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(
        Server::builder()
            .add_service(Delayed {
                inner: guarded,
                delay_ms: delay_ms.clone(),
            })
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = stopped.await;
            }),
    );
    let client =
        GrpcPostClient::connect(endpoint.clone(), Workload::Gateway, "gateway-batch", None).await?;
    let mut ids = vec![public_id.to_string(), private_id.to_string()];
    ids.extend((0..98).map(|_| Uuid::new_v4().to_string()));
    let decisions = client
        .notification_posts_visible(viewer.to_string(), ids.clone(), Duration::from_secs(1))
        .await?;
    assert_eq!(decisions.len(), 100);
    assert_eq!(decisions.get(&public_id.to_string()), Some(&true));
    assert!(ids
        .iter()
        .filter(|id| **id != public_id.to_string())
        .all(|id| decisions.get(id) == Some(&false)));
    assert_eq!(
        harness.posts.access_queries.lock().unwrap().len(),
        1,
        "one owner query handles the complete page"
    );

    let channel = Endpoint::from_shared(endpoint.clone())?.connect().await?;
    let mut unsigned = proto::post_service_client::PostServiceClient::with_interceptor(
        channel.clone(),
        ServiceIdentityClientInterceptor::new(Workload::Gateway, "gateway-batch")?,
    );
    let error = unsigned
        .authorize_notification_posts(proto::AuthorizeNotificationPostsRequest {
            viewer_id: viewer.to_string(),
            post_ids: ids.clone(),
        })
        .await
        .expect_err("missing actor proof");
    assert_eq!(error.code(), tonic::Code::Unauthenticated);
    let signing = ActorDelegationClient::new(
        channel,
        ActorDelegationSigner::local_parity(Workload::Gateway),
        Workload::Post,
    );
    let mut forged = proto::post_service_client::PostServiceClient::with_interceptor(
        signing,
        ServiceIdentityClientInterceptor::new(Workload::Gateway, "gateway-batch")?,
    );
    let error = forged
        .authorize_notification_posts(request_for(
            proto::AuthorizeNotificationPostsRequest {
                viewer_id: viewer.to_string(),
                post_ids: ids.clone(),
            },
            &Uuid::new_v4().to_string(),
        )?)
        .await
        .expect_err("actor mismatch");
    assert_eq!(error.code(), tonic::Code::PermissionDenied);
    let sibling = GrpcPostClient::connect(endpoint, Workload::Friend, "friend-batch", None).await?;
    let error = sibling
        .notification_posts_visible(viewer.to_string(), ids.clone(), Duration::from_secs(1))
        .await
        .expect_err("registered sibling denial");
    assert_eq!(
        error,
        api_gateway::application::post_client_error::PostClientError::PermissionDenied
    );
    assert_eq!(
        harness.posts.access_queries.lock().unwrap().len(),
        1,
        "denied calls never enter the repository"
    );

    delay_ms.store(200, Ordering::SeqCst);
    let started = Instant::now();
    let error = client
        .notification_posts_visible(viewer.to_string(), ids, Duration::from_millis(20))
        .await
        .expect_err("total projection deadline");
    assert_eq!(
        error,
        api_gateway::application::post_client_error::PostClientError::DeadlineExceeded
    );
    assert!(started.elapsed() < Duration::from_millis(150));
    stop.send(()).unwrap();
    server.await??;
    Ok(())
}

// Controlled error peers exercise production outbound clients over TCP.
// These peers do not implement owner applications or production method guards.
macro_rules! client_failure_peer {
    ($name:ident, $service:literal) => {
        #[derive(Clone)]
        struct $name(PostFailureWirePeer);
        impl tonic::server::NamedService for $name {
            const NAME: &'static str = $service;
        }
        impl Service<http::Request<tonic::body::Body>> for $name {
            type Response = http::Response<tonic::body::Body>;
            type Error = std::convert::Infallible;
            type Future = std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
            >;
            fn poll_ready(
                &mut self,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Result<(), Self::Error>> {
                self.0.poll_ready(cx)
            }
            fn call(&mut self, request: http::Request<tonic::body::Body>) -> Self::Future {
                use http_body_util::BodyExt;
                let peer = self.0.clone();
                Box::pin(async move {
                    // Drain the complete request before responding. Dropping an unread
                    // HTTP/2 body can reset the stream and mask the configured status.
                    request
                        .into_body()
                        .collect()
                        .await
                        .expect("controlled peer request body");
                    peer.calls.fetch_add(1, Ordering::SeqCst);
                    let status = peer.status.lock().unwrap().clone();
                    Ok(status.into_http())
                })
            }
        }
    };
}
client_failure_peer!(AuthFailurePeer, "auth.v1.AuthService");
client_failure_peer!(ProfileFailurePeer, "profile.v1.ProfileService");
client_failure_peer!(FriendFailurePeer, "friend.v1.FriendService");
client_failure_peer!(
    NotificationFailurePeer,
    "notification.v1.NotificationService"
);

struct ClientFailureProbe {
    auth: api_gateway::adapters::out::auth_client::GrpcAuthClient,
    profile: api_gateway::adapters::out::profile_client::GrpcProfileClient,
    friend: api_gateway::adapters::out::friend_client::GrpcFriendClient,
    notification: api_gateway::adapters::out::notification_client::GrpcNotificationClient,
}
impl ClientFailureProbe {
    async fn call(&self, port: &str) -> anyhow::Result<()> {
        use api_gateway::application::ports::{
            AuthClientPort, FriendClientPort, NotificationClientPort, ProfileClientPort,
            SigningKeySourcePort,
        };
        let actor = "11111111-1111-4111-8111-111111111111";
        match port {
            "auth" => self
                .auth
                .check_email(api_gateway::application::dto::auth_dto::EmailCheckRequest {
                    email: "public@example.com".into(),
                })
                .await
                .map(|_| ())
                .map_err(Into::into),
            "profile" => self
                .profile
                .get_profile(actor.into())
                .await
                .map(|_| ())
                .map_err(Into::into),
            "friend" => self
                .friend
                .get_counts(actor.into())
                .await
                .map(|_| ())
                .map_err(Into::into),
            "notification" => self
                .notification
                .list_notifications(actor.into(), None, 10)
                .await
                .map(|_| ())
                .map_err(Into::into),
            "signing-keys" => self
                .auth
                .signing_keys()
                .await
                .map(|_| ())
                .map_err(Into::into),
            "session" => self
                .auth
                .session_state(
                    &api_gateway::application::dto::issuer_dto::SessionAuthorityRequestDto {
                        user_id: actor.into(),
                        session_id: "session-a".into(),
                    },
                )
                .await
                .map(|_| ())
                .map_err(Into::into),
            _ => unreachable!("fixed probe port"),
        }
    }
}

/// The gRPC method each `ClientFailureProbe` port exercises.
///
/// Only an UNAVAILABLE from a replay-safe read may be attempted twice, so the
/// check below needs the method rather than only the port's display name.
const CLIENT_PROBE_METHODS: &[(&str, &str)] = &[
    ("auth", "/auth.v1.AuthService/CheckEmail"),
    ("profile", "/profile.v1.ProfileService/GetProfile"),
    ("friend", "/friend.v1.FriendService/GetFollowCounts"),
    (
        "notification",
        "/notification.v1.NotificationService/ListNotifications",
    ),
    ("signing-keys", "/auth.v1.AuthService/GetSigningKeys"),
    ("session", "/auth.v1.AuthService/CheckSession"),
];

/// The gRPC method each `ServiceFailureProbe` port exercises.
const SERVICE_PROBE_METHODS: &[(&str, &str)] = &[
    ("auth", "/auth.v1.AuthService/CheckEmail"),
    ("profile", "/profile.v1.ProfileService/GetProfile"),
    ("friend", "/friend.v1.FriendService/GetFollowCounts"),
    (
        "notification",
        "/notification.v1.NotificationService/MarkAllNotificationsRead",
    ),
    ("post", "/post.v1.PostService/GetFeed"),
    ("session", "/auth.v1.AuthService/CheckSession"),
];

/// Whether the bounded read retry may attempt this probe port's method twice.
fn probe_may_retry(methods: &[(&str, &str)], port: &str) -> bool {
    let method = methods
        .iter()
        .find(|(name, _)| *name == port)
        .map(|(_, method)| *method)
        .expect("every probe port declares its method");
    platform::grpc_retry::REPLAY_SAFE_READS.contains(&method)
}

/// Check the peer calls one probe case produced.
///
/// The assertion is per case rather than one global total because the retry
/// throttle is shared per edge: whether the n-th UNAVAILABLE is retried depends
/// on how many retryable failures earlier cases already spent, so a single exact
/// total cannot express the rule and silently went stale when the bounded read
/// retry landed.
///
/// A call outside `REPLAY_SAFE_READS`, or one that does not answer UNAVAILABLE,
/// is never retried, so exactly one peer call is the only correct answer and any
/// extra one is request amplification. A replay-safe read that answers
/// UNAVAILABLE may be attempted once more, bounded by the policy's own
/// `max_attempts`.
fn assert_probe_calls(port: &str, case: &str, delta: u64, may_retry: bool) {
    assert!(
        delta >= 1,
        "{port}/{case}: the request must reach the controlled peer"
    );
    if may_retry {
        let allowed = u64::from(platform::grpc_retry::READ_RETRY.max_attempts);
        assert!(
            delta <= allowed,
            "{port}/{case}: {delta} peer calls exceeds the {allowed} the read retry policy allows"
        );
    } else {
        assert_eq!(
            delta, 1,
            "{port}/{case}: a non-retryable call must be attempted exactly once"
        );
    }
}

#[tokio::test]
async fn remaining_client_failure_transport_projection_preserves_public_contract(
) -> anyhow::Result<()> {
    use api_gateway::adapters::r#in::http::map_service_error;
    use api_gateway::application::ports::{AuthClientPort, FriendClientPort, ProfileClientPort};
    use axum::response::IntoResponse;
    use serde_json::{json, Value};
    use tonic::Code;
    let peer = PostFailureWirePeer {
        status: Arc::new(std::sync::Mutex::new(tonic::Status::internal(
            "private-client-diagnostic",
        ))),
        calls: Arc::new(AtomicU64::new(0)),
    };
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}", listener.local_addr()?);
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(
        Server::builder()
            .add_service(AuthFailurePeer(peer.clone()))
            .add_service(ProfileFailurePeer(peer.clone()))
            .add_service(FriendFailurePeer(peer.clone()))
            .add_service(NotificationFailurePeer(peer.clone()))
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = stopped.await;
            }),
    );
    let result=async {
        let probe=ClientFailureProbe {
            auth:api_gateway::adapters::out::auth_client::GrpcAuthClient::connect(endpoint.clone(),Workload::Gateway,"gateway-client-errors",None).await?,
            profile:api_gateway::adapters::out::profile_client::GrpcProfileClient::connect(endpoint.clone(),Workload::Gateway,"gateway-client-errors",None).await?,
            friend:api_gateway::adapters::out::friend_client::GrpcFriendClient::connect(endpoint.clone(),Workload::Gateway,"gateway-client-errors",None).await?,
            notification:api_gateway::adapters::out::notification_client::GrpcNotificationClient::connect(endpoint,Workload::Gateway,"gateway-client-errors",None).await?,
        };
        let probe_methods: &[(&str, &str)] = CLIENT_PROBE_METHODS;
        // Count what the probe is allowed to do, then bound the wire total by it.
        let mut probe_calls: u64 = 0;
        let mut retry_permits: u64 = 0;
        for port in ["auth","profile","friend","notification","signing-keys","session"] {
        let rows = [
            (Code::InvalidArgument,400,"Invalid input"),
            (Code::Unauthenticated,401,"Invalid credentials"),
            (Code::PermissionDenied,403,"Forbidden"),
            (Code::NotFound,404,"Not found"),
            (Code::AlreadyExists,409,"Resource already exists"),
            (Code::Aborted,409,"Operation conflict; retry"),
            (Code::ResourceExhausted,429,"Too many requests"),
            (Code::FailedPrecondition,400,"Request is not valid in the current state"),
            (Code::Unavailable,503,"Service unavailable"),
            (Code::DeadlineExceeded,504,"Downstream request timed out"),
            (Code::Ok,500,"Internal error"),
            (Code::Cancelled,500,"Internal error"),
            (Code::Unknown,500,"Internal error"),
            (Code::OutOfRange,500,"Internal error"),
            (Code::Unimplemented,500,"Internal error"),
            (Code::Internal,500,"Internal error"),
            (Code::DataLoss,500,"Internal error"),
        ];
        let mut cases = Vec::new();
        for (code,status,message) in rows {
            // A grpc OK with no reply becomes a client decode fault, not an error-code projection.
            *peer.status.lock().unwrap() = tonic::Status::new(code, if code==Code::InvalidArgument {"Invalid input"} else {"private-post-diagnostic"});
            let before = peer.calls.load(Ordering::SeqCst);
            let error = probe.call(port).await.expect_err("controlled wire failure");
            let delta = peer.calls.load(Ordering::SeqCst) - before;
            probe_calls += 1;
            let may_retry = code == Code::Unavailable && probe_may_retry(probe_methods, port);
            if may_retry { retry_permits += 1; }
            assert_probe_calls(port, &format!("code-{code:?}"), delta, may_retry);
            let response = map_service_error(error).into_response();
            assert_eq!(response.status().as_u16(),status,"{code:?}");
            assert!(response.headers().get(http::header::RETRY_AFTER).is_none());
            let bytes=axum::body::to_bytes(response.into_body(),1<<20).await?;
            let body:Value=serde_json::from_slice(&bytes)?;
            assert_eq!(body["message"],message);
            assert!(!body.to_string().contains("private-post-diagnostic"));
            cases.push(json!({"port":port,"case":format!("code-{code:?}"),"status":status,"message":message}));
        }
        for (value,message) in [
            ("unknown","Operation outcome is unknown; reconcile using the original operation ID and key"),
            ("expired","Operation replay window expired; reconcile before issuing another command"),
            ("conflict","Operation identity or request conflicts with its durable receipt"),
            ("UNKNOWN","Resource already exists"),
        ] {
            let mut status=tonic::Status::already_exists("private-owner-diagnostic");
            status.metadata_mut().insert(platform::mutation_context::ERROR_CODE,value.parse()?);
            *peer.status.lock().unwrap()=status;
            let before=peer.calls.load(Ordering::SeqCst);
            let error=probe.call(port).await.expect_err("owner conflict");
            let delta=peer.calls.load(Ordering::SeqCst)-before;
            probe_calls += 1;
            assert_probe_calls(port, &format!("owner-{value}"), delta, false);
            let diagnostic=format!("{error:?}");
            let response=map_service_error(error).into_response();
            assert_eq!(response.status().as_u16(),409,"port={port}, metadata={value}, error={diagnostic}");
            let body:Value=serde_json::from_slice(&axum::body::to_bytes(response.into_body(),1<<20).await?)?;
            assert_eq!(body["message"],message);
            assert!(!body.to_string().contains("private-owner-diagnostic"));
            cases.push(json!({"port":port,"case":format!("owner-{value}"),"status":409,"message":message}));
        }
        for message in ["AUTH_BUSY","not AUTH_BUSY","auth_busy","AUTH_DEPENDENCY_UNAVAILABLE"] {
            *peer.status.lock().unwrap()=tonic::Status::unavailable(message);
            let before=peer.calls.load(Ordering::SeqCst);
            let error=probe.call(port).await.expect_err("unavailable");
            let delta=peer.calls.load(Ordering::SeqCst)-before;
            probe_calls += 1;
            let may_retry = probe_may_retry(probe_methods, port);
            if may_retry { retry_permits += 1; }
            assert_probe_calls(port, &format!("busy-{message}"), delta, may_retry);
            let diagnostic=format!("{error:?}");
            let response=map_service_error(error).into_response();
            assert_eq!(response.status().as_u16(),503,"port={port}, message={message}, error={diagnostic}");
            let busy=message=="AUTH_BUSY";
            assert_eq!(response.headers().get(http::header::RETRY_AFTER).map(|v|v.to_str().unwrap()),if busy {Some("1")} else {None});
            let body:Value=serde_json::from_slice(&axum::body::to_bytes(response.into_body(),1<<20).await?)?;
            let projected=if busy {"Service busy; retry later"} else {"Service unavailable"};
            assert_eq!(body["message"],projected);
            cases.push(json!({"port":port,"case":format!("busy-{message}"),"status":503,"message":projected,"retryAfter":if busy {Some("1")} else {None}}));
        }
        for (name,message,expected) in [("bounded",format!("  public\n{}  ","x".repeat(400)),format!("public{}","x".repeat(248))),("empty","\n\r\t  ".into(),"Invalid request".into())] {
            *peer.status.lock().unwrap()=tonic::Status::invalid_argument(message);
            let before=peer.calls.load(Ordering::SeqCst);
            let error=probe.call(port).await.expect_err("invalid argument");
            let delta=peer.calls.load(Ordering::SeqCst)-before;
            probe_calls += 1;
            assert_probe_calls(port, &format!("input-{name}"), delta, false);
            let diagnostic=format!("{error:?}");
            let response=map_service_error(error).into_response();
            assert_eq!(response.status().as_u16(),400,"port={port}, input={name}, error={diagnostic}");
            let body:Value=serde_json::from_slice(&axum::body::to_bytes(response.into_body(),1<<20).await?)?;
            assert_eq!(body["message"],expected);
            assert!(body["message"].as_str().unwrap().chars().count()<=256);
            cases.push(json!({"port":port,"case":format!("input-{name}"),"status":400,"message":expected}));
        }

            for case in cases {println!("CLIENT_FAILURE_HTTP {case}");}
        }
        let tail_calls=peer.calls.load(Ordering::SeqCst);
        *peer.status.lock().unwrap()=tonic::Status::not_found("private-receipt-diagnostic");
        assert!(probe.profile.get_mutation_receipt_status(Uuid::from_u128(19)).await?.is_none());
        assert!(probe.friend.get_mutation_receipt_status(Uuid::from_u128(19)).await?.is_none());
        let deletion_error=probe.auth.get_user_deletion("11111111-1111-4111-8111-111111111111".into()).await.expect_err("transport NotFound is not a found=false reply");
        assert_eq!(map_service_error(deletion_error).into_response().status().as_u16(),404);
        let saga_error=probe.auth.get_user_deletion_by_saga_id(Uuid::from_u128(19).to_string()).await.expect_err("transport NotFound is not a found=false reply");
        assert_eq!(map_service_error(saga_error).into_response().status().as_u16(),404);
        // The four lookup/receipt reads after the loop are not replay-safe
        // either, so they are attempted once each and the floor is exact.
        assert_eq!(
            peer.calls.load(Ordering::SeqCst)-tail_calls,
            4,
            "the lookup reads are attempted exactly once each"
        );
        let total=peer.calls.load(Ordering::SeqCst);
        let floor=probe_calls+4;
        assert!(
            total>=floor,
            "every probe case must reach the peer once: {total} < {floor}"
        );
        assert!(
            total<=floor+retry_permits,
            "only a replay-safe UNAVAILABLE may add a call: {total} > {floor}+{retry_permits}"
        );
        anyhow::Ok(())
    }.await;
    let _ = stop.send(());
    server.await??;
    result
}

// The service-level probe uses production outbound clients over TCP. Only the
// Post peer has the production method/actor guard; all peers supply controlled
// failures. This is neither a full router nor real owner/persistence acceptance.
client_failure_peer!(PostServiceFailurePeer, "post.v1.PostService");

struct ServiceFailureProbe {
    auth: api_gateway::application::service::AuthGatewayService,
    profile: api_gateway::application::service::ProfileGatewayService,
    friend: api_gateway::application::service::FriendGatewayService,
    notification: api_gateway::application::service::NotificationGatewayService,
    post: api_gateway::application::service::PostGatewayService,
    session: api_gateway::application::service::high_risk_session_service::HighRiskSessionGuard,
}
impl ServiceFailureProbe {
    async fn call(&self, port: &str) -> anyhow::Result<()> {
        let actor = "11111111-1111-4111-8111-111111111111";
        match port {
            "auth" => self
                .auth
                .check_email(api_gateway::application::dto::auth_dto::EmailCheckRequest {
                    email: "public@example.com".into(),
                })
                .await
                .map(|_| ())
                .map_err(Into::into),
            "profile" => self
                .profile
                .get_profile(actor.into())
                .await
                .map(|_| ())
                .map_err(Into::into),
            "friend" => self
                .friend
                .counts(actor.into())
                .await
                .map(|_| ())
                .map_err(Into::into),
            "notification" => self
                .notification
                .mark_all_read(actor.into())
                .await
                .map(|_| ())
                .map_err(Into::into),
            "post" => self
                .post
                .get_feed(actor.into(), None, 10)
                .await
                .map(|_| ())
                .map_err(Into::into),
            "session" => self
                .session
                .check(
                    actor,
                    "session-a",
                    api_gateway::domain::high_risk::HighRiskAction::Administration.into(),
                )
                .await
                .map(|_| ())
                .map_err(Into::into),
            _ => unreachable!("fixed service probe port"),
        }
    }
}

#[tokio::test]
async fn service_failure_transport_projection_preserves_public_contract() -> anyhow::Result<()> {
    use api_gateway::adapters::r#in::http::map_service_error;
    use axum::response::IntoResponse;
    use serde_json::{json, Value};
    use tonic::Code;
    let peer = PostFailureWirePeer {
        status: Arc::new(std::sync::Mutex::new(tonic::Status::internal(
            "private-client-diagnostic",
        ))),
        calls: Arc::new(AtomicU64::new(0)),
    };
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}", listener.local_addr()?);
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let guarded_post = platform::grpc_method_policy::guard(
        &POST_METHOD_POLICY,
        PostServiceFailurePeer(peer.clone()),
        CallerRegistry::new().register(Workload::Gateway, "gateway-client-errors")?,
        ActorDelegationVerifier::local_parity(Workload::Post),
    )?;
    let server = tokio::spawn(
        Server::builder()
            .add_service(guarded_post)
            .add_service(AuthFailurePeer(peer.clone()))
            .add_service(ProfileFailurePeer(peer.clone()))
            .add_service(FriendFailurePeer(peer.clone()))
            .add_service(NotificationFailurePeer(peer.clone()))
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = stopped.await;
            }),
    );
    let result=async {
        use api_gateway::application::service::{AuthGatewayService, FriendGatewayService, ProfileGatewayService, NotificationGatewayService, PostGatewayService};
        use api_gateway::application::service::high_risk_session_service::HighRiskSessionGuard;
        use api_gateway::application::search_cursor::SearchCursorCodec;
        let auth=Arc::new(api_gateway::adapters::out::auth_client::GrpcAuthClient::connect(endpoint.clone(),Workload::Gateway,"gateway-client-errors",None).await?);
        let profile=Arc::new(api_gateway::adapters::out::profile_client::GrpcProfileClient::connect(endpoint.clone(),Workload::Gateway,"gateway-client-errors",None).await?);
        let friend=Arc::new(api_gateway::adapters::out::friend_client::GrpcFriendClient::connect(endpoint.clone(),Workload::Gateway,"gateway-client-errors",None).await?);
        let notification=Arc::new(api_gateway::adapters::out::notification_client::GrpcNotificationClient::connect(endpoint.clone(),Workload::Gateway,"gateway-client-errors",None).await?);
        let post=Arc::new(GrpcPostClient::connect(endpoint,Workload::Gateway,"gateway-client-errors",None).await?);
        let codec=SearchCursorCodec::new(Arc::from(b"gateway-service-error-cursor-key-32-bytes-minimum".as_slice()),900,None, Arc::new(api_gateway::adapters::out::runtime::SystemGatewayRuntime))?;
        let probe=ServiceFailureProbe {
            auth:AuthGatewayService::new(auth.clone()),
            profile:ProfileGatewayService::new(profile,friend.clone(),post.clone(),codec.clone()),
            friend:FriendGatewayService::new(friend),
            notification:NotificationGatewayService::new(notification,post.clone(), Arc::new(api_gateway::adapters::out::runtime::SystemGatewayRuntime)),
            post:PostGatewayService::new(post,codec),
            session:HighRiskSessionGuard::over_auth_client(auth,Duration::ZERO, Arc::new(api_gateway::adapters::out::runtime::SystemGatewayRuntime)),
        };
        let probe_methods: &[(&str, &str)] = SERVICE_PROBE_METHODS;
        let mut probe_calls: u64 = 0;
        let mut retry_permits: u64 = 0;
        for port in ["auth","profile","friend","notification","post","session"] {
        let rows = [
            (Code::InvalidArgument,400,"Invalid input"),
            (Code::Unauthenticated,401,"Invalid credentials"),
            (Code::PermissionDenied,403,"Forbidden"),
            (Code::NotFound,404,"Not found"),
            (Code::AlreadyExists,409,"Resource already exists"),
            (Code::Aborted,409,"Operation conflict; retry"),
            (Code::ResourceExhausted,429,"Too many requests"),
            (Code::FailedPrecondition,400,"Request is not valid in the current state"),
            (Code::Unavailable,503,"Service unavailable"),
            (Code::DeadlineExceeded,504,"Downstream request timed out"),
            (Code::Ok,500,"Internal error"),
            (Code::Cancelled,500,"Internal error"),
            (Code::Unknown,500,"Internal error"),
            (Code::OutOfRange,500,"Internal error"),
            (Code::Unimplemented,500,"Internal error"),
            (Code::Internal,500,"Internal error"),
            (Code::DataLoss,500,"Internal error"),
        ];
        let mut cases = Vec::new();
        for (code,status,message) in rows {
            // A grpc OK with no reply becomes a client decode fault, not an error-code projection.
            *peer.status.lock().unwrap() = tonic::Status::new(code, if code==Code::InvalidArgument {"Invalid input"} else {"private-post-diagnostic"});
            let before = peer.calls.load(Ordering::SeqCst);
            let error = probe.call(port).await.expect_err("controlled wire failure");
            let delta = peer.calls.load(Ordering::SeqCst) - before;
            probe_calls += 1;
            let may_retry = code == Code::Unavailable && probe_may_retry(probe_methods, port);
            if may_retry { retry_permits += 1; }
            assert_probe_calls(port, &format!("code-{code:?}"), delta, may_retry);
            let response = map_service_error(error).into_response();
            assert_eq!(response.status().as_u16(),status,"{code:?}");
            assert!(response.headers().get(http::header::RETRY_AFTER).is_none());
            let bytes=axum::body::to_bytes(response.into_body(),1<<20).await?;
            let body:Value=serde_json::from_slice(&bytes)?;
            assert_eq!(body["message"],message);
            assert!(!body.to_string().contains("private-post-diagnostic"));
            cases.push(json!({"port":port,"case":format!("code-{code:?}"),"status":status,"message":message}));
        }
        for (value,message) in [
            ("unknown","Operation outcome is unknown; reconcile using the original operation ID and key"),
            ("expired","Operation replay window expired; reconcile before issuing another command"),
            ("conflict","Operation identity or request conflicts with its durable receipt"),
            ("UNKNOWN","Resource already exists"),
        ] {
            let mut status=tonic::Status::already_exists("private-owner-diagnostic");
            status.metadata_mut().insert(platform::mutation_context::ERROR_CODE,value.parse()?);
            *peer.status.lock().unwrap()=status;
            let before=peer.calls.load(Ordering::SeqCst);
            let error=probe.call(port).await.expect_err("owner conflict");
            let delta=peer.calls.load(Ordering::SeqCst)-before;
            probe_calls += 1;
            assert_probe_calls(port, &format!("owner-{value}"), delta, false);
            let diagnostic=format!("{error:?}");
            let response=map_service_error(error).into_response();
            assert_eq!(response.status().as_u16(),409,"port={port}, metadata={value}, error={diagnostic}");
            let body:Value=serde_json::from_slice(&axum::body::to_bytes(response.into_body(),1<<20).await?)?;
            assert_eq!(body["message"],message);
            assert!(!body.to_string().contains("private-owner-diagnostic"));
            cases.push(json!({"port":port,"case":format!("owner-{value}"),"status":409,"message":message}));
        }
        for message in ["AUTH_BUSY","not AUTH_BUSY","auth_busy","AUTH_DEPENDENCY_UNAVAILABLE"] {
            *peer.status.lock().unwrap()=tonic::Status::unavailable(message);
            let before=peer.calls.load(Ordering::SeqCst);
            let error=probe.call(port).await.expect_err("unavailable");
            let delta=peer.calls.load(Ordering::SeqCst)-before;
            probe_calls += 1;
            let may_retry = probe_may_retry(probe_methods, port);
            if may_retry { retry_permits += 1; }
            assert_probe_calls(port, &format!("busy-{message}"), delta, may_retry);
            let diagnostic=format!("{error:?}");
            let response=map_service_error(error).into_response();
            assert_eq!(response.status().as_u16(),503,"port={port}, message={message}, error={diagnostic}");
            let busy=message=="AUTH_BUSY";
            assert_eq!(response.headers().get(http::header::RETRY_AFTER).map(|v|v.to_str().unwrap()),if busy {Some("1")} else {None});
            let body:Value=serde_json::from_slice(&axum::body::to_bytes(response.into_body(),1<<20).await?)?;
            let projected=if busy {"Service busy; retry later"} else {"Service unavailable"};
            assert_eq!(body["message"],projected);
            cases.push(json!({"port":port,"case":format!("busy-{message}"),"status":503,"message":projected,"retryAfter":if busy {Some("1")} else {None}}));
        }
        for (name,message,expected) in [("bounded",format!("  public\n{}  ","x".repeat(400)),format!("public{}","x".repeat(248))),("empty","\n\r\t  ".into(),"Invalid request".into())] {
            *peer.status.lock().unwrap()=tonic::Status::invalid_argument(message);
            let before=peer.calls.load(Ordering::SeqCst);
            let error=probe.call(port).await.expect_err("invalid argument");
            let delta=peer.calls.load(Ordering::SeqCst)-before;
            probe_calls += 1;
            assert_probe_calls(port, &format!("input-{name}"), delta, false);
            let diagnostic=format!("{error:?}");
            let response=map_service_error(error).into_response();
            assert_eq!(response.status().as_u16(),400,"port={port}, input={name}, error={diagnostic}");
            let body:Value=serde_json::from_slice(&axum::body::to_bytes(response.into_body(),1<<20).await?)?;
            assert_eq!(body["message"],expected);
            assert!(body["message"].as_str().unwrap().chars().count()<=256);
            cases.push(json!({"port":port,"case":format!("input-{name}"),"status":400,"message":expected}));
        }

            for case in cases {println!("SERVICE_FAILURE_HTTP {case}");}
        }
        let total=peer.calls.load(Ordering::SeqCst);
        assert!(
            total>=probe_calls,
            "every probe case must reach the peer once: {total} < {probe_calls}"
        );
        assert!(
            total<=probe_calls+retry_permits,
            "only a replay-safe UNAVAILABLE may add a call: {total} > {probe_calls}+{retry_permits}"
        );
        anyhow::Ok(())
    }.await;
    let _ = stop.send(());
    server.await??;
    result
}
