// Derived from concert_participant.rs at c9dd4c980f39b12eda5a79fff871c8de48f8df71.
// Controlled signed transport peers; native qualification is separate.
#[allow(dead_code)]
#[path = "../unit/http_support.rs"]
mod gateway_support;
#[path = "../concert_participant_support/identity.rs"]
mod identity;
use api_gateway::adapters::{
    out::{
        concert_participant::{self as client, proto, Operation},
        concert_product::proto as product,
    },
    r#in::http::{concert_participant as http, RequestDeadline},
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
        let expires = r.get_ref().authority_expires_unix_micros;
        self.0.requests.lock().unwrap().push(r.into_inner());
        let mode = self.0.mode.load(Ordering::SeqCst);
        let mut result = proto::ParticipantResponse {
            response_json: serde_json::to_vec(&if avatar_only { avatar() } else { document() })
                .unwrap(),
            delivery: Some(cap()),
            failure: None,
            content_expires_unix_micros: if mode == 4 {
                0
            } else if matches!(mode, 5 | 6 | 7 | 8 | 9) {
                // A structurally valid shorter ceiling already elapsed when decoded.
                // Original request/access deadline remains unmodified.
                chrono::Utc::now().timestamp_micros() - 1
            } else if mode == 3 {
                expires.min(chrono::Utc::now().timestamp_micros() + 250_000)
            } else {
                expires
            },
        };
        if matches!(mode, 1 | 6) {
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
        if mode == 7 {
            let mut wrong = document();
            wrong["user_id"] = Uuid::new_v4().to_string().into();
            result.response_json = serde_json::to_vec(&wrong).unwrap();
        }
        if mode == 8 {
            result.delivery.as_mut().unwrap().completion_secret.clear();
        }
        let mut response = RpcResponse::new(result);
        response
            .metadata_mut()
            .insert("cache-control", "no-store".parse().unwrap());
        if mode == 9 {
            response.metadata_mut().remove("cache-control");
        }
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
        let mut gateway = Box::pin(gateway_support::RouteHarness::new()).await;
        let owners = Box::pin(identity::Running::start()).await;
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
