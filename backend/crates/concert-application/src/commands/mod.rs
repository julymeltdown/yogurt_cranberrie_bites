//! Internal BE15 seams. A scope is correlation, never authentication or authorization.
//! Producers must resolve canonical identity, validate closed input schemas and enforce
//! current target/policy guards inside the aggregate transaction. No public DTOs here.
mod registry;
use chrono::{DateTime, Utc};
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
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RequestHash([u8; 32]);
impl RequestHash {
    /// Query values must already be decoded, closed-schema validated with defaults resolved.
    /// Method is fixed by registry; path is constructed from canonical registered parameters.
    pub fn canonical(
        operation: RegisteredOperation,
        parameters: &[(&str, &str)],
        query: &[(&str, &str)],
        body: Option<&CanonicalValue>,
    ) -> Result<Self, CommandError> {
        if operation.binary {
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
        if operation.method == "DELETE" && body.is_some() {
            return Err(CommandError::InvalidInput);
        }
        Self::encode(operation, parameters, query, &canonical_body)
    }
    /// Exact binary bytes, at most 10 MiB; content/media schema checks remain producer
    /// obligations. Only length and SHA256 enter the request digest, never the bytes.
    pub fn binary(
        operation: RegisteredOperation,
        parameters: &[(&str, &str)],
        query: &[(&str, &str)],
        bytes: &[u8],
    ) -> Result<Self, CommandError> {
        if !operation.binary || bytes.len() > 10 * 1024 * 1024 {
            return Err(CommandError::InvalidInput);
        }
        let mut body = b"BINARY-SHA256:".to_vec();
        body.extend((bytes.len() as u64).to_be_bytes());
        body.extend(Sha256::digest(bytes));
        Self::encode(operation, parameters, query, &body)
    }
    fn encode(
        operation: RegisteredOperation,
        parameters: &[(&str, &str)],
        query: &[(&str, &str)],
        canonical_body: &[u8],
    ) -> Result<Self, CommandError> {
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
        let mut digest = Sha256::new();
        for part in [
            b"MCR-BE15-v1".as_slice(),
            operation.method.as_bytes(),
            path.as_bytes(),
            &canonical_query,
            canonical_body,
        ] {
            digest.update((part.len() as u64).to_be_bytes());
            digest.update(part);
        }
        Ok(Self(digest.finalize().into()))
    }
    pub fn as_storage(self) -> String {
        format!("sha256:{:x}", Sha256Output(self.0))
    }
    pub fn from_storage(value: &str) -> Result<Self, CommandError> {
        let hex = value
            .strip_prefix("sha256:")
            .ok_or(CommandError::InvalidInput)?;
        if hex.len() != 64
            || !hex
                .bytes()
                .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
        {
            return Err(CommandError::InvalidInput);
        }
        let mut out = [0; 32];
        for (i, v) in out.iter_mut().enumerate() {
            *v = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
                .map_err(|_| CommandError::InvalidInput)?;
        }
        Ok(Self(out))
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
}
impl SafeFailure {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidTransition => "INVALID_TRANSITION",
            Self::VersionConflict => "VERSION_CONFLICT",
            Self::DependencyUnavailable => "DEPENDENCY_UNAVAILABLE",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }
    pub fn parse(v: &str) -> Result<Self, CommandError> {
        match v {
            "INVALID_TRANSITION" => Ok(Self::InvalidTransition),
            "VERSION_CONFLICT" => Ok(Self::VersionConflict),
            "DEPENDENCY_UNAVAILABLE" => Ok(Self::DependencyUnavailable),
            "INTERNAL_ERROR" => Ok(Self::InternalError),
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
