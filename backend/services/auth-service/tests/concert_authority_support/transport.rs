use super::native::ActualProducer;
use super::*;
use platform::{
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::MethodPolicy,
    grpc_tls::TlsMaterial,
};
use sqlx::PgPool;
use std::sync::Arc;
use tonic::{Request, Response, Status};
fn tls(role: Workload) -> anyhow::Result<TlsMaterial> {
    let p = std::path::PathBuf::from(std::env::var("MCR_AUTHORITY_ROOT")?).join("pki");
    TlsMaterial::load(
        p.join(format!("{}.crt", role.as_str())),
        p.join(format!("{}.key", role.as_str())),
        p.join("ca.crt"),
    )
}
const PRODUCER_POLICY: MethodPolicy = MethodPolicy {
    service: "authority.lease.v1.ProducerSettlementService",
    workload: Some(Workload::Concert),
    methods: &[
        ("ResolveTransaction", &[Workload::Auth, Workload::Friend]),
        ("ResolveScope", &[Workload::Auth, Workload::Friend]),
    ],
    actor_fields: &[],
};
struct Peer(ActualProducer);
#[tonic::async_trait]
impl proto::producer_settlement_service_server::ProducerSettlementService for Peer {
    async fn resolve_scope(
        &self,
        q: Request<proto::ResolveScopeRequest>,
    ) -> Result<Response<proto::ResolveScopeResponse>, Status> {
        self.0
            .resolve_scope(
                q.into_inner(),
                &OperationBudget::new(Instant::now() + Duration::from_secs(1))?,
            )
            .await
            .map(Response::new)
    }
    async fn resolve_transaction(
        &self,
        q: Request<proto::ResolveTransactionRequest>,
    ) -> Result<Response<proto::ResolveTransactionResponse>, Status> {
        self.0
            .resolve_transaction(
                q.into_inner(),
                &OperationBudget::new(Instant::now() + Duration::from_secs(1))?,
            )
            .await
            .map(Response::new)
    }
}
pub(super) struct Running(tokio::task::JoinHandle<()>);
impl Drop for Running {
    fn drop(&mut self) {
        self.0.abort();
    }
}
pub(super) async fn rpc_store(
    p: &PgPool,
    prod: &PgPool,
) -> anyhow::Result<(PgOwnerAuthority, Running)> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("https://{}", listener.local_addr()?);
    let registry = CallerRegistry::new()
        .register(Workload::Auth, "auth-fixture")?
        .register(Workload::Friend, "friend-fixture")?;
    let service = platform::grpc_method_policy::guard_without_delegation(
        &PRODUCER_POLICY,
        proto::producer_settlement_service_server::ProducerSettlementServiceServer::new(Peer(
            ActualProducer(prod.clone()),
        )),
        registry,
    )?;
    let server = tonic::transport::Server::builder()
        .tls_config(tls(Workload::Concert)?.server_config()?)?
        .add_service(service);
    let task = tokio::spawn(async move {
        server
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    let resolver = auth_service::concert_authority::RpcSettlementResolver::new(
        &tls(Workload::Auth)?,
        "auth-fixture",
        &[(proto::Producer::Concert, url)],
    )?;
    Ok((
        PgOwnerAuthority::new(p.clone(), Arc::new(resolver))?,
        Running(task),
    ))
}

#[tokio::test]
#[ignore = "requires owned PG16 fixture and mTLS certificates"]
async fn owner_rpc_requires_actual_workload_and_signed_acquisition() -> anyhow::Result<()> {
    use platform::{
        actor_delegation::{
            relationship_read::{RelationshipReadSigner, RelationshipReadVerifier},
            request_for, ActorDelegationSigner, ActorDelegationVerifier,
        },
        grpc_auth::ServiceIdentityClientInterceptor,
    };
    use proto::auth_owner_authority_service_client::AuthOwnerAuthorityServiceClient as Client;
    let p = native::pool(false).await?;
    let prod = native::pool(true).await?;
    native::setup(&p, &prod).await?;
    let (actor, _, family) = native::fixture(&p).await?;
    let b = native::binding(actor, family);
    let scope = native::scope(&prod, &b).await?;
    let (store, _producer) = rpc_store(&p, &prod).await?;
    let gateway = ActorDelegationSigner::from_seed(Workload::Gateway, &[8; 32])?;
    let gateway = ActorDelegationVerifier::new(Workload::Auth, &[gateway.public_key()])?;
    let signer = RelationshipReadSigner::from_seed(&[19; 32], &gateway)?;
    let verifier = RelationshipReadVerifier::new(Workload::Auth, &[signer.public_key()], &gateway)?;
    std::env::set_var("CONCERT_SERVICE_CREDENTIAL", "concert-fixture");
    std::env::set_var("POST_SERVICE_CREDENTIAL", "post-fixture");
    std::env::set_var("PROFILE_SERVICE_CREDENTIAL", "profile-fixture");
    let registry = auth_service::concert_authority::owner_caller_registry()?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("https://{}", listener.local_addr()?);
    let service = auth_service::adapters::r#in::grpc::concert_authority::guarded_server(
        store, registry, verifier,
    )?;
    let server = tonic::transport::Server::builder()
        .tls_config(tls(Workload::Auth)?.server_config()?)?
        .add_service(service);
    let _owner = Running(tokio::spawn(async move {
        server
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    }));
    let channel = tls(Workload::Concert)?
        .endpoint(&url, Workload::Auth)?
        .connect()
        .await?;
    let id = ServiceIdentityClientInterceptor::new(Workload::Concert, "concert-fixture")?;
    let mut unsigned = Client::with_interceptor(channel.clone(), id.clone());
    let query = proto::AcquireRequest {
        binding: Some(b.clone()),
        response_scope: Some(scope),
    };
    assert!(unsigned.acquire(query.clone()).await.is_err());
    let signed = signer.client(channel, Workload::Auth)?;
    let mut client = Client::with_interceptor(signed, id);
    let g = client
        .acquire(request_for(query, &actor.to_string())?)
        .await?
        .into_inner();
    let checked = client
        .check(proto::CheckRequest {
            view: g.view.clone(),
            binding: Some(b.clone()),
        })
        .await?;
    assert_eq!(checked.metadata().get("cache-control").unwrap(), "no-store");
    assert_eq!(checked.get_ref().subjects.len(), 1);
    // A valid Post identity can read with a capability but cannot acquire/complete.
    let channel = tls(Workload::Post)?
        .endpoint(&url, Workload::Auth)?
        .connect()
        .await?;
    let mut post = Client::with_interceptor(
        channel,
        ServiceIdentityClientInterceptor::new(Workload::Post, "post-fixture")?,
    );
    assert!(post
        .check(proto::CheckRequest {
            view: g.view.clone(),
            binding: Some(b.clone())
        })
        .await
        .is_ok());
    assert!(post.complete(native::completion(&b, &g)).await.is_err());
    let channel = tls(Workload::Profile)?
        .endpoint(&url, Workload::Auth)?
        .connect()
        .await?;
    let mut profile = Client::with_interceptor(
        channel,
        ServiceIdentityClientInterceptor::new(Workload::Profile, "profile-fixture")?,
    );
    assert!(profile
        .check(proto::CheckRequest {
            view: g.view.clone(),
            binding: Some(b.clone())
        })
        .await
        .is_ok());
    assert!(profile.complete(native::completion(&b, &g)).await.is_err());
    let mut forged = g.view.clone().unwrap();
    forged.view_secret[0] ^= 1;
    assert!(post
        .check(proto::CheckRequest {
            view: Some(forged),
            binding: Some(b.clone())
        })
        .await
        .is_err());
    native::finish_scopes(&prod).await?;
    assert!(
        client
            .complete(native::completion(&b, &g))
            .await?
            .into_inner()
            .completed
    );
    println!("mTLS owner acquisition signed Concert-only; Post view capability; forged capability denied; configured producer settlement authenticated PASS");
    p.close().await;
    prod.close().await;
    Ok(())
}
