//! QA05 realtime failure suite against the actual World socket router and
//! runtime over loopback TCP with marker-owned PostgreSQL admission.
//!
//! Every frame observed by a client is produced by the production
//! `WorldSocketState`/`WorldRuntime`/`AdmissionBridge` implementations over a
//! real socket. External Auth/owner observations use the existing synthetic
//! `LiveWorldOwners` fixture (`new_fixture`), exactly like
//! `tests/world_runtime_support/postgres.rs`; this is not IN01/IN03 evidence.
//! A transport readiness gate emulates a saturated client send buffer for the
//! slow-consumer case; it gates delivery only and never fabricates a frame.
//!
//! Run only through `backend/scripts/mcr/qa05_realtime_suite.py`, which owns
//! the PostgreSQL 16 cluster and the `QA05_TEST_*` environment contract.
#![allow(clippy::too_many_arguments)]

use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use concert_application::{
    bootstrap::{BootstrapDefaults, BootstrapRelease, ValidatedProductRelease},
    identity::SessionId,
    world_ticket::*,
};
use concert_domain::{
    place::{Geometry, Interaction, PlaceId, Spawn},
    world::{Limits, WorldMap},
    AssetId, AuthUserId, AvatarSelection, EventId, ParticipantKey, Revision,
};
use concert_service::{
    adapters::r#in::world_ws::*,
    bootstrap::BootstrapService,
    world::{
        admission_bridge::*, ingress::*, interest::*, membership::*, replication::*,
        runtime::*, simulation::*, PgWorldRepository,
    },
};
use concert_application::world_ticket::WorldError;
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::oneshot;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::CloseFrame, Message},
    MaybeTlsStream, WebSocketStream,
};
use uuid::Uuid;

type Client = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
const ORIGIN: &str = "http://localhost:9000";

fn id(v: u128) -> Uuid {
    Uuid::from_u128(v)
}
fn asset(s: &str) -> AssetId {
    AssetId::new(s.into()).unwrap()
}
fn pins() -> concert_service::world::simulation::ResourcePin {
    concert_service::world::simulation::ResourcePin {
        map_id: asset("map_fixture"),
        map_revision: Revision::initial(),
        avatar_manifest_id: asset("avatar_fixture"),
        avatar_manifest_version: Revision::initial(),
    }
}
fn avatar() -> AvatarSelection {
    AvatarSelection {
        manifest_id: asset("avatar_fixture"),
        manifest_version: Revision::initial(),
        appearance: concert_domain::AvatarAppearance::Layered {
            body: asset("body_fixture"),
            eyes: asset("eyes_fixture"),
            hair: asset("hair_fixture"),
            outfit: asset("outfit_fixture"),
            accessory: None,
            skin_tone: asset("tone_fixture"),
        },
    }
}
fn identity() -> PublicIdentity {
    PublicIdentity {
        display_name: "QA05 fixture user".into(),
        avatar_revision: Revision::initial(),
        avatar: avatar(),
        facing: Facing::Down,
        moving: false,
    }
}
fn release() -> Arc<ValidatedProductRelease> {
    Arc::new(
        ValidatedProductRelease::from_validated(
            BootstrapRelease::new(
                BootstrapDefaults {
                    theme: "REVENGE".parse().unwrap(),
                    avatar: avatar(),
                },
                b"{}".to_vec(),
            )
            .unwrap(),
            "a".repeat(64),
        )
        .unwrap(),
    )
}
fn room_spec(room: Uuid, epoch: Uuid, capacity: usize) -> RoomSpec {
    let g = Geometry::new(
        12,
        8,
        32,
        32,
        "collision_fixture".into(),
        vec![vec![0; 12]; 8],
        vec![vec![1; 12]; 8],
        vec![vec![0; 12]; 8],
        vec![Spawn {
            id: "spawn_fixture".into(),
            x: 1,
            y: 2,
        }],
        vec![Interaction {
            id: "inspect_fixture".into(),
            place_id: PlaceId::from_str("00000000-0000-4000-8000-000000000090").unwrap(),
            x: 2,
            y: 2,
        }],
        "spawn_fixture".into(),
    )
    .unwrap();
    RoomSpec {
        room_id: room,
        room_epoch: epoch,
        resources: pins(),
        map: WorldMap::from_geometry(&g).unwrap(),
        limits: Limits::new(120.0, 48.0, 250).unwrap(),
        capacity,
        spawn_id: "spawn_fixture".into(),
    }
}

