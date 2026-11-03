use async_trait::async_trait;
use chrono::{TimeDelta, Utc};
use concert_application::{commands::*, world_ticket::*};
use concert_domain::{relationship::*, AuthUserId, EventId, ParticipantKey, Revision};
use concert_service::{
    adapters::{
        out::repository::{
            outbox::{Delivery, DeliveryFailure, PgOutbox},
            receipt::PgCommandRepository,
        },
        r#in::world_ws::*,
    },
    relationship::{commands::BlockCommand, *},
    workers::outbox::{run_batch, IdempotentInvalidationSink, WorldCompositeSink},
    world::{
        admission_bridge::*, ingress::*, interest::*, invalidation::*, membership::*, runtime::*,
        simulation::*,
    },
};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use sqlx::{PgConnection, PgPool, Row};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
    MaybeTlsStream, WebSocketStream,
};
use uuid::Uuid;
#[path = "geometry.rs"]
mod geometry;
use geometry::*;
#[path = "fixture.rs"]
mod fixture;
use fixture::*;
type Client = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
#[derive(Clone)]
struct Trace {
    native: SocketTransport,
    paused: Arc<AtomicBool>,
    waits: Arc<AtomicUsize>,
    sent: Arc<Mutex<Vec<(Binding, Value, Instant)>>>,
    hold_user: Arc<Mutex<Option<Uuid>>>,
    held: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
}
impl TransportPort for Trace {
    fn installed(&mut self, b: &Binding, n: u64) -> Result<(), WriteError> {
        self.native.installed(b, n)
    }
    fn ready(&self, b: &Binding) -> bool {
        if self.paused.load(Ordering::Acquire) {
            self.waits.fetch_add(1, Ordering::Relaxed);
            false
        } else {
            self.native.ready(b)
        }
    }
    fn send(
        &mut self,
        b: &Binding,
        d: concert_service::world::replication::Delivery,
    ) -> Result<(), WriteError> {
        let body = serde_json::from_slice(&d.bytes).unwrap();
        let hold = {
            let mut user = self.hold_user.lock().unwrap();
            if *user == Some(b.key.user_id.uuid()) {
                user.take();
                true
            } else {
                false
            }
        };
        if hold {
            self.held.store(true, Ordering::Release);
            let began = Instant::now();
            while !self.release.load(Ordering::Acquire) {
                assert!(
                    began.elapsed() < Duration::from_secs(1),
                    "native handoff hold not released"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        self.native.send(b, d)?;
        let mut sent = self.sent.lock().unwrap();
        assert!(sent.len() < 1000);
        sent.push((b.clone(), body, Instant::now()));
        Ok(())
    }
    fn close(&mut self, b: &Binding, r: SafeClose) {
        self.native.close(b, r)
    }
}
struct RemoteAuthorityFixture;
#[async_trait]
impl FencedRelationshipAuthority for RemoteAuthorityFixture {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        q: PolicyQuery,
        now: chrono::DateTime<Utc>,
    ) -> Result<FreshAuthority, RelationshipError> {
        // Actual Concert participant locks; only Auth/Friend account/session half
        // is a fixture. Local pair/block/admission invariants use the real producer.
        let count: i64 = sqlx::query_scalar(
            "select count(*) from concert_participants where event_id=$1 and user_id in($2,$3)",
        )
        .bind(q.event.uuid())
        .bind(q.actor.uuid())
        .bind(q.subject.uuid())
        .fetch_one(c)
        .await
        .map_err(|_| RelationshipError::Unavailable)?;
        if count != 2 {
            return Err(RelationshipError::Denied);
        }
        Ok(FreshAuthority {
            query: q,
            checked_at: now,
            valid_until: now + TimeDelta::seconds(5),
            revision: Revision::initial(),
            actor_allowed: true,
            subject_allowed: true,
            inherited_block: false,
        })
    }
}
fn keys() -> FingerprintKeys {
    FingerprintKeys::new(
        SigningKey::new("world-invalidation-test", &[67; 32]).unwrap(),
        vec![],
    )
    .unwrap()
}
async fn block(
    pool: &PgPool,
    event: Uuid,
    a: Uuid,
    b: Uuid,
    blocked: bool,
) -> anyhow::Result<SafeReceipt> {
    let q = PolicyQuery {
        event: EventId::from_uuid(event),
        actor: AuthUserId::from_uuid(a),
        subject: AuthUserId::from_uuid(b),
        field: Field::SafetyInteraction,
    };
    let scope = Scope {
        actor: a,
        event,
        key: Uuid::new_v4(),
        operation: RegisteredOperation::find(if blocked {
            "blockConcertUser"
        } else {
            "unblockConcertUser"
        })
        .unwrap(),
    };
    let request = CanonicalRequest::json(
        scope,
        &[("eventId", &event.to_string()), ("userId", &b.to_string())],
        &[],
        None,
    )?;
    let policy = PgRelationshipPolicy::new(pool.clone(), RemoteAuthorityFixture);
    Ok(PgCommandRepository::new(pool.clone())
        .execute(
            &request,
            b,
            &keys(),
            &BlockCommand {
                policy: &policy,
                query: q,
                blocked,
            },
        )
        .await?)
}
async fn seed(f: &DatabaseFixture, u: AuthUserId) -> anyhow::Result<()> {
    sqlx::query(
        "insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')",
    )
    .bind(f.event.uuid())
    .bind(u.uuid())
    .execute(&f.pool)
    .await?;
    sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(f.event.uuid()).bind(u.uuid()).execute(&f.pool).await?;
    Ok(())
}
async fn client(
    address: std::net::SocketAddr,
    f: &DatabaseFixture,
    u: AuthUserId,
) -> anyhow::Result<(Client, Value)> {
    client_in_room(address, f, u, f.room).await
}
async fn client_in_room(
    address: std::net::SocketAddr,
    f: &DatabaseFixture,
    u: AuthUserId,
    room: Uuid,
) -> anyhow::Result<(Client, Value)> {
    let mut request = format!("ws://{address}/world-ws").into_client_request()?;
    request
        .headers_mut()
        .insert("origin", "http://localhost:9000".parse()?);
    let (mut c, _) = connect_async(request).await?;
    let (raw, _) = f.ticket(room, u).await?;
    authenticate_in_room(&mut c, f, &raw, room).await?;
    let first = value(&mut c).await?;
    Ok((c, first))
}
async fn authenticate(c: &mut Client, f: &DatabaseFixture, raw: &str) -> anyhow::Result<()> {
    authenticate_in_room(c, f, raw, f.room).await
}
async fn authenticate_in_room(
    c: &mut Client,
    f: &DatabaseFixture,
    raw: &str,
    room: Uuid,
) -> anyhow::Result<()> {
    c.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"authenticate","ticket":raw,"eventId":f.event.uuid(),"roomId":room,"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1}).to_string())).await?;
    Ok(())
}
async fn value(c: &mut Client) -> anyhow::Result<Value> {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(2), c.next())
            .await?
            .ok_or_else(|| anyhow::anyhow!("closed before frame"))??;
        if let Message::Text(raw) = frame {
            let v: Value = serde_json::from_str(&raw)?;
            if v["type"] != "ping" {
                return Ok(v);
            }
        }
    }
}
// A required second fanout leg is deliberately unavailable until this test
// explicitly records its own durable effect. It never makes World a global sink.
struct OtherLeg {
    pool: PgPool,
    fail: AtomicBool,
}
#[async_trait]
impl IdempotentInvalidationSink for OtherLeg {
    async fn deliver(&self, d: &Delivery) -> Result<(), DeliveryFailure> {
        if self.fail.load(Ordering::Acquire) {
            return Err(DeliveryFailure::Retryable);
        }
        sqlx::query(
            "insert into world_invalidation_test_other(signal) values($1) on conflict do nothing",
        )
        .bind(d.id)
        .execute(&self.pool)
        .await
        .map_err(|_| DeliveryFailure::Retryable)?;
        Ok(())
    }
}
async fn claimed(pool: &PgPool, receipt: Uuid) -> anyhow::Result<Delivery> {
    PgOutbox::new(pool.clone())
        .claim(100)
        .await?
        .into_iter()
        .find(|d| d.receipt == Some(receipt))
        .ok_or_else(|| anyhow::anyhow!("missing claimed delivery"))
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires marker-owned world_invalidation/native_smoke.py"]
async fn native_block_outbox_fence_commit_retry_and_native_send() -> anyhow::Result<()> {
    Box::pin(scenario()).await
}
async fn scenario() -> anyhow::Result<()> {
    let f = DatabaseFixture::new().await?;
    let b = AuthUserId::from_uuid(Uuid::new_v4());
    let c = AuthUserId::from_uuid(Uuid::new_v4());
    let observer = AuthUserId::from_uuid(Uuid::new_v4());
    seed(&f, b).await?;
    seed(&f, c).await?;
    seed(&f, observer).await?;
    sqlx::query("create table world_invalidation_test_other(signal uuid primary key)")
        .execute(&f.pool)
        .await?;
    let h = Harness::start(f).await?;
    let f = &h.f;
    let trace = &h.trace;
    let address = h.address;
    let (mut ca, sa) = client(address, &f, f.user).await?;
    let (mut cb, _) = client(address, &f, b).await?;
    let (mut cc, sc) = client_in_room(address, &f, c, f.target).await?;
    let (mut observer_client, _) = client(address, &f, observer).await?;
    assert_eq!(sa["type"], "snapshot");
    let bindings: Vec<Binding> = trace
        .sent
        .lock()
        .unwrap()
        .iter()
        .map(|(b, _, _)| b.clone())
        .collect();
    let old_a = bindings
        .iter()
        .find(|x| x.key.user_id == f.user)
        .unwrap()
        .clone();
    let old_b = bindings
        .iter()
        .find(|x| x.key.user_id == b)
        .unwrap()
        .clone();
    let old_c = bindings
        .iter()
        .find(|x| x.key.user_id == c)
        .unwrap()
        .clone();
    trace.paused.store(true, Ordering::Release);
    ca.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"input","streamId":sa["streamId"],"roomEpoch":sa["roomEpoch"],"seq":1,"axisX":1,"axisY":0,"buttons":[]}).to_string())).await?;
    tokio::time::sleep(Duration::from_millis(120)).await;
    assert!(
        trace.waits.load(Ordering::Acquire) > 0,
        "pending presence exists behind native readiness"
    );
    let receipt = block(&f.pool, f.event.uuid(), f.user.uuid(), b.uuid(), true).await?;
    let d = claimed(&f.pool, receipt.id).await?;
    assert!(
        CanonicalAuthorityPort::current(&f.bridge, &old_a, f.now()).is_ok(),
        "lease current before consumer"
    );
    let consumer = Arc::new(WorldInvalidationConsumer::new(f.bridge.clone()));
    let other = Arc::new(OtherLeg {
        pool: f.pool.clone(),
        fail: AtomicBool::new(true),
    });
    let composite = Arc::new(WorldCompositeSink::new(consumer.clone(), other.clone()));
    sqlx::raw_sql("create table world_invalidation_test_fault(signal uuid primary key,mode text not null); create function world_invalidation_test_fault() returns trigger language plpgsql as $$ declare m text; begin select mode into m from world_invalidation_test_fault where signal=NEW.signal_id; if m='FAIL' then raise exception 'owned fault'; elsif m='DELAY' then perform pg_sleep(0.3); end if; return NEW; end $$; create trigger world_invalidation_test_fault before insert on concert_world_invalidation_receipts for each row execute function world_invalidation_test_fault();").execute(&f.pool).await?;
    sqlx::query("insert into world_invalidation_test_fault values($1,'DELAY')")
        .bind(d.id)
        .execute(&f.pool)
        .await?;
    let task = {
        let sink = composite.clone();
        let d = d.clone();
        tokio::spawn(async move { sink.deliver(&d).await })
    };
    let began = Instant::now();
    loop {
        let delayed:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event='PgSleep')").fetch_one(&f.pool).await?;
        if delayed {
            break;
        }
        anyhow::ensure!(
            began.elapsed() < Duration::from_secs(1),
            "consumer never reached actual SQL commit delay"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(CanonicalAuthorityPort::current(&f.bridge, &old_a, f.now()).is_err());
    let uncommitted: i64 = sqlx::query_scalar(
        "select count(*) from concert_world_invalidation_receipts where signal_id=$1",
    )
    .bind(d.id)
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(
        uncommitted, 0,
        "no application acknowledgement before commit"
    );
    assert_eq!(task.await?, Err(DeliveryFailure::Retryable));
    assert!(CanonicalAuthorityPort::current(&f.bridge, &old_a, f.now()).is_err());
    assert!(CanonicalAuthorityPort::current(&f.bridge, &old_b, f.now()).is_err());
    assert!(
        CanonicalAuthorityPort::current(&f.bridge, &old_c, f.now()).is_ok(),
        "unrelated user retains grant"
    );
    let durable: i64 = sqlx::query_scalar(
        "select count(*) from concert_world_invalidation_receipts where signal_id=$1",
    )
    .bind(d.id)
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(durable, 1);
    let state: String = sqlx::query_scalar("select state from concert_outbox where outbox_id=$1")
        .bind(d.id)
        .fetch_one(&f.pool)
        .await?;
    assert_eq!(state, "LEASED", "other fanout failure cannot ack");
    other.fail.store(false, Ordering::Release);
    composite
        .deliver(&d)
        .await
        .map_err(|_| anyhow::anyhow!("composite retry failed"))?;
    assert!(PgOutbox::new(f.pool.clone()).acknowledge(&d).await?);
    let ack = Instant::now();
    trace.paused.store(false, Ordering::Release);
    cc.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"input","streamId":sc["streamId"],"roomEpoch":sc["roomEpoch"],"seq":1,"axisX":1,"axisY":0,"buttons":[]}).to_string())).await?;
    tokio::time::sleep(Duration::from_millis(160)).await;
    let sent = trace.sent.lock().unwrap().clone();
    for (binding, body, at) in &sent {
        if *at >= ack && matches!(body["type"].as_str(), Some("snapshot" | "delta")) {
            assert_ne!(binding.key.user_id, f.user);
            assert_ne!(binding.key.user_id, b);
            for p in body["players"]
                .as_array()
                .or_else(|| body["upserts"].as_array())
                .into_iter()
                .flatten()
            {
                assert_ne!(p["userId"], f.user.uuid().to_string());
                assert_ne!(p["userId"], b.uuid().to_string());
            }
        }
    }
    let seq: Vec<u64> = sent
        .iter()
        .filter(|(b, _, _)| b.key.user_id == c)
        .filter_map(|(_, v, _)| v["streamSeq"].as_u64())
        .collect();
    assert!(
        seq.len() >= 2,
        "unrelated room continues delivering native deltas"
    );
    assert!(seq.windows(2).all(|w| w[1] == w[0] + 1));
    assert_eq!(consumer.apply(&d).await, Err(ApplyError::StaleLease));
    let effect:i64=sqlx::query_scalar("select count(*) from concert_world_memberships where event_id=$1 and user_id in($2,$3) and not disconnected").bind(f.event.uuid()).bind(f.user.uuid()).bind(b.uuid()).fetch_one(&f.pool).await?;
    assert_eq!(effect, 0);
    // Persist a source/outbox identity for the separate post-restart process.
    sqlx::query("create table world_invalidation_test_restart(event uuid,room uuid,owner_id text,signal uuid,receipt uuid,user_id uuid,peer uuid)").execute(&f.pool).await?;
    sqlx::query("insert into world_invalidation_test_restart values($1,$2,$3,$4,$5,$6,$7)")
        .bind(f.event.uuid())
        .bind(f.room)
        .bind(&f.owner.process_id)
        .bind(d.id)
        .bind(receipt.id)
        .bind(f.user.uuid())
        .bind(b.uuid())
        .execute(&f.pool)
        .await?;
    // A crash after local application but before queue ack is a valid replay.
    sqlx::query("update concert_outbox set state='READY',terminal_at=null,available_at=clock_timestamp() where outbox_id=$1").bind(d.id).execute(&f.pool).await?;
    let _ = ca.close(None).await;
    let _ = cb.close(None).await;
    let _ = cc.close(None).await;
    let _ = observer_client.close(None).await;
    h.shutdown().await
}

