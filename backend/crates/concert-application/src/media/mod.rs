//! BE24 headless media values. These are internal values, never browser authority.
use crate::commands::CommandError;
use async_trait::async_trait;
use uuid::Uuid;

pub const MAX_BYTES: usize = 10 * 1024 * 1024;
pub const MAX_DIMENSION: u32 = 4096;
pub const MAX_PIXELS: u64 = 16_000_000;
pub const UPLOAD_SECONDS: i64 = 900;
pub const ORPHAN_SECONDS: i64 = 86400;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Purpose {
    Post,
    Giveaway,
    Report,
}
impl Purpose {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Post => "POST",
            Self::Giveaway => "GIVEAWAY",
            Self::Report => "REPORT",
        }
    }
    pub fn parse(v: &str) -> Result<Self, CommandError> {
        match v {
            "POST" => Ok(Self::Post),
            "GIVEAWAY" => Ok(Self::Giveaway),
            "REPORT" => Ok(Self::Report),
            _ => Err(CommandError::InvalidInput),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mime {
    Jpeg,
    Png,
    Webp,
}
impl Mime {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Webp => "image/webp",
        }
    }
    pub fn parse(v: &str) -> Result<Self, CommandError> {
        match v {
            "image/jpeg" => Ok(Self::Jpeg),
            "image/png" => Ok(Self::Png),
            "image/webp" => Ok(Self::Webp),
            _ => Err(CommandError::InvalidInput),
        }
    }
}
/// Construct only from a freshly resolved canonical identity and current server policy.
/// The revision is a server fence, never a client assertion or a substitute for authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Context {
    pub actor: Uuid,
    pub event: Uuid,
    pub policy: PolicyFence,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicyFence {
    pub event_revision: i64,
    pub relationship_revision: i64,
    pub authority_revision: i64,
}
impl Context {
    pub fn validate(self) -> Result<Self, CommandError> {
        if self.actor.is_nil()
            || self.event.is_nil()
            || [
                self.policy.event_revision,
                self.policy.relationship_revision,
                self.policy.authority_revision,
            ]
            .iter()
            .any(|v| !(1..=9007199254740991).contains(v))
        {
            Err(CommandError::Denied)
        } else {
            Ok(self)
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitUpload {
    pub purpose: Purpose,
    pub mime: Mime,
    pub byte_length: usize,
    pub sha256: [u8; 32],
}
impl InitUpload {
    pub fn validate(&self) -> Result<(), CommandError> {
        if self.byte_length == 0 || self.byte_length > MAX_BYTES {
            Err(CommandError::InvalidInput)
        } else {
            Ok(())
        }
    }
}
/// No Debug: raw or sanitized image bytes must never appear in diagnostic logs.
pub struct Sanitized {
    pub bytes: Vec<u8>,
    pub mime: Mime,
    pub width: u32,
    pub height: u32,
}
#[async_trait]
pub trait ImageSanitizer: Send + Sync {
    /// Must admit bounded CPU work, verify detected format and single frame, fully decode,
    /// reencode without metadata and bound input/output/decoded allocations.
    async fn sanitize(&self, bytes: Vec<u8>, mime: Mime) -> Result<Sanitized, CommandError>;
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Access {
    Initialize(Purpose),
    Write(Purpose),
    Read(Purpose),
    Link(Purpose),
    Delete(Purpose),
}
/// Target linkage is trusted local storage, not supplied in a media read request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Target {
    pub resource: Uuid,
    pub purpose: Purpose,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaOperation {
    Initialize,
    Put,
    Finalize,
}
impl MediaOperation {
    pub fn id(self) -> &'static str {
        match self {
            Self::Initialize => "initConcertUpload",
            Self::Put => "putConcertUploadBytes",
            Self::Finalize => "finalizeConcertUpload",
        }
    }
}
