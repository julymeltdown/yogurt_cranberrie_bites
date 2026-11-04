//! Native component tests, NOT production Auth-to-Concert or HTTP/WS acceptance.
//! Actual Auth fixture helpers adapted from ticket owner 9bbd33e9; only Sharing
//! target mapping changes. No Auth user SQL seed, age claim or forced settlement.
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
use chrono::Utc;
use concert_service::{
    adapters::out::repository::outbox::DeliveryFailure,
    social_live::{
        account_deletion::SharingDeletion,
        invalidation::{
            PolicyReason, SocialInvalidationSink, SocialPolicyPublisher, VerifiedSocialInvalidation,
        },
    },
    world::account_deletion::*,
};
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};
use sha2::{Digest, Sha256};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool, Row,
};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration as WallDuration,
};
use tokio::time::Instant;
use uuid::Uuid;
fn deadline() -> Instant {
    Instant::now() + WallDuration::from_secs(4)
}

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
            || c.target != WorldDeletionTarget::Sharing
            || !source.steps.iter().any(|s| {
                s.target == AuthUserDeletionTargetDto::ConcertSharing
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
        target: WorldDeletionTarget::Sharing,
        actor_id: source.actor_id,
        retention_until: source.retention_until,
    })
}

/// Explicit component private-view fixture, not the production World publisher.
/// Every expected tuple has an actual retained local view. Unknown/empty tuples
/// are Retryable; no empty-registry acknowledgement stands in for withdrawal.
struct Publisher {
    fail: AtomicBool,
    hold: AtomicBool,
    views: Mutex<std::collections::BTreeMap<(Uuid, Uuid), Option<i64>>>,
    calls: AtomicUsize,
}
impl Publisher {
    fn new(tuples: impl IntoIterator<Item = (Uuid, Uuid)>) -> Self {
        Self {
            fail: AtomicBool::new(false),
            hold: AtomicBool::new(false),
            views: Mutex::new(tuples.into_iter().map(|tuple| (tuple, None)).collect()),
            calls: AtomicUsize::new(0),
        }
    }
}
#[async_trait]
impl SocialPolicyPublisher for Publisher {
    async fn publish(&self, signal: &VerifiedSocialInvalidation) -> Result<(), DeliveryFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.hold.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        if self.fail.load(Ordering::SeqCst)
            || signal.reason() != PolicyReason::CanonicalPolicyChanged
        {
            return Err(DeliveryFailure::Retryable);
        }
        let mut views = self.views.lock().unwrap();
        let view = views
            .get_mut(&(signal.event(), signal.actor()))
            .ok_or(DeliveryFailure::Retryable)?;
        *view = Some(view.unwrap_or(0).max(signal.policy_revision()));
        Ok(())
    }
}
fn owner(pool: &PgPool, auth: &PgPool, publisher: Arc<Publisher>) -> SharingDeletion {
    SharingDeletion::new(
        pool.clone(),
        Arc::new(AuthRepositoryTestTransport::new(auth.clone())),
        SocialInvalidationSink::new(pool.clone(), publisher),
    )
}
struct Missing;
#[async_trait]
impl DeletionAuthority for Missing {
    async fn verify(
        &self,
        _: &WorldDeletionCommand,
        _: Instant,
    ) -> Result<VerifiedDeletionSource, DeletionFailure> {
        Err(DeletionFailure::Unavailable)
    }
}
#[tokio::test]
async fn raw_command_missing_auth_or_wrong_target_cannot_touch_database() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgresql://localhost/unused")
        .unwrap();
    let sink = SocialInvalidationSink::new(pool.clone(), Arc::new(Publisher::new([])));
    let owner = SharingDeletion::new(pool, Arc::new(Missing), sink);
    let mut command = WorldDeletionCommand {
        saga_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        actor_id: Uuid::new_v4(),
        target: WorldDeletionTarget::Sharing,
        retention_until: Utc::now(),
    };
    assert_eq!(
        owner.apply(command.clone(), deadline()).await.err(),
        Some(DeletionFailure::Unavailable)
    );
    command.target = WorldDeletionTarget::Tickets;
    assert_eq!(
        owner.apply(command, deadline()).await.err(),
        Some(DeletionFailure::InvalidCommand)
    );
}
#[tokio::test]
async fn expired_and_extended_dispatch_are_rejected_before_authority() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgresql://localhost/unused")
        .unwrap();
    let owner = SharingDeletion::new(
        pool.clone(),
        Arc::new(Missing),
        SocialInvalidationSink::new(pool, Arc::new(Publisher::new([]))),
    );
    let command = WorldDeletionCommand {
        saga_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        actor_id: Uuid::new_v4(),
        target: WorldDeletionTarget::Sharing,
        retention_until: Utc::now(),
    };
    assert!(owner.apply(command.clone(), Instant::now()).await.is_err());
    assert!(owner
        .apply(command, Instant::now() + WallDuration::from_secs(6))
        .await
        .is_err());
}

