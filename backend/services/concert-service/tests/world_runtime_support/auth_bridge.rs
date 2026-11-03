//! Actual production bridge + sealed signed Auth read + isolated owning PG.
//! Profile, recipient and artwork remain synthetic; this is not IN01.
use super::*;
use crate::adapters::out::relationship_authority::world_auth::GrpcWorldAuthReader;
use crate::world::{
    ingress::{decode, ClientMessage},
    interest::Facing,
    membership::MembershipAdapter,
};
use concert_application::{
    bootstrap::{BootstrapDefaults, BootstrapRelease, ValidatedProductRelease},
    identity::SessionId,
};
use concert_domain::{AssetId, AuthUserId, AvatarSelection, ParticipantKey, Revision};
use platform::{
    actor_delegation::{
        relationship_read::{RelationshipReadSigner, RelationshipReadVerifier},
        ActorDelegationSigner, ActorDelegationVerifier,
    },
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, Workload},
};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::{os::unix::fs::MetadataExt, path::PathBuf, str::FromStr};
use tokio::sync::oneshot;

// Copied without weakening marker/inode/database checks from the qualified
// Auth producer native fixture at 30ca2a499edabb75720273a312d2a4196f18201d.
async fn pool(owner: &str) -> anyhow::Result<PgPool> {
    anyhow::ensure!(matches!(owner, "auth" | "concert"), "foreign owner refused");
    let root = PathBuf::from(std::env::var("MCR_WORLD_AUTH_ROOT")?);
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        root.is_absolute()
            && root.canonicalize()? == root
            && info.is_dir()
            && info.mode() & 0o077 == 0,
        "private owned fixture required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino()) == std::env::var("MCR_WORLD_AUTH_IDENTITY")?,
        "fixture inode changed"
    );
    let marker = root.join("mcr-owner.json");
    let mi = std::fs::symlink_metadata(&marker)?;
    anyhow::ensure!(
        mi.is_file()
            && !mi.file_type().is_symlink()
            && mi.mode() & 0o077 == 0
            && mi.uid() == info.uid(),
        "private marker required"
    );
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(marker)?)?;
    anyhow::ensure!(
        value["uid"].as_u64() == Some(info.uid() as u64)
            && value["token"].as_str() == Some(std::env::var("MCR_WORLD_AUTH_TOKEN")?.as_str()),
        "fixture owner mismatch"
    );
    for name in ["data", "socket"] {
        let child = root.join(name);
        let ci = std::fs::symlink_metadata(&child)?;
        anyhow::ensure!(
            ci.is_dir() && !ci.file_type().is_symlink() && child.canonicalize()? == child,
            "foreign fixture child refused"
        );
    }
    let options = PgConnectOptions::from_str(&std::env::var(format!(
        "MCR_WORLD_AUTH_{}_URL",
        owner.to_uppercase()
    ))?)?;
    let database = if owner == "concert" {
        "mcr_concert_world_auth".to_owned()
    } else {
        "mcr_world_auth_auth".to_owned()
    };
    anyhow::ensure!(
        options.get_database() == Some(database.as_str())
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_options().is_none()
            && options.get_port() == 5432,
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await?;
    let data: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        data == root.join("data").to_string_lossy(),
        "foreign data directory refused"
    );
    let identity: (String, String, String, i32) = sqlx::query_as(
        "select current_database(),current_user,current_setting('listen_addresses'),current_setting('server_version_num')::int"
    ).fetch_one(&pool).await?;
    anyhow::ensure!(
        identity == (database, "mcr_owner".into(), "".into(), 160014),
        "foreign server refused"
    );
    Ok(pool)
}

fn gateway(audience: Workload) -> ActorDelegationVerifier {
    ActorDelegationVerifier::new(
        audience,
        &[
            ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32])
                .unwrap()
                .public_key(),
        ],
    )
    .unwrap()
}
fn signer() -> RelationshipReadSigner {
    RelationshipReadSigner::from_seed(&[37; 32], &gateway(Workload::Concert)).unwrap()
}
struct Running {
    endpoint: String,
    stop: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}
