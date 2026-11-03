//! Fixtures for the configured Gateway Profile client against the real Profile service.
//!
//! Provenance: JWT key/context helpers follow the read-only pattern in the Gateway Profile
//! client transport test, and the owned-cluster checks follow the Profile self-RPC fixture at
//! 4b9d054 (retained as af8b458). Nothing is imported wholesale. No controlled owner or
//! hand-written reply exists here: the guarded generated server is built from the real
//! Profile AppState over the minimal SQL owner and an explicit native PostgreSQL 16 fixture.
#![allow(dead_code)]
use api_gateway::adapters::out::{
    jwt::{concert_access::ConcertAccessContext, JwtVerifier},
    profile_client::GrpcProfileClient,
};
use api_gateway::config::GatewayMetrics;
use base64ct::{Base64UrlUnpadded, Encoding};
use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use platform::{
    actor_delegation::{ActorDelegationSigner, ActorDelegationVerifier},
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, Workload},
};
use profile_service::adapters::r#in::grpc::{
    method_policy::PROFILE_METHOD_POLICY,
    server::{proto, ProfileGrpcServer},
};
use profile_service::config::{build_state, AppConfig, AppState};
use rand::rngs::OsRng;
use rsa::{pkcs8::EncodePrivateKey, traits::PublicKeyParts, RsaPrivateKey, RsaPublicKey};
use serde_json::{json, Value};
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
        Arc, OnceLock,
    },
    task::{Context, Poll},
    time::Duration,
};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use uuid::Uuid;

pub const GATEWAY_TOKEN: &str = "gateway-profile-owner-fixture-token";
pub const DATABASE: &str = "mcr_gateway_profile_owner";
pub const PRIVATE_ROLE_CANARY: &str = "PRIVATE_ROLE_CANARY";

pub fn registry() -> CallerRegistry {
    CallerRegistry::new()
        .register(Workload::Gateway, GATEWAY_TOKEN)
        .unwrap()
        .register(Workload::Post, "post-owner-fixture-token")
        .unwrap()
}

/// Counts HTTP requests that reach the guarded server, before caller or actor checks.
#[derive(Clone)]
pub struct CountLayer(pub Arc<AtomicUsize>);
#[derive(Clone)]
pub struct Count<S> {
    inner: S,
    count: Arc<AtomicUsize>,
}
impl<S> tower::Layer<S> for CountLayer {
    type Service = Count<S>;
    fn layer(&self, inner: S) -> Count<S> {
        Count {
            inner,
            count: self.0.clone(),
        }
    }
}
impl<S, R> tower::Service<R> for Count<S>
where
    S: tower::Service<R>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;
    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }
    fn call(&mut self, request: R) -> S::Future {
        self.count.fetch_add(1, Ordering::SeqCst);
        self.inner.call(request)
    }
}

/// Explicit test configuration: no seeding, no workers, no source-default database.
pub async fn profile_state(pool: PgPool) -> anyhow::Result<AppState> {
    let config = AppConfig {
        http_addr: "127.0.0.1:0".parse()?,
        metrics_addr: "127.0.0.1:0".parse()?,
        grpc_addr: "127.0.0.1:0".parse()?,
        database_url: "unused-explicit-gateway-profile-owner-config".into(),
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
        mutation_receipt_retention_worker_interval_ms: 3_600_000,
        mutation_receipt_retention_batch_size: 100,
    };
    build_state(&config, pool).await
}

pub struct Running {
    pub url: String,
    pub requests: Arc<AtomicUsize>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), tonic::transport::Error>>>,
}
impl Running {
    /// Actual generated Profile server, production method policy, caller registry,
    /// actor verifier and call-deadline layer over the real Profile AppState.
    pub async fn start(state: AppState) -> anyhow::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("http://{}", listener.local_addr()?);
        let requests = Arc::new(AtomicUsize::new(0));
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
                .layer(CountLayer(requests.clone()))
                .layer(CallDeadlineLayer)
                .add_service(guarded)
                .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                    let _ = stopped.await;
                }),
        );
        Ok(Self {
            url,
            requests,
            stop: Some(stop),
            task: Some(task),
        })
    }
    pub async fn client(&self) -> anyhow::Result<GrpcProfileClient> {
        self.client_with(GatewayMetrics::default()).await
    }
    pub async fn client_with(&self, metrics: GatewayMetrics) -> anyhow::Result<GrpcProfileClient> {
        GrpcProfileClient::connect_with_metrics(
            self.url.clone(),
            Workload::Gateway,
            GATEWAY_TOKEN,
            None,
            ActorDelegationSigner::local_parity(Workload::Gateway),
            metrics,
        )
        .await
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

struct Key {
    encoding: EncodingKey,
    public: RsaPublicKey,
}
fn key() -> &'static Key {
    static KEY: OnceLock<Key> = OnceLock::new();
    KEY.get_or_init(|| {
        let private = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
        let pem = private.to_pkcs8_pem(Default::default()).unwrap();
        Key {
            encoding: EncodingKey::from_rsa_pem(pem.as_bytes()).unwrap(),
            public: RsaPublicKey::from(&private),
        }
    })
}
fn jwk() -> Value {
    let key = key();
    json!({"kid":"gateway-profile-owner","kty":"RSA","alg":"RS256","use":"sig",
        "n":Base64UrlUnpadded::encode_string(&key.public.n().to_bytes_be()),
        "e":Base64UrlUnpadded::encode_string(&key.public.e().to_bytes_be())})
}
/// A real configured-key verification result for `user`; no forged context constructor.
pub fn context(user: Uuid, original_budget: Duration) -> ConcertAccessContext {
    context_with_exp(user, original_budget, Utc::now().timestamp() + 900)
}
pub fn context_with_exp(user: Uuid, original_budget: Duration, exp: i64) -> ConcertAccessContext {
    let verifier = JwtVerifier::from_jwks_json(
        "fixture-auth",
        "fixture-api",
        &json!({"keys":[jwk()]}).to_string(),
    )
    .unwrap();
    let claims = json!({"sub":user.to_string(),"sid":Uuid::new_v4().to_string(),
        "iss":"fixture-auth","aud":"fixture-api","token_type":"access","exp":exp,
        "roles":[PRIVATE_ROLE_CANARY]});
    let token = jsonwebtoken::encode(
        &Header {
            kid: Some("gateway-profile-owner".into()),
            ..Header::new(Algorithm::RS256)
        },
        &claims,
        &key().encoding,
    )
    .unwrap();
    verifier
        .verify_concert_access(
            &token,
            Some((tokio::time::Instant::now() + original_budget).into_std()),
        )
        .unwrap()
}