/// Real AES-GCM opaque fixture material, same headless AAD encoding as the
/// retained relationship/crypto.rs. No production key/provider/claim supplied.
fn encrypted(event: Uuid, user: Uuid) -> (Vec<u8>, Vec<u8>) {
    let random = SystemRandom::new();
    let mut key = [0; 32];
    let mut nonce = [0; 12];
    random.fill(&mut key).unwrap();
    random.fill(&mut nonce).unwrap();
    let key = aead::LessSafeKey::new(aead::UnboundKey::new(&aead::AES_256_GCM, &key).unwrap());
    let mut aad = b"MCR-social-handle-AES256GCM-v1".to_vec();
    for part in [
        event.as_bytes().as_slice(),
        user.as_bytes().as_slice(),
        b"INSTAGRAM".as_slice(),
        1i64.to_be_bytes().as_slice(),
        b"deletion_fixture".as_slice(),
    ] {
        aad.extend((part.len() as u64).to_be_bytes());
        aad.extend(part);
    }
    let mut body = b"private.deletion.fixture".to_vec();
    key.seal_in_place_append_tag(
        aead::Nonce::assume_unique_for_key(nonce),
        aead::Aad::from(aad),
        &mut body,
    )
    .unwrap();
    (nonce.to_vec(), body)
}
struct Fixture {
    auth: PgPool,
    pool: PgPool,
    user: Uuid,
    peer: Uuid,
    events: Vec<Uuid>,
    publisher: Arc<Publisher>,
}
impl Fixture {
    async fn new(events: usize, requests: usize) -> Result<Self> {
        let auth = owned_pool("mcr_world_deletion_auth").await?;
        let pool = owned_pool("mcr_world_deletion_concert").await?;
        let user = actual_auth_user(&auth).await?;
        let peer = actual_auth_user(&auth).await?;
        let mut ids = Vec::new();
        for _ in 0..events {
            let event = Uuid::new_v4();
            ids.push(event);
            let mut tx = pool.begin().await?;
            // Storage fixture guard ONLY: not a Participant command, canonical
            // policy proof, age claim, owner settlement, or deletion source.
            sqlx::query("select set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '4 seconds')::text,true)")
                .execute(&mut *tx).await?;
            sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Sharing deletion component','OPEN')")
                .bind(event).bind(format!("sharing-{}",event.simple())).execute(&mut *tx).await?;
            for actor in [user, peer] {
                sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)")
                    .bind(event).bind(actor).execute(&mut *tx).await?;
                let (nonce, ciphertext) = encrypted(event, actor);
                sqlx::query("insert into concert_social_handles(event_id,user_id,platform,key_id,nonce,ciphertext) values($1,$2,'INSTAGRAM','deletion_fixture',$3,$4)")
                    .bind(event).bind(actor).bind(nonce).bind(ciphertext).execute(&mut *tx).await?;
            }
            for n in 0..requests {
                let id = Uuid::new_v4();
                let (requester, recipient) = if n % 2 == 0 {
                    (user, peer)
                } else {
                    (peer, user)
                };
                sqlx::query("insert into concert_social_requests(event_id,request_id,requester_id,recipient_id,intro_id,intro_revision,authority_revision,relationship_revision,state,expires_at) values($1,$2,$3,$4,$5,1,1,1,'PENDING',clock_timestamp()+interval '1 hour')")
                    .bind(event).bind(id).bind(requester).bind(recipient).bind(Uuid::new_v4()).execute(&mut *tx).await?;
                sqlx::query("insert into concert_social_bindings(event_id,request_id,platform,requester_version,recipient_version,requester_consents,approved) values($1,$2,'INSTAGRAM',1,1,true,true)")
                    .bind(event).bind(id).execute(&mut *tx).await?;
                sqlx::query("update concert_social_requests set state='APPROVED',revision=2 where event_id=$1 and request_id=$2")
                    .bind(event).bind(id).execute(&mut *tx).await?;
                for (owner, viewer) in [(user, peer), (peer, user)] {
                    sqlx::query("insert into concert_social_grants(event_id,request_id,owner_id,viewer_id,platform,handle_version,peer_handle_version,state,expires_at) select event_id,request_id,$3,$4,'INSTAGRAM',1,1,'ACTIVE',expires_at from concert_social_requests where event_id=$1 and request_id=$2")
                        .bind(event).bind(id).bind(owner).bind(viewer).execute(&mut *tx).await?;
                }
            }
            tx.commit().await?;
        }
        let publisher = Arc::new(Publisher::new(
            ids.iter()
                .flat_map(|event| [(*event, user), (*event, peer)]),
        ));
        Ok(Self {
            auth,
            pool,
            user,
            peer,
            events: ids,
            publisher,
        })
    }
    async fn finish(&self, command: &WorldDeletionCommand) -> Result<DeletionAcknowledgement> {
        for _ in 0..64 {
            match owner(&self.pool, &self.auth, self.publisher.clone())
                .apply(command.clone(), deadline())
                .await
            {
                Ok(ack) => return Ok(ack),
                Err(DeletionFailure::Incomplete) => (),
                Err(error) => anyhow::bail!("Sharing deletion failed: {error:?}"),
            }
        }
        anyhow::bail!("bounded deletion fixture did not converge")
    }
}
#[tokio::test]
#[ignore = "explicit marker/inode-owned native PG16 qualification"]
async fn native_all_events_incoming_outgoing_retry_withdrawal_tombstone_and_exact_replay(
) -> Result<()> {
    let f = Fixture::new(2, 70).await?;
    let ordinary_before:(i64,i64)=sqlx::query_as("select (select count(*) from concert_command_receipts),(select count(*) from concert_outbox)")
        .fetch_one(&f.pool).await?;
    let command = actual_delete(&f.auth, f.user).await?;
    f.publisher.fail.store(true, Ordering::SeqCst);
    assert_eq!(
        owner(&f.pool, &f.auth, f.publisher.clone())
            .apply(command.clone(), deadline())
            .await
            .err(),
        Some(DeletionFailure::Incomplete)
    );
    let fence:bool=sqlx::query_scalar("select exists(select 1 from concert_world_account_deletion_fences where user_id=$1 and saga_id=$2)")
        .bind(f.user).bind(command.saga_id).fetch_one(&f.pool).await?;
    assert!(fence);
    let completed: bool = sqlx::query_scalar(
        "select completed_at is not null from concert_social_account_deletions where saga_id=$1",
    )
    .bind(command.saga_id)
    .fetch_one(&f.pool)
    .await?;
    assert!(!completed);
    let cascade = sqlx::query("delete from concert_events where event_id=$1")
        .bind(f.events[0])
        .execute(&f.pool)
        .await;
    assert!(
        cascade.is_err(),
        "pending source must survive event cascade"
    );
    let changed_signal=sqlx::query("update concert_social_policy_signals set deletion_source_binding=decode(repeat('01',32),'hex') where deletion_saga=$1")
        .bind(command.saga_id).execute(&f.pool).await;
    assert!(
        changed_signal.is_err(),
        "exact sealed deletion source is immutable"
    );
    assert!(sqlx::query("truncate public.concert_social_policy_signals")
        .execute(&f.pool)
        .await
        .is_err());
    assert!(
        sqlx::query("delete from concert_social_bindings where event_id=$1")
            .bind(f.events[0])
            .execute(&f.pool)
            .await
            .is_err()
    );
    // Even a normal still-live storage guard cannot recreate deleted material.
    let mut tx = f.pool.begin().await?;
    sqlx::query("select set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '4 seconds')::text,true)").execute(&mut *tx).await?;
    let denied=sqlx::query("update concert_social_handles set key_id='bad',nonce=decode(repeat('01',12),'hex'),ciphertext=decode(repeat('01',20),'hex'),revision=revision+1 where event_id=$1 and user_id=$2")
        .bind(f.events[0]).bind(f.user).execute(&mut *tx).await;
    assert!(denied.is_err());
    tx.rollback().await?;
    f.publisher.fail.store(false, Ordering::SeqCst);
    let ack = f.finish(&command).await?;
    assert!(ack.command == command);
    assert_eq!(ack.fence_revision, 1);
    let counts:(i64,i64,i64)=sqlx::query_as("select (select count(*) from concert_social_handles where user_id=$1 and ciphertext is not null),(select count(*) from concert_social_requests where $1 in(requester_id,recipient_id) and state in('PENDING','APPROVED')),(select count(*) from concert_social_grants where $1 in(owner_id,viewer_id) and state='ACTIVE')")
        .bind(f.user).fetch_one(&f.pool).await?;
    assert_eq!(counts, (0, 0, 0));
    let retained:i64=sqlx::query_scalar("select count(*) from concert_social_bindings b join concert_social_requests r using(event_id,request_id) where $1 in(r.requester_id,r.recipient_id)")
        .bind(f.user).fetch_one(&f.pool).await?;
    assert_eq!(retained, 140);
    let retained_grants: i64 = sqlx::query_scalar(
        "select count(*) from concert_social_grants where $1 in(owner_id,viewer_id)",
    )
    .bind(f.user)
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(retained_grants, 280);
    let tombstones:i64=sqlx::query_scalar("select count(*) from concert_social_handles where user_id=$1 and key_id is null and nonce is null and ciphertext is null and revision=2")
        .bind(f.user).fetch_one(&f.pool).await?;
    assert_eq!(tombstones, 2);
    assert!(f
        .publisher
        .views
        .lock()
        .unwrap()
        .values()
        .all(Option::is_some));
    let views = f.publisher.calls.load(Ordering::SeqCst);
    assert!(f.finish(&command).await?.command == command);
    assert_eq!(f.publisher.calls.load(Ordering::SeqCst), views);
    let ordinary_after:(i64,i64)=sqlx::query_as("select (select count(*) from concert_command_receipts),(select count(*) from concert_outbox)")
        .fetch_one(&f.pool).await?;
    assert_eq!(ordinary_before, ordinary_after);
    let changed=sqlx::query("update concert_social_account_deletions set source_binding=decode(repeat('01',32),'hex'),progress_revision=progress_revision+1 where saga_id=$1")
        .bind(command.saga_id).execute(&f.pool).await;
    assert!(changed.is_err());
    Ok(())
}
#[tokio::test]
#[ignore = "explicit marker/inode-owned native PG16 qualification"]
async fn native_cancellation_keeps_source_pending_and_does_not_ack_empty_registry() -> Result<()> {
    let f = Fixture::new(1, 1).await?;
    let command = actual_delete(&f.auth, f.user).await?;
    f.publisher.hold.store(true, Ordering::SeqCst);
    assert!(tokio::time::timeout(
        WallDuration::from_millis(200),
        owner(&f.pool, &f.auth, f.publisher.clone()).apply(command.clone(), deadline())
    )
    .await
    .is_err());
    f.publisher.hold.store(false, Ordering::SeqCst);
    let empty = Arc::new(Publisher::new([]));
    assert_eq!(
        owner(&f.pool, &f.auth, empty)
            .apply(command.clone(), deadline())
            .await
            .err(),
        Some(DeletionFailure::Incomplete)
    );
    let pending:i64=sqlx::query_scalar("select count(*) from concert_social_deletion_audience where saga_id=$1 and published_at is null")
        .bind(command.saga_id).fetch_one(&f.pool).await?;
    assert!(pending > 0);
    assert!(f.finish(&command).await?.command == command);
    Ok(())
}
#[tokio::test]
#[ignore = "explicit marker/inode-owned native PG16 qualification"]
async fn native_actual_auth_source_rejects_wrong_subject_retention_and_undeleted_user() -> Result<()>
{
    let f = Fixture::new(1, 0).await?;
    let active = WorldDeletionCommand {
        saga_id: Uuid::new_v4(),
        user_id: f.user,
        actor_id: f.user,
        target: WorldDeletionTarget::Sharing,
        retention_until: Utc::now(),
    };
    assert!(owner(&f.pool, &f.auth, f.publisher.clone())
        .apply(active, deadline())
        .await
        .is_err());
    let command = actual_delete(&f.auth, f.user).await?;
    for index in 0..3 {
        let mut other = command.clone();
        match index {
            0 => other.retention_until += chrono::Duration::nanoseconds(1),
            1 => other.target = WorldDeletionTarget::Sessions,
            _ => other.saga_id = Uuid::new_v4(),
        }
        assert!(owner(&f.pool, &f.auth, f.publisher.clone())
            .apply(other, deadline())
            .await
            .is_err());
    }
    let mut wrong = command.clone();
    wrong.actor_id = f.peer;
    assert!(owner(&f.pool, &f.auth, f.publisher.clone())
        .apply(wrong, deadline())
        .await
        .is_err());
    let mut wrong = command.clone();
    wrong.user_id = f.peer;
    assert!(owner(&f.pool, &f.auth, f.publisher.clone())
        .apply(wrong, deadline())
        .await
        .is_err());
    // Auth source loss at the pre-commit recheck must roll the entire first
    // round back, including the common fence and leaf progress.
    let mut disappearing = AuthRepositoryTestTransport::new(f.auth.clone());
    disappearing.reject_after = Some(1);
    let disappearing = SharingDeletion::new(
        f.pool.clone(),
        Arc::new(disappearing),
        SocialInvalidationSink::new(f.pool.clone(), f.publisher.clone()),
    );
    assert_eq!(
        disappearing.apply(command.clone(), deadline()).await.err(),
        Some(DeletionFailure::Unavailable)
    );
    let remaining: i64 = sqlx::query_scalar(
        "select count(*) from concert_world_account_deletion_fences where user_id=$1",
    )
    .bind(f.user)
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(remaining, 0);
    f.finish(&command).await?;
    Ok(())
}
#[tokio::test]
#[ignore = "explicit marker/inode-owned native PG16 qualification"]
async fn native_original_deadline_while_common_user_lock_waits_cannot_create_partial_fence(
) -> Result<()> {
    let f = Fixture::new(1, 1).await?;
    let mut inflight = f.pool.begin().await?;
    lock_user(&mut inflight, f.user).await?;
    let command = actual_delete(&f.auth, f.user).await?;
    let started = Instant::now();
    let result = owner(&f.pool, &f.auth, f.publisher.clone())
        .apply(command.clone(), started + WallDuration::from_millis(150))
        .await;
    assert!(result.is_err());
    assert!(started.elapsed() < WallDuration::from_secs(2));
    let count: i64 = sqlx::query_scalar(
        "select count(*) from concert_social_account_deletions where saga_id=$1",
    )
    .bind(command.saga_id)
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(count, 0);
    inflight.rollback().await?;
    f.finish(&command).await?;
    Ok(())
}
#[tokio::test]
#[ignore = "explicit owned setup before native cases; runs actual SQLx migrators"]
async fn native_initialize_owned_schemas() -> Result<()> {
    let auth = owned_pool("mcr_world_deletion_auth").await?;
    let pool = owned_pool("mcr_world_deletion_concert").await?;
    sqlx::migrate!("../auth-service/migrations")
        .run(&auth)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(())
}
#[tokio::test]
#[ignore = "explicit AFTER_RESTART phase in the same marker/inode-owned PG16"]
async fn native_replay_after_external_physical_postmaster_restart() -> Result<()> {
    anyhow::ensure!(
        std::env::var("BE42_SHARING_PHASE").as_deref() == Ok("AFTER_RESTART"),
        "actual external restart phase required"
    );
    let auth = owned_pool("mcr_world_deletion_auth").await?;
    let pool = owned_pool("mcr_world_deletion_concert").await?;
    let before = std::env::var("BE42_SHARING_BEFORE_POSTMASTER")?;
    let current: String = sqlx::query_scalar("select pg_postmaster_start_time()::text")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(current != before, "physical postmaster did not change");
    let users:Vec<Uuid>=sqlx::query_scalar("select user_id from concert_social_account_deletions where completed_at is not null order by user_id")
        .fetch_all(&pool).await?;
    anyhow::ensure!(!users.is_empty(), "retained actual saga missing");
    for user in users {
        let command = actual_delete(&auth, user).await?;
        let no_views = Arc::new(Publisher::new([]));
        // A retained durable completion can replay; no new publication or empty
        // registry is used to complete an unfinished operation.
        let ack = owner(&pool, &auth, no_views.clone())
            .apply(command.clone(), deadline())
            .await
            .map_err(|_| anyhow::anyhow!("durable replay failed"))?;
        assert!(ack.command == command);
        assert_eq!(no_views.calls.load(Ordering::SeqCst), 0);
    }
    Ok(())
}
