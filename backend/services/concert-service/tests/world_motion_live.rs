//! Actual native sockets, synthetic canonical authority/geometry metadata fixtures only. Helpers copied from world_ws.rs at cf1ca629. No live Auth/product acceptance claim.
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
        let mut runtime = WorldRuntime::new(
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
        runtime
            .configure_motion_profile(&format!("sha256:{}", "0".repeat(64)))
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
async fn native_opt_in_and_legacy_pair_receive_actual_owner_motion_then_remove_ghost() {
    let h = Harness::start().await;
    let mut a = h.connect().await;
    let mut b = h.connect().await;
    let mut auth: Value = match h.auth('A') {
        Message::Text(s) => serde_json::from_str(&s).unwrap(),
        _ => unreachable!(),
    };
    auth["motionProfileVersion"] = json!("1.0.0");
    a.send(Message::Text(auth.to_string())).await.unwrap();
    let first = value(&mut a).await;
    assert_eq!(first["motionProfile"]["version"], "1.0.0");
    assert_eq!(
        first["selfMotion"]["position"],
        first["players"][0]["position"]
    );
    b.send(h.auth('B')).await.unwrap();
    let old = value(&mut b).await;
    assert!(old.get("motionProfile").is_none());
    a.send(Message::Text(json!({"protocolVersion":"1.0.0","type":"input","streamId":first["streamId"],"roomEpoch":first["roomEpoch"],"seq":1,"axisX":1,"axisY":0,"buttons":[]}).to_string())).await.unwrap();
    let anchor = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let m = value(&mut a).await;
            if m["ackInputSeq"] == 1 {
                break m;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(anchor["selfMotion"]["consumedInputSeq"], 1);
    assert!(anchor["selfMotion"]["inputAppliedTicks"].as_u64().unwrap() > 0);
    assert_eq!(anchor["selfMotion"]["serverTick"], anchor["serverTick"]);
    assert!(anchor["selfMotion"].get("interact").is_none());
    a.close(None).await.unwrap();
    let departed = first["selfUserId"].clone();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let m = value(&mut b).await;
            assert!(m.get("selfMotion").is_none());
            if m["removes"]
                .as_array()
                .is_some_and(|r| r.contains(&departed))
            {
                break;
            }
        }
    })
    .await
    .unwrap();
    b.close(None).await.unwrap();
    h.close().await;
}
#[tokio::test]
async fn native_unsupported_negotiation_and_unknown_ticket_never_publish_profile() {
    let h = Harness::start().await;
    for (ticket, version, code) in [('A', "2.0.0", 4400), ('C', "1.0.0", 4401)] {
        let mut c = h.connect().await;
        let mut auth: Value = match h.auth(ticket) {
            Message::Text(s) => serde_json::from_str(&s).unwrap(),
            _ => unreachable!(),
        };
        auth["motionProfileVersion"] = json!(version);
        c.send(Message::Text(auth.to_string())).await.unwrap();
        assert_eq!(close_code(&mut c).await, code);
    }
    h.close().await;
}
