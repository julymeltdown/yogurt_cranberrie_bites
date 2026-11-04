//! Exact C04 report admission. Parsing grants no canonical owner authority.
use super::{canonical_uuid, proto::ProductRequest};
use concert_domain::report::{CreateReport, Reason, TargetType};
use serde::Deserialize;
use tonic::Status;
use uuid::Uuid;

pub const MAX_REPORT_BODY: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportOperation {
    Create,
}
impl ReportOperation {
    pub fn id(self) -> &'static str {
        "createConcertReport"
    }
    pub fn mutation(self) -> bool {
        true
    }
    pub fn route(event: Uuid, method: &str, target: &str) -> Result<Self, Status> {
        if !event.is_nil()
            && method == "POST"
            && target == format!("/api/concert/events/{event}/reports")
        {
            Ok(Self::Create)
        } else {
            Err(invalid())
        }
    }
    pub fn validate(self, input: &ProductRequest) -> Result<(), Status> {
        canonical_uuid(&input.user_id)?;
        let event = canonical_uuid(&input.event_id)?;
        if Self::route(event, &input.http_method, &input.http_target)? != self
            || input.content_type != "application/json"
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
