//! Explicit fixtures for the generated guarded service. No Docker or default database.
#![allow(dead_code)]
use platform::actor_delegation::{ActorDelegationSigner, ActorDelegationVerifier, HEADER};
use platform::call_deadline::{CallDeadlineLayer, REMAINING_HEADER};
use platform::grpc_auth::{CallerRegistry, Workload};
use profile_application::concert_identity::*;
use profile_service::adapters::r#in::grpc::{
    method_policy::PROFILE_METHOD_POLICY,
    server::{proto, ProfileGrpcServer},
};
use profile_service::config::{build_state, AppConfig, AppState};
use prost::Message;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    str::FromStr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};
use tonic::codegen::tokio_stream::wrappers::TcpListenerStream;
use tonic::{
    transport::{Channel, Endpoint, Server},
    Request, Response, Status,
};
use uuid::Uuid;

pub const METHOD: &str = "/profile.v1.ProfileService/ReadConcertSelfProfile";
pub type Msg = proto::ReadConcertSelfProfileRequest;
pub type Reply = proto::ReadConcertSelfProfileResponse;
pub fn registry() -> CallerRegistry {
    CallerRegistry::new()
        .register(Workload::Gateway, "profile-rpc-gateway-fixture")
        .unwrap()
        .register(Workload::Post, "profile-rpc-post-fixture")
        .unwrap()
        .register(Workload::Auth, "profile-rpc-auth-fixture")
        .unwrap()
        .register(Workload::Friend, "profile-rpc-friend-fixture")
        .unwrap()
}
pub fn lazy_pool() -> PgPool {
    PgPoolOptions::new().connect_lazy_with(
        PgConnectOptions::new()
            .host("/invalid-mcr-profile-rpc-fixture")
            .username("unused")
            .database("unused"),
    )
}
pub async fn state(
    pool: PgPool,
    repository: Option<Arc<dyn ConcertProfileRepository>>,
) -> anyhow::Result<AppState> {
    let config = AppConfig {
        http_addr: "127.0.0.1:0".parse()?,
        metrics_addr: "127.0.0.1:0".parse()?,
        grpc_addr: "127.0.0.1:0".parse()?,
        database_url: "unused-explicit-profile-rpc-config".into(),
        database_migration_mode: migration_runtime::MigrationMode::Startup,
        database_pool_max_connections: 2,
        seed_enabled: false,
        caller_registry: registry(),
        actor_delegation: ActorDelegationVerifier::local_parity(Workload::Profile),
        tls_material: None,
        retention_purge_worker_enabled: false,
        retention_purge_worker_interval_ms: 1000,
        retention_purge_actor_id: Uuid::from_u128(1).to_string(),
        mutation_receipt_retention_worker_enabled: false,
        mutation_receipt_retention_worker_interval_ms: 3600000,
        mutation_receipt_retention_batch_size: 100,
        naver_oauth: profile_service::identity::NaverOAuthConfig::local(),
        identity_subject_salt: b"profile-service-test-identity-subject-salt".to_vec(),
    };
    let mut state = build_state(&config, pool).await?;
    if let Some(repository) = repository {
        state.concert_self_profile = ReadConcertSelfProfile::new(repository);
    }
    Ok(state)
}
pub struct Running {
    pub client: proto::profile_service_client::ProfileServiceClient<Channel>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), tonic::transport::Error>>>,
}
impl Running {
    pub async fn start(state: AppState) -> anyhow::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let (stop, stopped) = oneshot::channel();
        let guarded = platform::grpc_method_policy::guard(
            &PROFILE_METHOD_POLICY,
            proto::profile_service_server::ProfileServiceServer::new(ProfileGrpcServer::new(
                Arc::new(state),
            )),
            registry(),
            ActorDelegationVerifier::local_parity(Workload::Profile),
        )?;
        let task = tokio::spawn(
            Server::builder()
                .layer(CallDeadlineLayer)
                .add_service(guarded)
                .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                    let _ = stopped.await;
                }),
        );
        let client = proto::profile_service_client::ProfileServiceClient::new(
            Endpoint::from_shared(endpoint)?.connect().await?,
        );
        Ok(Self {
            client,
            stop: Some(stop),
            task: Some(task),
        })
    }
    pub async fn stop(mut self) -> anyhow::Result<()> {
        let _ = self.stop.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(3), self.task.take().unwrap()).await???;
        Ok(())
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}
pub fn message(user: Uuid) -> Msg {
    Msg {
        user_id: user.to_string(),
    }
}
pub fn frame(message: &Msg) -> Vec<u8> {
    let bytes = message.encode_to_vec();
    let mut frame = vec![0];
    frame.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    frame.extend(bytes);
    frame
}
pub fn token(message: &Msg, actor: Uuid) -> String {
    ActorDelegationSigner::local_parity(Workload::Gateway).issue(
        Workload::Profile,
        METHOD,
        actor,
        None,
        &frame(message),
        chrono::Utc::now().timestamp(),
    )
}
pub fn request(message: Msg, proof: Option<&str>, remaining: Option<&str>) -> Request<Msg> {
    let mut r = Request::new(message);
    r.metadata_mut()
        .insert("x-service-caller", "gateway".parse().unwrap());
    r.metadata_mut().insert(
        "x-service-token",
        "profile-rpc-gateway-fixture".parse().unwrap(),
    );
    if let Some(p) = proof {
        r.metadata_mut().insert(HEADER, p.parse().unwrap());
    }
    if let Some(d) = remaining {
        r.metadata_mut()
            .insert(REMAINING_HEADER, d.parse().unwrap());
    }
    r
}
pub async fn call(server: &mut Running, msg: Msg) -> Result<Response<Reply>, Status> {
    let proof = token(&msg, Uuid::parse_str(&msg.user_id).unwrap());
    server
        .client
        .read_concert_self_profile(request(msg, Some(&proof), Some("2S")))
        .await
}
#[derive(Clone, Copy)]
pub enum Mode {
    Success,
    Missing,
    Error,
    Malformed,
    Misbound,
    Delay(Duration),
    Block(Duration),
    Hold,
}
pub struct Controlled {
    pub mode: Mutex<Mode>,
    pub calls: AtomicUsize,
    pub completed: AtomicUsize,
    pub dropped: AtomicUsize,
}
impl Controlled {
    pub fn new(mode: Mode) -> Arc<Self> {
        Arc::new(Self {
            mode: Mutex::new(mode),
            calls: AtomicUsize::new(0),
            completed: AtomicUsize::new(0),
            dropped: AtomicUsize::new(0),
        })
    }
}
struct Dropped<'a>(&'a AtomicUsize);
impl Drop for Dropped<'_> {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[async_trait::async_trait]
impl ConcertProfileRepository for Controlled {
    async fn find_self_content(
        &self,
        q: ConcertProfileQuery,
    ) -> Result<Option<ConcertProfileRecord>, ConcertProfileError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let _dropped = Dropped(&self.dropped);
        let mode = self.mode.lock().unwrap().clone();
        match mode {
            Mode::Missing => return Ok(None),
            Mode::Error => return Err(ConcertProfileError::DependencyUnavailable),
            Mode::Delay(d) => tokio::time::sleep(d).await,
            Mode::Block(d) => std::thread::sleep(d),
            Mode::Hold => std::future::pending::<()>().await,
            _ => {}
        }
        self.completed.fetch_add(1, Ordering::SeqCst);
        Ok(Some(ConcertProfileRecord {
            user_id: if matches!(mode, Mode::Misbound) {
                Uuid::new_v4()
            } else {
                q.user_id()
            },
            nickname: if matches!(mode, Mode::Malformed) {
                "invalid\ncontent".into()
            } else {
                "Mira".into()
            },
            bio: "Self content".into(),
            observed_at: chrono::Utc::now(),
        }))
    }
}
pub async fn controlled(mode: Mode) -> anyhow::Result<(Running, Arc<Controlled>)> {
    let owner = Controlled::new(mode);
    Ok((
        Running::start(state(lazy_pool(), Some(owner.clone())).await?).await?,
        owner,
    ))
}

