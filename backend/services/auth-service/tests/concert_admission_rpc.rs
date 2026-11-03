//! Generated Auth service behind production caller/actor guard and deadline layer.
#[path = "support/auth_fixture_config.rs"]
mod auth_fixture_config;
use auth_application::concert_identity::*;
use auth_service::adapters::r#in::grpc::{
    method_policy::AUTH_METHOD_POLICY,
    server::{proto, AuthGrpcServer},
};
use auth_service::config::{build_state, AppState};
use platform::actor_delegation::{
    ActorDelegationSigner, ActorDelegationVerifier, HEADER, LIFETIME_SECONDS,
};
use platform::call_deadline::{CallDeadlineLayer, REMAINING_HEADER};
use platform::grpc_auth::{CallerRegistry, Workload};
use prost::Message;
use rsa::pkcs8::EncodePrivateKey;
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
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
use tokio::{
    net::TcpListener,
    sync::{oneshot, Notify},
    task::JoinHandle,
};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{
    transport::{Channel, Endpoint, Server},
    Code, Request, Response, Status,
};
use uuid::Uuid;
const METHOD: &str = "/auth.v1.AuthService/ReadConcertAdmission";
async fn owned_pool() -> anyhow::Result<PgPool> {
    let database = "mcr_auth_admission_rpc";
    let root = PathBuf::from(
        std::env::var("AUTH_ADMISSION_RPC_TEST_OWNER_ROOT")
            .map_err(|_| anyhow::anyhow!("owned fixture root required"))?,
    );
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        root.is_absolute()
            && info.is_dir()
            && !info.file_type().is_symlink()
            && info.permissions().mode() & 0o077 == 0,
        "private owned root required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino())
            == std::env::var("AUTH_ADMISSION_RPC_TEST_OWNER_IDENTITY")?,
        "fixture inode changed"
    );
    let marker_path = root.join("mcr-owner.json");
    let marker_info = std::fs::symlink_metadata(&marker_path)?;
    anyhow::ensure!(
        marker_info.is_file()
            && !marker_info.file_type().is_symlink()
            && marker_info.permissions().mode() & 0o077 == 0,
        "private marker required"
    );
    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(marker_path)?)?;
    anyhow::ensure!(
        marker["token"].as_str()
            == Some(std::env::var("AUTH_ADMISSION_RPC_TEST_OWNER_TOKEN")?.as_str())
            && marker["uid"].as_u64() == Some(info.uid() as u64),
        "fixture owner mismatch"
    );
    let options =
        PgConnectOptions::from_str(&std::env::var("AUTH_ADMISSION_RPC_TEST_DATABASE_URL")?)
            .map_err(|_| anyhow::anyhow!("invalid owned fixture options"))?;
    anyhow::ensure!(
        options.get_database() == Some(database)
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await
        .map_err(|_| anyhow::anyhow!("owned fixture connection failed"))?;
    let actual: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        actual == root.join("data").to_string_lossy(),
        "foreign data directory refused"
    );
    Ok(pool)
}

