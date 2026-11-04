//! Closed HTTP/C04 parsing and serialization for party list/detail/create.
use super::{PartyCursorBinding, PartyLiveBinding, PartyLiveError, PartyPosition};
use crate::adapters::out::relationship_authority::guarded::GuardBinding;
use crate::product_rpc::{proto::ProductRequest, MAX_BODY};
use chrono::{DateTime, SecondsFormat, Utc};
use concert_domain::community::party::{text, Party, PartyFields, PartyState};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, Row};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex, OnceLock},
};
use uuid::Uuid;

pub const PARTY_OPERATIONS: [&str; 3] = [
    "listConcertParties",
    "getConcertParty",
    "createConcertParty",
];

#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartyQuery {
    pub limit: Option<u16>,
    pub cursor: Option<String>,
    pub ticket_section: Option<String>,
    pub state: Option<PartyState>,
    pub has_space: Option<bool>,
    pub starts_after: Option<DateTime<Utc>>,
    pub starts_before: Option<DateTime<Utc>>,
}

impl Default for PartyQuery {
    fn default() -> Self {
        Self {
            limit: None,
            cursor: None,
            ticket_section: None,
            state: None,
            has_space: None,
            starts_after: None,
            starts_before: None,
        }
    }
}

impl PartyQuery {
    pub fn limit(&self) -> Result<u16, PartyLiveError> {
        let limit = self.limit.unwrap_or(20);
        if !(1..=100).contains(&limit) {
            return Err(PartyLiveError::InvalidInput);
        }
        Ok(limit)
    }

    pub fn validate(&self) -> Result<(), PartyLiveError> {
        self.limit()?;
        if self
            .ticket_section
            .as_ref()
            .is_some_and(|value| !text(value, 1, 120))
            || self
                .starts_after
                .zip(self.starts_before)
                .is_some_and(|(after, before)| after >= before)
        {
            return Err(PartyLiveError::InvalidInput);
        }
        Ok(())
    }

    pub fn cursor_binding(
        &self,
        binding: &PartyLiveBinding,
    ) -> Result<PartyCursorBinding, PartyLiveError> {
        self.validate()?;
        Ok(PartyCursorBinding {
            actor: binding.actor(),
            event: binding.event(),
            operation: "listConcertParties".into(),
            collection: "parties".into(),
            ticket_section: self.ticket_section.clone(),
            state: self.state,
            has_space: self.has_space,
            starts_after: self.starts_after,
            starts_before: self.starts_before,
            limit: self.limit()?,
        })
    }
}

pub enum PartyLiveAction {
    List(PartyQuery),
    Detail(Uuid),
    Create(PartyFields),
}

