//! Actual owner mutations -> guarded mTLS replay -> World native handoff and SQL.
//! Current-read services are actual guarded local HTTP fixtures. Map/art metadata
//! is synthetic. This does not qualify public root, product art or IN01.
use super::*;
use crate::{
    adapters::r#in::world_ws::{router, AdmissionPort, SocketTransport, WorldSocketState},
    world::{
        ingress::{OriginPolicy, SafeClose},
        membership::CanonicalAuthorityPort,
        runtime::{RuntimeHandle, ShutdownReport, TransportPort, WorldRuntime, WriteError},
        simulation::Binding,
    },
};
use anyhow::Context;
use auth_service::{
    adapters::out::{cache::session_store::PgRefreshTokenStore, runtime::SystemAuthRuntime},
    application::ports::RefreshTokenStorePort,
};
use concert_application::world_ticket::{WorldError, WorldSubject};
use friend_service::{
    adapters::out::persistence::follow_repository::PgFollowRepository,
    application::ports::{RelationshipPolicyRepositoryPort, UserLifecycleRepositoryPort},
};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Mutex,
};
use tokio::sync::oneshot;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
    MaybeTlsStream, WebSocketStream,
};

#[path = "world_fixture.rs"]
mod fixture;
#[path = "geometry.rs"]
mod geometry;
#[path = "owned.rs"]
mod owned;
#[path = "tls.rs"]
mod tls;
#[path = "trace.rs"]
mod trace;
type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

