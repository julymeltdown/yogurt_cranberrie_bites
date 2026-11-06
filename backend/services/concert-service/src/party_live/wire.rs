//! Closed HTTP/C04 parsing and serialization for party list/detail/create.
use super::{PartyCursorBinding, PartyLiveBinding, PartyLiveError, PartyPosition};
use crate::adapters::out::relationship_authority::guarded::GuardBinding;
use crate::product_rpc::{proto::ProductRequest, MAX_BODY};
use chrono::{DateTime, SecondsFormat, Utc};
use concert_domain::community::party::{
    text, Application, ApplicationState, Member, MemberState, Party, PartyFields, PartyState,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, Row};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex, OnceLock},
};
use uuid::Uuid;

pub const PARTY_OPERATIONS: [&str; 8] = [
    "listConcertParties",
    "getConcertParty",
    "createConcertParty",
    "applyConcertParty",
    "decideConcertApplication",
    "cancelConcertApplication",
    "removeConcertMember",
    "cancelConcertParty",
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
    Apply {
        party: Uuid,
        party_revision: i64,
        note: String,
    },
    Decide {
        application: Uuid,
        expected_revision: i64,
        party_revision: i64,
        approve: bool,
    },
    CancelApplication {
        application: Uuid,
        expected_revision: i64,
    },
    Remove {
        party: Uuid,
        user: Uuid,
        expected_revision: i64,
        party_revision: i64,
    },
    CancelParty {
        party: Uuid,
        expected_revision: i64,
    },
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
    let operation = parse_operation(event, method, target)?;
    let root = format!("/api/concert/events/{event}/parties");
    let (path, query) = target
        .split_once('?')
        .map_or((target, None), |(path, query)| (path, Some(query)));
    let key = match operation {
        "listConcertParties" | "getConcertParty" => {
            if !content_type.is_empty() || !idempotency_key.is_empty() || !body.is_empty() {
                return Err(PartyLiveError::InvalidInput);
            }
            None
        }
        "cancelConcertApplication" | "removeConcertMember" => {
            if !content_type.is_empty() || canonical(idempotency_key).is_none() || !body.is_empty()
            {
                return Err(PartyLiveError::InvalidInput);
            }
            canonical(idempotency_key)
        }
        _ => {
            if body.is_empty()
                || content_type != "application/json"
                || canonical(idempotency_key).is_none()
            {
                return Err(PartyLiveError::InvalidInput);
            }
            canonical(idempotency_key)
        }
    };
    let action = match operation {
        "listConcertParties" => {
            let value = parse_query(query.unwrap_or_default())?;
            value.validate()?;
            PartyLiveAction::List(value)
        }
        "getConcertParty" => {
            if query.is_some_and(|value| !value.is_empty()) {
                return Err(PartyLiveError::InvalidInput);
            }
            PartyLiveAction::Detail(
                canonical(
                    path.strip_prefix(&(root + "/"))
                        .ok_or(PartyLiveError::InvalidInput)?,
                )
                .ok_or(PartyLiveError::InvalidInput)?,
            )
        }
        "createConcertParty" => PartyLiveAction::Create(create_body(body)?),
        "applyConcertParty" => {
            if query.is_some() {
                return Err(PartyLiveError::InvalidInput);
            }
            let party = path_id(path, &root, &["applications"])?;
            let (party_revision, note) = apply_body(body)?;
            PartyLiveAction::Apply {
                party,
                party_revision,
                note,
            }
        }
        "decideConcertApplication" => {
            if query.is_some() {
                return Err(PartyLiveError::InvalidInput);
            }
            let application = application_path(event, path, &["decision"])?;
            let (expected_revision, party_revision, approve) = decision_body(body)?;
            PartyLiveAction::Decide {
                application,
                expected_revision,
                party_revision,
                approve,
            }
        }
        "cancelConcertApplication" => {
            let application = application_path(event, path, &[])?;
            let values = revision_query(query, &["expected_revision"])?;
            PartyLiveAction::CancelApplication {
                application,
                expected_revision: values[0],
            }
        }
        "removeConcertMember" => {
            let (party, user) = member_path(event, path)?;
            let values = revision_query(query, &["expected_revision", "party_revision"])?;
            PartyLiveAction::Remove {
                party,
                user,
                expected_revision: values[0],
                party_revision: values[1],
            }
        }
        "cancelConcertParty" => {
            if query.is_some() {
                return Err(PartyLiveError::InvalidInput);
            }
            let party = path_id(path, &root, &["cancel"])?;
            PartyLiveAction::CancelParty {
                party,
                expected_revision: revision_body(body)?,
            }
        }
        _ => return Err(PartyLiveError::InvalidInput),
    };
    Ok((operation, action, key))
}

