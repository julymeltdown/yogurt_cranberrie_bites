//! Actual Gateway router and an owned mTLS Participant response scheduler.
//! Forward the original signed request and genuine owner capability unchanged.
use super::product;
use api_gateway::{
    adapters::{
        out::concert_participant::{self as participant, proto},
        r#in::http::{self, client_identity::TrustedProxies, server::with_transport_layers},
    },
    config::{build_state, connect_pool, AppConfig},
};
use axum::{
    body::Body,
    extract::{Request, State},
    middleware::{from_fn_with_state, Next},
    response::Response,
};
use http_body::{Body as HttpBody, Frame, SizeHint};
use platform::{
    actor_delegation::ActorDelegationVerifier,
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::{guard, MethodPolicy},
    grpc_tls::TlsMaterial,
};
use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};
use tonic::{transport::Channel, Request as RpcRequest, Response as RpcResponse, Status};
use uuid::Uuid;
fn env(name: &str) -> anyhow::Result<String> {
    Ok(std::env::var(name)?)
}
fn mode(root: &Path) -> usize {
    std::fs::read(root.join("retention-mode.json"))
        .ok()
        .and_then(|v| serde_json::from_slice::<usize>(&v).ok())
        .unwrap_or(0)
}
fn marker(root: &Path, name: &str, value: serde_json::Value) -> anyhow::Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(root.join(name))?;
    file.write_all(&serde_json::to_vec(&value)?)?;
    file.sync_all()?;
    Ok(())
}
#[derive(Clone)]
struct Proxy {
    participant:
        proto::concert_participant_service_client::ConcertParticipantServiceClient<Channel>,
    delivery: product::product_delivery_service_client::ProductDeliveryServiceClient<Channel>,
    root: Arc<PathBuf>,
}
impl Proxy {
    async fn forward(
        &self,
        request: RpcRequest<proto::ParticipantRequest>,
        operation: participant::Operation,
    ) -> Result<RpcResponse<proto::ParticipantResponse>, Status> {
        let mode = mode(&self.root);
        let original = request.get_ref().authority_expires_unix_micros;
        let mut client = self.participant.clone();
        // Metadata, actor seal, request bytes and original timeout are forwarded;
        // no fresh actor/deadline/capability is minted by this fixture.
        let mut reply = match operation {
            participant::Operation::Read => client.get_participant(request).await?,
            participant::Operation::Avatar => client.set_avatar(request).await?,
            participant::Operation::Update => client.update_preferences(request).await?,
        };
        if mode != 0 {
            let value = reply.get_ref();
            let cap = value
                .delivery
                .as_ref()
                .ok_or_else(|| Status::internal("actual delivery missing"))?;
            if value.content_expires_unix_micros <= 0
                || value.content_expires_unix_micros > original
            {
                return Err(Status::internal("actual bound invalid"));
            }
            marker(&self.root,&format!("retention-reply-{mode}.json"),serde_json::json!({"scope_id":cap.scope_id,"instance_id":cap.instance_id,"original_expiry":original,"actual_content_expiry":value.content_expires_unix_micros,"actual_delivery":true})).map_err(|_|Status::internal("private fixture observation"))?;
            let short = chrono::Utc::now().timestamp_micros() + 20_000;
            let ceiling = short.min(value_expiry(&reply));
            reply.get_mut().content_expires_unix_micros = ceiling;
            tokio::time::sleep(Duration::from_millis(35)).await;
            if mode == 5 {
                let mut document: serde_json::Value =
                    serde_json::from_slice(&reply.get_ref().response_json)
                        .map_err(|_| Status::internal("fixture document"))?;
                document["user_id"] = Uuid::new_v4().to_string().into();
                reply.get_mut().response_json = serde_json::to_vec(&document)
                    .map_err(|_| Status::internal("fixture document"))?;
            }
        }
        Ok(reply)
    }
}
fn value_expiry(reply: &RpcResponse<proto::ParticipantResponse>) -> i64 {
    reply.get_ref().content_expires_unix_micros
}
#[tonic::async_trait]
impl proto::concert_participant_service_server::ConcertParticipantService for Proxy {
    async fn get_participant(
        &self,
        r: RpcRequest<proto::ParticipantRequest>,
    ) -> Result<RpcResponse<proto::ParticipantResponse>, Status> {
        self.forward(r, participant::Operation::Read).await
    }
    async fn set_avatar(
        &self,
        r: RpcRequest<proto::ParticipantRequest>,
    ) -> Result<RpcResponse<proto::ParticipantResponse>, Status> {
        self.forward(r, participant::Operation::Avatar).await
    }
    async fn update_preferences(
        &self,
        r: RpcRequest<proto::ParticipantRequest>,
    ) -> Result<RpcResponse<proto::ParticipantResponse>, Status> {
        self.forward(r, participant::Operation::Update).await
    }
}
#[tonic::async_trait]
impl product::product_delivery_service_server::ProductDeliveryService for Proxy {
    async fn complete_response(
        &self,
        r: RpcRequest<product::ResponseDelivery>,
    ) -> Result<RpcResponse<product::CompleteResponseResult>, Status> {
        self.delivery.clone().complete_response(r).await
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
struct GateBody {
    inner: Body,
    root: Arc<PathBuf>,
    mode: usize,
    sent: bool,
    wait: Pin<Box<dyn Future<Output = ()> + Send>>,
}
impl Drop for GateBody {
    fn drop(&mut self) {
        let _ = marker(
            &self.root,
            &format!("retention-body-dropped-{}.json", self.mode),
            serde_json::json!({"partial_data":self.sent}),
        );
    }
}
impl HttpBody for GateBody {
    type Data = axum::body::Bytes;
    type Error = axum::Error;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        if self.mode == 3 || self.sent {
            let _ = self.wait.as_mut().poll(cx);
            return Poll::Pending;
        }
        match Pin::new(&mut self.inner).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => match frame.into_data() {
                Ok(mut bytes) => {
                    self.sent = true;
                    let half = bytes.len() / 2;
                    let first = bytes.split_to(half.max(1));
                    Poll::Ready(Some(Ok(Frame::data(first))))
                }
                Err(frame) => Poll::Ready(Some(Ok(frame))),
            },
            other => other,
        }
    }
    fn is_end_stream(&self) -> bool {
        false
    }
    fn size_hint(&self) -> SizeHint {
        SizeHint::default()
    }
}
async fn gate(State(root): State<Arc<PathBuf>>, request: Request, next: Next) -> Response {
    let selected = mode(&root);
    let participant = request.uri().path().ends_with("/me");
    let response = next.run(request).await;
    if !participant || !matches!(selected, 3 | 4) {
        return response;
    }
    let (parts, inner) = response.into_parts();
    Response::from_parts(
        parts,
        Body::new(GateBody {
            inner,
            root,
            mode: selected,
            sent: false,
            wait: Box::pin(std::future::pending()),
        }),
    )
}
#[tokio::test]
#[ignore = "explicit copied binary and owned native PG16/mTLS controller only"]
async fn actual_native_participant_delivery_retention_host() -> anyhow::Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let root = PathBuf::from(env("MCR_RETENTION_ROOT")?);
    let metadata = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        root.is_absolute()
            && metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && metadata.permissions().mode() & 0o077 == 0
    );
    anyhow::ensure!(
        format!("{}:{}", metadata.dev(), metadata.ino()) == env("MCR_RETENTION_ROOT_IDENTITY")?
    );
    let owner: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("mcr-owner.json"))?)?;
    anyhow::ensure!(
        owner["token"].as_str() == Some(env("MCR_RETENTION_ROOT_TOKEN")?.as_str())
            && owner["uid"].as_u64() == Some(metadata.uid() as u64)
    );
    let root = Arc::new(root);
    let config = AppConfig::load()?;
    let actual = config
        .concert_grpc_endpoint
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("explicit native Concert endpoint"))?;
    let tls = config
        .tls_material
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("native mTLS required"))?;
    let channel = tls.connect(actual, Workload::Concert).await?;
    let proxy = Proxy {
        participant:
            proto::concert_participant_service_client::ConcertParticipantServiceClient::new(
                channel.clone(),
            ),
        delivery: product::product_delivery_service_client::ProductDeliveryServiceClient::new(
            channel,
        ),
        root: root.clone(),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("https://{}", listener.local_addr()?);
    let registry = CallerRegistry::new().register(Workload::Gateway, &config.outbound_identity)?;
    let participant = guard(
        &POLICY,
        proto::concert_participant_service_server::ConcertParticipantServiceServer::new(
            proxy.clone(),
        ),
        registry.clone(),
        ActorDelegationVerifier::new(
            Workload::Concert,
            &[config.actor_delegation_signer.public_key()],
        )?,
    )?;
    let delivery =
        product::product_delivery_service_server::ProductDeliveryServiceServer::with_interceptor(
            proxy,
            platform::grpc_auth::RequireRegisteredCallerInterceptor::new(registry)?,
        );
    let directory = root.join("pki");
    let server_tls = TlsMaterial::load(
        directory.join("concert.crt"),
        directory.join("concert.key"),
        directory.join("ca.crt"),
    )?;
    let server = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .tls_config(server_tls.server_config().unwrap())
            .unwrap()
            .add_service(participant)
            .add_service(delivery)
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
    });
    let mut state = build_state(&config, connect_pool(&config).await?).await?;
    state.concert_participant_client = Some(Arc::new(
        participant::GrpcConcertParticipantClient::connect(
            &endpoint,
            &config.outbound_identity,
            Some(tls),
            config.actor_delegation_signer.clone(),
            config
                .concert_release_digest
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("explicit release"))?,
        )?,
    ));
    let application = http::router(Arc::new(state)).layer(from_fn_with_state(root.clone(), gate));
    let router = with_transport_layers(application, &config, TrustedProxies::parse("")?);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let http = tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
    });
    marker(
        &root,
        "retention-host-ready.json",
        serde_json::json!({"http_url":format!("http://{address}"),"genuine_owner_transport":true}),
    )?;
    tokio::time::timeout(Duration::from_secs(90), async {
        while !root.join("retention-stop.json").is_file() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    http.abort();
    let _ = http.await;
    server.abort();
    let _ = server.await;
    Ok(())
}
