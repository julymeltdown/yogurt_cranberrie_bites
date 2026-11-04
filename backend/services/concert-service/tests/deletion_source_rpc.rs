//! BE42-AUTH-SOURCE-BRIDGE-01. Real Auth stores and real mTLS consumers.
//! Auth identity helpers reused from auth_deletion_owner_ordering.rs at 7490f15e.
#[path = "canonical_owner_composition_support/environment.rs"]
mod environment;
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
    adapters::out::relationship_authority::deletion_source::{proto, AuthDeletionSource},
    world::account_deletion::{
        DeletionAuthority, TicketDeletion, WorldDeletionCommand, WorldDeletionTarget,
    },
};
use platform::{
    grpc_auth::{CallerRegistry, ServiceIdentityClientInterceptor, Workload},
    grpc_tls::TlsMaterial,
};
use prost::Message;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::{
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    sync::Arc,
    time::Duration,
};
use tokio::time::Instant;
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
async fn serve(pool: PgPool, role: Workload) -> anyhow::Result<Running> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("https://{}", listener.local_addr()?);
    let server = auth_service::adapters::r#in::grpc::concert_deletion_source::guarded_server(
        pool,
        CallerRegistry::new()
            .register(Workload::Concert, "concert-fixture")?
            .register(Workload::Profile, "profile-fixture")?,
    )?;
    let tls = environment::tls(role)?.server_config()?;
    let incoming = futures::stream::unfold(listener, |l| async {
        Some((l.accept().await.map(|(s, _)| s), l))
    });
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .tls_config(tls)
            .expect("fixture TLS")
            .layer(platform::call_deadline::CallDeadlineLayer)
            .add_service(server)
            .serve_with_incoming(incoming)
            .await
            .expect("actual Auth source service");
    });
    Ok(Running { url, task })
}
fn stop() -> Instant {
    Instant::now() + Duration::from_secs(5)
}
fn from_response(r: &DeleteUserResponseDto) -> anyhow::Result<WorldDeletionCommand> {
    Ok(WorldDeletionCommand {
        saga_id: r.saga_id.parse()?,
        user_id: r.user_id.parse()?,
        actor_id: r.user_id.parse()?,
        target: WorldDeletionTarget::Tickets,
        retention_until: r.saga.retention_until,
    })
}
fn wire(c: &WorldDeletionCommand) -> proto::DeletionCommand {
    proto::DeletionCommand {
        saga_id: c.saga_id.to_string(),
        user_id: c.user_id.to_string(),
        actor_id: c.actor_id.to_string(),
        target: match c.target {
            WorldDeletionTarget::Tickets => 1,
            WorldDeletionTarget::Sessions => 2,
            WorldDeletionTarget::Sharing => 3,
            WorldDeletionTarget::PrivateData => 4,
        },
        retention_seconds: c.retention_until.timestamp(),
        retention_nanos: c.retention_until.timestamp_subsec_nanos() as i32,
    }
}
async fn raw(
    url: &str,
    role: Workload,
    credential: &str,
    c: Option<proto::DeletionCommand>,
) -> anyhow::Result<tonic::Code> {
    let channel = environment::tls(role)?
        .endpoint(url, Workload::Auth)?
        .connect()
        .await?;
    let mut client=proto::auth_deletion_source_service_client::AuthDeletionSourceServiceClient::with_interceptor(
        channel,ServiceIdentityClientInterceptor::new(role,credential)?);
    let mut q = tonic::Request::new(proto::VerifyDeletionSourceRequest {
        command: c,
        request_nonce: Uuid::new_v4().as_bytes().to_vec(),
        expires_unix_micros: chrono::Utc::now().timestamp_micros() + 1_000_000,
    });
    q.set_timeout(Duration::from_secs(1));
    Ok(client
        .verify_source(q)
        .await
        .expect_err("negative request must not mint source")
        .code())
}
#[test]
fn plaintext_constructor_has_no_positive_fallback() {
    let tls = TlsMaterial::from_pem(Vec::new(), Vec::new(), Vec::new());
    assert!(AuthDeletionSource::connect("http://127.0.0.1:1", &tls, "fixture").is_err());
}
#[derive(Serialize, Deserialize)]
struct Saved {
    command: Vec<u8>,
    source: Vec<u8>,
    source_pid: u32,
    pg_started: String,
}
fn path() -> anyhow::Result<std::path::PathBuf> {
    Ok(environment::root()?.join("be42-source-private.json"))
}
fn save(s: &Saved) -> anyhow::Result<()> {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path()?)?;
    f.write_all(&serde_json::to_vec(s)?)?;
    f.sync_all()?;
    Ok(())
}
fn load() -> anyhow::Result<Saved> {
    let p = path()?;
    let m = std::fs::symlink_metadata(&p)?;
    anyhow::ensure!(
        m.is_file()
            && !m.file_type().is_symlink()
            && m.permissions().mode() & 0o777 == 0o600
            && m.len() < 16384,
        "private source handoff required"
    );
    Ok(serde_json::from_slice(&std::fs::read(p)?)?)
}
#[tokio::test]
#[ignore = "requires owned PG16/mTLS, actual SOURCE then physical restart and fresh test PID"]
async fn canonical_auth_deletion_source_rpc() -> anyhow::Result<()> {
    let auth = environment::pool("auth").await?;
    let concert = environment::pool("concert").await?;
    let server = serve(auth.clone(), Workload::Auth).await?;
    let client = AuthDeletionSource::connect(
        &server.url,
        &environment::tls(Workload::Concert)?,
        "concert-fixture",
    )?;
    match std::env::var("MCR_BE42_SOURCE_PHASE")?.as_str() {
        "SOURCE" => {
            let (actor, _family) = identity(&auth).await?;
            let deleted = delete(auth.clone(), actor).await?;
            let c = from_response(&deleted)?;
            // Source verification neither advances steps nor records owner ACKs.
            let before: Vec<(String, String)> = sqlx::query_as(
                "select target,status from user_deletion_steps where saga_id=$1 order by target",
            )
            .bind(c.saga_id)
            .fetch_all(&auth)
            .await?;
            assert_eq!(before.len(), 8);
            let original = stop();
            let verified = client
                .verify(&c, original)
                .await
                .map_err(|_| anyhow::anyhow!("real Auth source denied"))?;
            verified
                .check(&c, original)
                .map_err(|_| anyhow::anyhow!("verified source binding"))?;
            assert_eq!(
                verified,
                client
                    .verify(&c, original)
                    .await
                    .map_err(|_| anyhow::anyhow!("stable source retry"))?
            );
            assert_eq!(verified.original_deadline(), original);
            assert!(verified
                .check(&c, original + Duration::from_nanos(1))
                .is_err());
            for target in [
                WorldDeletionTarget::Sessions,
                WorldDeletionTarget::Sharing,
                WorldDeletionTarget::PrivateData,
            ] {
                let mut next = c.clone();
                next.target = target;
                assert!(client.verify(&next, stop()).await.is_ok());
                assert!(verified.check(&next, original).is_err());
            }
            for field in 0..5 {
                let mut bad = c.clone();
                match field {
                    0 => bad.saga_id = Uuid::new_v4(),
                    1 => bad.user_id = Uuid::new_v4(),
                    2 => bad.actor_id = Uuid::new_v4(),
                    3 => bad.retention_until += chrono::Duration::seconds(1),
                    _ => bad.retention_until += chrono::Duration::nanoseconds(1),
                };
                assert!(client.verify(&bad, stop()).await.is_err());
            }
            let (live, _) = identity(&auth).await?;
            let mut rebound = c.clone();
            rebound.user_id = live;
            rebound.actor_id = live;
            assert!(client.verify(&rebound, stop()).await.is_err());
            assert_eq!(
                raw(
                    &server.url,
                    Workload::Profile,
                    "profile-fixture",
                    Some(wire(&c))
                )
                .await?,
                tonic::Code::PermissionDenied
            );
            assert_eq!(
                raw(
                    &server.url,
                    Workload::Concert,
                    "bad-credential",
                    Some(wire(&c))
                )
                .await?,
                tonic::Code::Unauthenticated
            );
            assert_eq!(
                raw(
                    &server.url,
                    Workload::Profile,
                    "concert-fixture",
                    Some(wire(&c))
                )
                .await?,
                tonic::Code::Unauthenticated
            );
            let mut unknown = wire(&c);
            unknown.target = 99;
            assert_eq!(
                raw(
                    &server.url,
                    Workload::Concert,
                    "concert-fixture",
                    Some(unknown)
                )
                .await?,
                tonic::Code::InvalidArgument
            );
            assert_eq!(
                raw(&server.url, Workload::Concert, "concert-fixture", None).await?,
                tonic::Code::InvalidArgument
            );
            let foreign = serve(auth.clone(), Workload::Friend).await?;
            let wrong = AuthDeletionSource::connect(
                &foreign.url,
                &environment::tls(Workload::Concert)?,
                "concert-fixture",
            )?;
            assert!(
                wrong.verify(&c, stop()).await.is_err(),
                "Auth endpoint must verify Auth certificate role"
            );
            assert!(client.verify(&c, Instant::now()).await.is_err());
            assert!(client
                .verify(&c, Instant::now() + Duration::from_secs(6))
                .await
                .is_err());
            let mut lock = auth.begin().await?;
            sqlx::query("select id from user_deletion_sagas where id=$1 for update")
                .bind(c.saga_id)
                .fetch_one(&mut *lock)
                .await?;
            let expired = Instant::now() + Duration::from_millis(25);
            let started = Instant::now();
            assert!(client.verify(&c, expired).await.is_err());
            assert!(
                started.elapsed() < Duration::from_secs(1),
                "original call clock was extended"
            );
            lock.rollback().await?;
            assert!(
                client.verify(&c, expired).await.is_err(),
                "expired dispatch cannot be renewed"
            );
            // An altered stored source cannot rebind the immutable canonical
            // account/audit facts. These are negative-only owned corruption;
            // no user, session, claim or positive authority is SQL-seeded.
            sqlx::query("update user_deletion_sagas set retention_until=retention_until+interval '1 second' where id=$1")
                .bind(c.saga_id).execute(&auth).await?;
            let mut changed = c.clone();
            changed.retention_until += chrono::Duration::seconds(1);
            assert!(client.verify(&c, stop()).await.is_err());
            assert!(client.verify(&changed, stop()).await.is_err());
            sqlx::query("update user_deletion_sagas set retention_until=$2 where id=$1")
                .bind(c.saga_id)
                .bind(c.retention_until)
                .execute(&auth)
                .await?;
            let d = stop();
            let restored = client
                .verify(&c, d)
                .await
                .map_err(|_| anyhow::anyhow!("restored original source unavailable"))?;
            assert_eq!(restored.source_binding(), verified.source_binding());
            let tickets = TicketDeletion::new(concert.clone(), Arc::new(client.clone()));
            let ack = tickets
                .apply(c.clone(), stop())
                .await
                .map_err(|_| anyhow::anyhow!("actual Ticket source consumer unavailable"))?;
            assert!(ack.fence_revision > 0 && ack.command == c);
            let after: Vec<(String, String)> = sqlx::query_as(
                "select target,status from user_deletion_steps where saga_id=$1 order by target",
            )
            .bind(c.saga_id)
            .fetch_all(&auth)
            .await?;
            assert_eq!(
                before, after,
                "source/leaf verification cannot manufacture Auth job success"
            );
            let pg_started: String = sqlx::query_scalar("select pg_postmaster_start_time()::text")
                .fetch_one(&auth)
                .await?;
            save(&Saved {
                command: wire(&c).encode_to_vec(),
                source: verified.source_binding().to_vec(),
                source_pid: std::process::id(),
                pg_started,
            })?;
            println!("BE42 SOURCE BRIDGE SOURCE PASS: actual Auth signup/verify/login/deletion; four exact target source proofs via mTLS; canonical tuple/nanos/live/rebound/wrong caller/credential/certificate/deadline negatives; real common Ticket consumer; no Auth job ACK or portable hash authority");
        }
        "RESTARTED" => {
            let saved = load()?;
            assert_ne!(saved.source_pid, std::process::id());
            let now: String = sqlx::query_scalar("select pg_postmaster_start_time()::text")
                .fetch_one(&auth)
                .await?;
            assert_ne!(now, saved.pg_started);
            let wire = proto::DeletionCommand::decode(saved.command.as_slice())?;
            let c = WorldDeletionCommand {
                saga_id: wire.saga_id.parse()?,
                user_id: wire.user_id.parse()?,
                actor_id: wire.actor_id.parse()?,
                target: WorldDeletionTarget::Tickets,
                retention_until: chrono::DateTime::from_timestamp(
                    wire.retention_seconds,
                    wire.retention_nanos as u32,
                )
                .ok_or_else(|| anyhow::anyhow!("saved retention"))?,
            };
            let verified = client
                .verify(&c, stop())
                .await
                .map_err(|_| anyhow::anyhow!("restarted actual Auth source unavailable"))?;
            assert_eq!(verified.source_binding().as_slice(), saved.source);
            let tickets = TicketDeletion::new(concert.clone(), Arc::new(client));
            let replay = tickets
                .apply(c.clone(), stop())
                .await
                .map_err(|_| anyhow::anyhow!("actual Ticket replay unavailable"))?;
            assert!(replay.command == c && replay.fence_revision > 0);
            let pending: i64 = sqlx::query_scalar(
                "select count(*) from user_deletion_steps where saga_id=$1 and status='PENDING'",
            )
            .bind(c.saga_id)
            .fetch_one(&auth)
            .await?;
            assert_eq!(pending, 8);
            println!("BE42 SOURCE BRIDGE RESTARTED PASS: changed PostgreSQL and test PID; actual authenticated canonical source binding stable; real common Ticket consumer receipt replay; all Auth target steps remain pending");
        }
        _ => anyhow::bail!("explicit source/restarted phase required"),
    }
    for p in [&auth, &concert] {
        let prepared: i64 = sqlx::query_scalar(
            "select count(*) from pg_prepared_xacts where database=current_database()",
        )
        .fetch_one(p)
        .await?;
        assert_eq!(prepared, 0);
    }
    Ok(())
}