pub(super) fn parse_operation(
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
    if method == "GET" && path == root {
        parse_query(query.unwrap_or_default())?;
        return Ok("listConcertParties");
    }
    let party_suffix = path.strip_prefix(&(root + "/"));
    if let Some(suffix) = party_suffix {
        let pieces: Vec<_> = suffix.split('/').collect();
        if method == "GET" && pieces.len() == 1 {
            canonical(pieces[0]).ok_or(PartyLiveError::InvalidInput)?;
            if query.is_some_and(|value| !value.is_empty()) {
                return Err(PartyLiveError::InvalidInput);
            }
            return Ok("getConcertParty");
        }
        if method == "POST" && pieces.len() == 2 && pieces[1] == "applications" {
            canonical(pieces[0]).ok_or(PartyLiveError::InvalidInput)?;
            if query.is_some() {
                return Err(PartyLiveError::InvalidInput);
            }
            return Ok("applyConcertParty");
        }
        if method == "POST" && pieces.len() == 2 && pieces[1] == "cancel" {
            canonical(pieces[0]).ok_or(PartyLiveError::InvalidInput)?;
            if query.is_some() {
                return Err(PartyLiveError::InvalidInput);
            }
            return Ok("cancelConcertParty");
        }
        if method == "DELETE" && pieces.len() == 3 && pieces[1] == "members" {
            canonical(pieces[0]).ok_or(PartyLiveError::InvalidInput)?;
            canonical(pieces[2]).ok_or(PartyLiveError::InvalidInput)?;
            revision_query(query, &["expected_revision", "party_revision"])?;
            return Ok("removeConcertMember");
        }
    }
    let application_root = format!("/api/concert/events/{event}/party-applications/");
    let application_suffix = path
        .strip_prefix(&application_root)
        .ok_or(PartyLiveError::InvalidInput)?;
    let pieces: Vec<_> = application_suffix.split('/').collect();
    canonical(pieces[0]).ok_or(PartyLiveError::InvalidInput)?;
    if method == "DELETE" && pieces.len() == 1 {
        revision_query(query, &["expected_revision"])?;
        return Ok("cancelConcertApplication");
    }
    if method == "POST" && pieces.len() == 2 && pieces[1] == "decision" {
        if query.is_some() {
            return Err(PartyLiveError::InvalidInput);
        }
        return Ok("decideConcertApplication");
    }
    Err(PartyLiveError::InvalidInput)
}

fn path_id(path: &str, root: &str, tail: &[&str]) -> Result<Uuid, PartyLiveError> {
    let suffix = path
        .strip_prefix(&(root.to_owned() + "/"))
        .ok_or(PartyLiveError::InvalidInput)?;
    let pieces: Vec<_> = suffix.split('/').collect();
    if pieces.len() != tail.len() + 1 || pieces.get(1..).is_some_and(|values| values != tail) {
        return Err(PartyLiveError::InvalidInput);
    }
    canonical(pieces[0]).ok_or(PartyLiveError::InvalidInput)
}

fn application_path(event: Uuid, path: &str, tail: &[&str]) -> Result<Uuid, PartyLiveError> {
    path_id(
        path,
        &format!("/api/concert/events/{event}/party-applications"),
        tail,
    )
}

