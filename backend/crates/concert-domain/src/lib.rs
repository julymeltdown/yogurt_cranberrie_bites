//! Pure Concert values. Auth UUIDs are references, never a second identity store.
pub mod avatar_motion_v2;
pub mod report;
pub mod place;
pub mod venue_meeting;
use chrono::{DateTime, Utc};
use std::str::FromStr;
use uuid::Uuid;

pub const MAX_REVISION: i64 = 9_007_199_254_740_991;
pub const EVENT_TIMEZONE: &str = "Asia/Seoul";

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    #[error("identifier must be a canonical lower-case UUID")]
    Identifier,
    #[error("event slug is invalid")]
    Slug,
    #[error("event title is invalid")]
    Title,
    #[error("revision is outside the positive safe-integer range")]
    Revision,
    #[error("asset identifier is invalid")]
    AssetId,
    #[error("event phase is invalid")]
    Phase,
    #[error("theme is invalid")]
    Theme,
    #[error("update time precedes creation time")]
    Timestamp,
}

macro_rules! identifier {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub struct $name(Uuid);
        impl $name {
            pub fn from_uuid(value: Uuid) -> Self {
                Self(value)
            }
            pub fn uuid(self) -> Uuid {
                self.0
            }
        }
        impl FromStr for $name {
            type Err = ValidationError;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                let id = Uuid::parse_str(value).map_err(|_| ValidationError::Identifier)?;
                if id.to_string() != value {
                    return Err(ValidationError::Identifier);
                }
                Ok(Self(id))
            }
        }
    };
}
identifier!(EventId);
identifier!(AuthUserId);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParticipantKey {
    pub event_id: EventId,
    pub user_id: AuthUserId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Revision(i64);
impl Revision {
    pub fn new(value: i64) -> Result<Self, ValidationError> {
        if !(1..=MAX_REVISION).contains(&value) {
            return Err(ValidationError::Revision);
        }
        Ok(Self(value))
    }
    pub fn initial() -> Self {
        Self(1)
    }
    pub fn get(self) -> i64 {
        self.0
    }
    pub fn next(self) -> Result<Self, ValidationError> {
        Self::new(self.0 + 1)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventSlug(String);
impl EventSlug {
    pub fn new(value: String) -> Result<Self, ValidationError> {
        if value.is_empty()
            || value.len() > 64
            || !value.as_bytes()[0].is_ascii_alphanumeric()
            || !value
                .bytes()
                .all(|v| v.is_ascii_lowercase() || v.is_ascii_digit() || v == b'-')
        {
            return Err(ValidationError::Slug);
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventTitle(String);
impl EventTitle {
    pub fn new(value: String) -> Result<Self, ValidationError> {
        if value.chars().count() > 120
            || value.trim().is_empty()
            || value.chars().any(char::is_control)
        {
            return Err(ValidationError::Title);
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetId(String);
impl AssetId {
    pub fn new(value: String) -> Result<Self, ValidationError> {
        if value.is_empty()
            || value.len() > 64
            || !value.as_bytes()[0].is_ascii_lowercase()
            || !value
                .bytes()
                .all(|v| v.is_ascii_lowercase() || v.is_ascii_digit() || v == b'_')
        {
            return Err(ValidationError::AssetId);
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventPhase {
    Draft,
    Open,
    Live,
    ReadOnly,
    Archived,
}
impl EventPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "DRAFT",
            Self::Open => "OPEN",
            Self::Live => "LIVE",
            Self::ReadOnly => "READ_ONLY",
            Self::Archived => "ARCHIVED",
        }
    }
    pub fn permits_participant_writes(self) -> bool {
        matches!(self, Self::Open | Self::Live)
    }
}
impl FromStr for EventPhase {
    type Err = ValidationError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "DRAFT" => Ok(Self::Draft),
            "OPEN" => Ok(Self::Open),
            "LIVE" => Ok(Self::Live),
            "READ_ONLY" => Ok(Self::ReadOnly),
            "ARCHIVED" => Ok(Self::Archived),
            _ => Err(ValidationError::Phase),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Revenge,
    Parade,
    Danger,
}
impl Theme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Revenge => "REVENGE",
            Self::Parade => "PARADE",
            Self::Danger => "DANGER",
        }
    }
}
impl FromStr for Theme {
    type Err = ValidationError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "REVENGE" => Ok(Self::Revenge),
            "PARADE" => Ok(Self::Parade),
            "DANGER" => Ok(Self::Danger),
            _ => Err(ValidationError::Theme),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub theme: Theme,
    pub low_power: bool,
    pub reduced_motion: bool,
    pub sound_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub id: EventId,
    pub slug: EventSlug,
    pub title: EventTitle,
    pub phase: EventPhase,
    pub revision: Revision,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Participant {
    pub key: ParticipantKey,
    pub revision: Revision,
    pub preferences: Preferences,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Structural references only. Catalog compatibility/provenance is BE07 policy.
/// The representation is exclusive; no synthetic component IDs or URLs are invented.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvatarSelection {
    pub manifest_id: AssetId,
    pub manifest_version: Revision,
    pub appearance: AvatarAppearance,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AvatarAppearance {
    ReviewedMotion(avatar_motion_v2::reviewed::Appearance),
    Layered {
        body: AssetId,
        eyes: AssetId,
        hair: AssetId,
        outfit: AssetId,
        accessory: Option<AssetId>,
        skin_tone: AssetId,
    },
    WholeCharacter {
        preset_id: AssetId,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvatarSettings {
    pub key: ParticipantKey,
    pub revision: Revision,
    pub selection: AvatarSelection,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub fn validate_times(
    created: DateTime<Utc>,
    updated: DateTime<Utc>,
) -> Result<(), ValidationError> {
    if updated < created {
        return Err(ValidationError::Timestamp);
    }
    Ok(())
}

pub mod relationship;
pub mod beacon;

pub mod rally;
pub mod feed;
pub mod avatar;
/// WP-08: the profile-owned avatar. No event, no participant key.
pub mod profile_avatar;
/// WP-07: the personal room layout. Subordinate to the profile, never an intro post.
pub mod profile_room;
pub mod world;
pub mod community;

pub mod intro_policy;
pub mod age_policy;

pub mod event_lifecycle;

pub mod mural;

pub mod notification;
pub mod venue_location;
pub mod venue_notes;
