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
use chrono::{TimeDelta, Utc};
use concert_application::world_ticket::*;
use concert_domain::{AuthUserId, EventId, ParticipantKey};
use concert_service::world::{account_deletion::*, PgWorldRepository};
use sha2::{Digest, Sha256};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool, Row,
};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration as WallDuration,
};
use tokio::time::Instant;
use uuid::Uuid;

// Auth test transport/helpers copied from Ticket source 9bbd33e98e4ab6b9240181b2d5311af2121722e9;
// only the target is changed from Tickets to Sessions. No production transport.
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
            || c.target != WorldDeletionTarget::Sessions
            || !source.steps.iter().any(|s| {
                s.target == AuthUserDeletionTargetDto::ConcertSessions
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
        target: WorldDeletionTarget::Sessions,
        actor_id: source.actor_id,
        retention_until: source.retention_until,
    })
}

// Fixture geometry and headless admission setup adapted from the retained
// world_runtime_support files at base89f26e6e; these owners/art are synthetic.
mod world_runtime_support;
use concert_service::{
    adapters::r#in::world_ws::*,
    world::{account_sessions::*, admission_bridge::*, ingress::*, membership::*, runtime::*},
};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Mutex,
    time::Duration,
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
    MaybeTlsStream, WebSocketStream,
};
use world_runtime_support::*;
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(4)
}
mod fixture {
    use super::*;
    use concert_application::bootstrap::{
        BootstrapDefaults, BootstrapRelease, ValidatedProductRelease,
    };
    use concert_service::bootstrap::BootstrapService;
    use std::time::Instant;
    struct Owners {
        fence: Arc<Mutex<()>>,
        generation: Mutex<Uuid>,
        calls: AtomicUsize,
        delay_publish: bool,
        delay_switch: AtomicBool,
    }
    #[async_trait]
    impl LiveWorldOwners for Owners {
        fn publication_fence(&self) -> Arc<Mutex<()>> {
            self.fence.clone()
        }
        fn generation(&self) -> Uuid {
            *self.generation.lock().unwrap()
        }
        async fn current(
            &self,
            subject: &WorldSubject,
            now: chrono::DateTime<Utc>,
        ) -> Result<LiveOwnerView, WorldError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if self.delay_switch.load(Ordering::SeqCst) && call >= 2 {
                tokio::time::sleep(Duration::from_millis(120)).await;
            }
            if self.delay_publish && call == 1 {
                tokio::time::sleep(Duration::from_secs(4)).await;
            }
            Ok(LiveOwnerView {
                subject: subject.clone(),
                checked_at: now,
                expires_at: now + TimeDelta::milliseconds(5000),
                identity: identity(),
            })
        }
        async fn visible(
            &self,
            _: &WorldSubject,
            subjects: &[WorldSubject],
            _: chrono::DateTime<Utc>,
        ) -> Result<BTreeSet<Uuid>, WorldError> {
            Ok(subjects.iter().map(|s| s.key.user_id.uuid()).collect())
        }
    }
    pub(super) struct DatabaseFixture {
        pub(super) pool: PgPool,
        pub(super) bridge: Arc<AdmissionBridge>,
        pub(super) owners: Arc<Owners>,
        pub(super) anchor: ClockAnchor,
        pub(super) event: EventId,
        pub(super) room: Uuid,
        pub(super) target: Uuid,
        pub(super) user: AuthUserId,
        pub(super) session: Uuid,
        pub(super) owner: OwnerIdentity,
        pub(super) started: Instant,
    }
    impl DatabaseFixture {
        pub(super) async fn new(pool: PgPool, user: Uuid) -> anyhow::Result<Self> {
            let event = Uuid::new_v4();
            let room = Uuid::new_v4();
            let target = Uuid::new_v4();
            let session = Uuid::new_v4();
            let owner = OwnerIdentity::new(Uuid::new_v4().to_string(), Uuid::new_v4())?;
            sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Bridge test','OPEN')").bind(event).bind(format!("bridge-{}",event.simple())).execute(&pool).await?;
            sqlx::query(
            "insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')",
        )
        .bind(event)
        .bind(user)
        .execute(&pool)
        .await?;
            sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(event).bind(user).execute(&pool).await?;
            sqlx::query("insert into concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,is_default) values($1,$2,'Current',50,'map_fixture',1,'avatar_fixture',1,true),($1,$3,'Target',50,'map_fixture',1,'avatar_fixture',1,false)").bind(event).bind(room).bind(target).execute(&pool).await?;
            let release = Arc::new(
                ValidatedProductRelease::from_validated(
                    BootstrapRelease::new(
                        BootstrapDefaults {
                            theme: "REVENGE".parse()?,
                            avatar: identity().avatar,
                        },
                        b"{}".to_vec(),
                    )
                    .unwrap(),
                    "a".repeat(64),
                )
                .unwrap(),
            );
            let bootstrap = Arc::new(BootstrapService::new(pool.clone(), release));
            let owners = Arc::new(Owners {
                fence: Arc::new(Mutex::new(())),
                generation: Mutex::new(Uuid::new_v4()),
                calls: AtomicUsize::new(0),
                delay_publish: false,
                delay_switch: AtomicBool::new(false),
            });
            let anchor = ClockAnchor {
                monotonic_ms: 0,
                utc: Utc::now(),
            };
            let started = Instant::now();
            let bridge = AdmissionBridge::new_fixture(
                pool.clone(),
                bootstrap,
                owners.clone(),
                owner.clone(),
                EventId::from_uuid(event),
                BTreeMap::from([(room, id(6)), (target, id(7))]),
                pins(),
                anchor.clone(),
                started,
            )?;
            bridge.claim_rooms().await?;
            Ok(Self {
                pool,
                bridge,
                owners,
                anchor,
                event: EventId::from_uuid(event),
                room,
                target,
                user: AuthUserId::from_uuid(user),
                session,
                owner,
                started,
            })
        }
        pub(super) fn now(&self) -> u64 {
            self.started.elapsed().as_millis() as u64
        }
        pub(super) async fn ticket(
            &self,
            room: Uuid,
            user: AuthUserId,
        ) -> anyhow::Result<(String, AdmissionIntent)> {
            use ring::rand::SecureRandom;
            let mut entropy = [0; 32];
            ring::rand::SystemRandom::new()
                .fill(&mut entropy)
                .map_err(|_| anyhow::anyhow!("test entropy unavailable"))?;
            let ticket = AdmissionTicket::from_entropy(&entropy)?;
            let now = Utc::now();
            let pin = concert_application::world_ticket::ResourcePin {
                event_id: self.event,
                room_id: RoomId::from_uuid(room),
                map: MapPin::new(asset("map_fixture"), concert_domain::Revision::initial()),
                avatar: AvatarPin::new(
                    asset("avatar_fixture"),
                    concert_domain::Revision::initial(),
                ),
            };
            PgWorldRepository::new(self.pool.clone())
                .issue(TicketRecord {
                    digest: ticket.digest(),
                    subject: WorldSubject {
                        key: ParticipantKey {
                            event_id: self.event,
                            user_id: user,
                        },
                        event_id: self.event,
                        session_id: concert_application::identity::SessionId::new(
                            self.session.to_string(),
                        )?,
                    },
                    pin,
                    issued_at: now,
                    expires_at: now + TimeDelta::seconds(20),
                    access_expires_at: self.anchor.utc + TimeDelta::seconds(60),
                })
                .await?;
            let raw = ticket.as_str().to_owned();
            let ClientMessage::Authenticate(intent)=decode(json!({"protocolVersion":"1.0.0","type":"authenticate","ticket":raw,"eventId":self.event.uuid(),"roomId":room,"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1}).to_string().as_bytes()).unwrap() else{unreachable!()};
            Ok((raw, intent))
        }
    }
}
use fixture::*;

type Client = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
// Deterministic external Social metadata only. Actual Slot.policy is armed and
// observed on the native socket, so deletion does not merely clear an empty cache.
struct FixturePolicy;
#[async_trait]
impl policy::HeldPolicyBaseline for FixturePolicy {
    fn revision(&self) -> i64 {
        1
    }
    async fn commit(self: Box<Self>) -> Result<(), policy::PolicyUnavailable> {
        Ok(())
    }
}
#[async_trait]
impl policy::PolicyBaselinePort for FixturePolicy {
    async fn held_baseline(
        &self,
        _: Uuid,
        _: Uuid,
    ) -> Result<Box<dyn policy::HeldPolicyBaseline>, policy::PolicyUnavailable> {
        Ok(Box::new(FixturePolicy))
    }
}
struct Serving {
    state: Arc<WorldSocketState>,
    address: std::net::SocketAddr,
    owner: tokio::task::JoinHandle<ShutdownReport>,
    server: tokio::task::JoinHandle<()>,
}
impl Serving {
    async fn start(f: &DatabaseFixture) -> Result<Self> {
        let transport = SocketTransport::default().with_policy_baseline(Arc::new(FixturePolicy));
        let mut target = spec(f.target, 50);
        target.room_epoch = id(7);
        let runtime = WorldRuntime::new(
            f.event,
            Uuid::parse_str(&f.owner.process_id)?,
            f.anchor.clone(),
            vec![spec(f.room, 50), target],
            f.bridge.clone(),
            f.bridge.clone(),
            transport.clone(),
        )
        .unwrap();
        let (started, origin) = runtime.clock_origin();
        let (handle, owner) = runtime.spawn();
        let state = WorldSocketState::new(
            OriginPolicy::new(vec!["http://localhost:9000".into()], true).unwrap(),
            transport,
            f.bridge.clone(),
            Arc::new(handle),
            started,
            origin,
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let router = router(state.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Ok(Self {
            state,
            address,
            owner,
            server,
        })
    }
    fn leaf(&self, f: &DatabaseFixture) -> SessionOwner {
        SessionOwner::new(f.bridge.clone(), &self.state).unwrap()
    }
    async fn connect(
        &self,
        f: &DatabaseFixture,
        user: AuthUserId,
        room: Uuid,
    ) -> Result<(Client, Value)> {
        let (raw, _) = f.ticket(room, user).await?;
        let mut request = format!("ws://{}/world-ws", self.address).into_client_request()?;
        request
            .headers_mut()
            .insert("origin", "http://localhost:9000".parse()?);
        let (mut client, _) = connect_async(request).await?;
        client.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"authenticate","ticket":raw,"eventId":f.event.uuid(),"roomId":room,"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1}).to_string())).await?;
        let snapshot = value(&mut client).await?;
        anyhow::ensure!(
            snapshot["type"] == "snapshot",
            "actual native initial snapshot missing"
        );
        let baseline = value(&mut client).await?;
        anyhow::ensure!(
            baseline["type"] == "policy.baseline" && baseline["policyRevision"] == 1,
            "actual private policy baseline was not armed before deletion"
        );
        Ok((client, snapshot))
    }
}
impl Drop for Serving {
    fn drop(&mut self) {
        self.state.owner.request_stop();
        self.server.abort();
        self.owner.abort();
    }
}
async fn value(client: &mut Client) -> Result<Value> {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(2), client.next())
            .await?
            .ok_or_else(|| anyhow::anyhow!("socket closed before value"))??;
        if let Message::Text(raw) = frame {
            let value: Value = serde_json::from_str(&raw)?;
            if value["type"] != "ping" {
                return Ok(value);
            }
        }
    }
}
async fn closed(client: &mut Client) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match client.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return Ok(()),
                Some(Ok(_)) => {}
            }
        }
    })
    .await?
}
async fn add_user(f: &DatabaseFixture, user: AuthUserId) -> Result<()> {
    sqlx::query(
        "insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')",
    )
    .bind(f.event.uuid())
    .bind(user.uuid())
    .execute(&f.pool)
    .await?;
    sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)")
        .bind(f.event.uuid()).bind(user.uuid()).execute(&f.pool).await?;
    Ok(())
}
async fn completed(pool: &PgPool, user: Uuid) -> Result<bool> {
    Ok(sqlx::query_scalar("select coalesce((select completed_at is not null from concert_world_session_deletion_progress where user_id=$1),false)")
        .bind(user).fetch_one(pool).await?)
}
#[tokio::test]
async fn empty_registry_has_no_positive_default() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://unused@127.0.0.1:1/unused")
        .unwrap();
    assert!(matches!(
        SessionDeletion::new(
            pool.clone(),
            Arc::new(AuthRepositoryTestTransport::new(pool)),
            vec![]
        ),
        Err(DeletionFailure::Unavailable)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires fresh marker-owned Auth/Concert PG16 and explicit resource allocation"]
async fn native_sessions_actual_auth_active_pending_multievent_and_exact_retry() -> Result<()> {
    let auth = owned_pool("mcr_world_deletion_auth").await?;
    let pool = owned_pool("mcr_world_deletion_concert").await?;
    let user = actual_auth_user(&auth).await?;
    let other = AuthUserId::from_uuid(Uuid::new_v4());
    let f = DatabaseFixture::new(pool.clone(), user).await?;
    let second = DatabaseFixture::new(pool.clone(), user).await?;
    let pending = DatabaseFixture::new(pool.clone(), user).await?;
    let serving = Serving::start(&f).await?;
    let serving2 = Serving::start(&second).await?;
    let serving3 = Serving::start(&pending).await?;
    add_user(&f, other).await?;
    let (mut victim, _) = serving.connect(&f, f.user, f.room).await?;
    let (mut victim2, _) = serving2.connect(&second, second.user, second.room).await?;
    let (mut survivor, initial) = serving.connect(&f, other, f.room).await?;
    let (_, intent) = pending.ticket(pending.room, pending.user).await?;
    let delayed = pending
        .bridge
        .admit(intent, Uuid::new_v4())
        .await
        .map_err(|_| anyhow::anyhow!("pending admission failed"))?;
    let before: (Uuid,chrono::DateTime<Utc>)=sqlx::query_as("select connection_id,access_expires_at from concert_world_memberships where event_id=$1 and user_id=$2")
        .bind(f.event.uuid()).bind(other.uuid()).fetch_one(&pool).await?;
    let (_, old_ticket) = f.ticket(f.room, f.user).await?;
    let command = actual_delete(&auth, user).await?;
    let deletion = SessionDeletion::new(
        pool.clone(),
        Arc::new(AuthRepositoryTestTransport::new(auth.clone())),
        vec![
            serving.leaf(&f),
            serving2.leaf(&second),
            serving3.leaf(&pending),
        ],
    )
    .unwrap();
    let receipt = deletion.apply(command.clone(), deadline()).await.unwrap();
    assert_eq!(receipt.command, command);
    assert_eq!(receipt.fence_revision, 1);
    assert!(completed(&pool, user).await?);
    closed(&mut victim).await?;
    closed(&mut victim2).await?;
    let (reply, answer) = tokio::sync::oneshot::channel();
    serving3
        .state
        .owner
        .try_submit(Command::Install {
            binding: delayed.clone(),
            cause: concert_service::world::replication::SnapshotCause::Initial,
            reply,
        })
        .unwrap();
    assert!(
        answer.await?.is_err(),
        "removed pending view cannot install after ACK"
    );
    assert!(pending
        .bridge
        .resync(&delayed, Uuid::new_v4())
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from concert_world_memberships where user_id=$1"
        )
        .bind(user)
        .fetch_one(&pool)
        .await?,
        0
    );
    let after:(Uuid,chrono::DateTime<Utc>)=sqlx::query_as("select connection_id,access_expires_at from concert_world_memberships where event_id=$1 and user_id=$2")
        .bind(f.event.uuid()).bind(other.uuid()).fetch_one(&pool).await?;
    assert_eq!(
        before, after,
        "unrelated membership and captured deadline retained"
    );
    let next = value(&mut survivor).await?;
    assert_eq!(
        initial["streamId"], next["streamId"],
        "unrelated stream generation retained"
    );
    let stored: Value = sqlx::query_scalar(
        "select to_jsonb(p) from concert_world_session_deletion_progress p where user_id=$1",
    )
    .bind(user)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        deletion.apply(command.clone(), deadline()).await.unwrap(),
        receipt
    );
    let replayed: Value = sqlx::query_scalar(
        "select to_jsonb(p) from concert_world_session_deletion_progress p where user_id=$1",
    )
    .bind(user)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        stored, replayed,
        "completed receipt is byte-stable on exact retry"
    );
    let mut rebound = command.clone();
    rebound.retention_until += TimeDelta::nanoseconds(1);
    assert_eq!(
        deletion.apply(rebound, deadline()).await.err(),
        Some(DeletionFailure::InvalidCommand)
    );
    assert!(
        f.ticket(f.room, f.user).await.is_err(),
        "permanent common fence denies fresh tickets"
    );
    assert!(
        f.bridge.admit(old_ticket, Uuid::new_v4()).await.is_err(),
        "a pre-deletion unconsumed ticket cannot reconnect"
    );
    assert!(sqlx::query("insert into concert_world_memberships(event_id,user_id,room_id,session_id,connection_id,owner_process_id,owner_epoch,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,admitted_at,lease_until,access_expires_at) values($1,$2,$3,$4,$5,$6,$7,'map_fixture',1,'avatar_fixture',1,clock_timestamp(),clock_timestamp()+interval '5 seconds',clock_timestamp()+interval '60 seconds')")
        .bind(f.event.uuid()).bind(user).bind(f.room).bind(Uuid::new_v4()).bind(Uuid::new_v4())
        .bind(&f.owner.process_id).bind(f.owner.epoch).execute(&pool).await.is_err(),
        "the membership guard itself denies bypassing the ticket boundary");
    assert!(sqlx::query("update concert_world_session_deletion_progress set progress_revision=progress_revision+1 where user_id=$1").bind(user).execute(&pool).await.is_err());
    assert!(
        sqlx::query("delete from concert_world_session_deletion_progress where user_id=$1")
            .bind(user)
            .execute(&pool)
            .await
            .is_err()
    );
    drop(deletion);
    drop(serving);
    drop(serving2);
    drop(serving3);
    pool.close().await;
    auth.close().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires fresh marker-owned Auth/Concert PG16 and explicit resource allocation"]
