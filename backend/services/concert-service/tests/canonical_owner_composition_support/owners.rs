//! CF01 adapted from owner_authority_support/owners.rs at 591d908a85a01fe0a494e04c0109bb0bec64a3e1; original SHA256 b9bbf048a2d236f759ac5eb5d935d3ccb0d7e9d76343428f23415d5fd3a37a00.
//! Production owner stores, production guards and actual authenticated settlement
//! clients. Main-process startup itself is qualified by the owner-store runner.
use super::{capture, environment};
use platform::{
    actor_delegation::{
        relationship_read::{RelationshipReadSigner, RelationshipReadVerifier},
        ActorDelegationVerifier,
    },
    grpc_auth::Workload,
};
use sqlx::PgPool;
use std::sync::Arc;

pub(super) struct Running {
    pub(super) url: String,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(super) fn signer() -> anyhow::Result<RelationshipReadSigner> {
    signer_for_gateway(capture::gateway()?.public_key())
}
pub(super) fn signer_for_gateway(key: [u8; 32]) -> anyhow::Result<RelationshipReadSigner> {
    let gateway = ActorDelegationVerifier::new(Workload::Concert, &[key])?;
    RelationshipReadSigner::from_seed(&[75; 32], &gateway)
}
fn verifier(owner: Workload, key: [u8; 32]) -> anyhow::Result<RelationshipReadVerifier> {
    RelationshipReadVerifier::new(
        owner,
        &[signer_for_gateway(key)?.public_key()],
        &ActorDelegationVerifier::new(owner, &[key])?,
    )
}
fn incoming(
    listener: tokio::net::TcpListener,
) -> impl futures::Stream<Item = Result<tokio::net::TcpStream, std::io::Error>> + Send + 'static {
    futures::stream::unfold(listener, |listener| async {
        let result = listener.accept().await.map(|(stream, _)| stream);
        Some((result, listener))
    })
}

pub(super) async fn auth(pool: PgPool, producer_url: &str) -> anyhow::Result<Running> {
    auth_for_gateway(pool, producer_url, capture::gateway()?.public_key()).await
}
pub(super) async fn auth_for_gateway(
    pool: PgPool,
    producer_url: &str,
    key: [u8; 32],
) -> anyhow::Result<Running> {
    use auth_service::concert_authority::{proto, PgOwnerAuthority, RpcSettlementResolver};
    let tls = environment::tls(Workload::Auth)?;
    let resolver = RpcSettlementResolver::new(
        &tls,
        "auth-fixture",
        &[(proto::Producer::Concert, producer_url.to_owned())],
    )?;
    let store = PgOwnerAuthority::new(pool, Arc::new(resolver))?;
    let server = auth_service::adapters::r#in::grpc::concert_authority::guarded_server(
        store,
        auth_service::concert_authority::owner_caller_registry()?,
        verifier(Workload::Auth, key)?,
    )?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("https://{}", listener.local_addr()?);
    let server = tonic::transport::Server::builder()
        .tls_config(tls.server_config()?)?
        .add_service(server);
    let task = tokio::spawn(async move {
        server
            .serve_with_incoming(incoming(listener))
            .await
            .expect("actual Auth authority server");
    });
    Ok(Running { url, task })
}
pub(super) async fn friend(pool: PgPool, producer_url: &str) -> anyhow::Result<Running> {
    friend_for_gateway(pool, producer_url, capture::gateway()?.public_key()).await
}
pub(super) async fn friend_for_gateway(
    pool: PgPool,
    producer_url: &str,
    key: [u8; 32],
) -> anyhow::Result<Running> {
    use friend_service::concert_authority::{proto, PgOwnerAuthority, RpcSettlementResolver};
    let tls = environment::tls(Workload::Friend)?;
    let resolver = RpcSettlementResolver::new(
        &tls,
        "friend-fixture",
        &[(proto::Producer::Concert, producer_url.to_owned())],
    )?;
    let store = PgOwnerAuthority::new(pool, Arc::new(resolver))?;
    let server = friend_service::adapters::r#in::grpc::concert_authority::guarded_server(
        store,
        friend_service::concert_authority::owner_caller_registry()?,
        verifier(Workload::Friend, key)?,
    )?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("https://{}", listener.local_addr()?);
    let server = tonic::transport::Server::builder()
        .tls_config(tls.server_config()?)?
        .add_service(server);
    let task = tokio::spawn(async move {
        server
            .serve_with_incoming(incoming(listener))
            .await
            .expect("actual Friend authority server");
    });
    Ok(Running { url, task })
}