struct Harness {
    f: DatabaseFixture,
    transport: SocketTransport,
    trace: Trace,
    address: std::net::SocketAddr,
    handle: Arc<RuntimeHandle>,
    owner: tokio::task::JoinHandle<ShutdownReport>,
    stop: tokio::sync::oneshot::Sender<()>,
    server: tokio::task::JoinHandle<()>,
}
impl Harness {
    async fn start(f: DatabaseFixture) -> anyhow::Result<Self> {
        let transport = SocketTransport::default();
        let trace = Trace {
            native: transport.clone(),
            paused: Arc::new(AtomicBool::new(false)),
            waits: Arc::new(AtomicUsize::new(0)),
            sent: Arc::new(Mutex::new(vec![])),
            hold_user: Arc::new(Mutex::new(None)),
            held: Arc::new(AtomicBool::new(false)),
            release: Arc::new(AtomicBool::new(false)),
        };
        let mut target = spec(f.target, 4);
        target.room_epoch = id(7);
        let runtime = WorldRuntime::new(
            f.event,
            Uuid::parse_str(&f.owner.process_id)?,
            f.anchor.clone(),
            vec![spec(f.room, 4), target],
            f.bridge.clone(),
            f.bridge.clone(),
            trace.clone(),
        )
        .unwrap();
        let (started, origin) = runtime.clock_origin();
        let (handle, owner) = runtime.spawn();
        let handle = Arc::new(handle);
        let state = WorldSocketState::new(
            OriginPolicy::new(vec!["http://localhost:9000".into()], true).unwrap(),
            transport.clone(),
            f.bridge.clone(),
            handle.clone(),
            started,
            origin,
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
        Ok(Self {
            f,
            transport,
            trace,
            address,
            handle,
            owner,
            stop,
            server,
        })
    }
    async fn shutdown(self) -> anyhow::Result<()> {
        self.transport.close_all(SafeClose::UNAVAILABLE);
        self.handle.request_stop();
        let _ = tokio::time::timeout(Duration::from_secs(2), self.owner).await??;
        let _ = self.stop.send(());
        tokio::time::timeout(Duration::from_secs(2), self.server).await??;
        self.f.pool.close().await;
        Ok(())
    }
}

async fn admit(f: &DatabaseFixture, user: AuthUserId) -> anyhow::Result<Binding> {
    let (_, intent) = f.ticket(f.room, user).await?;
    let b = AdmissionPort::admit(&*f.bridge, intent, Uuid::new_v4())
        .await
        .map_err(|_| anyhow::anyhow!("owned admission failed"))?;
    CanonicalAuthorityPort::install_pending(&f.bridge, &b, f.now())
        .map_err(|_| anyhow::anyhow!("owned install failed"))?;
    Ok(b)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires marker-owned world_invalidation/native_smoke.py after actual PostgreSQL restart"]
async fn native_restart_durable_receipt_preserves_fresh_connection() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let r = sqlx::query("select * from world_invalidation_test_restart")
        .fetch_one(&pool)
        .await?;
    let event: Uuid = r.get("event");
    let room: Uuid = r.get("room");
    let user: Uuid = r.get("user_id");
    let started = Instant::now();
    while sqlx::query_scalar::<_,bool>("select exists(select 1 from concert_world_owner_leases where event_id=$1 and lease_until>clock_timestamp())").bind(event).fetch_one(&pool).await? {
        anyhow::ensure!(started.elapsed()<Duration::from_secs(6),"old owner did not expire");
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    let f = DatabaseFixture::resume(pool, event, room, user, r.get("owner_id")).await?;
    let fresh = admit(&f, f.user).await?;
    let d = claimed(&f.pool, r.get("receipt")).await?;
    assert_eq!(d.id, r.get::<Uuid, _>("signal"));
    let ack = WorldInvalidationConsumer::new(f.bridge.clone())
        .apply(&d)
        .await
        .map_err(|e| anyhow::anyhow!("restart application {e:?}"))?;
    assert!(
        ack.duplicate,
        "dedup survives actual database and binary restart"
    );
    assert!(CanonicalAuthorityPort::current(&f.bridge, &fresh, f.now()).is_ok());
    let active: bool = sqlx::query_scalar(
        "select not disconnected from concert_world_memberships where event_id=$1 and user_id=$2",
    )
    .bind(event)
    .bind(user)
    .fetch_one(&f.pool)
    .await?;
    assert!(
        active,
        "persisted exact victims cannot revoke fresh connection"
    );
    assert!(PgOutbox::new(f.pool.clone()).acknowledge(&d).await?);
    f.pool.close().await;
    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires marker-owned world_invalidation/native_smoke.py"]
async fn native_failed_commit_retry_duplicate_reorder_and_metadata_fences() -> anyhow::Result<()> {
    let f = DatabaseFixture::new().await?;
    let peer = AuthUserId::from_uuid(Uuid::new_v4());
    seed(&f, peer).await?;
    let a = admit(&f, f.user).await?;
    let b = admit(&f, peer).await?;
    let unrelated = DatabaseFixture::new().await?;
    seed(&unrelated, f.user).await?;
    let unrelated_binding = admit(&unrelated, f.user).await?;
    let consumer = WorldInvalidationConsumer::new(f.bridge.clone());
    let receipt = block(&f.pool, f.event.uuid(), f.user.uuid(), peer.uuid(), true).await?;
    let d = claimed(&f.pool, receipt.id).await?;
    // Delivery structs are transport metadata, never authority by themselves.
    for bad in [
        Delivery {
            event: Uuid::new_v4(),
            ..d.clone()
        },
        Delivery {
            reference: SafeReference {
                revision: d.reference.revision + 1,
                ..d.reference
            },
            ..d.clone()
        },
        Delivery {
            receipt: Some(Uuid::new_v4()),
            ..d.clone()
        },
    ] {
        assert_eq!(consumer.apply(&bad).await, Err(ApplyError::Unbound));
        assert!(CanonicalAuthorityPort::current(&f.bridge, &a, f.now()).is_ok());
    }
    sqlx::query("insert into world_invalidation_test_fault values($1,'FAIL')")
        .bind(d.id)
        .execute(&f.pool)
        .await?;
    assert_eq!(consumer.apply(&d).await, Err(ApplyError::Unavailable));
    assert!(CanonicalAuthorityPort::current(&f.bridge, &a, f.now()).is_err());
    assert!(CanonicalAuthorityPort::current(&f.bridge, &b, f.now()).is_err());
    let active: i64 = sqlx::query_scalar(
        "select count(*) from concert_world_memberships where event_id=$1 and not disconnected",
    )
    .bind(f.event.uuid())
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(active, 2, "failed transaction rolls back SQL effects");
    let durable: i64 = sqlx::query_scalar(
        "select count(*) from concert_world_invalidation_receipts where signal_id=$1",
    )
    .bind(d.id)
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(durable, 0);
    let blocked = AdmissionPort::admit(
        &*f.bridge,
        f.ticket(f.room, f.user).await?.1,
        Uuid::new_v4(),
    )
    .await;
    assert!(
        blocked.is_err(),
        "failed commit retains removal fence against new publication"
    );
    let outbox = PgOutbox::new(f.pool.clone());
    assert!(outbox.fail(&d, DeliveryFailure::Retryable).await?);
    assert_eq!(consumer.apply(&d).await, Err(ApplyError::StaleLease));
    // Only the owned fixture advances retry eligibility; lease/attempt claiming is real.
    sqlx::query("update concert_outbox set available_at=clock_timestamp() where outbox_id=$1")
        .bind(d.id)
        .execute(&f.pool)
        .await?;
    sqlx::query("delete from world_invalidation_test_fault where signal=$1")
        .bind(d.id)
        .execute(&f.pool)
        .await?;
    let retry = claimed(&f.pool, receipt.id).await?;
    assert_ne!(retry.lease, d.lease);
    assert_eq!(retry.attempt, d.attempt + 1);
    let applied = consumer
        .apply(&retry)
        .await
        .map_err(|e| anyhow::anyhow!("retry {e:?}"))?;
    assert!(!applied.duplicate);
    let fresh_a = admit(&f, f.user).await?;
    let fresh_b = admit(&f, peer).await?;
    assert!(
        consumer
            .apply(&retry)
            .await
            .map_err(|e| anyhow::anyhow!("duplicate {e:?}"))?
            .duplicate
    );
    for x in [&fresh_a, &fresh_b] {
        assert!(CanonicalAuthorityPort::current(&f.bridge, x, f.now()).is_ok());
    }
    assert!(!outbox.acknowledge(&d).await?);
    assert!(outbox.acknowledge(&retry).await?);
    let r2 = block(&f.pool, f.event.uuid(), f.user.uuid(), peer.uuid(), false).await?;
    let r3 = block(&f.pool, f.event.uuid(), f.user.uuid(), peer.uuid(), true).await?;
    let r4 = block(&f.pool, f.event.uuid(), f.user.uuid(), peer.uuid(), false).await?;
    let batch = outbox.claim(100).await?;
    let find = |id| batch.iter().find(|d| d.receipt == Some(id)).unwrap();
    let newest = find(r4.id);
    assert!(
        !consumer
            .apply(newest)
            .await
            .map_err(|e| anyhow::anyhow!("newest {e:?}"))?
            .superseded
    );
    let fresh = admit(&f, f.user).await?;
    for d in [find(r3.id), find(r2.id)] {
        assert!(
            consumer
                .apply(d)
                .await
                .map_err(|e| anyhow::anyhow!("reordered {e:?}"))?
                .superseded
        );
        assert!(CanonicalAuthorityPort::current(&f.bridge, &fresh, f.now()).is_ok());
        assert!(outbox.acknowledge(d).await?);
    }
    assert!(outbox.acknowledge(newest).await?);
    assert!(
        CanonicalAuthorityPort::current(&unrelated.bridge, &unrelated_binding, unrelated.now())
            .is_ok(),
        "same user in unrelated event survives"
    );
    let r5 = block(&f.pool, f.event.uuid(), f.user.uuid(), peer.uuid(), true).await?;
    assert_eq!(r5.state, ReceiptState::Succeeded);
    let composite = Arc::new(WorldCompositeSink::new(
        Arc::new(consumer),
        Arc::new(OtherLeg {
            pool: f.pool.clone(),
            fail: AtomicBool::new(false),
        }),
    ));
    let batch = run_batch(&outbox, composite, 100).await?;
    assert_eq!(
        (batch.claimed, batch.acknowledged, batch.failed, batch.stale),
        (1, 1, 0, 0)
    );
    unrelated.pool.close().await;
    f.pool.close().await;
    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires marker-owned world_invalidation/native_smoke.py"]
async fn native_captured_credential_expiry_closes_without_renewal() -> anyhow::Result<()> {
    let h = Harness::start(DatabaseFixture::new().await?).await?;
    let f = &h.f;
    let mut request = format!("ws://{}/world-ws", h.address).into_client_request()?;
    request
        .headers_mut()
        .insert("origin", "http://localhost:9000".parse()?);
    let (mut c, _) = connect_async(request).await?;
    let access = Utc::now() + TimeDelta::milliseconds(400);
    let (raw, _) = f.ticket_expiring(f.room, f.user, access).await?;
    authenticate(&mut c, f, &raw).await?;
    let first = value(&mut c).await?;
    assert_eq!(first["type"], "snapshot");
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let frame = tokio::time::timeout_at(deadline.into(), c.next())
            .await?
            .ok_or_else(|| anyhow::anyhow!("missing expiry close"))??;
        match frame {
            Message::Close(Some(close)) => {
                assert_eq!(u16::from(close.code), 4401);
                break;
            }
            Message::Text(raw) => {
                let v: Value = serde_json::from_str(&raw)?;
                if v["type"] == "revoke" {
                    assert_eq!(v["reason"], "SESSION_EXPIRED");
                    break;
                }
            }
            _ => {}
        }
    }
    for (_, body, at) in h.trace.sent.lock().unwrap().iter() {
        if matches!(body["type"].as_str(), Some("snapshot" | "delta")) {
            assert!(
                *at < f.started
                    + Duration::from_millis((access - f.anchor.utc).num_milliseconds() as u64)
            );
        }
    }
    let _ = c.close(None).await;
    h.shutdown().await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires marker-owned controller to stop the actual PostgreSQL postmaster"]
async fn native_database_outage_never_acknowledges_and_expires_live_views() -> anyhow::Result<()> {
    let f = DatabaseFixture::new().await?;
    let peer = AuthUserId::from_uuid(Uuid::new_v4());
    seed(&f, peer).await?;
    let h = Harness::start(f).await?;
    let f = &h.f;
    let (mut ca, first) = client(h.address, f, f.user).await?;
    assert_eq!(first["type"], "snapshot");
    let receipt = block(&f.pool, f.event.uuid(), f.user.uuid(), peer.uuid(), true).await?;
    let d = claimed(&f.pool, receipt.id).await?;
    let root = std::path::PathBuf::from(std::env::var("WORLD_INVALIDATION_TEST_OWNER_ROOT")?);
    // Root/marker/inode/socket identity was verified by owned_pool before this handshake.
    std::fs::write(root.join("outage-ready"), b"ready")?;
    let began = Instant::now();
    while !root.join("outage-stopped").exists() {
        anyhow::ensure!(
            began.elapsed() < Duration::from_secs(8),
            "controller did not stop owned PostgreSQL"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let cutoff = Instant::now();
    let consumer = WorldInvalidationConsumer::new(f.bridge.clone());
    assert_eq!(consumer.apply(&d).await, Err(ApplyError::Unavailable));
    assert!(
        PgOutbox::new(f.pool.clone()).acknowledge(&d).await.is_err(),
        "database outage cannot ack"
    );
    // Consume through terminal; a five-second live lease never becomes an outage grant.
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        let frame = tokio::time::timeout_at(deadline.into(), ca.next())
            .await?
            .ok_or_else(|| anyhow::anyhow!("missing terminal"))??;
        match frame {
            Message::Text(raw) => {
                let v: Value = serde_json::from_str(&raw)?;
                if matches!(v["type"].as_str(), Some("revoke" | "resyncRequired")) {
                    break;
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
    assert!(h
        .trace
        .sent
        .lock()
        .unwrap()
        .iter()
        .all(|(_, v, at)| *at < cutoff + Duration::from_secs(5)
            || !matches!(v["type"].as_str(), Some("snapshot" | "delta"))));
    let _ = ca.close(None).await;
    h.shutdown().await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires marker-owned world_invalidation/native_smoke.py"]
async fn native_inflight_handoff_serializes_before_invalidation_ack() -> anyhow::Result<()> {
    let f = DatabaseFixture::new().await?;
    let peer = AuthUserId::from_uuid(Uuid::new_v4());
    seed(&f, peer).await?;
    let h = Harness::start(f).await?;
    let f = &h.f;
    let (mut ca, first) = client(h.address, f, f.user).await?;
    let (mut cb, _) = client(h.address, f, peer).await?;
    let receipt = block(&f.pool, f.event.uuid(), f.user.uuid(), peer.uuid(), true).await?;
    let d = claimed(&f.pool, receipt.id).await?;
    *h.trace.hold_user.lock().unwrap() = Some(f.user.uuid());
    let applying = Arc::new(AtomicBool::new(false));
    let acknowledged = Arc::new(AtomicBool::new(false));
    let task = {
        let held = h.trace.held.clone();
        let applying = applying.clone();
        let acknowledged = acknowledged.clone();
        let consumer = WorldInvalidationConsumer::new(f.bridge.clone());
        let d = d.clone();
        let runtime = tokio::runtime::Handle::current();
        std::thread::spawn(move || {
            let began = Instant::now();
            while !held.load(Ordering::Acquire) {
                assert!(began.elapsed() < Duration::from_secs(1));
                std::thread::sleep(Duration::from_millis(1));
            }
            applying.store(true, Ordering::Release);
            let result = runtime.block_on(consumer.apply(&d));
            if result.is_ok() {
                acknowledged.store(true, Ordering::Release);
            }
            result
        })
    };
    let releaser = {
        let held = h.trace.held.clone();
        let release = h.trace.release.clone();
        std::thread::spawn(move || {
            let began = Instant::now();
            while !held.load(Ordering::Acquire) {
                assert!(began.elapsed() < Duration::from_secs(1));
                std::thread::sleep(Duration::from_millis(1));
            }
            std::thread::sleep(Duration::from_millis(60));
            let serialized =
                applying.load(Ordering::Acquire) && !acknowledged.load(Ordering::Acquire);
            release.store(true, Ordering::Release);
            serialized
        })
    };
    ca.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"input","streamId":first["streamId"],"roomEpoch":first["roomEpoch"],"seq":1,"axisX":1,"axisY":0,"buttons":[]}).to_string())).await?;
    tokio::task::spawn_blocking(move || task.join().unwrap())
        .await?
        .map_err(|e| anyhow::anyhow!("race application {e:?}"))?;
    assert!(
        releaser.join().unwrap(),
        "local ack cannot pass an in-flight native handoff"
    );
    let ack = Instant::now();
    assert!(PgOutbox::new(f.pool.clone()).acknowledge(&d).await?);
    tokio::time::sleep(Duration::from_millis(120)).await;
    assert!(h
        .trace
        .sent
        .lock()
        .unwrap()
        .iter()
        .all(|(_, v, at)| *at < ack || !matches!(v["type"].as_str(), Some("snapshot" | "delta"))));
    let _ = ca.close(None).await;
    let _ = cb.close(None).await;
    h.shutdown().await
}
