//! Separate claimed INTERNAL notification protocol. No listener or key is
//! activated here. Source, selected membership, policy and writes are distinct.
use super::*;
use crate::grpc_auth::VerifiedCaller;
use http_body_util::{BodyExt, Full, Limited};
use tonic::server::NamedService;

pub const PAGE_METHOD: &str =
    "/authority.v1.NotificationProjectionService/ListNotificationFollowers";
pub const WRITE_METHOD: &str = "/notification.v1.NotificationService/CreateNotification";
pub const POLICY_METHOD: &str =
    "/authority.v1.NotificationPolicyService/CheckNotificationRecipient";
pub const HEADER: &str = "x-notification-grant";
pub const MAX_BYTES: usize = 16 * 1024;
const MAX_WITNESS: usize = 8 * 1024;
const MAX_LIFETIME: i64 = 60_000_000;
const OWNER_CONTEXT: &[u8] = b"port-rust notification source owner v1\n";
const MEMBERSHIP_CONTEXT: &[u8] = b"port-rust notification selected membership v1\n";
const POLICY_QUERY_CONTEXT: &[u8] = b"port-rust notification policy query v1\n";
const POLICY_CONTEXT: &[u8] = b"port-rust notification current policy v1\n";
const GRANT_CONTEXT: &[u8] = b"port-rust notification gateway grant v1\n";

#[derive(Clone, Copy)]
pub struct LifetimePolicy(i64);
impl LifetimePolicy {
    pub fn new(micros: i64) -> Result<Self, Denial> {
        if !(1..=MAX_LIFETIME).contains(&micros) {
            return Err(Denial::LifetimeTooLong);
        }
        Ok(Self(micros))
    }
}

