//! Fresh actual-binary fixtures and a byte-transparent mTLS fault relay.
#![allow(dead_code)]
use super::{
    owner_process::{address, live, put, tls, Evidence, OwnedChild, ProofGuard},
    support,
};
use anyhow::{Context, Result};
use base64ct::{Base64, Encoding};
use platform::{actor_delegation::ActorDelegationSigner, grpc_auth::Workload};
use serde_json::json;
use sqlx::PgPool;
use std::{
    collections::BTreeMap,
    net::TcpListener,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener as AsyncListener, TcpStream},
    sync::{oneshot, watch},
    task::{JoinHandle, JoinSet},
};
use uuid::Uuid;

#[derive(Clone, Copy)]
enum OwnerKind {
    Friend,
    Profile,
}
impl OwnerKind {
    fn prefix(self) -> &'static str {
        match self {
            Self::Friend => "FRIEND",
            Self::Profile => "PROFILE",
        }
    }
    fn role(self) -> &'static str {
        match self {
            Self::Friend => "friend",
            Self::Profile => "profile",
        }
    }
}

pub(super) struct WireFixture {
    pub app: Option<support::TestApp>,
    pub owner_pool: PgPool,
    pub owner: OwnedChild,
    pub gateway: Option<OwnedChild>,
    pub origin: String,
    pub gateway_address: String,
    pub proof: Evidence,
    pub relay: Option<OpaqueRelay>,
    directory: PathBuf,
    gateway_metrics: String,
    gateway_env: BTreeMap<String, String>,
    gateway_binary: PathBuf,
    owner_binary: PathBuf,
    owner_env: BTreeMap<String, String>,
    owner_metrics: String,
    _absent: TcpListener,
    _guard: ProofGuard,
}
impl WireFixture {
    pub async fn start(directory: PathBuf, boundary: &str, opaque_relay: bool) -> Result<Self> {
        Self::start_owner(directory, boundary, opaque_relay, OwnerKind::Friend).await
    }
    pub async fn start_profile(directory: PathBuf, boundary: &str) -> Result<Self> {
        Self::start_owner(directory, boundary, false, OwnerKind::Profile).await
    }
    async fn start_owner(
        directory: PathBuf,
        boundary: &str,
        opaque_relay: bool,
        kind: OwnerKind,
    ) -> Result<Self> {
        let prefix = kind.prefix();
        anyhow::ensure!(
            std::env::var("GATEWAY_TEST_CREATE_ISOLATED_DATABASES").as_deref() == Ok("1"),
            "owned isolated fixture required"
        );
        let proof = Evidence::new(directory.clone(), boundary)?;
        let guard = ProofGuard(proof.clone());
        let app = support::start_gateway_app(false).await?;
        let lineage: String =
            sqlx::query_scalar("select system_identifier::text from pg_control_system()")
                .fetch_one(&app.pool)
                .await?;
        anyhow::ensure!(
            std::env::var("GATEWAY_TEST_INITDB_SYSTEM_IDENTIFIER").as_deref() == Ok(&lineage),
            "owned initdb lineage"
        );
        let database = format!("port_rust_{}_wire_{}", kind.role(), Uuid::new_v4().simple());
        sqlx::query(&format!("create database {database} template template0"))
            .execute(&app.pool)
            .await?;
        let owner_url = format!(
            "{}/{database}",
            app.config
                .database_url
                .rsplit_once('/')
                .context("fixture URL")?
                .0
        );
        let owner_pool = PgPool::connect(&owner_url).await?;
        let empty: bool = sqlx::query_scalar(
            "select not exists(select 1 from pg_tables where schemaname='public')",
        )
        .fetch_one(&owner_pool)
        .await?;
        anyhow::ensure!(
            empty,
            "owner fixture must be empty before actual binary migrations"
        );
        proof.event(json!({"stage":"fixtures","systemIdentifier":lineage,"ownerDatabase":database,"template":"template0","emptyBeforeMigrations":empty}))?;
        let owner_leaf = test_support::tls::issue(kind.role())?;
        let gateway_leaf = test_support::tls::issue("gateway")?;
        let mut seed = [0u8; 32];
        ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut seed)
            .map_err(|_| anyhow::anyhow!("entropy"))?;
        let signer = ActorDelegationSigner::from_seed(Workload::Gateway, &seed)?;
        let credential = format!("http-process-{}", Uuid::new_v4());
        let owner_grpc = address()?;
        let owner_metrics = address()?;
        let gateway_metrics = address()?;
        let gateway_http = address()?;
        let origin = format!("http://{gateway_http}");
        let absent = TcpListener::bind("127.0.0.1:0")?;
        let absent_endpoint = format!("https://{}", absent.local_addr()?);
        let mut owner_env = BTreeMap::new();
        put(&mut owner_env, &format!("{prefix}_RUNTIME_MODE"), "parity");
        put(&mut owner_env, &format!("{prefix}_SEED_ENABLED"), "false");
        let workers: &[&str] = match kind {
            OwnerKind::Friend => &[
                "RECOMMENDATION_WORKER_ENABLED",
                "RECOMMENDATION_RETENTION_WORKER_ENABLED",
                "MUTATION_RECEIPT_RETENTION_WORKER_ENABLED",
            ],
            OwnerKind::Profile => &[
                "RETENTION_PURGE_WORKER_ENABLED",
                "MUTATION_RECEIPT_RETENTION_WORKER_ENABLED",
            ],
        };
        for worker in workers {
            put(&mut owner_env, &format!("{prefix}_{worker}"), "false");
        }
        put(&mut owner_env, &format!("{prefix}_HTTP_ADDR"), address()?);
        put(
            &mut owner_env,
            &format!("{prefix}_METRICS_ADDR"),
            &owner_metrics,
        );
        put(&mut owner_env, &format!("{prefix}_GRPC_ADDR"), &owner_grpc);
        put(
            &mut owner_env,
            &format!("{prefix}_DATABASE_URL"),
            &owner_url,
        );
        if matches!(kind, OwnerKind::Friend) {
            put(
                &mut owner_env,
                "FRIEND_PROFILE_GRPC_ENDPOINT",
                &absent_endpoint,
            );
        }
        put(&mut owner_env, "GATEWAY_SERVICE_CREDENTIAL", &credential);
        put(
            &mut owner_env,
            &format!("{prefix}_SERVICE_CREDENTIAL"),
            format!("owner-{}", Uuid::new_v4()),
        );
        put(
            &mut owner_env,
            "ACTOR_DELEGATION_VERIFYING_KEYS",
            Base64::encode_string(&signer.public_key()),
        );
        tls(&mut owner_env, prefix, &owner_leaf);
        let relay = if opaque_relay {
            Some(OpaqueRelay::start(owner_grpc.clone()).await?)
        } else {
            None
        };
        let owner_endpoint = relay.as_ref().map_or_else(
            || format!("https://{owner_grpc}"),
            |relay| format!("https://{}", relay.address),
        );
        let mut gateway_env = BTreeMap::new();
        put(&mut gateway_env, "GATEWAY_RUNTIME_MODE", "parity");
        put(&mut gateway_env, "GATEWAY_SEED_ENABLED", "false");
        put(&mut gateway_env, "GATEWAY_HTTP_ADDR", &gateway_http);
        put(&mut gateway_env, "GATEWAY_METRICS_ADDR", &gateway_metrics);
        put(
            &mut gateway_env,
            "GATEWAY_DATABASE_URL",
            &app.config.database_url,
        );
        put(
            &mut gateway_env,
            &format!("GATEWAY_{prefix}_GRPC_ENDPOINT"),
            owner_endpoint,
        );
        for owner in ["AUTH", "PROFILE", "FRIEND", "POST", "NOTIFICATION"] {
            if owner == prefix {
                continue;
            }
            put(
                &mut gateway_env,
                &format!("GATEWAY_{owner}_GRPC_ENDPOINT"),
                &absent_endpoint,
            );
        }
        put(&mut gateway_env, "GATEWAY_SERVICE_CREDENTIAL", &credential);
        put(
            &mut gateway_env,
            "ACTOR_DELEGATION_SIGNING_SEED",
            Base64::encode_string(&seed),
        );
        put(
            &mut gateway_env,
            "GATEWAY_JWT_JWKS_JSON",
            &app.config.jwt_jwks_json,
        );
        put(
            &mut gateway_env,
            "GATEWAY_JWT_ISSUER",
            &app.config.jwt_issuer,
        );
        put(
            &mut gateway_env,
            "GATEWAY_JWT_AUDIENCE",
            &app.config.jwt_audience,
        );
        put(&mut gateway_env, "GATEWAY_JWT_KEYS_REFRESH_SECONDS", "0");
        tls(&mut gateway_env, "GATEWAY", &gateway_leaf);
        let owner_binary = PathBuf::from(std::env::var(format!("GATEWAY_TEST_{prefix}_BINARY"))?);
        let gateway_binary = PathBuf::from(std::env::var("GATEWAY_TEST_GATEWAY_BINARY")?);
        let mut owner = OwnedChild::spawn(
            &owner_binary,
            &format!("http-{}-1", kind.role()),
            &owner_env,
            &directory,
            &proof,
        )?;
        live(&mut owner, &owner_metrics).await?;
        Ok(Self {
            app: Some(app),
            owner_pool,
            owner,
            gateway: None,
            origin,
            gateway_address: gateway_http,
            proof,
            relay,
            directory,
            gateway_metrics,
            gateway_env,
            gateway_binary,
            owner_binary,
            owner_env,
            owner_metrics,
            _absent: absent,
            _guard: guard,
        })
    }
    pub async fn restart_owner(&mut self, label: &str) -> Result<()> {
        self.owner.stop()?;
        self.owner = OwnedChild::spawn(
            &self.owner_binary,
            label,
            &self.owner_env,
            &self.directory,
            &self.proof,
        )?;
        live(&mut self.owner, &self.owner_metrics).await
    }
    pub fn pool(&self) -> &PgPool {
        &self.app.as_ref().expect("app").pool
    }
    pub async fn start_gateway(&mut self, label: &str) -> Result<()> {
        anyhow::ensure!(self.gateway.is_none(), "owned Gateway already present");
        self.gateway = Some(OwnedChild::spawn(
            &self.gateway_binary,
            label,
            &self.gateway_env,
            &self.directory,
            &self.proof,
        )?);
        live(
            self.gateway.as_mut().expect("gateway"),
            &self.gateway_metrics,
        )
        .await
    }
    pub fn gateway_alive(&mut self) -> Result<()> {
        self.gateway
            .as_mut()
            .context("owned Gateway missing")?
            .alive()
    }
    pub fn kill_gateway(&mut self) -> Result<()> {
        if let Some(mut child) = self.gateway.take() {
            child.stop()?;
        }
        Ok(())
    }
    pub async fn finish(mut self, scenario: Result<()>) -> Result<()> {
        let mut cleanup_errors = Vec::new();
        if let Err(error) = &scenario {
            if let Err(record_error) = self
                .proof
                .event(json!({"stage":"scenario-failed","error":error.to_string()}))
            {
                cleanup_errors.push(record_error.to_string());
            }
        }
        if let Err(error) = self.kill_gateway() {
            cleanup_errors.push(error.to_string());
        }
        if let Err(error) = self.owner.stop() {
            cleanup_errors.push(error.to_string());
        }
        if let Some(mut relay) = self.relay.take() {
            match relay.stop().await {
                Ok(stats) => {
                    if let Err(error) = self
                        .proof
                        .event(json!({"stage":"opaque-relay-stopped","stats":stats}))
                    {
                        cleanup_errors.push(error.to_string());
                    }
                }
                Err(error) => cleanup_errors.push(error.to_string()),
            }
        }
        self.owner_pool.close().await;
        if let Err(error) = self.app.take().expect("app").shutdown().await {
            cleanup_errors.push(error.to_string());
        }
        self.proof.event(json!({"stage":"owned-children-reaped-and-observer-pools-closed","cleanupErrors":cleanup_errors}))?;
        self.proof
            .finish(if scenario.is_ok() && cleanup_errors.is_empty() {
                "PASS"
            } else {
                "INCOMPLETE"
            })?;
        if !cleanup_errors.is_empty() {
            anyhow::bail!(
                "process fixture cleanup failed: {}",
                cleanup_errors.join("; ")
            );
        }
        scenario
    }
}

