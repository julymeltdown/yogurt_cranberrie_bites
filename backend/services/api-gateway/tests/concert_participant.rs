//! C03 Gateway boundary with controlled signed transport peers; no native/public-stack acceptance.
#[allow(dead_code)]
#[path = "unit/http_support.rs"]
mod gateway_support;
#[path = "concert_participant_support/identity.rs"]
mod identity;
use api_gateway::{
    adapters::{
        out::{
            concert_participant::{self as client, proto, Operation},
            concert_product::proto as product,
        },
        r#in::http::{concert_participant as http, RequestDeadline},
    },
    config::AppState,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use platform::{
    actor_delegation::{
        response_scope::RetainedScopeLayer, ActorDelegationSigner, ActorDelegationVerifier,
        VerifiedActor,
    },
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, VerifiedCaller, Workload},
    grpc_method_policy::{guard, MethodPolicy},
};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::oneshot;
use tonic::{Request as RpcRequest, Response as RpcResponse, Status};
use tower::ServiceExt;
use uuid::Uuid;
const EVENT: &str = "a06544cf-fd5c-4a98-bdbf-5846739651ca";
const CREDENTIAL: &str = "participant-transport-only";
#[derive(Default)]
struct Observed {
    requests: Mutex<Vec<proto::ParticipantRequest>>,
    completions: AtomicUsize,
    mode: AtomicUsize,
}
#[derive(Clone)]
struct Peer(Arc<Observed>);
fn cap() -> product::ResponseDelivery {
    product::ResponseDelivery {
        instance_id: Uuid::new_v4().to_string(),
        scope_id: Uuid::new_v4().to_string(),
        binding_digest: vec![6; 32],
        completion_secret: vec![7; 32],
    }
}
fn avatar() -> serde_json::Value {
    serde_json::json!({"revision":1,"config":{"contract_version":"1.1.0","kind":"avatar_config","manifest_id":"mcr_selected_static_20261103","manifest_version":1,"representation":"whole_character","preset_id":"photo01_front_01"}})
}
fn document() -> serde_json::Value {
    serde_json::json!({"event_id":EVENT,"user_id":identity::USER,"revision":1,"profile":{"user_id":identity::USER,"display_name":"Mira","bio":null,"avatar":avatar()},"preferences":{"theme":"PARADE","low_power":false,"reduced_motion":false,"sound_enabled":true},"age_status":"UNKNOWN"})
}
impl Peer {
    fn respond(
        &self,
        r: RpcRequest<proto::ParticipantRequest>,
        avatar_only: bool,
    ) -> Result<RpcResponse<proto::ParticipantResponse>, Status> {
        assert_eq!(
            r.extensions().get::<VerifiedCaller>().unwrap().0,
            Workload::Gateway
        );
        assert_eq!(
            r.extensions().get::<VerifiedActor>().unwrap().0.to_string(),
            r.get_ref().user_id
        );
        assert!(!r.metadata().contains_key("authorization"));
        self.0.requests.lock().unwrap().push(r.into_inner());
        let mode = self.0.mode.load(Ordering::SeqCst);
        let mut result = proto::ParticipantResponse {
            response_json: serde_json::to_vec(&if avatar_only { avatar() } else { document() })
                .unwrap(),
            delivery: Some(cap()),
            failure: None,
        };
        if mode == 1 {
            result.response_json.clear();
            result.failure = Some(proto::ParticipantFailure {
                grpc_code: tonic::Code::Aborted as u32,
                core_reason: "VERSION_CONFLICT".into(),
            });
        }
        if mode == 2 {
            result.response_json =
                serde_json::to_vec(&serde_json::json!({"private":"PRIVATE_OWNER_CANARY"})).unwrap();
        }
        let mut response = RpcResponse::new(result);
        response
            .metadata_mut()
            .insert("cache-control", "no-store".parse().unwrap());
        Ok(response)
    }
}
#[tonic::async_trait]
impl proto::concert_participant_service_server::ConcertParticipantService for Peer {
    async fn get_participant(
        &self,
        r: RpcRequest<proto::ParticipantRequest>,
    ) -> Result<RpcResponse<proto::ParticipantResponse>, Status> {
        self.respond(r, false)
    }
    async fn set_avatar(
        &self,
        r: RpcRequest<proto::ParticipantRequest>,
    ) -> Result<RpcResponse<proto::ParticipantResponse>, Status> {
        self.respond(r, true)
    }
    async fn update_preferences(
        &self,
        r: RpcRequest<proto::ParticipantRequest>,
    ) -> Result<RpcResponse<proto::ParticipantResponse>, Status> {
        self.respond(r, false)
    }
}
#[tonic::async_trait]
impl product::product_delivery_service_server::ProductDeliveryService for Peer {
    async fn complete_response(
        &self,
        r: RpcRequest<product::ResponseDelivery>,
    ) -> Result<RpcResponse<product::CompleteResponseResult>, Status> {
        assert_eq!(r.get_ref().completion_secret, vec![7; 32]);
        assert!(!r
            .metadata()
            .contains_key(platform::actor_delegation::HEADER));
        self.0.completions.fetch_add(1, Ordering::SeqCst);
        Ok(RpcResponse::new(product::CompleteResponseResult {
            completed: true,
        }))
    }
}
const POLICY: MethodPolicy = MethodPolicy {
    service: "concert.participant.v1.ConcertParticipantService",
    workload: Some(Workload::Concert),
    methods: &[
        ("GetParticipant", &[Workload::Gateway]),
        ("SetAvatar", &[Workload::Gateway]),
        ("UpdatePreferences", &[Workload::Gateway]),
    ],
    actor_fields: &[
        ("GetParticipant", &[&[1]]),
        ("SetAvatar", &[&[1]]),
        ("UpdatePreferences", &[&[1]]),
    ],
};
struct Fixture {
    gateway: gateway_support::RouteHarness,
    owners: identity::Running,
    seen: Arc<Observed>,
    router: Router,
    stop: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}
