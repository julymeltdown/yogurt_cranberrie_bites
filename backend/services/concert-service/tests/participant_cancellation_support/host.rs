use concert_service::{
    adapters::out::{relationship_authority::guarded::*, repository::avatar::AvatarPersistence},
    bootstrap::{BootstrapFeatures, BootstrapService, ProductReleaseBundle},
    config::canonical_runtime::CanonicalRuntimeCore,
    participant::{
        self,
        live::{
            auth::GrpcCanonicalAuth,
            coordinator::ParticipantProfileJournal,
            profile::GrpcCanonicalSelfProfile,
            service::{GuardedParticipantService, ParticipantDeliveryPublisher},
            settlement::ParticipantNativeResolver,
        },
    },
    product_rpc::{
        self, delivery,
        scope_ledger::{
            ChildIntentSettler, ChildRecoveryKey, ChildRecoveryKeys, ProducerTransaction,
        },
        settlement::NativeTransactionResolver,
    },
    rpc,
};
use platform::{
    actor_delegation::{
        concert_self_profile::ConcertSelfProfileSigner,
        relationship_read::RelationshipReadSigner,
        response_scope::{ResponseScopeToken, RetainedScopeLayer},
        ActorDelegationVerifier,
    },
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::guard,
};
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
        Arc,
    },
    time::Duration,
};
use tokio::sync::mpsc;
use tonic::Status;
use uuid::Uuid;

