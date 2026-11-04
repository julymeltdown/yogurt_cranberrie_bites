//! BE42-AUTH-CONCERT-APPLY-01: real Ticket via paired Auth-source/Concert RPC.
//! Identity helper logic copied from deletion_source_rpc.rs at fa6f3f72.
#[path = "canonical_owner_composition_support/environment.rs"]
mod environment;
use auth_service::adapters::out::grpc::concert_deletion_client::{
    proto as auth_proto, ConcertDeletionClient,
};
use auth_service::application::concert_deletion::*;
use auth_service::{
    adapters::out::{
        cache::session_store::PgRefreshTokenStore,
        persistence::{
            user_deletion_saga_repository::PgUserDeletionSagaRepository,
            user_repository::{PgAuthAuditRepository, PgLoginAuditRepository, PgUserRepository},
        },
        runtime::SystemAuthRuntime,
        security::{
            admitted_argon2::AdmittedArgon2PasswordWork,
            email_verification::SecureEmailVerification,
        },
    },
    application::{
        dto::auth_dto::{DeleteUserDto, DeleteUserResponseDto},
        password_work::{PasswordWork, PasswordWorkLimits},
        use_cases::{
            delete_user::DeleteUserUseCase,
            login::{LoginUseCase, LoginUseCaseDeps},
            signup::SignupUseCase,
        },
    },
    config::JwtTokenService,
};
use concert_service::{
    adapters::{
        out::relationship_authority::deletion_source::AuthDeletionSource,
        r#in::deletion_rpc::{self, proto, DeletionOwner, DeletionOwners},
    },
    world::account_deletion::{
        DeletionAcknowledgement, DeletionAuthority, DeletionFailure, TicketDeletion,
        VerifiedDeletionSource, WorldDeletionCommand, WorldDeletionTarget,
    },
};
use platform::{
    grpc_auth::{CallerRegistry, ServiceIdentityClientInterceptor, Workload},
    grpc_tls::TlsMaterial,
};
use prost::Message;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::{
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{sync::Notify, time::Instant};
use uuid::Uuid;

type Login = LoginUseCase<
    Arc<PgUserRepository>,
    Arc<PgLoginAuditRepository>,
    Arc<PgRefreshTokenStore>,
    Arc<JwtTokenService>,
>;
fn login(p: &PgPool) -> anyhow::Result<Login> {
    let runtime = Arc::new(SystemAuthRuntime);
    let users = Arc::new(PgUserRepository::new(p.clone(), runtime.clone()));
    Ok(LoginUseCase::new(LoginUseCaseDeps {
        user_repository: users.clone(),
        login_audit_repository: Arc::new(PgLoginAuditRepository::new(p.clone())),
        refresh_token_store: Arc::new(PgRefreshTokenStore::new(p.clone(), runtime.clone())),
        token_service: Arc::new(JwtTokenService::new(
            "be42-owned-test",
            "mcr-browser",
            900,
            7200,
        )?),
        transactions: users,
        password_work: Arc::new(AdmittedArgon2PasswordWork::new(
            PasswordWorkLimits::default(),
        )?),
        runtime,
    }))
}
async fn identity(p: &PgPool) -> anyhow::Result<(Uuid, Uuid)> {
    let runtime = Arc::new(SystemAuthRuntime);
    let users = Arc::new(PgUserRepository::new(p.clone(), runtime.clone()));
    let password: PasswordWork = Arc::new(AdmittedArgon2PasswordWork::new(
        PasswordWorkLimits::default(),
    )?);
    let signup = SignupUseCase::new(
        users.clone(),
        users,
        password,
        runtime,
        Arc::new(SecureEmailVerification),
    );
    let email = format!("ordering-{}@example.invalid", Uuid::new_v4().simple());
    let created = signup.signup_email(&email, "Owned-ordering-42!").await?;
    assert_eq!(created.status, "PENDING_VERIFICATION");
    // Observe the challenge produced by the actual transactional outbox; no
    // identity, verification state, password or session is inserted by the test.
    let code: String = sqlx::query_scalar("select code from email_verifications where user_id=$1")
        .bind(created.user_id)
        .fetch_one(p)
        .await?;
    let verified = signup.verify_email(&email, &code).await?;
    assert_eq!(verified.user_id, created.user_id);
    assert_eq!(verified.status, "VERIFIED");
    let session = login(p)?
        .login(&email, "Owned-ordering-42!", None, None)
        .await?;
    assert!(!session.access_token.is_empty() && !session.refresh_token.is_empty());
    let family = sqlx::query_scalar(
        "select family_id from refresh_tokens where user_id=$1 and revoked_at is null",
    )
    .bind(created.user_id)
    .fetch_one(p)
    .await?;
    Ok((created.user_id, family))
}
async fn delete(p: PgPool, actor: Uuid) -> anyhow::Result<DeleteUserResponseDto> {
    let runtime = Arc::new(SystemAuthRuntime);
    Ok(DeleteUserUseCase::new(
        Arc::new(PgUserRepository::new(p.clone(), runtime.clone())),
        Arc::new(PgAuthAuditRepository::new(p.clone())),
        PgUserDeletionSagaRepository::new(p),
        runtime,
    )
    .execute(DeleteUserDto {
        user_id: actor.to_string(),
        actor_id: actor.to_string(),
        request_id: None,
        ip_address: None,
        user_agent: None,
    })
    .await?)
}

struct Running {
    url: String,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn listener() -> anyhow::Result<(String, tokio::net::TcpListener)> {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    Ok((format!("https://{}", l.local_addr()?), l))
}
async fn auth_server(pool: PgPool) -> anyhow::Result<Running> {
    let (url, l) = listener().await?;
    let s = auth_service::adapters::r#in::grpc::concert_deletion_source::guarded_server(
        pool,
        CallerRegistry::new().register(Workload::Concert, "concert-fixture")?,
    )?;
    let tls = environment::tls(Workload::Auth)?.server_config()?;
    let incoming =
        futures::stream::unfold(l, |l| async { Some((l.accept().await.map(|(s, _)| s), l)) });
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .tls_config(tls)
            .unwrap()
            .layer(platform::call_deadline::CallDeadlineLayer)
            .add_service(s)
            .serve_with_incoming(incoming)
            .await
            .unwrap();
    });
    Ok(Running { url, task })
}
async fn concert_server(
    owners: DeletionOwners,
    authority: Arc<dyn DeletionAuthority>,
    role: Workload,
) -> anyhow::Result<Running> {
    let (url, l) = listener().await?;
    let s = deletion_rpc::guarded_server(
        owners,
        authority,
        CallerRegistry::new()
            .register(Workload::Auth, "auth-fixture")?
            .register(Workload::Gateway, "gateway-fixture")?,
    )?;
    let tls = environment::tls(role)?.server_config()?;
    let incoming =
        futures::stream::unfold(l, |l| async { Some((l.accept().await.map(|(s, _)| s), l)) });
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .tls_config(tls)
            .unwrap()
            .layer(platform::call_deadline::CallDeadlineLayer)
            .add_service(s)
            .serve_with_incoming(incoming)
            .await
            .unwrap();
    });
    Ok(Running { url, task })
}
fn client(s: &Running) -> anyhow::Result<ConcertDeletionClient> {
    ConcertDeletionClient::connect(&s.url, &environment::tls(Workload::Auth)?, "auth-fixture")
}
fn stop() -> Instant {
    Instant::now() + Duration::from_secs(5)
}
fn command(r: &DeleteUserResponseDto) -> anyhow::Result<ConcertDeletionCommand> {
    Ok(ConcertDeletionCommand {
        key: ConcertDeletionKey {
            saga_id: r.saga_id.parse()?,
            user_id: r.user_id.parse()?,
            target: ConcertDeletionTarget::Tickets,
        },
        actor_id: r.user_id.parse()?,
        retention_until: r.saga.retention_until,
    })
}
fn world(c: &ConcertDeletionCommand) -> WorldDeletionCommand {
    WorldDeletionCommand {
        saga_id: c.key.saga_id,
        user_id: c.key.user_id,
        actor_id: c.actor_id,
        retention_until: c.retention_until,
        target: match c.key.target {
            ConcertDeletionTarget::Tickets => WorldDeletionTarget::Tickets,
            ConcertDeletionTarget::Sessions => WorldDeletionTarget::Sessions,
            ConcertDeletionTarget::Sharing => WorldDeletionTarget::Sharing,
            ConcertDeletionTarget::PrivateData => WorldDeletionTarget::PrivateData,
        },
    }
}
fn wire(c: &ConcertDeletionCommand) -> proto::Command {
    proto::Command {
        saga_id: c.key.saga_id.to_string(),
        user_id: c.key.user_id.to_string(),
        actor_id: c.actor_id.to_string(),
        target: match c.key.target {
            ConcertDeletionTarget::Tickets => 1,
            ConcertDeletionTarget::Sessions => 2,
            ConcertDeletionTarget::Sharing => 3,
            ConcertDeletionTarget::PrivateData => 4,
        },
        retention_seconds: c.retention_until.timestamp(),
        retention_nanos: c.retention_until.timestamp_subsec_nanos() as i32,
    }
}
async fn raw(
    s: &Running,
    certificate: Workload,
    caller: Workload,
    credential: &str,
    c: Option<proto::Command>,
) -> anyhow::Result<tonic::Code> {
    let ch = environment::tls(certificate)?
        .endpoint(&s.url, Workload::Concert)?
        .connect()
        .await?;
    let mut client =
        proto::account_deletion_service_client::AccountDeletionServiceClient::with_interceptor(
            ch,
            ServiceIdentityClientInterceptor::new(caller, credential)?,
        );
    let mut q = tonic::Request::new(proto::ApplyRequest {
        command: c,
        request_nonce: Uuid::new_v4().as_bytes().to_vec(),
        expires_unix_micros: chrono::Utc::now().timestamp_micros() + 1_000_000,
    });
    q.set_timeout(Duration::from_secs(1));
    Ok(client
        .apply(q)
        .await
        .expect_err("negative only raw request")
        .code())
}
struct LegacyUnavailable;
#[async_trait::async_trait]
impl auth_service::application::ports::UserDeletionDispatcherPort for LegacyUnavailable {
    async fn apply(
        &self,
        _: auth_service::application::dto::deletion_dto::AuthUserDeletionTargetDto,
        _: Uuid,
        _: Uuid,
        _: Uuid,
        _: chrono::DateTime<chrono::Utc>,
    ) -> auth_service::application::errors::AuthResult<()> {
        Err(auth_service::application::errors::AuthError::DependencyUnavailable)
    }
}
struct IncompleteOnly;
#[async_trait::async_trait]
impl DeletionOwner for IncompleteOnly {
    async fn apply(
        &self,
        _: WorldDeletionCommand,
        _: Instant,
    ) -> Result<DeletionAcknowledgement, DeletionFailure> {
        Err(DeletionFailure::Incomplete)
    }
}
struct CommitThenLose {
    real: Arc<TicketDeletion>,
    committed: Arc<Notify>,
}
#[async_trait::async_trait]
impl DeletionOwner for CommitThenLose {
    async fn apply(
        &self,
        c: WorldDeletionCommand,
        d: Instant,
    ) -> Result<DeletionAcknowledgement, DeletionFailure> {
        self.real.apply(c, d).await?;
        self.committed.notify_one();
        std::future::pending().await
    }
}
struct SourceGate {
    real: AuthDeletionSource,
    calls: AtomicUsize,
    entered: Arc<Notify>,
}
#[async_trait::async_trait]
impl DeletionAuthority for SourceGate {
    async fn verify(
        &self,
        c: &WorldDeletionCommand,
        d: Instant,
    ) -> Result<VerifiedDeletionSource, DeletionFailure> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        let actual = self.real.verify(c, d).await?;
        if n == 3 {
            self.entered.notify_one();
            std::future::pending::<()>().await;
        }
        Ok(actual)
    }
}
#[derive(Serialize, Deserialize)]
struct Saved {
    command: Vec<u8>,
    source: Vec<u8>,
    revision: u64,
    pid: u32,
    pg_started: String,
}
fn saved_path() -> anyhow::Result<std::path::PathBuf> {
    Ok(environment::root()?.join("be42-apply-private.json"))
}
fn save(s: &Saved) -> anyhow::Result<()> {
    let mut f = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(saved_path()?)?;
    f.write_all(&serde_json::to_vec(s)?)?;
    f.sync_all()?;
    Ok(())
}
fn load() -> anyhow::Result<Saved> {
    let p = saved_path()?;
    let m = std::fs::symlink_metadata(&p)?;
    anyhow::ensure!(
        m.is_file()
            && !m.file_type().is_symlink()
            && m.permissions().mode() & 0o777 == 0o600
            && m.len() < 16384,
        "private bounded handoff"
    );
    Ok(serde_json::from_slice(&std::fs::read(p)?)?)
}
#[test]
fn method_and_exact_wire_shape_deny_unknown_authority() {
    let path = "/concert.account_deletion.v1.AccountDeletionService/Apply";
    assert!(deletion_rpc::POLICY.actor_fields.is_empty());
    assert!(deletion_rpc::POLICY.allows(path, Workload::Auth));
    for caller in [
        Workload::Gateway,
        Workload::Concert,
        Workload::Friend,
        Workload::Post,
        Workload::Profile,
        Workload::Notification,
    ] {
        assert!(!deletion_rpc::POLICY.allows(path, caller));
    }
    assert!(!deletion_rpc::POLICY.allows(
        "/concert.account_deletion.v1.AccountDeletionService/Ack",
        Workload::Auth
    ));
    let c = proto::Command {
        saga_id: Uuid::new_v4().to_string(),
        user_id: Uuid::new_v4().to_string(),
        actor_id: Uuid::new_v4().to_string(),
        target: 1,
        retention_seconds: 1_800_000_000,
        retention_nanos: 123_456_789,
    };
    let auth = auth_proto::Command::decode(c.encode_to_vec().as_slice()).unwrap();
    assert_eq!(auth.retention_nanos, 123_456_789);
    assert_eq!(
        deletion_rpc::decode_command(&c)
            .unwrap()
            .retention_until
            .timestamp_subsec_nanos(),
        123_456_789
    );
    for target in [0, 99, -1] {
        let mut v = c.clone();
        v.target = target;
        assert!(deletion_rpc::decode_command(&v).is_err());
    }
    for nanos in [-1, 1_000_000_000] {
        let mut v = c.clone();
        v.retention_nanos = nanos;
        assert!(deletion_rpc::decode_command(&v).is_err());
    }
    let mut v = c;
    v.user_id = Uuid::nil().to_string();
    assert!(deletion_rpc::decode_command(&v).is_err());
}