/// Synthetic external authority: it grants every configured subject and exposes
/// a generation so projection invalidation is observable. It is not Auth/Profile.
struct Owners {
    fence: Arc<Mutex<()>>,
    generation: Mutex<Uuid>,
    event: EventId,
}
impl Owners {
    fn new(event: EventId) -> Self {
        Self {
            fence: Arc::new(Mutex::new(())),
            generation: Mutex::new(Uuid::new_v4()),
            event,
        }
    }
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
        now: DateTime<Utc>,
    ) -> Result<LiveOwnerView, WorldError> {
        if subject.key.event_id != self.event {
            return Err(WorldError::Denied);
        }
        let expires = now + TimeDelta::milliseconds(4900);
        Ok(LiveOwnerView {
            subject: subject.clone(),
            checked_at: now,
            expires_at: expires,
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

/// `send`/`close`/`installed` always delegate to the real `SocketTransport`;
/// only the runtime's readiness observation is gated, mirroring a saturated
/// native send buffer without inventing a frame.
#[derive(Clone)]
struct GatedTransport {
    inner: SocketTransport,
    blocked: Arc<Mutex<Option<Uuid>>>,
}
impl TransportPort for GatedTransport {
    fn installed(&mut self, b: &Binding, now: u64) -> Result<(), WriteError> {
        self.inner.installed(b, now)
    }
    fn ready(&self, b: &Binding) -> bool {
        self.blocked
            .lock()
            .map(|blocked| *blocked != Some(b.key.user_id.uuid()))
            .unwrap_or(false)
            && self.inner.ready(b)
    }
    fn send(&mut self, b: &Binding, delivery: Delivery) -> Result<(), WriteError> {
        self.inner.send(b, delivery)
    }
    fn close(&mut self, b: &Binding, reason: SafeClose) {
        self.inner.close(b, reason)
    }
}

struct Generation {
    bridge: Arc<AdmissionBridge>,
    transport: SocketTransport,
    blocked: Arc<Mutex<Option<Uuid>>>,
    handle: Arc<RuntimeHandle>,
    owner_task: tokio::task::JoinHandle<ShutdownReport>,
    server: Option<tokio::task::JoinHandle<()>>,
    stop: oneshot::Sender<()>,
    maintenance: tokio::task::JoinHandle<()>,
    address: std::net::SocketAddr,
}

async fn spawn_generation(
    pool: PgPool,
    event: EventId,
    owner: OwnerIdentity,
    anchor: ClockAnchor,
    started: Instant,
    bootstrap: Arc<BootstrapService>,
    capacity: BTreeMap<Uuid, usize>,
    budget: usize,
    epochs: BTreeMap<Uuid, Uuid>,
) -> anyhow::Result<Generation> {
    let owners = Arc::new(Owners::new(event));
    let bridge = AdmissionBridge::new_fixture(
        pool,
        bootstrap,
        owners.clone(),
        owner.clone(),
        event,
        epochs.clone(),
        pins(),
        anchor.clone(),
        started,
    )
    .map_err(|_| anyhow::anyhow!("fixture bridge"))?;
    bridge
        .claim_rooms()
        .await
        .map_err(|_| anyhow::anyhow!("claim rooms"))?;
    let transport = SocketTransport::default();
    let blocked = Arc::new(Mutex::new(None));
    let gated = GatedTransport {
        inner: transport.clone(),
        blocked: blocked.clone(),
    };
    let specs = capacity
        .iter()
        .map(|(room, cap)| room_spec(*room, epochs[room], *cap))
        .collect();
    let runtime = WorldRuntime::new(
        event,
        Uuid::parse_str(&owner.process_id)?,
        anchor,
        specs,
        bridge.clone(),
        bridge.clone(),
        gated,
    )
    .map_err(|_| anyhow::anyhow!("runtime"))?;
    let (clock_started, origin_ms) = runtime.clock_origin();
    let (handle, owner_task) = runtime.spawn();
    let handle = Arc::new(handle);
    let state = WorldSocketState::with_native_write_budget(
        OriginPolicy::new(vec![ORIGIN.into()], true).map_err(|_| anyhow::anyhow!("origin"))?,
        transport.clone(),
        bridge.clone(),
        handle.clone(),
        clock_started,
        origin_ms,
        budget,
    )
    .map_err(|_| anyhow::anyhow!("socket state"))?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let (stop, stopped) = oneshot::channel();
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, router(state))
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await;
    });
    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let maintained = bridge.clone();
    let maintenance = tokio::spawn(async move {
        loop {
            ticker.tick().await;
            if maintained.maintain().await.is_err() {
                break;
            }
        }
    });
    Ok(Generation {
        bridge,
        transport,
        blocked,
        handle,
        owner_task,
        server: Some(server),
        stop,
        maintenance,
        address,
    })
}

struct World {
    pool: PgPool,
    event: EventId,
    owner: OwnerIdentity,
    anchor: ClockAnchor,
    started: Instant,
    bootstrap: Arc<BootstrapService>,
    capacity: BTreeMap<Uuid, usize>,
    epochs: BTreeMap<Uuid, Uuid>,
    budget: usize,
    sessions: Mutex<BTreeMap<Uuid, Uuid>>,
    order: Vec<Uuid>,
    generation: Generation,
}

