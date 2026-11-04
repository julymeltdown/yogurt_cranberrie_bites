//! Uses the existing canonical fixture ownership/PKI and the actual service
//! constructors. No new store/cluster/default authority or positive fake owner.
use super::*;
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
pub const AUTH: &str = "auth-fixture";
pub const CONCERT: &str = "concert-fixture";
pub struct Running {
    pub url: String,
    task: Option<tokio::task::JoinHandle<Result<(), tonic::transport::Error>>>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
}
impl Drop for Running {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
impl Running {
    pub async fn finish(mut self) -> anyhow::Result<()> {
        let _ = self.stop.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(3), self.task.take().unwrap()).await???;
        Ok(())
    }
}
async fn listener() -> anyhow::Result<(String, tokio::net::TcpListener)> {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    Ok((format!("https://{}", l.local_addr()?), l))
}
macro_rules! serve {
    ($service:expr,$role:expr) => {{
        let (url, l) = listener().await?;
        let (stop, shutdown) = tokio::sync::oneshot::channel();
        let incoming =
            futures::stream::unfold(l, |l| async { Some((l.accept().await.map(|(s, _)| s), l)) });
        let server = tonic::transport::Server::builder()
            .tls_config(environment::tls($role)?.server_config()?)?
            .layer(platform::call_deadline::CallDeadlineLayer)
            .add_service($service);
        let task = tokio::spawn(server.serve_with_incoming_shutdown(incoming, async {
            let _ = shutdown.await;
        }));
        Running {
            url,
            task: Some(task),
            stop: Some(stop),
        }
    }};
}
pub async fn source_server(p: PgPool) -> anyhow::Result<Running> {
    Ok(serve!(
        auth_service::adapters::r#in::grpc::concert_deletion_source::guarded_server(
            p,
            CallerRegistry::new().register(Workload::Concert, CONCERT)?
        )?,
        Workload::Auth
    ))
}
pub async fn factory_server(
    p: PgPool,
    source: &str,
    role: Workload,
    live: LiveDeletionOwners<'_>,
) -> anyhow::Result<Running> {
    let factory = AccountDeletionRuntime::compose(
        AccountDeletionSettings::Managed,
        p,
        source,
        Some(&environment::tls(Workload::Concert)?),
        CONCERT,
        AuthDeletionCallers::from_credentials(AUTH, Some("previous-auth"))?,
        live,
    )?
    .unwrap();
    Ok(serve!(factory.service()?, role))
}
pub async fn notification_server(pool: PgPool) -> anyhow::Result<Running> {
    use notification_service::{
        adapters::r#in::grpc::{
            method_policy::NOTIFICATION_METHOD_POLICY,
            server::{proto, NotificationGrpcServer},
        },
        config::{build_state, AppConfig},
    };
    let registry = CallerRegistry::new().register(Workload::Auth, AUTH)?;
    let config = AppConfig {
        database_migration_mode: migration_runtime::MigrationMode::Startup,
        http_addr: "127.0.0.1:0".parse()?,
        metrics_addr: "127.0.0.1:0".parse()?,
        grpc_addr: "127.0.0.1:0".parse()?,
        database_url: std::env::var("MCR_CANONICAL_NOTIFICATION_URL")?,
        database_pool_max_connections: 6,
        actor_delegation: platform::actor_delegation::ActorDelegationVerifier::local_parity(
            Workload::Notification,
        ),
        caller_registry: registry.clone(),
        tls_material: Some(environment::tls(Workload::Notification)?),
        retention_purge_worker_enabled: false,
        retention_purge_worker_interval_ms: 1000,
        retention_purge_actor_id: Uuid::new_v4().to_string(),
    };
    let state = Arc::new(build_state(&config, pool).await?);
    let service = platform::grpc_method_policy::guard(
        &NOTIFICATION_METHOD_POLICY,
        proto::notification_service_server::NotificationServiceServer::new(
            NotificationGrpcServer::new(state),
        ),
        registry,
        config.actor_delegation,
    )?;
    Ok(serve!(service, Workload::Notification))
}
pub fn runtime_dispatcher(
    concert: &str,
    notification: &str,
    ms: u64,
) -> anyhow::Result<auth_service::config::concert_deletion::RuntimeDispatcher> {
    let mut c = auth_fixture::test_config("postgres://unused/mcr_auth".into())?;
    c.outbound_identity = AUTH.into();
    c.tls_material = Some(environment::tls(Workload::Auth)?);
    c.profile_grpc_endpoint = "https://127.0.0.1:1".into();
    c.friend_grpc_endpoint = c.profile_grpc_endpoint.clone();
    c.post_grpc_endpoint = c.profile_grpc_endpoint.clone();
    c.notification_grpc_endpoint = notification.into();
    settings(concert, ms)?.dispatcher(&c, Default::default())
}
#[derive(Clone)]
struct Hostile {
    delay: Arc<AtomicBool>,
    calls: Arc<AtomicUsize>,
}
#[tonic::async_trait]
impl proto::account_deletion_service_server::AccountDeletionService for Hostile {
    async fn apply(
        &self,
        _: tonic::Request<proto::ApplyRequest>,
    ) -> Result<tonic::Response<proto::ApplyResponse>, tonic::Status> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.delay.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(250)).await;
            return Err(tonic::Status::deadline_exceeded(
                "Negative-only delayed response",
            ));
        }
        // Authenticated but malformed/missing proof: deliberately never positive.
        let mut r = tonic::Response::new(proto::ApplyResponse::default());
        r.metadata_mut()
            .insert("cache-control", "no-store".parse().unwrap());
        Ok(r)
    }
}
pub struct HostileRunning {
    pub server: Running,
    pub calls: Arc<AtomicUsize>,
    pub delay: Arc<AtomicBool>,
}
pub async fn hostile_server() -> anyhow::Result<HostileRunning> {
    let peer = Hostile {
        delay: Arc::new(AtomicBool::new(false)),
        calls: Arc::new(AtomicUsize::new(0)),
    };
    let service = platform::grpc_method_policy::guard_without_delegation(
        &deletion_rpc::POLICY,
        proto::account_deletion_service_server::AccountDeletionServiceServer::new(peer.clone()),
        CallerRegistry::new().register(Workload::Auth, AUTH)?,
    )?;
    Ok(HostileRunning {
        server: serve!(service, Workload::Concert),
        calls: peer.calls,
        delay: peer.delay,
    })
}
#[derive(Serialize, Deserialize)]
pub struct Saved {
    pub saga: Uuid,
    pub ticket: serde_json::Value,
    pub notification: serde_json::Value,
    pub pid: u32,
    pub pg_started: String,
}
fn saved_path() -> anyhow::Result<std::path::PathBuf> {
    Ok(environment::root()?.join("be42-runtime-private.json"))
}
pub fn save(value: &Saved) -> anyhow::Result<()> {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(saved_path()?)?;
    f.write_all(&serde_json::to_vec(value)?)?;
    f.sync_all()?;
    Ok(())
}
pub fn load() -> anyhow::Result<Saved> {
    let p = saved_path()?;
    let m = std::fs::symlink_metadata(&p)?;
    anyhow::ensure!(
        m.is_file()
            && !m.file_type().is_symlink()
            && m.permissions().mode() & 0o777 == 0o600
            && m.len() < 16384,
        "Private bounded owned handoff"
    );
    Ok(serde_json::from_slice(&std::fs::read(p)?)?)
}