#[tokio::test]
#[ignore = "requires owned PG16, actual Auth source and Apply mTLS RPC, SOURCE then physical restart/fresh PID"]
async fn actual_auth_to_ticket_apply_restart_and_cancellation() -> anyhow::Result<()> {
    let auth = environment::pool("auth").await?;
    let concert = environment::pool("concert").await?;
    let auth_server = auth_server(auth.clone()).await?;
    let source = AuthDeletionSource::connect(
        &auth_server.url,
        &environment::tls(Workload::Concert)?,
        "concert-fixture",
    )?;
    let authority: Arc<dyn DeletionAuthority> = Arc::new(source.clone());
    let ticket = Arc::new(TicketDeletion::new(concert.clone(), authority.clone()));
    let server = concert_server(
        DeletionOwners {
            tickets: Some(ticket.clone()),
            ..Default::default()
        },
        authority.clone(),
        Workload::Concert,
    )
    .await?;
    let client = client(&server)?;
    match std::env::var("MCR_BE42_APPLY_PHASE")?.as_str() {
        "SOURCE" => {
            let (user, _) = identity(&auth).await?;
            let deleted = delete(auth.clone(), user).await?;
            let c = command(&deleted)?;
            let dispatcher = auth_service::concert_deletion::ConcertDeletionDispatcher::new(
                LegacyUnavailable,
                client.owners(),
                Duration::from_secs(5),
            )?;
            dispatcher.dispatch(c.clone()).await?;
            let ack = client.apply_with_deadline(c.clone(), stop()).await?;
            assert!(ack.verifies(&c));
            let proof = source
                .verify(&world(&c), stop())
                .await
                .map_err(|_| anyhow::anyhow!("actual canonical source"))?;
            // Correct routing to unavailable owning adapters remains a failure;
            // this does not qualify Sessions, Sharing or PrivateData business work.
            for target in [
                ConcertDeletionTarget::Sessions,
                ConcertDeletionTarget::Sharing,
                ConcertDeletionTarget::PrivateData,
            ] {
                let mut missing = c.clone();
                missing.key.target = target;
                assert_eq!(
                    client.apply_with_deadline(missing.clone(), stop()).await,
                    Err(ConcertDeletionFailure::Unavailable)
                );
                assert!(dispatcher.dispatch(missing).await.is_err());
            }
            assert_eq!(
                raw(
                    &server,
                    Workload::Gateway,
                    Workload::Gateway,
                    "gateway-fixture",
                    Some(wire(&c))
                )
                .await?,
                tonic::Code::PermissionDenied
            );
            assert_eq!(
                raw(
                    &server,
                    Workload::Auth,
                    Workload::Auth,
                    "bad-credential",
                    Some(wire(&c))
                )
                .await?,
                tonic::Code::Unauthenticated
            );
            assert_eq!(
                raw(
                    &server,
                    Workload::Gateway,
                    Workload::Auth,
                    "auth-fixture",
                    Some(wire(&c))
                )
                .await?,
                tonic::Code::Unauthenticated
            );
            let mut unknown = wire(&c);
            unknown.target = 99;
            assert_eq!(
                raw(
                    &server,
                    Workload::Auth,
                    Workload::Auth,
                    "auth-fixture",
                    Some(unknown)
                )
                .await?,
                tonic::Code::InvalidArgument
            );
            assert_eq!(
                raw(
                    &server,
                    Workload::Auth,
                    Workload::Auth,
                    "auth-fixture",
                    None
                )
                .await?,
                tonic::Code::InvalidArgument
            );
            for field in 0..5 {
                let mut bad = c.clone();
                match field {
                    0 => bad.key.saga_id = Uuid::new_v4(),
                    1 => bad.key.user_id = Uuid::new_v4(),
                    2 => bad.actor_id = Uuid::new_v4(),
                    3 => bad.retention_until += chrono::Duration::seconds(1),
                    _ => bad.retention_until += chrono::Duration::nanoseconds(1),
                };
                assert!(client.apply_with_deadline(bad, stop()).await.is_err());
            }
            assert!(client
                .apply_with_deadline(c.clone(), Instant::now())
                .await
                .is_err());
            assert!(client
                .apply_with_deadline(c.clone(), Instant::now() + Duration::from_secs(6))
                .await
                .is_err());
            let foreign = concert_server(
                DeletionOwners {
                    tickets: Some(ticket.clone()),
                    ..Default::default()
                },
                authority.clone(),
                Workload::Friend,
            )
            .await?;
            let wrong = ConcertDeletionClient::connect(
                &foreign.url,
                &environment::tls(Workload::Auth)?,
                "auth-fixture",
            )?;
            assert!(wrong.apply_with_deadline(c.clone(), stop()).await.is_err());
            let absent = concert_server(
                DeletionOwners::default(),
                authority.clone(),
                Workload::Concert,
            )
            .await?;
            assert!(super_client(&absent)?
                .apply_with_deadline(c.clone(), stop())
                .await
                .is_err());
            let incomplete = concert_server(
                DeletionOwners {
                    sharing: Some(Arc::new(IncompleteOnly)),
                    ..Default::default()
                },
                authority.clone(),
                Workload::Concert,
            )
            .await?;
            let mut sharing = c.clone();
            sharing.key.target = ConcertDeletionTarget::Sharing;
            assert_eq!(
                super_client(&incomplete)?
                    .apply_with_deadline(sharing, stop())
                    .await,
                Err(ConcertDeletionFailure::Incomplete)
            );

            // Actual owner commit followed by lost transport reply is uncertain,
            // never ACK. Replay uses the same durable owner identity and fence.
            let (lost_user, _) = identity(&auth).await?;
            let lost = command(&delete(auth.clone(), lost_user).await?)?;
            let committed = Arc::new(Notify::new());
            let lost_server = concert_server(
                DeletionOwners {
                    tickets: Some(Arc::new(CommitThenLose {
                        real: ticket.clone(),
                        committed: committed.clone(),
                    })),
                    ..Default::default()
                },
                authority.clone(),
                Workload::Concert,
            )
            .await?;
            let lost_client = super_client(&lost_server)?;
            let exact = lost.clone();
            let pending = tokio::spawn(async move {
                lost_client
                    .apply_with_deadline(exact, Instant::now() + Duration::from_millis(300))
                    .await
            });
            tokio::time::timeout(Duration::from_secs(1), committed.notified()).await?;
            assert!(pending.await?.is_err());
            let durable:i64=sqlx::query_scalar("select count(*) from concert_world_ticket_deletion_progress where user_id=$1 and completed_at is not null")
                .bind(lost_user).fetch_one(&concert).await?;
            assert_eq!(durable, 1);
            let recovered = client.apply_with_deadline(lost.clone(), stop()).await?;
            assert!(recovered.verifies(&lost));

            // The gate waits only AFTER actual source verification on the real
            // Ticket precommit check. It never fabricates positive authority.
            let (cancel_user, _) = identity(&auth).await?;
            let cancelled = command(&delete(auth.clone(), cancel_user).await?)?;
            let entered = Arc::new(Notify::new());
            let gate = Arc::new(SourceGate {
                real: source.clone(),
                calls: AtomicUsize::new(0),
                entered: entered.clone(),
            });
            let gated_ticket = Arc::new(TicketDeletion::new(concert.clone(), gate.clone()));
            let gated = concert_server(
                DeletionOwners {
                    tickets: Some(gated_ticket),
                    ..Default::default()
                },
                gate.clone(),
                Workload::Concert,
            )
            .await?;
            let cancel_client = super_client(&gated)?;
            let retry_client = cancel_client.clone();
            let exact = cancelled.clone();
            let original = Instant::now() + Duration::from_millis(300);
            let pending =
                tokio::spawn(
                    async move { cancel_client.apply_with_deadline(exact, original).await },
                );
            tokio::time::timeout(Duration::from_secs(1), entered.notified()).await?;
            pending.abort();
            assert!(pending.await.unwrap_err().is_cancelled());
            let limit = Instant::now() + Duration::from_secs(1);
            loop {
                let idle:i64=sqlx::query_scalar("select count(*) from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and xact_start is not null")
                    .fetch_one(&concert).await?;
                if idle == 0 {
                    break;
                }
                anyhow::ensure!(
                    Instant::now() < limit,
                    "original deadline did not release owner transaction"
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            let fences: i64 = sqlx::query_scalar(
                "select count(*) from concert_world_account_deletion_fences where user_id=$1",
            )
            .bind(cancel_user)
            .fetch_one(&concert)
            .await?;
            let progress: i64 = sqlx::query_scalar(
                "select count(*) from concert_world_ticket_deletion_progress where user_id=$1",
            )
            .bind(cancel_user)
            .fetch_one(&concert)
            .await?;
            assert_eq!(
                (fences, progress),
                (0, 0),
                "cancelled real precommit writes must roll back"
            );
            assert_eq!(gate.calls.load(Ordering::SeqCst), 3);
            // Wait to the original instant if HTTP cancellation released earlier.
            tokio::time::sleep_until(original).await;
            assert!(retry_client
                .apply_with_deadline(cancelled.clone(), original)
                .await
                .is_err());
            assert!(client.apply_with_deadline(cancelled, stop()).await.is_ok());
            let pending: i64 = sqlx::query_scalar(
                "select count(*) from user_deletion_steps where saga_id=$1 and status='PENDING'",
            )
            .bind(c.key.saga_id)
            .fetch_one(&auth)
            .await?;
            assert_eq!(pending, 8);
            let pg_started: String = sqlx::query_scalar("select pg_postmaster_start_time()::text")
                .fetch_one(&auth)
                .await?;
            save(&Saved {
                command: wire(&c).encode_to_vec(),
                source: proof.source_binding().to_vec(),
                revision: ack.fence_revision,
                pid: std::process::id(),
                pg_started,
            })?;
            println!("BE42 APPLY SOURCE PASS: actual Auth dispatch clock and paired source/Apply mTLS, actual Ticket commit/replay, caller/tuple/peer/deadline refusals, missing owners and negative-only incomplete fixture, lost reply after actual commit, cancelled precommit rollback; eight Auth steps pending");
        }
        "RESTARTED" => {
            let saved = load()?;
            assert_ne!(saved.pid, std::process::id());
            let pg_started: String = sqlx::query_scalar("select pg_postmaster_start_time()::text")
                .fetch_one(&auth)
                .await?;
            assert_ne!(pg_started, saved.pg_started);
            let stored = proto::Command::decode(saved.command.as_slice())?;
            let w = deletion_rpc::decode_command(&stored)?;
            let c = ConcertDeletionCommand {
                key: ConcertDeletionKey {
                    saga_id: w.saga_id,
                    user_id: w.user_id,
                    target: ConcertDeletionTarget::Tickets,
                },
                actor_id: w.actor_id,
                retention_until: w.retention_until,
            };
            let ack = client.apply_with_deadline(c.clone(), stop()).await?;
            assert!(ack.verifies(&c));
            assert_eq!(ack.fence_revision, saved.revision);
            let source = source
                .verify(&w, stop())
                .await
                .map_err(|_| anyhow::anyhow!("restarted actual source"))?;
            assert_eq!(source.source_binding().as_slice(), saved.source);
            let pending: i64 = sqlx::query_scalar(
                "select count(*) from user_deletion_steps where saga_id=$1 and status='PENDING'",
            )
            .bind(c.key.saga_id)
            .fetch_one(&auth)
            .await?;
            assert_eq!(pending, 8);
            println!("BE42 APPLY RESTARTED PASS: changed PG and test PID, actual Auth-source/Concert-Apply mTLS, identical durable Ticket receipt/source binding and all Auth steps pending");
        }
        _ => anyhow::bail!("explicit SOURCE/RESTARTED phase required"),
    }
    for pool in [&auth, &concert] {
        let prepared: i64 = sqlx::query_scalar(
            "select count(*) from pg_prepared_xacts where database=current_database()",
        )
        .fetch_one(pool)
        .await?;
        assert_eq!(prepared, 0);
    }
    Ok(())
}
// Keep the local `client` binding in the native case distinct from construction.
fn super_client(s: &Running) -> anyhow::Result<ConcertDeletionClient> {
    client(s)
}
