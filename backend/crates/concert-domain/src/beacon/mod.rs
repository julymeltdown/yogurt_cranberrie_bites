//! Physical self-reports. No virtual position, GPS evidence, photo URL or inferred arrival.
use crate::MAX_REVISION;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum Audience {
    PartyMembers { party_id: Uuid },
    ExplicitUsers { user_ids: Vec<Uuid> },
}
impl Audience {
    pub fn valid(&self) -> bool {
        match self {
            Self::PartyMembers { party_id } => !party_id.is_nil(),
            Self::ExplicitUsers { user_ids } => {
                !user_ids.is_empty()
                    && user_ids.len() <= 50
                    && user_ids.iter().all(|id| !id.is_nil())
                    && user_ids
                        .iter()
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        == user_ids.len()
            }
        }
    }
    pub fn explicitly_includes(&self, user: Uuid) -> bool {
        matches!(self, Self::ExplicitUsers { user_ids } if user_ids.contains(&user))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeetingDescription {
    pub place_id: Uuid,
    pub map_revision: i64,
    /// Current reviewed place state revision; distinct from the event catalog.
    pub place_revision: i64,
    pub instructions: String,
    pub identification: String,
    pub audience: Audience,
    pub expires_at: DateTime<Utc>,
}
impl MeetingDescription {
    pub fn valid(&self) -> bool {
        fn text(value: &str, limit: usize) -> bool {
            !value.trim().is_empty()
                && value.chars().count() <= limit
                && !value.chars().any(char::is_control)
        }
        !self.place_id.is_nil()
            && (1..=MAX_REVISION).contains(&self.map_revision)
            && (1..=MAX_REVISION).contains(&self.place_revision)
            && text(&self.instructions, 1000)
            && text(&self.identification, 240)
            && self.audience.valid()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateBeacon {
    pub meeting: MeetingDescription,
    pub physical_presence_acknowledged: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateBeacon {
    pub expected_revision: i64,
    pub meeting: MeetingDescription,
    pub physical_presence_acknowledged: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BeaconState {
    Active,
    Expired,
    Revoked,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeetBeacon {
    pub beacon_id: Uuid,
    pub event_id: Uuid,
    pub owner_id: Uuid,
    pub meeting: MeetingDescription,
    pub created_at: DateTime<Utc>,
    /// Last explicit confirmation; updates always require a fresh acknowledgement.
    pub updated_at: DateTime<Utc>,
    pub state: BeaconState,
    pub revision: i64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeaconStatus {
    pub beacon_id: Uuid,
    pub event_id: Uuid,
    pub state: BeaconState,
    pub revision: i64,
}
