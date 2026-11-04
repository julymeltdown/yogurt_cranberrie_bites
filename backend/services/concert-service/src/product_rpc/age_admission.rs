//! Closed C04 Age admission. This grants neither owner authority nor proof trust.
use super::{canonical_uuid, proto::ProductRequest};
use serde::Deserialize;
use tonic::Status;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgeOperation {
    GetStatus,
    Start,
    Callback,
}
impl AgeOperation {
    pub fn id(self) -> &'static str {
        match self {
            Self::GetStatus => "getConcertAgeStatus",
            Self::Start => "startConcertAgeVerification",
            Self::Callback => "completeConcertAgeVerification",
        }
    }
    pub fn mutation(self) -> bool {
        !matches!(self, Self::GetStatus)
    }
}

pub(super) fn route(event: Uuid, method: &str, target: &str) -> Result<AgeOperation, Status> {
    if event.is_nil() {
        return Err(invalid());
    }
    let base = format!("/api/concert/events/{event}/me/");
    match (method, target.strip_prefix(&base)) {
        ("GET", Some("age-status")) => Ok(AgeOperation::GetStatus),
        ("POST", Some("age-verifications")) => Ok(AgeOperation::Start),
        ("POST", Some("age-verifications/callback")) => Ok(AgeOperation::Callback),
        _ => Err(invalid()),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {
    return_target: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Callback {
    verification_id: String,
    state_token: String,
    provider_response_token: String,
}

pub(super) fn request(input: &ProductRequest, expected: AgeOperation) -> Result<(), Status> {
    canonical_uuid(&input.user_id)?;
    let event = canonical_uuid(&input.event_id)?;
    if route(event, &input.http_method, &input.http_target)? != expected {
        return Err(invalid());
    }
    if !expected.mutation() {
        return if input.http_body.is_empty()
            && input.content_type.is_empty()
            && input.idempotency_key.is_empty()
        {
            Ok(())
        } else {
            Err(invalid())
        };
    }
    canonical_uuid(&input.idempotency_key)?;
    // Keep the existing Age producer's wire limit. Do not normalize signed bytes.
    if input.content_type != "application/json" || input.http_body.len() > 8192 {
        return Err(invalid());
    }
    let shape: serde_json::Value =
        serde_json::from_slice(&input.http_body).map_err(|_| invalid())?;
    if !shape.is_object() {
        return Err(invalid());
    }
    match expected {
        AgeOperation::Start => {
            // Parse the original bytes again: Value alone would lose duplicate keys.
            let body: Start = serde_json::from_slice(&input.http_body).map_err(|_| invalid())?;
            if body.return_target != concert_application::age_verification::RETURN_TARGET {
                return Err(invalid());
            }
        }
        AgeOperation::Callback => {
            let body: Callback = serde_json::from_slice(&input.http_body).map_err(|_| invalid())?;
            canonical_uuid(&body.verification_id)?;
            token(&body.state_token, 512)?;
            token(&body.provider_response_token, 4096)?;
        }
        AgeOperation::GetStatus => return Err(invalid()),
    }
    Ok(())
}
fn token(value: &str, maximum: usize) -> Result<(), Status> {
    if value.is_empty() || value.chars().count() > maximum || value.chars().any(char::is_control) {
        Err(invalid())
    } else {
        Ok(())
    }
}
fn invalid() -> Status {
    Status::invalid_argument("Invalid Concert age request")
}
