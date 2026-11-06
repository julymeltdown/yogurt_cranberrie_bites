//! Actual loopback sockets and independent clients. Canonical authority below is
//! a synthetic test fixture; no deployed Auth, reviewed art or IN01 pass is claimed.
mod world_runtime_support;
use async_trait::async_trait;
use chrono::{TimeDelta, Utc};
use concert_domain::{AuthUserId, EventId, ParticipantKey};
use concert_service::{
    adapters::r#in::world_ws::*,
    world::{ingress::*, interest::*, membership::*, runtime::*, simulation::*},
};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
    MaybeTlsStream, WebSocketStream,
};
use uuid::Uuid;
use world_runtime_support::*;
type Client = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
#[derive(Default)]
struct Records {
    entries: BTreeMap<Uuid, CanonicalMembership>,
    pending: BTreeMap<Uuid, CanonicalMembership>,
    tickets: BTreeMap<String, u128>,
    blocked: BTreeSet<(Uuid, Uuid)>,
    generation: u128,
}
struct Authority {
    fence: Arc<Mutex<()>>,
    records: Mutex<Records>,
    anchor: chrono::DateTime<Utc>,
    event: EventId,
    room: Uuid,
}
impl Authority {
    fn revoke(&self, user: Uuid) {
        let _f = self.fence.lock().unwrap();
        self.records.lock().unwrap().entries.remove(&user);
    }
}
#[derive(Clone)]
struct OwnerPort(Arc<Authority>);
impl std::ops::Deref for OwnerPort {
    type Target = Authority;
    fn deref(&self) -> &Authority {
        &self.0
    }
}
impl CanonicalAuthorityPort for OwnerPort {
    fn publication_fence(&self) -> Arc<Mutex<()>> {
        self.fence.clone()
    }
    fn install_pending(&self, b: &Binding, _now: u64) -> Result<(), AuthorityError> {
        let mut records = self.records.lock().unwrap();
        if records
            .entries
            .get(&b.key.user_id.uuid())
            .is_some_and(|e| e.binding == *b)
        {
            return Ok(());
        }
        let entry = records
            .pending
            .remove(&b.key.user_id.uuid())
            .filter(|e| e.binding == *b)
            .ok_or(AuthorityError::Revoked)?;
        records.entries.insert(b.key.user_id.uuid(), entry);
        Ok(())
    }
    fn current(&self, b: &Binding, now: u64) -> Result<CanonicalMembership, AuthorityError> {
        let mut e = self
            .records
            .lock()
            .unwrap()
            .entries
            .get(&b.key.user_id.uuid())
            .filter(|e| e.binding == *b)
            .cloned()
            .ok_or(AuthorityError::Revoked)?;
        e.checked_at_ms = now;
        Ok(e)
    }
    fn release(&self, b: &Binding) -> Result<(), AuthorityError> {
        let mut r = self.records.lock().unwrap();
        if r.entries
            .get(&b.key.user_id.uuid())
            .is_some_and(|e| e.binding == *b)
        {
            r.entries.remove(&b.key.user_id.uuid());
        }
        Ok(())
    }
}
impl RecipientPolicy for OwnerPort {
    fn current(
        &self,
        b: &Binding,
        subjects: &[Binding],
        now: u64,
    ) -> Result<RecipientView, ProjectionError> {
        let r = self.records.lock().unwrap();
        let actor = b.key.user_id.uuid();
        Ok(RecipientView {
            binding: b.clone(),
            checked_at_ms: now,
            valid_until_ms: now + 1000,
            invalidation_id: id(999 + r.generation),
            visible_users: subjects
                .iter()
                .filter(|s| {
                    !r.blocked.contains(&(actor, s.key.user_id.uuid()))
                        && !r.blocked.contains(&(s.key.user_id.uuid(), actor))
                })
                .map(|s| s.key.user_id.uuid())
                .collect(),
        })
    }
}
#[async_trait]
impl AdmissionPort for Authority {
    async fn admit(&self, intent: AdmissionIntent, connection: Uuid) -> Result<Binding, SafeClose> {
        let _f = self.fence.lock().unwrap();
        let mut r = self.records.lock().unwrap();
        let user = r
            .tickets
            .remove(intent.ticket.expose_for_consumption())
            .ok_or(SafeClose::AUTH_REQUIRED)?;
        if intent.event_id != self.event.uuid()
            || intent.room_id != self.room
            || intent.map_id != pins().map_id
        {
            return Err(SafeClose::AUTH_REQUIRED);
        }
        let b = Binding {
            key: ParticipantKey {
                event_id: self.event,
                user_id: AuthUserId::from_uuid(id(user)),
            },
            session_id: id(100 + user),
            connection_id: connection,
            owner_id: id(4),
            room_id: self.room,
            room_epoch: id(6),
            stream_id: Uuid::new_v4(),
            resources: pins(),
        };
        r.pending.insert(
            id(user),
            CanonicalMembership {
                binding: b.clone(),
                checked_at_ms: 0,
                lease_expires_at: self.anchor + TimeDelta::milliseconds(5000),
                access_expires_at: self.anchor + TimeDelta::milliseconds(6000),
                identity: identity(),
            },
        );
        Ok(b)
    }
    async fn switch(
        &self,
        _: &Binding,
        _: AdmissionIntent,
        _: Uuid,
    ) -> Result<Binding, SwitchFailure> {
        Err(SwitchFailure::Rejected(SwitchRejection::RoomFull))
    }
    async fn resync(&self, current: &Binding, _: Uuid) -> Result<Binding, SafeClose> {
        let _f = self.fence.lock().unwrap();
        let mut records = self.records.lock().unwrap();
        let mut entry = records
            .entries
            .get(&current.key.user_id.uuid())
            .filter(|e| e.binding == *current)
            .cloned()
            .ok_or(SafeClose::AUTH_REQUIRED)?;
        entry.binding.stream_id = Uuid::new_v4();
        let result = entry.binding.clone();
        records.pending.insert(current.key.user_id.uuid(), entry);
        Ok(result)
    }
    async fn disconnected(&self, b: &Binding) {
        let _f = self.fence.lock().unwrap();
        let mut r = self.records.lock().unwrap();
        if r.entries
            .get(&b.key.user_id.uuid())
            .is_some_and(|e| e.binding == *b)
        {
            r.entries.remove(&b.key.user_id.uuid());
        }
    }
}
struct Harness {
    address: std::net::SocketAddr,
    authority: Arc<Authority>,
    transport: SocketTransport,
    handle: Arc<RuntimeHandle>,
    owner: tokio::task::JoinHandle<ShutdownReport>,
    server: tokio::task::JoinHandle<()>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
}
impl Harness {
    async fn start() -> Self {
        Self::with_budget(NATIVE_WRITE_BYTES).await
    }
    async fn with_budget(budget: usize) -> Self {
        Self::configured(budget, None).await
    }
    async fn configured(
        budget: usize,
        policy: Option<Arc<dyn policy::PolicyBaselinePort>>,
    ) -> Self {
        let transport = match policy {
            Some(policy) => SocketTransport::default().with_policy_baseline(policy),
            None => SocketTransport::default(),
        };
        Self::with_transport(budget, transport).await
    }
    async fn with_transport(budget: usize, transport: SocketTransport) -> Self {
        let event = EventId::from_uuid(Uuid::new_v4());
        let anchor = Utc::now();
        let room = id(50);
        let authority = Arc::new(Authority {
            fence: Arc::new(Mutex::new(())),
            records: Mutex::new(Records {
                tickets: BTreeMap::from([("A".repeat(43), 1), ("B".repeat(43), 2)]),
                ..Default::default()
            }),
            anchor,
            event,
            room,
        });
        let runtime = WorldRuntime::new(
            event,
            id(4),
            ClockAnchor {
                monotonic_ms: 0,
                utc: anchor,
            },
            vec![spec(room, 50)],
            OwnerPort(authority.clone()),
            OwnerPort(authority.clone()),
            transport.clone(),
        )
        .unwrap();
        let (started, origin_ms) = runtime.clock_origin();
        let (handle, owner) = runtime.spawn();
        let handle = Arc::new(handle);
        let state = WorldSocketState::with_native_write_budget(
            OriginPolicy::new(vec!["http://localhost:9000".into()], true).unwrap(),
            transport.clone(),
            authority.clone(),
            handle.clone(),
            started,
            origin_ms,
            budget,
        )
        .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            axum::serve(listener, router(state))
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap()
        });
        Self {
            address,
            authority,
            transport,
            handle,
            owner,
            server,
            stop: Some(stop),
        }
    }
    async fn connect(&self) -> Client {
        let mut request = format!("ws://{}/world-ws", self.address)
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("origin", "http://localhost:9000".parse().unwrap());
        connect_async(request).await.unwrap().0
    }
    fn auth(&self, ticket: char) -> Message {
        Message::Text(json!({"protocolVersion":"1.0.0","type":"authenticate","ticket":ticket.to_string().repeat(43),"eventId":self.authority.event.uuid(),"roomId":self.authority.room,"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1}).to_string())
    }
    async fn close(mut self) {
        self.transport.close_all(SafeClose::UNAVAILABLE);
        self.handle.request_stop();
        let report = tokio::time::timeout(Duration::from_secs(2), self.owner)
            .await
            .unwrap()
            .unwrap();
        assert!(report.remaining_cleanup.is_empty());
        let _ = self.stop.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(2), self.server)
            .await
            .unwrap()
            .unwrap();
    }
}

struct BaselineFixture {
    committed: Arc<std::sync::atomic::AtomicBool>,
    commit_ok: bool,
}
struct HeldBaselineFixture {
    committed: Arc<std::sync::atomic::AtomicBool>,
    commit_ok: bool,
}
#[async_trait]
impl policy::PolicyBaselinePort for BaselineFixture {
    async fn held_baseline(
        &self,
        _event: Uuid,
        _actor: Uuid,
    ) -> Result<Box<dyn policy::HeldPolicyBaseline>, policy::PolicyUnavailable> {
        Ok(Box::new(HeldBaselineFixture {
            committed: self.committed.clone(),
            commit_ok: self.commit_ok,
        }))
    }
}
#[async_trait]
impl policy::HeldPolicyBaseline for HeldBaselineFixture {
    fn revision(&self) -> i64 {
        7
    }
    async fn commit(self: Box<Self>) -> Result<(), policy::PolicyUnavailable> {
        if !self.commit_ok {
            return Err(policy::PolicyUnavailable);
        }
        self.committed
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn policy_baseline_follows_actual_snapshot_and_committed_guard_on_same_socket() {
    let committed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let h = Harness::configured(
        NATIVE_WRITE_BYTES,
        Some(Arc::new(BaselineFixture {
            committed: committed.clone(),
            commit_ok: true,
        })),
    )
    .await;
    let mut client = h.connect().await;
    client.send(h.auth('A')).await.unwrap();
    let snapshot = value(&mut client).await;
    assert_eq!(snapshot["type"], "snapshot");
    let baseline = value(&mut client).await;
    assert!(committed.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(
        baseline,
        json!({
            "protocolVersion":"1.0.0", "type":"policy.baseline", "scope":"SOCIAL",
            "streamId":snapshot["streamId"], "eventId":h.authority.event.uuid(), "policyRevision":7,
        })
    );
    client.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"input","streamId":snapshot["streamId"],"roomEpoch":snapshot["roomEpoch"],"seq":1,"axisX":1,"axisY":0,"buttons":[]}).to_string())).await.unwrap();
    assert_eq!(value(&mut client).await["type"], "delta");
    h.close().await;
}

