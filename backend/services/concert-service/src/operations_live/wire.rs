//! Closed C04 transport values. Private request bodies have no Debug output.
use super::cursor::CursorBinding;
use crate::{
    adapters::out::{
        relationship_authority::guarded::GuardBinding, repository::event_operations::EventCommand,
    },
    product_rpc::proto::ProductRequest,
};
use chrono::{DateTime, Utc};
use concert_application::{
    admin::{
        event::{Features, Settings},
        notices::{NoticeInput, NoticeState},
        places::PlaceChange,
    },
    commands::CommandError,
};
use concert_domain::{
    place::{Place, Provenance},
    report::CreateReport,
    Revision,
};
use prost::Message;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use uuid::Uuid;

pub struct OperationsRequest {
    pub(super) action: Operation,
    pub(super) digest: [u8; 32],
    pub(super) key: Option<Uuid>,
    pub(super) operation: &'static str,
    pub(super) query: Query,
}
pub(super) enum Operation {
    CreateReport(CreateReport),
    OwnReports,
    ReportQueue,
    ReportDetail(Uuid),
    Settings,
    Event(EventCommand, Option<Uuid>),
    Places,
    Notices,
    AdminNotices,
    Notice(Uuid),
}
#[derive(Clone, Default)]
pub(super) struct Query {
    pub limit: u16,
    pub cursor: Option<String>,
    pub filters: BTreeMap<String, String>,
}
impl Query {
    pub fn binding(&self, actor: Uuid, event: Uuid, operation: &str) -> CursorBinding {
        CursorBinding {
            actor,
            event,
            operation: operation.into(),
            filters: self.filters.clone(),
            limit: self.limit,
        }
    }
}
pub struct OperationsReply {
    pub status: u16,
    pub body: Value,
}
impl OperationsRequest {
    pub fn from_owner_scope(
        scope: &GuardBinding,
        request: &ProductRequest,
    ) -> Result<Self, CommandError> {
        scope.check().map_err(super::authority_error)?;
        // VerifiedParentAuthority fingerprints the complete admitted unary gRPC
        // frame, including its compression flag and four-byte message length.
        let encoded = request.encode_to_vec();
        let size = u32::try_from(encoded.len()).map_err(|_| CommandError::InvalidInput)?;
        let mut parent_frame = Sha256::new();
        parent_frame.update([0]);
        parent_frame.update(size.to_be_bytes());
        parent_frame.update(&encoded);
        let parent_frame_digest: [u8; 32] = parent_frame.finalize().into();
        if scope.parent_method.as_ref() != "/concert.product.v1.ProductService/Dispatch"
            || canonical(&request.user_id) != Some(scope.actor())
            || canonical(&request.event_id) != Some(scope.event())
            || canonical(&request.request_id) != Some(scope.request_id())
            || canonical(&request.session_family_id) != Some(scope.session_family())
            || request.http_method != scope.http_method()
            || request.http_target != scope.http_target()
            || request.http_body.len() > 64 * 1024
            || Sha256::digest(&request.http_body).as_slice() != scope.body_digest()
            || parent_frame_digest != scope.parent_body_digest
        {
            return Err(CommandError::Denied);
        }
        let now = Utc::now().timestamp_micros();
        let expires = request
            .authority_expires_unix_micros
            .min(request.access_expires_unix_micros);
        if expires <= now
            || scope.deadline().expires_micros() > expires
            || request.auth_observed_at_unix_micros <= 0
            || request.auth_observed_at_unix_micros > now
            || request.profile_observed_at_unix_micros <= 0
            || request.profile_observed_at_unix_micros > now
        {
            return Err(CommandError::Denied);
        }
        let mut parsed = parse(
            scope.event(),
            &request.http_method,
            &request.http_target,
            &request.http_body,
            &request.content_type,
            &request.idempotency_key,
        )?;
        if parsed.operation != scope.operation() {
            return Err(CommandError::Denied);
        }
        let mut hash = Sha256::new();
        hash.update(b"MCR-operations-request-v1\0");
        hash.update(scope.digest());
        hash.update(request.encode_to_vec());
        parsed.digest = hash.finalize().into();
        Ok(parsed)
    }
}
pub fn validate_body_for_fixture(
    event: Uuid,
    method: &str,
    target: &str,
    body: &[u8],
    content_type: &str,
    key: &str,
) -> Result<(), CommandError> {
    parse(event, method, target, body, content_type, key).map(|_| ())
}
fn parse(
    event: Uuid,
    method: &str,
    target: &str,
    body: &[u8],
    content_type: &str,
    key: &str,
) -> Result<OperationsRequest, CommandError> {
    if event.is_nil()
        || target.len() > 4096
        || !target.is_ascii()
        || target.chars().any(char::is_control)
        || body.len() > 64 * 1024
    {
        return Err(CommandError::InvalidInput);
    }
    let (path, raw_query) = target
        .split_once('?')
        .map_or((target, None), |(p, q)| (p, Some(q)));
    let root = format!("/api/concert/events/{event}/");
    let route = path.strip_prefix(&root).ok_or(CommandError::InvalidInput)?;
    let mut query = Query {
        limit: 20,
        ..Default::default()
    };
    let (operation, action, key) = if method == "GET" {
        if !body.is_empty() || !content_type.is_empty() || !key.is_empty() {
            return Err(CommandError::InvalidInput);
        }
        let (operation, action, allowed): (&str, Operation, &[&str]) = match route {
            "me/reports" => ("listOwnConcertReports", Operation::OwnReports, &["state"]),
            "admin/reports" => (
                "listConcertAdminReports",
                Operation::ReportQueue,
                &["state", "target_type"],
            ),
            "admin/settings" => ("getConcertSettings", Operation::Settings, &[]),
            "admin/places" => ("listConcertAdminPlaces", Operation::Places, &["closed"]),
            "admin/notices" => (
                "listConcertAdminNotices",
                Operation::AdminNotices,
                &["state"],
            ),
            "notices" => ("listConcertNotices", Operation::Notices, &[]),
            r if r.starts_with("admin/reports/") => (
                "getConcertAdminReport",
                Operation::ReportDetail(canonical(&r[14..]).ok_or(CommandError::InvalidInput)?),
                &[],
            ),
            r if r.starts_with("notices/") => (
                "getConcertNotice",
                Operation::Notice(canonical(&r[8..]).ok_or(CommandError::InvalidInput)?),
                &[],
            ),
            _ => return Err(CommandError::InvalidInput),
        };
        let detail = matches!(
            action,
            Operation::Settings | Operation::ReportDetail(_) | Operation::Notice(_)
        );
        if detail && raw_query.is_some() {
            return Err(CommandError::InvalidInput);
        }
        query = parse_query(raw_query.unwrap_or(""), allowed)?;
        (operation, action, None)
    } else {
        if raw_query.is_some() || content_type != "application/json" || body.is_empty() {
            return Err(CommandError::InvalidInput);
        }
        let key = canonical(key).ok_or(CommandError::InvalidInput)?;
        let (operation, action) = match (method, route) {
            ("POST", "reports") => {
                let input: CreateReport = decode(body)?;
                input.validate().map_err(|_| CommandError::InvalidInput)?;
                ("createConcertReport", Operation::CreateReport(input))
            }
            ("PUT", "admin/settings") => {
                let b: SettingsBody = decode(body)?;
                let command = EventCommand::Settings {
                    expected: revision(b.expected_revision)?,
                    settings: b.settings()?,
                    reason: b.reason,
                };
                ("setConcertSettings", Operation::Event(command, None))
            }
            ("POST", "admin/phase") => {
                let b: PhaseBody = decode(body)?;
                if !matches!(b.phase.as_str(), "OPEN" | "LIVE") {
                    return Err(CommandError::InvalidInput);
                }
                (
                    "setConcertEventPhase",
                    Operation::Event(
                        EventCommand::Phase {
                            expected: revision(b.expected_revision)?,
                            phase: b.phase.parse().map_err(|_| CommandError::InvalidInput)?,
                            reason: b.reason,
                        },
                        None,
                    ),
                )
            }
            ("POST", "admin/notices") => {
                let b: NoticeBody = decode(body)?;
                (
                    "createConcertNotice",
                    Operation::Event(EventCommand::NoticeCreate(b.input()?), None),
                )
            }
            ("PUT", r) if r.starts_with("admin/notices/") => {
                let id = canonical(&r[14..]).ok_or(CommandError::InvalidInput)?;
                let b: NoticeUpdateBody = decode(body)?;
                (
                    "updateConcertNotice",
                    Operation::Event(
                        EventCommand::NoticeUpdate {
                            expected: revision(b.expected_revision)?,
                            input: b.input()?,
                        },
                        Some(id),
                    ),
                )
            }
            ("PUT", r) if r.starts_with("admin/places/") => {
                let id = canonical(&r[13..]).ok_or(CommandError::InvalidInput)?;
                let b: PlaceBody = decode(body)?;
                let change = b.change()?;
                if change.place.id.uuid() != id {
                    return Err(CommandError::InvalidInput);
                }
                (
                    "setConcertPlace",
                    Operation::Event(EventCommand::Place(change), Some(id)),
                )
            }
            _ => return Err(CommandError::InvalidInput),
        };
        (operation, action, Some(key))
    };
    if let Some(schema) = match operation {
        "createConcertReport" => Some("Domain_admin_CreateReport"),
        "setConcertSettings" => Some("Domain_admin_SetEventSettings"),
        "setConcertEventPhase" => Some("Domain_admin_SetEventPhase"),
        "createConcertNotice" => Some("Domain_admin_CreateNotice"),
        "updateConcertNotice" => Some("Domain_admin_PublishNotice"),
        "setConcertPlace" => Some("Domain_admin_SetPlace"),
        _ => None,
    } {
        validate_value(schema, &decode::<Value>(body)?)?;
    }
    if let Some(state) = query.filters.get("state") {
        if matches!(
            operation,
            "listOwnConcertReports" | "listConcertAdminReports"
        ) {
            concert_domain::report::ReportState::parse(state)
                .map_err(|_| CommandError::InvalidInput)?;
        } else if !matches!(state.as_str(), "DRAFT" | "PUBLISHED" | "WITHDRAWN") {
            return Err(CommandError::InvalidInput);
        }
    }
    Ok(OperationsRequest {
        action,
        digest: [0; 32],
        key,
        operation,
        query,
    })
}
fn parse_query(raw: &str, allowed: &[&str]) -> Result<Query, CommandError> {
    let mut result = Query {
        limit: 20,
        ..Default::default()
    };
    let mut seen = std::collections::BTreeSet::new();
    if raw.is_empty() {
        return Ok(result);
    }
    for part in raw.split('&') {
        let (k, v) = part.split_once('=').ok_or(CommandError::InvalidInput)?;
        if v.is_empty() || !seen.insert(k) || v.contains(['%', '+', '#', '?']) {
            return Err(CommandError::InvalidInput);
        }
        match k {
            "limit" => {
                let n: u16 = v.parse().map_err(|_| CommandError::InvalidInput)?;
                if !(1..=100).contains(&n) || n.to_string() != v {
                    return Err(CommandError::InvalidInput);
                }
                result.limit = n;
            }
            "cursor" => {
                if v.len() > 1024 {
                    return Err(CommandError::InvalidInput);
                }
                result.cursor = Some(v.into());
            }
            k if allowed.contains(&k) => {
                match k {
                    "state"
                        if !matches!(
                            v,
                            "OPEN"
                                | "IN_REVIEW"
                                | "RESOLVED"
                                | "DISMISSED"
                                | "DRAFT"
                                | "PUBLISHED"
                                | "WITHDRAWN"
                        ) =>
                    {
                        return Err(CommandError::InvalidInput)
                    }
                    "target_type" if concert_domain::report::TargetType::parse(v).is_err() => {
                        return Err(CommandError::InvalidInput)
                    }
                    "closed" if !matches!(v, "true" | "false") => {
                        return Err(CommandError::InvalidInput)
                    }
                    _ => {}
                }
                result.filters.insert(k.into(), v.into());
            }
            _ => return Err(CommandError::InvalidInput),
        }
    }
    Ok(result)
}
fn decode<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, CommandError> {
    serde_json::from_slice(body).map_err(|_| CommandError::InvalidInput)
}
fn canonical(value: &str) -> Option<Uuid> {
    Uuid::parse_str(value)
        .ok()
        .filter(|v| !v.is_nil() && v.to_string() == value)
}
fn revision(value: i64) -> Result<Revision, CommandError> {
    Revision::new(value).map_err(|_| CommandError::InvalidInput)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FeaturesBody {
    parties: bool,
    giveaways: bool,
    intros: bool,
    social: bool,
    mural: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsBody {
    expected_revision: i64,
    features: FeaturesBody,
    beacon_ttl_seconds: i64,
    pickup_grace_seconds: i64,
    social_grant_ttl_seconds: i64,
    mural_cooldown_ms: i64,
    reason: String,
}
impl SettingsBody {
    fn settings(&self) -> Result<Settings, CommandError> {
        Settings {
            features: Features {
                parties: self.features.parties,
                giveaways: self.features.giveaways,
                intros: self.features.intros,
                social: self.features.social,
                mural: self.features.mural,
            },
            beacon_ttl_seconds: self.beacon_ttl_seconds,
            pickup_grace_seconds: self.pickup_grace_seconds,
            social_grant_ttl_seconds: self.social_grant_ttl_seconds,
            mural_cooldown_ms: self.mural_cooldown_ms,
        }
        .validate()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PhaseBody {
    expected_revision: i64,
    phase: String,
    reason: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoticeBody {
    title: String,
    text: String,
    source_uri: String,
    confirmed_at: DateTime<Utc>,
    state: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoticeUpdateBody {
    expected_revision: i64,
    title: String,
    text: String,
    source_uri: String,
    confirmed_at: DateTime<Utc>,
    state: String,
}
fn notice(
    title: String,
    text: String,
    source_uri: String,
    confirmed_at: DateTime<Utc>,
    state: String,
) -> Result<NoticeInput, CommandError> {
    let state = match state.as_str() {
        "DRAFT" => NoticeState::Draft,
        "PUBLISHED" => NoticeState::Published,
        "WITHDRAWN" => NoticeState::Withdrawn,
        _ => return Err(CommandError::InvalidInput),
    };
    let v = NoticeInput {
        title,
        text,
        source_uri,
        confirmed_at,
        state,
    };
    v.validate(Utc::now())?;
    Ok(v)
}
impl NoticeBody {
    fn input(self) -> Result<NoticeInput, CommandError> {
        notice(
            self.title,
            self.text,
            self.source_uri,
            self.confirmed_at,
            self.state,
        )
    }
}
impl NoticeUpdateBody {
    fn input(self) -> Result<NoticeInput, CommandError> {
        notice(
            self.title,
            self.text,
            self.source_uri,
            self.confirmed_at,
            self.state,
        )
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlaceBody {
    expected_revision: i64,
    place: PlaceWire,
    reason: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PlaceWire {
    place_id: String,
    label: String,
    place_kind: String,
    verification_state: String,
    closure_state: String,
    accessibility_notes: String,
    provenance: ProvenanceWire,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProvenanceWire {
    source_id: String,
    source_kind: String,
    source_revision: String,
    source_uri: Option<String>,
    author: String,
    license: String,
    review_state: String,
    reviewed_by: Option<String>,
    reviewed_at: Option<DateTime<Utc>>,
    photo_permission: String,
}
impl PlaceBody {
    fn change(self) -> Result<PlaceChange, CommandError> {
        let p = self.place;
        let v = p.provenance;
        let provenance = Provenance::new(
            v.source_id,
            v.source_kind
                .parse()
                .map_err(|_| CommandError::InvalidInput)?,
            v.source_revision,
            v.source_uri,
            v.author,
            v.license,
            v.review_state
                .parse()
                .map_err(|_| CommandError::InvalidInput)?,
            v.reviewed_by,
            v.reviewed_at,
            v.photo_permission
                .parse()
                .map_err(|_| CommandError::InvalidInput)?,
        )
        .map_err(|_| CommandError::InvalidInput)?;
        let place = Place::new(
            p.place_id.parse().map_err(|_| CommandError::InvalidInput)?,
            p.label,
            p.place_kind
                .parse()
                .map_err(|_| CommandError::InvalidInput)?,
            p.verification_state
                .parse()
                .map_err(|_| CommandError::InvalidInput)?,
            p.closure_state
                .parse()
                .map_err(|_| CommandError::InvalidInput)?,
            p.accessibility_notes,
            provenance,
        )
        .map_err(|_| CommandError::InvalidInput)?;
        let change = PlaceChange {
            place,
            expected: revision(self.expected_revision)?,
            reason: self.reason,
        };
        change.validate()?;
        Ok(change)
    }
}

/// Build from the committed generated contract only; remote schema loading is
/// unnecessary. Factory initialization warms this before a guarded request.
pub fn warm_contracts() -> Result<(), CommandError> {
    validators().map(|_| ())
}
fn validators() -> Result<&'static BTreeMap<&'static str, jsonschema::Validator>, CommandError> {
    static V: std::sync::OnceLock<Result<BTreeMap<&'static str, jsonschema::Validator>, ()>> =
        std::sync::OnceLock::new();
    V.get_or_init(||{
        let source:Value=serde_json::from_str(include_str!("../../../../../contracts/generated/mcr-domain.openapi.json")).map_err(|_|())?;
        ["Domain_admin_CreateReport","Domain_admin_SetEventSettings","Domain_admin_SetEventPhase","Domain_admin_CreateNotice","Domain_admin_PublishNotice","Domain_admin_SetPlace","Domain_admin_Report","Domain_admin_ReportList","Domain_admin_ReportDetail","Domain_admin_EventSettings","Domain_admin_PlaceAdminList","Domain_feed_Notice","Domain_feed_NoticeList","Domain_common_CommandReceipt"].into_iter().map(|name|{
            if source["components"]["schemas"][name].is_null(){return Err(());}
            let schema=serde_json::json!({"$schema":"https://json-schema.org/draft/2020-12/schema","$ref":format!("#/components/schemas/{name}"),"components":source["components"]});
            Ok((name,jsonschema::options().should_validate_formats(true).build(&schema).map_err(|_|())?))
        }).collect()
    }).as_ref().map_err(|_|CommandError::Unavailable)
}
pub fn validate_value(schema: &str, value: &Value) -> Result<(), CommandError> {
    if validators()?
        .get(schema)
        .ok_or(CommandError::Unavailable)?
        .is_valid(value)
    {
        Ok(())
    } else {
        Err(CommandError::InvalidInput)
    }
}