async fn state(
    pool: PgPool,
    port: Option<Arc<dyn CurrentAdmissionPort>>,
) -> anyhow::Result<AppState> {
    static KEY: OnceLock<String> = OnceLock::new();
    let key = KEY.get_or_init(|| {
        rsa::RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048)
            .unwrap()
            .to_pkcs8_pem(Default::default())
            .unwrap()
            .to_string()
    });
    let mut config = auth_fixture_config::test_config("unused-explicit-test-config".into())?;
    config.jwt_private_key_pem = Some(key.clone());
    config.jwt_key_id = Some("rpc-fixture-only".into());
    let mut state = build_state(&config, pool).await?;
    if let Some(port) = port {
        state.concert_admission = port;
    }
    Ok(state)
}
async fn native_fixture() -> anyhow::Result<(PgPool, proto::ReadConcertAdmissionRequest)> {
    let pool = owned_pool().await?;
    let user = Uuid::new_v4();
    let family = Uuid::new_v4();
    sqlx::query("insert into users(id,primary_email,password_hash,status) values($1,$2,'private-rpc-password-hash','ACTIVE')").bind(user).bind(format!("{user}@private-rpc.invalid")).execute(&pool).await?;
    sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) values($1,'private-rpc-token-hash',$2,$3,clock_timestamp()+interval '1 hour')").bind(Uuid::new_v4().to_string()).bind(user).bind(family).execute(&pool).await?;
    Ok((
        pool,
        proto::ReadConcertAdmissionRequest {
            user_id: user.to_string(),
            session_family_id: family.to_string(),
            binding: None,
        },
    ))
}
#[tokio::test]
async fn native_email_only_rpc_uses_real_owner_and_excludes_private_contents() -> anyhow::Result<()>
{
    let (pool, msg) = native_fixture().await?;
    let mut server = Running::start(state(pool.clone(), None).await?).await?;
    let mut lock = pool.begin().await?;
    sqlx::query("lock table user_identities in access exclusive mode")
        .execute(&mut *lock)
        .await?;
    let response = call(&mut server, msg).await?;
    let facts = response.get_ref();
    assert!(facts.account_active && facts.session_live);
    assert_eq!(facts.binding_state, 1);
    assert!(facts.observed_at_unix_micros > 0);
    let wire = facts.encode_to_vec();
    for secret in ["private-rpc-password", "private-rpc-token", "@private-rpc"] {
        assert!(!wire
            .windows(secret.len())
            .any(|bytes| bytes == secret.as_bytes()));
    }
    lock.rollback().await?;
    server.stop().await
}
#[tokio::test]
async fn native_current_account_and_session_negatives_are_honest_rpc_facts() -> anyhow::Result<()> {
    let (pool, msg) = native_fixture().await?;
    let mut server = Running::start(state(pool.clone(), None).await?).await?;
    let user = Uuid::parse_str(&msg.user_id)?;
    let mut missing = msg.clone();
    missing.session_family_id = Uuid::new_v4().to_string();
    let response = call(&mut server, missing).await?.into_inner();
    assert!(response.account_active);
    assert!(!response.session_live);
    sqlx::query("update refresh_tokens set expires_at=clock_timestamp()-interval '1 second' where user_id=$1").bind(user).execute(&pool).await?;
    assert!(!call(&mut server, msg.clone()).await?.get_ref().session_live);
    sqlx::query("update refresh_tokens set expires_at=clock_timestamp()+interval '1 hour',revoked_at=clock_timestamp() where user_id=$1").bind(user).execute(&pool).await?;
    assert!(!call(&mut server, msg.clone()).await?.get_ref().session_live);
    sqlx::query("update users set status='PENDING_VERIFICATION' where id=$1")
        .bind(user)
        .execute(&pool)
        .await?;
    let facts = call(&mut server, msg.clone()).await?.into_inner();
    assert!(!facts.account_active && !facts.session_live);
    sqlx::query("delete from users where id=$1")
        .bind(user)
        .execute(&pool)
        .await?;
    let facts = call(&mut server, msg).await?.into_inner();
    assert!(!facts.account_active && !facts.session_live);
    server.stop().await
}
#[tokio::test]
async fn native_exact_foreign_deleted_relinked_provider_references_are_owner_checked(
) -> anyhow::Result<()> {
    let (pool, mut msg) = native_fixture().await?;
    let (_, other) = native_fixture().await?;
    let id = Uuid::new_v4();
    let user = Uuid::parse_str(&msg.user_id)?;
    let epoch:i64=sqlx::query_scalar("insert into user_identities(id,user_id,provider_type,provider_subject,email) values($1,$2,'X','private-rpc-subject','private-rpc-email@example.invalid') returning binding_epoch").bind(id).bind(user).fetch_one(&pool).await?;
    let exact = proto::ConcertBindingReference {
        identity_id: id.to_string(),
        provider: "X".into(),
        binding_epoch: epoch,
    };
    msg.binding = Some(exact.clone());
    let mut server = Running::start(state(pool.clone(), None).await?).await?;
    assert_eq!(
        call(&mut server, msg.clone())
            .await?
            .get_ref()
            .binding_state,
        2
    );
    let mut foreign = other;
    foreign.binding = Some(exact.clone());
    assert_eq!(call(&mut server, foreign).await?.get_ref().binding_state, 3);
    let new_epoch: i64 = sqlx::query_scalar(
        "update user_identities set linked_at=now() where id=$1 returning binding_epoch",
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;
    assert_ne!(new_epoch, epoch);
    assert_eq!(
        call(&mut server, msg.clone())
            .await?
            .get_ref()
            .binding_state,
        3
    );
    msg.binding.as_mut().unwrap().binding_epoch = new_epoch;
    assert_eq!(
        call(&mut server, msg.clone())
            .await?
            .get_ref()
            .binding_state,
        2
    );
    sqlx::query("update user_identities set deleted_at=now() where id=$1")
        .bind(id)
        .execute(&pool)
        .await?;
    assert_eq!(
        call(&mut server, msg.clone())
            .await?
            .get_ref()
            .binding_state,
        3
    );
    msg.binding = None;
    assert_eq!(call(&mut server, msg).await?.get_ref().binding_state, 1);
    server.stop().await
}
#[tokio::test]
async fn native_rpc_deadline_and_one_connection_pool_cleanup_recover() -> anyhow::Result<()> {
    let (pool, msg) = native_fixture().await?;
    let limited = PgPoolOptions::new()
        .max_connections(1)
        .connect_with((*pool.connect_options()).clone())
        .await?;
    let mut server = Running::start(state(limited.clone(), None).await?).await?;
    let mut lock = pool.begin().await?;
    sqlx::query("lock table users in access exclusive mode")
        .execute(&mut *lock)
        .await?;
    let proof = token(&msg, Uuid::parse_str(&msg.user_id)?);
    let status = server
        .client
        .read_concert_admission(request(msg.clone(), Some(&proof), Some("50m")))
        .await
        .unwrap_err();
    assert_eq!(status.code(), Code::DeadlineExceeded);
    lock.rollback().await?;
    assert!(call(&mut server, msg).await?.get_ref().session_live);
    let timeout: String = sqlx::query_scalar("show statement_timeout")
        .fetch_one(&limited)
        .await?;
    assert_eq!(timeout, "0");
    server.stop().await
}
#[tokio::test]
async fn native_closed_owner_returns_safe_unavailable_without_fallback() -> anyhow::Result<()> {
    let (pool, msg) = native_fixture().await?;
    let mut server = Running::start(state(pool.clone(), None).await?).await?;
    pool.close().await;
    let status = call(&mut server, msg).await.unwrap_err();
    assert_eq!(status.code(), Code::Unavailable);
    assert_eq!(status.message(), "Admission owner unavailable");
    server.stop().await
}
fn lazy_pool() -> PgPool {
    PgPoolOptions::new().connect_lazy_with(
        PgConnectOptions::new()
            .host("127.0.0.1")
            .port(9)
            .username("unused_rpc_fixture")
            .database("mcr_rpc_no_connection"),
    )
}
struct Running {
    client: proto::auth_service_client::AuthServiceClient<Channel>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), tonic::transport::Error>>>,
}
impl Running {
    async fn start(state: AppState) -> anyhow::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let (stop, stopped) = oneshot::channel();
        let guarded = platform::grpc_method_policy::guard(
            &AUTH_METHOD_POLICY,
            proto::auth_service_server::AuthServiceServer::new(AuthGrpcServer::new(Arc::new(
                state,
            ))),
            CallerRegistry::new()
                .register(Workload::Gateway, "rpc-gateway-fixture")?
                .register(Workload::Post, "rpc-post-fixture")?,
            ActorDelegationVerifier::local_parity(Workload::Auth),
        )?;
        let task = tokio::spawn(
            Server::builder()
                .layer(CallDeadlineLayer)
                .add_service(guarded)
                .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                    let _ = stopped.await;
                }),
        );
        let client = proto::auth_service_client::AuthServiceClient::new(
            Endpoint::from_shared(endpoint)?.connect().await?,
        );
        Ok(Self {
            client,
            stop: Some(stop),
            task: Some(task),
        })
    }
    async fn stop(mut self) -> anyhow::Result<()> {
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
fn message(user: Uuid) -> proto::ReadConcertAdmissionRequest {
    proto::ReadConcertAdmissionRequest {
        user_id: user.to_string(),
        session_family_id: Uuid::new_v4().to_string(),
        binding: None,
    }
}
fn frame(message: &proto::ReadConcertAdmissionRequest) -> Vec<u8> {
    let bytes = message.encode_to_vec();
    let mut frame = vec![0];
    frame.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    frame.extend(bytes);
    frame
}
fn token(message: &proto::ReadConcertAdmissionRequest, actor: Uuid) -> String {
    ActorDelegationSigner::local_parity(Workload::Gateway).issue(
        Workload::Auth,
        METHOD,
        actor,
        None,
        &frame(message),
        chrono::Utc::now().timestamp(),
    )
}
fn request(
    message: proto::ReadConcertAdmissionRequest,
    proof: Option<&str>,
    remaining: Option<&str>,
) -> Request<proto::ReadConcertAdmissionRequest> {
    let mut request = Request::new(message);
    request
        .metadata_mut()
        .insert("x-service-caller", "gateway".parse().unwrap());
    request
        .metadata_mut()
        .insert("x-service-token", "rpc-gateway-fixture".parse().unwrap());
    if let Some(proof) = proof {
        request
            .metadata_mut()
            .insert(HEADER, proof.parse().unwrap());
    }
    if let Some(remaining) = remaining {
        request
            .metadata_mut()
            .insert(REMAINING_HEADER, remaining.parse().unwrap());
    }
    request
}
async fn call(
    server: &mut Running,
    message: proto::ReadConcertAdmissionRequest,
) -> Result<Response<proto::ReadConcertAdmissionResponse>, Status> {
    let actor = Uuid::parse_str(&message.user_id).unwrap();
    let proof = token(&message, actor);
    server
        .client
        .read_concert_admission(request(message, Some(&proof), Some("2S")))
        .await
}
#[derive(Clone, Copy)]
enum Mode {
    Success,
    Error(ReadError),
    Delay(Duration),
    Block(Duration),
    Hold,
}
struct ControlledOwner {
    mode: Mutex<Mode>,
    calls: AtomicUsize,
    budgets: Mutex<Vec<ReadBudget>>,
    entered: Notify,
}
impl ControlledOwner {
    fn new(mode: Mode) -> Arc<Self> {
        Arc::new(Self {
            mode: Mutex::new(mode),
            calls: AtomicUsize::new(0),
            budgets: Mutex::new(Vec::new()),
            entered: Notify::new(),
        })
    }
}
#[async_trait::async_trait]
impl CurrentAdmissionPort for ControlledOwner {
    async fn read_current(
        &self,
        _: AdmissionRead,
        budget: &ReadBudget,
    ) -> Result<CurrentAdmission, ReadError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.budgets.lock().unwrap().push(budget.clone());
        self.entered.notify_one();
        let mode = *self.mode.lock().unwrap();
        match mode {
            Mode::Success => {}
            Mode::Error(error) => return Err(error),
            Mode::Delay(delay) => tokio::time::sleep(delay).await,
            Mode::Block(delay) => std::thread::sleep(delay),
            Mode::Hold => std::future::pending::<()>().await,
        }
        Ok(CurrentAdmission {
            observed_at: chrono::Utc::now(),
            account: AccountEligibility::Active,
            session: SessionEligibility::Live,
            binding: BindingEligibility::NotRequested,
        })
    }
}
async fn controlled(mode: Mode) -> anyhow::Result<(Running, Arc<ControlledOwner>)> {
    let owner = ControlledOwner::new(mode);
    let state = state(lazy_pool(), Some(owner.clone())).await?;
    Ok((Running::start(state).await?, owner))
}

