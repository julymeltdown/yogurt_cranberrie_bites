//! Reuses marker/inode ownership checks from committed world_admission.rs;
//! external live owners below are synthetic. Real DB is socket-only and isolated.
use super::*;
use concert_application::{
    bootstrap::{BootstrapDefaults, BootstrapRelease, ValidatedProductRelease},
    world_ticket::*,
};
use concert_service::{
    bootstrap::BootstrapService,
    world::{admission_bridge::*, PgWorldRepository},
};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::time::Instant;
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    str::FromStr,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Duration as StdDuration,
};
async fn owned_pool() -> anyhow::Result<PgPool> {
    let root = PathBuf::from(std::env::var("WORLD_TEST_OWNER_ROOT")?);
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        info.is_dir() && !info.file_type().is_symlink() && info.permissions().mode() & 0o077 == 0,
        "private owned root required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino()) == std::env::var("WORLD_TEST_OWNER_IDENTITY")?,
        "fixture identity changed"
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
        marker["token"].as_str() == Some(&std::env::var("WORLD_TEST_OWNER_TOKEN")?),
        "fixture owner mismatch"
    );
    let options = PgConnectOptions::from_str(&std::env::var("WORLD_TEST_DATABASE_URL")?)
        .map_err(|_| anyhow::anyhow!("invalid owned fixture options"))?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_world")
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(StdDuration::from_secs(3))
        .connect_with(options)
        .await?;
    let actual: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        actual == root.join("data").to_string_lossy(),
        "foreign data directory refused"
    );
    Ok(pool)
}

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
struct DatabaseFixture {
    pool: PgPool,
    bridge: Arc<AdmissionBridge>,
    owners: Arc<Owners>,
    anchor: ClockAnchor,
    event: EventId,
    room: Uuid,
    target: Uuid,
    user: AuthUserId,
    session: Uuid,
    owner: OwnerIdentity,
    started: Instant,
}
impl DatabaseFixture {
    async fn new(delay_publish: bool) -> anyhow::Result<Self> {
        let pool = owned_pool().await?;
        let event = Uuid::new_v4();
        let user = Uuid::new_v4();
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
        sqlx::query("insert into concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,is_default) values($1,$2,'Current',1,'map_fixture',1,'avatar_fixture',1,true),($1,$3,'Target',1,'map_fixture',1,'avatar_fixture',1,false)").bind(event).bind(room).bind(target).execute(&pool).await?;
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
            delay_publish,
            delay_switch: AtomicBool::new(false),
        });
        let anchor = ClockAnchor {
            monotonic_ms: 0,
            utc: Utc::now(),
        };
        let started = Instant::now();
        let bridge = AdmissionBridge::new(
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
    fn now(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }
    async fn ticket(
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
            avatar: AvatarPin::new(asset("avatar_fixture"), concert_domain::Revision::initial()),
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
#[tokio::test]
#[ignore = "requires world_runtime_support/bridge_native.py marker-owned PostgreSQL"]
async fn native_database_bridge_consumption_atomic_resync_owner_fences_and_cleanup(
) -> anyhow::Result<()> {
    let f = DatabaseFixture::new(false).await?;
    let (_secret, intent) = f.ticket(f.room, f.user).await?;
    let connection = Uuid::new_v4();
    let b = f
        .bridge
        .admit(intent.clone(), connection)
        .await
        .map_err(|_| anyhow::anyhow!("initial admission failed"))?;
    assert!(
        CanonicalAuthorityPort::current(&f.bridge, &b, f.now()).is_err(),
        "pending grant must not change active canonical view before owner install"
    );
    let mut adapter = MembershipAdapter::new(f.bridge.clone(), f.anchor.clone(), 50).unwrap();
    adapter.install(&b, f.now()).unwrap();
    assert!(f.bridge.admit(intent, Uuid::new_v4()).await.is_err());
    let current = PgWorldRepository::new(f.pool.clone())
        .current_owned_membership(b.key, Utc::now())
        .await?
        .unwrap();
    let expiry = current.membership.access_expires_at;
    let resync = f
        .bridge
        .resync(&b, Uuid::new_v4())
        .await
        .map_err(|_| anyhow::anyhow!("resync failed"))?;
    assert!(CanonicalAuthorityPort::current(&f.bridge, &b, f.now()).is_ok());
    assert!(CanonicalAuthorityPort::current(&f.bridge, &resync, f.now()).is_err());
    adapter.install(&resync, f.now()).unwrap();
    assert!(CanonicalAuthorityPort::release(&f.bridge, &b).is_ok());
    assert_eq!(f.bridge.cleanup().await?, 0);
    let kept = PgWorldRepository::new(f.pool.clone())
        .current_owned_membership(b.key, Utc::now())
        .await?
        .unwrap();
    assert_eq!(kept.membership.connection_id.uuid(), connection);
    assert_eq!(kept.membership.access_expires_at, expiry);
    let (_, mut wrong) = f.ticket(f.room, f.user).await?;
    wrong.map_revision = concert_domain::Revision::new(2)?;
    assert!(f.bridge.admit(wrong.clone(), Uuid::new_v4()).await.is_err());
    wrong.map_revision = concert_domain::Revision::initial();
    assert!(
        f.bridge.admit(wrong, Uuid::new_v4()).await.is_err(),
        "wrong pin denial consumed the ticket"
    );
    f.bridge.maintain().await?;
    let renewed = PgWorldRepository::new(f.pool.clone())
        .current_owned_membership(b.key, Utc::now())
        .await?
        .unwrap();
    assert_eq!(renewed.membership.access_expires_at, expiry);
    let (_, takeover) = f.ticket(f.room, f.user).await?;
    let newest = f
        .bridge
        .admit(takeover, Uuid::new_v4())
        .await
        .map_err(|_| anyhow::anyhow!("takeover failed"))?;
    adapter.install(&newest, f.now()).unwrap();
    let _ = CanonicalAuthorityPort::release(&f.bridge, &resync);
    assert_eq!(f.bridge.cleanup().await?, 0);
    assert_eq!(
        PgWorldRepository::new(f.pool.clone())
            .current_owned_membership(newest.key, Utc::now())
            .await?
            .unwrap()
            .membership
            .connection_id
            .uuid(),
        newest.connection_id
    );
    {
        let _f = f.owners.fence.lock().unwrap();
        *f.owners.generation.lock().unwrap() = Uuid::new_v4();
    }
    assert!(CanonicalAuthorityPort::current(&f.bridge, &resync, f.now()).is_err());
    f.bridge.disconnected(&newest).await;
    assert_eq!(f.bridge.cleanup().await?, 0);
    assert!(PgWorldRepository::new(f.pool.clone())
        .current_owned_membership(b.key, Utc::now())
        .await?
        .is_none());
    f.pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "requires world_runtime_support/bridge_native.py marker-owned PostgreSQL"]
async fn native_database_bridge_cancelled_postcommit_admission_retains_cleanup(
) -> anyhow::Result<()> {
    let f = DatabaseFixture::new(true).await?;
    let (_, intent) = f.ticket(f.room, f.user).await?;
    assert!(tokio::time::timeout(
        Duration::from_millis(100),
        f.bridge.admit(intent, Uuid::new_v4())
    )
    .await
    .is_err());
    let key = ParticipantKey {
        event_id: f.event,
        user_id: f.user,
    };
    assert!(
        PgWorldRepository::new(f.pool.clone())
            .current_owned_membership(key, Utc::now())
            .await?
            .is_some(),
        "cancel exercised after durable admission"
    );
    assert_eq!(f.bridge.cleanup().await?, 0);
    assert!(PgWorldRepository::new(f.pool.clone())
        .current_owned_membership(key, Utc::now())
        .await?
        .is_none());
    f.pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "requires world_runtime_support/bridge_native.py marker-owned PostgreSQL"]
async fn native_database_bridge_full_target_rejection_preserves_current_membership(
) -> anyhow::Result<()> {
    let f = DatabaseFixture::new(false).await?;
    let (_, intent) = f.ticket(f.room, f.user).await?;
    let b = f
        .bridge
        .admit(intent, Uuid::new_v4())
        .await
        .map_err(|_| anyhow::anyhow!("initial admission"))?;
    let mut adapter = MembershipAdapter::new(f.bridge.clone(), f.anchor.clone(), 50).unwrap();
    adapter.install(&b, f.now()).unwrap();
    // Issue first, then let a different user take the final target slot.
    let (_, switch) = f.ticket(f.target, f.user).await?;
    let (raw, other) = f
        .ticket(f.target, AuthUserId::from_uuid(Uuid::new_v4()))
        .await?;
    let repo = PgWorldRepository::new(f.pool.clone());
    let requested = concert_application::world_ticket::ResourcePin {
        event_id: f.event,
        room_id: RoomId::from_uuid(f.target),
        map: MapPin::new(other.map_id, other.map_revision),
        avatar: AvatarPin::new(other.avatar_manifest_id, other.avatar_manifest_version),
    };
    let ConsumeOutcome::Consumed(claim) = repo
        .consume(ConsumeClaim {
            digest: AdmissionTicket::from_secret(raw)?.digest(),
            connection_id: ConnectionId::from_uuid(Uuid::new_v4())?,
            requested,
            now: Utc::now(),
        })
        .await?
    else {
        panic!("fixture claim")
    };
    assert!(matches!(
        repo.admit(&f.owner, &claim, Utc::now()).await?,
        AdmissionOutcome::Admitted(_)
    ));
    assert!(matches!(
        f.bridge.switch(&b, switch.clone(), Uuid::new_v4()).await,
        Err(SwitchFailure::Rejected(SwitchRejection::RoomFull))
    ));
    assert!(CanonicalAuthorityPort::current(&f.bridge, &b, f.now()).is_ok());
    assert!(matches!(
        f.bridge.switch(&b, switch, Uuid::new_v4()).await,
        Err(SwitchFailure::Rejected(SwitchRejection::TicketInvalid))
    ));
    let current = repo
        .current_owned_membership(b.key, Utc::now())
        .await?
        .unwrap();
    assert_eq!(current.membership.room_id.uuid(), f.room);
    assert_eq!(current.membership.connection_id.uuid(), b.connection_id);
    f.bridge.disconnected(&b).await;
    f.pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "requires world_runtime_support/bridge_native.py marker-owned PostgreSQL"]
async fn native_database_bridge_successful_socket_switch_spans_owner_ticks() -> anyhow::Result<()> {
    let f = DatabaseFixture::new(false).await?;
    let mut target = spec(f.target, 1);
    target.room_epoch = id(7);
    let transport = SocketTransport::default();
    let runtime = WorldRuntime::new(
        f.event,
        Uuid::parse_str(&f.owner.process_id)?,
        f.anchor.clone(),
        vec![spec(f.room, 1), target],
        f.bridge.clone(),
        f.bridge.clone(),
        transport.clone(),
    )
    .unwrap();
    let (started, origin_ms) = runtime.clock_origin();
    let (handle, owner) = runtime.spawn();
    let handle = Arc::new(handle);
    let state = WorldSocketState::new(
        OriginPolicy::new(vec!["http://localhost:9000".into()], true).unwrap(),
        transport.clone(),
        f.bridge.clone(),
        handle.clone(),
        started,
        origin_ms,
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        axum::serve(listener, router(state))
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await
            .unwrap()
    });
    let mut request = format!("ws://{address}/world-ws").into_client_request()?;
    request
        .headers_mut()
        .insert("origin", "http://localhost:9000".parse()?);
    let (mut client, _) = connect_async(request).await?;
    let (raw, _) = f.ticket(f.room, f.user).await?;
    client.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"authenticate","ticket":raw,"eventId":f.event.uuid(),"roomId":f.room,"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1}).to_string())).await?;
    let initial = value(&mut client).await;
    let (raw, _) = f.ticket(f.target, f.user).await?;
    let request_id = Uuid::new_v4();
    f.owners.delay_switch.store(true, Ordering::SeqCst);
    client.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"switchRoom","requestId":request_id,"streamId":initial["streamId"],"ticket":raw,"eventId":f.event.uuid(),"roomId":f.target,"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1}).to_string())).await?;
    let mut switched = false;
    for _ in 0..8 {
        let frame = value(&mut client).await;
        if frame["type"] == "snapshot" {
            assert_eq!(frame["roomId"], f.target.to_string());
            assert_eq!(frame["cause"], "ROOM_SWITCH");
            assert_eq!(frame["requestId"], request_id.to_string());
            assert_ne!(frame["streamId"], initial["streamId"]);
            assert_eq!(frame["streamSeq"], 0);
            assert_eq!(frame["accessExpiresAt"], initial["accessExpiresAt"]);
            switched = true;
            break;
        }
    }
    assert!(switched);
    let current = PgWorldRepository::new(f.pool.clone())
        .current_owned_membership(
            ParticipantKey {
                event_id: f.event,
                user_id: f.user,
            },
            Utc::now(),
        )
        .await?
        .unwrap();
    assert_eq!(current.membership.room_id.uuid(), f.target);
    client.close(None).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    f.bridge.cleanup().await?;
    transport.close_all(SafeClose::UNAVAILABLE);
    handle.request_stop();
    let report = tokio::time::timeout(Duration::from_secs(2), owner).await??;
    for b in report.remaining_cleanup {
        let _ = CanonicalAuthorityPort::release(&f.bridge, &b);
    }
    assert_eq!(f.bridge.cleanup().await?, 0);
    let _ = stop.send(());
    tokio::time::timeout(Duration::from_secs(2), server).await??;
    f.pool.close().await;
    Ok(())
}