struct Pump {
    remote: Arc<tokio::sync::Mutex<RemoteInvalidationRuntime>>,
    stop: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<Result<(), ApplyError>>,
}
impl Pump {
    fn new(remote: RemoteInvalidationRuntime) -> Self {
        let remote = Arc::new(tokio::sync::Mutex::new(remote));
        let (stop, mut rx) = oneshot::channel();
        let r = remote.clone();
        let task = tokio::spawn(async move {
            let mut tick = tokio::time::interval(POLL);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {biased;
                    _=&mut rx=>return r.lock().await.shutdown().await,
                    _=tick.tick()=> {
                        while !r.lock().await.poll().await? {tokio::task::yield_now().await;}
                    }
                }
            }
        });
        Self { remote, stop, task }
    }
    async fn finish(self) -> anyhow::Result<()> {
        let _ = self.stop.send(());
        tokio::time::timeout(Duration::from_secs(2), self.task).await???;
        Ok(())
    }
}
struct Native {
    address: std::net::SocketAddr,
    trace: trace::Trace,
    transport: SocketTransport,
    handle: Arc<RuntimeHandle>,
    task: tokio::task::JoinHandle<ShutdownReport>,
    stop: oneshot::Sender<()>,
    server: tokio::task::JoinHandle<()>,
    authority_stop: oneshot::Sender<()>,
    authority: tokio::task::JoinHandle<anyhow::Result<()>>,
}
impl Native {
    async fn new(s: &fixture::Stack) -> anyhow::Result<Self> {
        Self::with_capacity(s, 2).await
    }
    async fn with_capacity(s: &fixture::Stack, capacity: usize) -> anyhow::Result<Self> {
        let transport = SocketTransport::default();
        let trace = trace::Trace {
            native: transport.clone(),
            paused: Arc::new(AtomicBool::new(false)),
            waits: Arc::new(AtomicUsize::new(0)),
            sent: Arc::new(Mutex::new(vec![])),
            hold_user: Arc::new(Mutex::new(None)),
            held: Arc::new(AtomicBool::new(false)),
            release: Arc::new(AtomicBool::new(false)),
        };
        let mut room = geometry::spec(s.room, capacity);
        room.room_epoch = s.room_epoch;
        let runtime = WorldRuntime::new(
            s.event,
            s.bridge.invalidation_context().1,
            s.anchor.clone(),
            vec![room],
            s.bridge.clone(),
            s.bridge.clone(),
            trace.clone(),
        )
        .unwrap();
        let (clock, origin) = runtime.clock_origin();
        let (handle, task) = runtime.spawn();
        let handle = Arc::new(handle);
        let state = WorldSocketState::new(
            OriginPolicy::new(vec!["http://localhost:9000".into()], true).unwrap(),
            transport.clone(),
            s.bridge.clone(),
            handle.clone(),
            clock,
            origin,
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let (stop, rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            axum::serve(listener, router(state))
                .with_graceful_shutdown(async {
                    let _ = rx.await;
                })
                .await
                .unwrap();
        });
        let (authority_stop, mut authority_rx) = oneshot::channel();
        let bridge = s.bridge.clone();
        let authority = tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(1));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {biased;
                    _=&mut authority_rx=>return Ok(()),
                    _=tick.tick()=>{tokio::time::timeout(Duration::from_secs(2),bridge.maintain()).await??;}
                }
            }
        });
        Ok(Self {
            address,
            trace,
            transport,
            handle,
            task,
            stop,
            server,
            authority_stop,
            authority,
        })
    }
    async fn connect(
        &mut self,
        s: &fixture::Stack,
        subject: &WorldSubject,
    ) -> anyhow::Result<(Socket, Binding, Value)> {
        let intent = fixture::ticket(
            &s.repo(),
            subject,
            s.room,
            chrono::Utc::now() + chrono::TimeDelta::seconds(40),
        )
        .await
        .context("issue unique native fixture ticket")?;
        let mut request = format!("ws://{}/world-ws", self.address).into_client_request()?;
        request
            .headers_mut()
            .insert("origin", "http://localhost:9000".parse()?);
        let (mut socket, _) = connect_async(request).await?;
        socket.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"authenticate","ticket":intent.ticket.expose_for_consumption(),"eventId":s.event.uuid(),"roomId":s.room,"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1}).to_string())).await?;
        let value = next_snapshot(&mut socket)
            .await
            .context("read real socket admission snapshot")?;
        let binding = self
            .trace
            .sent
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|(b, _, _)| {
                b.key == subject.key && b.session_id.to_string() == subject.session_id.as_str()
            })
            .ok_or_else(|| anyhow::anyhow!("native binding missing"))?
            .0
            .clone();
        Ok((socket, binding, value))
    }
    async fn finish(self, s: &fixture::Stack) -> anyhow::Result<()> {
        let _ = self.authority_stop.send(());
        tokio::time::timeout(Duration::from_secs(2), self.authority).await???;
        self.transport.close_all(SafeClose::UNAVAILABLE);
        self.handle.request_stop();
        drop(self.handle);
        let _ = self.stop.send(());
        tokio::time::timeout(Duration::from_secs(2), self.server).await??;
        let report = tokio::time::timeout(Duration::from_secs(2), self.task).await?;
        for b in report?.remaining_cleanup {
            let _ = CanonicalAuthorityPort::release(&s.bridge, &b);
        }
        anyhow::ensure!(s.bridge.cleanup().await? == 0, "native cleanup remains");
        Ok(())
    }
}
async fn next_snapshot(socket: &mut Socket) -> anyhow::Result<Value> {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match socket.next().await {
                Some(Ok(Message::Text(raw))) => {
                    let value: Value = serde_json::from_str(&raw)?;
                    if value["type"] == "snapshot" {
                        return Ok(value);
                    }
                }
                Some(Ok(Message::Close(frame))) => {
                    anyhow::bail!("socket closed before snapshot: {frame:?}")
                }
                None => anyhow::bail!("socket ended before snapshot"),
                Some(Err(e)) => return Err(e.into()),
                _ => {}
            }
        }
    })
    .await?
}
async fn position(pool: &PgPool, consumer: Uuid, owner: InvalidationOwner) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar("select sequence from concert_world_remote_checkpoints where consumer_id=$1 and source_owner=$2").bind(consumer).bind(super::super::remote::owner_name(owner)).fetch_one(pool).await?)
}
async fn applied(
    s: &fixture::Stack,
    source: &tls::Server,
    owner: InvalidationOwner,
    pump: &Pump,
) -> anyhow::Result<Instant> {
    let consumer = s.bridge.invalidation_context().1;
    let head = source
        .source(owner)?
        .read_head(
            consumer,
            &ReplayBudget::for_duration(Duration::from_secs(1), ReplayCancellation::default())?,
        )
        .await?
        .cursor();
    let began = Instant::now();
    while position(&s.concert, consumer, owner).await? < head.sequence() {
        anyhow::ensure!(
            began.elapsed() < Duration::from_secs(2),
            "real signal was not durably applied"
        );
        tokio::time::sleep(Duration::from_millis(3)).await;
    }
    let committed = Instant::now();
    while pump.remote.lock().await.readiness.snapshot().is_none() {
        anyhow::ensure!(
            began.elapsed() < Duration::from_secs(2),
            "applied cursor has not reached caught-up publication readiness"
        );
        tokio::time::sleep(Duration::from_millis(3)).await;
    }
    Ok(committed)
}
fn revoked(s: &fixture::Stack, b: &Binding) -> bool {
    CanonicalAuthorityPort::current(&s.bridge, b, s.now()).is_err()
}
fn no_private_after(native: &Native, after: Instant, bindings: &[Binding]) {
    for (b, v, at) in native.trace.sent.lock().unwrap().iter() {
        if *at > after && bindings.contains(b) {
            assert!(
                !matches!(v["type"].as_str(), Some("snapshot" | "delta")),
                "private handoff after durable application"
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires marker-owned remote controller, actual four DBs and mTLS"]
async fn native_pair_block_unblock_deletion_fences_two_real_sockets() -> anyhow::Result<()> {
    let s = fixture::Stack::new().await?;
    // Match this three-user native test's room metadata before any ticket or
    // publication permit exists. Other native fixtures retain capacity two.
    let changed =
        sqlx::query("update concert_world_rooms set capacity=3 where event_id=$1 and room_id=$2")
            .bind(s.event.uuid())
            .bind(s.room)
            .execute(&s.concert)
            .await?
            .rows_affected();
    assert_eq!(changed, 1);
    let peer = s.add_user().await?;
    let unrelated = s.add_user().await?;
    let servers = tls::Servers::new(&s).await?;
    let mut remote = servers.runtime(&s)?;
    assert!(
        s.bridge.startup_ready().is_err(),
        "unbootstrapped replay is not ready"
    );
    remote.bootstrap().await?;
    s.bridge.claim_rooms().await?;
    while !remote.poll().await? {}
    s.bridge.startup_ready()?;
    let pump = Pump::new(remote);
    let mut native = Native::with_capacity(&s, 3).await?;
    let (a, ba, _) = native
        .connect(&s, &s.subject)
        .await
        .context("initial actor admission")?;
    let (b, bb, _) = native
        .connect(&s, &peer)
        .await
        .context("initial peer admission")?;
    let (observer, observer_binding, _) = native.connect(&s, &unrelated).await?;
    s.bridge.maintain().await?;
    let repository = PgFollowRepository::new(s.friend.clone());
    repository
        .block(
            s.subject.key.user_id.uuid(),
            peer.key.user_id.uuid(),
            chrono::Utc::now(),
        )
        .await?;
    let ack = applied(&s, &servers.friend, InvalidationOwner::Friend, &pump).await?;
    assert!(revoked(&s, &ba) && revoked(&s, &bb));
    assert!(
        !revoked(&s, &observer_binding),
        "pair scope preserves unrelated user: {}",
        s.bridge.native_view_state(&observer_binding, s.now())
    );
    // Rebuild the surviving observer's own bounded permit after source catch-up;
    // a checkpoint alone never grants it. Its same exact stream must survive.
    s.bridge.maintain().await?;
    assert!(!revoked(&s, &observer_binding));
    assert!(CanonicalAuthorityPort::publication_ready(
        &s.bridge,
        &observer_binding
    ));
    let began = Instant::now();
    loop {
        let sent = native.trace.sent.lock().unwrap();
        let observer_frames: Vec<_> = sent
            .iter()
            .filter(|(binding, value, at)| {
                binding == &observer_binding
                    && *at > ack
                    && matches!(value["type"].as_str(), Some("snapshot" | "delta"))
            })
            .collect();
        if !observer_frames.is_empty() {
            for (_, value, _) in observer_frames {
                for player in value["players"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .chain(value["upserts"].as_array().into_iter().flatten())
                {
                    assert_ne!(player["userId"], json!(s.subject.key.user_id.uuid()));
                    assert_ne!(player["userId"], json!(peer.key.user_id.uuid()));
                }
            }
            break;
        }
        drop(sent);
        anyhow::ensure!(
            began.elapsed() < Duration::from_secs(1),
            "surviving observer stopped publishing"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    no_private_after(&native, ack, &[ba, bb]);
    drop(a);
    drop(b);
    let (a, ba, _) = native
        .connect(&s, &s.subject)
        .await
        .context("blocked actor admission")?;
    let (b, bb, snapshot) = native
        .connect(&s, &peer)
        .await
        .context("blocked peer admission")?;
    assert_eq!(
        snapshot["players"].as_array().unwrap().len(),
        2,
        "blocked opposite user absent from actual fresh snapshot"
    );
    repository
        .unblock(s.subject.key.user_id.uuid(), peer.key.user_id.uuid())
        .await?;
    let ack = applied(&s, &servers.friend, InvalidationOwner::Friend, &pump).await?;
    assert!(revoked(&s, &ba) && revoked(&s, &bb));
    assert!(
        !revoked(&s, &observer_binding),
        "pair scope preserves unrelated user: {}",
        s.bridge.native_view_state(&observer_binding, s.now())
    );
    // Rebuild the surviving observer's own bounded permit after source catch-up;
    // a checkpoint alone never grants it. Its same exact stream must survive.
    s.bridge.maintain().await?;
    assert!(!revoked(&s, &observer_binding));
    assert!(CanonicalAuthorityPort::publication_ready(
        &s.bridge,
        &observer_binding
    ));
    no_private_after(&native, ack, &[ba, bb]);
    drop(a);
    drop(b);
    let (a, ba, _) = native
        .connect(&s, &s.subject)
        .await
        .context("unblocked actor admission")?;
    assert!(
        !revoked(&s, &ba),
        "fresh unblocked actor remains current before peer admission: {}",
        s.bridge.native_view_state(&ba, s.now())
    );
    let (b, bb, snapshot) = native
        .connect(&s, &peer)
        .await
        .context("unblocked peer admission")?;
    assert!(
        !revoked(&s, &ba),
        "peer admission cannot revoke fresh unblocked actor: {}",
        s.bridge.native_view_state(&ba, s.now())
    );
    assert_eq!(
        snapshot["players"].as_array().unwrap().len(),
        3,
        "unblock permits visibility only through fresh real owning reads: actor {} peer {}",
        s.bridge.native_view_state(&ba, s.now()),
        s.bridge.native_view_state(&bb, s.now())
    );
    // This identity has no historical block edge, so its actual deletion emits
    // only user scope and must preserve both unrelated members.
    repository
        .apply_user_deletion(
            Uuid::new_v4(),
            unrelated.key.user_id.uuid(),
            s.subject.key.user_id.uuid(),
            chrono::Utc::now() + chrono::TimeDelta::days(30),
        )
        .await?;
    let ack = applied(&s, &servers.friend, InvalidationOwner::Friend, &pump).await?;
    assert!(revoked(&s, &observer_binding));
    assert!(!revoked(&s, &ba) && !revoked(&s, &bb));
    no_private_after(&native, ack, &[observer_binding]);
    drop(observer);
    let consumer = s.bridge.invalidation_context().1;
    let source = servers.friend.source(InvalidationOwner::Friend)?;
    let before = source
        .read_head(
            consumer,
            &ReplayBudget::for_duration(Duration::from_secs(1), ReplayCancellation::default())?,
        )
        .await?
        .cursor();
    repository
        .apply_user_deletion(
            Uuid::new_v4(),
            peer.key.user_id.uuid(),
            s.subject.key.user_id.uuid(),
            chrono::Utc::now() + chrono::TimeDelta::days(30),
        )
        .await?;
    applied(&s, &servers.friend, InvalidationOwner::Friend, &pump).await?;
    // Account deletion also updates its historical user_blocks row. Read the
    // real sealed journal page: this is a pair removal as well as a user removal.
    let page = source
        .read_after(
            consumer,
            before,
            256,
            &ReplayBudget::for_duration(Duration::from_secs(1), ReplayCancellation::default())?,
        )
        .await?;
    use crate::adapters::out::relationship_authority::invalidation::SignalScope;
    assert!(page.signals().iter().any(|signal| matches!(signal.scope(),
        SignalScope::User { user } if user == peer.key.user_id.uuid())));
    assert!(page.signals().iter().any(|signal| matches!(signal.scope(),
        SignalScope::Pair { actor, subject }
        if [actor, subject].contains(&s.subject.key.user_id.uuid())
            && [actor, subject].contains(&peer.key.user_id.uuid()))));
    assert!(revoked(&s, &ba) && revoked(&s, &bb));
    drop(a);
    drop(b);
    pump.finish().await?;
    native.finish(&s).await?;
    servers.finish().await?;
    s.finish().await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires marker-owned remote controller, actual four DBs and mTLS"]
async fn native_bootstrap_fences_old_socket_before_head_and_requires_fresh_reads(
) -> anyhow::Result<()> {
    let s = fixture::Stack::new().await?;
    let mut native = Native::new(&s).await?;
    let (old, binding, _) = native.connect(&s, &s.subject).await?;
    assert!(!revoked(&s, &binding));
    let servers = tls::Servers::new(&s).await?;
    let mut remote = servers.runtime(&s)?;
    assert!(
        !CanonicalAuthorityPort::publication_ready(&s.bridge, &binding),
        "constructor gate suppresses the old view before head RPC"
    );
    remote.bootstrap().await?;
    assert!(revoked(&s, &binding));
    let disconnected: bool = sqlx::query_scalar(
        "select disconnected from concert_world_memberships where event_id=$1 and user_id=$2",
    )
    .bind(s.event.uuid())
    .bind(s.subject.key.user_id.uuid())
    .fetch_one(&s.concert)
    .await?;
    assert!(disconnected);
    let pump = Pump::new(remote);
    drop(old);
    let (fresh, new, _) = native.connect(&s, &s.subject).await?;
    assert_ne!(new.connection_id, binding.connection_id);
    assert!(!revoked(&s, &new));
    drop(fresh);
    pump.finish().await?;
    native.finish(&s).await?;
    servers.finish().await?;
    s.finish().await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires marker-owned remote controller, actual four DBs and mTLS"]
async fn native_auth_family_user_and_cross_event_scopes_preserve_other_generations(
) -> anyhow::Result<()> {
    use auth_service::{
        adapters::out::persistence::user_repository::PgUserRepository,
        application::ports::UserRepositoryPort,
    };
    let consumer = Uuid::new_v4();
    let first = fixture::Stack::with_owner(consumer).await?;
    let second = fixture::Stack::with_owner(consumer).await?;
    let other_family = second.add_family(first.subject.key.user_id).await?;
    let servers = tls::Servers::new(&first).await?;
    let mut remote = RemoteInvalidationRuntime::new(
        first.concert.clone(),
        consumer,
        servers.auth.source(InvalidationOwner::Auth)?,
        servers.friend.source(InvalidationOwner::Friend)?,
        vec![first.bridge.clone(), second.bridge.clone()],
    )?;
    remote.bootstrap().await?;
    let pump = Pump::new(remote);
    let mut one = Native::new(&first).await?;
    let mut two = Native::new(&second).await?;
    let (a, ba, _) = one.connect(&first, &first.subject).await?;
    let (b, bb, _) = two.connect(&second, &other_family).await?;
    let (unrelated, bu, _) = two.connect(&second, &second.subject).await?;
    assert!(
        !revoked(&second, &bb),
        "second ticket/admission must preserve the existing other-family socket: {}",
        second.bridge.native_view_state(&bb, second.now())
    );
    let store = PgRefreshTokenStore::new(first.auth.clone(), Arc::new(SystemAuthRuntime));
    store
        .revoke_family(Uuid::parse_str(first.subject.session_id.as_str())?)
        .await?;
    let ack = applied(&first, &servers.auth, InvalidationOwner::Auth, &pump).await?;
    assert!(revoked(&first, &ba));
    assert!(
        !revoked(&second, &bb),
        "other family survives the exact family signal: {}",
        second.bridge.native_view_state(&bb, second.now())
    );
    assert!(
        !revoked(&second, &bu),
        "unrelated user survives the exact family signal"
    );
    no_private_after(&one, ack, &[ba]);
    drop(a);
    let replacement = first.add_family(first.subject.key.user_id).await?;
    applied(&first, &servers.auth, InvalidationOwner::Auth, &pump).await?;
    let (a, ba, _) = one.connect(&first, &replacement).await?;
    PgUserRepository::new(first.auth.clone(), Arc::new(SystemAuthRuntime))
        .update_password_hash(
            first.subject.key.user_id.uuid(),
            "changed-private-runtime-fixture",
        )
        .await?;
    let ack = applied(&first, &servers.auth, InvalidationOwner::Auth, &pump).await?;
    assert!(
        revoked(&first, &ba) && revoked(&second, &bb),
        "user-scoped credentials signal crosses local events and families"
    );
    assert!(!revoked(&second, &bu));
    no_private_after(&one, ack, &[ba]);
    no_private_after(&two, ack, &[bb]);
    drop(a);
    drop(b);
    // Actual credentials changes revoke every old family, not just its view.
    assert!(one.connect(&first, &replacement).await.is_err());
    let replacement = first.add_family(first.subject.key.user_id).await?;
    applied(&first, &servers.auth, InvalidationOwner::Auth, &pump).await?;
    let (a, ba, _) = one.connect(&first, &replacement).await?;
    sqlx::query("update users set status='SUSPENDED' where id=$1")
        .bind(first.subject.key.user_id.uuid())
        .execute(&first.auth)
        .await?;
    applied(&first, &servers.auth, InvalidationOwner::Auth, &pump).await?;
    assert!(revoked(&first, &ba));
    assert!(!revoked(&second, &bu));
    drop(a);
    assert!(
        one.connect(&first, &replacement).await.is_err(),
        "current typed Auth denies a suspended user's fresh socket"
    );
    sqlx::query("update users set status='ACTIVE' where id=$1")
        .bind(first.subject.key.user_id.uuid())
        .execute(&first.auth)
        .await?;
    applied(&first, &servers.auth, InvalidationOwner::Auth, &pump).await?;
    assert!(
        one.connect(&first, &replacement).await.is_err(),
        "restoring ACTIVE does not restore a revoked family"
    );
    let replacement = first.add_family(first.subject.key.user_id).await?;
    applied(&first, &servers.auth, InvalidationOwner::Auth, &pump).await?;
    let (a, ba, _) = one.connect(&first, &replacement).await?;
    sqlx::query("delete from users where id=$1")
        .bind(first.subject.key.user_id.uuid())
        .execute(&first.auth)
        .await?;
    applied(&first, &servers.auth, InvalidationOwner::Auth, &pump).await?;
    assert!(revoked(&first, &ba));
    assert!(!revoked(&second, &bu));
    drop(a);
    drop(unrelated);
    pump.finish().await?;
    one.finish(&first).await?;
    two.finish(&second).await?;
    servers.finish().await?;
    first.finish().await?;
    second.finish().await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires marker-owned remote controller, actual four DBs and mTLS"]
async fn native_checkpoint_rollback_retry_and_duplicate_preserve_fresh_connection(
) -> anyhow::Result<()> {
    let s = fixture::Stack::new().await?;
    let peer = s.add_user().await?;
    let servers = tls::Servers::new(&s).await?;
    let mut remote = servers.runtime(&s)?;
    remote.bootstrap().await?;
    let pump = Pump::new(remote);
    let mut native = Native::new(&s).await?;
    let (socket, old, _) = native.connect(&s, &s.subject).await?;
    // Serialize against the pump and use only a real sealed response from the
    // actual guarded owner. This is not an unchecked signal constructor.
    let mut r = pump.remote.lock().await;
    let before = r.cursors[1].unwrap();
    PgFollowRepository::new(s.friend.clone())
        .block(
            s.subject.key.user_id.uuid(),
            peer.key.user_id.uuid(),
            chrono::Utc::now(),
        )
        .await?;
    let budget = r.budget()?;
    let page = r.sources[1]
        .read_after(r.consumer, before, 256, &budget)
        .await?;
    assert!(!page.signals().is_empty());
    sqlx::query("create function world_remote_injected_receipt_failure() returns trigger language plpgsql as $$ begin raise exception 'owned fixture failure'; end $$").execute(&s.concert).await?;
    sqlx::query("create trigger world_remote_injected_receipt_failure before insert on concert_world_remote_effects for each row execute function world_remote_injected_receipt_failure()").execute(&s.concert).await?;
    assert!(r.apply_page(before, &page, &r.budget()?).await.is_err());
    assert!(revoked(&s, &old));
    assert_eq!(
        position(&s.concert, r.consumer, InvalidationOwner::Friend).await?,
        before.sequence(),
        "native effects cannot be rolled back into a grant; SQL cursor stays put"
    );
    let disconnected: bool = sqlx::query_scalar(
        "select disconnected from concert_world_memberships where event_id=$1 and user_id=$2",
    )
    .bind(s.event.uuid())
    .bind(s.subject.key.user_id.uuid())
    .fetch_one(&s.concert)
    .await?;
    assert!(
        !disconnected,
        "receipt failure rolls back SQL disconnection with cursor"
    );
    sqlx::query(
        "drop trigger world_remote_injected_receipt_failure on concert_world_remote_effects",
    )
    .execute(&s.concert)
    .await?;
    sqlx::query("drop function world_remote_injected_receipt_failure()")
        .execute(&s.concert)
        .await?;
    let next = r.apply_page(before, &page, &r.budget()?).await?;
    r.cursors[1] = Some(next);
    assert_eq!(next, page.next());
    assert!(r.poll().await?);
    drop(r);
    drop(socket);
    let (fresh, new, _) = native.connect(&s, &s.subject).await?;
    assert_ne!(old.connection_id, new.connection_id);
    let mut r = pump.remote.lock().await;
    assert_eq!(r.apply_page(before, &page, &r.budget()?).await?, next);
    assert!(
        !revoked(&s, &new),
        "duplicate receipt must use exact old victims, not a user-scope removal"
    );
    assert!(r.poll().await?);
    drop(r);
    drop(fresh);
    pump.finish().await?;
    native.finish(&s).await?;
    servers.finish().await?;
    s.finish().await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires marker-owned remote controller, actual four DBs and mTLS"]
async fn native_remote_checkpoint_waits_for_actual_inflight_socket_handoff() -> anyhow::Result<()> {
    let s = fixture::Stack::new().await?;
    let peer = s.add_user().await?;
    let servers = tls::Servers::new(&s).await?;
    let mut remote = servers.runtime(&s)?;
    remote.bootstrap().await?;
    let consumer = remote.consumer;
    let before = remote.cursors[1].unwrap().sequence();
    let gate = remote.readiness.clone();
    let pump = Pump::new(remote);
    let mut native = Native::new(&s).await?;
    let (mut socket, binding, snapshot) = native.connect(&s, &s.subject).await?;
    *native.trace.hold_user.lock().unwrap() = Some(s.subject.key.user_id.uuid());
    let held = native.trace.held.clone();
    let release = native.trace.release.clone();
    let actor = s.subject.key.user_id.uuid();
    let subject = peer.key.user_id.uuid();
    let thread = std::thread::spawn(move || -> anyhow::Result<()> {
        let start = Instant::now();
        while !held.load(Ordering::Acquire) {
            anyhow::ensure!(
                start.elapsed() < Duration::from_secs(1),
                "actual native handoff did not hold"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        let result = (|| -> anyhow::Result<()> {
            // This observer owns its reactor and connections. It must continue
            // measuring SQL while the app workers wait on the held native lock.
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            let (pool, friend) = rt.block_on(async {
                Ok::<_, anyhow::Error>((
                    owned::pool("concert").await?,
                    owned::pool("friend").await?,
                ))
            })?;
            rt.block_on(PgFollowRepository::new(friend.clone()).block(
                actor,
                subject,
                chrono::Utc::now(),
            ))?;
            let start = Instant::now();
            while gate.snapshot().is_some() {
                anyhow::ensure!(
                    start.elapsed() < Duration::from_millis(700),
                    "known signal did not suppress publication"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
            assert_eq!(
                rt.block_on(position(&pool, consumer, InvalidationOwner::Friend))?,
                before,
                "cursor cannot pass the actual in-flight native mutex"
            );
            std::thread::sleep(Duration::from_millis(35));
            assert_eq!(
                rt.block_on(position(&pool, consumer, InvalidationOwner::Friend))?,
                before
            );
            rt.block_on(async {
                pool.close().await;
                friend.close().await;
            });
            Ok(())
        })();
        release.store(true, Ordering::Release);
        result
    });
    socket.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"input","streamId":snapshot["streamId"],"roomEpoch":snapshot["roomEpoch"],"seq":1,"axisX":1,"axisY":0,"buttons":[]}).to_string())).await?;
    tokio::task::spawn_blocking(move || thread.join().unwrap()).await??;
    let ack = applied(&s, &servers.friend, InvalidationOwner::Friend, &pump).await?;
    assert!(revoked(&s, &binding));
    tokio::time::sleep(Duration::from_millis(80)).await;
    no_private_after(&native, ack, &[binding]);
    drop(socket);
    pump.finish().await?;
    native.finish(&s).await?;
    servers.finish().await?;
    s.finish().await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires marker-owned remote controller, actual four DBs and mTLS"]
async fn native_bad_query_epoch_gap_reorder_conflict_timeout_close_publication(
) -> anyhow::Result<()> {
    let s = fixture::Stack::new().await?;
    let peer = s.add_user().await?;
    let mode = Arc::new(AtomicUsize::new(0));
    let auth = tls::Server::corruptible_auth(s.auth.clone(), mode.clone()).await?;
    let friend = tls::Server::new(InvalidationOwner::Friend, s.friend.clone()).await?;
    let mut remote = RemoteInvalidationRuntime::new(
        s.concert.clone(),
        s.bridge.invalidation_context().1,
        auth.source(InvalidationOwner::Auth)?,
        friend.source(InvalidationOwner::Friend)?,
        vec![s.bridge.clone()],
    )?;
    remote.bootstrap().await?;
    let mut native = Native::new(&s).await?;
    for corruption in 1..=7 {
        let (socket, binding, _) = native.connect(&s, &s.subject).await?;
        let before = position(&s.concert, remote.consumer, InvalidationOwner::Auth).await?;
        // Both records are genuine source mutations. The negative fixture
        // corrupts only their guarded response after the owning consistent read.
        for state in ["SUSPENDED", "ACTIVE"] {
            sqlx::query("update users set status=$2 where id=$1")
                .bind(peer.key.user_id.uuid())
                .bind(state)
                .execute(&s.auth)
                .await?;
        }
        mode.store(corruption, Ordering::Release);
        let began = Instant::now();
        assert!(
            remote.poll().await.is_err(),
            "corrupt response {corruption} became catch-up"
        );
        assert!(began.elapsed() < Duration::from_millis(1400));
        assert!(remote.readiness.snapshot().is_none());
        assert!(revoked(&s, &binding));
        assert_eq!(
            position(&s.concert, remote.consumer, InvalidationOwner::Auth).await?,
            before
        );
        drop(socket);
        mode.store(0, Ordering::Release);
        remote.bootstrap().await?;
        assert!(
            remote.readiness.snapshot().is_some(),
            "explicit resync fully invalidates old authority before actual replay"
        );
    }
    remote.shutdown().await?;
    drop(remote);
    native.finish(&s).await?;
    auth.finish().await?;
    friend.finish().await?;
    s.finish().await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires marker-owned remote controller, actual four DBs and mTLS"]
async fn native_owner_outage_stops_existing_socket_and_preserves_checkpoint() -> anyhow::Result<()>
{
    let s = fixture::Stack::new().await?;
    let servers = tls::Servers::new(&s).await?;
    let mut remote = servers.runtime(&s)?;
    remote.bootstrap().await?;
    let mut native = Native::new(&s).await?;
    let (socket, binding, _) = native.connect(&s, &s.subject).await?;
    let before = position(&s.concert, remote.consumer, InvalidationOwner::Auth).await?;
    let tls::Servers { auth, friend } = servers;
    auth.finish().await?;
    let began = Instant::now();
    assert!(remote.poll().await.is_err());
    assert!(began.elapsed() < Duration::from_millis(1400));
    assert!(remote.readiness.snapshot().is_none());
    assert!(revoked(&s, &binding));
    assert_eq!(
        position(&s.concert, remote.consumer, InvalidationOwner::Auth).await?,
        before
    );
    assert!(
        remote.poll().await.is_err(),
        "no automatic ready retry after loss of established owner"
    );
    drop(socket);
    remote.shutdown().await?;
    drop(remote);
    native.finish(&s).await?;
    friend.finish().await?;
    s.finish().await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires marker-owned remote controller, actual four DBs and mTLS"]
async fn native_backlog_never_admits_between_bounded_applied_pages() -> anyhow::Result<()> {
    let s = fixture::Stack::new().await?;
    let peer = s.add_user().await?;
    let servers = tls::Servers::new(&s).await?;
    let mut remote = servers.runtime(&s)?;
    remote.bootstrap().await?;
    let pump = Pump::new(remote);
    let mut native = Native::new(&s).await?;
    let (socket, binding, _) = native.connect(&s, &s.subject).await?;
    let mut r = pump.remote.lock().await;
    let after = r.cursors[1].unwrap();
    let auth_before = r.cursors[0].unwrap();
    let repo = PgFollowRepository::new(s.friend.clone());
    repo.block(
        s.subject.key.user_id.uuid(),
        peer.key.user_id.uuid(),
        chrono::Utc::now(),
    )
    .await?;
    for _ in 0..259 {
        repo.block(
            s.subject.key.user_id.uuid(),
            Uuid::new_v4(),
            chrono::Utc::now(),
        )
        .await?;
    }
    let budget = r.budget()?;
    let page = r.sources[1]
        .read_after(r.consumer, after, 256, &budget)
        .await?;
    assert_eq!(page.signals().len(), 256);
    assert!(page.has_more());
    let next = r.apply_page(after, &page, &budget).await?;
    r.cursors[1] = Some(next);
    assert_eq!(next.sequence(), after.sequence() + 256);
    assert_eq!(
        r.cursors[0],
        Some(auth_before),
        "Friend checkpoint cannot advance Auth"
    );
    assert!(r.readiness.snapshot().is_none());
    assert!(revoked(&s, &binding));
    let intent = fixture::ticket(
        &s.repo(),
        &s.subject,
        s.room,
        chrono::Utc::now() + chrono::TimeDelta::seconds(30),
    )
    .await?;
    assert_eq!(
        s.bridge.admit(intent, Uuid::new_v4()).await.unwrap_err(),
        SafeClose::UNAVAILABLE
    );
    assert!(r.poll().await?);
    assert!(r.readiness.snapshot().is_some());
    assert_eq!(r.cursors[1].unwrap().sequence(), after.sequence() + 260);
    drop(r);
    drop(socket);
    let (fresh, _, _) = native.connect(&s, &s.subject).await?;
    drop(fresh);
    pump.finish().await?;
    native.finish(&s).await?;
    servers.finish().await?;
    s.finish().await
}

async fn effects(pool: &PgPool, consumer: Uuid, auth: i64, friend: i64) -> anyhow::Result<Value> {
    Ok(sqlx::query_scalar("select coalesce(jsonb_agg(to_jsonb(e) order by source_owner,sequence),'[]'::jsonb) from concert_world_remote_effects e where consumer_id=$1 and ((source_owner='AUTH' and sequence<=$2) or (source_owner='FRIEND' and sequence<=$3))").bind(consumer).bind(auth).bind(friend).fetch_one(pool).await?)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires marker-owned remote controller before physical PostgreSQL restart"]
async fn native_restart_prepare_durable_effects_and_owner_positions() -> anyhow::Result<()> {
    use auth_service::{
        adapters::out::persistence::user_repository::PgUserRepository,
        application::ports::UserRepositoryPort,
    };
    let s = fixture::Stack::new().await?;
    let peer = s.add_user().await?;
    let servers = tls::Servers::new(&s).await?;
    let mut remote = servers.runtime(&s)?;
    remote.bootstrap().await?;
    let pump = Pump::new(remote);
    let mut native = Native::new(&s).await?;
    let (socket, old, _) = native.connect(&s, &s.subject).await?;
    PgFollowRepository::new(s.friend.clone())
        .block(
            s.subject.key.user_id.uuid(),
            peer.key.user_id.uuid(),
            chrono::Utc::now(),
        )
        .await?;
    applied(&s, &servers.friend, InvalidationOwner::Friend, &pump).await?;
    assert!(revoked(&s, &old));
    drop(socket);
    let (socket, old, _) = native.connect(&s, &s.subject).await?;
    PgUserRepository::new(s.auth.clone(), Arc::new(SystemAuthRuntime))
        .update_password_hash(s.subject.key.user_id.uuid(), "restart-private-fixture")
        .await?;
    applied(&s, &servers.auth, InvalidationOwner::Auth, &pump).await?;
    assert!(revoked(&s, &old));
    drop(socket);
    let r = pump.remote.lock().await;
    let ac = r.cursors[0].unwrap();
    let fc = r.cursors[1].unwrap();
    let data = json!({"consumer":r.consumer,"auth_epoch":ac.epoch(),"auth_sequence":ac.sequence(),"friend_epoch":fc.epoch(),"friend_sequence":fc.sequence(),"effects":effects(&s.concert,r.consumer,ac.sequence(),fc.sequence()).await?,"event":s.event.uuid(),"user":s.subject.key.user_id.uuid(),"family":s.subject.session_id.as_str(),"room":s.room,"target":s.target,"room_epoch":s.room_epoch});
    assert!(data["effects"].as_array().unwrap().len() >= 2);
    sqlx::query("create table world_remote_restart_fixture(singleton bool primary key check(singleton),data jsonb not null)").execute(&s.concert).await?;
    sqlx::query("insert into world_remote_restart_fixture values(true,$1)")
        .bind(data)
        .execute(&s.concert)
        .await?;
    drop(r);
    pump.finish().await?;
    native.finish(&s).await?;
    servers.finish().await?;
    s.finish().await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires marker-owned remote controller after physical PostgreSQL restart"]
async fn native_restart_resumes_durable_cursors_and_replays_intervening_mutations(
) -> anyhow::Result<()> {
    let pool = owned::pool("concert").await?;
    let data: Value =
        sqlx::query_scalar("select data from world_remote_restart_fixture where singleton")
            .fetch_one(&pool)
            .await?;
    let consumer = Uuid::parse_str(data["consumer"].as_str().unwrap())?;
    let auth = data["auth_sequence"].as_i64().unwrap();
    let friend = data["friend_sequence"].as_i64().unwrap();
    assert_eq!(
        effects(&pool, consumer, auth, friend).await?,
        data["effects"],
        "immutable local victim receipts survive actual postmaster restart"
    );
    assert_eq!(
        position(&pool, consumer, InvalidationOwner::Auth).await?,
        auth
    );
    assert_eq!(
        position(&pool, consumer, InvalidationOwner::Friend).await?,
        friend
    );
    pool.close().await;
    let event =
        concert_domain::EventId::from_uuid(Uuid::parse_str(data["event"].as_str().unwrap())?);
    let subject = WorldSubject {
        event_id: event,
        key: concert_domain::ParticipantKey {
            event_id: event,
            user_id: concert_domain::AuthUserId::from_uuid(Uuid::parse_str(
                data["user"].as_str().unwrap(),
            )?),
        },
        session_id: concert_application::identity::SessionId::new(
            data["family"].as_str().unwrap().to_owned(),
        )?,
    };
    let s = fixture::Stack::restore(
        consumer,
        event,
        subject,
        Uuid::parse_str(data["room"].as_str().unwrap())?,
        Uuid::parse_str(data["target"].as_str().unwrap())?,
    )
    .await?;
    assert_ne!(
        s.room_epoch.to_string(),
        data["room_epoch"].as_str().unwrap(),
        "new room generation after restart"
    );
    let peer = s.add_user().await?;
    let servers = tls::Servers::new(&s).await?;
    let mut native = Native::new(&s).await?;
    // Password change before restart correctly invalidated the original
    // family. Establish an actual fresh family to create the pre-bootstrap
    // view, then revoke that exact family as the intervening mutation.
    let pre_bootstrap = s.add_family(s.subject.key.user_id).await?;
    let (socket, old, _) = native.connect(&s, &pre_bootstrap).await?;
    PgFollowRepository::new(s.friend.clone())
        .block(
            s.subject.key.user_id.uuid(),
            peer.key.user_id.uuid(),
            chrono::Utc::now(),
        )
        .await?;
    PgRefreshTokenStore::new(s.auth.clone(), Arc::new(SystemAuthRuntime))
        .revoke_family(Uuid::parse_str(pre_bootstrap.session_id.as_str())?)
        .await?;
    let mut remote = servers.runtime(&s)?;
    remote.bootstrap().await?;
    assert!(revoked(&s, &old));
    assert_eq!(
        remote.cursors[0].unwrap().epoch().to_string(),
        data["auth_epoch"].as_str().unwrap()
    );
    assert_eq!(
        remote.cursors[1].unwrap().epoch().to_string(),
        data["friend_epoch"].as_str().unwrap()
    );
    assert!(
        remote.cursors[0].unwrap().sequence() > auth
            && remote.cursors[1].unwrap().sequence() > friend
    );
    assert_eq!(
        effects(&s.concert, consumer, auth, friend).await?,
        data["effects"]
    );
    assert!(
        one_shot_denied(&mut native, &s, &pre_bootstrap).await,
        "cursor or restart cannot restore the revoked family"
    );
    let fresh_subject = s.add_family(s.subject.key.user_id).await?;
    let pump = Pump::new(remote);
    drop(socket);
    applied(&s, &servers.auth, InvalidationOwner::Auth, &pump).await?;
    let (fresh, new, _) = native.connect(&s, &fresh_subject).await?;
    assert!(!revoked(&s, &new));
    drop(fresh);
    pump.finish().await?;
    native.finish(&s).await?;
    servers.finish().await?;
    s.finish().await
}
async fn one_shot_denied(native: &mut Native, s: &fixture::Stack, subject: &WorldSubject) -> bool {
    native.connect(s, subject).await.is_err()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires marker-owned remote controller, actual four DBs and mTLS"]
async fn native_cancellation_during_guarded_replay_fences_socket_without_checkpoint(
) -> anyhow::Result<()> {
    let s = fixture::Stack::new().await?;
    let peer = s.add_user().await?;
    let mode = Arc::new(AtomicUsize::new(0));
    let auth = tls::Server::corruptible_auth(s.auth.clone(), mode.clone()).await?;
    let friend = tls::Server::new(InvalidationOwner::Friend, s.friend.clone()).await?;
    let mut remote = RemoteInvalidationRuntime::new(
        s.concert.clone(),
        s.bridge.invalidation_context().1,
        auth.source(InvalidationOwner::Auth)?,
        friend.source(InvalidationOwner::Friend)?,
        vec![s.bridge.clone()],
    )?;
    remote.bootstrap().await?;
    let mut native = Native::new(&s).await?;
    let (socket, binding, _) = native.connect(&s, &s.subject).await?;
    let before = position(&s.concert, remote.consumer, InvalidationOwner::Auth).await?;
    sqlx::query("update users set status='SUSPENDED' where id=$1")
        .bind(peer.key.user_id.uuid())
        .execute(&s.auth)
        .await?;
    let cancellation = remote.cancel.clone();
    mode.store(8, Ordering::Release);
    let task = tokio::spawn(async move {
        let result = remote.poll().await;
        (remote, result)
    });
    let began = Instant::now();
    while mode.load(Ordering::Acquire) != 9 {
        anyhow::ensure!(
            began.elapsed() < Duration::from_millis(700),
            "actual guarded ReadAfter was not entered"
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    cancellation.cancel();
    let (mut remote, result) = tokio::time::timeout(Duration::from_millis(300), task).await??;
    assert!(result.is_err());
    assert!(remote.readiness.snapshot().is_none());
    assert!(revoked(&s, &binding));
    assert_eq!(
        position(&s.concert, remote.consumer, InvalidationOwner::Auth).await?,
        before
    );
    let applied = Instant::now();
    tokio::time::sleep(Duration::from_millis(70)).await;
    no_private_after(&native, applied, &[binding]);
    assert!(
        remote.poll().await.is_err(),
        "cancelled runtime cannot auto-resume publication"
    );
    drop(socket);
    remote.shutdown().await?;
    drop(remote);
    native.finish(&s).await?;
    auth.finish().await?;
    friend.finish().await?;
    s.finish().await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires marker-owned remote controller, actual four DBs and mTLS"]
async fn native_ticket_issuance_does_not_wait_for_live_publication_permit() -> anyhow::Result<()> {
    let s = fixture::Stack::new().await?;
    let peer = s.add_user().await?;
    let servers = tls::Servers::new(&s).await?;
    let mut remote = servers.runtime(&s)?;
    remote.bootstrap().await?;
    let pump = Pump::new(remote);
    let mut native = Native::new(&s).await?;
    let (first, binding, _) = native.connect(&s, &s.subject).await?;
    assert!(CanonicalAuthorityPort::publication_ready(
        &s.bridge, &binding
    ));
    // This is the actual SQL-backed publication permit, not a fabricated row
    // lock. A second issue is only a bounded read and creates no membership.
    let _unused = tokio::time::timeout(
        Duration::from_millis(300),
        fixture::ticket(
            &s.repo(),
            &peer,
            s.room,
            chrono::Utc::now() + chrono::TimeDelta::seconds(40),
        ),
    )
    .await
    .context("second ticket waited behind a live publication permit")??;
    assert!(
        !revoked(&s, &binding),
        "ticket issue cannot revoke the incumbent"
    );
    let (second, second_binding, _) = native.connect(&s, &peer).await?;
    assert!(
        !revoked(&s, &binding),
        "peer admission preserves the incumbent"
    );
    AdmissionPort::disconnected(s.bridge.as_ref(), &second_binding).await;
    assert!(
        CanonicalAuthorityPort::publication_ready(&s.bridge, &binding),
        "cleanup of a departed peer must recheck and restore the incumbent permit"
    );
    // Periodic neutral owner refresh must preserve the exact existing stream,
    // rather than forcing a baseline change at each one-second maintenance tick.
    for _ in 0..2 {
        s.bridge.maintain().await?;
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(!revoked(&s, &binding));
        assert!(CanonicalAuthorityPort::publication_ready(
            &s.bridge, &binding
        ));
    }
    assert!(!native
        .trace
        .sent
        .lock()
        .unwrap()
        .iter()
        .any(|(b, value, _)| b == &binding
            && matches!(value["type"].as_str(), Some("revoke" | "resyncRequired"))));
    drop(first);
    drop(second);
    pump.finish().await?;
    native.finish(&s).await?;
    servers.finish().await?;
    s.finish().await
}