impl World {
    async fn new(
        pool: PgPool,
        users: &[Uuid],
        rooms: &[(usize, Uuid)],
        budget: usize,
    ) -> anyhow::Result<Self> {
        let event = Uuid::new_v4();
        sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'QA05 realtime','OPEN')")
            .bind(event)
            .bind(format!("qa05-{}", Uuid::new_v4().simple()))
            .execute(&pool)
            .await?;
        let room_ids: Vec<Uuid> = rooms.iter().map(|_| Uuid::new_v4()).collect();
        for (index, ((capacity, _), room)) in rooms.iter().zip(&room_ids).enumerate() {
            sqlx::query(
                "insert into concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,is_default) values($1,$2,$3,$4,'map_fixture',1,'avatar_fixture',1,$5)",
            )
            .bind(event)
            .bind(room)
            .bind(format!("QA05 room {index}"))
            .bind(*capacity as i32)
            .bind(index == 0)
            .execute(&pool)
            .await?;
        }
        for user in users {
            sqlx::query(
                "insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')",
            )
            .bind(event)
            .bind(user)
            .execute(&pool)
            .await?;
            sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)")
                .bind(event)
                .bind(user)
                .execute(&pool)
                .await?;
        }
        let bootstrap = Arc::new(BootstrapService::new(pool.clone(), release()));
        let owner = OwnerIdentity::new(Uuid::new_v4().to_string(), Uuid::new_v4())?;
        let anchor = ClockAnchor {
            monotonic_ms: 0,
            utc: Utc::now(),
        };
        let capacity: BTreeMap<Uuid, usize> = room_ids
            .iter()
            .zip(rooms)
            .map(|(room, (cap, _))| (*room, *cap))
            .collect();
        let epochs: BTreeMap<Uuid, Uuid> = room_ids
            .iter()
            .zip(rooms)
            .map(|(room, (_, epoch))| (*room, *epoch))
            .collect();
        let started = Instant::now();
        let generation = spawn_generation(
            pool.clone(),
            EventId::from_uuid(event),
            owner.clone(),
            anchor.clone(),
            started,
            bootstrap.clone(),
            capacity.clone(),
            budget,
            epochs.clone(),
        )
        .await?;
        Ok(Self {
            pool,
            event: EventId::from_uuid(event),
            owner,
            anchor,
            started,
            bootstrap,
            capacity,
            epochs,
            budget,
            sessions: Mutex::new(BTreeMap::new()),
            order: room_ids.clone(),
            generation,
        })
    }

    fn room(&self, index: usize) -> Uuid {
        self.order[index]
    }

    fn block(&self, user: Uuid) {
        *self.generation.blocked.lock().unwrap() = Some(user);
    }

    fn unblock(&self) {
        *self.generation.blocked.lock().unwrap() = None;
    }

    async fn stop(&mut self) {
        let generation = &mut self.generation;
        generation.maintenance.abort();
        generation.transport.close_all(SafeClose::UNAVAILABLE);
        generation.handle.request_stop();
        if let Ok(Ok(report)) =
            tokio::time::timeout(Duration::from_secs(3), &mut generation.owner_task).await
        {
            for b in report.remaining_cleanup {
                let _ = CanonicalAuthorityPort::release(&generation.bridge, &b);
            }
        }
        let _ = tokio::time::timeout(Duration::from_secs(3), generation.bridge.cleanup()).await;
        let stop = std::mem::replace(&mut generation.stop, oneshot::channel().0);
        let _ = stop.send(());
        if let Some(server) = generation.server.take() {
            let _ = tokio::time::timeout(Duration::from_secs(3), server).await;
        }
    }

    async fn restart(&mut self) -> anyhow::Result<BTreeMap<Uuid, Uuid>> {
        self.stop().await;
        self.epochs = self
            .capacity
            .keys()
            .map(|room| (*room, Uuid::new_v4()))
            .collect();
        self.anchor = ClockAnchor {
            monotonic_ms: 0,
            utc: Utc::now(),
        };
        self.started = Instant::now();
        self.generation = spawn_generation(
            self.pool.clone(),
            self.event,
            self.owner.clone(),
            self.anchor.clone(),
            self.started,
            self.bootstrap.clone(),
            self.capacity.clone(),
            self.budget,
            self.epochs.clone(),
        )
        .await?;
        Ok(self.epochs.clone())
    }

    async fn ticket(&self, room: Uuid, user: Uuid) -> (String, AdmissionIntent) {
        use ring::rand::SecureRandom;
        let mut entropy = [0; 32];
        ring::rand::SystemRandom::new().fill(&mut entropy).unwrap();
        let ticket = AdmissionTicket::from_entropy(&entropy).unwrap();
        let now = Utc::now();
        let user_id = AuthUserId::from_uuid(user);
        let session = {
            let mut sessions = self.sessions.lock().unwrap();
            *sessions.entry(user).or_insert_with(Uuid::new_v4)
        };
        let pin = concert_application::world_ticket::ResourcePin {
            event_id: self.event,
            room_id: concert_application::world_ticket::RoomId::from_uuid(room),
            map: concert_application::world_ticket::MapPin::new(
                asset("map_fixture"),
                Revision::initial(),
            ),
            avatar: concert_application::world_ticket::AvatarPin::new(
                asset("avatar_fixture"),
                Revision::initial(),
            ),
        };
        PgWorldRepository::new(self.pool.clone())
            .issue(TicketRecord {
                digest: ticket.digest(),
                subject: WorldSubject {
                    key: ParticipantKey {
                        event_id: self.event,
                        user_id,
                    },
                    event_id: self.event,
                    session_id: SessionId::new(session.to_string()).unwrap(),
                },
                pin,
                issued_at: now,
                expires_at: now + TimeDelta::seconds(20),
                access_expires_at: self.anchor.utc + TimeDelta::seconds(120),
            })
            .await
            .unwrap();
        let raw = ticket.as_str().to_owned();
        let ClientMessage::Authenticate(intent) = decode(
            json!({"protocolVersion":"1.0.0","type":"authenticate","ticket":raw,"eventId":self.event.uuid(),"roomId":room,"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1})
                .to_string()
                .as_bytes(),
        )
        .unwrap() else {
            unreachable!()
        };
        (raw, intent)
    }

    async fn connect(&self) -> Client {
        let mut request = format!("ws://{}/world-ws", self.generation.address)
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("origin", ORIGIN.parse().unwrap());
        let (client, _) = connect_async(request).await.unwrap();
        client
    }

    async fn authenticate(&self, client: &mut Client, room: Uuid, user: Uuid) -> (String, Value) {
        let (raw, _) = self.ticket(room, user).await;
        let snapshot = self.authenticate_raw(client, &raw, room).await;
        (raw, snapshot)
    }

    async fn authenticate_raw(&self, client: &mut Client, raw: &str, room: Uuid) -> Value {
        client
            .send(Message::Text(
                json!({"protocolVersion":"1.0.0","type":"authenticate","ticket":raw,"eventId":self.event.uuid(),"roomId":room,"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1})
                    .to_string(),
            ))
            .await
            .unwrap();
        let value = json_frame(client).await;
        assert_eq!(value["type"], "snapshot", "authenticate must yield a snapshot");
        value
    }

    async fn active_members(&self, user: Option<Uuid>) -> Vec<(Uuid, Uuid, Uuid)> {
        sqlx::query_as(
            "select user_id, room_id, connection_id from public.concert_world_memberships \
             where event_id=$1 and ($2::uuid is null or user_id=$2) \
             and not disconnected and lease_until > clock_timestamp()",
        )
        .bind(self.event.uuid())
        .bind(user)
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }

    async fn all_member_rows(&self, user: Uuid) -> i64 {
        sqlx::query_scalar(
            "select count(*) from public.concert_world_memberships where event_id=$1 and user_id=$2",
        )
        .bind(self.event.uuid())
        .bind(user)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
}

async fn owned_pool() -> anyhow::Result<PgPool> {
    let root = PathBuf::from(std::env::var("QA05_TEST_OWNER_ROOT")?);
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        info.is_dir() && !info.file_type().is_symlink() && info.permissions().mode() & 0o077 == 0,
        "private owned root required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino()) == std::env::var("QA05_TEST_OWNER_IDENTITY")?,
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
    let marker: Value = serde_json::from_slice(&std::fs::read(marker_path)?)?;
    anyhow::ensure!(
        marker["token"].as_str() == Some(&std::env::var("QA05_TEST_OWNER_TOKEN")?),
        "fixture owner mismatch"
    );
    let options = PgConnectOptions::from_str(&std::env::var("QA05_TEST_DATABASE_URL")?)
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
        .acquire_timeout(Duration::from_secs(3))
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

enum Incoming {
    Json(Value),
    Close(u16, String),
}

async fn incoming(client: &mut Client) -> Incoming {
    incoming_within(client, Duration::from_secs(5)).await
}

async fn incoming_within(client: &mut Client, within: Duration) -> Incoming {
    loop {
        let next = tokio::time::timeout(within, client.next())
            .await
            .expect("frame deadline")
            .expect("socket ended")
            .expect("socket error");
        match next {
            Message::Text(text) => {
                let value: Value = serde_json::from_str(&text).expect("json frame");
                if value["type"] == "ping" {
                    client
                        .send(Message::Text(
                            json!({"protocolVersion":"1.0.0","type":"pong","heartbeatId":value["heartbeatId"]})
                                .to_string(),
                        ))
                        .await
                        .unwrap();
                    continue;
                }
                return Incoming::Json(value);
            }
            Message::Close(frame) => return match frame {
                Some(frame) => Incoming::Close(u16::from(frame.code), frame.reason.to_string()),
                None => Incoming::Close(1005, String::new()),
            },
            Message::Ping(_) | Message::Pong(_) | Message::Binary(_) | Message::Frame(_) => continue,
        }
    }
}

async fn json_frame(client: &mut Client) -> Value {
    match incoming(client).await {
        Incoming::Json(value) => value,
        Incoming::Close(code, reason) => panic!(
            "expected a json frame, got close {code} {reason:?}"
        ),
    }
}

async fn close_frame(client: &mut Client) -> CloseFrame<'static> {
    close_frame_within(client, Duration::from_secs(5)).await
}

async fn close_frame_within(client: &mut Client, within: Duration) -> CloseFrame<'static> {
    loop {
        match incoming_within(client, within).await {
            Incoming::Close(code, reason) => {
                return CloseFrame {
                    code: code.into(),
                    reason: reason.into(),
                }
            }
            Incoming::Json(_) => {}
        }
    }
}

async fn until<F>(client: &mut Client, mut predicate: F) -> Value
where
    F: FnMut(&Value) -> bool,
{
    for _ in 0..16 {
        let value = json_frame(client).await;
        if predicate(&value) {
            return value;
        }
    }
    panic!("expected frame never arrived");
}

fn assert_unique_players(snapshot: &Value) {
    let players = snapshot["players"].as_array().expect("players");
    let mut seen = BTreeSet::new();
    for player in players {
        assert!(seen.insert(player["userId"].as_str().unwrap().to_owned()));
        assert!(player["position"]["x"].as_f64().is_some());
        assert!(player["position"]["y"].as_f64().is_some());
    }
}

fn player_ids(snapshot: &Value) -> BTreeSet<String> {
    snapshot["players"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["userId"].as_str().unwrap().to_owned())
        .collect()
}

async fn input(client: &mut Client, snapshot: &Value, seq: u64, axis: i64) {
    client
        .send(Message::Text(
            json!({"protocolVersion":"1.0.0","type":"input","streamId":snapshot["streamId"],"roomEpoch":snapshot["roomEpoch"],"seq":seq,"axisX":axis,"axisY":0,"buttons":[]})
                .to_string(),
        ))
        .await
        .unwrap();
}

/// Delayed reads coalesce without corruption, a client-observed sequence gap is
/// recovered by a correlated resync, and the fresh snapshot is coherent.
#[tokio::test]
#[ignore = "requires qa05_realtime_suite.py marker-owned PostgreSQL"]
async fn qa05_delayed_and_gapped_frames_recover_to_a_fresh_snapshot() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let a_user = Uuid::new_v4();
    let b_user = Uuid::new_v4();
    let mut world = World::new(pool, &[a_user, b_user], &[(3, id(61))], NATIVE_WRITE_BYTES).await?;
    let room = world.room(0);
    let mut a = world.connect().await;
    let (_, initial) = world.authenticate(&mut a, room, a_user).await;
    assert_eq!(initial["cause"], "INITIAL");
    assert_eq!(initial["streamSeq"], 0);
    assert_unique_players(&initial);
    let mut b = world.connect().await;
    let (_, b_snapshot) = world.authenticate(&mut b, room, b_user).await;
    assert_unique_players(&b_snapshot);
    assert_eq!(player_ids(&b_snapshot).len(), 2);
    let _ = until(&mut a, |v| {
        v["upserts"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(|p| p["userId"] == b_user.to_string()))
    })
    .await;

    // A deliberately stops reading while B moves for 400ms; the server keeps a
    // single coalesced pending frame per stream. On resume, sequence numbers
    // must stay monotonic and the latest roster coherent.
    for seq in 1..=4u64 {
        input(&mut b, &b_snapshot, seq, 1).await;
        tokio::time::sleep(Duration::from_millis(90)).await;
    }
    tokio::time::sleep(Duration::from_millis(400)).await;
    let mut last_seq = 0u64;
    let mut resumed = 0;
    for _ in 0..6 {
        let frame = json_frame(&mut a).await;
        if frame["type"] != "delta" {
            continue;
        }
        let seq = frame["streamSeq"].as_u64().unwrap();
        assert!(seq > last_seq, "delayed delivery must stay monotonic");
        last_seq = seq;
        resumed += 1;
        if resumed >= 2 {
            break;
        }
    }
    assert!(resumed >= 1, "a delayed client still receives coalesced state");

    // The client observes a real sequence gap: consume delta k, discard k+1,
    // then receive k+2 and recover from last good seq.
    input(&mut b, &b_snapshot, 5, 1).await;
    let good = until(&mut a, |v| v["type"] == "delta").await;
    let good_seq = good["streamSeq"].as_u64().unwrap();
    input(&mut b, &b_snapshot, 6, 0).await;
    let _discarded = until(&mut a, |v| v["type"] == "delta").await;
    input(&mut b, &b_snapshot, 7, 1).await;
    let gap = until(&mut a, |v| v["type"] == "delta").await;
    let gap_seq = gap["streamSeq"].as_u64().unwrap();
    assert!(gap_seq > good_seq + 1, "client observed a real sequence gap");
    let request = Uuid::new_v4();
    a.send(Message::Text(json!({
        "protocolVersion":"1.0.0","type":"resync","requestId":request,
        "streamId":gap["streamId"],"roomEpoch":gap["roomEpoch"],
        "lastStreamSeq":good_seq,"reason":"GAP"
    }).to_string()))
    .await
    .unwrap();
    let fresh = until(&mut a, |v| v["type"] == "snapshot").await;
    assert_eq!(fresh["cause"], "RESYNC");
    assert_eq!(fresh["requestId"], request.to_string());
    assert_eq!(fresh["streamSeq"], 0);
    assert_ne!(fresh["streamId"], gap["streamId"]);
    assert_eq!(fresh["roomEpoch"], gap["roomEpoch"]);
    assert_unique_players(&fresh);
    assert_eq!(player_ids(&fresh).len(), 2, "fresh snapshot keeps both actors once");
    let active = world.active_members(None).await;
    assert_eq!(active.len(), 2);
    let distinct: BTreeSet<_> = active.iter().map(|(user, _, _)| *user).collect();
    assert_eq!(distinct.len(), 2, "no duplicate actor membership");
    assert_eq!(world.all_member_rows(a_user).await, 1);
    a.close(None).await.ok();
    b.close(None).await.ok();
    world.stop().await;
    Ok(())
}

/// A saturated sink is backpressured at the server-owned boundary: pending
/// delivery ages out into the frozen 4409 BACKPRESSURE terminal, membership is
/// released, and a reconnect obtains a fresh snapshot without a ghost.
#[tokio::test]
#[ignore = "requires qa05_realtime_suite.py marker-owned PostgreSQL"]
async fn qa05_slow_consumer_backpressure_recovers_without_ghost() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let a_user = Uuid::new_v4();
    let b_user = Uuid::new_v4();
    let mut world = World::new(pool, &[a_user, b_user], &[(3, id(62))], NATIVE_WRITE_BYTES).await?;
    let room = world.room(0);
    let mut a = world.connect().await;
    let (_, _a_snapshot) = world.authenticate(&mut a, room, a_user).await;
    let mut b = world.connect().await;
    let (_, b_snapshot) = world.authenticate(&mut b, room, b_user).await;
    let _ = until(&mut a, |v| {
        v["upserts"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(|p| p["userId"] == b_user.to_string()))
    })
    .await;

    // Only A's sink becomes not-ready; the runtime coalesces one pending frame
    // per stream and the real socket remains the only delivery path.
    world.block(a_user);
    let started = Instant::now();
    for seq in 1..=3u64 {
        input(&mut b, &b_snapshot, seq, if seq % 2 == 0 { -1 } else { 1 }).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let close = close_frame_within(&mut a, Duration::from_secs(10)).await;
    let elapsed = started.elapsed();
    world.unblock();
    assert_eq!(u16::from(close.code), 4409, "slow consumer ends with resync-required");
    assert_eq!(close.reason.as_ref(), "BACKPRESSURE");
    assert!(
        elapsed >= Duration::from_millis(4900) && elapsed <= Duration::from_millis(9000),
        "bounded slow-consumer window: {elapsed:?}"
    );
    let active = world.active_members(None).await;
    assert_eq!(active.len(), 1, "backpressured client releases its membership");
    assert_eq!(active[0].0, b_user);
    let mut again = world.connect().await;
    let (_, fresh) = world.authenticate(&mut again, room, a_user).await;
    assert_eq!(fresh["cause"], "INITIAL");
    assert_eq!(fresh["streamSeq"], 0);
    assert_unique_players(&fresh);
    assert_eq!(player_ids(&fresh).len(), 2);
    let active = world.active_members(Some(a_user)).await;
    assert_eq!(active.len(), 1, "exactly one membership after recovery");
    assert_eq!(world.all_member_rows(a_user).await, 1);
    b.close(None).await.ok();
    again.close(None).await.ok();
    world.stop().await;
    Ok(())
}

/// Background/foreground resync installs a fresh correlated snapshot on the
/// same socket, preserving the access deadline and never duplicating actors.
#[tokio::test]
#[ignore = "requires qa05_realtime_suite.py marker-owned PostgreSQL"]
async fn qa05_foreground_background_resync_is_fresh_and_uncorrupted() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let a_user = Uuid::new_v4();
    let b_user = Uuid::new_v4();
    let mut world = World::new(pool, &[a_user, b_user], &[(3, id(63))], NATIVE_WRITE_BYTES).await?;
    let room = world.room(0);
    let mut a = world.connect().await;
    let (_, mut snapshot) = world.authenticate(&mut a, room, a_user).await;
    let mut b = world.connect().await;
    let (_, b_snapshot) = world.authenticate(&mut b, room, b_user).await;
    let _ = until(&mut a, |v| {
        v["upserts"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(|p| p["userId"] == b_user.to_string()))
    })
    .await;
    input(&mut b, &b_snapshot, 1, 1).await;
    let _ = until(&mut a, |v| v["type"] == "delta").await;
    let access = snapshot["accessExpiresAt"].clone();
    for reason in ["FOREGROUND", "PROTOCOL_RECOVERY"] {
        let request = Uuid::new_v4();
        a.send(Message::Text(json!({
            "protocolVersion":"1.0.0","type":"resync","requestId":request,
            "streamId":snapshot["streamId"],"roomEpoch":snapshot["roomEpoch"],
            "lastStreamSeq":0,"reason":reason
        }).to_string()))
        .await
        .unwrap();
        let fresh = until(&mut a, |v| v["type"] == "snapshot").await;
        assert_eq!(fresh["cause"], "RESYNC", "{reason}");
        assert_eq!(fresh["requestId"], request.to_string(), "{reason}");
        assert_eq!(fresh["streamSeq"], 0);
        assert_ne!(fresh["streamId"], snapshot["streamId"]);
        assert_eq!(fresh["accessExpiresAt"], access, "{reason} must not extend access");
        assert_unique_players(&fresh);
        assert_eq!(player_ids(&fresh).len(), 2, "{reason}: one entry per actor");
        snapshot = fresh;
        tokio::time::sleep(Duration::from_millis(1050)).await;
    }
    a.send(Message::Text(json!({
        "protocolVersion":"1.0.0","type":"input","streamId":Uuid::new_v4(),
        "roomEpoch":snapshot["roomEpoch"],"seq":99,"axisX":1,"axisY":0,"buttons":[]
    }).to_string()))
    .await
    .unwrap();
    let refused = close_frame(&mut a).await;
    assert_eq!(u16::from(refused.code), 4409);
    assert_eq!(refused.reason.as_ref(), "RESYNC_REQUIRED");
    b.close(None).await.ok();
    world.stop().await;
    Ok(())
}

/// A consumed ticket is one use: replay is refused with the documented 4401
/// AUTH_REQUIRED close and never creates a second membership or actor.
#[tokio::test]
#[ignore = "requires qa05_realtime_suite.py marker-owned PostgreSQL"]
async fn qa05_ticket_replay_is_rejected_without_a_second_membership() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let a_user = Uuid::new_v4();
    let b_user = Uuid::new_v4();
    let mut world = World::new(pool, &[a_user, b_user], &[(3, id(64))], NATIVE_WRITE_BYTES).await?;
    let room = world.room(0);
    let mut a = world.connect().await;
    let (ticket, a_snapshot) = world.authenticate(&mut a, room, a_user).await;
    let mut b = world.connect().await;
    let (_, b_snapshot) = world.authenticate(&mut b, room, b_user).await;
    let _ = until(&mut a, |v| {
        v["upserts"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(|p| p["userId"] == b_user.to_string()))
    })
    .await;
    assert_eq!(world.active_members(Some(a_user)).await.len(), 1);

    let mut replay = world.connect().await;
    replay
        .send(Message::Text(json!({"protocolVersion":"1.0.0","type":"authenticate","ticket":ticket,"eventId":world.event.uuid(),"roomId":room,"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1}).to_string()))
        .await
        .unwrap();
    let close = close_frame(&mut replay).await;
    assert_eq!(u16::from(close.code), 4401);
    assert_eq!(close.reason.as_ref(), "AUTH_REQUIRED");
    assert_eq!(world.active_members(Some(a_user)).await.len(), 1);
    assert_eq!(world.all_member_rows(a_user).await, 1, "replay inserts no second row");

    input(&mut b, &b_snapshot, 1, 1).await;
    let delta = until(&mut a, |v| v["type"] == "delta").await;
    assert!(delta["streamSeq"].as_u64().unwrap() >= 1);
    assert_eq!(a_snapshot["streamSeq"], 0);
    a.close(None).await.ok();
    b.close(None).await.ok();
    world.stop().await;
    Ok(())
}

/// Mid-session World restart: clients observe the shutdown close, then a fresh
/// owner/epoch serves a coherent snapshot; stale tickets stay consumed and no
/// duplicate or ghost membership survives.
#[tokio::test]
#[ignore = "requires qa05_realtime_suite.py marker-owned PostgreSQL"]
async fn qa05_server_restart_mid_session_recovers_with_a_new_epoch() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let a_user = Uuid::new_v4();
    let b_user = Uuid::new_v4();
    let mut world = World::new(pool, &[a_user, b_user], &[(3, id(65))], NATIVE_WRITE_BYTES).await?;
    let room = world.room(0);
    let mut a = world.connect().await;
    let (a_ticket, _) = world.authenticate(&mut a, room, a_user).await;
    let mut b = world.connect().await;
    let (_, b_snapshot) = world.authenticate(&mut b, room, b_user).await;
    let _ = until(&mut a, |v| {
        v["upserts"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(|p| p["userId"] == b_user.to_string()))
    })
    .await;
    input(&mut b, &b_snapshot, 1, 1).await;
    let last = until(&mut a, |v| v["type"] == "delta").await;
    let old_epoch = last["roomEpoch"].as_str().unwrap().to_owned();
    let old_stream = last["streamId"].as_str().unwrap().to_owned();

    let epochs = world.restart().await?;
    let new_epoch = epochs[&room].to_string();
    assert_ne!(old_epoch, new_epoch, "restart publishes a new room epoch");
    let a_close = close_frame(&mut a).await;
    let b_close = close_frame(&mut b).await;
    assert_eq!(u16::from(a_close.code), 4500);
    assert_eq!(u16::from(b_close.code), 4500);
    assert_eq!(a_close.reason.as_ref(), "SERVER_UNAVAILABLE");

    let mut a2 = world.connect().await;
    let (_, fresh_a) = world.authenticate(&mut a2, room, a_user).await;
    let mut b2 = world.connect().await;
    let (_, fresh_b) = world.authenticate(&mut b2, room, b_user).await;
    assert_eq!(fresh_a["cause"], "INITIAL");
    assert_eq!(fresh_a["streamSeq"], 0);
    assert_eq!(fresh_a["roomEpoch"], new_epoch);
    assert_ne!(fresh_a["streamId"], old_stream);
    assert_unique_players(&fresh_a);
    assert_eq!(player_ids(&fresh_a), BTreeSet::from([a_user.to_string()]));
    assert_eq!(fresh_b["cause"], "INITIAL");
    assert_eq!(fresh_b["streamSeq"], 0);
    assert_eq!(fresh_b["roomEpoch"], new_epoch);
    assert_ne!(fresh_b["streamId"], old_stream);
    assert_unique_players(&fresh_b);
    assert_eq!(player_ids(&fresh_b).len(), 2, "both actors once after restart");
    let _ = until(&mut a2, |v| {
        v["upserts"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(|p| p["userId"] == b_user.to_string()))
    })
    .await;
    let rows: Vec<(Uuid, bool)> = sqlx::query_as(
        "select room_id, disconnected from public.concert_world_memberships where event_id=$1 and user_id=$2",
    )
    .bind(world.event.uuid())
    .bind(a_user)
    .fetch_all(&world.pool)
    .await?;
    assert_eq!(rows.len(), 1, "no duplicate actor membership after restart");
    assert!(!rows[0].1);
    assert_eq!(rows[0].0, room);
    assert_eq!(world.active_members(None).await.len(), 2);

    let mut stale = world.connect().await;
    stale
        .send(Message::Text(json!({"protocolVersion":"1.0.0","type":"authenticate","ticket":a_ticket,"eventId":world.event.uuid(),"roomId":room,"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1}).to_string()))
        .await
        .unwrap();
    let stale_close = close_frame(&mut stale).await;
    assert_eq!(u16::from(stale_close.code), 4401);
    assert_eq!(stale_close.reason.as_ref(), "AUTH_REQUIRED");
    a2.close(None).await.ok();
    b2.close(None).await.ok();
    world.stop().await;
    Ok(())
}

/// Room switch moves one actor with a ROOM_SWITCH snapshot, releases the old
/// room membership, refuses replay of the switch ticket and keeps capacity
/// rejection from disturbing the current stream.
#[tokio::test]
#[ignore = "requires qa05_realtime_suite.py marker-owned PostgreSQL"]
async fn qa05_room_switch_is_atomic_and_replay_safe() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let a_user = Uuid::new_v4();
    let b_user = Uuid::new_v4();
    let d_user = Uuid::new_v4();
    let mut world =
        World::new(pool, &[a_user, b_user, d_user], &[(3, id(66)), (1, id(67))], NATIVE_WRITE_BYTES)
            .await?;
    let room = world.room(0);
    let target = world.room(1);
    let mut a = world.connect().await;
    let (_, a_snapshot) = world.authenticate(&mut a, room, a_user).await;
    let mut b = world.connect().await;
    let (_, _b_snapshot) = world.authenticate(&mut b, room, b_user).await;
    let mut d = world.connect().await;
    let (_, d_snapshot) = world.authenticate(&mut d, room, d_user).await;
    let _ = until(&mut a, |v| {
        v["upserts"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(|p| p["userId"] == b_user.to_string()))
    })
    .await;
    assert!(
        player_ids(&d_snapshot).contains(&a_user.to_string())
            && player_ids(&d_snapshot).contains(&b_user.to_string()),
        "D's initial snapshot includes both actors once"
    );
    assert_unique_players(&d_snapshot);
    assert_eq!(world.active_members(None).await.len(), 3);

    let (full_ticket, _) = world.ticket(target, d_user).await;

    let (switch_ticket, _) = world.ticket(target, a_user).await;
    let request = Uuid::new_v4();
    a.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"switchRoom","requestId":request,"streamId":a_snapshot["streamId"],"ticket":switch_ticket.clone(),"eventId":world.event.uuid(),"roomId":target,"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1}).to_string()))
        .await
        .unwrap();
    let switched = until(&mut a, |v| v["type"] == "snapshot").await;
    assert_eq!(switched["cause"], "ROOM_SWITCH");
    assert_eq!(switched["requestId"], request.to_string());
    assert_eq!(switched["roomId"], target.to_string());
    assert_eq!(switched["streamSeq"], 0);
    assert_ne!(switched["streamId"], a_snapshot["streamId"]);
    assert_unique_players(&switched);
    assert_eq!(player_ids(&switched), BTreeSet::from([a_user.to_string()]));
    let _ = until(&mut b, |v| {
        v["removes"]
            .as_array()
            .is_some_and(|rows| rows.contains(&json!(a_user)))
    })
    .await;
    let rows: Vec<(Uuid, bool)> = sqlx::query_as(
        "select room_id, disconnected from public.concert_world_memberships where event_id=$1 and user_id=$2",
    )
    .bind(world.event.uuid())
    .bind(a_user)
    .fetch_all(&world.pool)
    .await?;
    assert_eq!(rows.len(), 1, "switch leaves one membership row");
    assert_eq!(rows[0].0, target);
    assert!(!rows[0].1);
    assert_eq!(world.active_members(Some(a_user)).await.len(), 1);

    let mut replay = world.connect().await;
    replay
        .send(Message::Text(json!({"protocolVersion":"1.0.0","type":"authenticate","ticket":switch_ticket,"eventId":world.event.uuid(),"roomId":target,"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1}).to_string()))
        .await
        .unwrap();
    let close = close_frame(&mut replay).await;
    assert_eq!(u16::from(close.code), 4401);
    assert_eq!(close.reason.as_ref(), "AUTH_REQUIRED");

    d.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"switchRoom","requestId":Uuid::new_v4(),"streamId":d_snapshot["streamId"],"ticket":full_ticket,"eventId":world.event.uuid(),"roomId":target,"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1}).to_string()))
        .await
        .unwrap();
    let rejected = until(&mut d, |v| v["type"] == "switchRejected").await;
    assert_eq!(rejected["reason"], "ROOM_FULL");
    assert_eq!(rejected["streamId"], d_snapshot["streamId"]);
    let d_now: Vec<(Uuid,)> = sqlx::query_as(
        "select room_id from public.concert_world_memberships where event_id=$1 and user_id=$2 and not disconnected",
    )
    .bind(world.event.uuid())
    .bind(d_user)
    .fetch_all(&world.pool)
    .await?;
    assert_eq!(d_now.len(), 1);
    assert_eq!(d_now[0].0, room);
    a.close(None).await.ok();
    b.close(None).await.ok();
    d.close(None).await.ok();
    world.stop().await;
    Ok(())
}