impl Fixture {
    async fn new() -> Self {
        let mut gateway = gateway_support::RouteHarness::new().await;
        let owners = identity::Running::start().await;
        Arc::get_mut(&mut gateway.state).unwrap().concert_identity = Some(owners.reader());
        let seen = Arc::new(Observed::default());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let service = guard(
            &POLICY,
            proto::concert_participant_service_server::ConcertParticipantServiceServer::new(Peer(
                seen.clone(),
            )),
            CallerRegistry::new()
                .register(Workload::Gateway, CREDENTIAL)
                .unwrap(),
            ActorDelegationVerifier::local_parity(Workload::Concert),
        )
        .unwrap();
        let completion=product::product_delivery_service_server::ProductDeliveryServiceServer::with_interceptor(Peer(seen.clone()),platform::grpc_auth::RequireRegisteredCallerInterceptor::new(CallerRegistry::new().register(Workload::Gateway,CREDENTIAL).unwrap()).unwrap());
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .layer(CallDeadlineLayer)
                .add_service(service)
                .add_service(completion)
                .serve_with_incoming_shutdown(
                    tokio_stream::wrappers::TcpListenerStream::new(listener),
                    async {
                        let _ = stopped.await;
                    },
                )
                .await
                .unwrap();
        });
        let rpc = client::GrpcConcertParticipantClient::connect(
            &endpoint,
            CREDENTIAL,
            None,
            ActorDelegationSigner::local_parity(Workload::Gateway),
            &"a".repeat(64),
        )
        .unwrap();
        let router = http::router(gateway.state.clone(), Arc::new(rpc))
            .layer(axum::Extension(RequestDeadline(
                (tokio::time::Instant::now() + Duration::from_secs(2)).into_std(),
            )))
            .layer(axum::middleware::from_fn(http::protect_response))
            .layer(RetainedScopeLayer);
        Self {
            gateway,
            owners,
            seen,
            router,
            stop,
            task,
        }
    }
    fn request(&self, method: &str, suffix: &str, body: &str) -> Request<Body> {
        let mut r = Request::builder()
            .method(method)
            .uri(format!("/api/concert/events/{EVENT}/me{suffix}"))
            .header("authorization", self.gateway.bearer_token(identity::USER));
        if method != "GET" {
            r = r
                .header(
                    "origin",
                    self.gateway.state.browser_auth_origins.first().unwrap(),
                )
                .header("x-mcr-intent", "browser")
                .header("content-type", "application/json");
        }
        r.body(Body::from(body.to_owned())).unwrap()
    }
    async fn close(self) {
        drop(self.router);
        let _ = self.stop.send(());
        tokio::time::timeout(Duration::from_secs(2), self.task)
            .await
            .unwrap()
            .unwrap();
        self.owners.close().await;
        self.gateway.pool.close().await;
    }
}
#[tokio::test]
async fn actual_sealed_http_access_and_signed_identity_forward_original_c03_bytes() {
    let h = Fixture::new().await;
    for (method, suffix, body) in [
        ("GET", "", ""),
        ("PUT", "/avatar", r#"{ "expected_revision":1,"config":{} }"#),
        (
            "PATCH",
            "",
            r#"{"expected_revision":1,"display_name":"New name"}"#,
        ),
    ] {
        let r = h
            .router
            .clone()
            .oneshot(h.request(method, suffix, body))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(r.headers()["cache-control"], "no-store");
        let before = h.seen.completions.load(Ordering::SeqCst);
        let mut b = r.into_body();
        assert!(b.frame().await.unwrap().unwrap().is_data());
        assert_eq!(h.seen.completions.load(Ordering::SeqCst), before);
        assert!(b.frame().await.is_none());
        assert_eq!(h.seen.completions.load(Ordering::SeqCst), before + 1);
        let seen = h.seen.requests.lock().unwrap();
        let last = seen.last().unwrap();
        assert_eq!(last.user_id, identity::USER);
        assert_eq!(last.http_method, method);
        assert_eq!(
            last.http_target,
            format!("/api/concert/events/{EVENT}/me{suffix}")
        );
        assert_eq!(last.request_json, body.as_bytes());
        assert_eq!(last.release_digest, "a".repeat(64));
        assert_eq!(last.session_family_id, gateway_support::SESSION_ID);
    }
    assert_eq!(h.owners.auth_reads(), 3);
    h.close().await;
}
#[tokio::test]
async fn typed_conflict_keeps_delivery_but_dropped_and_invalid_bodies_never_complete() {
    let h = Fixture::new().await;
    h.seen.mode.store(1, Ordering::SeqCst);
    let r = h
        .router
        .clone()
        .oneshot(h.request("PATCH", "", r#"{"expected_revision":1}"#))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CONFLICT);
    let bytes = to_bytes(r.into_body(), 16384).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["details"]["reason"], "VERSION_CONFLICT");
    assert_eq!(h.seen.completions.load(Ordering::SeqCst), 1);
    assert!(!String::from_utf8_lossy(&bytes).contains("completion_secret"));
    h.seen.mode.store(0, Ordering::SeqCst);
    let r = h
        .router
        .clone()
        .oneshot(h.request("GET", "", ""))
        .await
        .unwrap();
    drop(r);
    tokio::task::yield_now().await;
    assert_eq!(h.seen.completions.load(Ordering::SeqCst), 1);
    h.seen.mode.store(2, Ordering::SeqCst);
    let r = h
        .router
        .clone()
        .oneshot(h.request("GET", "", ""))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
    let bytes = to_bytes(r.into_body(), 16384).await.unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("PRIVATE_OWNER_CANARY"));
    assert_eq!(h.seen.completions.load(Ordering::SeqCst), 1);
    h.close().await;
}
#[tokio::test]
async fn auth_browser_origin_body_limits_and_noncanonical_paths_refuse_before_owner_dispatch() {
    let h = Fixture::new().await;
    let mut cases = Vec::new();
    let mut r = h.request("GET", "", "");
    r.headers_mut().remove("authorization");
    cases.push((r, StatusCode::UNAUTHORIZED));
    let mut r = h.request("PATCH", "", "{}");
    r.headers_mut().remove("origin");
    cases.push((r, StatusCode::FORBIDDEN));
    let mut r = h.request("PUT", "/avatar", "{}");
    r.headers_mut().remove("x-mcr-intent");
    cases.push((r, StatusCode::FORBIDDEN));
    cases.push((h.request("GET", "?foreign=1", ""), StatusCode::BAD_REQUEST));
    cases.push((
        h.request("PATCH", "", &"x".repeat(client::MAX_BODY + 1)),
        StatusCode::PAYLOAD_TOO_LARGE,
    ));
    for (r, status) in cases {
        let response = h.router.clone().oneshot(r).await.unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let _ = to_bytes(response.into_body(), 16384).await.unwrap();
    }
    assert!(h.seen.requests.lock().unwrap().is_empty());
    assert_eq!(h.owners.auth_reads(), 0);
    h.close().await;
}
#[test]
fn response_allowlist_rejects_cross_identity_private_content_and_mismatched_failure_pairs() {
    let actor = Uuid::parse_str(identity::USER).unwrap();
    let event = Uuid::parse_str(EVENT).unwrap();
    let original = document();
    assert!(client::validate_document(
        &serde_json::to_vec(&original).unwrap(),
        Operation::Read,
        actor,
        event
    )
    .is_ok());
    for path in ["user_id", "event_id"] {
        let mut v = original.clone();
        v[path] = Uuid::new_v4().to_string().into();
        assert!(client::validate_document(
            &serde_json::to_vec(&v).unwrap(),
            Operation::Read,
            actor,
            event
        )
        .is_err());
    }
    let mut v = original;
    v["profile"]["email"] = "PRIVATE_OWNER_CANARY".into();
    assert!(client::validate_document(
        &serde_json::to_vec(&v).unwrap(),
        Operation::Read,
        actor,
        event
    )
    .is_err());
    assert!(client::failure_status(&proto::ParticipantFailure {
        grpc_code: tonic::Code::Aborted as u32,
        core_reason: "PERMISSION_DENIED".into()
    })
    .is_err());
    assert!(client::GrpcConcertParticipantClient::connect(
        "http://127.0.0.1:1",
        CREDENTIAL,
        None,
        ActorDelegationSigner::local_parity(Workload::Gateway),
        "bad-release"
    )
    .is_err());
}