pub struct PartyLiveRequest {
    pub binding: PartyLiveBinding,
    pub action: PartyLiveAction,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PartyLiveReply {
    pub status: u16,
    pub body: Value,
}

impl PartyLiveRequest {
    /// Root closes the Product route and acquires the canonical owner scope before
    /// calling this parser. The signed parent and Party generation remain separate
    /// fingerprints: the root-held scope authenticates transport, while the Party
    /// binding additionally covers the parsed operation and idempotency key. Root
    /// must pass the exact Product protobuf acquired by the sealed parent; the
    /// parent-body digest prevents a substituted request even when its URL matches.
    pub fn from_owner_scope(
        scope: &GuardBinding,
        request: &ProductRequest,
    ) -> Result<Self, PartyLiveError> {
        scope
            .check()
            .map_err(|_| PartyLiveError::DependencyUnavailable)?;
        if canonical(&request.user_id) != Some(scope.actor())
            || canonical(&request.event_id) != Some(scope.event())
            || canonical(&request.session_family_id) != Some(scope.session_family())
            || canonical(&request.request_id) != Some(scope.request_id())
            || request.http_method != scope.http_method()
            || request.http_target != scope.http_target()
            || request.http_body.len() > MAX_BODY
            || Sha256::digest(&request.http_body).as_slice() != scope.body_digest()
            || (!request.idempotency_key.is_empty()
                && canonical(&request.idempotency_key).is_none())
        {
            return Err(PartyLiveError::InvalidInput);
        }
        let expires = request
            .authority_expires_unix_micros
            .min(request.access_expires_unix_micros);
        let now = Utc::now().timestamp_micros();
        if expires <= now
            || scope.deadline().expires_micros() > expires
            || request.auth_observed_at_unix_micros <= 0
            || request.auth_observed_at_unix_micros > now
            || request.profile_observed_at_unix_micros <= 0
            || request.profile_observed_at_unix_micros > now
            || expires.saturating_sub(request.auth_observed_at_unix_micros)
                > crate::product_rpc::BUDGET.as_micros() as i64
            || expires.saturating_sub(request.profile_observed_at_unix_micros)
                > crate::product_rpc::BUDGET.as_micros() as i64
        {
            return Err(PartyLiveError::Denied);
        }
        let (operation, action, idempotency_key) = parse(
            scope.event(),
            &request.http_method,
            &request.http_target,
            &request.http_body,
            &request.content_type,
            &request.idempotency_key,
        )?;
        let binding = PartyLiveBinding::from_owner_guard(
            scope,
            operation,
            &request.http_body,
            idempotency_key,
        )?;
        binding.check()?;
        Ok(Self { binding, action })
    }
}

pub fn parse_verified_body_for_fixture(
    event: Uuid,
    method: &str,
    target: &str,
    body: &[u8],
    content_type: &str,
    idempotency_key: &str,
) -> Result<(), PartyLiveError> {
    parse(event, method, target, body, content_type, idempotency_key).map(|_| ())
}

#[doc(hidden)]
pub fn operation_for_fixture(
    event: Uuid,
    method: &str,
    target: &str,
) -> Result<&'static str, PartyLiveError> {
    parse_operation(event, method, target)
}

fn parse(
    event: Uuid,
    method: &str,
    target: &str,
    body: &[u8],
    content_type: &str,
    idempotency_key: &str,
) -> Result<(&'static str, PartyLiveAction, Option<Uuid>), PartyLiveError> {
    if target.chars().any(char::is_control) {
        return Err(PartyLiveError::InvalidInput);
    }
    let root = format!("/api/concert/events/{event}/parties");
    let (path, query) = target
        .split_once('?')
        .map_or((target, None), |(path, query)| (path, Some(query)));
    if method == "POST" && path == root && query.is_none() {
        if body.is_empty()
            || content_type != "application/json"
            || canonical(idempotency_key).is_none()
        {
            return Err(PartyLiveError::InvalidInput);
        }
        let fields = create_body(body)?;
        return Ok((
            "createConcertParty",
            PartyLiveAction::Create(fields),
            canonical(idempotency_key),
        ));
    }
    if method != "GET"
        || !content_type.is_empty()
        || !idempotency_key.is_empty()
        || !body.is_empty()
    {
        return Err(PartyLiveError::InvalidInput);
    }
    if path == root {
        let query = parse_query(query.unwrap_or_default())?;
        query.validate()?;
        return Ok(("listConcertParties", PartyLiveAction::List(query), None));
    }
    let suffix = path
        .strip_prefix(&(root.clone() + "/"))
        .ok_or(PartyLiveError::InvalidInput)?;
    let party = canonical(suffix).ok_or(PartyLiveError::InvalidInput)?;
    if query.is_some_and(|value| !value.is_empty()) {
        return Err(PartyLiveError::InvalidInput);
    }
    Ok(("getConcertParty", PartyLiveAction::Detail(party), None))
}

