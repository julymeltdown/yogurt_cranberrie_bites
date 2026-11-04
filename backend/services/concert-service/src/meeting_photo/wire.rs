//! Closed beacon-photo adapter values. Image identifiers never confer authority.
//! Intentionally no Debug: guidance and permission assertions are private input.
use chrono::{DateTime, Utc};
use concert_application::beacon::{BeaconMutation, MeetingError};
use concert_domain::beacon::{self, Audience, MeetBeacon};
use serde::{de::Error, Deserialize, Deserializer, Serialize};
use uuid::Uuid;

fn uuid<'de, D: Deserializer<'de>>(d: D) -> Result<Uuid, D::Error> {
    let value = String::deserialize(d)?;
    let id = Uuid::parse_str(&value).map_err(D::Error::custom)?;
    if id.is_nil() || id.to_string() != value {
        return Err(D::Error::custom("invalid identifier"));
    }
    Ok(id)
}
fn optional_uuid<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Uuid>, D::Error> {
    let value = Option::<String>::deserialize(d)?;
    value
        .map(|value| {
            let id = Uuid::parse_str(&value).map_err(D::Error::custom)?;
            if id.is_nil() || id.to_string() != value {
                return Err(D::Error::custom("invalid identifier"));
            }
            Ok(id)
        })
        .transpose()
}
fn uuids<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Uuid>, D::Error> {
    let values = Vec::<String>::deserialize(d)?;
    values
        .into_iter()
        .map(|value| {
            let id = Uuid::parse_str(&value).map_err(D::Error::custom)?;
            if id.is_nil() || id.to_string() != value {
                return Err(D::Error::custom("invalid identifier"));
            }
            Ok(id)
        })
        .collect()
}
fn utc<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
    let text = String::deserialize(d)?;
    if !text.ends_with('Z') {
        return Err(D::Error::custom("UTC timestamp required"));
    }
    DateTime::parse_from_rfc3339(&text)
        .map(|v| v.with_timezone(&Utc))
        .map_err(D::Error::custom)
}
fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum PhotoAudience {
    PartyMembers {
        #[serde(deserialize_with = "uuid")]
        party_id: Uuid,
    },
    ExplicitUsers {
        #[serde(deserialize_with = "uuids")]
        user_ids: Vec<Uuid>,
    },
}
impl PhotoAudience {
    fn domain(&self) -> Audience {
        match self {
            Self::PartyMembers { party_id } => Audience::PartyMembers {
                party_id: *party_id,
            },
            Self::ExplicitUsers { user_ids } => Audience::ExplicitUsers {
                user_ids: user_ids.clone(),
            },
        }
    }
}
impl From<Audience> for PhotoAudience {
    fn from(v: Audience) -> Self {
        match v {
            Audience::PartyMembers { party_id } => Self::PartyMembers { party_id },
            Audience::ExplicitUsers { user_ids } => Self::ExplicitUsers { user_ids },
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhotoDescription {
    #[serde(deserialize_with = "uuid")]
    pub place_id: Uuid,
    pub map_revision: i64,
    pub place_revision: i64,
    pub instructions: String,
    pub identification: String,
    pub audience: PhotoAudience,
    #[serde(deserialize_with = "utc")]
    pub expires_at: DateTime<Utc>,
    #[serde(
        default,
        deserialize_with = "optional_uuid",
        skip_serializing_if = "Option::is_none"
    )]
    pub media_id: Option<Uuid>,
}
impl PhotoDescription {
    pub fn domain(&self) -> beacon::MeetingDescription {
        beacon::MeetingDescription {
            place_id: self.place_id,
            map_revision: self.map_revision,
            place_revision: self.place_revision,
            instructions: self.instructions.clone(),
            identification: self.identification.clone(),
            audience: self.audience.domain(),
            expires_at: self.expires_at,
        }
    }
    pub fn validate(&self) -> Result<(), MeetingError> {
        if !self.domain().valid() || self.media_id.is_some_and(|id| id.is_nil()) {
            Err(MeetingError::InvalidInput)
        } else {
            Ok(())
        }
    }
    pub fn from_domain(v: beacon::MeetingDescription, media_id: Option<Uuid>) -> Self {
        Self {
            place_id: v.place_id,
            map_revision: v.map_revision,
            place_revision: v.place_revision,
            instructions: v.instructions,
            identification: v.identification,
            audience: v.audience.into(),
            expires_at: v.expires_at,
            media_id,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatePhotoBeacon {
    pub meeting: PhotoDescription,
    pub physical_presence_acknowledged: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub photo_permission_acknowledged: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdatePhotoBeacon {
    pub expected_revision: i64,
    pub meeting: PhotoDescription,
    pub physical_presence_acknowledged: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub photo_permission_acknowledged: bool,
}
impl CreatePhotoBeacon {
    pub fn parse(bytes: &[u8]) -> Result<Self, MeetingError> {
        if bytes.len() > 65536 {
            return Err(MeetingError::InvalidInput);
        }
        let v: Self = serde_json::from_slice(bytes).map_err(|_| MeetingError::InvalidInput)?;
        v.validate()?;
        Ok(v)
    }
    pub fn validate(&self) -> Result<(), MeetingError> {
        self.meeting.validate()?;
        if !self.physical_presence_acknowledged
            || (self.meeting.media_id.is_some() && !self.photo_permission_acknowledged)
        {
            Err(MeetingError::InvalidInput)
        } else {
            Ok(())
        }
    }
    pub fn text_mutation(&self) -> BeaconMutation {
        BeaconMutation::Create(beacon::CreateBeacon {
            meeting: self.meeting.domain(),
            physical_presence_acknowledged: self.physical_presence_acknowledged,
        })
    }
}
impl UpdatePhotoBeacon {
    pub fn parse(bytes: &[u8]) -> Result<Self, MeetingError> {
        if bytes.len() > 65536 {
            return Err(MeetingError::InvalidInput);
        }
        let v: Self = serde_json::from_slice(bytes).map_err(|_| MeetingError::InvalidInput)?;
        v.validate()?;
        Ok(v)
    }
    pub fn validate(&self) -> Result<(), MeetingError> {
        self.meeting.validate()?;
        if !self.physical_presence_acknowledged
            || (self.meeting.media_id.is_some() && !self.photo_permission_acknowledged)
            || !(1..concert_domain::MAX_REVISION).contains(&self.expected_revision)
        {
            Err(MeetingError::InvalidInput)
        } else {
            Ok(())
        }
    }
    pub fn text_mutation(&self) -> BeaconMutation {
        BeaconMutation::Update(beacon::UpdateBeacon {
            expected_revision: self.expected_revision,
            meeting: self.meeting.domain(),
            physical_presence_acknowledged: self.physical_presence_acknowledged,
        })
    }
}
#[derive(Serialize)]
pub struct PhotoBeacon {
    pub beacon_id: Uuid,
    pub event_id: Uuid,
    pub owner_id: Uuid,
    pub meeting: PhotoDescription,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub state: beacon::BeaconState,
    pub revision: i64,
}
impl PhotoBeacon {
    pub fn from_authorized(beacon: MeetBeacon, media_id: Option<Uuid>) -> Self {
        Self {
            beacon_id: beacon.beacon_id,
            event_id: beacon.event_id,
            owner_id: beacon.owner_id,
            meeting: PhotoDescription::from_domain(beacon.meeting, media_id),
            created_at: beacon.created_at,
            updated_at: beacon.updated_at,
            state: beacon.state,
            revision: beacon.revision,
        }
    }
}