async fn native_sessions_foreign_owner_unavailable_and_original_deadline_remain_retryable(
) -> Result<()> {
    let auth = owned_pool("mcr_world_deletion_auth").await?;
    let pool = owned_pool("mcr_world_deletion_concert").await?;
    let user = actual_auth_user(&auth).await?;
    let f = DatabaseFixture::new(pool.clone(), user).await?;
    let foreign = DatabaseFixture::new(pool.clone(), user).await?;
    let serving = Serving::start(&f).await?;
    let (_, intent) = foreign.ticket(foreign.room, foreign.user).await?;
    foreign
        .bridge
        .admit(intent, Uuid::new_v4())
        .await
        .map_err(|_| anyhow::anyhow!("foreign pending failed"))?;
    let command = actual_delete(&auth, user).await?;
    let deletion = SessionDeletion::new(
        pool.clone(),
        Arc::new(AuthRepositoryTestTransport::new(auth.clone())),
        vec![serving.leaf(&f)],
    )
    .unwrap();
    assert_eq!(
        deletion.apply(command.clone(), deadline()).await.err(),
        Some(DeletionFailure::Incomplete)
    );
    assert!(!completed(&pool, user).await?);
    let foreign_rows: i64 =
        sqlx::query_scalar("select count(*) from concert_world_memberships where user_id=$1")
            .bind(user)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        foreign_rows, 1,
        "known foreign victim evidence cannot be erased"
    );
    assert_eq!(
        deletion.apply(command.clone(), Instant::now()).await.err(),
        Some(DeletionFailure::Unavailable)
    );
    // Retry with the actual additional publication leg, not an empty registry.
    let foreign_serving = Serving::start(&foreign).await?;
    let fixed = SessionDeletion::new(
        pool.clone(),
        Arc::new(AuthRepositoryTestTransport::new(auth.clone())),
        vec![serving.leaf(&f), foreign_serving.leaf(&foreign)],
    )
    .unwrap();
    serving.state.owner.request_stop();
    tokio::time::sleep(Duration::from_millis(25)).await;
    assert_eq!(
        fixed.apply(command.clone(), deadline()).await.err(),
        Some(DeletionFailure::Unavailable)
    );
    assert!(
        !completed(&pool, user).await?,
        "unavailable native owner is never acknowledged"
    );
    drop(fixed);
    drop(deletion);
    drop(serving);
    tokio::time::sleep(Duration::from_millis(25)).await;
    let repaired = Serving::start(&f).await?;
    let resumed = SessionDeletion::new(
        pool.clone(),
        Arc::new(AuthRepositoryTestTransport::new(auth.clone())),
        vec![repaired.leaf(&f), foreign_serving.leaf(&foreign)],
    )
    .unwrap();
    assert_eq!(
        resumed
            .apply(command, deadline())
            .await
            .unwrap()
            .fence_revision,
        1
    );
    assert!(
        completed(&pool, user).await?,
        "a repaired actual owner completes the retained operation"
    );
    drop(resumed);
    drop(repaired);
    drop(foreign_serving);
    pool.close().await;
    auth.close().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires fresh marker-owned Auth/Concert PG16 and explicit resource allocation"]
