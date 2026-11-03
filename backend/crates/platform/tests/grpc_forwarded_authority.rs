//! Real mTLS and protobuf transport for the child-proof guard. These are
//! recording owner fixtures, not production owner or database acceptance.
use platform::actor_delegation::exchange::{
    ChildRequest, DelegationExchange, ParentRequest, ResourceEvidenceVerifier,
};
use platform::actor_delegation::{
    ActorDelegationSigner, ActorDelegationVerifier, VerifiedActor, HEADER,
};
use platform::grpc_auth::{CallerRegistry, Workload};
use platform::grpc_method_policy::{guard_with_forwarding, ForwardedMethod, MethodPolicy};
use platform::grpc_tls::{workload_dns_name, workload_uri, TlsMaterial};
use prost::Message;
use rcgen::{
    BasicConstraints, CertificateParams, ExtendedKeyUsagePurpose, IsCa, KeyPair, KeyUsagePurpose,
};
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::body::Body;
use tonic::transport::{Channel, Server};
use tonic::{Code, Request, Response, Status};
use uuid::Uuid;

const PROFILE: &str = "/authority.v1.ProfileAuthorAuthorityService/CheckAuthorProfile";
const FOLLOWING: &str = "/friend.v1.FriendService/ListFollowing";
static PROFILE_POLICY: MethodPolicy = MethodPolicy {
    service: "authority.v1.ProfileAuthorAuthorityService",
    workload: Some(Workload::Profile),
    methods: &[("CheckAuthorProfile", &[Workload::Post])],
    actor_fields: &[],
};
static FRIEND_POLICY: MethodPolicy = MethodPolicy {
    service: "friend.v1.FriendService",
    workload: Some(Workload::Friend),
    methods: &[("ListFollowing", &[Workload::Post])],
    actor_fields: &[],
};
static PROFILE_PROOF: &[ForwardedMethod] = &[ForwardedMethod {
    method: "CheckAuthorProfile",
    caller: Workload::Post,
    actor_fields: &[],
}];
static FRIEND_PROOF: &[ForwardedMethod] = &[ForwardedMethod {
    method: "ListFollowing",
    caller: Workload::Post,
    actor_fields: &[&[1]],
}];
#[derive(Clone, PartialEq, Message)]
struct ReadRequest {
    #[prost(string, tag = "1")]
    user_id: String,
}
#[derive(Clone, PartialEq, Message)]
struct ReadResponse {
    #[prost(string, tag = "1")]
    verified_actor: String,
}

#[derive(Clone)]
struct Owner<const FRIEND: bool> {
    effects: Arc<AtomicUsize>,
}
impl<const FRIEND: bool> tonic::server::NamedService for Owner<FRIEND> {
    const NAME: &'static str = if FRIEND {
        "friend.v1.FriendService"
    } else {
        "authority.v1.ProfileAuthorAuthorityService"
    };
}
type RpcFuture = Pin<Box<dyn Future<Output = Result<Response<ReadResponse>, Status>> + Send>>;
impl<const FRIEND: bool> tonic::server::UnaryService<ReadRequest> for Owner<FRIEND> {
    type Response = ReadResponse;
    type Future = RpcFuture;
    fn call(&mut self, request: Request<ReadRequest>) -> Self::Future {
        let effects = self.effects.clone();
        Box::pin(async move {
            tokio::task::yield_now().await;
            let actor = request
                .extensions()
                .get::<VerifiedActor>()
                .ok_or_else(|| Status::internal("fixture did not receive verified actor"))?
                .0;
            effects.fetch_add(1, Ordering::SeqCst);
            Ok(Response::new(ReadResponse {
                verified_actor: actor.to_string(),
            }))
        })
    }
}
impl<const FRIEND: bool> tower::Service<http::Request<Body>> for Owner<FRIEND> {
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, request: http::Request<Body>) -> Self::Future {
        let owner = self.clone();
        Box::pin(async move {
            let codec = tonic_prost::ProstCodec::<ReadResponse, ReadRequest>::default();
            let mut grpc = tonic::server::Grpc::new(codec);
            Ok(grpc.unary(owner, request).await)
        })
    }
}
fn frame(message: &ReadRequest) -> Vec<u8> {
    let message = message.encode_to_vec();
    let mut bytes = vec![0];
    bytes.extend(u32::try_from(message.len()).unwrap().to_be_bytes());
    bytes.extend(message);
    bytes
}
fn issue(audience: Workload, method: &str, actor: Uuid, at: i64) -> String {
    let parent_method = if audience == Workload::Friend {
        "/post.v1.PostService/GetFeed"
    } else {
        "/post.v1.PostService/CreatePost"
    };
    let body = frame(&ReadRequest {
        user_id: actor.to_string(),
    });
    let signer = ActorDelegationSigner::local_parity(Workload::Gateway);
    let token = signer.issue(Workload::Post, parent_method, actor, None, &body, at);
    let exchange = DelegationExchange::new(
        signer,
        ActorDelegationVerifier::local_parity(Workload::Post),
        ResourceEvidenceVerifier::new(&[[11; 32]]).unwrap(),
    )
    .unwrap();
    let prepared = exchange
        .prepare(
            Workload::Post,
            ParentRequest {
                token: &token,
                method: parent_method,
                body: &body,
                operation: None,
            },
            ChildRequest {
                audience,
                method,
                body: &body,
                operation: None,
            },
            at,
        )
        .unwrap();
    exchange.finish(prepared, None, at).unwrap().token
}
fn materials() -> anyhow::Result<(TlsMaterial, TlsMaterial, TlsMaterial, TlsMaterial)> {
    let mut params = CertificateParams::new(vec![])?;
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    let key = KeyPair::generate()?;
    let ca = params.self_signed(&key)?;
    let issuer = rcgen::Issuer::from_params(&params, &key);
    let leaf = |workload: Workload| -> anyhow::Result<TlsMaterial> {
        let mut params = CertificateParams::new(vec![workload_dns_name(workload)])?;
        params
            .subject_alt_names
            .push(rcgen::SanType::URI(workload_uri(workload).try_into()?));
        params.extended_key_usages = vec![
            ExtendedKeyUsagePurpose::ServerAuth,
            ExtendedKeyUsagePurpose::ClientAuth,
        ];
        let key = KeyPair::generate()?;
        let cert = params.signed_by(&key, &issuer)?;
        Ok(TlsMaterial::from_pem(
            cert.pem(),
            key.serialize_pem(),
            ca.pem(),
        ))
    };
    Ok((
        leaf(Workload::Profile)?,
        leaf(Workload::Friend)?,
        leaf(Workload::Post)?,
        leaf(Workload::Gateway)?,
    ))
}
async fn call(
    channel: Channel,
    method: &'static str,
    actor: Uuid,
    token: Option<&str>,
    caller: Workload,
    credential: &str,
) -> Result<String, Code> {
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.map_err(|_| Code::Unavailable)?;
    let mut request = Request::new(ReadRequest {
        user_id: actor.to_string(),
    });
    request
        .metadata_mut()
        .insert("x-service-caller", caller.as_str().parse().unwrap());
    request
        .metadata_mut()
        .insert("x-service-token", credential.parse().unwrap());
    if let Some(token) = token {
        request
            .metadata_mut()
            .insert(HEADER, token.parse().unwrap());
    }
    let response: Result<Response<ReadResponse>, Status> = client
        .unary(
            request,
            http::uri::PathAndQuery::from_static(method),
            tonic_prost::ProstCodec::<ReadRequest, ReadResponse>::default(),
        )
        .await;
    response
        .map(|response| response.into_inner().verified_actor)
        .map_err(|status| status.code())
}