fn member_path(event: Uuid, path: &str) -> Result<(Uuid, Uuid), PartyLiveError> {
    let root = format!("/api/concert/events/{event}/parties/");
    let suffix = path
        .strip_prefix(&root)
        .ok_or(PartyLiveError::InvalidInput)?;
    let pieces: Vec<_> = suffix.split('/').collect();
    if pieces.len() != 3 || pieces[1] != "members" {
        return Err(PartyLiveError::InvalidInput);
    }
    Ok((
        canonical(pieces[0]).ok_or(PartyLiveError::InvalidInput)?,
        canonical(pieces[2]).ok_or(PartyLiveError::InvalidInput)?,
    ))
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

fn apply_body(bytes: &[u8]) -> Result<(i64, String), PartyLiveError> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Apply {
        party_revision: i64,
        note: String,
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| PartyLiveError::InvalidInput)?;
    validate_name("Domain_party_Apply", &value)?;
    let typed: Apply = serde_json::from_slice(bytes).map_err(|_| PartyLiveError::InvalidInput)?;
    if !text(&typed.note, 0, 500) {
        return Err(PartyLiveError::InvalidInput);
    }
    Ok((typed.party_revision, typed.note))
}

fn decision_body(bytes: &[u8]) -> Result<(i64, i64, bool), PartyLiveError> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Decision {
        expected_revision: i64,
        party_revision: i64,
        decision: String,
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| PartyLiveError::InvalidInput)?;
    validate_name("Domain_party_ApplicationDecision", &value)?;
    let typed: Decision =
        serde_json::from_slice(bytes).map_err(|_| PartyLiveError::InvalidInput)?;
    let approve = match typed.decision.as_str() {
        "APPROVED" => true,
        "REJECTED" => false,
        _ => return Err(PartyLiveError::InvalidInput),
    };
    Ok((typed.expected_revision, typed.party_revision, approve))
}

fn revision_body(bytes: &[u8]) -> Result<i64, PartyLiveError> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Revision {
        expected_revision: i64,
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| PartyLiveError::InvalidInput)?;
    validate_name("Domain_common_RevisionRequest", &value)?;
    let typed: Revision =
        serde_json::from_slice(bytes).map_err(|_| PartyLiveError::InvalidInput)?;
    Ok(typed.expected_revision)
}

fn revision_query(value: Option<&str>, required: &[&str]) -> Result<Vec<i64>, PartyLiveError> {
    let value = value.ok_or(PartyLiveError::InvalidInput)?;
    let mut values = BTreeMap::new();
    for component in value.split('&') {
        let (key, value) = component
            .split_once('=')
            .ok_or(PartyLiveError::InvalidInput)?;
        let key = percent_decode(key, false)?;
        let value = percent_decode(value, false)?;
        if !required.contains(&key.as_str()) || values.insert(key, value).is_some() {
            return Err(PartyLiveError::InvalidInput);
        }
    }
    if values.len() != required.len() {
        return Err(PartyLiveError::InvalidInput);
    }
    required
        .iter()
        .map(|name| {
            let value = values.get(*name).ok_or(PartyLiveError::InvalidInput)?;
            if value.is_empty()
                || (value.len() > 1 && value.starts_with('0'))
                || !value.bytes().all(|byte| byte.is_ascii_digit())
            {
                return Err(PartyLiveError::InvalidInput);
            }
            value
                .parse::<i64>()
                .ok()
                .filter(|revision| (1..=9_007_199_254_740_991).contains(revision))
                .ok_or(PartyLiveError::InvalidInput)
        })
        .collect()
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

pub async fn application_from_receipt(
    connection: &mut PgConnection,
    binding: &PartyLiveBinding,
    application: Uuid,
    guard: &Arc<dyn super::PartyLiveOwnerGuard>,
) -> Result<PartyLiveReply, PartyLiveError> {
    let party = load_application(connection, binding.event(), application).await?;
    let record = party
        .applications
        .iter()
        .find(|value| value.id == application)
        .ok_or(PartyLiveError::DependencyUnavailable)?;
    // Receipt authorization committed in a different transaction. Recheck the
    // current aggregate before projecting a private applicant note.
    if binding.actor() != party.owner && binding.actor() != record.user {
        return Err(PartyLiveError::Denied);
    }
    let applicant = super::profile_for_subject(connection, binding, record.user, guard).await?;
    let value = application_record(&party, record, applicant)?;
    Ok(PartyLiveReply {
        status: 200,
        body: value,
    })
}

pub fn member(party: &Party, user: Uuid) -> Result<PartyLiveReply, PartyLiveError> {
    let value = party
        .members
        .iter()
        .find(|value| value.user == user)
        .ok_or(PartyLiveError::ResourceUnavailable)?;
    Ok(PartyLiveReply {
        status: 200,
        body: member_value(party, value)?,
    })
}

pub fn application_record(
    party: &Party,
    value: &Application,
    applicant: Value,
) -> Result<Value, PartyLiveError> {
    if applicant["user_id"].as_str() != Some(&value.user.to_string()) {
        return Err(PartyLiveError::DependencyUnavailable);
    }
    validate_name("PublicProfile", &applicant)?;
    let result = json!({
        "application_id": value.id,
        "event_id": party.event,
        "party_id": party.id,
        "applicant": applicant,
        "note": value.note,
        "created_at": value.created_at.to_rfc3339_opts(SecondsFormat::Micros, true),
        "state": application_state(value.state),
        "revision": value.revision,
    });
    validate_name("Domain_party_Application", &result)?;
    Ok(result)
}

fn member_value(party: &Party, value: &Member) -> Result<Value, PartyLiveError> {
    let result = json!({
        "party_id": party.id,
        "user_id": value.user,
        "role": if value.owner { "OWNER" } else { "MEMBER" },
        "state": match value.state {
            MemberState::Active => "ACTIVE",
            MemberState::Removed => "REMOVED",
            MemberState::Left => "LEFT",
        },
        "revision": value.revision,
        "joined_at": value.joined_at.to_rfc3339_opts(SecondsFormat::Micros, true),
        "event_id": party.event,
    });
    validate_name("Domain_party_Member", &result)?;
    Ok(result)
}

fn application_state(value: ApplicationState) -> &'static str {
    match value {
        ApplicationState::Pending => "PENDING",
        ApplicationState::Approved => "APPROVED",
        ApplicationState::Rejected => "REJECTED",
        ApplicationState::Cancelled => "CANCELLED",
    }
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
    load_locked(connection, event, party, false).await
}

