//! Closed HTTP boundary. Gateway authentication/origin/CSRF and route mounting
//! remain integrator-owned; Actor comes only from a verified canonical context.
use super::{
    archive::{PgPublicArchive, PublicArchiveOwner},
    FencedLifecycleAuthority, LifecycleOwners, PgEventLifecycle,
};
use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{SecondsFormat, Utc};
use concert_application::{
    commands::*,
    event_lifecycle::{Actor, CloseCommand},
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};
use uuid::Uuid;
#[derive(Deserialize)]
#[serde(tag = "confirmation", deny_unknown_fields)]
enum Input {
    #[serde(rename = "CLOSE_EVENT")]
    Start {
        expected_revision: i64,
        reason: String,
    },
    #[serde(rename = "RESUME_CLOSURE")]
    Resume {
        job_id: Uuid,
        expected_revision: i64,
        reason: String,
    },
}
pub fn validate(name: &str, value: &Value) -> Result<(), CommandError> {
    if !matches!(
        name,
        "Domain_admin_CloseEvent"
            | "Domain_admin_ClosureJob"
            | "Domain_admin_Archive"
            | "Domain_common_CommandReceipt"
    ) {
        return Err(CommandError::InvalidInput);
    }
    static CACHE: OnceLock<Mutex<HashMap<String, Arc<jsonschema::Validator>>>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| CommandError::Unavailable)?;
    let validator = if let Some(v) = cache.get(name) {
        v.clone()
    } else {
        let source: Value = serde_json::from_str(include_str!(
            "../../../../../contracts/generated/mcr-domain.openapi.json"
        ))
        .map_err(|_| CommandError::Unavailable)?;
        let wrapper = json!({"$schema":"https://json-schema.org/draft/2020-12/schema","$ref":format!("#/components/schemas/{name}"),"components":source["components"]});
        let v = Arc::new(
            jsonschema::options()
                .should_validate_formats(true)
                .build(&wrapper)
                .map_err(|_| CommandError::Unavailable)?,
        );
        cache.insert(name.into(), v.clone());
        v
    };
    drop(cache);
    if validator.is_valid(value) {
        Ok(())
    } else {
        Err(CommandError::InvalidInput)
    }
}
/// Until Delivery freezes/regenerates the explicit RESUME union, generated schema
/// validation deliberately rejects that new input at this HTTP boundary.
pub fn parse(bytes: &[u8]) -> Result<CloseCommand, CommandError> {
    if bytes.len() > 8192 {
        return Err(CommandError::InvalidInput);
    }
    let input: Input = serde_json::from_slice(bytes).map_err(|_| CommandError::InvalidInput)?;
    let value: Value = serde_json::from_slice(bytes).map_err(|_| CommandError::InvalidInput)?;
    validate("Domain_admin_CloseEvent", &value)?;
    Ok(match input {
        Input::Start {
            expected_revision,
            reason,
        } => CloseCommand::Start {
            expected_revision,
            reason,
        },
        Input::Resume {
            job_id,
            expected_revision,
            reason,
        } => CloseCommand::Resume {
            job_id,
            expected_revision,
            reason,
        },
    })
}
fn response(status: StatusCode, value: Value) -> Response {
    let mut response = (status, Json(value)).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}
fn error(reason: &str, status: StatusCode) -> Response {
    response(
        status,
        json!({"code":format!("GATEWAY_{}",status.as_u16()),"message":"The requested action is unavailable.","traceId":Uuid::new_v4(),"timestamp":Utc::now().to_rfc3339_opts(SecondsFormat::Millis,true),"details":{"reason":reason}}),
    )
}
fn command_error(e: CommandError) -> Response {
    let (status, reason) = match e {
        CommandError::InvalidInput => (StatusCode::BAD_REQUEST, "INVALID_INPUT"),
        CommandError::Denied => (StatusCode::NOT_FOUND, "RESOURCE_UNAVAILABLE"),
        CommandError::Conflict => (StatusCode::CONFLICT, "IDEMPOTENCY_CONFLICT"),
        CommandError::Expired => (StatusCode::CONFLICT, "IDEMPOTENCY_EXPIRED"),
        CommandError::Capacity => (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED"),
        CommandError::InvalidState => (StatusCode::CONFLICT, "INVALID_TRANSITION"),
        CommandError::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE"),
    };
    error(reason, status)
}
fn encode<T: serde::Serialize>(name: &str, value: T) -> Result<Value, CommandError> {
    let v = serde_json::to_value(value).map_err(|_| CommandError::InvalidState)?;
    validate(name, &v).map_err(|_| CommandError::InvalidState)?;
    Ok(v)
}
pub async fn close_response<A: FencedLifecycleAuthority, O: LifecycleOwners>(
    service: &PgEventLifecycle<A, O>,
    actor: Actor,
    key: Uuid,
    body: &[u8],
    keys: &FingerprintKeys,
) -> Response {
    let command = match parse(body) {
        Ok(v) => v,
        Err(e) => return command_error(e),
    };
    let receipt = match service.command(actor, key, command, keys).await {
        Ok(v) => v,
        Err(e) => return command_error(e),
    };
    match receipt.state {
        ReceiptState::Succeeded => {
            let Some(reference) = receipt.reference else {
                return command_error(CommandError::InvalidState);
            };
            let job = match service.job(actor, reference.resource).await {
                Ok(v) => v,
                Err(e) => return command_error(e),
            };
            if job.job_id != reference.resource || job.revision < reference.revision {
                return command_error(CommandError::InvalidState);
            }
            match encode("Domain_admin_ClosureJob", job) {
                Ok(v) => response(StatusCode::OK, v),
                Err(e) => command_error(e),
            }
        }
        ReceiptState::Pending => {
            if receipt.reference.is_some()
                || receipt.failure.is_some()
                || receipt.result_status.is_some()
            {
                return command_error(CommandError::InvalidState);
            }
            let dto = json!({"receipt_id":receipt.id,"event_id":receipt.scope.event,"actor_id":receipt.scope.actor,"operation_id":"closeConcertEvent","idempotency_key":receipt.scope.key,"state":"PENDING","resource_id":null,"resource_revision":null,"failure_reason":null,"result_status":null,"created_at":receipt.created_at,"expires_at":receipt.expires_at});
            match validate("Domain_common_CommandReceipt", &dto) {
                Ok(()) => response(StatusCode::ACCEPTED, dto),
                Err(e) => command_error(e),
            }
        }
        ReceiptState::Failed => error(
            receipt
                .failure
                .map_or("INTERNAL_ERROR", SafeFailure::as_str),
            StatusCode::CONFLICT,
        ),
        ReceiptState::Retired => command_error(CommandError::Expired),
    }
}
pub async fn job_response<A: FencedLifecycleAuthority, O: LifecycleOwners>(
    service: &PgEventLifecycle<A, O>,
    actor: Actor,
    job: Uuid,
) -> Response {
    match service
        .job(actor, job)
        .await
        .and_then(|v| encode("Domain_admin_ClosureJob", v))
    {
        Ok(v) => response(StatusCode::OK, v),
        Err(e) => command_error(e),
    }
}
pub async fn archive_response<P: PublicArchiveOwner>(
    service: &PgPublicArchive<P>,
    event: Uuid,
    limit: u16,
    cursor: Option<&str>,
) -> Response {
    match service
        .page(event, limit, cursor)
        .await
        .and_then(|v| encode("Domain_admin_Archive", v))
    {
        Ok(v) => response(StatusCode::OK, v),
        Err(CommandError::Conflict) => error("VERSION_CONFLICT", StatusCode::CONFLICT),
        Err(e) => command_error(e),
    }
}
