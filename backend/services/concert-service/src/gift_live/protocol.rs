//! Closed C04 input boundary. Original body/key/target remain available to owner fencing.
use chrono::{DateTime, Utc};
use concert_application::{commands::CommandError, community::giveaway::PickupCode};
use concert_domain::community::giveaway::{GiftAction, GiftFields, GiftState, PickupMeeting};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, sync::OnceLock, time::Duration};
use uuid::Uuid;

pub const BUDGET: Duration = Duration::from_secs(2);
pub const MAX_BODY: usize = 16 * 1024;
/// Captured by the guarded product transport. This is correlation, NOT permission;
/// runtime must acquire the canonical owning scope from the actual sealed parent.
#[derive(Clone, Copy)]
pub struct GiftIdentity {
    pub actor: Uuid,
    pub event: Uuid,
    pub session_family: Uuid,
    pub request_id: Uuid,
    pub deadline: AuthorityRpcDeadline,
}
impl GiftIdentity {
    pub fn check(&self) -> Result<(), InputError> {
        if [self.actor, self.event, self.session_family, self.request_id]
            .iter()
            .any(Uuid::is_nil)
        {
            return Err(InputError::Invalid);
        }
        self.deadline
            .remaining(BUDGET, self.deadline.expires_micros())
            .map_err(|_| InputError::Expired)?;
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputError {
    Invalid,
    Expired,
    Unavailable,
}
impl From<CommandError> for InputError {
    fn from(_: CommandError) -> Self {
        Self::Invalid
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GiftOperation {
    List,
    Detail,
    Create,
    Update,
    Management,
    SetMeeting,
    Reserve,
    Activity,
    Reservation,
    Cancel,
    Claim,
    Pickup,
    IssueCode,
}
impl GiftOperation {
    pub fn id(self) -> &'static str {
        match self {
            Self::List => "listConcertGiveaways",
            Self::Detail => "getConcertGiveaway",
            Self::Create => "createConcertGiveaway",
            Self::Update => "updateConcertGiveaway",
            Self::Management => "getConcertGiveawayManagement",
            Self::SetMeeting => "setConcertPickupMeeting",
            Self::Reserve => "reserveConcertGiveaway",
            Self::Activity => "getConcertGiftActivity",
            Self::Reservation => "getConcertReservation",
            Self::Cancel => "cancelConcertReservation",
            Self::Claim => "claimConcertReservation",
            Self::Pickup => "getConcertPickup",
            Self::IssueCode => "issueConcertPickupCode",
        }
    }
    pub fn mutation(self) -> bool {
        matches!(
            self,
            Self::Create
                | Self::Update
                | Self::SetMeeting
                | Self::Reserve
                | Self::Cancel
                | Self::Claim
                | Self::IssueCode
        )
    }
    fn method(self) -> &'static str {
        match self {
            Self::Create | Self::Reserve | Self::Claim | Self::IssueCode => "POST",
            Self::Update => "PATCH",
            Self::SetMeeting => "PUT",
            Self::Cancel => "DELETE",
            _ => "GET",
        }
    }
    /// Exact route classification for the frozen thirteen Gift operations.
    /// This parses shape only; `GiftRequest::from_verified` still validates the
    /// body, query, media type and keyed mutation envelope.
    pub fn route(event: Uuid, method: &str, target: &str) -> Result<Self, InputError> {
        if event.is_nil() || target.len() > 4096 || target.chars().any(char::is_control) {
            return Err(InputError::Invalid);
        }
        let (path, _) = target
            .split_once('?')
            .map_or((target, None), |(a, b)| (a, Some(b)));
        let suffix = path
            .strip_prefix(&format!("/api/concert/events/{event}/"))
            .ok_or(InputError::Invalid)?;
        let pieces: Vec<_> = suffix.split('/').collect();
        let uuid = |value: &str| {
            Uuid::parse_str(value)
                .ok()
                .filter(|id| !id.is_nil() && id.to_string() == value)
        };
        let operation = match (method, pieces.as_slice()) {
            ("GET", ["giveaways"]) => Self::List,
            ("POST", ["giveaways"]) => Self::Create,
            ("GET", ["giveaways", _]) => Self::Detail,
            ("PATCH", ["giveaways", _]) => Self::Update,
            ("GET", ["giveaways", _, "management"]) => Self::Management,
            ("PUT", ["giveaways", _, "pickup-meeting"]) => Self::SetMeeting,
            ("POST", ["giveaways", _, "reservations"]) => Self::Reserve,
            ("GET", ["me", "giveaways"]) => Self::Activity,
            ("GET", ["reservations", _]) => Self::Reservation,
            ("DELETE", ["reservations", _]) => Self::Cancel,
            ("POST", ["reservations", _, "pickup-code"]) => Self::IssueCode,
            ("POST", ["reservations", _, "claim"]) => Self::Claim,
            ("GET", ["reservations", _, "pickup"]) => Self::Pickup,
            _ => return Err(InputError::Invalid),
        };
        if pieces.len() >= 2 && pieces[0] != "me" && uuid(pieces[1]).is_none() {
            return Err(InputError::Invalid);
        }
        Ok(operation)
    }
    pub(crate) fn parse(id: &str) -> Result<Self, InputError> {
        [
            Self::List,
            Self::Detail,
            Self::Create,
            Self::Update,
            Self::Management,
            Self::SetMeeting,
            Self::Reserve,
            Self::Activity,
            Self::Reservation,
            Self::Cancel,
            Self::Claim,
            Self::Pickup,
            Self::IssueCode,
        ]
        .into_iter()
        .find(|op| op.id() == id)
        .ok_or(InputError::Invalid)
    }
}
/// No Debug/Serialize: action may contain a submitted pickup code.
pub enum GiftInput {
    Read {
        target: Option<Uuid>,
        query: BTreeMap<String, String>,
    },
    Write {
        giveaway: Option<Uuid>,
        reservation: Option<Uuid>,
        action: GiftAction,
        code: Option<PickupCode>,
    },
}
/// Carries exact immutable edge material for current owner binding/registration.
/// Never serialize/log requests: pickup codes live only in request memory.
pub struct GiftRequest {
    identity: GiftIdentity,
    operation: GiftOperation,
    method: String,
    target: String,
    content_type: String,
    key: Option<Uuid>,
    body: Vec<u8>,
    input: GiftInput,
}
impl GiftRequest {
    pub fn from_verified(
        identity: GiftIdentity,
        operation: &str,
        method: &str,
        target: &str,
        content_type: &str,
        idempotency_key: &str,
        body: Vec<u8>,
    ) -> Result<Self, InputError> {
        identity.check()?;
        let op = GiftOperation::parse(operation)?;
        if method != op.method()
            || target.len() > 4096
            || target.chars().any(char::is_control)
            || body.len() > MAX_BODY
        {
            return Err(InputError::Invalid);
        }
        let (path, raw_query) = target
            .split_once('?')
            .map_or((target, None), |(a, b)| (a, Some(b)));
        let suffix = path
            .strip_prefix(&format!("/api/concert/events/{}/", identity.event))
            .ok_or(InputError::Invalid)?;
        let pieces: Vec<_> = suffix.split('/').collect();
        let (root, id, tail) = match pieces.as_slice() {
            ["giveaways"] => ("giveaways", None, None),
            ["me", "giveaways"] => ("activity", None, None),
            [root, id] if matches!(*root, "giveaways" | "reservations") => {
                (*root, Some(canonical_uuid(id)?), None)
            }
            [root, id, tail] if matches!(*root, "giveaways" | "reservations") => {
                (*root, Some(canonical_uuid(id)?), Some(*tail))
            }
            _ => return Err(InputError::Invalid),
        };
        let route = match op {
            GiftOperation::List | GiftOperation::Create => {
                root == "giveaways" && id.is_none() && tail.is_none()
            }
            GiftOperation::Detail | GiftOperation::Update => {
                root == "giveaways" && id.is_some() && tail.is_none()
            }
            GiftOperation::Management => root == "giveaways" && tail == Some("management"),
            GiftOperation::SetMeeting => root == "giveaways" && tail == Some("pickup-meeting"),
            GiftOperation::Reserve => root == "giveaways" && tail == Some("reservations"),
            GiftOperation::Activity => root == "activity",
            GiftOperation::Reservation | GiftOperation::Cancel => {
                root == "reservations" && id.is_some() && tail.is_none()
            }
            GiftOperation::Claim => root == "reservations" && tail == Some("claim"),
            GiftOperation::Pickup => root == "reservations" && tail == Some("pickup"),
            GiftOperation::IssueCode => root == "reservations" && tail == Some("pickup-code"),
        };
        if !route {
            return Err(InputError::Invalid);
        }
        let query = parse_query(raw_query)?;
        let key = if op.mutation() {
            Some(canonical_uuid(idempotency_key)?)
        } else {
            if !idempotency_key.is_empty() {
                return Err(InputError::Invalid);
            }
            None
        };
        let input = if !op.mutation() {
            if !body.is_empty() || !content_type.is_empty() {
                return Err(InputError::Invalid);
            }
            validate_query(op, &query)?;
            GiftInput::Read { target: id, query }
        } else {
            if op == GiftOperation::Cancel {
                if !body.is_empty() || !content_type.is_empty() || query.len() != 1 {
                    return Err(InputError::Invalid);
                }
            } else if content_type != "application/json" || !query.is_empty() {
                return Err(InputError::Invalid);
            }
            let (action, code) = match op {
                GiftOperation::Create => {
                    let f: GiftFields = wire("CreateGiveaway", &body)?;
                    valid_fields(&f)?;
                    (GiftAction::Create(f), None)
                }
                GiftOperation::Update => {
                    let f: Update = wire("UpdateGiveaway", &body)?;
                    let fields = f.fields();
                    valid_fields(&fields)?;
                    (
                        GiftAction::Update {
                            expected: revision(f.expected_revision)?,
                            fields,
                            state: f.state,
                        },
                        None,
                    )
                }
                GiftOperation::SetMeeting => {
                    let f: Meeting = wire("SetPickupMeeting", &body)?;
                    if !f.physical_presence_acknowledged || f.meeting.place_id.is_nil() {
                        return Err(InputError::Invalid);
                    }
                    revision(f.meeting.map_revision)?;
                    revision(f.meeting.place_revision)?;
                    (
                        GiftAction::SetMeeting {
                            expected: revision(f.expected_revision)?,
                            meeting: f.meeting,
                            physical_acknowledged: true,
                        },
                        None,
                    )
                }
                GiftOperation::Reserve => {
                    let f: Reserve = wire("Reserve", &body)?;
                    (
                        GiftAction::Reserve {
                            giveaway_revision: revision(f.giveaway_revision)?,
                        },
                        None,
                    )
                }
                GiftOperation::Cancel => {
                    let expected = query.get("expected_revision").ok_or(InputError::Invalid)?;
                    (
                        GiftAction::Cancel {
                            reservation: id.ok_or(InputError::Invalid)?,
                            expected: parse_revision(expected)?,
                        },
                        None,
                    )
                }
                GiftOperation::IssueCode => {
                    let f: Expected = wire("IssuePickupCode", &body)?;
                    (
                        GiftAction::IssueCode {
                            reservation: id.ok_or(InputError::Invalid)?,
                            expected: revision(f.expected_revision)?,
                        },
                        None,
                    )
                }
                GiftOperation::Claim => {
                    let f: Claim = wire("ClaimReservation", &body)?;
                    (
                        GiftAction::Claim {
                            reservation: id.ok_or(InputError::Invalid)?,
                            expected: revision(f.expected_revision)?,
                        },
                        Some(PickupCode::parse(f.confirmation_code)?),
                    )
                }
                _ => return Err(InputError::Invalid),
            };
            GiftInput::Write {
                giveaway: if root == "giveaways" { id } else { None },
                reservation: if root == "reservations" { id } else { None },
                action,
                code,
            }
        };
        // Cold schema compilation is bounded by the same original stop, too.
        identity.check()?;
        Ok(Self {
            identity,
            operation: op,
            method: method.into(),
            target: target.into(),
            content_type: content_type.into(),
            key,
            body,
            input,
        })
    }
    pub fn identity(&self) -> GiftIdentity {
        self.identity
    }
    pub fn operation(&self) -> GiftOperation {
        self.operation
    }
    pub fn method(&self) -> &str {
        &self.method
    }
    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn content_type(&self) -> &str {
        &self.content_type
    }
    pub fn idempotency_key(&self) -> Option<Uuid> {
        self.key
    }
    pub fn body(&self) -> &[u8] {
        &self.body
    }
    pub fn input(&self) -> &GiftInput {
        &self.input
    }
    pub(crate) fn into_input(self) -> GiftInput {
        self.input
    }
}
pub enum GiftReply {
    Json { status: u16, body: Value },
}
pub fn canonical_uuid(raw: &str) -> Result<Uuid, InputError> {
    Uuid::parse_str(raw)
        .ok()
        .filter(|v| !v.is_nil() && v.to_string() == raw)
        .ok_or(InputError::Invalid)
}
fn revision(v: i64) -> Result<i64, InputError> {
    if (1..=concert_domain::MAX_REVISION).contains(&v) {
        Ok(v)
    } else {
        Err(InputError::Invalid)
    }
}
fn parse_revision(v: &str) -> Result<i64, InputError> {
    let n = v.parse::<i64>().map_err(|_| InputError::Invalid)?;
    if n.to_string() != v {
        return Err(InputError::Invalid);
    }
    revision(n)
}
fn valid_fields(f: &GiftFields) -> Result<(), InputError> {
    if f.public_place_id.is_nil() || f.image_media_id.is_some_and(|v| v.is_nil()) {
        Err(InputError::Invalid)
    } else {
        Ok(())
    }
}
fn wire<T: serde::de::DeserializeOwned>(schema: &str, body: &[u8]) -> Result<T, InputError> {
    // Typed deserialization rejects duplicate keys before Value could erase them.
    let typed = serde_json::from_slice(body).map_err(|_| InputError::Invalid)?;
    let value: Value = serde_json::from_slice(body).map_err(|_| InputError::Invalid)?;
    static CONTRACT: OnceLock<Value> = OnceLock::new();
    let contract = CONTRACT.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../contracts/generated/mcr-domain.openapi.json"
        ))
        .expect("pinned C04 JSON")
    });
    // Compile each pinned schema once; requests retain their original budget.
    type Cached = OnceLock<Result<jsonschema::Validator, ()>>;
    static CREATE: Cached = OnceLock::new();
    static UPDATE: Cached = OnceLock::new();
    static MEETING: Cached = OnceLock::new();
    static RESERVE: Cached = OnceLock::new();
    static ISSUE: Cached = OnceLock::new();
    static CLAIM: Cached = OnceLock::new();
    let cache = match schema {
        "CreateGiveaway" => &CREATE,
        "UpdateGiveaway" => &UPDATE,
        "SetPickupMeeting" => &MEETING,
        "Reserve" => &RESERVE,
        "IssuePickupCode" => &ISSUE,
        "ClaimReservation" => &CLAIM,
        _ => return Err(InputError::Unavailable),
    };
    let validator = cache.get_or_init(|| jsonschema::validator_for(
        &serde_json::json!({"$ref":format!("#/components/schemas/Domain_giveaway_{schema}"),"components":contract["components"]})
    ).map_err(|_| ())).as_ref().map_err(|_| InputError::Unavailable)?;
    if !validator.is_valid(&value) {
        return Err(InputError::Invalid);
    }
    Ok(typed)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Update {
    expected_revision: i64,
    title: String,
    description: String,
    image_media_id: Option<Uuid>,
    quantity_total: i32,
    pickup_starts_at: DateTime<Utc>,
    pickup_ends_at: DateTime<Utc>,
    public_place_id: Uuid,
    state: GiftState,
}
impl Update {
    fn fields(&self) -> GiftFields {
        GiftFields {
            title: self.title.clone(),
            description: self.description.clone(),
            image_media_id: self.image_media_id,
            quantity_total: self.quantity_total,
            pickup_starts_at: self.pickup_starts_at,
            pickup_ends_at: self.pickup_ends_at,
            public_place_id: self.public_place_id,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Meeting {
    expected_revision: i64,
    meeting: PickupMeeting,
    physical_presence_acknowledged: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reserve {
    giveaway_revision: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    expected_revision: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Claim {
    expected_revision: i64,
    confirmation_code: String,
}
pub(crate) fn parse_query(raw: Option<&str>) -> Result<BTreeMap<String, String>, InputError> {
    let mut query = BTreeMap::new();
    if let Some(raw) = raw {
        if raw.is_empty() {
            return Err(InputError::Invalid);
        }
        for pair in raw.split('&') {
            let (k, v) = pair.split_once('=').ok_or(InputError::Invalid)?;
            let k = decode(k)?;
            let v = decode(v)?;
            if k.is_empty() || v.is_empty() || query.insert(k, v).is_some() {
                return Err(InputError::Invalid);
            }
        }
    }
    Ok(query)
}
fn decode(raw: &str) -> Result<String, InputError> {
    let mut out = Vec::with_capacity(raw.len());
    let mut bytes = raw.bytes();
    while let Some(b) = bytes.next() {
        out.push(match b {
            b'+' => b' ',
            b'%' => {
                let a = bytes
                    .next()
                    .and_then(|b| (b as char).to_digit(16))
                    .ok_or(InputError::Invalid)?;
                let b = bytes
                    .next()
                    .and_then(|b| (b as char).to_digit(16))
                    .ok_or(InputError::Invalid)?;
                ((a << 4) | b) as u8
            }
            _ => b,
        });
    }
    let v = String::from_utf8(out).map_err(|_| InputError::Invalid)?;
    if v.chars().any(char::is_control) {
        return Err(InputError::Invalid);
    }
    Ok(v)
}
pub(crate) fn validate_query(
    op: GiftOperation,
    q: &BTreeMap<String, String>,
) -> Result<(), InputError> {
    let allowed: &[&str] = match op {
        GiftOperation::List => &[
            "limit",
            "cursor",
            "state",
            "in_stock",
            "pickup_after",
            "pickup_before",
            "public_place_id",
        ],
        GiftOperation::Management => &["limit", "cursor", "state"],
        GiftOperation::Activity => &["limit", "giving_cursor", "receiving_cursor"],
        _ => &[],
    };
    for (k, v) in q {
        if !allowed.contains(&k.as_str()) {
            return Err(InputError::Invalid);
        }
        match k.as_str() {
            "limit" => {
                let n = v.parse::<u16>().map_err(|_| InputError::Invalid)?;
                if !(1..=100).contains(&n) || n.to_string() != *v {
                    return Err(InputError::Invalid);
                }
            }
            "cursor" | "giving_cursor" | "receiving_cursor" => {
                if v.len() > 1024
                    || !v
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                {
                    return Err(InputError::Invalid);
                }
            }
            "state" => {
                let states: &[&str] = if op == GiftOperation::Management {
                    &["HELD", "READY", "CLAIMED", "CANCELLED", "EXPIRED"]
                } else {
                    &["OPEN", "PAUSED", "CLOSED", "CANCELLED"]
                };
                if !states.contains(&v.as_str()) {
                    return Err(InputError::Invalid);
                }
            }
            "in_stock" => {
                if v != "true" && v != "false" {
                    return Err(InputError::Invalid);
                }
            }
            "public_place_id" => {
                canonical_uuid(v)?;
            }
            "pickup_after" | "pickup_before" => {
                if !v.ends_with('Z') {
                    return Err(InputError::Invalid);
                }
                DateTime::parse_from_rfc3339(v).map_err(|_| InputError::Invalid)?;
            }
            _ => return Err(InputError::Invalid),
        }
    }
    if let (Some(a), Some(b)) = (q.get("pickup_after"), q.get("pickup_before")) {
        if DateTime::parse_from_rfc3339(a).map_err(|_| InputError::Invalid)?
            >= DateTime::parse_from_rfc3339(b).map_err(|_| InputError::Invalid)?
        {
            return Err(InputError::Invalid);
        }
    }
    Ok(())
}