#[tokio::test]
async fn failed_policy_guard_preserves_world_without_a_private_baseline() {
    let h = Harness::configured(
        NATIVE_WRITE_BYTES,
        Some(Arc::new(BaselineFixture {
            committed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            commit_ok: false,
        })),
    )
    .await;
    let mut client = h.connect().await;
    client.send(h.auth('A')).await.unwrap();
    let snapshot = value(&mut client).await;
    assert_eq!(snapshot["type"], "snapshot");
    client.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"input","streamId":snapshot["streamId"],"roomEpoch":snapshot["roomEpoch"],"seq":1,"axisX":1,"axisY":0,"buttons":[]}).to_string())).await.unwrap();
    assert_eq!(value(&mut client).await["type"], "delta");
    h.close().await;
}

#[tokio::test]
async fn actual_social_baseline_database_unavailability_preserves_basic_world() {
    // The actual SQL baseline adapter is used, with no database provisioned.
    // This is a failure-boundary test, never a positive Social sharing proof.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(50))
        .connect_lazy("postgres://unused@127.0.0.1:9/mcr_concert_unavailable")
        .unwrap();
    let policy =
        concert_service::config::social_policy_runtime::SocialWorldPolicy::compose(pool.clone());
    let h = Harness::with_transport(NATIVE_WRITE_BYTES, policy.transport).await;
    let mut client = h.connect().await;
    client.send(h.auth('A')).await.unwrap();
    let snapshot = value(&mut client).await;
    assert_eq!(snapshot["type"], "snapshot");
    client.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"input","streamId":snapshot["streamId"],"roomEpoch":snapshot["roomEpoch"],"seq":1,"axisX":1,"axisY":0,"buttons":[]}).to_string())).await.unwrap();
    assert_eq!(value(&mut client).await["type"], "delta");
    h.close().await;
    pool.close().await;
}
async fn value(client: &mut Client) -> Value {
    loop {
        let next = tokio::time::timeout(Duration::from_secs(2), client.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        if let Message::Text(text) = next {
            return serde_json::from_str(&text).unwrap();
        }
    }
}
async fn close_code(client: &mut Client) -> u16 {
    close_frame(client).await.code.into()
}
async fn close_frame(client: &mut Client) -> tokio_tungstenite::tungstenite::protocol::CloseFrame {
    loop {
        match tokio::time::timeout(Duration::from_secs(4), client.next())
            .await
            .unwrap()
        {
            Some(Ok(Message::Close(Some(close)))) => return close,
            Some(Ok(_)) => {}
            value => panic!("expected safe close, got {value:?}"),
        }
    }
}

#[tokio::test]
async fn two_independent_sockets_receive_snapshots_and_actual_authoritative_movement() {
    let h = Harness::start().await;
    let mut a = h.connect().await;
    let mut b = h.connect().await;
    a.send(h.auth('A')).await.unwrap();
    let snapshot = value(&mut a).await;
    assert_eq!(snapshot["type"], "snapshot");
    assert_eq!(snapshot["streamSeq"], 0);
    b.send(h.auth('B')).await.unwrap();
    assert_eq!(value(&mut b).await["type"], "snapshot");
    let x = snapshot["players"][0]["position"]["x"].as_f64().unwrap();
    a.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"input","streamId":snapshot["streamId"],"roomEpoch":snapshot["roomEpoch"],"seq":1,"axisX":1,"axisY":0,"buttons":[]}).to_string())).await.unwrap();
    let mut moved = false;
    for _ in 0..8 {
        let delta = value(&mut b).await;
        if delta["upserts"].as_array().is_some_and(|v| {
            v.iter().any(|p| {
                p["userId"] == id(1).to_string()
                    && p["position"]["x"].as_f64().is_some_and(|px| px > x)
            })
        }) {
            moved = true;
            break;
        }
    }
    assert!(moved);
    h.close().await;
}
#[tokio::test]
async fn handshake_rejects_missing_null_foreign_duplicate_origin_and_ticket_query() {
    let h = Harness::start().await;
    for origin in [None, Some("null"), Some("https://untrusted.example")] {
        let mut request = format!("ws://{}/world-ws", h.address)
            .into_client_request()
            .unwrap();
        if let Some(origin) = origin {
            request
                .headers_mut()
                .insert("origin", origin.parse().unwrap());
        }
        let error = connect_async(request).await.unwrap_err();
        assert!(
            matches!(error,tokio_tungstenite::tungstenite::Error::Http(ref r) if r.status()==403)
        );
    }
    let mut request = format!("ws://{}/world-ws?ticket=forbidden", h.address)
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("origin", "http://localhost:9000".parse().unwrap());
    assert!(connect_async(request).await.is_err());
    let mut request = format!("ws://{}/world-ws", h.address)
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .append("origin", "http://localhost:9000".parse().unwrap());
    request
        .headers_mut()
        .append("origin", "http://localhost:9000".parse().unwrap());
    assert!(connect_async(request).await.is_err());
    h.close().await;
}
#[tokio::test]
async fn unknown_ticket_and_replay_never_receive_a_world_snapshot() {
    let h = Harness::start().await;
    let mut unknown = h.connect().await;
    unknown.send(h.auth('Z')).await.unwrap();
    assert_eq!(close_code(&mut unknown).await, 4401);
    let mut a = h.connect().await;
    a.send(h.auth('A')).await.unwrap();
    assert_eq!(value(&mut a).await["type"], "snapshot");
    let mut replay = h.connect().await;
    replay.send(h.auth('A')).await.unwrap();
    let close = close_frame(&mut replay).await;
    assert_eq!(u16::from(close.code), 4401);
    // The replay is refused as an authentication failure and the close reason
    // is the fixed label: it never echoes the consumed ticket or any room data.
    assert_eq!(close.reason.as_ref(), "AUTH_REQUIRED");
    assert!(!close.reason.contains("AAAA"));
    h.close().await;
}
#[tokio::test]
async fn well_formed_unissued_ticket_is_refused_without_a_snapshot() {
    let h = Harness::start().await;
    let mut socket = h.connect().await;
    // Canonical 43-character shape, but absent from the canonical ticket set.
    socket.send(h.auth('Q')).await.unwrap();
    assert_eq!(close_code(&mut socket).await, 4401);
    h.close().await;
}