async fn native_sessions_bounded_persistence_and_lock_deadline() -> Result<()> {
    let auth = owned_pool("mcr_world_deletion_auth").await?;
    let pool = owned_pool("mcr_world_deletion_concert").await?;
    let user = actual_auth_user(&auth).await?;
    let f = DatabaseFixture::new(pool.clone(), user).await?;
    let serving = Serving::start(&f).await?;
    // Expired foreign rows have no remaining publication authority. These are
    // metadata fixtures, separate from the actual socket proof above.
    for _ in 0..129 {
        let event = Uuid::new_v4();
        let room = Uuid::new_v4();
        sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Expired session fixture','OPEN')")
            .bind(event).bind(format!("expired-{}",event.simple())).execute(&pool).await?;
        sqlx::query("insert into concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,is_default) values($1,$2,'Expired',50,'map_fixture',1,'avatar_fixture',1,true)")
            .bind(event).bind(room).execute(&pool).await?;
        sqlx::query("insert into concert_world_memberships(event_id,user_id,room_id,session_id,connection_id,owner_process_id,owner_epoch,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,admitted_at,lease_until,access_expires_at) values($1,$2,$3,$4,$5,$6,$7,'map_fixture',1,'avatar_fixture',1,clock_timestamp()-interval '20 seconds',clock_timestamp()-interval '10 seconds',clock_timestamp()-interval '5 seconds')")
            .bind(event).bind(user).bind(room).bind(Uuid::new_v4()).bind(Uuid::new_v4())
            .bind(Uuid::new_v4().to_string()).bind(Uuid::new_v4()).execute(&pool).await?;
    }
    let command = actual_delete(&auth, user).await?;
    let authority = Arc::new(AuthRepositoryTestTransport::new(auth.clone()));
    let deletion =
        SessionDeletion::new(pool.clone(), authority.clone(), vec![serving.leaf(&f)]).unwrap();
    let mut lock = pool.begin().await?;
    concert_service::world::account_deletion::lock_user(&mut lock, user).await?;
    let began = Instant::now();
    assert_eq!(
        deletion
            .apply(command.clone(), began + Duration::from_millis(100))
            .await
            .err(),
        Some(DeletionFailure::Unavailable)
    );
    assert!(
        began.elapsed() < Duration::from_millis(750),
        "original dispatch deadline was renewed"
    );
    lock.rollback().await?;
    assert!(!completed(&pool, user).await?);
    assert_eq!(
        deletion.apply(command.clone(), deadline()).await.err(),
        Some(DeletionFailure::Incomplete)
    );
    let left: i64 =
        sqlx::query_scalar("select count(*) from concert_world_memberships where user_id=$1")
            .bind(user)
            .fetch_one(&pool)
            .await?;
    assert_eq!(left, 1, "one bounded 128-row SQL batch per attempt");
    assert!(!completed(&pool, user).await?);
    drop(deletion);
    // Recreate the dispatcher: progress comes from durable SQL, not a job flag.
    let resumed = SessionDeletion::new(pool.clone(), authority, vec![serving.leaf(&f)]).unwrap();
    assert_eq!(
        resumed
            .apply(command, deadline())
            .await
            .unwrap()
            .fence_revision,
        1
    );
    let removed: i64 = sqlx::query_scalar(
        "select removed_count from concert_world_session_deletion_progress where user_id=$1",
    )
    .bind(user)
    .fetch_one(&pool)
    .await?;
    assert_eq!(removed, 129);
    drop(resumed);
    drop(serving);
    pool.close().await;
    auth.close().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "run only after actual postmaster restart in the owned native harness"]
