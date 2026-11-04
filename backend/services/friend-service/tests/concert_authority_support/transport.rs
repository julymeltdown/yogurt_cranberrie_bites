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
pub(super) struct Running(tokio::task::JoinHandle<()>, String);
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
    let resolver = friend_service::concert_authority::RpcSettlementResolver::new(
        &tls(Workload::Friend)?,
        "friend-fixture",
        &[(proto::Producer::Concert, url.clone())],
    )?;
    Ok((
        PgOwnerAuthority::new(p.clone(), Arc::new(resolver))?,
        Running(task, url),
    ))
}

#[tokio::test]
#[ignore = "requires owned PG16 fixture and mTLS certificates"]
async fn owner_rpc_requires_actual_workload_and_signed_acquisition() -> anyhow::Result<()> {
    use platform::{
        actor_delegation::{
            relationship_read::RelationshipReadSigner, request_for, ActorDelegationSigner,
            ActorDelegationVerifier,
        },
        grpc_auth::ServiceIdentityClientInterceptor,
    };
    use proto::friend_owner_authority_service_client::FriendOwnerAuthorityServiceClient as Client;
    let p = native::pool(false).await?;
    let prod = native::pool(true).await?;
    native::setup(&p, &prod).await?;
    let (actor, _, family) = native::fixture(&p).await?;
    let (_store, producer) = rpc_store(&p, &prod).await?;
    let gateway = ActorDelegationSigner::from_seed(Workload::Gateway, &[8; 32])?;
    let gateway_key = gateway.public_key();
    let gateway = ActorDelegationVerifier::new(Workload::Friend, &[gateway.public_key()])?;
    let signer = RelationshipReadSigner::from_seed(&[19; 32], &gateway)?;
    let (_owner, url) = start_main(&signer.public_key(), &gateway_key, &producer.1).await?;
    let channel = tls(Workload::Concert)?
        .endpoint(&url, Workload::Friend)?
        .connect()
        .await?;
    let b = native::binding(actor, family);
    let scope = native::scope(&prod, &b).await?;
    let id = ServiceIdentityClientInterceptor::new(Workload::Concert, "concert-fixture")?;
    let mut unsigned = Client::with_interceptor(channel.clone(), id.clone());
    let query = proto::AcquireRequest {
        binding: Some(b.clone()),
        response_scope: Some(scope),
    };
    assert!(unsigned.acquire(query.clone()).await.is_err());
    let signed = signer.client(channel, Workload::Friend)?;
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
        .endpoint(&url, Workload::Friend)?
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
        .endpoint(&url, Workload::Friend)?
        .connect()
        .await?;
    let legacy_channel = channel.clone();
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
    let mut legacy=friend_service::adapters::r#in::grpc::server::proto::friend_service_client::FriendServiceClient::with_interceptor(legacy_channel,ServiceIdentityClientInterceptor::new(Workload::Profile,"profile-fixture")?);
    assert_eq!(
        legacy
            .list_followers(
                friend_service::adapters::r#in::grpc::server::proto::ListFollowersRequest::default(
                )
            )
            .await
            .unwrap_err()
            .code(),
        tonic::Code::Unauthenticated,
        "New authority caller must not enter retained Friend registry"
    );
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

struct MainProcess(std::process::Child);
impl Drop for MainProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
async fn start_main(
    read_key: &[u8; 32],
    gateway_key: &[u8; 32],
    producer_url: &str,
) -> anyhow::Result<(MainProcess, String)> {
    use base64ct::{Base64UrlUnpadded, Encoding};
    const OWNER: &str = "friend";
    let root = std::path::PathBuf::from(std::env::var("MCR_AUTHORITY_ROOT")?);
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    drop(listener);
    let url = format!("https://{address}");
    let prefix = OWNER.to_ascii_uppercase();
    let log = root.join(format!("{OWNER}-main.log"));
    let output = std::fs::File::create(&log)?;
    let mut command = std::process::Command::new(std::env::var("MCR_AUTHORITY_MAIN_BINARY")?);
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("TMPDIR", &root)
        .env(format!("{prefix}_RUNTIME_MODE"), "parity")
        .env(
            format!("{prefix}_DATABASE_URL"),
            std::env::var("MCR_AUTHORITY_OWNER_URL")?,
        )
        .env(format!("{prefix}_GRPC_ADDR"), address.to_string())
        .env(format!("{prefix}_HTTP_ADDR"), "127.0.0.1:0")
        .env(format!("{prefix}_METRICS_ADDR"), "127.0.0.1:0")
        .env(format!("{prefix}_SEED_ENABLED"), "false")
        .env(
            format!("{prefix}_GRPC_TLS_CERT_PATH"),
            root.join("pki").join(format!("{OWNER}.crt")),
        )
        .env(
            format!("{prefix}_GRPC_TLS_KEY_PATH"),
            root.join("pki").join(format!("{OWNER}.key")),
        )
        .env(
            format!("{prefix}_GRPC_TLS_CA_PATH"),
            root.join("pki/ca.crt"),
        )
        .env(
            format!("{prefix}_SERVICE_CREDENTIAL"),
            format!("{OWNER}-fixture"),
        )
        .env("CONCERT_SERVICE_CREDENTIAL", "concert-fixture")
        .env("POST_SERVICE_CREDENTIAL", "post-fixture")
        .env("PROFILE_SERVICE_CREDENTIAL", "profile-fixture")
        .env(
            "ACTOR_DELEGATION_VERIFYING_KEYS",
            Base64UrlUnpadded::encode_string(gateway_key),
        )
        .env(
            "CONCERT_RELATIONSHIP_VERIFYING_KEYS",
            Base64UrlUnpadded::encode_string(read_key),
        )
        .env("CONCERT_OWNER_AUTHORITY_ENABLED", "true")
        .env("AUTHORITY_CONCERT_SETTLEMENT_ENDPOINT", producer_url)
        .stdout(output.try_clone()?)
        .stderr(output);
    for suffix in ["PROFILE", "FRIEND", "POST", "NOTIFICATION"] {
        command.env(
            format!("{prefix}_{suffix}_GRPC_ENDPOINT"),
            "https://127.0.0.1:1",
        );
    }
    for key in [
        "AUTH_LIFECYCLE_CLEANUP_WORKER_ENABLED",
        "AUTH_USER_DELETION_WORKER_ENABLED",
        "FRIEND_RECOMMENDATION_WORKER_ENABLED",
        "FRIEND_RECOMMENDATION_RETENTION_WORKER_ENABLED",
        "FRIEND_MUTATION_RECEIPT_RETENTION_WORKER_ENABLED",
    ] {
        command.env(key, "false");
    }
    let mut child = MainProcess(command.spawn()?);
    let stop = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.0.try_wait()? {
            anyhow::bail!(
                "actual owner main exited {status}: {}",
                std::fs::read_to_string(&log)?
                    .lines()
                    .rev()
                    .take(4)
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
        if tls(Workload::Concert)?
            .endpoint(&url, Workload::Friend)?
            .connect()
            .await
            .is_ok()
        {
            break;
        }
        anyhow::ensure!(Instant::now() < stop, "actual owner main did not start");
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    Ok((child, url))
}
