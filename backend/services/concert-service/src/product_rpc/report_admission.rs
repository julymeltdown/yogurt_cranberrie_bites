//! Exact C04 report admission. Parsing grants no canonical owner authority.
use super::{canonical_uuid, proto::ProductRequest};
use concert_domain::report::{CreateReport, Reason, ReportState, TargetType};
use serde::Deserialize;
use tonic::Status;
use uuid::Uuid;

pub const MAX_REPORT_BODY: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportOperation {
    Create,
    Own,
}
impl ReportOperation {
    pub fn id(self) -> &'static str {
        match self {
            Self::Create => "createConcertReport",
            Self::Own => "listOwnConcertReports",
        }
    }
    pub fn mutation(self) -> bool {
        matches!(self, Self::Create)
    }
    pub fn route(event: Uuid, method: &str, target: &str) -> Result<Self, Status> {
        if event.is_nil() || target.contains('#') {
            return Err(invalid());
        }
        let path = target.split_once('?').map_or(target, |(path, _)| path);
        match method {
            "POST" if target == format!("/api/concert/events/{event}/reports") => Ok(Self::Create),
            "GET" if path == format!("/api/concert/events/{event}/me/reports") => Ok(Self::Own),
            _ => Err(invalid()),
        }
    }
    pub fn validate(self, input: &ProductRequest) -> Result<(), Status> {
        canonical_uuid(&input.user_id)?;
        let event = canonical_uuid(&input.event_id)?;
        if Self::route(event, &input.http_method, &input.http_target)? != self {
            return Err(invalid());
        }
        if self == Self::Own {
            if !input.content_type.is_empty()
                || !input.http_body.is_empty()
                || !input.idempotency_key.is_empty()
            {
                return Err(invalid());
            }
            return own_query(&input.http_target);
        }
        if input.content_type != "application/json"
            || input.http_body.is_empty()
            || input.http_body.len() > MAX_REPORT_BODY
        {
            return Err(invalid());
        }
        canonical_uuid(&input.idempotency_key)?;
        // Decode the ORIGINAL bytes: a Value intermediate would discard duplicate
        // keys. Strings preserve canonical UUID spelling before domain conversion.
        let body: Body = serde_json::from_slice(&input.http_body).map_err(|_| invalid())?;
        let report = CreateReport {
            target_type: TargetType::parse(&body.target_type).map_err(|_| invalid())?,
            target_id: canonical_uuid(&body.target_id)?,
            reason: Reason::parse(&body.reason).map_err(|_| invalid())?,
            text: body.text,
            evidence_media_ids: body
                .evidence_media_ids
                .iter()
                .map(|value| canonical_uuid(value))
                .collect::<Result<_, _>>()?,
        };
        report.validate().map_err(|_| invalid())?;
        if report.target_type != TargetType::User || !report.evidence_media_ids.is_empty() {
            return Err(Status::unavailable(
                "Report target or media owner unavailable",
            ));
        }
        Ok(())
    }
}

// Match the retained Operations wire's canonical raw query. Keep the original
// target bytes for the signed parent, owner binding and authenticated cursor.
fn own_query(target: &str) -> Result<(), Status> {
    let raw = target.split_once('?').map_or("", |(_, query)| query);
    if raw.is_empty() {
        return Ok(());
    }
    let mut seen = std::collections::BTreeSet::new();
    for part in raw.split('&') {
        let (key, value) = part.split_once('=').ok_or_else(invalid)?;
        if value.is_empty() || !seen.insert(key) || value.contains(['%', '+', '#', '?']) {
            return Err(invalid());
        }
        match key {
            "limit" => {
                let limit: u16 = value.parse().map_err(|_| invalid())?;
                if !(1..=100).contains(&limit) || limit.to_string() != value {
                    return Err(invalid());
                }
            }
            "state" => {
                ReportState::parse(value).map_err(|_| invalid())?;
            }
            "cursor" if value.len() <= 1024 => {}
            _ => return Err(invalid()),
        }
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Body {
    target_type: String,
    target_id: String,
    reason: String,
    text: String,
    evidence_media_ids: Vec<String>,
}
fn invalid() -> Status {
    Status::invalid_argument("Invalid Concert report request")
}
