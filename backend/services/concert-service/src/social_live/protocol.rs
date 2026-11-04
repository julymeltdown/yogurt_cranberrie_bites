//! Strict C04 input decoding. Parsed input is correlation, never authority.
use concert_application::relationship::social::{CreateSharing, SharingDecision};
use concert_domain::{
    relationship::social::{ConsentBinding, Platform, PrivateHandle, SharingContext},
    AuthUserId, EventId, Revision, MAX_REVISION,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use tonic::Status;
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SocialOperation {
    Handles,
    SetHandle(Platform),
    RemoveHandle(Platform),
    Create,
    Requests,
    Request(Uuid),
    Decide(Uuid),
    Cancel(Uuid),
    Revoke(Uuid),
    Reveal(Uuid),
    Blocks,
    Block(Uuid),
    Unblock(Uuid),
}
impl SocialOperation {
    pub fn id(self) -> &'static str {
        match self {
            Self::Handles => "listOwnConcertHandles",
            Self::SetHandle(_) => "setConcertHandle",
            Self::RemoveHandle(_) => "removeConcertHandle",
            Self::Create => "createConcertSocialRequest",
            Self::Requests => "listConcertSocialRequests",
            Self::Request(_) => "getConcertSocialRequest",
            Self::Decide(_) => "decideConcertSocialRequest",
            Self::Cancel(_) => "cancelConcertSocialRequest",
            Self::Revoke(_) => "revokeConcertSocialRequest",
            Self::Reveal(_) => "revealConcertSocialHandle",
            Self::Blocks => "listConcertBlocks",
            Self::Block(_) => "blockConcertUser",
            Self::Unblock(_) => "unblockConcertUser",
        }
    }
    pub fn mutation(self) -> bool {
        matches!(
            self,
            Self::SetHandle(_)
                | Self::RemoveHandle(_)
                | Self::Create
                | Self::Decide(_)
                | Self::Cancel(_)
                | Self::Revoke(_)
                | Self::Block(_)
                | Self::Unblock(_)
        )
    }
}
#[derive(Default)]
pub struct Page {
    pub limit: u16,
    pub cursor: Option<String>,
    pub direction: Option<Direction>,
    pub state: Option<String>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Incoming,
    Outgoing,
}
/// Never Debug/Serialize: Set contains a private submitted handle.
pub enum SocialInput {
    Read,
    Page(Page),
    Set {
        expected: i64,
        handle: PrivateHandle,
    },
    Remove(Revision),
    Create(CreateSharing),
    Decide(SharingDecision),
    Cancel(Revision),
    Revoke(Revision),
    Reveal(Platform),
    Block(bool),
}
pub struct SocialRequest {
    pub(crate) operation: SocialOperation,
    pub(crate) input: SocialInput,
    pub(crate) key: Option<Uuid>,
}
fn invalid() -> Status {
    Status::invalid_argument("Invalid social input")
}
pub fn canonical(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or_else(invalid)
}
fn platform(value: &str) -> Result<Platform, Status> {
    value.parse().map_err(|_| invalid())
}
pub fn route(event: Uuid, method: &str, target: &str) -> Result<SocialOperation, Status> {
    if event.is_nil() || target.len() > 4096 || target.chars().any(char::is_control) {
        return Err(invalid());
    }
    let path = target.split_once('?').map_or(target, |(p, _)| p);
    let prefix = format!("/api/concert/events/{event}/");
    let suffix = path.strip_prefix(&prefix).ok_or_else(invalid)?;
    let pieces: Vec<_> = suffix.split('/').collect();
    let operation = match (method, pieces.as_slice()) {
        ("GET", ["me", "social-handles"]) => SocialOperation::Handles,
        ("PUT", ["me", "social-handles", p]) => SocialOperation::SetHandle(platform(p)?),
        ("DELETE", ["me", "social-handles", p]) => SocialOperation::RemoveHandle(platform(p)?),
        ("POST", ["social-requests"]) => SocialOperation::Create,
        ("GET", ["me", "social-requests"]) => SocialOperation::Requests,
        ("GET", ["social-requests", id]) => SocialOperation::Request(canonical(id)?),
        ("POST", ["social-requests", id, "decision"]) => SocialOperation::Decide(canonical(id)?),
        ("DELETE", ["social-requests", id]) => SocialOperation::Cancel(canonical(id)?),
        ("POST", ["social-requests", id, "revoke"]) => SocialOperation::Revoke(canonical(id)?),
        ("GET", ["social-requests", id, "reveal"]) => SocialOperation::Reveal(canonical(id)?),
        ("GET", ["me", "blocks"]) => SocialOperation::Blocks,
        ("PUT", ["me", "blocks", id]) => SocialOperation::Block(canonical(id)?),
        ("DELETE", ["me", "blocks", id]) => SocialOperation::Unblock(canonical(id)?),
        _ => return Err(invalid()),
    };
    Ok(operation)
}
impl SocialRequest {
    pub fn operation(&self) -> SocialOperation {
        self.operation
    }
    pub fn parse(
        actor: Uuid,
        event: Uuid,
        method: &str,
        target: &str,
        body: &[u8],
        content_type: &str,
        key: &str,
    ) -> Result<Self, Status> {
        if actor.is_nil() || body.len() > 8192 {
            return Err(invalid());
        }
        let operation = route(event, method, target)?;
        let key = if operation.mutation() {
            Some(canonical(key)?)
        } else {
            if !key.is_empty() {
                return Err(invalid());
            }
            None
        };
        let mut query = query(target)?;
        let input = match operation {
            SocialOperation::Handles | SocialOperation::Request(_) => {
                empty(body, content_type)?;
                SocialInput::Read
            }
            SocialOperation::Requests | SocialOperation::Blocks => {
                empty(body, content_type)?;
                let limit = query
                    .remove("limit")
                    .map(|n| n.parse::<u16>().map_err(|_| invalid()))
                    .transpose()?
                    .unwrap_or(20);
                if !(1..=100).contains(&limit) {
                    return Err(invalid());
                }
                let cursor = query.remove("cursor");
                if cursor.as_ref().is_some_and(|v| {
                    v.is_empty()
                        || v.len() > 1024
                        || !v
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
                }) {
                    return Err(invalid());
                }
                let (direction, state) = if operation == SocialOperation::Requests {
                    let direction = query
                        .remove("direction")
                        .map(|v| match v.as_str() {
                            "INCOMING" => Ok(Direction::Incoming),
                            "OUTGOING" => Ok(Direction::Outgoing),
                            _ => Err(invalid()),
                        })
                        .transpose()?;
                    let state = query.remove("state");
                    if state.as_ref().is_some_and(|v| {
                        !matches!(
                            v.as_str(),
                            "PENDING"
                                | "APPROVED"
                                | "REJECTED"
                                | "CANCELLED"
                                | "REVOKED"
                                | "EXPIRED"
                        )
                    }) {
                        return Err(invalid());
                    }
                    (direction, state)
                } else {
                    (None, None)
                };
                SocialInput::Page(Page {
                    limit,
                    cursor,
                    direction,
                    state,
                })
            }
            SocialOperation::SetHandle(p) => {
                let b: Set = json(body, content_type)?;
                if !(0..MAX_REVISION).contains(&b.expected_revision) {
                    return Err(invalid());
                }
                SocialInput::Set {
                    expected: b.expected_revision,
                    handle: PrivateHandle::normalize(p, &b.handle).map_err(|_| invalid())?,
                }
            }
            SocialOperation::RemoveHandle(_) | SocialOperation::Cancel(_) => {
                empty(body, content_type)?;
                let expected = query
                    .remove("expected_revision")
                    .ok_or_else(invalid)?
                    .parse()
                    .map_err(|_| invalid())?;
                let revision = Revision::new(expected).map_err(|_| invalid())?;
                if matches!(operation, SocialOperation::RemoveHandle(_)) {
                    SocialInput::Remove(revision)
                } else {
                    SocialInput::Cancel(revision)
                }
            }
            SocialOperation::Create => {
                let b: Create = json(body, content_type)?;
                let value = CreateSharing {
                    context: SharingContext {
                        event: EventId::from_uuid(event),
                        requester: AuthUserId::from_uuid(actor),
                        recipient: AuthUserId::from_uuid(canonical(&b.recipient_id)?),
                        intro_id: canonical(&b.intro_id)?,
                        intro_revision: Revision::new(b.intro_revision).map_err(|_| invalid())?,
                    },
                    consents: b
                        .consents
                        .into_iter()
                        .map(|c| {
                            Ok(ConsentBinding {
                                platform: platform(&c.platform)?,
                                requester_version: Revision::new(c.requester_handle_version)
                                    .map_err(|_| invalid())?,
                                recipient_version: Revision::new(c.recipient_handle_version)
                                    .map_err(|_| invalid())?,
                                requester_consents: c.requester_consents,
                            })
                        })
                        .collect::<Result<_, Status>>()?,
                };
                value.validate().map_err(|_| invalid())?;
                SocialInput::Create(value)
            }
            SocialOperation::Decide(_) => {
                let b: Decision = json(body, content_type)?;
                let approve = match b.decision.as_str() {
                    "APPROVED" => true,
                    "REJECTED" => false,
                    _ => return Err(invalid()),
                };
                let value = SharingDecision {
                    expected: Revision::new(b.expected_revision).map_err(|_| invalid())?,
                    approve,
                    approved_platforms: b
                        .approved_platforms
                        .into_iter()
                        .map(|v| platform(&v))
                        .collect::<Result<_, _>>()?,
                    recipient_consents: b.recipient_consents,
                };
                value.validate().map_err(|_| invalid())?;
                SocialInput::Decide(value)
            }
            SocialOperation::Revoke(_) => {
                let b: Expected = json(body, content_type)?;
                SocialInput::Revoke(Revision::new(b.expected_revision).map_err(|_| invalid())?)
            }
            SocialOperation::Reveal(_) => {
                empty(body, content_type)?;
                SocialInput::Reveal(platform(&query.remove("platform").ok_or_else(invalid)?)?)
            }
            SocialOperation::Block(_) => {
                let _: Empty = json(body, content_type)?;
                SocialInput::Block(true)
            }
            SocialOperation::Unblock(_) => {
                empty(body, content_type)?;
                SocialInput::Block(false)
            }
        };
        if !query.is_empty() {
            return Err(invalid());
        }
        Ok(Self {
            operation,
            input,
            key,
        })
    }
}
fn empty(body: &[u8], content: &str) -> Result<(), Status> {
    if !body.is_empty() || !content.is_empty() {
        Err(invalid())
    } else {
        Ok(())
    }
}
fn json<T: serde::de::DeserializeOwned>(body: &[u8], content: &str) -> Result<T, Status> {
    if content != "application/json" || body.is_empty() {
        return Err(invalid());
    }
    serde_json::from_slice(body).map_err(|_| invalid())
}
fn query(target: &str) -> Result<BTreeMap<String, String>, Status> {
    let mut result = BTreeMap::new();
    if let Some((_, q)) = target.split_once('?') {
        if q.is_empty() {
            return Err(invalid());
        }
        for pair in q.split('&') {
            let (name, value) = pair.split_once('=').ok_or_else(invalid)?;
            let name = decode(name)?;
            let value = decode(value)?;
            if name.is_empty() || value.is_empty() || result.insert(name, value).is_some() {
                return Err(invalid());
            }
        }
    }
    Ok(result)
}
fn decode(value: &str) -> Result<String, Status> {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let h = std::str::from_utf8(&bytes[i + 1..i + 3]).map_err(|_| invalid())?;
                out.push(u8::from_str_radix(h, 16).map_err(|_| invalid())?);
                i += 3;
            }
            b'%' => return Err(invalid()),
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    let s = String::from_utf8(out).map_err(|_| invalid())?;
    if s.chars().any(char::is_control) {
        Err(invalid())
    } else {
        Ok(s)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Set {
    expected_revision: i64,
    handle: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    expected_revision: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    recipient_id: String,
    intro_id: String,
    intro_revision: i64,
    consents: Vec<Consent>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Consent {
    platform: String,
    requester_handle_version: i64,
    recipient_handle_version: i64,
    requester_consents: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decision {
    expected_revision: i64,
    decision: String,
    approved_platforms: Vec<String>,
    recipient_consents: bool,
}