#[tokio::test]
async fn deterministic_registered_gateway_reaches_actual_generated_handler() -> anyhow::Result<()> {
    let (mut server, owner) = controlled(Mode::Success).await?;
    let response = call(&mut server, message(Uuid::new_v4())).await?;
    assert_eq!(
        response.metadata().get("cache-control").unwrap(),
        "no-store"
    );
    assert!(response.get_ref().account_active && response.get_ref().session_live);
    assert_eq!(response.get_ref().binding_state, 1);
    assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
    server.stop().await
}
#[tokio::test]
async fn deterministic_requested_binding_cannot_be_replaced_by_not_requested_facts(
) -> anyhow::Result<()> {
    let (mut server, owner) = controlled(Mode::Success).await?;
    let mut msg = message(Uuid::new_v4());
    msg.binding = Some(proto::ConcertBindingReference {
        identity_id: Uuid::new_v4().to_string(),
        provider: "X".into(),
        binding_epoch: 1,
    });
    let status = call(&mut server, msg).await.unwrap_err();
    assert_eq!(status.code(), Code::Unavailable);
    assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
    server.stop().await
}
#[tokio::test]
async fn deterministic_missing_wrong_and_sibling_callers_never_dispatch() -> anyhow::Result<()> {
    let (mut server, owner) = controlled(Mode::Success).await?;
    for (caller, credential, expected) in [
        (None, None, Code::Unauthenticated),
        (
            Some("gateway"),
            Some("wrong-private-token"),
            Code::Unauthenticated,
        ),
        (
            Some("post"),
            Some("rpc-post-fixture"),
            Code::PermissionDenied,
        ),
        (
            Some("gateway"),
            Some("rpc-post-fixture"),
            Code::Unauthenticated,
        ),
    ] {
        let msg = message(Uuid::new_v4());
        let proof = token(&msg, Uuid::parse_str(&msg.user_id)?);
        let mut req = request(msg, Some(&proof), Some("2S"));
        req.metadata_mut().remove("x-service-caller");
        req.metadata_mut().remove("x-service-token");
        if let Some(caller) = caller {
            req.metadata_mut()
                .insert("x-service-caller", caller.parse()?);
        }
        if let Some(credential) = credential {
            req.metadata_mut()
                .insert("x-service-token", credential.parse()?);
        }
        assert_eq!(
            server
                .client
                .read_concert_admission(req)
                .await
                .unwrap_err()
                .code(),
            expected
        );
    }
    assert_eq!(owner.calls.load(Ordering::SeqCst), 0);
    server.stop().await
}
#[tokio::test]
async fn deterministic_signature_audience_method_body_actor_and_bare_metadata_deny(
) -> anyhow::Result<()> {
    let (mut server, owner) = controlled(Mode::Success).await?;
    let actor = Uuid::new_v4();
    let msg = message(actor);
    let bytes = frame(&msg);
    let now = chrono::Utc::now().timestamp();
    let signer = ActorDelegationSigner::local_parity(Workload::Gateway);
    let forged = ActorDelegationSigner::from_seed(Workload::Gateway, &[42; 32])?;
    let proofs = [
        None,
        Some("malformed-private-proof".into()),
        Some(forged.issue(Workload::Auth, METHOD, actor, None, &bytes, now)),
        Some(signer.issue(Workload::Profile, METHOD, actor, None, &bytes, now)),
        Some(signer.issue(
            Workload::Auth,
            "/auth.v1.AuthService/CheckSession",
            actor,
            None,
            &bytes,
            now,
        )),
        Some(signer.issue(Workload::Auth, METHOD, Uuid::new_v4(), None, &bytes, now)),
        Some(signer.issue(
            Workload::Auth,
            METHOD,
            actor,
            None,
            &frame(&message(actor)),
            now,
        )),
    ];
    for proof in proofs {
        let mut req = request(msg.clone(), proof.as_deref(), Some("2S"));
        req.metadata_mut()
            .insert("x-verified-actor", actor.to_string().parse()?);
        let error = server.client.read_concert_admission(req).await.unwrap_err();
        assert!(matches!(
            error.code(),
            Code::Unauthenticated | Code::PermissionDenied
        ));
        assert!(!error.message().contains("private"));
    }
    assert_eq!(owner.calls.load(Ordering::SeqCst), 0);
    server.stop().await
}
#[tokio::test]
async fn deterministic_exact_proof_replay_does_not_redispatch() -> anyhow::Result<()> {
    let (mut server, owner) = controlled(Mode::Success).await?;
    let msg = message(Uuid::new_v4());
    let proof = token(&msg, Uuid::parse_str(&msg.user_id)?);
    server
        .client
        .read_concert_admission(request(msg.clone(), Some(&proof), Some("2S")))
        .await?;
    assert_eq!(
        server
            .client
            .read_concert_admission(request(msg, Some(&proof), Some("2S")))
            .await
            .unwrap_err()
            .code(),
        Code::Unauthenticated
    );
    assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
    server.stop().await
}
#[tokio::test]
async fn deterministic_malformed_canonical_ids_and_binding_never_dispatch() -> anyhow::Result<()> {
    let (mut server, owner) = controlled(Mode::Success).await?;
    let actor = Uuid::new_v4();
    let mut messages = Vec::new();
    for value in [
        "",
        "private-session",
        "00000000-0000-0000-0000-000000000000",
        "123456781234123412341234567890ab",
        "12345678-1234-1234-1234-1234567890AB",
    ] {
        let mut msg = message(actor);
        msg.session_family_id = value.into();
        messages.push(msg);
    }
    for value in [
        actor.simple().to_string(),
        actor.to_string().to_uppercase(),
        Uuid::nil().to_string(),
    ] {
        let mut msg = message(actor);
        msg.user_id = value;
        messages.push(msg);
    }
    for binding in [
        proto::ConcertBindingReference {
            identity_id: "private-reference".into(),
            provider: "X".into(),
            binding_epoch: 1,
        },
        proto::ConcertBindingReference {
            identity_id: Uuid::new_v4().to_string(),
            provider: "INSTAGRAM".into(),
            binding_epoch: 1,
        },
        proto::ConcertBindingReference {
            identity_id: Uuid::new_v4().to_string(),
            provider: "X".into(),
            binding_epoch: 0,
        },
        proto::ConcertBindingReference {
            identity_id: Uuid::new_v4().to_string(),
            provider: "X".into(),
            binding_epoch: ProviderBinding::MAX_EPOCH + 1,
        },
    ] {
        let mut msg = message(actor);
        msg.binding = Some(binding);
        messages.push(msg);
    }
    for msg in messages {
        let proof = token(&msg, actor);
        let error = server
            .client
            .read_concert_admission(request(msg, Some(&proof), Some("2S")))
            .await
            .unwrap_err();
        assert!(matches!(
            error.code(),
            Code::InvalidArgument | Code::PermissionDenied
        ));
        assert!(!error.message().contains("private"));
    }
    assert_eq!(owner.calls.load(Ordering::SeqCst), 0);
    server.stop().await
}
#[tokio::test]
async fn deterministic_missing_invalid_elapsed_original_deadline_never_dispatch(
) -> anyhow::Result<()> {
    let (mut server, owner) = controlled(Mode::Success).await?;
    for (remaining, expected) in [
        (None, Code::InvalidArgument),
        (Some("garbage"), Code::InvalidArgument),
        (Some("0n"), Code::DeadlineExceeded),
    ] {
        let msg = message(Uuid::new_v4());
        let proof = token(&msg, Uuid::parse_str(&msg.user_id)?);
        assert_eq!(
            server
                .client
                .read_concert_admission(request(msg, Some(&proof), remaining))
                .await
                .unwrap_err()
                .code(),
            expected
        );
    }
    assert_eq!(owner.calls.load(Ordering::SeqCst), 0);
    server.stop().await
}
#[tokio::test]
async fn deterministic_legacy_skew_valid_but_expired_proof_has_no_strict_authority(
) -> anyhow::Result<()> {
    let (mut server, owner) = controlled(Mode::Success).await?;
    let actor = Uuid::new_v4();
    let msg = message(actor);
    let proof = ActorDelegationSigner::local_parity(Workload::Gateway).issue(
        Workload::Auth,
        METHOD,
        actor,
        None,
        &frame(&msg),
        chrono::Utc::now().timestamp() - 31,
    );
    assert_eq!(
        server
            .client
            .read_concert_admission(request(msg, Some(&proof), Some("2S")))
            .await
            .unwrap_err()
            .code(),
        Code::Unauthenticated
    );
    assert_eq!(owner.calls.load(Ordering::SeqCst), 0);
    server.stop().await
}
#[tokio::test]
async fn deterministic_owner_errors_keep_fixed_safe_statuses() -> anyhow::Result<()> {
    let (mut server, owner) = controlled(Mode::Success).await?;
    for (error, code, expected) in [
        (
            ReadError::InvalidInput,
            Code::InvalidArgument,
            "Invalid admission input",
        ),
        (
            ReadError::Cancelled,
            Code::Cancelled,
            "Admission read cancelled",
        ),
        (
            ReadError::DeadlineExceeded,
            Code::DeadlineExceeded,
            "Admission read deadline exceeded",
        ),
        (
            ReadError::Unavailable,
            Code::Unavailable,
            "Admission owner unavailable",
        ),
    ] {
        *owner.mode.lock().unwrap() = Mode::Error(error);
        let status = call(&mut server, message(Uuid::new_v4()))
            .await
            .unwrap_err();
        assert_eq!(status.code(), code);
        assert_eq!(status.message(), expected);
    }
    assert_eq!(owner.calls.load(Ordering::SeqCst), 4);
    server.stop().await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deterministic_whole_owner_await_and_late_noncooperative_completion_are_fenced(
) -> anyhow::Result<()> {
    for mode in [
        Mode::Delay(Duration::from_millis(150)),
        Mode::Block(Duration::from_millis(150)),
    ] {
        let (mut server, owner) = controlled(mode).await?;
        let msg = message(Uuid::new_v4());
        let proof = token(&msg, Uuid::parse_str(&msg.user_id)?);
        let start = Instant::now();
        let error = server
            .client
            .read_concert_admission(request(msg, Some(&proof), Some("30m")))
            .await
            .unwrap_err();
        assert_eq!(error.code(), Code::DeadlineExceeded);
        assert!(start.elapsed() < Duration::from_secs(1));
        assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            owner.budgets.lock().unwrap()[0].check(),
            Err(ReadError::Cancelled)
        );
        server.stop().await?;
    }
    Ok(())
}
#[tokio::test]
async fn deterministic_proof_expiry_bounds_a_longer_original_transport_deadline(
) -> anyhow::Result<()> {
    let (mut server, owner) = controlled(Mode::Hold).await?;
    let actor = Uuid::new_v4();
    let msg = message(actor);
    let signer = ActorDelegationSigner::local_parity(Workload::Gateway);
    let body = frame(&msg);
    // The public signer fixes lifetime at 30s with integer-second claims. Align
    // before issuing the one request so it has 1.25..1.5s live authority: ample
    // dispatch margin, while still expiring before the 2s resource ceiling.
    let sampled = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let now = chrono::Utc::now();
            let micros = now.timestamp_subsec_micros();
            if (500_000..=750_000).contains(&micros) {
                break now;
            }
            let wait = if micros < 500_000 {
                500_000 - micros
            } else {
                1_500_000 - micros
            };
            tokio::time::sleep(Duration::from_micros(u64::from(wait) + 1_000)).await;
        }
    })
    .await?;
    let issued_at = sampled.timestamp() + 2 - LIFETIME_SECONDS;
    let expires_at = chrono::DateTime::from_timestamp(issued_at + LIFETIME_SECONDS, 0).unwrap();
    let proof = signer.issue(Workload::Auth, METHOD, actor, None, &body, issued_at);
    let sent = Instant::now();
    let error = {
        let rpc = server
            .client
            .read_concert_admission(request(msg, Some(&proof), Some("5S")));
        tokio::pin!(rpc);
        // Hold the owner pending until the handler cancels it. An early denial
        // fails directly; the signed RPC is never retried to obtain dispatch.
        tokio::time::timeout(Duration::from_millis(750), async {
            tokio::select! {
                biased;
                () = owner.entered.notified() => {},
                result = &mut rpc => anyhow::bail!("RPC returned before owner dispatch: {result:?}"),
            }
            anyhow::Ok(())
        })
        .await??;
        assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
        assert!(
            chrono::Utc::now() < expires_at,
            "proof expired before dispatch"
        );
        rpc.await.unwrap_err()
    };
    assert_eq!(error.code(), Code::DeadlineExceeded);
    assert!(
        chrono::Utc::now() >= expires_at,
        "response preceded signed expiry"
    );
    assert!(sent.elapsed() < ReadBudget::MAX_DURATION);
    assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        owner.budgets.lock().unwrap()[0].check(),
        Err(ReadError::Cancelled)
    );
    server.stop().await
}
