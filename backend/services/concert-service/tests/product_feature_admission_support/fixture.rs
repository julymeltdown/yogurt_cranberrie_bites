// Guard/deadline composition follows platform actor_delegation tests and the
// Notification fixture at base 29a42ca1. No owner-policy success is fabricated.
use concert_service::product_rpc::{
    proto::{product_service_server::ProductServiceServer, ProductRequest, ProductResponse},
    Operation, ProductOwner, ProductRpcService, VerifiedProduct, POLICY,
};
use http_body_util::{BodyExt, Full};
use platform::{
    actor_delegation::{
        exchange::VerifiedParentAuthority, ActorDelegationSigner, ActorDelegationVerifier, HEADER,
    },
    call_deadline::{self, CallDeadlineLayer},
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::{guard, GuardedService},
};
use prost::Message;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tonic::{
    body::Body,
    codegen::{http, Bytes},
    Code, Response, Status,
};
use tower::{Layer, ServiceExt};
use uuid::Uuid;

pub const METHOD: &str = "/concert.product.v1.ProductService/Dispatch";
pub const REFUSAL: &str = "recorded admission only";
pub fn signer() -> ActorDelegationSigner {
    ActorDelegationSigner::from_seed(Workload::Gateway, &[74; 32]).unwrap()
}
pub fn frame(request: &ProductRequest) -> Vec<u8> {
    let body = request.encode_to_vec();
    let mut frame = vec![0];
    frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
    frame.extend_from_slice(&body);
    frame
}
pub fn input(method: &str, suffix: &str, body: &str) -> ProductRequest {
    let event = Uuid::new_v4();
    let now = chrono::Utc::now().timestamp_micros();
    ProductRequest {
        user_id: Uuid::new_v4().to_string(),
        event_id: event.to_string(),
        session_family_id: Uuid::new_v4().to_string(),
        request_id: Uuid::new_v4().to_string(),
        http_method: method.into(),
        http_target: format!("/api/concert/events/{event}/{suffix}"),
        http_body: body.as_bytes().to_vec(),
        content_type: if body.is_empty() {
            String::new()
        } else {
            "application/json".into()
        },
        idempotency_key: if body.is_empty() {
            String::new()
        } else {
            Uuid::new_v4().to_string()
        },
        authority_expires_unix_micros: now + 1_900_000,
        access_expires_unix_micros: now + 1_900_000,
        auth_observed_at_unix_micros: now,
        profile_observed_at_unix_micros: now,
        trusted_network_bucket: vec![42; 32],
    }
}
pub fn signed(request: &ProductRequest, actor: Uuid, timeout: Option<&str>) -> http::Request<Body> {
    let bytes = frame(request);
    let proof = signer().issue(
        Workload::Concert,
        METHOD,
        actor,
        None,
        &bytes,
        chrono::Utc::now().timestamp(),
    );
    let mut builder = http::Request::builder()
        .uri(METHOD)
        .header("content-type", "application/grpc")
        .header("te", "trailers")
        .header("x-service-caller", "gateway")
        .header("x-service-token", "gateway-feature-fixture")
        .header(HEADER, proof);
    if let Some(timeout) = timeout {
        builder = builder.header("grpc-timeout", timeout);
    }
    builder
        .body(Body::new(Full::new(Bytes::from(bytes))))
        .unwrap()
}
pub fn wire(request: &ProductRequest) -> http::Request<Body> {
    signed(
        request,
        Uuid::parse_str(&request.user_id).unwrap(),
        Some("1800000u"),
    )
}
pub fn server(owner: Arc<Recorder>) -> GuardedService<ProductServiceServer<ProductRpcService>> {
    guard(
        &POLICY,
        ProductServiceServer::new(ProductRpcService::new(owner)),
        CallerRegistry::new()
            .register(Workload::Gateway, "gateway-feature-fixture")
            .unwrap(),
        ActorDelegationVerifier::new(Workload::Concert, &[signer().public_key()]).unwrap(),
    )
    .unwrap()
}
pub async fn call(
    server: GuardedService<ProductServiceServer<ProductRpcService>>,
    request: http::Request<Body>,
) -> (Code, String) {
    let response = CallDeadlineLayer.layer(server).oneshot(request).await;
    let response = response.unwrap();
    let headers = response.headers().clone();
    let collected = response.into_body().collect().await.unwrap();
    let trailers = collected.trailers();
    let status = Status::from_header_map(&headers)
        .or_else(|| trailers.and_then(Status::from_header_map))
        .expect("gRPC terminal status");
    (status.code(), status.message().to_owned())
}
#[derive(Clone, Debug)]
pub struct Admission {
    pub operation: Operation,
    pub request: ProductRequest,
    pub stop: Instant,
}
pub enum Mode {
    Refuse,
    Response(ProductResponse),
    Hold,
}
pub struct Recorder {
    pub admitted: Mutex<Vec<Admission>>,
    mode: Mode,
    pub release: tokio::sync::Semaphore,
    pub entered: tokio::sync::Semaphore,
    pub dropped: AtomicUsize,
}
impl Recorder {
    pub fn new(mode: Mode) -> Arc<Self> {
        Arc::new(Self {
            admitted: Mutex::new(vec![]),
            mode,
            release: tokio::sync::Semaphore::new(0),
            entered: tokio::sync::Semaphore::new(0),
            dropped: AtomicUsize::new(0),
        })
    }
    pub fn len(&self) -> usize {
        self.admitted.lock().unwrap().len()
    }
}
struct DropProbe<'a>(&'a AtomicUsize);
impl Drop for DropProbe<'_> {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[async_trait::async_trait]
impl ProductOwner for Recorder {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        request: ProductRequest,
    ) -> Result<Response<ProductResponse>, Status> {
        context.check()?;
        assert_eq!(context.actor().to_string(), request.user_id);
        assert_eq!(context.event().to_string(), request.event_id);
        assert_eq!(context.session().to_string(), request.session_family_id);
        assert_eq!(context.request().to_string(), request.request_id);
        assert_eq!(context.parent_authority().actor(), context.actor());
        assert_eq!(context.parent_authority().request().method, METHOD);
        assert_eq!(context.parent_authority().request().body, frame(&request));
        assert_eq!(
            ProductRequest::decode(context.parent_authority().message().unwrap()).unwrap(),
            request
        );
        let current =
            VerifiedParentAuthority::current().expect("original signed parent propagated");
        assert_eq!(
            current.parent_digest(),
            context.parent_authority().parent_digest()
        );
        assert_eq!(
            current.request_digest(),
            context.parent_authority().request_digest()
        );
        assert_eq!(
            call_deadline::current(),
            Some(context.deadline().stop().into_std())
        );
        assert!(
            context.deadline().expires_micros()
                <= request
                    .authority_expires_unix_micros
                    .min(request.access_expires_unix_micros)
        );
        assert!(context.trusted_pickup_context().is_some());
        self.admitted.lock().unwrap().push(Admission {
            operation: context.operation(),
            request,
            stop: context.deadline().stop().into_std(),
        });
        self.entered.add_permits(1);
        match &self.mode {
            Mode::Refuse => Err(Status::unavailable(REFUSAL)),
            Mode::Response(response) => Ok(Response::new(response.clone())),
            Mode::Hold => {
                let _drop = DropProbe(&self.dropped);
                self.release.acquire().await.unwrap().forget();
                Err(Status::unavailable(REFUSAL))
            }
        }
    }
}