fn parse_operation(
    event: Uuid,
    method: &str,
    target: &str,
) -> Result<&'static str, PartyLiveError> {
    if target.chars().any(char::is_control) {
        return Err(PartyLiveError::InvalidInput);
    }
    let root = format!("/api/concert/events/{event}/parties");
    let (path, query) = target
        .split_once('?')
        .map_or((target, None), |(path, query)| (path, Some(query)));
    if method == "POST" && path == root && query.is_none() {
        return Ok("createConcertParty");
    }
    if method != "GET" {
        return Err(PartyLiveError::InvalidInput);
    }
    if path == root {
        parse_query(query.unwrap_or_default())?;
        return Ok("listConcertParties");
    }
    let suffix = path
        .strip_prefix(&(root + "/"))
        .ok_or(PartyLiveError::InvalidInput)?;
    canonical(suffix).ok_or(PartyLiveError::InvalidInput)?;
    if query.is_some_and(|value| !value.is_empty()) {
        return Err(PartyLiveError::InvalidInput);
    }
    Ok("getConcertParty")
}

fn parse_query(value: &str) -> Result<PartyQuery, PartyLiveError> {
    if value.is_empty() {
        return Ok(PartyQuery::default());
    }
    let mut values = BTreeMap::new();
    for component in value.split('&') {
        let (key, value) = component
            .split_once('=')
            .ok_or(PartyLiveError::InvalidInput)?;
        let key = percent_decode(key, false)?;
        let value = percent_decode(value, true)?;
        if !matches!(
            key.as_str(),
            "limit"
                | "cursor"
                | "ticket_section"
                | "state"
                | "has_space"
                | "starts_after"
                | "starts_before"
        ) || values.insert(key, value).is_some()
        {
            return Err(PartyLiveError::InvalidInput);
        }
    }
    let timestamp = |key: &str| -> Result<Option<DateTime<Utc>>, PartyLiveError> {
        values
            .get(key)
            .map(|value| {
                DateTime::parse_from_rfc3339(value)
                    .map(|value| value.with_timezone(&Utc))
                    .map_err(|_| PartyLiveError::InvalidInput)
            })
            .transpose()
    };
    let state = values
        .get("state")
        .map(|value| match value.as_str() {
            "OPEN" => Ok(PartyState::Open),
            "CLOSED" => Ok(PartyState::Closed),
            "CANCELLED" => Ok(PartyState::Cancelled),
            "COMPLETED" => Ok(PartyState::Completed),
            _ => Err(PartyLiveError::InvalidInput),
        })
        .transpose()?;
    let has_space = values
        .get("has_space")
        .map(|value| match value.as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(PartyLiveError::InvalidInput),
        })
        .transpose()?;
    let limit = values
        .get("limit")
        .map(|value| {
            value
                .parse::<u16>()
                .map_err(|_| PartyLiveError::InvalidInput)
        })
        .transpose()?;
    let query = PartyQuery {
        limit,
        cursor: values.get("cursor").cloned(),
        ticket_section: values.get("ticket_section").cloned(),
        state,
        has_space,
        starts_after: timestamp("starts_after")?,
        starts_before: timestamp("starts_before")?,
    };
    query.validate()?;
    Ok(query)
}

fn percent_decode(value: &str, plus_space: bool) -> Result<String, PartyLiveError> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                if index + 2 >= bytes.len() {
                    return Err(PartyLiveError::InvalidInput);
                }
                let high = hex(bytes[index + 1])?;
                let low = hex(bytes[index + 2])?;
                output.push((high << 4) | low);
                index += 3;
            }
            b'+' if plus_space => {
                output.push(b' ');
                index += 1;
            }
            value => {
                output.push(value);
                index += 1;
            }
        }
    }
    String::from_utf8(output).map_err(|_| PartyLiveError::InvalidInput)
}

fn hex(value: u8) -> Result<u8, PartyLiveError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(PartyLiveError::InvalidInput),
    }
}

fn create_body(bytes: &[u8]) -> Result<PartyFields, PartyLiveError> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Create {
        title: String,
        description: String,
        ticket_section: Option<String>,
        capacity: i32,
        meeting_starts_at: DateTime<Utc>,
        meeting_ends_at: DateTime<Utc>,
        requirements: String,
        public_place_id: Uuid,
    }
    let typed: Create = serde_json::from_slice(bytes).map_err(|_| PartyLiveError::InvalidInput)?;
    let value: Value = serde_json::from_slice(bytes).map_err(|_| PartyLiveError::InvalidInput)?;
    validate_name("Domain_party_CreateParty", &value)?;
    let result = PartyFields {
        title: typed.title,
        description: typed.description,
        ticket_section: typed.ticket_section,
        capacity: typed.capacity,
        meeting_starts_at: typed.meeting_starts_at,
        meeting_ends_at: typed.meeting_ends_at,
        requirements: typed.requirements,
        public_place_id: typed.public_place_id,
    };
    result
        .validate(Utc::now())
        .map_err(|_| PartyLiveError::InvalidInput)?;
    Ok(result)
}

