//! Bounded C05 application-frame ingress. No socket listener or ticket authority is enabled here.
use concert_domain::{world::Input, AssetId, Revision};
use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};
use uuid::Uuid;

pub const CLIENT_BYTES: usize = 4096;
pub const MAX_FRAGMENTS: usize = 128;
pub const AUTHENTICATE_MS: u64 = 3000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SafeClose {
    pub code: u16,
    pub label: &'static str,
}
impl SafeClose {
    pub const INVALID: Self = Self {
        code: 4400,
        label: "INVALID_MESSAGE",
    };
    pub const UNAVAILABLE: Self = Self {
        code: 4500,
        label: "SERVER_UNAVAILABLE",
    };
    pub const AUTH_REQUIRED: Self = Self {
        code: 4401,
        label: "AUTH_REQUIRED",
    };
    pub const AUTH_TIMEOUT: Self = Self {
        code: 4408,
        label: "AUTH_TIMEOUT",
    };
    pub const THROTTLED: Self = Self {
        code: 4429,
        label: "RATE_LIMITED",
    };
    pub const TOO_BIG: Self = Self {
        code: 1009,
        label: "MESSAGE_TOO_BIG",
    };
}
#[derive(Clone)]
pub struct OpaqueTicket(String);
impl OpaqueTicket {
    pub fn expose_for_consumption(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for OpaqueTicket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OpaqueTicket([REDACTED])")
    }
}
#[derive(Clone, Debug)]
pub struct AdmissionIntent {
    pub ticket: OpaqueTicket,
    pub event_id: Uuid,
    pub room_id: Uuid,
    pub map_id: AssetId,
    pub map_revision: Revision,
    pub avatar_manifest_id: AssetId,
    pub avatar_manifest_version: Revision,
}
#[derive(Clone, Debug)]
pub struct ResyncIntent {
    pub request_id: Uuid,
    pub stream_id: Uuid,
    pub room_epoch: Uuid,
    pub last_stream_seq: u64,
    pub reason: String,
}
#[derive(Clone, Debug)]
pub enum ClientMessage {
    Authenticate(AdmissionIntent),
    Input(Input),
    Resync(ResyncIntent),
    SwitchRoom {
        request_id: Uuid,
        stream_id: Uuid,
        admission: AdmissionIntent,
    },
    Pong(Uuid),
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AdmissionWire {
    protocol_version: String,
    #[serde(rename = "type")]
    kind: String,
    ticket: String,
    event_id: String,
    room_id: String,
    map_id: String,
    map_revision: i64,
    avatar_manifest_id: String,
    avatar_manifest_version: i64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SwitchWire {
    protocol_version: String,
    #[serde(rename = "type")]
    kind: String,
    request_id: String,
    stream_id: String,
    ticket: String,
    event_id: String,
    room_id: String,
    map_id: String,
    map_revision: i64,
    avatar_manifest_id: String,
    avatar_manifest_version: i64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResyncWire {
    protocol_version: String,
    #[serde(rename = "type")]
    kind: String,
    request_id: String,
    stream_id: String,
    room_epoch: String,
    last_stream_seq: u64,
    reason: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PongWire {
    protocol_version: String,
    #[serde(rename = "type")]
    kind: String,
    heartbeat_id: String,
}
fn uuid(s: &str) -> Result<Uuid, SafeClose> {
    Uuid::parse_str(s)
        .ok()
        .filter(|id| id.to_string() == s)
        .ok_or(SafeClose::INVALID)
}
impl AdmissionWire {
    fn intent(self) -> Result<AdmissionIntent, SafeClose> {
        if self.protocol_version != "1.0.0"
            || !matches!(self.kind.as_str(), "authenticate" | "switchRoom")
            || self.ticket.len() != 43
            || !self
                .ticket
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            return Err(SafeClose::INVALID);
        }
        Ok(AdmissionIntent {
            ticket: OpaqueTicket(self.ticket),
            event_id: uuid(&self.event_id)?,
            room_id: uuid(&self.room_id)?,
            map_id: AssetId::new(self.map_id).map_err(|_| SafeClose::INVALID)?,
            map_revision: Revision::new(self.map_revision).map_err(|_| SafeClose::INVALID)?,
            avatar_manifest_id: AssetId::new(self.avatar_manifest_id)
                .map_err(|_| SafeClose::INVALID)?,
            avatar_manifest_version: Revision::new(self.avatar_manifest_version)
                .map_err(|_| SafeClose::INVALID)?,
        })
    }
}
// serde_json::Value alone silently overwrites duplicate keys. This visitor rejects
// them at every object level before the typed C05 decode; the entire input is <=4096 bytes.
struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UniqueVisitor;
        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = Unique;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("unique-key JSON")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Unique, E> {
                Ok(Unique(Value::Bool(v)))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Unique, E> {
                Ok(Unique(Value::from(v)))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Unique, E> {
                Ok(Unique(Value::from(v)))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Unique, E> {
                serde_json::Number::from_f64(v)
                    .map(|n| Unique(Value::Number(n)))
                    .ok_or_else(|| E::custom("nonfinite"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Unique, E> {
                Ok(Unique(Value::String(v.to_owned())))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Unique, E> {
                Ok(Unique(Value::String(v)))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_none<E: de::Error>(self) -> Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Unique, A::Error> {
                let mut values = Vec::new();
                while let Some(Unique(v)) = a.next_element()? {
                    values.push(v);
                }
                Ok(Unique(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Unique, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some((k, Unique(v))) = a.next_entry::<String, Unique>()? {
                    if values.insert(k, v).is_some() {
                        return Err(de::Error::custom("duplicate key"));
                    }
                }
                Ok(Unique(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(UniqueVisitor)
    }
}
fn bounded_json(bytes: &[u8]) -> Result<Value, SafeClose> {
    if bytes.len() > CLIENT_BYTES {
        return Err(SafeClose::TOO_BIG);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| SafeClose {
        code: 1007,
        label: "INVALID_UTF8",
    })?;
    let (mut depth, mut quoted, mut escaped) = (0u32, false, false);
    for byte in text.bytes() {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
        } else {
            match byte {
                b'"' => quoted = true,
                b'{' | b'[' => {
                    depth += 1;
                    if depth > 16 {
                        return Err(SafeClose::INVALID);
                    }
                }
                b'}' | b']' => {
                    depth = depth.checked_sub(1).ok_or(SafeClose::INVALID)?;
                }
                _ => {}
            }
        }
    }
    if depth != 0 || quoted {
        return Err(SafeClose::INVALID);
    }
    let Unique(v) = serde_json::from_str(text).map_err(|_| SafeClose::INVALID)?;
    if !v.is_object() {
        return Err(SafeClose::INVALID);
    }
    Ok(v)
}
pub fn decode(bytes: &[u8]) -> Result<ClientMessage, SafeClose> {
    let v = bounded_json(bytes)?;
    if v.get("protocolVersion").and_then(Value::as_str) != Some("1.0.0") {
        return Err(SafeClose::INVALID);
    }
    match v
        .get("type")
        .and_then(Value::as_str)
        .ok_or(SafeClose::INVALID)?
    {
        "authenticate" => Ok(ClientMessage::Authenticate(
            serde_json::from_value::<AdmissionWire>(v)
                .map_err(|_| SafeClose::INVALID)?
                .intent()?,
        )),
        "input" => Ok(ClientMessage::Input(
            serde_json::from_value(v).map_err(|_| SafeClose::INVALID)?,
        )),
        "resync" => {
            let w: ResyncWire = serde_json::from_value(v).map_err(|_| SafeClose::INVALID)?;
            if w.protocol_version != "1.0.0"
                || w.kind != "resync"
                || w.last_stream_seq > concert_domain::world::MAX_SEQUENCE
                || !matches!(
                    w.reason.as_str(),
                    "GAP" | "EPOCH_CHANGED" | "FOREGROUND" | "PROTOCOL_RECOVERY"
                )
            {
                return Err(SafeClose::INVALID);
            }
            Ok(ClientMessage::Resync(ResyncIntent {
                request_id: uuid(&w.request_id)?,
                stream_id: uuid(&w.stream_id)?,
                room_epoch: uuid(&w.room_epoch)?,
                last_stream_seq: w.last_stream_seq,
                reason: w.reason,
            }))
        }
        "switchRoom" => {
            let w: SwitchWire = serde_json::from_value(v).map_err(|_| SafeClose::INVALID)?;
            let request_id = uuid(&w.request_id)?;
            let stream_id = uuid(&w.stream_id)?;
            let admission = AdmissionWire {
                protocol_version: w.protocol_version,
                kind: w.kind,
                ticket: w.ticket,
                event_id: w.event_id,
                room_id: w.room_id,
                map_id: w.map_id,
                map_revision: w.map_revision,
                avatar_manifest_id: w.avatar_manifest_id,
                avatar_manifest_version: w.avatar_manifest_version,
            }
            .intent()?;
            Ok(ClientMessage::SwitchRoom {
                request_id,
                stream_id,
                admission,
            })
        }
        "pong" => {
            let w: PongWire = serde_json::from_value(v).map_err(|_| SafeClose::INVALID)?;
            if w.protocol_version != "1.0.0" || w.kind != "pong" {
                return Err(SafeClose::INVALID);
            }
            Ok(ClientMessage::Pong(uuid(&w.heartbeat_id)?))
        }
        _ => Err(SafeClose::INVALID),
    }
}

/// Literal browser origins only. Host/path normalization is not an authorization rule.
pub struct OriginPolicy {
    allowed: BTreeMap<String, ()>,
}
impl OriginPolicy {
    pub fn new(allowed: Vec<String>, allow_loopback_http: bool) -> Result<Self, SafeClose> {
        if allowed.is_empty() || allowed.len() > 16 {
            return Err(SafeClose::UNAVAILABLE);
        }
        let mut origins = BTreeMap::new();
        for origin in allowed {
            let uri: axum::http::Uri = origin.parse().map_err(|_| SafeClose::UNAVAILABLE)?;
            let authority = uri.authority().ok_or(SafeClose::UNAVAILABLE)?;
            let host = authority.host();
            let scheme_ok = uri.scheme_str() == Some("https")
                || (allow_loopback_http
                    && uri.scheme_str() == Some("http")
                    && matches!(host, "127.0.0.1" | "localhost" | "[::1]"));
            let suffix = origin
                .split_once("://")
                .map(|(_, v)| v)
                .ok_or(SafeClose::UNAVAILABLE)?;
            let port_ok = if authority.as_str().ends_with(']') {
                true
            } else if let Some((_, port)) = authority.as_str().rsplit_once(':') {
                !port.is_empty() && port.parse::<u16>().is_ok_and(|n| n > 0)
            } else {
                true
            };
            if !scheme_ok
                || !port_ok
                || origin.len() > 256
                || !origin.is_ascii()
                || suffix.is_empty()
                || suffix
                    .bytes()
                    .any(|b| b.is_ascii_whitespace() || b"/?#@*\\".contains(&b))
                || origins.insert(origin, ()).is_some()
            {
                return Err(SafeClose::UNAVAILABLE);
            }
        }
        Ok(Self { allowed: origins })
    }
    pub fn check(&self, origins: &[&str]) -> Result<(), SafeClose> {
        if origins.len() == 1 && self.allowed.contains_key(origins[0]) {
            Ok(())
        } else {
            Err(SafeClose {
                code: 4403,
                label: "FORBIDDEN",
            })
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum FragmentKind {
    Text,
    Continuation,
    Binary,
}
struct Bucket {
    credit: u64,
    updated: u64,
    rate: u64,
    burst: u64,
}
impl Bucket {
    fn new(rate: u64, burst: u64, now: u64) -> Self {
        Self {
            credit: burst * 1000,
            updated: now,
            rate,
            burst,
        }
    }
    fn take(&mut self, now: u64) -> bool {
        self.credit = self
            .credit
            .saturating_add((now - self.updated).saturating_mul(self.rate))
            .min(self.burst * 1000);
        self.updated = now;
        if self.credit < 1000 {
            false
        } else {
            self.credit -= 1000;
            true
        }
    }
}
pub struct ConnectionIngress {
    bytes: Vec<u8>,
    fragments: usize,
    assembling: bool,
    auth_deadline: u64,
    auth_seen: bool,
    active: Option<(Uuid, Uuid)>,
    last_now: u64,
    all: Bucket,
    input: Bucket,
    closed: bool,
    next_ping: u64,
    outstanding_ping: Option<(Uuid, u64)>,
    last_ping: Option<Uuid>,
}
impl ConnectionIngress {
    pub fn new(now: u64) -> Result<Self, SafeClose> {
        Ok(Self {
            bytes: Vec::with_capacity(CLIENT_BYTES),
            fragments: 0,
            assembling: false,
            auth_deadline: now
                .checked_add(AUTHENTICATE_MS)
                .ok_or(SafeClose::UNAVAILABLE)?,
            auth_seen: false,
            active: None,
            last_now: now,
            all: Bucket::new(40, 80, now),
            input: Bucket::new(30, 60, now),
            closed: false,
            next_ping: now.saturating_add(10000),
            outstanding_ping: None,
            last_ping: None,
        })
    }
    pub fn check_deadline(&mut self, now: u64) -> Result<(), SafeClose> {
        let error = if self.closed || now < self.last_now {
            Some(SafeClose::UNAVAILABLE)
        } else if self.active.is_none() && now >= self.auth_deadline {
            Some(SafeClose::AUTH_TIMEOUT)
        } else if self
            .outstanding_ping
            .is_some_and(|(_, deadline)| now >= deadline)
        {
            Some(SafeClose {
                code: 4408,
                label: "HEARTBEAT_TIMEOUT",
            })
        } else {
            None
        };
        if let Some(e) = error {
            self.closed = true;
            self.bytes.clear();
            return Err(e);
        }
        self.last_now = now;
        Ok(())
    }
    /// Called only after trusted ticket consumption and owner admission, before a snapshot.
    pub fn activate(&mut self, stream: Uuid, epoch: Uuid, now: u64) -> Result<(), SafeClose> {
        self.check_deadline(now)?;
        if !self.auth_seen || stream.is_nil() || epoch.is_nil() {
            return Err(SafeClose::AUTH_REQUIRED);
        }
        self.bytes.clear();
        self.assembling = false;
        self.fragments = 0;
        if self.active.is_none() {
            self.next_ping = now.saturating_add(10000);
        }
        self.active = Some((stream, epoch));
        Ok(())
    }
    /// Hand the returned ping directly to bounded transport. IDs must be freshly server-generated.
    /// Heartbeats never touch membership/access deadlines.
    pub fn ping(&mut self, now: u64, heartbeat_id: Uuid) -> Result<Option<Vec<u8>>, SafeClose> {
        self.check_deadline(now)?;
        if self.active.is_none() || self.outstanding_ping.is_some() || now < self.next_ping {
            return Ok(None);
        }
        if heartbeat_id.is_nil() || self.last_ping == Some(heartbeat_id) {
            return Err(SafeClose::UNAVAILABLE);
        }
        let deadline = now.checked_add(10000).ok_or(SafeClose::UNAVAILABLE)?;
        self.outstanding_ping = Some((heartbeat_id, deadline));
        self.last_ping = Some(heartbeat_id);
        self.next_ping = deadline;
        Ok(Some(serde_json::to_vec(&serde_json::json!({"protocolVersion":"1.0.0","type":"ping","heartbeatId":heartbeat_id})).map_err(|_| SafeClose::UNAVAILABLE)?))
    }

    pub fn buffered_bytes(&self) -> usize {
        self.bytes.len()
    }
    pub fn receive(
        &mut self,
        kind: FragmentKind,
        final_fragment: bool,
        bytes: &[u8],
        now: u64,
    ) -> Result<Option<ClientMessage>, SafeClose> {
        let result = self.receive_inner(kind, final_fragment, bytes, now);
        if result.is_err() {
            self.closed = true;
            self.bytes.clear();
            self.assembling = false;
        }
        result
    }
    fn receive_inner(
        &mut self,
        kind: FragmentKind,
        final_fragment: bool,
        bytes: &[u8],
        now: u64,
    ) -> Result<Option<ClientMessage>, SafeClose> {
        self.check_deadline(now)?;
        match kind {
            FragmentKind::Binary => {
                return Err(SafeClose {
                    code: 1003,
                    label: "UNSUPPORTED_DATA",
                })
            }
            FragmentKind::Text if self.assembling => return Err(SafeClose::INVALID),
            FragmentKind::Continuation if !self.assembling => return Err(SafeClose::INVALID),
            _ => {}
        }
        if !self.assembling {
            self.fragments = 0;
        }
        self.fragments += 1;
        if self.fragments > MAX_FRAGMENTS
            || self.bytes.len().saturating_add(bytes.len()) > CLIENT_BYTES
        {
            return Err(SafeClose::TOO_BIG);
        }
        self.bytes.extend_from_slice(bytes);
        self.assembling = !final_fragment;
        if !final_fragment {
            return Ok(None);
        }
        if !self.all.take(now) {
            return Err(SafeClose::THROTTLED);
        }
        let message = decode(&self.bytes)?;
        self.bytes.clear();
        self.fragments = 0;
        match (&self.active, &message) {
            (None, ClientMessage::Authenticate(_)) if !self.auth_seen => self.auth_seen = true,
            (None, _) => return Err(SafeClose::AUTH_REQUIRED),
            (Some(_), ClientMessage::Authenticate(_)) => return Err(SafeClose::INVALID),
            (Some((stream, epoch)), ClientMessage::Input(input)) => {
                if input.stream_id() != *stream || input.room_epoch() != *epoch {
                    return Err(SafeClose {
                        code: 4409,
                        label: "RESYNC_REQUIRED",
                    });
                }
                if !self.input.take(now) {
                    return Err(SafeClose::THROTTLED);
                }
            }
            (Some((stream, epoch)), ClientMessage::Resync(request))
                if request.stream_id != *stream || request.room_epoch != *epoch =>
            {
                return Err(SafeClose::INVALID)
            }
            (Some((stream, _)), ClientMessage::SwitchRoom { stream_id, .. })
                if stream_id != *stream =>
            {
                return Err(SafeClose::INVALID)
            }
            (Some(_), ClientMessage::Pong(id)) => {
                if !self
                    .outstanding_ping
                    .is_some_and(|(expected, _)| expected == *id)
                {
                    return Err(SafeClose::INVALID);
                }
                self.outstanding_ping = None;
            }
            _ => {}
        }
        Ok(Some(message))
    }
}
