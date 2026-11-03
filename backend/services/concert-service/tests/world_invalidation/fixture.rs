//! Reuses marker/inode ownership checks from committed world_admission.rs;
//! external live owners below are synthetic. Real DB is socket-only and isolated.
use super::*;
// Adapted from world_runtime_support/postgres.rs at 18d4af02. Missing remote authority is explicitly synthetic.
use concert_application::{
    bootstrap::{BootstrapDefaults, BootstrapRelease, ValidatedProductRelease},
    world_ticket::*,
};
use concert_service::{
    bootstrap::BootstrapService,
    world::{admission_bridge::*, PgWorldRepository},
};
use sqlx::Row;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::time::Instant;
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    str::FromStr,
    time::Duration as StdDuration,
};
pub(super) async fn owned_pool() -> anyhow::Result<PgPool> {
    let root = PathBuf::from(std::env::var("WORLD_INVALIDATION_TEST_OWNER_ROOT")?);
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        root.is_absolute()
            && root.canonicalize()? == root
            && info.is_dir()
            && !info.file_type().is_symlink()
            && info.permissions().mode() & 0o077 == 0,
        "private owned root required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino())
            == std::env::var("WORLD_INVALIDATION_TEST_OWNER_IDENTITY")?,
        "fixture identity changed"
    );
    let marker_path = root.join("mcr-owner.json");
    let marker_info = std::fs::symlink_metadata(&marker_path)?;
    anyhow::ensure!(
        marker_info.is_file()
            && !marker_info.file_type().is_symlink()
            && marker_info.permissions().mode() & 0o077 == 0
            && marker_info.uid() == info.uid(),
        "private marker required"
    );
    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(marker_path)?)?;
    anyhow::ensure!(
        marker["token"].as_str() == Some(&std::env::var("WORLD_INVALIDATION_TEST_OWNER_TOKEN")?),
        "fixture owner mismatch"
    );
    let options =
        PgConnectOptions::from_str(&std::env::var("WORLD_INVALIDATION_TEST_DATABASE_URL")?)
            .map_err(|_| anyhow::anyhow!("invalid owned fixture options"))?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_world_invalidation")
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
    pub(super) pool: PgPool,
    fence: Arc<Mutex<()>>,
    generation: Mutex<Uuid>,
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
        Ok(LiveOwnerView {
            subject: subject.clone(),
            checked_at: now,
            expires_at: now + TimeDelta::milliseconds(5000),
            identity: identity(),
        })
    }
    async fn visible(
        &self,
        actor: &WorldSubject,
        subjects: &[WorldSubject],
        _: chrono::DateTime<Utc>,
    ) -> Result<BTreeSet<Uuid>, WorldError> {
        let mut visible = BTreeSet::new();
        for s in subjects {
            let a = actor.key.user_id.uuid();
            let b = s.key.user_id.uuid();
            let blocked:bool=sqlx::query_scalar("select coalesce((select low_blocks_high or high_blocks_low from concert_relationship_pairs where event_id=$1 and low_user=$2 and high_user=$3),false)")
                .bind(actor.event_id.uuid()).bind(a.min(b)).bind(a.max(b)).fetch_one(&self.pool).await.map_err(|_|WorldError::Unavailable)?;
            if !blocked {
                visible.insert(b);
            }
        }
        Ok(visible)
    }
}
pub(super) struct DatabaseFixture {
    pub(super) pool: PgPool,
    pub(super) bridge: Arc<AdmissionBridge>,
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
    pub(super) async fn new() -> anyhow::Result<Self> {
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
        sqlx::query("insert into concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,is_default) values($1,$2,'Current',4,'map_fixture',1,'avatar_fixture',1,true),($1,$3,'Target',4,'map_fixture',1,'avatar_fixture',1,false)").bind(event).bind(room).bind(target).execute(&pool).await?;
        Self::bound(pool, event, user, room, target, session, owner).await
    }
    pub(super) async fn resume(
        pool: PgPool,
        event: Uuid,
        room: Uuid,
        user: Uuid,
        owner_id: String,
    ) -> anyhow::Result<Self> {
        let target: Uuid = sqlx::query_scalar(
            "select room_id from concert_world_rooms where event_id=$1 and room_id<>$2 limit 1",
        )
        .bind(event)
        .bind(room)
        .fetch_one(&pool)
        .await?;
        Self::bound(
            pool,
            event,
            user,
            room,
            target,
            Uuid::new_v4(),
            OwnerIdentity::new(owner_id, Uuid::new_v4())?,
        )
        .await
    }
    async fn bound(
        pool: PgPool,
        event: Uuid,
        user: Uuid,
        room: Uuid,
        target: Uuid,
        session: Uuid,
        owner: OwnerIdentity,
    ) -> anyhow::Result<Self> {
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
            pool: pool.clone(),
            fence: Arc::new(Mutex::new(())),
            generation: Mutex::new(Uuid::new_v4()),
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
        self.ticket_expiring(room, user, self.anchor.utc + TimeDelta::seconds(60))
            .await
    }
    pub(super) async fn ticket_expiring(
        &self,
        room: Uuid,
        user: AuthUserId,
        access: chrono::DateTime<Utc>,
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
                expires_at: (now + TimeDelta::seconds(20)).min(access),
                access_expires_at: access,
            })
            .await?;
        let raw = ticket.as_str().to_owned();
        let ClientMessage::Authenticate(intent)=decode(json!({"protocolVersion":"1.0.0","type":"authenticate","ticket":raw,"eventId":self.event.uuid(),"roomId":room,"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1}).to_string().as_bytes()).unwrap() else{unreachable!()};
        Ok((raw, intent))
    }
}