pub fn owned_options() -> anyhow::Result<PgConnectOptions> {
    let root = PathBuf::from(std::env::var("MCR_PROFILE_RPC_OWNER_ROOT")?);
    let metadata = root.symlink_metadata()?;
    anyhow::ensure!(
        root.is_absolute()
            && metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && metadata.permissions().mode() & 0o077 == 0,
        "private owned root required"
    );
    anyhow::ensure!(
        root.to_string_lossy().starts_with("/private/tmp/mcr-pg-")
            || root.to_string_lossy().starts_with("/tmp/mcr-pg-"),
        "dedicated native root required"
    );
    anyhow::ensure!(
        format!("{}:{}", metadata.dev(), metadata.ino())
            == std::env::var("MCR_PROFILE_RPC_OWNER_IDENTITY")?,
        "fixture inode changed"
    );
    let marker_path = root.join("mcr-owner.json");
    let meta = marker_path.symlink_metadata()?;
    anyhow::ensure!(
        meta.is_file() && !meta.file_type().is_symlink() && meta.permissions().mode() & 0o077 == 0,
        "private marker required"
    );
    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(marker_path)?)?;
    anyhow::ensure!(
        marker["token"].as_str() == Some(std::env::var("MCR_PROFILE_RPC_OWNER_TOKEN")?.as_str())
            && marker["uid"].as_u64() == Some(metadata.uid() as u64),
        "fixture owner mismatch"
    );
    let opts = PgConnectOptions::from_str(&std::env::var("MCR_PROFILE_RPC_DATABASE_URL")?)
        .map_err(|_| anyhow::anyhow!("invalid owned connection"))?;
    anyhow::ensure!(
        opts.get_socket() == Some(&root.join("socket"))
            && opts.get_port() == 5432
            && opts.get_database() == Some("mcr_profile_self_rpc")
            && opts.get_username() == "mcr_owner"
            && opts.get_options().is_none(),
        "foreign connection refused"
    );
    Ok(opts)
}
pub async fn owner_pool() -> anyhow::Result<PgPool> {
    let opts = owned_options()?;
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .acquire_timeout(Duration::from_millis(500))
        .connect_with(opts)
        .await
        .map_err(|_| anyhow::anyhow!("owned fixture unavailable"))?;
    let directory: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        directory
            == PathBuf::from(std::env::var("MCR_PROFILE_RPC_OWNER_ROOT")?)
                .join("data")
                .to_string_lossy(),
        "foreign directory refused"
    );
    let version: i32 = sqlx::query_scalar("select current_setting('server_version_num')::int")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!((160000..170000).contains(&version), "PG16 required");
    Ok(pool)
}
pub async fn reader_pool() -> anyhow::Result<PgPool> {
    Ok(PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_millis(150))
        .after_connect(|conn, _| {
            Box::pin(async move {
                sqlx::query("set role mcr_profile_rpc_reader")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("set default_transaction_read_only=on")
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
        .connect_with(owned_options()?)
        .await
        .map_err(|_| anyhow::anyhow!("owned reader unavailable"))?)
}
pub async fn seed(pool: &PgPool, name: &str, bio: &str) -> anyhow::Result<Uuid> {
    let user = Uuid::new_v4();
    sqlx::query("insert into profiles(user_id,username,nickname,bio,avatar_url,account_visibility,follower_count,following_count,post_count,delete_reason) values($1,$2,$3,$4,'https://private.invalid/?token=legacy-token-canary','PRIVATE',971,972,973,'private-delete-canary')")
        .bind(user).bind(format!("private-handle-{user}")).bind(name).bind(bio).execute(pool).await?;
    Ok(user)
}