pub fn public_party(party: &Party, owner: Value) -> Result<Value, PartyLiveError> {
    party
        .validate()
        .map_err(|_| PartyLiveError::DependencyUnavailable)?;
    if owner["user_id"].as_str() != Some(&party.owner.to_string()) {
        return Err(PartyLiveError::DependencyUnavailable);
    }
    validate_name("PublicProfile", &owner)?;
    let value = json!({
        "party_id":party.id,
        "event_id":party.event,
        "owner":owner,
        "title":party.fields.title,
        "description":party.fields.description,
        "ticket_section":party.fields.ticket_section,
        "capacity":party.fields.capacity,
        "member_count":party.member_count(),
        "meeting_starts_at":party.fields.meeting_starts_at.to_rfc3339_opts(SecondsFormat::Micros, true),
        "meeting_ends_at":party.fields.meeting_ends_at.to_rfc3339_opts(SecondsFormat::Micros, true),
        "requirements":party.fields.requirements,
        "created_at":party.created_at.to_rfc3339_opts(SecondsFormat::Micros, true),
        "state":state_name(party.state),
        "revision":party.revision,
        "public_place_id":party.fields.public_place_id,
        "meeting_revision":party.meeting_revision,
    });
    validate_name("Domain_party_PartyPublic", &value)?;
    Ok(value)
}

pub fn party(value: Value) -> Result<PartyLiveReply, PartyLiveError> {
    validate_name("Domain_party_PartyPublic", &value)?;
    Ok(PartyLiveReply {
        status: 200,
        body: value,
    })
}

pub fn list(
    items: Vec<Value>,
    next_cursor: Option<String>,
) -> Result<PartyLiveReply, PartyLiveError> {
    let value = json!({"items":items,"next_cursor":next_cursor});
    validate_name("Domain_party_PartyList", &value)?;
    Ok(PartyLiveReply {
        status: 200,
        body: value,
    })
}

pub struct ScannedParty {
    pub party: Party,
    pub position: PartyPosition,
}

pub async fn scan(
    connection: &mut PgConnection,
    event: Uuid,
    query: &PartyCursorBinding,
    upper: DateTime<Utc>,
    after: Option<PartyPosition>,
    limit: i64,
) -> Result<Vec<ScannedParty>, PartyLiveError> {
    let rows = sqlx::query(
        "select aggregate from public.concert_community_parties where event_id=$1 and ($2::text is null or aggregate->'fields'->>'ticket_section'=$2) and ($3::text is null or state=$3) and ($4::boolean is null or (member_count<capacity)=$4) and ($5::timestamptz is null or (aggregate->'fields'->>'meeting_starts_at')::timestamptz >=$5) and ($6::timestamptz is null or (aggregate->'fields'->>'meeting_starts_at')::timestamptz<$6) and (aggregate->>'created_at')::timestamptz <=$7 and ($8::timestamptz is null or ((aggregate->>'created_at')::timestamptz,party_id)>($8,$9)) order by (aggregate->>'created_at')::timestamptz,party_id limit $10 for share",
    )
    .bind(event)
    .bind(&query.ticket_section)
    .bind(query.state.map(state_name))
    .bind(query.has_space)
    .bind(query.starts_after)
    .bind(query.starts_before)
    .bind(upper)
    .bind(after.map(|value| value.created_at))
    .bind(after.map(|value| value.party_id))
    .bind(limit)
    .fetch_all(&mut *connection)
    .await
    .map_err(super::db)?;
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        let value: Value = row.try_get("aggregate").map_err(super::db)?;
        let party: Party =
            serde_json::from_value(value).map_err(|_| PartyLiveError::DependencyUnavailable)?;
        if party.event != event {
            return Err(PartyLiveError::DependencyUnavailable);
        }
        party
            .validate()
            .map_err(|_| PartyLiveError::DependencyUnavailable)?;
        result.push(ScannedParty {
            position: PartyPosition {
                created_at: party.created_at,
                party_id: party.id,
            },
            party,
        });
    }
    Ok(result)
}

