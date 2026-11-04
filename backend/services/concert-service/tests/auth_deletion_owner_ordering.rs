//! BE42-AUTH-ORDERING-01: actual Auth deletion against canonical owner locks.
//! Reuses read-only CF01/CF02 support at 5f72fcf00fa7ba12599d6417d2f4744c5c49fbcb.
//! Delivery helpers below derive from canonical_owner_composition.rs at that base.
//! No production cleanup adapter, public Gateway journey or erasure acceptance.
#[path = "canonical_owner_restore_support/capture.rs"]
mod capture;
#[path = "canonical_owner_restore_support/environment.rs"]
mod environment;
#[path = "canonical_owner_restore_support/fault.rs"]
mod fault;
#[path = "canonical_owner_restore_support/fixture.rs"]
mod fixture;
#[path = "canonical_owner_restore_support/notification.rs"]
mod notification;
#[path = "canonical_owner_restore_support/owners.rs"]
mod owners;
#[path = "canonical_owner_restore_support/release.rs"]
mod release;
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
    adapters::out::relationship_authority::guarded::OwnerGuardScope,
    config::canonical_runtime::CanonicalRuntimeCore,
    product_rpc::{
        child_recovery::ChildRecoverySettings,
        delivery::{DeliveryService, POLICY},
        proto,
        scope_ledger::{ScopeKey, ScopePhase},
    },
};
use http_body_util::{BodyExt, Full};
use platform::{
    actor_delegation::response_scope::{ResponseScope, ResponseScopeToken, RetainedScopeLayer},
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::guard_without_delegation,
};
use prost::Message;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::{
    future::Future,
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tonic::{codegen::Bytes, Status};
use tower::{Layer, ServiceExt};
use uuid::Uuid;

async fn terminal(core: CanonicalRuntimeCore, cap: proto::ResponseDelivery) -> Result<(), Status> {
    let service = guard_without_delegation(
        &POLICY,
        proto::product_delivery_service_server::ProductDeliveryServiceServer::new(
            DeliveryService::new(core.delivery()),
        ),
        CallerRegistry::new()
            .register(Workload::Gateway, "gateway-fixture")
            .map_err(|_| Status::internal("caller"))?,
    )
    .map_err(|_| Status::internal("guard"))?;
    let bytes = cap.encode_to_vec();
    let mut frame = vec![0];
    frame.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    frame.extend(bytes);
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/concert.product.v1.ProductDeliveryService/CompleteResponse")
        .header("content-type", "application/grpc")
        .header("te", "trailers")
        .header("x-service-caller", "gateway")
        .header("x-service-token", "gateway-fixture")
        .body(axum::body::Body::from(frame))
        .map_err(|_| Status::internal("request"))?;
    let response = service
        .oneshot(request)
        .await
        .map_err(|_| Status::unavailable("delivery transport"))?;
    let headers = response.headers().clone();
    let body = response
        .into_body()
        .collect()
        .await
        .map_err(|_| Status::unavailable("delivery body"))?;
    let trailers = body.trailers();
    let status = headers
        .get("grpc-status")
        .or_else(|| trailers.and_then(|h| h.get("grpc-status")))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("0");
    if status != "0" {
        return Err(Status::unavailable("actual delivery refused"));
    }
    let bytes = body.to_bytes();
    if bytes.len() < 5 {
        return Err(Status::internal("delivery frame"));
    }
    let reply = proto::CompleteResponseResult::decode(&bytes[5..])
        .map_err(|_| Status::internal("delivery reply"))?;
    if !reply.completed {
        return Err(Status::unavailable("actual delivery incomplete"));
    }
    Ok(())
}
struct Terminal {
    core: CanonicalRuntimeCore,
    cap: proto::ResponseDelivery,
    calls: Arc<AtomicUsize>,
}
impl ResponseScope for Terminal {
    fn finish_response(
        self: Arc<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Status>> + Send + 'static>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            terminal(self.core.clone(), self.cap.clone()).await
        })
    }
}
async fn consume(token: ResponseScopeToken) -> bool {
    let service = tower::service_fn(move |_: ()| {
        let token = token.clone();
        async move {
            let mut response =
                axum::http::Response::new(Full::new(Bytes::from_static(b"safe fixture body")));
            response.extensions_mut().insert(token);
            Ok::<_, std::convert::Infallible>(response)
        }
    });
    RetainedScopeLayer
        .layer(service)
        .oneshot(())
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .is_ok()
}
async fn serialize(
    f: &fixture::Fixture,
    scope: Arc<OwnerGuardScope>,
) -> anyhow::Result<proto::ResponseDelivery> {
    let (cap, token) = f.owners.core.delivery().publish(scope).await?;
    anyhow::ensure!(
        consume(token).await,
        "actual inner body serialization failed"
    );
    Ok(cap)
}
async fn deliver(f: &fixture::Fixture, cap: proto::ResponseDelivery) -> bool {
    consume(ResponseScopeToken::new(Arc::new(Terminal {
        core: f.owners.core.clone(),
        cap,
        calls: Arc::new(AtomicUsize::new(0)),
    })))
    .await
}

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
async fn generation(p: &PgPool, actor: Uuid) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "select generation from auth_concert_user_security_generation where user_id=$1",
    )
    .bind(actor)
    .fetch_one(p)
    .await?)
}
async fn prepared(p: &PgPool) -> anyhow::Result<Vec<String>> {
    Ok(sqlx::query_scalar(
        "select gid from pg_prepared_xacts where database=current_database() order by gid",
    )
    .fetch_all(p)
    .await?)
}
fn key(scope: &OwnerGuardScope) -> anyhow::Result<ScopeKey> {
    let p = scope.producer_scope();
    Ok(ScopeKey {
        instance: p.instance_id.parse()?,
        scope: p.scope_id.parse()?,
        binding_digest: p
            .binding_digest
            .try_into()
            .map_err(|_| anyhow::anyhow!("digest"))?,
    })
}
async fn pending(p: &PgPool, k: &ScopeKey) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar("select count(*) from concert_product_child_intents where instance_id=$1 and scope_id=$2 and completed_at is null")
        .bind(k.instance).bind(k.scope).fetch_one(p).await?)
}
async fn deletion_waiting(
    p: &PgPool,
    task: &tokio::task::JoinHandle<anyhow::Result<DeleteUserResponseDto>>,
) -> anyhow::Result<()> {
    let stop = Instant::now() + Duration::from_secs(1);
    loop {
        let blocked: bool = sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and wait_event_type='Lock' and query like '%update users%' and 0=any(pg_blocking_pids(pid)))")
            .fetch_one(p).await?;
        anyhow::ensure!(
            !task.is_finished(),
            "deletion completed while canonical source SHARE remained"
        );
        if blocked {
            return Ok(());
        }
        anyhow::ensure!(
            Instant::now() < stop,
            "actual prepared blocker not observed"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}
async fn assert_deleted(p: &PgPool, actor: Uuid, before: i64) -> anyhow::Result<String> {
    let row = sqlx::query(
        "select primary_email,status,deleted_at is not null deleted from users where id=$1",
    )
    .bind(actor)
    .fetch_one(p)
    .await?;
    assert_eq!(row.get::<String, _>("status"), "DELETED");
    assert!(row.get::<bool, _>("deleted"));
    assert!(
        generation(p, actor).await? > before,
        "Auth0017 tombstone must advance with deletion"
    );
    let live: i64 = sqlx::query_scalar(
        "select count(*) from refresh_tokens where user_id=$1 and revoked_at is null",
    )
    .bind(actor)
    .fetch_one(p)
    .await?;
    assert_eq!(live, 0);
    let email: String = row.get("primary_email");
    assert!(login(p)?
        .login(&email, "Owned-ordering-42!", None, None)
        .await
        .is_err());
    let first = delete(p.clone(), actor).await?;
    let repeated = delete(p.clone(), actor).await?;
    assert_eq!(first.saga_id, repeated.saga_id);
    assert!(first.deleted && repeated.deleted);
    assert_eq!(first.saga.steps.len(), 8);
    assert_ne!(
        first.saga.status, "COMPLETED",
        "owner adapters have not completed BE42 cleanup"
    );
    Ok(first.saga_id)
}

// Private operator state contains identity/receipt references, never access or
// refresh tokens, full C03 bodies, completion secrets or reconstructed authority.
#[derive(Serialize, Deserialize)]
struct Saved {
    instance: Uuid,
    scope: Uuid,
    digest: [u8; 32],
    actor: Uuid,
    generation: i64,
    expiry: i64,
    old_pid: u32,
    pg_started: String,
    auth_prepared: Vec<String>,
    friend_prepared: Vec<String>,
}
fn state_path() -> anyhow::Result<std::path::PathBuf> {
    Ok(environment::root()?.join("be42-ordering-private.json"))
}
fn save(s: &Saved) -> anyhow::Result<()> {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(state_path()?)?;
    f.write_all(&serde_json::to_vec(s)?)?;
    f.sync_all()?;
    Ok(())
}
fn load() -> anyhow::Result<Saved> {
    let p = state_path()?;
    let m = std::fs::symlink_metadata(&p)?;
    anyhow::ensure!(
        m.is_file()
            && !m.file_type().is_symlink()
            && m.permissions().mode() & 0o777 == 0o600
            && m.len() < 16_384,
        "private owned handoff required"
    );
    Ok(serde_json::from_slice(&std::fs::read(p)?)?)
}

#[tokio::test]
#[ignore = "requires frozen owned PG16/mTLS source then physical restart and fresh test PID"]
async fn auth_deletion_owner_ordering() -> anyhow::Result<()> {
    match std::env::var("MCR_BE42_ORDERING_PHASE")?.as_str() {
        "SOURCE" => source().await,
        "RESTARTED" => restarted().await,
        _ => anyhow::bail!("explicit source/restarted phase required"),
    }
}
async fn source() -> anyhow::Result<()> {
    let instance = Uuid::new_v4();
    let f = fixture::Fixture::new(instance).await?;
    // First isolate the successful response-lifetime boundary from later faults.
    let (actor, family) = identity(&f.auth).await?;
    let before = generation(&f.auth, actor).await?;
    let scope = f
        .owners
        .fence
        .acquire(capture::binding(actor, family, f.event, false).await?)
        .await?;
    let k = key(&scope)?;
    let cap = serialize(&f, scope.clone()).await?;
    let deletion = tokio::spawn(delete(f.auth.clone(), actor));
    deletion_waiting(&f.auth, &deletion).await?;
    assert_eq!(
        f.owners.core.ledger().resolve_scope(&k).await?,
        ScopePhase::Active
    );
    let calls = Arc::new(AtomicUsize::new(0));
    let terminal = Arc::new(Terminal {
        core: f.owners.core.clone(),
        cap: cap.clone(),
        calls: calls.clone(),
    });
    let service = tower::service_fn(move |_: ()| {
        let t = terminal.clone();
        async move {
            let mut r = axum::http::Response::new(Full::new(Bytes::from_static(b"unread")));
            r.extensions_mut().insert(ResponseScopeToken::new(t));
            Ok::<_, std::convert::Infallible>(r)
        }
    });
    drop(RetainedScopeLayer.layer(service).oneshot(()).await?);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    deletion_waiting(&f.auth, &deletion).await?;
    anyhow::ensure!(
        deliver(&f, cap.clone()).await,
        "outer EOF must settle actual guards"
    );
    assert!(deliver(&f, cap).await, "terminal retry must be idempotent");
    let result = tokio::time::timeout(Duration::from_secs(3), deletion).await???;
    assert!(result.deleted);
    assert!(f.owners.core.ledger().delivery_completed(&k).await?);
    assert_deleted(&f.auth, actor, before).await?;
    assert!(prepared(&f.auth).await?.is_empty() && prepared(&f.friend).await?.is_empty());

    // A second canonical account exercises actual Track and unresolved native
    // participant/child retention across process exit and physical PG restart.
    let (actor, family) = identity(&f.auth).await?;
    let before = generation(&f.auth, actor).await?;
    let scope = f
        .owners
        .fence
        .acquire(capture::binding(actor, family, f.event, true).await?)
        .await?;
    let k = key(&scope)?;
    let mut tx = f.concert.begin().await?;
    let reference = Uuid::new_v4().as_bytes().to_vec();
    use sha2::Digest;
    let effect: [u8; 32] =
        sha2::Sha256::digest(b"BE42 actual unresolved producer transaction").into();
    scope
        .register_transaction_in(&mut tx, effect, reference.clone())
        .await?;
    scope
        .register_transaction_in(&mut tx, effect, reference)
        .await?;
    let tracked: i64 = sqlx::query_scalar("select count(*) from concert_product_owner_transactions where instance_id=$1 and scope_id=$2")
        .bind(k.instance).bind(k.scope).fetch_one(&f.concert).await?;
    assert_eq!(tracked, 1, "exact native Track retry must deduplicate");
    use concert_service::notification_live::proto::{
        execute_request::Input, CountInput, InboxOperation,
    };
    let error = f
        .owners
        .notification
        .execute(
            scope.clone(),
            InboxOperation::UnreadCount,
            Input::Count(CountInput {}),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), tonic::Code::Unavailable);
    assert_eq!(
        error.message(),
        "Notification root target policy unavailable"
    );
    assert_eq!(pending(&f.concert, &k).await?, 1);
    f.observation.deny_complete.store(true, Ordering::SeqCst);
    let cap = serialize(&f, scope.clone()).await?;
    let deletion = tokio::spawn(delete(f.auth.clone(), actor));
    deletion_waiting(&f.auth, &deletion).await?;
    assert!(
        !deliver(&f, cap).await,
        "live native participant cannot certify terminal completion"
    );
    assert_eq!(
        f.owners.core.ledger().resolve_scope(&k).await?,
        ScopePhase::RootFinished
    );
    assert!(!f.owners.core.ledger().delivery_completed(&k).await?);
    deletion_waiting(&f.auth, &deletion).await?;
    let mut worker = f
        .owners
        .core
        .child_recovery_worker(ChildRecoverySettings::default())?;
    let counts = worker.tick().await?;
    assert_eq!(counts.completed, 0);
    assert_eq!(pending(&f.concert, &k).await?, 1);
    assert_eq!(
        generation(&f.auth, actor).await?,
        before,
        "blocked disable must not commit a generation"
    );
    // Stop only the test's blocked deletion future. Actual producer rollback is
    // performed on its owning SQLx transaction, never on an inferred/prepared GID.
    deletion.abort();
    assert!(deletion.await.unwrap_err().is_cancelled());
    tx.rollback().await?;
    let pg_started: String = sqlx::query_scalar("select pg_postmaster_start_time()::text")
        .fetch_one(&f.auth)
        .await?;
    let a = prepared(&f.auth).await?;
    let b = prepared(&f.friend).await?;
    assert_eq!((a.len(), b.len()), (1, 1));
    save(&Saved {
        instance,
        scope: k.scope,
        digest: k.binding_digest,
        actor,
        generation: before,
        expiry: scope.binding().deadline().expires_micros(),
        old_pid: std::process::id(),
        pg_started,
        auth_prepared: a,
        friend_prepared: b,
    })?;
    println!("BE42 SOURCE PASS: real signup/verify/login; canonical deletion waits on prepared source SHARE; unread body retains; EOF settles; actual Track native in-progress withholds completion; unresolved original guards and child retained");
    Ok(())
}
async fn restarted() -> anyhow::Result<()> {
    let s = load()?;
    anyhow::ensure!(
        s.old_pid != std::process::id(),
        "fresh test process required"
    );
    let f = fixture::Fixture::new(s.instance).await?;
    let current: String = sqlx::query_scalar("select pg_postmaster_start_time()::text")
        .fetch_one(&f.auth)
        .await?;
    assert_ne!(
        current, s.pg_started,
        "actual physical PostgreSQL restart required"
    );
    assert_eq!(prepared(&f.auth).await?, s.auth_prepared);
    assert_eq!(prepared(&f.friend).await?, s.friend_prepared);
    let k = ScopeKey {
        instance: s.instance,
        scope: s.scope,
        binding_digest: s.digest,
    };
    assert_eq!(
        f.owners.core.ledger().resolve_scope(&k).await?,
        ScopePhase::RootFinished
    );
    let producer =
        concert_service::adapters::out::relationship_authority::guarded::proto::ProducerScope {
            instance_id: s.instance.to_string(),
            scope_id: s.scope.to_string(),
            binding_digest: s.digest.to_vec(),
        };
    assert!(
        f.owners
            .core
            .registry()
            .lookup_for_settlement(&producer)
            .is_err(),
        "no reconstructed actor authority"
    );
    assert_eq!(pending(&f.concert, &k).await?, 1);
    f.observation.deny_complete.store(true, Ordering::SeqCst);
    let deletion = tokio::spawn(delete(f.auth.clone(), s.actor));
    deletion_waiting(&f.auth, &deletion).await?;
    while chrono::Utc::now().timestamp_micros() <= s.expiry {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    // Actual authenticated resolver reaches the real reconstructed producer
    // settlement service and checks native full-XID state and immutable Track.
    use auth_service::concert_authority as a;
    let resolver = a::RpcSettlementResolver::new(
        &environment::tls(Workload::Auth)?,
        "auth-fixture",
        &[(a::proto::Producer::Concert, f.root_url.clone())],
    )?;
    let store = a::PgOwnerAuthority::new(f.auth.clone(), Arc::new(resolver))?;
    let budget = || a::OperationBudget::new(Instant::now() + Duration::from_secs(2));
    assert_eq!(store.recover(&budget()?).await?, 1);
    use friend_service::concert_authority as b;
    let resolver = b::RpcSettlementResolver::new(
        &environment::tls(Workload::Friend)?,
        "friend-fixture",
        &[(b::proto::Producer::Concert, f.root_url.clone())],
    )?;
    let store = b::PgOwnerAuthority::new(f.friend.clone(), Arc::new(resolver))?;
    assert_eq!(
        store
            .recover(&b::OperationBudget::new(
                Instant::now() + Duration::from_secs(2)
            )?)
            .await?,
        1
    );
    assert!(
        f.native_queries.load(Ordering::SeqCst) >= 2,
        "real native resolver must observe both original participant identities"
    );
    let result = tokio::time::timeout(Duration::from_secs(3), deletion).await???;
    assert!(result.deleted);
    let saga = assert_deleted(&f.auth, s.actor, s.generation).await?;
    let mut worker = f
        .owners
        .core
        .child_recovery_worker(ChildRecoverySettings::default())?;
    let counts = worker.tick().await?;
    assert_eq!(counts.completed, 0);
    assert_eq!(
        pending(&f.concert, &k).await?,
        1,
        "negative owner completion cannot forge a child ACK"
    );
    f.observation.deny_complete.store(false, Ordering::SeqCst);
    let counts = worker.tick().await?;
    assert_eq!(counts.completed, 1);
    assert_eq!(pending(&f.concert, &k).await?, 0);
    assert_eq!(delete(f.auth.clone(), s.actor).await?.saga_id, saga);
    assert!(
        !f.owners.core.ledger().delivery_completed(&k).await?,
        "restart cleanup does not manufacture successful response delivery"
    );
    for p in [&f.auth, &f.friend, &f.concert, &f.profile, &f.notification] {
        assert!(
            prepared(p).await?.is_empty(),
            "actual prepared work remains before owned teardown"
        );
    }
    println!("BE42 RESTARTED PASS: distinct PostgreSQL/test PID; exact original prepared guards retained; actual native abort/Track/seal recovery releases; canonical deletion generation advances and login denied; all eight saga targets remain truthful; original durable child recovers after account disable with no renewed authority; no fabricated response completion");
    Ok(())
}