pub fn owned_options() -> anyhow::Result<PgConnectOptions> {
    let root = PathBuf::from(std::env::var("MCR_GATEWAY_PROFILE_OWNER_ROOT")?);
    let metadata = root.symlink_metadata()?;
    anyhow::ensure!(
        root.is_absolute()
            && metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && metadata.permissions().mode() & 0o077 == 0,
        "private owned root required"
    );
    anyhow::ensure!(
        root.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("mcr-pg-")),
        "dedicated native root required"
    );
    anyhow::ensure!(
        format!("{}:{}", metadata.dev(), metadata.ino())
            == std::env::var("MCR_GATEWAY_PROFILE_OWNER_IDENTITY")?,
        "fixture inode changed"
    );
    let marker_path = root.join("mcr-owner.json");
    let marker_meta = marker_path.symlink_metadata()?;
    anyhow::ensure!(
        marker_meta.is_file()
            && !marker_meta.file_type().is_symlink()
            && marker_meta.permissions().mode() & 0o077 == 0,
        "private marker required"
    );
    let marker: Value = serde_json::from_slice(&std::fs::read(marker_path)?)?;
    anyhow::ensure!(
        marker["token"].as_str()
            == Some(std::env::var("MCR_GATEWAY_PROFILE_OWNER_TOKEN")?.as_str())
            && marker["uid"].as_u64() == Some(metadata.uid() as u64),
        "fixture owner mismatch"
    );
    let options =
        PgConnectOptions::from_str(&std::env::var("MCR_GATEWAY_PROFILE_OWNER_DATABASE_URL")?)
            .map_err(|_| anyhow::anyhow!("invalid owned connection"))?;
    anyhow::ensure!(
        options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_database() == Some(DATABASE)
            && options.get_username() == "mcr_owner"
            && options.get_options().is_none(),
        "foreign connection refused"
    );
    Ok(options)
}
pub async fn owner_pool() -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_millis(500))
        .connect_with(owned_options()?)
        .await
        .map_err(|_| anyhow::anyhow!("owned fixture unavailable"))?;
    let directory: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        directory
            == PathBuf::from(std::env::var("MCR_GATEWAY_PROFILE_OWNER_ROOT")?)
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
/// One connection: stale or leaked owner work shows up as pool exhaustion.
pub async fn reader_pool(role: &'static str) -> anyhow::Result<PgPool> {
    Ok(PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_millis(150))
        .after_connect(move |connection, _| {
            Box::pin(async move {
                sqlx::query(&format!("set role {role}"))
                    .execute(&mut *connection)
                    .await?;
                sqlx::query("set default_transaction_read_only=on")
                    .execute(&mut *connection)
                    .await?;
                Ok(())
            })
        })
        .connect_with(owned_options()?)
        .await
        .map_err(|_| anyhow::anyhow!("owned reader unavailable"))?)
}
pub async fn seed(pool: &PgPool, nickname: &str, bio: &str) -> anyhow::Result<Uuid> {
    let user = Uuid::new_v4();
    sqlx::query("insert into profiles(user_id,username,nickname,bio,avatar_url,account_visibility,follower_count,following_count,post_count,delete_reason) values($1,$2,$3,$4,'https://private.invalid/?token=legacy-token-canary','PRIVATE',971,972,973,'private-delete-canary')")
        .bind(user)
        .bind(format!("private-handle-{user}"))
        .bind(nickname)
        .bind(bio)
        .execute(pool)
        .await?;
    Ok(user)
}

/// Never connects: used where the production guard must refuse before owner work.
pub fn lazy_pool() -> PgPool {
    PgPoolOptions::new().connect_lazy_with(
        PgConnectOptions::new()
            .host("/invalid-mcr-gateway-profile-owner")
            .username("unused")
            .database("unused"),
    )
}