async fn native_sessions_postmaster_restart_exact_receipts() -> Result<()> {
    let auth = owned_pool("mcr_world_deletion_auth").await?;
    let pool = owned_pool("mcr_world_deletion_concert").await?;
    let users:Vec<Uuid>=sqlx::query_scalar("select user_id from concert_world_session_deletion_progress where completed_at is not null order by user_id").fetch_all(&pool).await?;
    anyhow::ensure!(
        users.len() >= 2,
        "missing prior completed native operations"
    );
    for user in users {
        let source = PgUserDeletionSagaRepository::new(auth.clone())
            .find_by_user_id(user)
            .await?
            .unwrap();
        let command = WorldDeletionCommand {
            saga_id: source.id,
            user_id: user,
            target: WorldDeletionTarget::Sessions,
            actor_id: source.actor_id,
            retention_until: source.retention_until,
        };
        let stored: Value = sqlx::query_scalar(
            "select to_jsonb(p) from concert_world_session_deletion_progress p where user_id=$1",
        )
        .bind(user)
        .fetch_one(&pool)
        .await?;
        let f = DatabaseFixture::new(pool.clone(), user).await?;
        let serving = Serving::start(&f).await?;
        let deletion = SessionDeletion::new(
            pool.clone(),
            Arc::new(AuthRepositoryTestTransport::new(auth.clone())),
            vec![serving.leaf(&f)],
        )
        .unwrap();
        assert_eq!(
            deletion
                .apply(command, deadline())
                .await
                .unwrap()
                .fence_revision,
            1
        );
        let replayed: Value = sqlx::query_scalar(
            "select to_jsonb(p) from concert_world_session_deletion_progress p where user_id=$1",
        )
        .bind(user)
        .fetch_one(&pool)
        .await?;
        assert_eq!(stored, replayed);
        assert!(
            f.ticket(f.room, f.user).await.is_err(),
            "restart cannot reopen a deleted account"
        );
        drop(deletion);
        drop(serving);
    }
    pool.close().await;
    auth.close().await;
    Ok(())
}