pub(super) struct OpaqueRelay {
    pub address: String,
    hold: watch::Sender<bool>,
    task: Option<JoinHandle<Result<()>>>,
    stop: Option<oneshot::Sender<()>>,
    connections: Arc<AtomicU64>,
    held: Arc<AtomicU64>,
}
impl OpaqueRelay {
    async fn start(owner_address: String) -> Result<Self> {
        let listener = AsyncListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?.to_string();
        let (hold, _) = watch::channel(false);
        let connections = Arc::new(AtomicU64::new(0));
        let held = Arc::new(AtomicU64::new(0));
        let gate = hold.clone();
        let accepted = connections.clone();
        let blocked = held.clone();
        let (stop, mut stopping) = oneshot::channel();
        let task = tokio::spawn(async move {
            let mut peers = JoinSet::new();
            loop {
                tokio::select! {
                    _=&mut stopping=>break,
                    socket=listener.accept()=>{
                        let (gateway,_)=socket?;
                        accepted.fetch_add(1,Ordering::SeqCst);
                        let owner_address=owner_address.clone(); let rx=gate.subscribe(); let blocked=blocked.clone();
                        peers.spawn(async move { relay_peer(gateway,&owner_address,rx,blocked).await });
                    }
                    Some(result)=peers.join_next(), if !peers.is_empty()=>{
                        // Peer closure during a deadline/crash is expected; task
                        // panics are evidence failures rather than silent success.
                        let _peer_result=result.context("relay peer panicked")?;
                    }
                }
            }
            drop(listener);
            peers.abort_all();
            while let Some(result) = peers.join_next().await {
                if let Err(error) = result {
                    anyhow::ensure!(error.is_cancelled(), "relay peer panicked during cleanup");
                }
            }
            Ok(())
        });
        Ok(Self {
            address,
            hold,
            task: Some(task),
            stop: Some(stop),
            connections,
            held,
        })
    }
    pub fn gate(&self, hold: bool) {
        self.hold.send_replace(hold);
    }
    pub fn held_bytes(&self) -> u64 {
        self.held.load(Ordering::SeqCst)
    }
    pub fn connections(&self) -> u64 {
        self.connections.load(Ordering::SeqCst)
    }
    async fn stop(&mut self) -> Result<serde_json::Value> {
        if let Some(task) = self.task.take() {
            self.stop
                .take()
                .context("relay stop handle")?
                .send(())
                .map_err(|_| anyhow::anyhow!("relay already terminated"))?;
            task.await.context("relay listener panicked")??;
        }
        Ok(
            json!({"acceptedConnections":self.connections(),"heldOwnerBytes":self.held_bytes(),"listenerTaskJoined":true,"peerTasksDrained":true}),
        )
    }
}
impl Drop for OpaqueRelay {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}
async fn relay_peer(
    mut gateway: TcpStream,
    owner_address: &str,
    mut gate: watch::Receiver<bool>,
    held: Arc<AtomicU64>,
) -> Result<()> {
    let mut owner = TcpStream::connect(owner_address).await?;
    let (mut inbound, mut outbound) = gateway.split();
    let (mut owner_in, mut owner_out) = owner.split();
    tokio::select! {
        result=tokio::io::copy(&mut inbound,&mut owner_out)=>{result?;}
        result=async {
            let mut bytes=[0u8;16384];
            loop {
                let count=owner_in.read(&mut bytes).await?;
                if count==0 {return Ok::<(),anyhow::Error>(());}
                if *gate.borrow(){held.fetch_add(count as u64,Ordering::SeqCst);}
                while *gate.borrow(){gate.changed().await?;}
                outbound.write_all(&bytes[..count]).await?;
            }
        }=>{result?;}
    }
    Ok(())
}