fn env(name: &str) -> anyhow::Result<String> {
    Ok(std::env::var(name)?)
}
struct NoChildren;
#[async_trait::async_trait]
impl ChildIntentSettler for NoChildren {
    async fn recover_one(&self, _: &ChildRecoveryReference) -> Result<(), Status> {
        Err(Status::unavailable(
            "No child resolver in the cancellation read fixture",
        ))
    }
}
struct Observed {
    scope: Arc<OwnerGuardScope>,
    held: Option<RootReadPermit>,
}
struct Gate {
    publisher: delivery::DeliveryPublisher,
    registry: Arc<ScopeRegistry>,
    observed: mpsc::UnboundedSender<Observed>,
    calls: AtomicUsize,
    pool: PgPool,
    root: PathBuf,
}
#[async_trait::async_trait]
impl ParticipantDeliveryPublisher for Gate {
    async fn publish(
        &self,
        scope: Arc<OwnerGuardScope>,
    ) -> Result<(product_rpc::proto::ResponseDelivery, ResponseScopeToken), Status> {
        // Do not manufacture a capability or consume/replace a response body.
        let published = self.publisher.publish(scope.clone()).await?;
        let next = self.calls.load(Ordering::SeqCst) + 1;
        if !self.root.join(format!("cancel-arm-{next}.json")).is_file() {
            return Ok(published);
        }
        let index = self.calls.fetch_add(1, Ordering::SeqCst);
        if index == 2 {
            // Deliberate negative producer fixture: an actual native COMMIT
            // without a same-transaction Participant witness is not a receipt.
            // Registration precedes the COMMIT, with no escaped work task.
            let mut tx = self
                .pool
                .begin()
                .await
                .map_err(|_| Status::unavailable("Negative native fixture"))?;
            scope
                .register_transaction_in(
                    &mut tx,
                    [99; 32],
                    scope.binding().request_id().as_bytes().to_vec(),
                )
                .await?;
            tx.commit()
                .await
                .map_err(|_| Status::unavailable("Negative native fixture commit"))?;
        }
        let held = if index == 1 {
            Some(self.registry.lookup_root_read(&scope.root_read_view())?)
        } else {
            None
        };
        self.observed
            .send(Observed { scope, held })
            .map_err(|_| Status::unavailable("Fixture observer closed"))?;
        // This actual future is owned and destroyed by run_owned. The token
        // never reaches a serializer or HTTP client, and no EOF is synthesized.
        std::future::pending::<()>().await;
        Ok(published)
    }
}
fn marker(root: &std::path::Path, name: &str, value: serde_json::Value) -> anyhow::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(root.join(name))?;
    file.write_all(&serde_json::to_vec(&value)?)?;
    file.sync_all()?;
    Ok(())
}
async fn wait_marker(root: &std::path::Path, name: &str) -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(20), async {
        while !root.join(name).is_file() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    Ok(())
}
async fn owned_pool(root: &std::path::Path) -> anyhow::Result<PgPool> {
    let m = std::fs::symlink_metadata(root)?;
    anyhow::ensure!(
        root.is_absolute()
            && m.is_dir()
            && !m.file_type().is_symlink()
            && m.permissions().mode() & 0o077 == 0
    );
    anyhow::ensure!(format!("{}:{}", m.dev(), m.ino()) == env("MCR_CANCEL_ROOT_IDENTITY")?);
    let owner: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("mcr-owner.json"))?)?;
    anyhow::ensure!(
        owner["uid"].as_u64() == Some(m.uid() as u64)
            && owner["token"].as_str() == Some(env("MCR_CANCEL_ROOT_TOKEN")?.as_str())
    );
    let options = PgConnectOptions::from_str(&env("CONCERT_DATABASE_URL")?)?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_concert")
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none()
    );
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await?;
    let actual: (String,String,String,String) = sqlx::query_as("SELECT current_database(),current_setting('data_directory'),current_setting('listen_addresses'),(SELECT system_identifier::text FROM pg_control_system())").fetch_one(&pool).await?;
    anyhow::ensure!(
        actual
            == (
                "mcr_concert".into(),
                root.join("data").to_string_lossy().into_owned(),
                String::new(),
                env("MCR_CANCEL_SYSTEM")?
            )
    );
    sqlx::migrate::Migrator::new(concert_service::config::migration_directory().as_path())
        .await?
        .run(&pool)
        .await?;
    Ok(pool)
}
async fn observation(
    pool: &PgPool,
    scope: &OwnerGuardScope,
) -> anyhow::Result<(String, bool, bool, bool, bool)> {
    let producer = scope.producer_scope();
    Ok(sqlx::query_as("SELECT phase,serialized_at IS NOT NULL,delivery_completed_at IS NOT NULL,response_abandoned_at IS NOT NULL,cancellation_settled_at IS NOT NULL FROM concert_product_owner_scopes WHERE instance_id=$1 AND scope_id=$2")
        .bind(Uuid::parse_str(&producer.instance_id)?).bind(Uuid::parse_str(&producer.scope_id)?).fetch_one(pool).await?)
}
struct Server(tokio::task::JoinHandle<Result<(), tonic::transport::Error>>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub async fn run() -> anyhow::Result<()> {
    let root = PathBuf::from(env("MCR_CANCEL_ROOT")?);
    let pool = owned_pool(&root).await?;
    let repo = PathBuf::from(env("MCR_CANCEL_REPO")?);
    anyhow::ensure!(repo.is_absolute() && repo.canonicalize()? == repo);
    let bundle = repo.join("backend/releases/operator-bundle/quality-v6");
    let release = ProductReleaseBundle::from_source_bound_files(
        &bundle.join("bootstrap.json"),
        &repo.join("contracts/fixtures/assets/selected-static/avatar-manifest.json"),
        &repo.join("backend/releases/reference-scene-v2/quality-v6/map-manifest.json"),
        [
            &bundle.join("theme-danger.json"),
            &bundle.join("theme-parade.json"),
            &bundle.join("theme-revenge.json"),
        ],
        BootstrapFeatures::default(),
        &repo.join("frontend/public"),
        &repo.join("backend/releases/reference-scene-v2/quality-v6/source-binding.json"),
    )
    .map_err(|_| anyhow::anyhow!("Source-bound product release required"))?;
    let bootstrap = BootstrapService::new(pool.clone(), release.clone());
    let event = bootstrap
        .release_scope()
        .map_err(|_| anyhow::anyhow!("Release event"))?
        .0
        .uuid();
    sqlx::query("INSERT INTO concert_events(event_id,slug,title,phase) VALUES($1,$2,'C03 cancellation fixture','OPEN')")
        .bind(event).bind(format!("cancel-{event}")).execute(&pool).await?;
    let tls = platform::config::resolve_tls_material(true, "concert")?
        .ok_or_else(|| anyhow::anyhow!("Actual mTLS required"))?;
    let actors = ActorDelegationVerifier::from_env(true, Workload::Concert)?;
    let mut callers = CallerRegistry::new();
    for (role, key) in [
        (Workload::Gateway, "GATEWAY"),
        (Workload::Auth, "AUTH"),
        (Workload::Friend, "FRIEND"),
        (Workload::Profile, "PROFILE"),
    ] {
        callers = callers.register(role, &env(&format!("{key}_SERVICE_CREDENTIAL"))?)?;
    }
    let core = CanonicalRuntimeCore::new(
        pool.clone(),
        Uuid::new_v4(),
        Arc::new(ChildRecoveryKeys::new(
            ChildRecoveryKey::new("cancel-test", &[98; 32])?,
            vec![],
        )?),
        Arc::new(NoChildren),
    )?;
    let signer = RelationshipReadSigner::from_env(&actors)?;
    let credential = env("CONCERT_SERVICE_CREDENTIAL")?;
    let auth_url = env("CONCERT_PARTICIPANT_AUTH_GRPC_ENDPOINT")?;
    let fence = Arc::new(core.owner_fence(
        &auth_url,
        &env("CONCERT_PARTICIPANT_FRIEND_GRPC_ENDPOINT")?,
        &credential,
        &tls,
        signer.clone(),
    )?);
    let auth = GrpcCanonicalAuth::connect(&auth_url, &credential, Some(&tls), signer)?;
    let profile = GrpcCanonicalSelfProfile::connect(
        &env("CONCERT_PARTICIPANT_PROFILE_GRPC_ENDPOINT")?,
        &credential,
        Some(&tls),
        ConcertSelfProfileSigner::from_env(&actors)?,
    )?;
    let journal = Arc::new(
        ParticipantProfileJournal::from_product(pool.clone(), &release)
            .map_err(|_| anyhow::anyhow!("Product journal"))?,
    );
    let store = Arc::new(
        AvatarPersistence::from_product(pool.clone(), release.clone())
            .map_err(|_| anyhow::anyhow!("Product store"))?,
    );
    let (sender, mut observed) = mpsc::unbounded_channel();
    let service = GuardedParticipantService::new(
        store,
        auth,
        profile,
        fence,
        journal,
        Arc::new(Gate {
            publisher: core.delivery(),
            registry: core.registry(),
            observed: sender,
            calls: AtomicUsize::new(0),
            pool: pool.clone(),
            root: root.clone(),
        }),
    );
    let participant = guard(
        &participant::POLICY,
        service.server(),
        callers.clone(),
        actors.clone(),
    )?;
    let bootstrap = guard(
        &rpc::POLICY,
        rpc::proto::concert_service_server::ConcertServiceServer::new(bootstrap),
        callers.clone(),
        actors.clone(),
    )?;
    let settlement = guard(
        &product_rpc::settlement::POLICY,
        core.settlement_service(Arc::new(ParticipantNativeResolver::new(
            pool.clone(),
            core.ledger().instance(),
        )?))
        .server(),
        callers.clone(),
        actors.clone(),
    )?;
    let delivery = guard(
        &delivery::POLICY,
        product_rpc::proto::product_delivery_service_server::ProductDeliveryServiceServer::new(
            delivery::DeliveryService::new(core.delivery()),
        ),
        callers,
        actors,
    )?;
    let address: std::net::SocketAddr = env("CONCERT_GRPC_ADDR")?.parse()?;
    anyhow::ensure!(address.ip().is_loopback() && address.port() > 1024);
    let listener = tokio::net::TcpListener::bind(address).await?;
    let incoming = futures::stream::unfold(listener, |listener| async {
        Some((listener.accept().await.map(|(stream, _)| stream), listener))
    });
    let (shutdown, stopped) = tokio::sync::oneshot::channel();
    let server = Server(tokio::spawn(
        tonic::transport::Server::builder()
            .tls_config(tls.server_config()?)?
            .layer(RetainedScopeLayer)
            .layer(platform::call_deadline::CallDeadlineLayer)
            .add_service(bootstrap)
            .add_service(participant)
            .add_service(settlement)
            .add_service(delivery)
            .serve_with_incoming_shutdown(incoming, async {
                let _ = stopped.await;
            }),
    ));
    marker(
        &root,
        "cancel-ready.json",
        serde_json::json!({"release_digest":release.source_digest(),"event_id":event}),
    )?;
    let mut results = Vec::new();
    for ordinal in 1..=3 {
        let mut observed = tokio::time::timeout(Duration::from_secs(90), observed.recv())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Missing actual published scope"))?;
        anyhow::ensure!(
            observation(&pool, &observed.scope).await?
                == ("ACTIVE".into(), false, false, false, false)
        );
        marker(
            &root,
            &format!("cancel-gated-{ordinal}.json"),
            serde_json::json!({"published":true,"held_callback":observed.held.is_some()}),
        )?;
        wait_marker(&root, &format!("cancel-disconnected-{ordinal}.json")).await?;
        if ordinal == 1 {
            tokio::time::timeout(Duration::from_secs(6), async {
                loop {
                    let state = observation(&pool, &observed.scope).await?;
                    anyhow::ensure!(
                        !state.1 && !state.2,
                        "Cancellation invented serialization/delivery"
                    );
                    if state.0 == "ROOT_FINISHED" && state.3 && state.4 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Ok::<_, anyhow::Error>(())
            })
            .await??;
            marker(
                &root,
                "cancel-recovered.json",
                serde_json::json!({"abandoned":true,"settled":true,"serialized":false,"delivered":false}),
            )?;
            wait_marker(&root, "cancel-login-recovered.json").await?;
            results.push("interrupted public C03 read settled without HTTP completion; fresh real login checked by controller");
        } else if ordinal == 2 {
            // Longer than both original disclosure TTL and CR08's 2s cleanup.
            tokio::time::sleep(Duration::from_millis(4500)).await;
            anyhow::ensure!(
                observation(&pool, &observed.scope).await?
                    == ("SEALED".into(), false, false, false, false),
                "Unresolved admitted callback must quarantine past TTL"
            );
            anyhow::ensure!(
                core.registry()
                    .lookup_root_read(&observed.scope.root_read_view())
                    .is_err(),
                "Cancellation must close admission"
            );
            // Actual callback completion alone cannot invent a new supervisor
            // proof after the bounded cleanup has already become unresolved.
            drop(observed.held.take());
            tokio::time::sleep(Duration::from_millis(100)).await;
            anyhow::ensure!(
                observation(&pool, &observed.scope).await?
                    == ("SEALED".into(), false, false, false, false)
            );
            marker(
                &root,
                "cancel-unknown.json",
                serde_json::json!({"phase":"SEALED","abandoned":false,"settled":false,"serialized":false,"delivered":false,"expired_callback_did_not_release":true}),
            )?;
            results.push("unknown callback stays quarantined past TTL and actual callback release; no synthesized terminal proof");
        } else {
            tokio::time::sleep(Duration::from_millis(4500)).await;
            let state = observation(&pool, &observed.scope).await?;
            anyhow::ensure!(
                !state.1 && !state.2 && !state.4,
                "Unknown native outcome must not release owners or invent HTTP completion"
            );
            let producer = observed.scope.producer_scope();
            let count: i64 = sqlx::query_scalar("SELECT count(*) FROM concert_product_owner_transactions WHERE instance_id=$1 AND scope_id=$2")
                .bind(Uuid::parse_str(&producer.instance_id)?).bind(Uuid::parse_str(&producer.scope_id)?).fetch_one(&pool).await?;
            anyhow::ensure!(
                count == 1,
                "Unknown native fixture must have an actual durable registration"
            );
            let (identity,native): (serde_json::Value,Option<String>) = sqlx::query_as("SELECT identity,pg_xact_status((identity->>'full_xid')::xid8) FROM concert_product_owner_transactions WHERE instance_id=$1 AND scope_id=$2")
                .bind(Uuid::parse_str(&producer.instance_id)?).bind(Uuid::parse_str(&producer.scope_id)?).fetch_one(&pool).await?;
            anyhow::ensure!(
                native.as_deref() == Some("committed"),
                "Negative case requires an actual native COMMIT"
            );
            let identity: ProducerTransaction = serde_json::from_value(identity)?;
            let resolution =
                ParticipantNativeResolver::new(pool.clone(), core.ledger().instance())?
                    .resolve(&observed.scope.binding().owner_binding(), &identity)
                    .await?;
            anyhow::ensure!(
                resolution.status == proto::TransactionStatus::Unknown
                    && resolution.receipt_evidence.is_empty(),
                "COMMIT without an immutable witness must resolve UNKNOWN"
            );
            marker(
                &root,
                "cancel-unknown-native.json",
                serde_json::json!({"phase":state.0,"abandoned":state.3,"settled":false,"serialized":false,"delivered":false,"registered_transactions":count,"native_status":"committed","resolution":"UNKNOWN","receipt_evidence":false}),
            )?;
            results.push("registered actual native COMMIT without witness remains unknown; owner fences retained");
        }
    }
    wait_marker(&root, "cancel-stop.json").await?;
    let _ = shutdown.send(());
    tokio::time::timeout(Duration::from_secs(3), async {
        while !server.0.is_finished() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    println!(
        "C03-CANCEL01 native host PASS: {}; {}; {}",
        results[0], results[1], results[2]
    );
    Ok(())
}