/// A fresh private PKI directory is supplied only by the bounded smoke runner.
/// No ambient certificate/database configuration is consumed.
pub fn tls(workload: Workload) -> anyhow::Result<platform::grpc_tls::TlsMaterial> {
    use std::{
        os::unix::fs::{MetadataExt, PermissionsExt},
        path::PathBuf,
    };
    let root = PathBuf::from(std::env::var("MCR_FEATURE_PKI")?);
    let metadata = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        root.is_absolute()
            && metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && metadata.permissions().mode() & 0o077 == 0,
        "private PKI directory required"
    );
    anyhow::ensure!(
        format!("{}:{}", metadata.dev(), metadata.ino())
            == std::env::var("MCR_FEATURE_PKI_IDENTITY")?,
        "replaced PKI directory"
    );
    let marker: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("owner.json"))?)?;
    anyhow::ensure!(
        marker["uid"].as_u64() == Some(metadata.uid() as u64)
            && marker["token"].as_str() == Some(std::env::var("MCR_FEATURE_PKI_TOKEN")?.as_str()),
        "foreign PKI marker"
    );
    platform::grpc_tls::TlsMaterial::load(
        root.join(format!("{}.crt", workload.as_str())),
        root.join(format!("{}.key", workload.as_str())),
        root.join("ca.crt"),
    )
}
pub async fn bounded_enter(owner: &Recorder, count: u32) {
    tokio::time::timeout(Duration::from_secs(1), owner.entered.acquire_many(count))
        .await
        .unwrap()
        .unwrap()
        .forget();
}
