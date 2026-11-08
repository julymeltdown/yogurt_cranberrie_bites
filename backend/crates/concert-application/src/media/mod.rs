//! BE24 headless media values. These are internal values, never browser authority.
use crate::commands::CommandError;
use async_trait::async_trait;
use uuid::Uuid;

pub const MAX_BYTES: usize = 10 * 1024 * 1024;
pub const MAX_DIMENSION: u32 = 4096;
pub const MAX_PIXELS: u64 = 16_000_000;
pub const UPLOAD_SECONDS: i64 = 900;
pub const ORPHAN_SECONDS: i64 = 86400;
/// The closed purpose set of one media asset.
///
/// The first four are the event-scoped concert purposes (`concert_media`). The
/// last four are the profile purposes (`profile_media`, WP-02 T-106). They live
/// in **one** enum on purpose: a second media domain would be a second place for
/// the byte, dimension and pixel limits to drift. What keeps the two scopes
/// apart is not the type but [`Purpose::scope`], which every producer checks
/// before it touches a row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Purpose {
    Post,
    Giveaway,
    Report,
    Beacon,
    ProfileImage,
    ProfilePost,
    IntroPost,
    RoomPoster,
}

/// Which media table a purpose belongs to.
///
/// A purpose is never valid in both scopes. The concert table's CHECK constraint
/// refuses the profile values and the profile table's CHECK constraint refuses
/// the concert values, so this method is the in-process half of the same rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaScope {
    /// `concert_media`: event-scoped, participant-fenced.
    Concert,
    /// `profile_media`: profile-scoped, no event.
    Profile,
}

impl Purpose {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Post => "POST",
            Self::Giveaway => "GIVEAWAY",
            Self::Report => "REPORT",
            Self::Beacon => "BEACON",
            Self::ProfileImage => "PROFILE_IMAGE",
            Self::ProfilePost => "PROFILE_POST",
            Self::IntroPost => "INTRO_POST",
            Self::RoomPoster => "ROOM_POSTER",
        }
    }
    pub fn parse(v: &str) -> Result<Self, CommandError> {
        match v {
            "POST" => Ok(Self::Post),
            "GIVEAWAY" => Ok(Self::Giveaway),
            "REPORT" => Ok(Self::Report),
            "BEACON" => Ok(Self::Beacon),
            "PROFILE_IMAGE" => Ok(Self::ProfileImage),
            "PROFILE_POST" => Ok(Self::ProfilePost),
            "INTRO_POST" => Ok(Self::IntroPost),
            "ROOM_POSTER" => Ok(Self::RoomPoster),
            _ => Err(CommandError::InvalidInput),
        }
    }
    /// The media table this purpose is stored in.
    pub fn scope(self) -> MediaScope {
        match self {
            Self::Post | Self::Giveaway | Self::Report | Self::Beacon => MediaScope::Concert,
            Self::ProfileImage | Self::ProfilePost | Self::IntroPost | Self::RoomPoster => {
                MediaScope::Profile
            }
        }
    }
    /// Whether this purpose is one of the four profile purposes.
    pub fn is_profile(self) -> bool {
        self.scope() == MediaScope::Profile
    }
    /// The four profile purposes, in the order the contract lists them.
    pub const PROFILE: [Self; 4] = [
        Self::ProfileImage,
        Self::ProfilePost,
        Self::IntroPost,
        Self::RoomPoster,
    ];
    /// The event-scoped concert purposes, in the order `concert_media` lists them.
    pub const CONCERT: [Self; 4] = [Self::Post, Self::Giveaway, Self::Report, Self::Beacon];
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
