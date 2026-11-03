use super::*;
use crate::adapters::out::media::Metadata;
use chrono::{DateTime, SecondsFormat, Utc};
use concert_application::commands::{ReceiptState, SafeReceipt, SafeReference};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AccessKind {
    Initialize,
    Write,
    Read,
}
pub(super) enum Action {
    Initialize(InitUpload),
    Put {
        id: Uuid,
        mime: Mime,
        bytes: Vec<u8>,
    },
    Finalize {
        id: Uuid,
        expected: i64,
    },
    Metadata(Uuid),
    Content(Uuid),
    Recover {
        operation: MediaOperation,
        key: Uuid,
    },
}
impl Action {
    pub(super) fn purpose(&self) -> Option<Purpose> {
        if let Self::Initialize(v) = self {
            Some(v.purpose)
        } else {
            None
        }
    }
    pub(super) fn access(&self) -> AccessKind {
        match self {
            Self::Initialize(_)
            | Self::Recover {
                operation: MediaOperation::Initialize,
                ..
            } => AccessKind::Initialize,
            Self::Put { .. } | Self::Finalize { .. } | Self::Recover { .. } => AccessKind::Write,
            Self::Metadata(_) | Self::Content(_) => AccessKind::Read,
        }
    }
}
/// Construct after transport verifies caller, actor delegation, exact HTTP body/
/// target binding and original authority expiry. This constructor is not authentication.
/// No Debug/Serialize: request bytes and authority stay private.
pub struct OwnerMediaRequest {
    pub(super) binding: OwnerMediaBinding,
    pub(super) action: Action,
}
impl OwnerMediaRequest {
    pub fn from_verified(
        identity: OwnerMediaIdentity,
        operation: &str,
        method: &str,
        target: &str,
        content_type: &str,
        idempotency_key: &str,
        body: Vec<u8>,
    ) -> Result<Self, CommandError> {
        if body.len() > MAX_BYTES || target.len() > 512 {
            return Err(CommandError::InvalidInput);
        }
        let event = identity.event;
        let root = format!("/api/concert/events/{event}");
        let (operation, expected_method, action) = match operation {
            "initConcertUpload" => {
                if target != format!("{root}/media/uploads") {
                    return Err(CommandError::InvalidInput);
                }
                json_type(content_type, &body)?;
                let input: InitBody =
                    serde_json::from_slice(&body).map_err(|_| CommandError::InvalidInput)?;
                let input = InitUpload {
                    purpose: Purpose::parse(&input.purpose)?,
                    mime: Mime::parse(&input.content_type)?,
                    byte_length: input.byte_length,
                    sha256: decode_sha(&input.sha256)?,
                };
                input.validate()?;
                ("initConcertUpload", "POST", Action::Initialize(input))
            }
            "putConcertUploadBytes" => {
                let id = media_id(target, &root, "/bytes")?;
                if body.is_empty() {
                    return Err(CommandError::InvalidInput);
                }
                (
                    "putConcertUploadBytes",
                    "PUT",
                    Action::Put {
                        id,
                        mime: Mime::parse(content_type)?,
                        bytes: body.clone(),
                    },
                )
            }
            "finalizeConcertUpload" => {
                let id = media_id(target, &root, "/finalize")?;
                json_type(content_type, &body)?;
                let input: FinalizeBody =
                    serde_json::from_slice(&body).map_err(|_| CommandError::InvalidInput)?;
                if !(1..=9007199254740991).contains(&input.expected_revision) {
                    return Err(CommandError::InvalidInput);
                }
                (
                    "finalizeConcertUpload",
                    "POST",
                    Action::Finalize {
                        id,
                        expected: input.expected_revision,
                    },
                )
            }
            "getConcertMedia" => (
                "getConcertMedia",
                "GET",
                Action::Metadata(media_id(target, &root, "")?),
            ),
            "readConcertMedia" => (
                "readConcertMedia",
                "GET",
                Action::Content(media_id(target, &root, "/content")?),
            ),
            "getConcertCommandStatus" => {
                let query = target
                    .strip_prefix(&format!("{root}/commands?"))
                    .ok_or(CommandError::InvalidInput)?;
                let mut op = None;
                let mut key = None;
                for part in query.split('&') {
                    let (k, v) = part.split_once('=').ok_or(CommandError::InvalidInput)?;
                    match k {
                        "operation_id" if op.is_none() => {
                            op = Some(match v {
                                "initConcertUpload" => MediaOperation::Initialize,
                                "putConcertUploadBytes" => MediaOperation::Put,
                                "finalizeConcertUpload" => MediaOperation::Finalize,
                                _ => return Err(CommandError::InvalidInput),
                            })
                        }
                        "idempotency_key" if key.is_none() => key = Some(canonical_uuid(v)?),
                        _ => return Err(CommandError::InvalidInput),
                    }
                }
                (
                    "getConcertCommandStatus",
                    "GET",
                    Action::Recover {
                        operation: op.ok_or(CommandError::InvalidInput)?,
                        key: key.ok_or(CommandError::InvalidInput)?,
                    },
                )
            }
            _ => return Err(CommandError::Unavailable),
        };
        if method != expected_method {
            return Err(CommandError::InvalidInput);
        }
        let key = if method == "GET" {
            if !body.is_empty() || !content_type.is_empty() || !idempotency_key.is_empty() {
                return Err(CommandError::InvalidInput);
            }
            None
        } else {
            Some(canonical_uuid(idempotency_key)?)
        };
        let binding = OwnerMediaBinding {
            identity,
            operation,
            method: expected_method,
            target: target.into(),
            content_type: content_type.into(),
            key,
            body_sha256: Sha256::digest(&body).into(),
        };
        binding.check_current()?;
        Ok(Self { binding, action })
    }
    pub fn binding(&self) -> &OwnerMediaBinding {
        &self.binding
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InitBody {
    purpose: String,
    content_type: String,
    byte_length: usize,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FinalizeBody {
    expected_revision: i64,
}
fn json_type(mime: &str, body: &[u8]) -> Result<(), CommandError> {
    if mime != "application/json" || body.len() > 65536 {
        Err(CommandError::InvalidInput)
    } else {
        Ok(())
    }
}
fn canonical_uuid(s: &str) -> Result<Uuid, CommandError> {
    let id = Uuid::parse_str(s).map_err(|_| CommandError::InvalidInput)?;
    if id.is_nil() || id.to_string() != s {
        Err(CommandError::InvalidInput)
    } else {
        Ok(id)
    }
}
fn media_id(target: &str, root: &str, suffix: &str) -> Result<Uuid, CommandError> {
    let id = target
        .strip_prefix(&format!("{root}/media/"))
        .and_then(|v| v.strip_suffix(suffix))
        .ok_or(CommandError::InvalidInput)?;
    canonical_uuid(id)
}
fn decode_sha(s: &str) -> Result<[u8; 32], CommandError> {
    if s.len() != 64
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(CommandError::InvalidInput);
    }
    let mut out = [0; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b =
            u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(|_| CommandError::InvalidInput)?;
    }
    Ok(out)
}
/// JSON is already a closed C04 product shape. Transport must preserve binary
/// headers and apply no-store to JSON/errors. Never serialize image bytes as JSON.
pub enum OwnerMediaReply {
    Json { status: u16, body: Value },
    Image(PrivateImage),
}
impl OwnerMediaReply {
    pub fn status(&self) -> u16 {
        match self {
            Self::Json { status, .. } => *status,
            Self::Image(_) => 200,
        }
    }
}
pub(super) fn successful(receipt: &SafeReceipt) -> Result<SafeReference, CommandError> {
    if receipt.state != ReceiptState::Succeeded {
        return Err(CommandError::InvalidState);
    }
    receipt.reference.ok_or(CommandError::InvalidState)
}
pub(super) fn intent(
    event: Uuid,
    id: Uuid,
    revision: i64,
    expires: DateTime<Utc>,
) -> OwnerMediaReply {
    OwnerMediaReply::Json {
        status: 200,
        body: json!({"media_id":id,"state":"PENDING","upload_path":format!("/api/concert/events/{event}/media/{id}/bytes"),"max_bytes":MAX_BYTES,"expires_at":timestamp(expires),"upload_method":"PUT","revision":revision}),
    }
}
pub(super) fn uploaded(id: Uuid, revision: i64) -> OwnerMediaReply {
    OwnerMediaReply::Json {
        status: 200,
        body: json!({"media_id":id,"state":"UPLOADED","revision":revision}),
    }
}
pub(super) fn metadata(m: Metadata) -> OwnerMediaReply {
    OwnerMediaReply::Json {
        status: 200,
        body: json!({"media_id":m.media_id,"state":"READY","revision":m.revision,"width":m.width,"height":m.height,"content_type":m.mime.as_str(),"purpose":m.purpose.as_str(),"read_path":m.read_path()}),
    }
}
pub(super) fn receipt(r: Option<SafeReceipt>) -> OwnerMediaReply {
    match r {
        Some(r) => OwnerMediaReply::Json {
            status: 200,
            body: json!({"receipt_id":r.id,"event_id":r.scope.event,"actor_id":r.scope.actor,"operation_id":r.scope.operation.id(),"idempotency_key":r.scope.key,"state":r.state.as_str(),"resource_id":r.reference.map(|v|v.resource),"resource_revision":r.reference.map(|v|v.revision),"failure_reason":r.failure.map(|v|v.as_str()),"created_at":timestamp(r.created_at),"expires_at":timestamp(r.expires_at),"result_status":r.result_status}),
        },
        None => OwnerMediaReply::Json {
            status: 404,
            body: json!({"code":"RESOURCE_UNAVAILABLE","message":"Resource unavailable","details":{"reason":"RESOURCE_UNAVAILABLE"}}),
        },
    }
}
fn timestamp(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Micros, true)
}