/// Mutations lock the same aggregate row used by community audience reads.
/// Keep this lock until BE15 commits both the aggregate and its receipt.
pub(crate) async fn load_for_update(
    connection: &mut PgConnection,
    event: Uuid,
    party: Uuid,
) -> Result<Party, PartyLiveError> {
    load_locked(connection, event, party, true)
        .await?
        .ok_or(PartyLiveError::ResourceUnavailable)
}

async fn load_locked(
    connection: &mut PgConnection,
    event: Uuid,
    party: Uuid,
    write: bool,
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
    let statement = if write {
        "select aggregate from public.concert_community_parties where event_id=$1 and party_id=$2 for update"
    } else {
        "select aggregate from public.concert_community_parties where event_id=$1 and party_id=$2 for share"
    };
    let row = sqlx::query(statement)
        .bind(event)
        .bind(party)
        .fetch_optional(&mut *connection)
        .await
        .map_err(super::db)?;
    let party_id = party;
    row.map(|row| {
        let value: Value = row.try_get("aggregate").map_err(super::db)?;
        let party: Party =
            serde_json::from_value(value).map_err(|_| PartyLiveError::DependencyUnavailable)?;
        if party.event != event || party.id != party_id {
            return Err(PartyLiveError::DependencyUnavailable);
        }
        party
            .validate()
            .map_err(|_| PartyLiveError::DependencyUnavailable)?;
        Ok(party)
    })
    .transpose()
}

pub async fn load_application(
    connection: &mut PgConnection,
    event: Uuid,
    application: Uuid,
) -> Result<Party, PartyLiveError> {
    let party: Option<Uuid> = sqlx::query_scalar(
        "select party_id from public.concert_community_applications where event_id=$1 and application_id=$2",
    )
    .bind(event)
    .bind(application)
    .fetch_optional(&mut *connection)
    .await
    .map_err(super::db)?;
    load(
        connection,
        event,
        party.ok_or(PartyLiveError::ResourceUnavailable)?,
    )
    .await?
    .ok_or(PartyLiveError::ResourceUnavailable)
}

pub async fn load_member_party(
    connection: &mut PgConnection,
    event: Uuid,
    party: Uuid,
) -> Result<Party, PartyLiveError> {
    load(connection, event, party)
        .await?
        .ok_or(PartyLiveError::ResourceUnavailable)
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
            | "Domain_party_Apply"
            | "Domain_party_ApplicationDecision"
            | "Domain_party_Application"
            | "Domain_party_Member"
            | "Domain_common_RevisionRequest"
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
