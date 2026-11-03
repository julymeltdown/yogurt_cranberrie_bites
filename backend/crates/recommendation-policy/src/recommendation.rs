use base64ct::{Base64UrlUnpadded, Encoding};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};
use uuid::Uuid;

pub const RECOMMENDATION_SCHEMA_VERSION: u16 = 1;
pub const DEFAULT_RECENT_ACTION_LIMIT: u16 = 20;
pub const MAX_RECENT_ACTION_LIMIT: u16 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecentActionLimit(u16);

impl RecentActionLimit {
    pub fn new(requested: i32) -> Self {
        if requested <= 0 {
            Self(DEFAULT_RECENT_ACTION_LIMIT)
        } else {
            Self(
                u16::try_from(requested)
                    .unwrap_or(u16::MAX)
                    .min(MAX_RECENT_ACTION_LIMIT),
            )
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}
const CURSOR_VERSION: u8 = 1;
pub const MAX_RECOMMENDATION_CURSOR_LENGTH: usize = 512;
const MAX_SOURCE_LENGTH: usize = 64;
const MAX_MODEL_VERSION_LENGTH: usize = 64;
const MAX_REQUEST_ID_LENGTH: usize = 128;
const MAX_METADATA_LENGTH: usize = 4096;
const MAX_POSITION: u32 = 999;
const MAX_DWELL_MS: u32 = 86_400_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RolloutMode {
    Deterministic,
    Shadow,
    Canary,
    Learned,
}

impl RolloutMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deterministic => "DETERMINISTIC",
            Self::Shadow => "SHADOW",
            Self::Canary => "CANARY",
            Self::Learned => "LEARNED",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RecommendationContractError> {
        match value.trim().to_ascii_uppercase().as_str() {
            "DETERMINISTIC" | "LINEAR" | "FALLBACK" | "OFF" => Ok(Self::Deterministic),
            "SHADOW" => Ok(Self::Shadow),
            "CANARY" => Ok(Self::Canary),
            "LEARNED" | "ACTIVE" => Ok(Self::Learned),
            _ => Err(RecommendationContractError::new(
                "Unsupported recommendation rollout mode",
            )),
        }
    }

    pub fn is_deterministic_primary(self) -> bool {
        self == Self::Deterministic
    }

    pub fn uses_deterministic_primary(self) -> bool {
        matches!(self, Self::Deterministic | Self::Shadow | Self::Canary)
    }

    pub fn requests_learned_scoring(self) -> bool {
        matches!(self, Self::Shadow | Self::Canary | Self::Learned)
    }
}

pub fn stable_experiment_bucket(viewer_id: Uuid, experiment_key: &str, buckets: u32) -> u32 {
    if buckets == 0 {
        return 0;
    }
    let mut hash = 0xcbf29ce484222325u64;
    for byte in viewer_id
        .as_bytes()
        .iter()
        .copied()
        .chain(experiment_key.as_bytes().iter().copied())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    (hash % u64::from(buckets)) as u32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecommendationSurface {
    PeopleRecommendation,
    HomeFeed,
    Search,
    Profile,
}

impl RecommendationSurface {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PeopleRecommendation => "PEOPLE_RECOMMENDATION",
            Self::HomeFeed => "HOME_FEED",
            Self::Search => "SEARCH",
            Self::Profile => "PROFILE",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RecommendationContractError> {
        match value.trim().to_ascii_uppercase().as_str() {
            "PEOPLE_RECOMMENDATION" => Ok(Self::PeopleRecommendation),
            "HOME_FEED" => Ok(Self::HomeFeed),
            "SEARCH" => Ok(Self::Search),
            "PROFILE" => Ok(Self::Profile),
            _ => Err(RecommendationContractError::new(
                "Unsupported recommendation surface",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecommendationSubjectType {
    User,
    Post,
    Comment,
    Recommendation,
}

impl RecommendationSubjectType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "USER",
            Self::Post => "POST",
            Self::Comment => "COMMENT",
            Self::Recommendation => "RECOMMENDATION",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RecommendationContractError> {
        match value.trim().to_ascii_uppercase().as_str() {
            "USER" => Ok(Self::User),
            "POST" => Ok(Self::Post),
            "COMMENT" => Ok(Self::Comment),
            "RECOMMENDATION" => Ok(Self::Recommendation),
            _ => Err(RecommendationContractError::new(
                "Unsupported recommendation subject type",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecommendationAction {
    Impression,
    Click,
    Dwell,
    Like,
    Comment,
    Follow,
    AcceptFollow,
    Hide,
    NotInterested,
    Block,
    Mute,
    Report,
    Save,
    Share,
    FastScroll,
}

impl RecommendationAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Impression => "IMPRESSION",
            Self::Click => "CLICK",
            Self::Dwell => "DWELL",
            Self::Like => "LIKE",
            Self::Comment => "COMMENT",
            Self::Follow => "FOLLOW",
            Self::AcceptFollow => "ACCEPT_FOLLOW",
            Self::Hide => "HIDE",
            Self::NotInterested => "NOT_INTERESTED",
            Self::Block => "BLOCK",
            Self::Mute => "MUTE",
            Self::Report => "REPORT",
            Self::Save => "SAVE",
            Self::Share => "SHARE",
            Self::FastScroll => "FAST_SCROLL",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RecommendationContractError> {
        match value.trim().to_ascii_uppercase().as_str() {
            "IMPRESSION" => Ok(Self::Impression),
            "CLICK" | "PROFILE_OPEN" => Ok(Self::Click),
            "DWELL" => Ok(Self::Dwell),
            "LIKE" => Ok(Self::Like),
            "COMMENT" => Ok(Self::Comment),
            "FOLLOW" | "FOLLOW_REQUEST" => Ok(Self::Follow),
            "ACCEPT_FOLLOW" | "FOLLOW_ACCEPTED" => Ok(Self::AcceptFollow),
            "HIDE" => Ok(Self::Hide),
            "NOT_INTERESTED" | "DISMISS" => Ok(Self::NotInterested),
            "BLOCK" => Ok(Self::Block),
            "MUTE" => Ok(Self::Mute),
            "REPORT" => Ok(Self::Report),
            "SAVE" | "BOOKMARK" => Ok(Self::Save),
            "SHARE" => Ok(Self::Share),
            "FAST_SCROLL" | "SHORT_ABANDONMENT" => Ok(Self::FastScroll),
            _ => Err(RecommendationContractError::new(
                "Unsupported recommendation action",
            )),
        }
    }

    pub fn is_impression(self) -> bool {
        self == Self::Impression
    }

    pub fn is_negative(self) -> bool {
        matches!(
            self,
            Self::Hide
                | Self::NotInterested
                | Self::Block
                | Self::Mute
                | Self::Report
                | Self::FastScroll
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecommendationEvent {
    pub event_id: Uuid,
    pub schema_version: u16,
    pub occurred_at: DateTime<Utc>,
    pub viewer_id: Uuid,
    pub subject_type: RecommendationSubjectType,
    pub subject_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub surface: RecommendationSurface,
    pub position: Option<u32>,
    pub request_id: Option<String>,
    pub session_id: Option<Uuid>,
    pub action: RecommendationAction,
    pub source: String,
    pub model_version: String,
    pub impression_id: Option<Uuid>,
    pub propensity_micros: Option<u32>,
    pub dwell_ms: Option<u32>,
    pub metadata_json: String,
}

impl RecommendationEvent {
    pub fn validate(&self) -> Result<(), RecommendationContractError> {
        if self.schema_version != RECOMMENDATION_SCHEMA_VERSION {
            return Err(RecommendationContractError::new(
                "Unsupported recommendation schema version",
            ));
        }
        if self.event_id.is_nil() || self.viewer_id.is_nil() || self.subject_id.is_nil() {
            return Err(RecommendationContractError::new(
                "Recommendation identifiers must be non-nil UUIDs",
            ));
        }
        validate_required_text(
            &self.source,
            MAX_SOURCE_LENGTH,
            "Recommendation source is required",
            "Recommendation source is too long",
        )?;
        validate_required_text(
            &self.model_version,
            MAX_MODEL_VERSION_LENGTH,
            "Recommendation model version is required",
            "Recommendation model version is too long",
        )?;
        if let Some(request_id) = self.request_id.as_deref() {
            if request_id.len() > MAX_REQUEST_ID_LENGTH || request_id.trim().is_empty() {
                return Err(RecommendationContractError::new(
                    "Recommendation request ID is invalid",
                ));
            }
        }
        if let Some(position) = self.position {
            if position > MAX_POSITION {
                return Err(RecommendationContractError::new(
                    "Recommendation position is out of range",
                ));
            }
        }
        if let Some(propensity) = self.propensity_micros {
            if propensity == 0 || propensity > 1_000_000 {
                return Err(RecommendationContractError::new(
                    "Recommendation propensity is out of range",
                ));
            }
        }
        if self.dwell_ms.is_some_and(|value| value > MAX_DWELL_MS) {
            return Err(RecommendationContractError::new(
                "Recommendation dwell time is out of range",
            ));
        }
        if self.metadata_json.len() > MAX_METADATA_LENGTH {
            return Err(RecommendationContractError::new(
                "Recommendation metadata is too large",
            ));
        }
        let metadata: serde_json::Value =
            serde_json::from_str(&self.metadata_json).map_err(|_| {
                RecommendationContractError::new("Recommendation metadata is invalid JSON")
            })?;
        if !metadata.is_object() {
            return Err(RecommendationContractError::new(
                "Recommendation metadata must be a JSON object",
            ));
        }

        if self.action.is_impression() {
            if self.position.is_none() {
                return Err(RecommendationContractError::new(
                    "Recommendation impression position is required",
                ));
            }
            if self.impression_id != Some(self.event_id) {
                return Err(RecommendationContractError::new(
                    "Impression ID must equal event ID for impression events",
                ));
            }
        } else if self.impression_id.is_none() {
            return Err(RecommendationContractError::new(
                "Recommendation outcome requires an impression ID",
            ));
        }

        if self.action == RecommendationAction::Dwell && self.dwell_ms.is_none() {
            return Err(RecommendationContractError::new(
                "Dwell events require dwell time",
            ));
        }
        Ok(())
    }
}

fn validate_required_text(
    value: &str,
    max: usize,
    missing: &str,
    oversized: &str,
) -> Result<(), RecommendationContractError> {
    if value.len() > max {
        return Err(RecommendationContractError::new(oversized));
    }
    if value.trim().is_empty() {
        return Err(RecommendationContractError::new(missing));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecommendationCursor {
    pub version: u8,
    pub snapshot_id: Uuid,
    pub score_micros: i64,
    pub occurred_at: DateTime<Utc>,
    pub tie_breaker_id: Uuid,
}

impl RecommendationCursor {
    pub fn encode(&self) -> String {
        let payload = format!(
            "{}|{}|{}|{}|{}",
            self.version,
            self.snapshot_id,
            self.score_micros,
            self.occurred_at.timestamp_millis(),
            self.tie_breaker_id
        );
        Base64UrlUnpadded::encode_string(payload.as_bytes())
    }

    pub fn parse(value: &str) -> Result<Self, RecommendationContractError> {
        if value.len() > MAX_RECOMMENDATION_CURSOR_LENGTH {
            return Err(RecommendationContractError::new(
                "Invalid recommendation cursor",
            ));
        }
        let value = value.trim();
        if value.is_empty() {
            return Err(RecommendationContractError::new(
                "Invalid recommendation cursor",
            ));
        }
        let decoded = Base64UrlUnpadded::decode_vec(value)
            .map_err(|_| RecommendationContractError::new("Invalid recommendation cursor"))?;
        let decoded = std::str::from_utf8(&decoded)
            .map_err(|_| RecommendationContractError::new("Invalid recommendation cursor"))?;
        let mut parts = decoded.split('|');
        let version = parts
            .next()
            .and_then(|value| value.parse::<u8>().ok())
            .ok_or_else(|| RecommendationContractError::new("Invalid recommendation cursor"))?;
        if version != CURSOR_VERSION {
            return Err(RecommendationContractError::new(
                "Unsupported recommendation cursor version",
            ));
        }
        let snapshot_id = parts
            .next()
            .and_then(|value| Uuid::parse_str(value).ok())
            .filter(|value| !value.is_nil())
            .ok_or_else(|| RecommendationContractError::new("Invalid recommendation cursor"))?;
        let score_micros = parts
            .next()
            .and_then(|value| value.parse::<i64>().ok())
            .ok_or_else(|| RecommendationContractError::new("Invalid recommendation cursor"))?;
        let occurred_at = parts
            .next()
            .and_then(|value| value.parse::<i64>().ok())
            .and_then(DateTime::<Utc>::from_timestamp_millis)
            .ok_or_else(|| RecommendationContractError::new("Invalid recommendation cursor"))?;
        let tie_breaker_id = parts
            .next()
            .and_then(|value| Uuid::parse_str(value).ok())
            .filter(|value| !value.is_nil())
            .ok_or_else(|| RecommendationContractError::new("Invalid recommendation cursor"))?;
        if parts.next().is_some() {
            return Err(RecommendationContractError::new(
                "Invalid recommendation cursor",
            ));
        }
        let cursor = Self {
            version,
            snapshot_id,
            score_micros,
            occurred_at,
            tie_breaker_id,
        };
        if cursor.encode() != value {
            return Err(RecommendationContractError::new(
                "Invalid recommendation cursor",
            ));
        }
        Ok(cursor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankMetadata {
    pub source: String,
    pub reason: String,
    pub rank_version: String,
    pub score_micros: i64,
    pub impression_id: Uuid,
    pub request_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecommendationContractError {
    message: String,
}

impl RecommendationContractError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for RecommendationContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for RecommendationContractError {}
