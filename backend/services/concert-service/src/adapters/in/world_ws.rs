//! Actual bounded Axum WebSocket transport. Admission is a separately qualified
//! canonical port; this boundary never trusts user/session identifiers in frames.
use crate::world::{
    ingress::{
        AdmissionIntent, ClientMessage, ConnectionIngress, FragmentKind, OriginPolicy, SafeClose,
        SwitchFailure, CLIENT_BYTES,
    },
    replication::{Delivery, SnapshotCause},
    runtime::{Command, RuntimeHandle, TransportPort, WriteError},
    simulation::Binding,
};
use async_trait::async_trait;
use axum::{
    extract::{
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use futures::{future::poll_fn, task::noop_waker_ref, Sink, Stream};
use std::{
    collections::BTreeMap,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tokio::sync::{oneshot, Semaphore};
use uuid::Uuid;

pub mod policy;
use policy::{PolicyBaselinePort, PolicyState};

pub const MAX_SOCKETS: usize = 864;
pub const NATIVE_WRITE_BYTES: usize = 262144;
/// Each successful result must already be committed, owner-fenced and published
/// into the exact CanonicalAuthorityPort used by the owner. No default allows admission.
#[async_trait]
pub trait AdmissionPort: Send + Sync + 'static {
    async fn admit(&self, intent: AdmissionIntent, connection: Uuid) -> Result<Binding, SafeClose>;
    async fn switch(
        &self,
        current: &Binding,
        intent: AdmissionIntent,
        request: Uuid,
    ) -> Result<Binding, SwitchFailure>;
    async fn resync(&self, current: &Binding, request: Uuid) -> Result<Binding, SafeClose>;
    /// Must retain/retry persistent cleanup even if this request is cancelled.
    async fn disconnected(&self, current: &Binding);
}
pub struct UnavailableAdmission;
#[async_trait]
impl AdmissionPort for UnavailableAdmission {
    async fn admit(&self, _: AdmissionIntent, _: Uuid) -> Result<Binding, SafeClose> {
        Err(SafeClose::UNAVAILABLE)
    }
    async fn switch(
        &self,
        _: &Binding,
        _: AdmissionIntent,
        _: Uuid,
    ) -> Result<Binding, SwitchFailure> {
        Err(SwitchFailure::Close(SafeClose::UNAVAILABLE))
    }
    async fn resync(&self, _: &Binding, _: Uuid) -> Result<Binding, SafeClose> {
        Err(SafeClose::UNAVAILABLE)
    }
    async fn disconnected(&self, _: &Binding) {}
}
struct Slot {
    socket: WebSocket,
    binding: Option<Binding>,
    terminal: Option<SafeClose>,
    policy: PolicyState,
}
#[derive(Clone, Default)]
pub struct SocketTransport {
    slots: Arc<Mutex<BTreeMap<Uuid, Arc<Mutex<Slot>>>>>,
    policy_baseline: Option<Arc<dyn PolicyBaselinePort>>,
}
impl SocketTransport {
    /// Withdraw the actual installed registry and its private policy baseline.
    /// Retained serve_socket handles observe terminal on their next bounded tick;
    /// removing the registry also forbids any further native publication now.
    pub(crate) fn withdraw_account(&self, user: Uuid) -> Result<Vec<Binding>, WriteError> {
        let mut slots = self.slots.lock().map_err(|_| WriteError::Failed)?;
        let mut victims = Vec::new();
        let mut connections = Vec::new();
        for (connection, slot) in slots.iter() {
            let mut slot = slot.lock().map_err(|_| WriteError::Failed)?;
            if let Some(binding) = slot
                .binding
                .as_ref()
                .filter(|b| b.key.user_id.uuid() == user)
                .cloned()
            {
                slot.terminal = Some(SafeClose {
                    code: 4401,
                    label: "SESSION_REVOKED",
                });
                slot.policy = PolicyState::default();
                slot.binding = None;
                victims.push(binding);
                connections.push(*connection);
            }
        }
        for connection in connections {
            slots.remove(&connection);
        }
        Ok(victims)
    }
    fn slot(&self, id: Uuid) -> Result<Arc<Mutex<Slot>>, WriteError> {
        self.slots
            .lock()
            .map_err(|_| WriteError::Failed)?
            .get(&id)
            .cloned()
            .ok_or(WriteError::Failed)
    }
    fn register(&self, id: Uuid, socket: WebSocket) -> Result<Arc<Mutex<Slot>>, WriteError> {
        let mut slots = self.slots.lock().map_err(|_| WriteError::Failed)?;
        if slots.len() >= MAX_SOCKETS || slots.contains_key(&id) {
            return Err(WriteError::Failed);
        }
        let slot = Arc::new(Mutex::new(Slot {
            socket,
            binding: None,
            terminal: None,
            policy: PolicyState::default(),
        }));
        slots.insert(id, slot.clone());
        Ok(slot)
    }
    pub fn close_all(&self, reason: SafeClose) {
        if let Ok(slots) = self.slots.lock() {
            for slot in slots.values() {
                if let Ok(mut slot) = slot.lock() {
                    slot.terminal.get_or_insert(reason);
                }
            }
        }
    }
    fn remove(&self, id: Uuid) {
        if let Ok(mut slots) = self.slots.lock() {
            slots.remove(&id);
        }
    }
}
fn handoff(slot: &mut Slot, message: Message) -> Result<(), WriteError> {
    let mut cx = Context::from_waker(noop_waker_ref());
    match Pin::new(&mut slot.socket).poll_ready(&mut cx) {
        Poll::Ready(Ok(())) => {}
        Poll::Pending => return Err(WriteError::NotReady),
        _ => return Err(WriteError::Failed),
    }
    Pin::new(&mut slot.socket)
        .start_send(message)
        .map_err(|_| WriteError::Failed)?;
    // Native tungstenite write buffer only, bounded by upgrade configuration.
    // There is no second application queue or copied stale recipient projection.
    match Pin::new(&mut slot.socket).poll_flush(&mut cx) {
        Poll::Ready(Err(_)) => Err(WriteError::Failed),
        _ => Ok(()),
    }
}
impl TransportPort for SocketTransport {
    fn installed(&mut self, b: &Binding, _: u64) -> Result<(), WriteError> {
        let slot = self.slot(b.connection_id)?;
        let mut slot = slot.lock().map_err(|_| WriteError::Failed)?;
        if slot.terminal.is_some()
            || slot
                .binding
                .as_ref()
                .is_some_and(|old| old.key != b.key || old.session_id != b.session_id)
        {
            return Err(WriteError::Failed);
        }
        slot.binding = Some(b.clone());
        slot.policy.installed(b);
        Ok(())
    }
    fn ready(&self, b: &Binding) -> bool {
        let Ok(slot) = self.slot(b.connection_id) else {
            return false;
        };
        let Ok(mut slot) = slot.lock() else {
            return false;
        };
        if slot.binding.as_ref() != Some(b) || slot.terminal.is_some() {
            return false;
        }
        matches!(
            Pin::new(&mut slot.socket).poll_ready(&mut Context::from_waker(noop_waker_ref())),
            Poll::Ready(Ok(()))
        )
    }
    fn send(&mut self, b: &Binding, delivery: Delivery) -> Result<(), WriteError> {
        if delivery.bytes.len() > 65536 {
            return Err(WriteError::Failed);
        }
        let slot = self.slot(b.connection_id)?;
        let mut slot = slot.lock().map_err(|_| WriteError::Failed)?;
        if slot.binding.as_ref() != Some(b) || slot.terminal.is_some() {
            return Err(WriteError::Failed);
        }
        let snapshot = !slot.policy.snapshot_sent && policy::snapshot_matches(b, &delivery.bytes);
        let bytes = String::from_utf8(delivery.bytes).map_err(|_| WriteError::Failed)?;
        handoff(&mut slot, Message::Text(bytes.into()))?;
        // Only an actual native snapshot handoff may precede the policy baseline.
        // The World owner, not client frames, supplies this message.
        slot.policy.snapshot_sent |= snapshot;
        policy::flush_baseline(&mut slot, b)?;
        if let Some(code) = delivery.close_code {
            slot.terminal = Some(SafeClose {
                code,
                label: "STREAM_CLOSED",
            });
        }
        Ok(())
    }
    fn close(&mut self, b: &Binding, reason: SafeClose) {
        if let Ok(slot) = self.slot(b.connection_id) {
            if let Ok(mut slot) = slot.lock() {
                if slot.binding.as_ref() == Some(b) {
                    slot.terminal.get_or_insert(reason);
                }
            }
        }
    }
}
pub struct WorldSocketState {
    pub origins: OriginPolicy,
    pub transport: SocketTransport,
    pub admission: Arc<dyn AdmissionPort>,
    pub owner: Arc<RuntimeHandle>,
    pub started: Instant,
    pub origin_ms: u64,
    permits: Arc<Semaphore>,
    native_write_bytes: usize,
}
impl WorldSocketState {
    /// Bind deletion to the same admission object, owner handle and registry
    /// actually serving this state. A parallel empty transport is not evidence.
    pub(crate) fn account_session_parts(
        &self,
        bridge: &Arc<crate::world::admission_bridge::AdmissionBridge>,
    ) -> Result<(SocketTransport, Arc<RuntimeHandle>), WriteError> {
        let admission: Arc<dyn AdmissionPort> = bridge.clone();
        if !Arc::ptr_eq(&self.admission, &admission) {
            return Err(WriteError::Failed);
        }
        Ok((self.transport.clone(), self.owner.clone()))
    }
    pub fn new(
        origins: OriginPolicy,
        transport: SocketTransport,
        admission: Arc<dyn AdmissionPort>,
        owner: Arc<RuntimeHandle>,
        started: Instant,
        origin_ms: u64,
    ) -> Arc<Self> {
        Self::with_native_write_budget(
            origins,
            transport,
            admission,
            owner,
            started,
            origin_ms,
            NATIVE_WRITE_BYTES,
        )
        .expect("frozen native write cap")
    }
    /// Server-owned tightening only. Never accepts a client-supplied budget.
    pub fn with_native_write_budget(
        origins: OriginPolicy,
        transport: SocketTransport,
        admission: Arc<dyn AdmissionPort>,
        owner: Arc<RuntimeHandle>,
        started: Instant,
        origin_ms: u64,
        native_write_bytes: usize,
    ) -> Result<Arc<Self>, SafeClose> {
        if !(256..=NATIVE_WRITE_BYTES).contains(&native_write_bytes) {
            return Err(SafeClose::UNAVAILABLE);
        }
        Ok(Arc::new(Self {
            origins,
            transport,
            admission,
            owner,
            started,
            origin_ms,
            permits: Arc::new(Semaphore::new(MAX_SOCKETS)),
            native_write_bytes,
        }))
    }
    fn now(&self) -> Result<u64, SafeClose> {
        self.origin_ms
            .checked_add(
                self.started
                    .elapsed()
                    .as_millis()
                    .try_into()
                    .map_err(|_| SafeClose::UNAVAILABLE)?,
            )
            .ok_or(SafeClose::UNAVAILABLE)
    }
}
pub fn router(state: Arc<WorldSocketState>) -> Router {
    Router::new()
        .route("/world-ws", get(upgrade))
        .with_state(state)
}
async fn upgrade(
    State(state): State<Arc<WorldSocketState>>,
    headers: HeaderMap,
    uri: Uri,
    ws: WebSocketUpgrade,
) -> Response {
    if uri.query().is_some() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let values: Result<Vec<_>, _> = headers
        .get_all("origin")
        .iter()
        .map(|v| v.to_str())
        .collect();
    if values
        .as_ref()
        .ok()
        .is_none_or(|values| state.origins.check(values).is_err())
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    // Tickets are never accepted in URLs, cookies, subprotocols or alternate paths.
    if headers.contains_key("sec-websocket-protocol") {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let Ok(permit) = state.permits.clone().try_acquire_owned() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    ws.max_message_size(CLIENT_BYTES)
        .max_frame_size(CLIENT_BYTES)
        .write_buffer_size(0)
        .max_write_buffer_size(state.native_write_bytes)
        .on_upgrade(move |socket| async move {
            let _permit = permit;
            serve_socket(state, socket).await;
        })
}
async fn owner_install(
    state: &WorldSocketState,
    b: Binding,
    cause: SnapshotCause,
) -> Result<(), SafeClose> {
    // No DB guard is held across a socket/network wait. The owner command only
    // installs local state; snapshot publication happens on its later tick.
    let policy = match &state.transport.policy_baseline {
        Some(port) => tokio::time::timeout(
            Duration::from_millis(500),
            port.held_baseline(b.key.event_id.uuid(), b.key.user_id.uuid()),
        )
        .await
        .ok()
        .and_then(Result::ok),
        None => None,
    };
    let policy_binding = b.clone();
    let (reply, answer) = oneshot::channel();
    state
        .owner
        .try_submit(Command::Install {
            binding: b,
            cause,
            reply,
        })
        .map_err(|_| SafeClose::UNAVAILABLE)?;
    tokio::time::timeout(Duration::from_millis(100), answer)
        .await
        .map_err(|_| SafeClose::UNAVAILABLE)?
        .map_err(|_| SafeClose::UNAVAILABLE)?
        .map_err(|_| SafeClose::UNAVAILABLE)?;
    if let Some(guard) = policy {
        let revision = guard.revision();
        if state
            .transport
            .prepare_policy(&policy_binding, revision)
            .is_ok()
        {
            let committed = tokio::time::timeout(Duration::from_millis(500), guard.commit())
                .await
                .is_ok_and(|result| result.is_ok());
            state
                .transport
                .finish_policy(&policy_binding, revision, committed);
        }
    }
    Ok(())
}
async fn serve_socket(state: Arc<WorldSocketState>, socket: WebSocket) {
    let connection = Uuid::new_v4();
    let Ok(slot) = state.transport.register(connection, socket) else {
        return;
    };
    let mut bound: Option<Binding> = None;
    let result = session(&state, &slot, connection, &mut bound).await;
    if let Some(b) = bound.as_ref() {
        let (reply, _) = oneshot::channel();
        let _ = state.owner.try_submit(Command::Disconnect {
            binding: b.clone(),
            reply,
        });
        // Runtime also retains exact cleanup when its bounded command queue is full.
        let _ = tokio::time::timeout(Duration::from_secs(1), state.admission.disconnected(b)).await;
    }
    let reason = result.err().unwrap_or(SafeClose {
        code: 1000,
        label: "CLOSED",
    });
    let mut close_sent = false;
    let _ = tokio::time::timeout(
        Duration::from_millis(100),
        poll_fn(|cx| {
            let Ok(mut slot) = slot.lock() else {
                return Poll::Ready(());
            };
            if close_sent {
                return Pin::new(&mut slot.socket).poll_flush(cx).map(|_| ());
            }
            match Pin::new(&mut slot.socket).poll_ready(cx) {
                Poll::Pending => Poll::Pending,
                _ => {
                    close_sent = true;
                    let _ =
                        Pin::new(&mut slot.socket).start_send(Message::Close(Some(CloseFrame {
                            code: reason.code,
                            reason: reason.label.into(),
                        })));
                    Pin::new(&mut slot.socket).poll_flush(cx).map(|_| ())
                }
            }
        }),
    )
    .await;
    state.transport.remove(connection);
}
async fn session(
    state: &WorldSocketState,
    slot: &Arc<Mutex<Slot>>,
    connection: Uuid,
    bound: &mut Option<Binding>,
) -> Result<(), SafeClose> {
    let accepted = tokio::time::Instant::now();
    let auth_deadline = accepted + Duration::from_millis(3000);
    let mut ingress = ConnectionIngress::new(state.now()?)?;
    let mut timer = tokio::time::interval(Duration::from_millis(25));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! { biased;
            _ = platform::shutdown::global().cancelled() => return Err(SafeClose::UNAVAILABLE),
            _ = timer.tick() => {
                let now = state.now()?; ingress.check_deadline(now)?;
                let mut slot = slot.lock().map_err(|_| SafeClose::UNAVAILABLE)?;
                if let Some(reason) = slot.terminal { return Err(reason); }
                if let Some(bytes) = ingress.ping(now, Uuid::new_v4())? {
                    let text = String::from_utf8(bytes).map_err(|_| SafeClose::UNAVAILABLE)?;
                    handoff(&mut slot, Message::Text(text.into())).map_err(|_| SafeClose::UNAVAILABLE)?;
                }
                let mut cx = Context::from_waker(noop_waker_ref());
                if matches!(Pin::new(&mut slot.socket).poll_flush(&mut cx), Poll::Ready(Err(_))) { return Err(SafeClose::UNAVAILABLE); }
            },
            message = poll_fn(|cx| {
                let Ok(mut slot) = slot.lock() else { return Poll::Ready(None); };
                Pin::new(&mut slot.socket).poll_next(cx)
            }) => {
                let message = match message { Some(Ok(v)) => v, None => return Ok(()), Some(Err(error)) => return Err(native_error(error)) };
                let (kind, bytes) = match message {
                    Message::Text(text) => (FragmentKind::Text, text.as_bytes().to_vec()),
                    Message::Binary(bytes) => (FragmentKind::Binary, bytes.to_vec()),
                    Message::Close(_) => return Ok(()),
                    Message::Ping(_) | Message::Pong(_) => return Err(SafeClose::INVALID),
                };
                let Some(message) = ingress.receive(kind, true, &bytes, state.now()?)? else { continue; };
                match message {
                    ClientMessage::Authenticate(intent) => {
                        let deadline = auth_deadline;
                        let b = tokio::time::timeout_at(deadline, state.admission.admit(intent, connection)).await.map_err(|_| SafeClose::AUTH_TIMEOUT)??;
                        *bound = Some(b.clone());
                        if b.connection_id != connection { return Err(SafeClose::UNAVAILABLE); }
                        ingress.activate(b.stream_id, b.room_epoch, state.now()?)?;
                        tokio::time::timeout_at(auth_deadline,owner_install(state,b,SnapshotCause::Initial)).await.map_err(|_| SafeClose::AUTH_TIMEOUT)??;
                    },
                    ClientMessage::Input(input) => {
                        let b = bound.as_ref().ok_or(SafeClose::AUTH_REQUIRED)?;
                        let (reply, answer) = oneshot::channel();
                        state.owner.try_submit(Command::Input { binding:b.clone(),input,reply }).map_err(|_| SafeClose::THROTTLED)?;
                        tokio::time::timeout(Duration::from_millis(100),answer).await.map_err(|_| SafeClose::UNAVAILABLE)?.map_err(|_| SafeClose::UNAVAILABLE)?.map_err(|_| SafeClose::INVALID)?;
                    },
                    ClientMessage::Resync(request) => {
                        let b = bound.as_ref().ok_or(SafeClose::AUTH_REQUIRED)?;
                        let next = tokio::time::timeout(Duration::from_millis(500),state.admission.resync(b,request.request_id)).await.map_err(|_| SafeClose::UNAVAILABLE)??;
                        if next.connection_id != connection || next.session_id != b.session_id || next.key != b.key { return Err(SafeClose::UNAVAILABLE); }
                        *bound = Some(next.clone());
                        owner_install(state,next.clone(),SnapshotCause::Resync(request.request_id)).await?;
                        ingress.activate(next.stream_id,next.room_epoch,state.now()?)?;
                    },
                    ClientMessage::SwitchRoom { request_id, admission, .. } => {
                        let b = bound.as_ref().ok_or(SafeClose::AUTH_REQUIRED)?;
                        let (reply,answer)=oneshot::channel();
                        state.owner.try_submit(Command::BeginSwitch{binding:b.clone(),request_id,reply}).map_err(|_|SafeClose::UNAVAILABLE)?;
                        tokio::time::timeout(Duration::from_millis(100),answer).await.map_err(|_|SafeClose::UNAVAILABLE)?.map_err(|_|SafeClose::UNAVAILABLE)?.map_err(|_|SafeClose::UNAVAILABLE)?;
                        let next = match tokio::time::timeout(Duration::from_millis(500),state.admission.switch(b,admission,request_id)).await.map_err(|_| SafeClose::UNAVAILABLE)? {
                            Ok(next)=>next,
                            Err(SwitchFailure::Close(close))=>return Err(close),
                            Err(SwitchFailure::Rejected(reason))=> {
                                let (reply,answer)=oneshot::channel();
                                state.owner.try_submit(Command::RejectSwitch{binding:b.clone(),request_id,reason,reply}).map_err(|_|SafeClose::UNAVAILABLE)?;
                                tokio::time::timeout(Duration::from_millis(100),answer).await.map_err(|_|SafeClose::UNAVAILABLE)?.map_err(|_|SafeClose::UNAVAILABLE)?.map_err(|_|SafeClose::UNAVAILABLE)?;
                                continue;
                            }
                        };
                        if next.connection_id != connection || next.session_id != b.session_id || next.key != b.key { return Err(SafeClose::UNAVAILABLE); }
                        *bound = Some(next.clone());
                        owner_install(state,next.clone(),SnapshotCause::RoomSwitch(request_id)).await?;
                        ingress.activate(next.stream_id,next.room_epoch,state.now()?)?;
                    },
                    ClientMessage::Pong(_) => {},
                }
            }
        }
    }
}

fn native_error(error: axum::Error) -> SafeClose {
    use tokio_tungstenite::tungstenite::Error;
    match error.into_inner().downcast::<Error>().as_deref() {
        Ok(Error::Capacity(_)) => SafeClose::TOO_BIG,
        Ok(Error::Utf8) => SafeClose {
            code: 1007,
            label: "INVALID_UTF8",
        },
        Ok(Error::Protocol(_)) => SafeClose::INVALID,
        _ => SafeClose::UNAVAILABLE,
    }
}
