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
    time::{Duration, Instant},
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
    tickets: BTreeMap<String, u128>,
    blocked: BTreeSet<(Uuid, Uuid)>,
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
impl CanonicalAuthorityPort for Arc<Authority> {
    fn publication_fence(&self) -> Arc<Mutex<()>> {
        self.fence.clone()
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
impl RecipientPolicy for Arc<Authority> {
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
            invalidation_id: id(999),
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
        r.entries.insert(
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
    async fn switch(&self, _: &Binding, _: AdmissionIntent, _: Uuid) -> Result<Binding, SafeClose> {
        Err(SafeClose::UNAVAILABLE)
    }
    async fn resync(&self, _: &Binding, _: Uuid) -> Result<Binding, SafeClose> {
        Err(SafeClose::UNAVAILABLE)
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
        let transport = SocketTransport::default();
        let runtime = WorldRuntime::new(
            event,
            id(4),
            ClockAnchor {
                monotonic_ms: 0,
                utc: anchor,
            },
            vec![spec(room, 50)],
            authority.clone(),
            authority.clone(),
            transport.clone(),
        )
        .unwrap();
        let (started, origin_ms) = runtime.clock_origin();
        let (handle, owner) = runtime.spawn();
        let handle = Arc::new(handle);
        let state = WorldSocketState::new(
            OriginPolicy::new(vec!["http://localhost:9000".into()], true).unwrap(),
            transport.clone(),
            authority.clone(),
            handle.clone(),
            started,
            origin_ms,
        );
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
    loop {
        match tokio::time::timeout(Duration::from_secs(4), client.next())
            .await
            .unwrap()
        {
            Some(Ok(Message::Close(Some(close)))) => return close.code.into(),
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
    assert_eq!(close_code(&mut replay).await, 4401);
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
