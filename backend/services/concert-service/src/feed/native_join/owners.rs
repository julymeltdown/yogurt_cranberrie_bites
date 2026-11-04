//! Feed adaptation of PublicProfile db92f895 native/owners.rs; original SHA-256 0d9c52e710823bd936d36daf46da25dae08226efeb5db96dd4209dde2280a9ce.
//! Adapted read-only from backend/services/concert-service/tests/owner_authority_support/owners.rs at 3e22615eb39a8912c71c7bbcb003930cfc24f373.
//! Source SHA-256: 5f2b9d7589a6faf5106c1984154bb20f63977ed27916391f325731059aaee8bc.
//! Public native fixture changes: crate-local paths; Profile allowed in exact owned DB list.
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
    let gateway =
        ActorDelegationVerifier::new(Workload::Concert, &[capture::gateway()?.public_key()])?;
    RelationshipReadSigner::from_seed(&[75; 32], &gateway)
}
fn verifier(owner: Workload) -> anyhow::Result<RelationshipReadVerifier> {
    RelationshipReadVerifier::new(
        owner,
        &[signer()?.public_key()],
        &ActorDelegationVerifier::new(owner, &[capture::gateway()?.public_key()])?,
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

pub(super) async fn auth(
    pool: PgPool,
    producer_url: &str,
    post_url: &str,
) -> anyhow::Result<Running> {
    use auth_service::concert_authority::{proto, PgOwnerAuthority, RpcSettlementResolver};
    let tls = environment::tls(Workload::Auth)?;
    let resolver = RpcSettlementResolver::new(
        &tls,
        "auth-fixture",
        &[
            (proto::Producer::Concert, producer_url.to_owned()),
            (proto::Producer::Post, post_url.to_owned()),
        ],
    )?;
    let store = PgOwnerAuthority::new(pool, Arc::new(resolver))?;
    let server = auth_service::adapters::r#in::grpc::concert_authority::guarded_server(
        store,
        auth_service::concert_authority::owner_caller_registry()?,
        verifier(Workload::Auth)?,
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
pub(super) async fn friend(
    pool: PgPool,
    producer_url: &str,
    post_url: &str,
) -> anyhow::Result<Running> {
    use friend_service::concert_authority::{proto, PgOwnerAuthority, RpcSettlementResolver};
    let tls = environment::tls(Workload::Friend)?;
    let resolver = RpcSettlementResolver::new(
        &tls,
        "friend-fixture",
        &[
            (proto::Producer::Concert, producer_url.to_owned()),
            (proto::Producer::Post, post_url.to_owned()),
        ],
    )?;
    let store = PgOwnerAuthority::new(pool, Arc::new(resolver))?;
    let server = friend_service::adapters::r#in::grpc::concert_authority::guarded_server(
        store,
        friend_service::concert_authority::owner_caller_registry()?,
        verifier(Workload::Friend)?,
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