#[tokio::test]
async fn three_second_auth_deadline_is_per_connection_even_after_server_uptime() {
    let h = Harness::start().await;
    tokio::time::sleep(Duration::from_millis(3100)).await;
    let mut a = h.connect().await;
    a.send(h.auth('A')).await.unwrap();
    assert_eq!(value(&mut a).await["type"], "snapshot");
    h.close().await;
}
#[tokio::test]
async fn unauthenticated_socket_times_out_and_binary_is_unsupported() {
    let h = Harness::start().await;
    let mut idle = h.connect().await;
    assert_eq!(close_code(&mut idle).await, 4408);
    let mut binary = h.connect().await;
    binary.send(Message::Binary(vec![1, 2])).await.unwrap();
    assert_eq!(close_code(&mut binary).await, 1003);
    h.close().await;
}
#[tokio::test]
async fn known_revocation_stops_stream_and_releases_room_without_more_input() {
    let h = Harness::start().await;
    let mut a = h.connect().await;
    a.send(h.auth('A')).await.unwrap();
    assert_eq!(value(&mut a).await["type"], "snapshot");
    h.authority.revoke(id(1));
    assert_eq!(close_code(&mut a).await, 4401);
    h.close().await;
}
#[tokio::test]
async fn shutdown_drains_two_active_connections_and_owner_cleanup() {
    let h = Harness::start().await;
    let mut a = h.connect().await;
    let mut b = h.connect().await;
    a.send(h.auth('A')).await.unwrap();
    b.send(h.auth('B')).await.unwrap();
    let _ = value(&mut a).await;
    let _ = value(&mut b).await;
    h.close().await;
    assert_eq!(close_code(&mut a).await, 4500);
    assert_eq!(close_code(&mut b).await, 4500);
}
#[test]
fn production_configuration_never_promotes_unavailable_or_partial_authority() {
    use concert_service::config::world_runtime::*;
    assert!(production_owners().is_err());
    assert!(WorldRuntimeSettings::settings(|_| Ok(None))
        .unwrap()
        .is_none());
    assert!(WorldRuntimeSettings::settings(|name| Ok(
        (name == "CONCERT_WORLD_OWNER_ID").then(|| id(4).to_string())
    ))
    .is_err());
}

