//! Closed C04 projections of already-authorized store results; never serialize aggregates.
//! These functions confer no permission. Caller must retain its actual owner scope.
use super::protocol::{GiftIdentity, GiftReply, InputError};
use crate::community::giveaway::{GiftServiceError, GiftSummary, IssuedCode};
use chrono::{SecondsFormat, Utc};
use concert_application::commands::{CommandError, SafeReceipt};
use concert_application::feed::PublicAuthor;
use concert_domain::community::giveaway::{
    GiftError, PickupMeeting, Reservation, ReservationState,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectionError;

/// Reuses the retained C03 allowlist boundary, not a manufactured owner profile.
pub fn giveaway(g: GiftSummary, author: PublicAuthor) -> Result<Value, ProjectionError> {
    ids(&[g.id, g.event, g.owner, g.fields.public_place_id])?;
    revision(g.revision)?;
    let owner = crate::feed::wire::author(author, g.owner).map_err(|_| ProjectionError)?;
    let value = json!({"giveaway_id":g.id,"event_id":g.event,"owner":owner,"title":g.fields.title,"description":g.fields.description,"image_media_id":g.fields.image_media_id,"quantity_total":g.fields.quantity_total,"available":g.available,"pickup_starts_at":g.fields.pickup_starts_at,"pickup_ends_at":g.fields.pickup_ends_at,"pickup_grace_seconds":g.pickup_grace_seconds,"public_place_id":g.fields.public_place_id,"created_at":g.created_at,"state":g.state,"revision":g.revision});
    validate("Domain_giveaway_GiveawayPublic", &value)?;
    Ok(value)
}

/// Final output validation catches omitted/extra fields before transport assembly.
/// Cache pattern adapted from retained feed/wire.rs at 1cbb1823.
pub fn checked_reply(
    operation: super::protocol::GiftOperation,
    status: u16,
    body: Value,
) -> Result<GiftReply, ProjectionError> {
    use super::protocol::GiftOperation::*;
    let schema = if status == 202 && operation.mutation() {
        "Domain_common_CommandReceipt"
    } else if status == 200 {
        match operation {
            List => "Domain_giveaway_GiveawayList",
            Detail | Create | Update => "Domain_giveaway_GiveawayPublic",
            Management => "Domain_giveaway_GiveawayManagement",
            SetMeeting => "Domain_giveaway_PickupMeetingState",
            Reserve | Reservation | Cancel => "Domain_giveaway_Reservation",
            Activity => "Domain_giveaway_GiveawayActivity",
            Claim => "Domain_giveaway_PickupReceipt",
            Pickup => "Domain_giveaway_PickupDetails",
            IssueCode => "Domain_giveaway_PickupCode",
        }
    } else {
        return Err(ProjectionError);
    };
    validate(schema, &body)?;
    Ok(GiftReply::Json { status, body })
}
fn validate(name: &'static str, value: &Value) -> Result<(), ProjectionError> {
    static CONTRACT: OnceLock<Value> = OnceLock::new();
    static VALIDATORS: OnceLock<Mutex<HashMap<&'static str, Arc<jsonschema::Validator>>>> =
        OnceLock::new();
    let source = CONTRACT.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../contracts/generated/mcr-domain.openapi.json"
        ))
        .expect("pinned C04 JSON")
    });
    let mut cache = VALIDATORS
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| ProjectionError)?;
    let validator = if let Some(v) = cache.get(name) {
        v.clone()
    } else {
        let wrapper = json!({"$schema":"https://json-schema.org/draft/2020-12/schema","$ref":format!("#/components/schemas/{name}"),"components":source["components"]});
        let v = Arc::new(
            jsonschema::options()
                .should_validate_formats(true)
                .build(&wrapper)
                .map_err(|_| ProjectionError)?,
        );
        cache.insert(name, v.clone());
        v
    };
    drop(cache);
    if validator.is_valid(value) {
        Ok(())
    } else {
        Err(ProjectionError)
    }
}
fn revision(v: i64) -> Result<(), ProjectionError> {
    if (1..=concert_domain::MAX_REVISION).contains(&v) {
        Ok(())
    } else {
        Err(ProjectionError)
    }
}
fn ids(v: &[Uuid]) -> Result<(), ProjectionError> {
    if v.iter().any(Uuid::is_nil) {
        Err(ProjectionError)
    } else {
        Ok(())
    }
}
pub fn reservation(event: Uuid, giveaway: Uuid, r: &Reservation) -> Result<Value, ProjectionError> {
    ids(&[event, giveaway, r.id, r.recipient])?;
    revision(r.revision)?;
    if r.claim_receipt_id.is_some_and(|v| v.is_nil()) {
        return Err(ProjectionError);
    }
    Ok(
        json!({"reservation_id":r.id,"event_id":event,"giveaway_id":giveaway,"recipient_id":r.recipient,"expires_at":r.expires_at,"created_at":r.created_at,"claim_receipt_id":r.claim_receipt_id,"state":r.state,"revision":r.revision}),
    )
}
pub fn pickup(
    event: Uuid,
    giveaway: Uuid,
    r: &Reservation,
    meeting: &PickupMeeting,
) -> Result<Value, ProjectionError> {
    ids(&[meeting.place_id])?;
    revision(meeting.map_revision)?;
    revision(meeting.place_revision)?;
    Ok(json!({"reservation":reservation(event,giveaway,r)?,"meeting":meeting}))
}
pub fn claimed(giveaway: Uuid, r: &Reservation) -> Result<Value, ProjectionError> {
    let receipt = r.claim_receipt_id.ok_or(ProjectionError)?;
    let at = r.claimed_at.ok_or(ProjectionError)?;
    ids(&[giveaway, r.id, receipt])?;
    revision(r.revision)?;
    if r.state != ReservationState::Claimed {
        return Err(ProjectionError);
    }
    Ok(
        json!({"receipt_id":receipt,"reservation_id":r.id,"giveaway_id":giveaway,"state":"CLAIMED","revision":r.revision,"claimed_at":at}),
    )
}
/// Only consume the actual one-shot result; no durable lookup can recover its code.
pub fn issued(code: IssuedCode) -> Result<Value, ProjectionError> {
    ids(&[code.reservation])?;
    revision(code.revision)?;
    Ok(
        json!({"reservation_id":code.reservation,"confirmation_code":code.code.expose(),"expires_at":code.expires_at,"reservation_revision":code.revision}),
    )
}
pub fn receipt(
    identity: GiftIdentity,
    operation: &str,
    key: Uuid,
    r: &SafeReceipt,
) -> Result<Value, ProjectionError> {
    ids(&[identity.actor, identity.event, key, r.id])?;
    if r.scope.actor != identity.actor
        || r.scope.event != identity.event
        || r.scope.key != key
        || r.scope.operation.id() != operation
    {
        return Err(ProjectionError);
    }
    if let Some(reference) = r.reference {
        ids(&[reference.resource])?;
        revision(reference.revision)?;
    }
    Ok(
        json!({"receipt_id":r.id,"event_id":r.scope.event,"actor_id":r.scope.actor,"operation_id":r.scope.operation.id(),"idempotency_key":r.scope.key,"state":r.state.as_str(),"resource_id":r.reference.map(|v|v.resource),"resource_revision":r.reference.map(|v|v.revision),"failure_reason":r.failure.map(|v|v.as_str()),"created_at":r.created_at,"expires_at":r.expires_at,"result_status":r.result_status}),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Failure {
    pub status: u16,
    pub reason: &'static str,
}
impl Failure {
    pub fn reply(self, request: Uuid) -> GiftReply {
        GiftReply::Json {
            status: self.status,
            body: json!({"code":format!("GATEWAY_{}",self.status),"message":"The requested action is unavailable.","traceId":request,"timestamp":Utc::now().to_rfc3339_opts(SecondsFormat::Millis,true),"details":{"reason":self.reason}}),
        }
    }
}
impl From<InputError> for Failure {
    fn from(e: InputError) -> Self {
        match e {
            InputError::Invalid => Self {
                status: 400,
                reason: "INVALID_INPUT",
            },
            InputError::Expired => Self {
                status: 504,
                reason: "DEADLINE_EXCEEDED",
            },
            InputError::Unavailable => Self {
                status: 503,
                reason: "DEPENDENCY_UNAVAILABLE",
            },
        }
    }
}
impl From<CommandError> for Failure {
    fn from(e: CommandError) -> Self {
        let (status, reason) = match e {
            CommandError::InvalidInput => (400, "INVALID_INPUT"),
            CommandError::Denied => (404, "RESOURCE_UNAVAILABLE"),
            CommandError::Conflict => (409, "IDEMPOTENCY_CONFLICT"),
            CommandError::Expired => (409, "IDEMPOTENCY_EXPIRED"),
            CommandError::Capacity => (429, "RATE_LIMITED"),
            CommandError::InvalidState => (409, "INVALID_TRANSITION"),
            CommandError::Unavailable => (503, "DEPENDENCY_UNAVAILABLE"),
        };
        Self { status, reason }
    }
}
impl From<GiftServiceError> for Failure {
    fn from(e: GiftServiceError) -> Self {
        let (status, reason) = match e {
            GiftServiceError::Command(e) => return e.into(),
            GiftServiceError::EventReadOnly => (409, "EVENT_READ_ONLY"),
            GiftServiceError::Gift(e) => match e {
                GiftError::InvalidInput => (400, "INVALID_INPUT"),
                GiftError::Unavailable | GiftError::Denied => (404, "RESOURCE_UNAVAILABLE"),
                GiftError::VersionConflict => (409, "VERSION_CONFLICT"),
                GiftError::InvalidTransition => (409, "INVALID_TRANSITION"),
                GiftError::SoldOut => (409, "GIVEAWAY_SOLD_OUT"),
                GiftError::DuplicateReservation => (409, "DUPLICATE_ACTIVE_RESERVATION"),
                GiftError::Expired => (410, "RESERVATION_EXPIRED"),
                GiftError::CodeInvalid => (403, "PICKUP_CODE_INVALID"),
            },
        };
        Self { status, reason }
    }
}