impl Running {
    async fn finish(mut self) {
        let _ = self.stop.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(2), &mut self.task)
            .await
            .unwrap()
            .unwrap();
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.task.abort();
    }
}
async fn server(pool: PgPool) -> Running {
    use auth_service::adapters::r#in::grpc::concert_relationship::{
        proto, RelationshipServer, POLICY,
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let verifier = RelationshipReadVerifier::new(
        Workload::Auth,
        &[signer().public_key()],
        &gateway(Workload::Auth),
    )
    .unwrap()
    .into_inner();
    let guarded = platform::grpc_method_policy::guard(
        &POLICY,
        proto::auth_concert_relationship_service_server::AuthConcertRelationshipServiceServer::new(
            RelationshipServer::from_pool(pool),
        ),
        CallerRegistry::new()
            .register(Workload::Concert, "world-auth-fixture-concert")
            .unwrap(),
        verifier,
    )
    .unwrap();
    let (stop, rx) = oneshot::channel();
    let incoming = futures::stream::unfold(listener, |listener| async move {
        Some((listener.accept().await.map(|(s, _)| s), listener))
    });
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .layer(CallDeadlineLayer)
            .add_service(guarded)
            .serve_with_incoming_shutdown(incoming, async {
                let _ = rx.await;
            })
            .await
            .unwrap();
    });
    Running {
        endpoint,
        stop: Some(stop),
        task,
    }
}
fn asset(s: &str) -> AssetId {
    AssetId::new(s.into()).unwrap()
}
fn pins() -> ResourcePin {
    ResourcePin {
        map_id: asset("map_fixture"),
        map_revision: Revision::initial(),
        avatar_manifest_id: asset("avatar_fixture"),
        avatar_manifest_version: Revision::initial(),
    }
}
fn identity() -> PublicIdentity {
    PublicIdentity {
        display_name: "Synthetic Profile".into(),
        avatar_revision: Revision::initial(),
        avatar: AvatarSelection {
            manifest_id: asset("avatar_fixture"),
            manifest_version: Revision::initial(),
            body: asset("body_fixture"),
            eyes: asset("eyes_fixture"),
            hair: asset("hair_fixture"),
            outfit: asset("outfit_fixture"),
            accessory: None,
            skin_tone: asset("tone_fixture"),
        },
        facing: Facing::Down,
        moving: false,
    }
}
struct ProfileFixture {
    fence: Arc<Mutex<()>>,
    generation: Uuid,
}
#[async_trait]
impl LiveWorldOwners for ProfileFixture {
    fn publication_fence(&self) -> Arc<Mutex<()>> {
        self.fence.clone()
    }
    fn generation(&self) -> Uuid {
        self.generation
    }
    async fn current(
        &self,
        subject: &WorldSubject,
        now: DateTime<Utc>,
    ) -> Result<LiveOwnerView, WorldError> {
        Ok(LiveOwnerView {
            subject: subject.clone(),
            checked_at: now,
            expires_at: now + TimeDelta::seconds(5),
            identity: identity(),
        })
    }
    async fn visible(
        &self,
        _: &WorldSubject,
        subjects: &[WorldSubject],
        _: DateTime<Utc>,
    ) -> Result<BTreeSet<Uuid>, WorldError> {
        Ok(subjects.iter().map(|s| s.key.user_id.uuid()).collect())
    }
}
async fn ticket(
    bridge: &Arc<AdmissionBridge>,
    subject: &WorldSubject,
    room: Uuid,
    expiry: DateTime<Utc>,
    seed: u8,
) -> anyhow::Result<AdmissionIntent> {
    let ticket = AdmissionTicket::from_entropy(&[seed; 32])?;
    let now = Utc::now();
    assert_eq!(
        bridge
            .repository
            .issue(TicketRecord {
                digest: ticket.digest(),
                subject: subject.clone(),
                pin: concert_application::world_ticket::ResourcePin {
                    event_id: subject.event_id,
                    room_id: RoomId::from_uuid(room),
                    map: MapPin::new(asset("map_fixture"), Revision::initial()),
                    avatar: AvatarPin::new(asset("avatar_fixture"), Revision::initial())
                },
                issued_at: now,
                expires_at: (now + TimeDelta::seconds(20)).min(expiry),
                access_expires_at: expiry
            })
            .await?,
        IssueOutcome::Issued
    );
    let raw=serde_json::json!({"protocolVersion":"1.0.0","type":"authenticate","ticket":ticket.as_str(),"eventId":subject.event_id.uuid(),"roomId":room,"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1}).to_string();
    let ClientMessage::Authenticate(intent) = decode(raw.as_bytes()).unwrap() else {
        unreachable!()
    };
    Ok(intent)
}
async fn lease(pool: &PgPool, key: ParticipantKey) -> anyhow::Result<DateTime<Utc>> {
    Ok(sqlx::query_scalar(
        "select lease_until from concert_world_memberships where event_id=$1 and user_id=$2",
    )
    .bind(key.event_id.uuid())
    .bind(key.user_id.uuid())
    .fetch_one(pool)
    .await?)
}

#[tokio::test]
#[ignore = "requires owned auth_bridge_native.py Auth and Concert PG"]
async fn native_database_auth_bridge_admit_switch_renew_and_revocation() -> anyhow::Result<()> {
    let auth = pool("auth").await?;
    let concert = pool("concert").await?;
    let event = EventId::from_uuid(Uuid::new_v4());
    let user = AuthUserId::from_uuid(Uuid::new_v4());
    let family = Uuid::new_v4();
    let subject = WorldSubject {
        key: ParticipantKey {
            event_id: event,
            user_id: user,
        },
        event_id: event,
        session_id: SessionId::new(family.to_string())?,
    };
    let room = Uuid::new_v4();
    let target = Uuid::new_v4();
    sqlx::query("insert into users(id,primary_email,password_hash,status) values($1,$2,'fixture-only','ACTIVE')").bind(user.uuid()).bind(format!("{family}@auth-bridge.invalid")).execute(&auth).await?;
    sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) values($1,'fixture-only',$2,$3,clock_timestamp()+interval '1 hour')").bind(Uuid::new_v4().to_string()).bind(user.uuid()).bind(family).execute(&auth).await?;
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Bridge Auth fixture','OPEN')").bind(event.uuid()).bind(format!("bridge-{}",event.uuid().simple())).execute(&concert).await?;
    sqlx::query(
        "insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')",
    )
    .bind(event.uuid())
    .bind(user.uuid())
    .execute(&concert)
    .await?;
    sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(event.uuid()).bind(user.uuid()).execute(&concert).await?;
    sqlx::query("insert into concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,is_default) values($1,$2,'First',2,'map_fixture',1,'avatar_fixture',1,true),($1,$3,'Second',2,'map_fixture',1,'avatar_fixture',1,false)").bind(event.uuid()).bind(room).bind(target).execute(&concert).await?;
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
    let server = server(auth.clone()).await;
    let reader = Arc::new(GrpcWorldAuthReader::connect(
        &server.endpoint,
        "world-auth-fixture-concert",
        None,
        signer(),
    )?);
    let anchor = ClockAnchor {
        monotonic_ms: 0,
        utc: Utc::now(),
    };
    let started = Instant::now();
    let bridge = AdmissionBridge::new_production(
        concert.clone(),
        Arc::new(BootstrapService::new(concert.clone(), release)),
        Arc::new(ProfileFixture {
            fence: Arc::new(Mutex::new(())),
            generation: Uuid::new_v4(),
        }),
        reader,
        OwnerIdentity::new(Uuid::new_v4().to_string(), Uuid::new_v4())?,
        event,
        BTreeMap::from([(room, Uuid::new_v4()), (target, Uuid::new_v4())]),
        pins(),
        anchor.clone(),
        started,
    )?;
    bridge.claim_rooms().await?;
    let expiry = anchor.utc + TimeDelta::seconds(60);
    let first = ticket(&bridge, &subject, room, expiry, 41).await?;
    assert!(bridge
        .repository
        .current_owned_membership(subject.key, Utc::now())
        .await?
        .is_none());
    let b = bridge
        .admit(first.clone(), Uuid::new_v4())
        .await
        .map_err(|_| anyhow::anyhow!("production Auth admission failed"))?;
    let mut adapter = MembershipAdapter::new(bridge.clone(), anchor.clone(), 50).unwrap();
    adapter.install(&b, bridge.now_ms()?).unwrap();
    let observed = CanonicalAuthorityPort::current(&bridge, &b, bridge.now_ms()?).unwrap();
    assert_eq!(observed.access_expires_at, expiry);
    assert!(observed.lease_expires_at <= Utc::now() + TimeDelta::seconds(2));
    let auth_deadline = anchor.deadline(observed.lease_expires_at).unwrap();
    assert!(
        CanonicalAuthorityPort::current(&bridge, &b, auth_deadline).is_err(),
        "Auth window expiry is enforced without a heartbeat"
    );
    assert!(bridge.admit(first, Uuid::new_v4()).await.is_err());
    let consumed: i64 = sqlx::query_scalar(
        "select count(*) from concert_world_tickets where event_id=$1 and consumed_at is not null",
    )
    .bind(event.uuid())
    .fetch_one(&concert)
    .await?;
    assert_eq!(consumed, 1, "sealed Auth must not consume a second ticket");
    let before = lease(&concert, subject.key).await?;
    assert!(before <= expiry);
    assert!(
        before <= Utc::now() + TimeDelta::seconds(5),
        "initial persisted lease uses milliseconds rather than seconds"
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    bridge.maintain().await?;
    let renewed = lease(&concert, subject.key).await?;
    assert!(renewed <= Utc::now() + TimeDelta::seconds(5));
    assert!(renewed > Utc::now());
    assert!(renewed > before);
    // Repeated fresh reads must advance a bounded lease without extending the
    // original credential expiry or the effective Auth publication window.
    tokio::time::sleep(Duration::from_millis(100)).await;
    bridge.maintain().await?;
    assert!(lease(&concert, subject.key).await? > renewed);
    assert_eq!(
        CanonicalAuthorityPort::current(&bridge, &b, bridge.now_ms()?)
            .unwrap()
            .access_expires_at,
        expiry
    );
    let change = ticket(&bridge, &subject, target, expiry, 42).await?;
    let moved = bridge
        .switch(&b, change, Uuid::new_v4())
        .await
        .map_err(|_| anyhow::anyhow!("production Auth switch failed"))?;
    adapter.install(&moved, bridge.now_ms()?).unwrap();
    assert_eq!(moved.room_id, target);
    assert!(
        lease(&concert, subject.key).await? <= Utc::now() + TimeDelta::seconds(5),
        "switch persisted lease uses the same five-second bound"
    );
    assert!(
        CanonicalAuthorityPort::current(&bridge, &moved, bridge.now_ms()?)
            .unwrap()
            .lease_expires_at
            <= Utc::now() + TimeDelta::seconds(2)
    );
    assert_eq!(
        CanonicalAuthorityPort::current(&bridge, &moved, bridge.now_ms()?)
            .unwrap()
            .access_expires_at,
        expiry
    );
    let before_denial = lease(&concert, subject.key).await?;
    sqlx::query("update refresh_tokens set revoked_at=clock_timestamp() where user_id=$1")
        .bind(user.uuid())
        .execute(&auth)
        .await?;
    bridge.maintain().await?;
    assert_eq!(
        lease(&concert, subject.key).await?,
        before_denial,
        "denied fresh Auth cannot extend DB lease"
    );
    assert!(CanonicalAuthorityPort::current(&bridge, &moved, bridge.now_ms()?).is_err());
    assert_eq!(bridge.cleanup().await?, 0);
    assert!(bridge
        .repository
        .current_owned_membership(subject.key, Utc::now())
        .await?
        .is_none());
    sqlx::query("update users set status='SUSPENDED' where id=$1")
        .bind(user.uuid())
        .execute(&auth)
        .await?;
    let denied = ticket(&bridge, &subject, target, expiry, 43).await?;
    assert_eq!(
        bridge.admit(denied, Uuid::new_v4()).await.unwrap_err().code,
        4403
    );
    assert!(bridge
        .repository
        .current_owned_membership(subject.key, Utc::now())
        .await?
        .is_none());
    sqlx::query("update users set status='ACTIVE' where id=$1")
        .bind(user.uuid())
        .execute(&auth)
        .await?;
    sqlx::query("update refresh_tokens set revoked_at=null where user_id=$1")
        .bind(user.uuid())
        .execute(&auth)
        .await?;
    let last = ticket(&bridge, &subject, target, expiry, 44).await?;
    let last = bridge
        .admit(last, Uuid::new_v4())
        .await
        .map_err(|_| anyhow::anyhow!("restored owner denied"))?;
    adapter.install(&last, bridge.now_ms()?).unwrap();
    let before_drift = lease(&concert, subject.key).await?;
    sqlx::query("update concert_world_memberships set access_expires_at=access_expires_at+interval '1 second' where event_id=$1 and user_id=$2")
        .bind(event.uuid()).bind(user.uuid()).execute(&concert).await?;
    bridge.maintain().await?;
    assert_eq!(lease(&concert, subject.key).await?, before_drift);
    assert!(
        CanonicalAuthorityPort::current(&bridge, &last, bridge.now_ms()?).is_err(),
        "canonical credential expiry drift cannot replace the captured lifetime"
    );
    assert_eq!(bridge.cleanup().await?, 0);
    let last = ticket(&bridge, &subject, target, expiry, 45).await?;
    let last = bridge
        .admit(last, Uuid::new_v4())
        .await
        .map_err(|_| anyhow::anyhow!("fresh admission after expiry drift failed"))?;
    adapter.install(&last, bridge.now_ms()?).unwrap();
    let before_outage = lease(&concert, subject.key).await?;
    server.finish().await;
    bridge.maintain().await?;
    assert_eq!(
        lease(&concert, subject.key).await?,
        before_outage,
        "unavailable Auth cannot extend DB lease"
    );
    assert!(CanonicalAuthorityPort::current(&bridge, &last, bridge.now_ms()?).is_err());
    assert_eq!(bridge.cleanup().await?, 0);
    concert.close().await;
    auth.close().await;
    Ok(())
}
