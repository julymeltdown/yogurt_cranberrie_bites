//! CF02 real owner services; adapted from CF01 owners.rs (provenance in docs).
//! Actual Profile SELF and public-content protocols share one configured owner.
use super::{capture, environment};
use platform::{
    actor_delegation::{
        relationship_read::{RelationshipReadSigner, RelationshipReadVerifier},
        ActorDelegationVerifier,
    },
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::guard,
};
use sqlx::PgPool;
use std::sync::Arc;

pub(crate) struct Running(tokio::task::JoinHandle<()>);
impl Running {
    pub(crate) async fn stop(mut self) {
        self.0.abort();
        let _ = (&mut self.0).await;
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        self.0.abort();
    }
}
pub(crate) fn signer() -> anyhow::Result<RelationshipReadSigner> {
    RelationshipReadSigner::from_seed(&[75; 32], &gateway(Workload::Concert)?)
}
pub(crate) fn gateway(role: Workload) -> anyhow::Result<ActorDelegationVerifier> {
    ActorDelegationVerifier::new(role, &[capture::gateway()?.public_key()])
}
fn verifier(role: Workload) -> anyhow::Result<RelationshipReadVerifier> {
    RelationshipReadVerifier::new(role, &[signer()?.public_key()], &gateway(role)?)
}
fn incoming(
    listener: tokio::net::TcpListener,
) -> impl futures::Stream<Item = Result<tokio::net::TcpStream, std::io::Error>> + Send + 'static {
    futures::stream::unfold(listener, |l| async {
        Some((l.accept().await.map(|(s, _)| s), l))
    })
}
fn spawn(
    f: impl std::future::Future<Output = Result<(), tonic::transport::Error>> + Send + 'static,
) -> Running {
    Running(tokio::spawn(async move {
        f.await.expect("actual CF02 owner server")
    }))
}
pub(crate) async fn auth(pool: PgPool) -> anyhow::Result<Running> {
    use auth_service::adapters::r#in::grpc::concert_relationship as rel;
    use auth_service::concert_authority::{proto, PgOwnerAuthority, RpcSettlementResolver};
    let tls = environment::tls(Workload::Auth)?;
    let resolver = RpcSettlementResolver::new(
        &tls,
        "auth-fixture",
        &[
            (proto::Producer::Concert, environment::url("concert")?),
            (proto::Producer::Profile, environment::url("profile")?),
        ],
    )?;
    let owner = auth_service::adapters::r#in::grpc::concert_authority::guarded_server(
        PgOwnerAuthority::new(pool.clone(), Arc::new(resolver))?,
        auth_service::concert_authority::owner_caller_registry()?,
        verifier(Workload::Auth)?,
    )?;
    let relationship=guard(&rel::POLICY,rel::proto::auth_concert_relationship_service_server::AuthConcertRelationshipServiceServer::new(rel::RelationshipServer::from_pool(pool)),CallerRegistry::new().register(Workload::Concert,"concert-fixture")?,verifier(Workload::Auth)?.into_inner())?;
    let listener = tokio::net::TcpListener::bind(environment::address("auth")?).await?;
    Ok(spawn(
        tonic::transport::Server::builder()
            .tls_config(tls.server_config()?)?
            .layer(CallDeadlineLayer)
            .add_service(owner)
            .add_service(relationship)
            .serve_with_incoming(incoming(listener)),
    ))
}
pub(crate) async fn friend(pool: PgPool) -> anyhow::Result<Running> {
    use friend_service::concert_authority::{proto, PgOwnerAuthority, RpcSettlementResolver};
    let tls = environment::tls(Workload::Friend)?;
    let resolver = RpcSettlementResolver::new(
        &tls,
        "friend-fixture",
        &[
            (proto::Producer::Concert, environment::url("concert")?),
            (proto::Producer::Profile, environment::url("profile")?),
        ],
    )?;
    let owner = friend_service::adapters::r#in::grpc::concert_authority::guarded_server(
        PgOwnerAuthority::new(pool, Arc::new(resolver))?,
        friend_service::concert_authority::owner_caller_registry()?,
        verifier(Workload::Friend)?,
    )?;
    let listener = tokio::net::TcpListener::bind(environment::address("friend")?).await?;
    Ok(spawn(
        tonic::transport::Server::builder()
            .tls_config(tls.server_config()?)?
            .layer(CallDeadlineLayer)
            .add_service(owner)
            .serve_with_incoming(incoming(listener)),
    ))
}
pub(crate) async fn profile(
    pool: PgPool,
    deny_complete: Arc<std::sync::atomic::AtomicBool>,
) -> anyhow::Result<Running> {
    use profile_service::{
        adapters::r#in::grpc::concert_public_profile as public,
        concert_self_profile::{
            server::{SelfProfileService, SELF_POLICY},
            settlement::SETTLEMENT_POLICY,
        },
    };
    let tls = environment::tls(Workload::Profile)?;
    // Process environment is supplied once by the isolated controller. No global
    // variables are mutated; these are the production service's actual keys.
    let (service, keys) = SelfProfileService::configured(
        pool.clone(),
        &gateway(Workload::Profile)?,
        true,
        Some(&tls),
    )?
    .ok_or_else(|| anyhow::anyhow!("actual SELF configuration required"))?;
    let callers = CallerRegistry::new()
        .register(Workload::Concert, "concert-fixture")?
        .register(Workload::Auth, "auth-fixture")?
        .register(Workload::Friend, "friend-fixture")?;
    let self_service = guard(
        &SELF_POLICY,
        service.clone().server(),
        callers.clone(),
        keys,
    )?;
    let settlement = platform::grpc_method_policy::guard_without_delegation(
        &SETTLEMENT_POLICY,
        service.settlement_server(),
        callers.clone(),
    )?;
    let public=guard(&public::PUBLIC_PROFILE_POLICY,public::proto::concert_public_profile_service_server::ConcertPublicProfileServiceServer::new(public::PublicProfileServer::from_pool(pool)),callers,verifier(Workload::Profile)?.into_inner())?;
    let public = super::fault::NegativeComplete::controlled(
        public,
        deny_complete,
        Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    );
    let listener = tokio::net::TcpListener::bind(environment::address("profile")?).await?;
    Ok(spawn(
        tonic::transport::Server::builder()
            .tls_config(tls.server_config()?)?
            .layer(CallDeadlineLayer)
            .add_service(self_service)
            .add_service(settlement)
            .add_service(public)
            .serve_with_incoming(incoming(listener)),
    ))
}
pub(crate) async fn concert(
    settlement: concert_service::product_rpc::settlement::ProducerSettlementService,
    decision: concert_service::participant::live::decision_rpc::ParticipantDecisionService,
    decision_keys: ActorDelegationVerifier,
) -> anyhow::Result<Running> {
    let callers = CallerRegistry::new()
        .register(Workload::Auth, "auth-fixture")?
        .register(Workload::Friend, "friend-fixture")?
        .register(Workload::Profile, "profile-fixture")?;
    let settlement = platform::grpc_method_policy::guard_without_delegation(
        &concert_service::product_rpc::settlement::POLICY,
        settlement.server(),
        callers.clone(),
    )?;
    let decision = guard(
        &concert_service::participant::live::decision_rpc::DECISION_POLICY,
        decision.server(),
        callers,
        decision_keys,
    )?;
    let listener = tokio::net::TcpListener::bind(environment::address("concert")?).await?;
    Ok(spawn(
        tonic::transport::Server::builder()
            .tls_config(environment::tls(Workload::Concert)?.server_config()?)?
            .layer(CallDeadlineLayer)
            .add_service(settlement)
            .add_service(decision)
            .serve_with_incoming(incoming(listener)),
    ))
}