/// Immutable source and actual INTERNAL attempt. Only a lease digest travels;
/// owning Post must independently compare it to its stored live lease token.
/// This admits count0 replay only when the owner validates stored provenance.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationClaim {
    pub event_id: Uuid,
    pub aggregate_id: Uuid,
    pub event_digest: String,
    pub attempt_digest: String,
    pub lease_digest: String,
    pub attempt_id: Uuid,
    pub attempt: i32,
    pub lease_expires_micros: i64,
}
impl NotificationClaim {
    pub fn digest(bytes: &[u8]) -> String {
        body_digest(bytes)
    }
    pub fn decode_digest(value: &str) -> Result<[u8; 32], Denial> {
        decode_digest(value)?
            .try_into()
            .map_err(|_| Denial::WrongBody)
    }
    pub fn encode_digest(value: &[u8; 32]) -> String {
        Base64UrlUnpadded::encode_string(value)
    }
    fn validate(&self) -> Result<(), Denial> {
        if [self.event_id, self.aggregate_id, self.attempt_id]
            .iter()
            .any(Uuid::is_nil)
            || self.attempt < 0
            || self.lease_expires_micros <= 0
            || chrono::DateTime::from_timestamp_micros(self.lease_expires_micros).is_none()
            || [&self.event_digest, &self.attempt_digest, &self.lease_digest]
                .iter()
                .any(|s| decode_digest(s).is_err())
        {
            return Err(Denial::WrongBody);
        }
        Ok(())
    }
    fn proto(&self) -> Result<Vec<u8>, Denial> {
        self.validate()?;
        let mut b = Vec::new();
        string(&mut b, 1, &self.event_id.to_string());
        string(&mut b, 2, &self.aggregate_id.to_string());
        bytes(&mut b, 3, &decode_digest(&self.event_digest)?);
        bytes(&mut b, 4, &decode_digest(&self.attempt_digest)?);
        bytes(&mut b, 5, &decode_digest(&self.lease_digest)?);
        string(&mut b, 6, &self.attempt_id.to_string());
        number(&mut b, 7, self.attempt as u64);
        number(&mut b, 8, self.lease_expires_micros as u64);
        Ok(b)
    }
}
fn decode_digest(s: &str) -> Result<Vec<u8>, Denial> {
    let b = Base64UrlUnpadded::decode_vec(s).map_err(|_| Denial::WrongBody)?;
    if b.len() != 32 || Base64UrlUnpadded::encode_string(&b) != s {
        return Err(Denial::WrongBody);
    }
    Ok(b)
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationPage {
    pub claim: NotificationClaim,
    pub author_id: Uuid,
    pub cursor: Option<String>,
    pub limit: i32,
}
fn cursor(value: Option<&str>) -> Result<Option<(chrono::DateTime<chrono::Utc>, Uuid)>, Denial> {
    value
        .map(|s| {
            if s.len() > 128 || s.trim() != s {
                return Err(Denial::WrongBody);
            }
            let (time, id) = s.split_once('|').ok_or(Denial::WrongBody)?;
            let time = chrono::DateTime::parse_from_rfc3339(time)
                .map_err(|_| Denial::WrongBody)?
                .with_timezone(&chrono::Utc);
            let id = Uuid::parse_str(id).map_err(|_| Denial::WrongBody)?;
            if id.is_nil() {
                return Err(Denial::WrongBody);
            }
            Ok((time, id))
        })
        .transpose()
}
impl NotificationPage {
    fn validate(&self) -> Result<(), Denial> {
        self.claim.validate()?;
        cursor(self.cursor.as_deref())?;
        if self.author_id.is_nil() || !(1..=100).contains(&self.limit) {
            return Err(Denial::WrongBody);
        }
        Ok(())
    }
    /// Canonical additive authority.v1 page request. Its adapter must compare
    /// this frame with the generated Proto message, including optional cursor.
    pub fn frame(&self) -> Result<Vec<u8>, Denial> {
        self.validate()?;
        let mut b = Vec::new();
        bytes(&mut b, 1, &self.claim.proto()?);
        string(&mut b, 2, &self.author_id.to_string());
        if let Some(c) = &self.cursor {
            bytes(&mut b, 3, c.as_bytes());
        }
        number(&mut b, 4, self.limit as u64);
        Ok(frame(b))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationKind {
    #[serde(rename = "POST_CREATED")]
    Created,
    #[serde(rename = "POST_LIKED")]
    Liked,
    #[serde(rename = "POST_COMMENTED")]
    Commented,
    #[serde(rename = "COMMENT")]
    LegacyComment,
}
impl NotificationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Created => "POST_CREATED",
            Self::Liked => "POST_LIKED",
            Self::Commented => "POST_COMMENTED",
            Self::LegacyComment => "COMMENT",
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceVisibility {
    Public,
    Friends,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationDelivery {
    pub claim: NotificationClaim,
    pub recipient_id: Uuid,
    pub actor_id: Uuid,
    pub post_id: Uuid,
    pub kind: NotificationKind,
    /// Exact original stored occurrence string, not a new timestamp/default.
    pub occurred_at: String,
    pub membership: Option<String>,
}
impl NotificationDelivery {
    fn validate(&self) -> Result<(), Denial> {
        self.claim.validate()?;
        if [self.recipient_id, self.actor_id, self.post_id]
            .iter()
            .any(Uuid::is_nil)
            || self.recipient_id == self.actor_id
            || self.occurred_at.len() > 64
            || chrono::DateTime::parse_from_rfc3339(&self.occurred_at).is_err()
            || self.occurred_at.trim() != self.occurred_at
            || (self.kind != NotificationKind::LegacyComment
                && self.post_id != self.claim.aggregate_id)
            || (self.kind == NotificationKind::Created && self.membership.is_none())
            || (self.kind != NotificationKind::Created && self.membership.is_some())
            || self
                .membership
                .as_ref()
                .is_some_and(|s| s.is_empty() || s.len() > MAX_WITNESS)
        {
            return Err(Denial::WrongBody);
        }
        Ok(())
    }
    /// Exact original notification.v1.CreateNotificationRequest, fields1..6.
    /// No omitted IDs, normalization or mutable text is allowed in this path.
    pub fn frame(&self) -> Result<Vec<u8>, Denial> {
        self.validate()?;
        let mut b = Vec::new();
        string(&mut b, 1, &self.recipient_id.to_string());
        string(&mut b, 2, self.kind.as_str());
        string(
            &mut b,
            3,
            &serde_json::json!({"payloadVersion":1,"postId":self.post_id}).to_string(),
        );
        string(&mut b, 4, &self.claim.event_id.to_string());
        string(&mut b, 5, &self.actor_id.to_string());
        string(&mut b, 6, &self.occurred_at);
        Ok(frame(b))
    }
}

/// Strictly typed, canonical envelope carried only through dedicated issuer/
/// admission RPCs and the signed grant. No caller-supplied free-form method.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "purpose", content = "binding", deny_unknown_fields)]
pub enum NotificationBinding {
    FollowerPage(NotificationPage),
    Delivery(NotificationDelivery),
}
impl NotificationBinding {
    pub fn claim(&self) -> &NotificationClaim {
        match self {
            Self::FollowerPage(p) => &p.claim,
            Self::Delivery(d) => &d.claim,
        }
    }
    pub fn method(&self) -> &'static str {
        match self {
            Self::FollowerPage(_) => PAGE_METHOD,
            Self::Delivery(_) => WRITE_METHOD,
        }
    }
    pub fn audience(&self) -> Workload {
        match self {
            Self::FollowerPage(_) => Workload::Friend,
            Self::Delivery(_) => Workload::Notification,
        }
    }
    pub fn frame(&self) -> Result<Vec<u8>, Denial> {
        match self {
            Self::FollowerPage(p) => p.frame(),
            Self::Delivery(d) => d.frame(),
        }
    }
    pub fn encode(&self) -> Result<Vec<u8>, Denial> {
        let b = serde_json::to_vec(self).map_err(|_| Denial::Malformed)?;
        if b.len() > MAX_BYTES {
            return Err(Denial::BodyTooLarge);
        }
        Ok(b)
    }
    pub fn decode(value: &[u8]) -> Result<Self, Denial> {
        if value.len() > MAX_BYTES {
            return Err(Denial::BodyTooLarge);
        }
        let b: Self = serde_json::from_slice(value).map_err(|_| Denial::Malformed)?;
        if b.encode()? != value {
            return Err(Denial::Malformed);
        }
        b.frame()?;
        Ok(b)
    }
    fn validate_wire(&self, body: &[u8]) -> Result<(), Denial> {
        if body.len() > MAX_BYTES {
            return Err(Denial::BodyTooLarge);
        }
        unary_message(body)?;
        if self.frame()? != body {
            return Err(Denial::WrongBody);
        }
        Ok(())
    }
    fn visibility(&self, v: Option<SourceVisibility>) -> Result<(), Denial> {
        let created = matches!(
            self,
            Self::FollowerPage(_)
                | Self::Delivery(NotificationDelivery {
                    kind: NotificationKind::Created,
                    ..
                })
        );
        if created != v.is_some() {
            return Err(Denial::WrongBody);
        }
        Ok(())
    }
}
fn varint(out: &mut Vec<u8>, mut n: u64) {
    while n >= 128 {
        out.push((n as u8 & 127) | 128);
        n >>= 7;
    }
    out.push(n as u8);
}
fn bytes(out: &mut Vec<u8>, field: u32, value: &[u8]) {
    varint(out, ((field << 3) | 2) as u64);
    varint(out, value.len() as u64);
    out.extend_from_slice(value);
}
fn string(out: &mut Vec<u8>, field: u32, value: &str) {
    if !value.is_empty() {
        bytes(out, field, value.as_bytes());
    }
}
fn number(out: &mut Vec<u8>, field: u32, value: u64) {
    if value != 0 {
        varint(out, (field << 3) as u64);
        varint(out, value);
    }
}
fn frame(b: Vec<u8>) -> Vec<u8> {
    let mut r = vec![0];
    r.extend_from_slice(&(b.len() as u32).to_be_bytes());
    r.extend(b);
    r
}

#[derive(Clone)]
struct Key {
    key: Arc<Ed25519KeyPair>,
    id: String,
}
impl Key {
    fn new(seed: &[u8; 32]) -> anyhow::Result<Self> {
        let key = Ed25519KeyPair::from_seed_unchecked(seed)
            .map_err(|_| anyhow::anyhow!("Invalid notification signing seed"))?;
        let id = key_id(key.public_key().as_ref());
        Ok(Self {
            key: Arc::new(key),
            id,
        })
    }
    fn public_key(&self) -> [u8; 32] {
        self.key
            .public_key()
            .as_ref()
            .try_into()
            .expect("Ed25519 key")
    }
    fn sign(&self, context: &[u8], value: &impl Serialize) -> Result<String, Denial> {
        let p = Base64UrlUnpadded::encode_string(
            &serde_json::to_vec(value).map_err(|_| Denial::Malformed)?,
        );
        let s = self.key.sign(&[context, p.as_bytes()].concat());
        let token = format!("{p}.{}", Base64UrlUnpadded::encode_string(s.as_ref()));
        if token.len() > MAX_BYTES {
            return Err(Denial::BodyTooLarge);
        }
        Ok(token)
    }
}
#[derive(Clone)]
struct Keys(Arc<Vec<(String, [u8; 32])>>);
impl Keys {
    fn new(keys: &[[u8; 32]]) -> anyhow::Result<Self> {
        anyhow::ensure!(!keys.is_empty(), "Notification verification keys required");
        Ok(Self(Arc::new(
            keys.iter().map(|k| (key_id(k), *k)).collect(),
        )))
    }
    fn verify<T: serde::de::DeserializeOwned>(
        &self,
        context: &[u8],
        token: &str,
    ) -> Result<T, Denial> {
        if token.len() > MAX_BYTES {
            return Err(Denial::BodyTooLarge);
        }
        let (p, s) = token.split_once('.').ok_or(Denial::Malformed)?;
        let b = Base64UrlUnpadded::decode_vec(p).map_err(|_| Denial::Malformed)?;
        let v: serde_json::Value = serde_json::from_slice(&b).map_err(|_| Denial::Malformed)?;
        let id = v
            .get("kid")
            .and_then(serde_json::Value::as_str)
            .ok_or(Denial::Malformed)?;
        let (_, key) = self
            .0
            .iter()
            .find(|(kid, _)| kid == id)
            .ok_or(Denial::UnknownKey)?;
        let s = Base64UrlUnpadded::decode_vec(s).map_err(|_| Denial::Malformed)?;
        UnparsedPublicKey::new(&ED25519, key)
            .verify(&[context, p.as_bytes()].concat(), &s)
            .map_err(|_| Denial::BadSignature)?;
        serde_json::from_slice(&b).map_err(|_| Denial::Malformed)
    }
}
/// Cross-process timestamps carry the same bounded skew this crate already
/// tolerates for Gateway/owner clocks. The notification evidence travels between
/// Gateway, Post and Friend, whose clocks are not synchronised, so an exact
/// microsecond ordering cannot hold: measured on this host against a live
/// PostgreSQL, one process ran 0.04-0.15 s ahead of another. Without this the
/// same request is admitted or refused depending on which side happens to read
/// its clock first. Replay, expiry and the lease ceiling remain exact.
const CLOCK_SKEW_MICROS: i64 = crate::actor_delegation::CLOCK_SKEW_SECONDS * 1_000_000;

fn valid_time(iat: i64, exp: i64, now: i64, lease: i64) -> Result<(), Denial> {
    if exp <= iat || exp.saturating_sub(iat) > MAX_LIFETIME || exp > lease {
        return Err(Denial::LifetimeTooLong);
    }
    if now.saturating_add(CLOCK_SKEW_MICROS) < iat {
        return Err(Denial::NotYetValid);
    }
    if now >= exp {
        return Err(Denial::Expired);
    }
    Ok(())
}

mod authority;
pub use authority::*;
mod deadline;
pub use deadline::*;
mod guard;
pub use guard::*;