pub async fn load(
    connection: &mut PgConnection,
    event: Uuid,
    party: Uuid,
) -> Result<Option<Party>, PartyLiveError> {
    let event_exists: Option<Uuid> = sqlx::query_scalar(
        "select event_id from public.concert_events where event_id=$1 for share",
    )
    .bind(event)
    .fetch_optional(&mut *connection)
    .await
    .map_err(super::db)?;
    if event_exists.is_none() {
        return Ok(None);
    }
    let row = sqlx::query(
        "select aggregate from public.concert_community_parties where event_id=$1 and party_id=$2 for share",
    )
    .bind(event)
    .bind(party)
    .fetch_optional(&mut *connection)
    .await
    .map_err(super::db)?;
    row.map(|row| {
        let value: Value = row.try_get("aggregate").map_err(super::db)?;
        let party: Party =
            serde_json::from_value(value).map_err(|_| PartyLiveError::DependencyUnavailable)?;
        if party.event != event {
            return Err(PartyLiveError::DependencyUnavailable);
        }
        party
            .validate()
            .map_err(|_| PartyLiveError::DependencyUnavailable)?;
        Ok(party)
    })
    .transpose()
}

fn canonical(value: &str) -> Option<Uuid> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
}

fn state_name(state: PartyState) -> &'static str {
    match state {
        PartyState::Open => "OPEN",
        PartyState::Closed => "CLOSED",
        PartyState::Cancelled => "CANCELLED",
        PartyState::Completed => "COMPLETED",
    }
}

fn validate_name(name: &str, value: &Value) -> Result<(), PartyLiveError> {
    if !matches!(
        name,
        "PublicProfile"
            | "Domain_party_PartyPublic"
            | "Domain_party_PartyList"
            | "Domain_party_CreateParty"
    ) {
        return Err(PartyLiveError::InvalidInput);
    }
    static SOURCE: OnceLock<Result<Value, ()>> = OnceLock::new();
    let source = SOURCE
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../../../../../contracts/generated/mcr-domain.openapi.json"
            ))
            .map_err(|_| ())
        })
        .as_ref()
        .map_err(|_| PartyLiveError::DependencyUnavailable)?;
    static VALIDATORS: OnceLock<Mutex<HashMap<String, Arc<jsonschema::Validator>>>> =
        OnceLock::new();
    let mut validators = VALIDATORS
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| PartyLiveError::DependencyUnavailable)?;
    let validator = if let Some(validator) = validators.get(name) {
        validator.clone()
    } else {
        let wrapper = json!({
            "$schema":"https://json-schema.org/draft/2020-12/schema",
            "$ref":format!("#/components/schemas/{name}"),
            "components":source["components"]
        });
        let validator = Arc::new(
            jsonschema::options()
                .should_validate_formats(true)
                .build(&wrapper)
                .map_err(|_| PartyLiveError::DependencyUnavailable)?,
        );
        validators.insert(name.into(), validator.clone());
        validator
    };
    drop(validators);
    if validator.is_valid(value) {
        Ok(())
    } else {
        Err(PartyLiveError::InvalidInput)
    }
}

pub fn error(error: PartyLiveError, trace: Uuid) -> PartyLiveReply {
    PartyLiveReply {
        status: error.status(),
        body: json!({
            "code":format!("CONCERT_{}",error.status()),
            "message":"Party request unavailable",
            "traceId":trace.to_string(),
            "timestamp":Utc::now().to_rfc3339_opts(SecondsFormat::Millis,true),
            "details":{"reason":error.reason()}
        }),
    }
}
