//! Internal BE15 seams. A scope is correlation, never authentication or authorization.
//! Producers must resolve canonical identity, validate closed input schemas and enforce
//! current target/policy guards inside the aggregate transaction. No public DTOs here.
mod registry;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
pub use registry::OPERATIONS;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegisteredOperation {
    pub(crate) id: &'static str,
    pub(crate) method: &'static str,
    pub(crate) path: &'static str,
    pub(crate) binary: bool,
}
impl RegisteredOperation {
    pub fn find(id: &str) -> Result<Self, CommandError> {
        OPERATIONS
            .iter()
            .find(|op| op.id == id)
            .copied()
            .ok_or(CommandError::InvalidInput)
    }
    pub fn id(self) -> &'static str {
        self.id
    }
    pub fn method(self) -> &'static str {
        self.method
    }
    pub fn template(self) -> &'static str {
        self.path
    }
    /// Canonical UUID parameters, or the closed social platform enum. No URL normalization.
    pub fn path(self, parameters: &[(&str, &str)]) -> Result<String, CommandError> {
        let mut path = self.path.to_owned();
        let mut seen = std::collections::BTreeSet::new();
        for (key, value) in parameters {
            let slot = format!("{{{key}}}");
            if !seen.insert(*key) || !path.contains(&slot) {
                return Err(CommandError::InvalidInput);
            }
            if *key == "platform" {
                if !matches!(*value, "X" | "INSTAGRAM") {
                    return Err(CommandError::InvalidInput);
                }
            } else if *key == "landmarkId" {
                if !concert_domain::venue_notes::valid_landmark_id(value) {
                    return Err(CommandError::InvalidInput);
                }
            } else if Uuid::parse_str(value)
                .map(|id| id.to_string() != *value)
                .unwrap_or(true)
            {
                return Err(CommandError::InvalidInput);
            }
            path = path.replace(&slot, value);
        }
        if path.contains(['{', '}']) {
            return Err(CommandError::InvalidInput);
        }
        Ok(path)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scope {
    pub actor: Uuid,
    pub event: Uuid,
    pub operation: RegisteredOperation,
    pub key: Uuid,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CommandError {
    #[error("Invalid command input")]
    InvalidInput,
    #[error("Current authorization denied")]
    Denied,
    #[error("Required dependency unavailable")]
    Unavailable,
    #[error("Idempotency key has different input")]
    Conflict,
    #[error("Idempotency key expired")]
    Expired,
    #[error("Command admission limit reached")]
    Capacity,
    #[error("Invalid command state")]
    InvalidState,
}
/// Validated, bounded canonical JSON algebra. Parsing/duplicate-key rejection belongs
/// to the producer's closed-schema adapter. No floats; all integers are JS-safe.
/// Intentionally no Debug/Serialize implementation: input may contain one-shot secrets.
pub enum CanonicalValue {
    Null,
    Bool(bool),
    Integer(i64),
    String(String),
    Array(Vec<Self>),
    Object(Vec<(String, Self)>),
}
impl CanonicalValue {
    fn encode(&self, output: &mut Vec<u8>, depth: usize) -> Result<(), CommandError> {
        if depth > 16 || output.len() > 65536 {
            return Err(CommandError::InvalidInput);
        }
        match self {
            Self::Null => output.extend(b"null"),
            Self::Bool(v) => output.extend(if *v {
                b"true".as_slice()
            } else {
                b"false".as_slice()
            }),
            Self::Integer(v) if (-9007199254740991..=9007199254740991).contains(v) => {
                output.extend(v.to_string().bytes())
            }
            Self::Integer(_) => return Err(CommandError::InvalidInput),
            Self::String(v) => encode_string(v, output)?,
            Self::Array(values) => {
                if values.len() > 1024 {
                    return Err(CommandError::InvalidInput);
                }
                output.push(b'[');
                for (i, v) in values.iter().enumerate() {
                    if i > 0 {
                        output.push(b',');
                    }
                    v.encode(output, depth + 1)?;
                }
                output.push(b']');
            }
            Self::Object(values) => {
                if values.len() > 1024 {
                    return Err(CommandError::InvalidInput);
                }
                let mut sorted: Vec<_> = values.iter().collect();
                sorted.sort_by(|a, b| a.0.cmp(&b.0));
                if sorted.windows(2).any(|v| v[0].0 == v[1].0) {
                    return Err(CommandError::InvalidInput);
                }
                output.push(b'{');
                for (i, (key, v)) in sorted.into_iter().enumerate() {
                    if i > 0 {
                        output.push(b',');
                    }
                    encode_string(key, output)?;
                    output.push(b':');
                    v.encode(output, depth + 1)?;
                }
                output.push(b'}');
            }
        }
        if output.len() > 65536 {
            return Err(CommandError::InvalidInput);
        }
        Ok(())
    }
}
fn encode_string(value: &str, out: &mut Vec<u8>) -> Result<(), CommandError> {
    if value.len() > 65536 {
        return Err(CommandError::InvalidInput);
    }
    out.push(b'"');
    for ch in value.chars() {
        match ch {
            '"' => out.extend(b"\\\""),
            '\\' => out.extend(b"\\\\"),
            c if c < '\u{20}' => out.extend(format!("\\u{:04x}", c as u32).bytes()),
            c => {
                let mut bytes = [0; 4];
                out.extend(c.encode_utf8(&mut bytes).as_bytes());
            }
        }
    }
    out.push(b'"');
    Ok(())
}
/// Transient validated canonical input, including scope. Never Debug/Serialize/log it.
/// Construct only after closed-schema transport parsing. No fingerprint is accepted
/// from a browser and no unkeyed fingerprint API exists.
pub struct CanonicalRequest {
    scope: Scope,
    framed: Vec<u8>,
}
impl CanonicalRequest {
    pub fn scope(&self) -> Scope {
        self.scope
    }
    pub fn json(
        scope: Scope,
        parameters: &[(&str, &str)],
        query: &[(&str, &str)],
        body: Option<&CanonicalValue>,
    ) -> Result<Self, CommandError> {
        if scope.operation.binary || (scope.operation.method == "DELETE" && body.is_some()) {
            return Err(CommandError::InvalidInput);
        }
        let mut canonical_body = Vec::new();
        match body {
            None => canonical_body.extend(b"ABSENT"),
            Some(value) => {
                canonical_body.extend(b"JSON:");
                value.encode(&mut canonical_body, 0)?;
            }
        }
        Self::encode(scope, parameters, query, &canonical_body)
    }
    /// Exact bytes, maximum 10 MiB before producer's stricter media schema validation.
    pub fn binary(
        scope: Scope,
        parameters: &[(&str, &str)],
        query: &[(&str, &str)],
        bytes: &[u8],
    ) -> Result<Self, CommandError> {
        if !scope.operation.binary || bytes.len() > 10 * 1024 * 1024 {
            return Err(CommandError::InvalidInput);
        }
        let mut body = b"BINARY-SHA256:".to_vec();
        body.extend((bytes.len() as u64).to_be_bytes());
        body.extend(Sha256::digest(bytes));
        Self::encode(scope, parameters, query, &body)
    }
    fn encode(
        scope: Scope,
        parameters: &[(&str, &str)],
        query: &[(&str, &str)],
        canonical_body: &[u8],
    ) -> Result<Self, CommandError> {
        let operation = scope.operation;
        if parameters
            .iter()
            .find(|(name, _)| *name == "eventId")
            .map(|(_, v)| *v)
            != Some(scope.event.to_string().as_str())
        {
            return Err(CommandError::InvalidInput);
        }
        let path = operation.path(parameters)?;
        let mut fields = query.to_vec();
        fields.sort();
        if fields.len() > 32
            || fields.windows(2).any(|v| v[0].0 == v[1].0)
            || fields
                .iter()
                .any(|(k, v)| k.is_empty() || k.chars().chain(v.chars()).any(char::is_control))
            || fields.iter().map(|(k, v)| k.len() + v.len()).sum::<usize>() > 8192
        {
            return Err(CommandError::InvalidInput);
        }
        let query = CanonicalValue::Object(
            fields
                .into_iter()
                .map(|(k, v)| (k.to_owned(), CanonicalValue::String(v.to_owned())))
                .collect(),
        );
        let mut canonical_query = Vec::new();
        query.encode(&mut canonical_query, 0)?;
        let mut framed = Vec::new();
        for part in [
            b"MCR-BE15-HMAC-v1".as_slice(),
            scope.actor.as_bytes(),
            scope.event.as_bytes(),
            operation.id.as_bytes(),
            scope.key.as_bytes(),
            operation.method.as_bytes(),
            path.as_bytes(),
            &canonical_query,
            canonical_body,
        ] {
            framed.extend((part.len() as u64).to_be_bytes());
            framed.extend(part);
        }
        Ok(Self { scope, framed })
    }
}
/// Only safe correlation text is durable. Intentionally no Debug/Serialize.
#[derive(Clone, PartialEq, Eq)]
pub struct Fingerprint {
    key_id: String,
    mac: [u8; 32],
}
fn valid_key_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 32
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
impl Fingerprint {
    pub fn as_storage(&self) -> String {
        format!(
            "hmac-sha256:v1:{}:{:x}",
            self.key_id,
            Sha256Output(self.mac)
        )
    }
    pub fn from_storage(value: &str) -> Result<Self, CommandError> {
        let mut parts = value.split(':');
        if parts.next() != Some("hmac-sha256") || parts.next() != Some("v1") {
            return Err(CommandError::InvalidState);
        }
        let id = parts.next().ok_or(CommandError::InvalidState)?;
        let hex = parts.next().ok_or(CommandError::InvalidState)?;
        if !valid_key_id(id)
            || parts.next().is_some()
            || hex.len() != 64
            || !hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(CommandError::InvalidState);
        }
        let mut mac = [0; 32];
        for (i, v) in mac.iter_mut().enumerate() {
            *v = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
                .map_err(|_| CommandError::InvalidState)?;
        }
        Ok(Self {
            key_id: id.to_owned(),
            mac,
        })
    }
}
/// Trusted key injection only. No defaults, environment reads, deserialization or Debug.
/// Reusing a key ID for different bytes is prohibited by the key-management contract.
pub struct SigningKey {
    id: String,
    secret: Vec<u8>,
}
impl SigningKey {
    pub fn new(id: &str, secret: &[u8]) -> Result<Self, CommandError> {
        if !valid_key_id(id) || secret.len() < 32 || secret.len() > 4096 {
            return Err(CommandError::InvalidInput);
        }
        Ok(Self {
            id: id.to_owned(),
            secret: secret.to_vec(),
        })
    }
}
/// Active signing key plus retained verification versions. Keep versions until reviewed
/// receipt/tombstone cleanup. Missing verification material fails closed, never reruns.
pub struct FingerprintKeys {
    active: SigningKey,
    retained: Vec<SigningKey>,
}
impl FingerprintKeys {
    pub fn new(active: SigningKey, retained: Vec<SigningKey>) -> Result<Self, CommandError> {
        if retained.len() > 31 {
            return Err(CommandError::InvalidInput);
        }
        let mut ids = std::collections::BTreeSet::new();
        ids.insert(active.id.as_str());
        if retained.iter().any(|key| !ids.insert(key.id.as_str())) {
            return Err(CommandError::InvalidInput);
        }
        Ok(Self { active, retained })
    }
    pub fn sign(&self, request: &CanonicalRequest) -> Result<Fingerprint, CommandError> {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.active.secret)
            .map_err(|_| CommandError::Unavailable)?;
        mac.update(&request.framed);
        Ok(Fingerprint {
            key_id: self.active.id.clone(),
            mac: mac.finalize().into_bytes().into(),
        })
    }
    pub fn verify(
        &self,
        request: &CanonicalRequest,
        stored: &Fingerprint,
    ) -> Result<bool, CommandError> {
        let key = std::iter::once(&self.active)
            .chain(self.retained.iter())
            .find(|key| key.id == stored.key_id)
            .ok_or(CommandError::Unavailable)?;
        let mut mac =
            Hmac::<Sha256>::new_from_slice(&key.secret).map_err(|_| CommandError::Unavailable)?;
        mac.update(&request.framed);
        Ok(mac.verify_slice(&stored.mac).is_ok())
    }
}
struct Sha256Output([u8; 32]);
impl std::fmt::LowerHex for Sha256Output {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SafeReference {
    pub resource: Uuid,
    pub revision: i64,
}
impl SafeReference {
    pub fn validate(self) -> Result<Self, CommandError> {
        if !(1..=9007199254740991).contains(&self.revision) {
            Err(CommandError::InvalidInput)
        } else {
            Ok(self)
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiptState {
    Pending,
    Succeeded,
    Failed,
    Retired,
}
impl ReceiptState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Succeeded => "SUCCEEDED",
            Self::Failed => "FAILED",
            Self::Retired => "RETIRED",
        }
    }
    pub fn parse(v: &str) -> Result<Self, CommandError> {
        match v {
            "PENDING" => Ok(Self::Pending),
            "SUCCEEDED" => Ok(Self::Succeeded),
            "FAILED" => Ok(Self::Failed),
            "RETIRED" => Ok(Self::Retired),
            _ => Err(CommandError::InvalidState),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SafeFailure {
    InvalidTransition,
    VersionConflict,
    DependencyUnavailable,
    InternalError,
    PartyFull,
    DuplicateApplication,
    GiveawaySoldOut,
    DuplicateActiveReservation,
    ReservationExpired,
    PickupCodeInvalid,
    ResourceUnavailable,
    InvalidInput,
    EventReadOnly,
    MapVersionMismatch,
    MuralPaused,
    MuralFrozen,
    MuralCooldown,
    MuralRegionLocked,
    MuralNoChange,
    MuralFillTooLarge,
    MuralBudgetExceeded,
    MuralUndoExpired,
    MuralUndoNotLatest,
    MuralUndoUnavailable,
    MuralToolUnavailable,
    MuralActionUnresolved,
}
impl SafeFailure {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidTransition => "INVALID_TRANSITION",
            Self::VersionConflict => "VERSION_CONFLICT",
            Self::DependencyUnavailable => "DEPENDENCY_UNAVAILABLE",
            Self::InternalError => "INTERNAL_ERROR",
            Self::PartyFull => "PARTY_FULL",
            Self::DuplicateApplication => "DUPLICATE_APPLICATION",
            Self::GiveawaySoldOut => "GIVEAWAY_SOLD_OUT",
            Self::DuplicateActiveReservation => "DUPLICATE_ACTIVE_RESERVATION",
            Self::ReservationExpired => "RESERVATION_EXPIRED",
            Self::PickupCodeInvalid => "PICKUP_CODE_INVALID",
            Self::ResourceUnavailable => "RESOURCE_UNAVAILABLE",
            Self::InvalidInput => "INVALID_INPUT",
            Self::EventReadOnly => "EVENT_READ_ONLY",
            Self::MapVersionMismatch => "MAP_VERSION_MISMATCH",
            Self::MuralPaused => "MURAL_PAUSED",
            Self::MuralFrozen => "MURAL_FROZEN",
            Self::MuralCooldown => "MURAL_COOLDOWN",
            Self::MuralRegionLocked => "MURAL_REGION_LOCKED",
            Self::MuralNoChange => "MURAL_NO_CHANGE",
            Self::MuralFillTooLarge => "MURAL_FILL_TOO_LARGE",
            Self::MuralBudgetExceeded => "MURAL_BUDGET_EXCEEDED",
            Self::MuralUndoExpired => "MURAL_UNDO_EXPIRED",
            Self::MuralUndoNotLatest => "MURAL_UNDO_NOT_LATEST",
            Self::MuralUndoUnavailable => "MURAL_UNDO_UNAVAILABLE",
            Self::MuralToolUnavailable => "MURAL_TOOL_UNAVAILABLE",
            Self::MuralActionUnresolved => "MURAL_ACTION_UNRESOLVED",
        }
    }
    pub fn parse(v: &str) -> Result<Self, CommandError> {
        match v {
            "INVALID_TRANSITION" => Ok(Self::InvalidTransition),
            "VERSION_CONFLICT" => Ok(Self::VersionConflict),
            "DEPENDENCY_UNAVAILABLE" => Ok(Self::DependencyUnavailable),
            "INTERNAL_ERROR" => Ok(Self::InternalError),
            "PARTY_FULL" => Ok(Self::PartyFull),
            "DUPLICATE_APPLICATION" => Ok(Self::DuplicateApplication),
            "GIVEAWAY_SOLD_OUT" => Ok(Self::GiveawaySoldOut),
            "DUPLICATE_ACTIVE_RESERVATION" => Ok(Self::DuplicateActiveReservation),
            "RESERVATION_EXPIRED" => Ok(Self::ReservationExpired),
            "PICKUP_CODE_INVALID" => Ok(Self::PickupCodeInvalid),
            "RESOURCE_UNAVAILABLE" => Ok(Self::ResourceUnavailable),
            "INVALID_INPUT" => Ok(Self::InvalidInput),
            "EVENT_READ_ONLY" => Ok(Self::EventReadOnly),
            "MAP_VERSION_MISMATCH" => Ok(Self::MapVersionMismatch),
            "MURAL_PAUSED" => Ok(Self::MuralPaused),
            "MURAL_FROZEN" => Ok(Self::MuralFrozen),
            "MURAL_COOLDOWN" => Ok(Self::MuralCooldown),
            "MURAL_REGION_LOCKED" => Ok(Self::MuralRegionLocked),
            "MURAL_NO_CHANGE" => Ok(Self::MuralNoChange),
            "MURAL_FILL_TOO_LARGE" => Ok(Self::MuralFillTooLarge),
            "MURAL_BUDGET_EXCEEDED" => Ok(Self::MuralBudgetExceeded),
            "MURAL_UNDO_EXPIRED" => Ok(Self::MuralUndoExpired),
            "MURAL_UNDO_NOT_LATEST" => Ok(Self::MuralUndoNotLatest),
            "MURAL_UNDO_UNAVAILABLE" => Ok(Self::MuralUndoUnavailable),
            "MURAL_TOOL_UNAVAILABLE" => Ok(Self::MuralToolUnavailable),
            "MURAL_ACTION_UNRESOLVED" => Ok(Self::MuralActionUnresolved),
            _ => Err(CommandError::InvalidState),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SafeReceipt {
    pub id: Uuid,
    pub scope: Scope,
    pub state: ReceiptState,
    pub reference: Option<SafeReference>,
    pub failure: Option<SafeFailure>,
    pub result_status: Option<i16>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completion {
    Pending,
    Succeeded {
        reference: SafeReference,
        status: i16,
    },
    Failed(SafeFailure),
}
impl Completion {
    pub fn validate(self) -> Result<Self, CommandError> {
        match self {
            Self::Succeeded { reference, status } => {
                reference.validate()?;
                if !(200..=299).contains(&status) || status == 202 {
                    return Err(CommandError::InvalidInput);
                }
            }
            _ => {}
        }
        Ok(self)
    }
}
/// Invalidations carry only a resource UUID/revision; consumers fetch after current authorization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Invalidation {
    pub reference: SafeReference,
}

pub mod maintenance;