#[tokio::test]
async fn native_oversize_and_fragment_aggregate_close_with_1009() {
    use tokio_tungstenite::tungstenite::protocol::frame::{
        coding::{Data, OpCode},
        Frame,
    };
    let h = Harness::start().await;
    let mut direct = h.connect().await;
    direct.send(Message::Text("x".repeat(4097))).await.unwrap();
    let close = close_frame(&mut direct).await;
    assert_eq!(u16::from(close.code), 1009);
    // The refusal is labelled with a fixed reason and never echoes the abusive
    // frame, a ticket, an actor or any room metadata back to the sender.
    assert_eq!(close.reason.as_ref(), "MESSAGE_TOO_BIG");
    assert!(!close.reason.contains("xxxx"));
    let mut fragments = h.connect().await;
    fragments
        .send(Message::Frame(Frame::message(
            vec![b'x'; 3000],
            OpCode::Data(Data::Text),
            false,
        )))
        .await
        .unwrap();
    fragments
        .send(Message::Frame(Frame::message(
            vec![b'x'; 2000],
            OpCode::Data(Data::Continue),
            true,
        )))
        .await
        .unwrap();
    let close = close_frame(&mut fragments).await;
    assert_eq!(u16::from(close.code), 1009);
    assert_eq!(close.reason.as_ref(), "MESSAGE_TOO_BIG");
    h.close().await;
}
#[tokio::test]
async fn native_invalid_utf8_closes_with_1007() {
    use tokio_tungstenite::tungstenite::protocol::frame::{
        coding::{Data, OpCode},
        Frame,
    };
    let h = Harness::start().await;
    let mut client = h.connect().await;
    client
        .send(Message::Frame(Frame::message(
            vec![0xff],
            OpCode::Data(Data::Text),
            true,
        )))
        .await
        .unwrap();
    assert_eq!(close_code(&mut client).await, 1007);
    h.close().await;
}
#[tokio::test]
async fn native_input_burst_is_rate_limited_with_4429() {
    let h = Harness::start().await;
    let mut client = h.connect().await;
    client.send(h.auth('A')).await.unwrap();
    let snapshot = value(&mut client).await;
    for seq in 1..=100 {
        let _=client.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"input","streamId":snapshot["streamId"],"roomEpoch":snapshot["roomEpoch"],"seq":seq,"axisX":1,"axisY":0,"buttons":[]}).to_string())).await;
    }
    assert_eq!(close_code(&mut client).await, 4429);
    h.close().await;
}
#[tokio::test]
async fn peer_close_releases_room_without_stopping_another_client() {
    let h = Harness::start().await;
    let mut a = h.connect().await;
    let mut b = h.connect().await;
    a.send(h.auth('A')).await.unwrap();
    let _ = value(&mut a).await;
    b.send(h.auth('B')).await.unwrap();
    let _ = value(&mut b).await;
    a.close(None).await.unwrap();
    let mut removed = false;
    for _ in 0..10 {
        if !h
            .authority
            .records
            .lock()
            .unwrap()
            .entries
            .contains_key(&id(1))
        {
            removed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(removed);
    assert_eq!(h.authority.records.lock().unwrap().entries.len(), 1);
    h.close().await;
}

#[tokio::test]
async fn full_room_switch_rejection_keeps_the_old_stream_and_accepts_more_input() {
    let h = Harness::start().await;
    let mut client = h.connect().await;
    client.send(h.auth('A')).await.unwrap();
    let initial = value(&mut client).await;
    let request = Uuid::new_v4();
    client.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"switchRoom","requestId":request,"streamId":initial["streamId"],"ticket":"B".repeat(43),"eventId":h.authority.event.uuid(),"roomId":id(51),"mapId":"map_fixture","mapRevision":1,"avatarManifestId":"avatar_fixture","avatarManifestVersion":1}).to_string())).await.unwrap();
    let mut rejected = false;
    for _ in 0..8 {
        let frame = value(&mut client).await;
        if frame["type"] == "switchRejected" {
            assert_eq!(frame["reason"], "ROOM_FULL");
            assert_eq!(frame["streamId"], initial["streamId"]);
            assert_eq!(frame["requestId"], request.to_string());
            rejected = true;
            break;
        }
    }
    assert!(rejected);
    client.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"input","streamId":initial["streamId"],"roomEpoch":initial["roomEpoch"],"seq":1,"axisX":1,"axisY":0,"buttons":[]}).to_string())).await.unwrap();
    let mut ack = false;
    for _ in 0..8 {
        if value(&mut client).await["ackInputSeq"] == 1 {
            ack = true;
            break;
        }
    }
    assert!(ack);
    h.close().await;
}
#[tokio::test]
async fn recipient_block_removes_the_subject_from_actual_socket_deltas() {
    let h = Harness::start().await;
    let mut a = h.connect().await;
    let mut b = h.connect().await;
    a.send(h.auth('A')).await.unwrap();
    let _ = value(&mut a).await;
    b.send(h.auth('B')).await.unwrap();
    let _ = value(&mut b).await;
    // Wait for the first peer baseline, then invalidate under the same owner fence.
    let mut seen = false;
    for _ in 0..8 {
        if value(&mut a).await["upserts"]
            .as_array()
            .is_some_and(|v| v.iter().any(|p| p["userId"] == id(2).to_string()))
        {
            seen = true;
            break;
        }
    }
    assert!(seen);
    {
        let _f = h.authority.fence.lock().unwrap();
        h.authority
            .records
            .lock()
            .unwrap()
            .blocked
            .insert((id(1), id(2)));
    }
    let mut removed = false;
    for _ in 0..8 {
        let frame = value(&mut a).await;
        if frame["removes"]
            .as_array()
            .is_some_and(|v| v.contains(&json!(id(2))))
        {
            removed = true;
            break;
        }
    }
    assert!(removed);
    // Unblocked control: clearing the same block restores the peer in the
    // actual socket projection within one further invalidation.
    {
        let _f = h.authority.fence.lock().unwrap();
        h.authority
            .records
            .lock()
            .unwrap()
            .blocked
            .remove(&(id(1), id(2)));
    }
    let mut restored = false;
    for _ in 0..8 {
        let frame = value(&mut a).await;
        if frame["upserts"]
            .as_array()
            .is_some_and(|v| v.iter().any(|p| p["userId"] == id(2).to_string()))
        {
            restored = true;
            break;
        }
    }
    assert!(restored, "unblocked peer must remain visible to the control client");
    h.close().await;
}
#[tokio::test]
async fn resync_installs_a_new_correlated_stream_snapshot_with_original_access_expiry() {
    let h = Harness::start().await;
    let mut client = h.connect().await;
    client.send(h.auth('A')).await.unwrap();
    let initial = value(&mut client).await;
    tokio::time::sleep(Duration::from_millis(1050)).await;
    let request = Uuid::new_v4();
    client.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"resync","requestId":request,"streamId":initial["streamId"],"roomEpoch":initial["roomEpoch"],"lastStreamSeq":0,"reason":"FOREGROUND"}).to_string())).await.unwrap();
    let mut found = false;
    for _ in 0..24 {
        let frame = value(&mut client).await;
        if frame["type"] == "snapshot" {
            assert_ne!(frame["streamId"], initial["streamId"]);
            assert_eq!(frame["streamSeq"], 0);
            assert_eq!(frame["cause"], "RESYNC");
            assert_eq!(frame["requestId"], request.to_string());
            assert_eq!(frame["accessExpiresAt"], initial["accessExpiresAt"]);
            found = true;
            break;
        }
    }
    assert!(found);
    h.close().await;
}
#[path = "world_runtime_support/postgres.rs"]
mod postgres;
#[tokio::test]
async fn actual_native_write_buffer_rejects_an_oversized_snapshot_without_replay() {
    let h = Harness::with_budget(256).await;
    let mut client = h.connect().await;
    client.send(h.auth('A')).await.unwrap();
    // The real tungstenite sink rejects its first snapshot against a deliberately
    // tightened server-owned native buffer. A numbered projection is never replayed.
    let message = tokio::time::timeout(Duration::from_secs(2), client.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(matches!(message,Message::Close(Some(ref frame)) if u16::from(frame.code)==4500));
    let mut cleaned = false;
    for _ in 0..10 {
        if h.authority.records.lock().unwrap().entries.is_empty() {
            cleaned = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(cleaned);
    h.close().await;
}

#[tokio::test]
async fn policy_generation_change_during_maintenance_delivers_gap_then_closes_4409() {
    let h = Harness::start().await;
    let mut client = h.connect().await;
    client.send(h.auth('A')).await.unwrap();
    let initial = value(&mut client).await;
    {
        let _f = h.authority.fence.lock().unwrap();
        h.authority.records.lock().unwrap().generation += 1;
    }
    let terminal = value(&mut client).await;
    assert_eq!(terminal["type"], "resyncRequired");
    assert_eq!(terminal["reason"], "GAP");
    assert_eq!(terminal["streamId"], initial["streamId"]);
    assert!(terminal.get("players").is_none());
    assert_eq!(close_code(&mut client).await, 4409);
    h.close().await;
}
