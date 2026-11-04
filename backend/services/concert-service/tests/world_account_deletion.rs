//! Component test transport only. Positive provenance comes from real Auth
//! signup/verification/login/deletion repositories in a fresh owned PG16.
//! This does not mount or certify production Auth -> World deletion transport.
use anyhow::Result;
use async_trait::async_trait;
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
        dto::{auth_dto::DeleteUserDto, deletion_dto::AuthUserDeletionTargetDto},
        password_work::PasswordWorkLimits,
        ports::{UserDeletionSagaPort, UserRepositoryPort},
        use_cases::{
            delete_user::DeleteUserUseCase,
            login::{LoginUseCase, LoginUseCaseDeps},
            signup::SignupUseCase,
        },
    },
    config::JwtTokenService,
    domain::user::{User, UserStatus},
};
use chrono::{Duration, Utc};
use concert_application::world_ticket::*;
use concert_domain::{AssetId, AuthUserId, EventId, ParticipantKey, Revision};
use concert_service::world::{account_deletion::*, PgWorldRepository};
use sha2::{Digest, Sha256};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool, Row,
};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration as WallDuration,
};
use tokio::time::Instant;
use uuid::Uuid;
fn command() -> WorldDeletionCommand {
    WorldDeletionCommand {
        saga_id: Uuid::from_u128(1),
        user_id: Uuid::from_u128(2),
        target: WorldDeletionTarget::Tickets,
        actor_id: Uuid::from_u128(2),
        retention_until: "2070-01-01T00:00:00.123456789Z".parse().unwrap(),
    }
}
fn deadline() -> Instant {
    Instant::now() + WallDuration::from_secs(4)
}
#[test]
fn exact_binding_preserves_target_actor_and_submicrosecond_retention() {
    let c = command();
    for target in [
        WorldDeletionTarget::Sessions,
        WorldDeletionTarget::Sharing,
        WorldDeletionTarget::PrivateData,
    ] {
        let mut other = c.clone();
        other.target = target;
        assert_eq!(c.identity_binding(), other.identity_binding());
        assert_ne!(c.binding(), other.binding());
    }
    let mut other = c.clone();
    other.retention_until += Duration::nanoseconds(1);
    assert_ne!(c.binding(), other.binding());
    other = c.clone();
    other.actor_id = Uuid::from_u128(3);
    assert_ne!(c.identity_binding(), other.identity_binding());
    assert_eq!(format!("{c:?}"), "WorldDeletionCommand([REDACTED])");
}
#[tokio::test]
async fn invalid_identity_deadline_and_unverified_source_fail_closed() {
    let mut c = command();
    c.user_id = Uuid::nil();
    assert_eq!(c.validate(), Err(DeletionFailure::InvalidCommand));
    c = command();
    assert!(VerifiedDeletionSource::from_verified_auth_source(&c, deadline(), [0; 32]).is_err());
    assert!(
        VerifiedDeletionSource::from_verified_auth_source(&c, Instant::now(), [1; 32]).is_err()
    );
    assert!(VerifiedDeletionSource::from_verified_auth_source(
        &c,
        Instant::now() + WallDuration::from_secs(6),
        [1; 32]
    )
    .is_err());
    let d = deadline();
    let proof = VerifiedDeletionSource::from_verified_auth_source(&c, d, [1; 32]).unwrap();
    let mut rebound = c.clone();
    rebound.saga_id = Uuid::from_u128(7);
    assert_eq!(
        proof.check(&rebound, d),
        Err(DeletionFailure::InvalidCommand)
    );
    assert_eq!(
        proof.check(&c, d + WallDuration::from_millis(1)),
        Err(DeletionFailure::InvalidCommand)
    );
}
struct MissingAuthority;
#[async_trait]
impl DeletionAuthority for MissingAuthority {
    async fn verify(
        &self,
        _: &WorldDeletionCommand,
        _: Instant,
    ) -> Result<VerifiedDeletionSource, DeletionFailure> {
        Err(DeletionFailure::Unavailable)
    }
}
#[tokio::test]
async fn missing_authority_cannot_turn_a_raw_command_into_a_proof() {
    assert_eq!(
        verify_source(&MissingAuthority, &command(), deadline())
            .await
            .err(),
        Some(DeletionFailure::Unavailable)
    );
}
/// Configured TEST transport. It checks the actual repository record and
/// canonical target inventory; callers cannot grant provenance by raw IDs.
struct AuthRepositoryTestTransport {
    auth: PgPool,
    reject_after: Option<usize>,
    calls: AtomicUsize,
}
impl AuthRepositoryTestTransport {
    fn new(auth: PgPool) -> Self {
        Self {
            auth,
            reject_after: None,
            calls: AtomicUsize::new(0),
        }
    }
}
#[async_trait]
impl DeletionAuthority for AuthRepositoryTestTransport {
    async fn verify(
        &self,
        c: &WorldDeletionCommand,
        d: Instant,
    ) -> Result<VerifiedDeletionSource, DeletionFailure> {
        if self
            .reject_after
            .is_some_and(|n| self.calls.fetch_add(1, Ordering::SeqCst) >= n)
        {
            return Err(DeletionFailure::Unavailable);
        }
        let source = PgUserDeletionSagaRepository::new(self.auth.clone())
            .find_by_user_id(c.user_id)
            .await
            .map_err(|_| DeletionFailure::Unavailable)?
            .ok_or(DeletionFailure::Unavailable)?;
        let user = PgUserRepository::new(self.auth.clone(), Arc::new(SystemAuthRuntime))
            .find_by_id(c.user_id)
            .await
            .map_err(|_| DeletionFailure::Unavailable)?
            .map(User::from)
            .ok_or(DeletionFailure::Unavailable)?;
        if user.status != UserStatus::Deleted
            || source.id != c.saga_id
            || source.user_id != c.user_id
            || source.actor_id != c.actor_id
            || source.retention_until != c.retention_until
            || c.target != WorldDeletionTarget::Tickets
            || !source.steps.iter().any(|s| {
                s.target == AuthUserDeletionTargetDto::ConcertTickets
                    && s.saga_id == c.saga_id
                    && s.user_id == c.user_id
                    && s.actor_id == c.actor_id
                    && s.retention_until == c.retention_until
            })
        {
            return Err(DeletionFailure::InvalidCommand);
        }
        let mut hash = Sha256::new();
        hash.update(b"TEST-TRANSPORT-ACTUAL-AUTH-REPOSITORY-v1");
        hash.update(c.binding()?);
        VerifiedDeletionSource::from_verified_auth_source(c, d, hash.finalize().into())
    }
}
async fn owned_pool(db: &str) -> Result<PgPool> {
    use std::{
        os::unix::fs::{MetadataExt, PermissionsExt},
        path::PathBuf,
        str::FromStr,
    };
    let root = PathBuf::from(std::env::var("BE42_OWNER_ROOT")?);
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        info.is_dir() && !info.file_type().is_symlink() && info.permissions().mode() & 0o077 == 0,
        "private owned root required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino()) == std::env::var("BE42_OWNER_IDENTITY")?,
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
        marker["token"].as_str() == Some(&std::env::var("BE42_OWNER_TOKEN")?),
        "foreign fixture marker"
    );
    let options =
        PgConnectOptions::from_str(&std::env::var(if db == "mcr_world_deletion_auth" {
            "BE42_AUTH_URL"
        } else {
            "BE42_CONCERT_URL"
        })?)?;
    anyhow::ensure!(
        options.get_database() == Some(db)
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(WallDuration::from_secs(2))
        .connect_with(options)
        .await?;
    let row=sqlx::query("select current_setting('data_directory') dir,current_setting('listen_addresses') listeners,current_setting('server_version_num')::int version").fetch_one(&pool).await?;
    anyhow::ensure!(
        row.get::<String, _>("dir") == root.join("data").to_string_lossy()
            && row.get::<String, _>("listeners").is_empty()
            && row.get::<i32, _>("version") / 10000 == 16,
        "foreign PG cluster refused"
    );
    Ok(pool)
}
async fn actual_auth_user(auth: &PgPool) -> Result<Uuid> {
    let runtime = Arc::new(SystemAuthRuntime);
    let users = Arc::new(PgUserRepository::new(auth.clone(), runtime.clone()));
    let work = Arc::new(AdmittedArgon2PasswordWork::new(
        PasswordWorkLimits::default(),
    )?);
    let signup = SignupUseCase::new(
        users.clone(),
        users.clone(),
        work.clone(),
        runtime.clone(),
        Arc::new(SecureEmailVerification),
    );
    let email = format!("{}@example.invalid", Uuid::new_v4().simple());
    let password = "BE42-actual-password-42!";
    let registered = signup.signup_email(&email, password).await?;
    let code: String = sqlx::query_scalar("select code from email_verifications where email=$1")
        .bind(&email)
        .fetch_one(auth)
        .await?;
    signup.verify_email(&email, &code).await?;
    let login = LoginUseCase::new(LoginUseCaseDeps {
        user_repository: users.clone(),
        login_audit_repository: Arc::new(PgLoginAuditRepository::new(auth.clone())),
        refresh_token_store: Arc::new(PgRefreshTokenStore::new(auth.clone(), runtime.clone())),
        token_service: Arc::new(JwtTokenService::new(
            "be42-component",
            "be42-test",
            900,
            1209600,
        )?),
        transactions: users,
        password_work: work,
        runtime,
    });
    let session = login.login(&email, password, None, None).await?;
    anyhow::ensure!(
        registered.user_id == session.user_id
            && !session.access_token.is_empty()
            && !session.refresh_token.is_empty(),
        "actual Auth login missing"
    );
    let count: i64 = sqlx::query_scalar(
        "select count(*) from refresh_tokens where user_id=$1 and revoked_at is null",
    )
    .bind(registered.user_id)
    .fetch_one(auth)
    .await?;
    anyhow::ensure!(count == 1, "actual login persistence missing");
    Ok(registered.user_id)
}
async fn actual_delete(auth: &PgPool, user: Uuid) -> Result<WorldDeletionCommand> {
    let response = DeleteUserUseCase::new(
        Arc::new(PgUserRepository::new(
            auth.clone(),
            Arc::new(SystemAuthRuntime),
        )),
        Arc::new(PgAuthAuditRepository::new(auth.clone())),
        PgUserDeletionSagaRepository::new(auth.clone()),
        Arc::new(SystemAuthRuntime),
    )
    .execute(DeleteUserDto {
        user_id: user.to_string(),
        actor_id: user.to_string(),
        request_id: None,
        ip_address: None,
        user_agent: None,
    })
    .await?;
    let source = PgUserDeletionSagaRepository::new(auth.clone())
        .find_by_user_id(user)
        .await?
        .unwrap();
    anyhow::ensure!(
        response.saga_id == source.id.to_string(),
        "canonical saga mismatch"
    );
    Ok(WorldDeletionCommand {
        saga_id: source.id,
        user_id: source.user_id,
        target: WorldDeletionTarget::Tickets,
        actor_id: source.actor_id,
        retention_until: source.retention_until,
    })
}
async fn room(pool: &PgPool) -> Result<ResourcePin> {
    let event = Uuid::new_v4();
    let room = Uuid::new_v4();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'BE42 component','OPEN')").bind(event).bind(format!("be42-{}",event.simple())).execute(pool).await?;
    sqlx::query("insert into concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,is_default) values($1,$2,'Room',50,'map_fixture',1,'avatar_manifest',1,true)").bind(event).bind(room).execute(pool).await?;
    Ok(ResourcePin {
        event_id: EventId::from_uuid(event),
        room_id: RoomId::from_uuid(room),
        map: MapPin::new(
            AssetId::new("map_fixture".into()).unwrap(),
            Revision::initial(),
        ),
        avatar: AvatarPin::new(
            AssetId::new("avatar_manifest".into()).unwrap(),
            Revision::initial(),
        ),
    })
}
fn record(pin: ResourcePin, user: Uuid) -> TicketRecord {
    let mut entropy = [0u8; TICKET_BYTES];
    entropy[..16].copy_from_slice(Uuid::new_v4().as_bytes());
    entropy[16..].copy_from_slice(Uuid::new_v4().as_bytes());
    let now = Utc::now();
    TicketRecord {
        digest: AdmissionTicket::from_entropy(&entropy).unwrap().digest(),
        subject: WorldSubject {
            key: ParticipantKey {
                event_id: pin.event_id,
                user_id: AuthUserId::from_uuid(user),
            },
            event_id: pin.event_id,
            session_id: concert_application::identity::SessionId::new(Uuid::new_v4().to_string())
                .unwrap(),
        },
        pin,
        issued_at: now,
        expires_at: now + Duration::seconds(30),
        access_expires_at: now + Duration::seconds(90),
    }
}
fn claim(r: &TicketRecord) -> ConsumeClaim {
    ConsumeClaim {
        digest: r.digest.clone(),
        connection_id: ConnectionId::from_uuid(Uuid::new_v4()).unwrap(),
        requested: r.pin.clone(),
        now: Utc::now(),
    }
}
async fn pending(pool: &PgPool, user: Uuid) -> Result<i64> {
    Ok(sqlx::query_scalar(
        "select count(*) from concert_world_tickets where user_id=$1 and consumed_at is null",
    )
    .bind(user)
    .fetch_one(pool)
    .await?)
}
async fn wait_user_lock(pool: &PgPool) -> Result<()> {
    tokio::time::timeout(WallDuration::from_secs(1),async {
        loop {
            let n:i64=sqlx::query_scalar("select count(*) from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and cardinality(pg_blocking_pids(pid))>0 and strpos(query,'mcr-world-account-deletion:')>0").fetch_one(pool).await?;
            if n>0{return Ok::<_,anyhow::Error>(())}
            tokio::time::sleep(WallDuration::from_millis(5)).await;
        }
    }).await??;
    Ok(())
}
#[tokio::test]
#[ignore = "fresh allocator-owned marker/inode PG16 only; explicit component test transport"]
async fn native_actual_auth_bounded_replay_rollback_and_ticket_races() -> Result<()> {
    let auth = owned_pool("mcr_world_deletion_auth").await?;
    let mut pool = owned_pool("mcr_world_deletion_concert").await?;
    let user = actual_auth_user(&auth).await?;
    let other = actual_auth_user(&auth).await?;
    let race = actual_auth_user(&auth).await?;
    let a = room(&pool).await?;
    let b = room(&pool).await?;
    let repo = PgWorldRepository::new(pool.clone());
    let consumed = record(a.clone(), user);
    assert_eq!(repo.issue(consumed.clone()).await?, IssueOutcome::Issued);
    assert!(matches!(
        repo.consume(claim(&consumed)).await?,
        ConsumeOutcome::Consumed(_)
    ));
    let preserved: serde_json::Value = sqlx::query_scalar(
        "select to_jsonb(t) from concert_world_tickets t where ticket_digest=$1",
    )
    .bind(consumed.digest.as_bytes().as_slice())
    .fetch_one(&pool)
    .await?;
    let mut revoked = None;
    for i in 0..129 {
        let mut r = record(if i % 2 == 0 { a.clone() } else { b.clone() }, user);
        if i == 0 {
            r.issued_at -= Duration::seconds(60);
            r.expires_at -= Duration::seconds(60);
        }
        assert_eq!(repo.issue(r.clone()).await?, IssueOutcome::Issued);
        revoked = Some(r);
    }
    let unrelated = record(a.clone(), other);
    assert_eq!(repo.issue(unrelated.clone()).await?, IssueOutcome::Issued);
    let transport = AuthRepositoryTestTransport::new(auth.clone());
    let mut unknown = command();
    unknown.user_id = user;
    unknown.actor_id = user;
    assert_eq!(
        verify_source(&transport, &unknown, deadline()).await.err(),
        Some(DeletionFailure::Unavailable)
    );
    let command = actual_delete(&auth, user).await?;
    let unavailable = TicketDeletion::new(pool.clone(), Arc::new(MissingAuthority));
    assert_eq!(
        unavailable.apply(command.clone(), deadline()).await.err(),
        Some(DeletionFailure::Unavailable)
    );
    assert_eq!(pending(&pool, user).await?, 129);
    let mut unstable = AuthRepositoryTestTransport::new(auth.clone());
    unstable.reject_after = Some(1);
    let rollback = TicketDeletion::new(pool.clone(), Arc::new(unstable));
    assert_eq!(
        rollback.apply(command.clone(), deadline()).await.err(),
        Some(DeletionFailure::Unavailable)
    );
    assert_eq!(pending(&pool, user).await?, 129);
    let fenced: bool = sqlx::query_scalar(
        "select exists(select 1 from concert_world_account_deletion_fences where user_id=$1)",
    )
    .bind(user)
    .fetch_one(&pool)
    .await?;
    assert!(!fenced);
    let authority = Arc::new(AuthRepositoryTestTransport::new(auth.clone()));
    let deletion = TicketDeletion::new(pool.clone(), authority.clone());
    assert_eq!(
        deletion.apply(command.clone(), deadline()).await.err(),
        Some(DeletionFailure::Incomplete)
    );
    assert_eq!(pending(&pool, user).await?, 1);
    assert_eq!(
        repo.issue(record(a.clone(), user)).await,
        Err(WorldError::Denied)
    );
    assert_eq!(
        repo.consume(claim(&consumed)).await,
        Err(WorldError::Denied)
    );
    assert!(matches!(
        repo.consume(claim(&revoked.unwrap())).await,
        Ok(ConsumeOutcome::Unknown) | Err(WorldError::Denied)
    ));
    let count: i64 = sqlx::query_scalar(
        "select revoked_count from concert_world_ticket_deletion_progress where user_id=$1",
    )
    .bind(user)
    .fetch_one(&pool)
    .await?;
    assert_eq!(count, 128);
    // Recreate pool and service to model loss of all in-memory worker progress.
    drop(unavailable);
    drop(rollback);
    drop(deletion);
    drop(repo);
    pool.close().await;
    pool = owned_pool("mcr_world_deletion_concert").await?;
    let deletion = TicketDeletion::new(pool.clone(), authority.clone());
    let receipt = deletion
        .apply(command.clone(), deadline())
        .await
        .map_err(|_| anyhow::anyhow!("completion missing"))?;
    assert_eq!(receipt.command, command);
    assert_eq!(receipt.fence_revision, 1);
    assert_eq!(
        deletion
            .apply(command.clone(), deadline())
            .await
            .map_err(|_| anyhow::anyhow!("replay missing"))?,
        receipt
    );
    assert_eq!(pending(&pool, user).await?, 0);
    assert_eq!(pending(&pool, other).await?, 1);
    let after: serde_json::Value = sqlx::query_scalar(
        "select to_jsonb(t) from concert_world_tickets t where ticket_digest=$1",
    )
    .bind(consumed.digest.as_bytes().as_slice())
    .fetch_one(&pool)
    .await?;
    assert_eq!(preserved, after);
    for change in 0..4 {
        let mut rebound = command.clone();
        match change {
            0 => rebound.actor_id = other,
            1 => rebound.retention_until += Duration::nanoseconds(1),
            2 => rebound.saga_id = Uuid::new_v4(),
            _ => rebound.target = WorldDeletionTarget::Sessions,
        };
        assert_eq!(
            deletion.apply(rebound, deadline()).await.err(),
            Some(DeletionFailure::InvalidCommand)
        );
    }
    for sql in [
        "delete from concert_world_account_deletion_fences",
        "update concert_world_account_deletion_fences set fence_revision=1",
        "truncate concert_world_account_deletion_fences cascade",
        "delete from concert_world_ticket_deletion_progress",
        "update concert_world_ticket_deletion_progress set progress_revision=progress_revision+1",
        "truncate concert_world_ticket_deletion_progress",
    ] {
        assert!(sqlx::query(sql).execute(&pool).await.is_err());
    }
    // First issuance waits behind deletion's user lock despite no fence row.
    let race_command = actual_delete(&auth, race).await?;
    let proof = verify_source(authority.as_ref(), &race_command, deadline())
        .await
        .map_err(|_| anyhow::anyhow!("race source missing"))?;
    let mut held = pool.begin().await?;
    establish_fence_in(&mut held, &race_command, &proof)
        .await
        .map_err(|_| anyhow::anyhow!("fence missing"))?;
    let contender = PgWorldRepository::new(pool.clone());
    let r = record(a.clone(), race);
    let issue = tokio::spawn(async move { contender.issue(r).await });
    wait_user_lock(&pool).await?;
    held.commit().await?;
    assert_eq!(issue.await?, Err(WorldError::Denied));
    assert_eq!(pending(&pool, race).await?, 0);
    let race_deletion = TicketDeletion::new(pool.clone(), authority.clone());
    assert!(race_deletion
        .apply(race_command.clone(), deadline())
        .await
        .is_ok());
    // An already-issued ticket consume waits behind the SAME namespace.
    let other_command = actual_delete(&auth, other).await?;
    let proof = verify_source(authority.as_ref(), &other_command, deadline())
        .await
        .map_err(|_| anyhow::anyhow!("consume source missing"))?;
    let mut held = pool.begin().await?;
    establish_fence_in(&mut held, &other_command, &proof)
        .await
        .map_err(|_| anyhow::anyhow!("consume fence missing"))?;
    let contender = PgWorldRepository::new(pool.clone());
    let request = claim(&unrelated);
    let consume = tokio::spawn(async move { contender.consume(request).await });
    wait_user_lock(&pool).await?;
    held.commit().await?;
    assert_eq!(consume.await?, Err(WorldError::Denied));
    assert!(deletion.apply(other_command, deadline()).await.is_ok());
    // Original deadline expires while waiting on the advisory lock; no effects.
    let fourth = actual_auth_user(&auth).await?;
    let fourth_command = actual_delete(&auth, fourth).await?;
    let mut gate = pool.begin().await?;
    lock_user(&mut gate, fourth).await?;
    let start = Instant::now();
    assert_eq!(
        deletion
            .apply(
                fourth_command.clone(),
                start + WallDuration::from_millis(80)
            )
            .await
            .err(),
        Some(DeletionFailure::Unavailable)
    );
    assert!(start.elapsed() < WallDuration::from_secs(1));
    gate.rollback().await?;
    assert!(deletion.apply(fourth_command, deadline()).await.is_ok());
    eprintln!("BE42 component PASS: actual Auth signup/verify/login/delete source; bounded128+1; rollback; restart/exactreplay; consumedreceipt/unrelated preserved; permanentfence; firstissue/consume locks; originaldeadline. NOT production transport or complete saga.");
    pool.close().await;
    auth.close().await;
    Ok(())
}