async fn run<const FRIEND: bool>() -> anyhow::Result<()> {
    let (profile, friend, post, gateway) = materials()?;
    let audience = if FRIEND {
        Workload::Friend
    } else {
        Workload::Profile
    };
    let method = if FRIEND { FOLLOWING } else { PROFILE };
    let (policy, proof) = if FRIEND {
        (&FRIEND_POLICY, FRIEND_PROOF)
    } else {
        (&PROFILE_POLICY, PROFILE_PROOF)
    };
    let registry = CallerRegistry::new()
        .register(Workload::Post, "post-credential")?
        .register(Workload::Gateway, "gateway-credential")?;
    let effects = Arc::new(AtomicUsize::new(0));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let owner_material = if FRIEND { friend } else { profile };
    let server = tokio::spawn(
        Server::builder()
            .tls_config(owner_material.server_config()?)?
            .add_service(guard_with_forwarding(
                policy,
                proof,
                Owner::<FRIEND> {
                    effects: effects.clone(),
                },
                registry,
                ActorDelegationVerifier::local_parity(audience),
            )?)
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = stopped.await;
            }),
    );
    let url = format!("https://{address}");
    let channel = post.connect(&url, audience).await?;
    let actor = Uuid::new_v4();
    let now = chrono::Utc::now().timestamp();
    let valid = issue(audience, method, actor, now);
    for (label, token, named, expected) in [
        ("missing", None, actor, Code::Unauthenticated),
        ("malformed", Some("garbage"), actor, Code::Unauthenticated),
        (
            "altered request",
            Some(valid.as_str()),
            Uuid::new_v4(),
            Code::PermissionDenied,
        ),
    ] {
        assert_eq!(
            call(
                channel.clone(),
                method,
                named,
                token,
                Workload::Post,
                "post-credential"
            )
            .await,
            Err(expected),
            "{label}"
        );
        assert_eq!(effects.load(Ordering::SeqCst), 0, "{label}");
    }
    let expired = issue(audience, method, actor, now - 100);
    assert_eq!(
        call(
            channel.clone(),
            method,
            actor,
            Some(&expired),
            Workload::Post,
            "post-credential"
        )
        .await,
        Err(Code::Unauthenticated)
    );
    let gateway_channel = gateway.connect(&url, audience).await?;
    assert_eq!(
        call(
            gateway_channel.clone(),
            method,
            actor,
            Some(&valid),
            Workload::Post,
            "post-credential"
        )
        .await,
        Err(Code::Unauthenticated),
        "Post credential with Gateway certificate"
    );
    assert_eq!(
        call(
            gateway_channel,
            method,
            actor,
            Some(&valid),
            Workload::Gateway,
            "gateway-credential"
        )
        .await,
        Err(Code::PermissionDenied),
        "valid credential/certificate but wrong child presenter"
    );
    assert_eq!(effects.load(Ordering::SeqCst), 0);
    assert_eq!(
        call(
            channel.clone(),
            method,
            actor,
            Some(&valid),
            Workload::Post,
            "post-credential"
        )
        .await,
        Ok(actor.to_string())
    );
    assert_eq!(effects.load(Ordering::SeqCst), 1);
    assert_eq!(
        call(
            channel,
            method,
            actor,
            Some(&valid),
            Workload::Post,
            "post-credential"
        )
        .await,
        Err(Code::Unauthenticated)
    );
    assert_eq!(effects.load(Ordering::SeqCst), 1);
    let _ = stop.send(());
    server.await??;
    Ok(())
}
#[tokio::test]
async fn profile_target_read_requires_child_proof_over_mtls() -> anyhow::Result<()> {
    run::<false>().await
}
#[tokio::test]
async fn friend_viewer_read_requires_child_proof_over_mtls() -> anyhow::Result<()> {
    run::<true>().await
}
