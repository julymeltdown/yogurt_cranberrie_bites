//! Transaction adapter contracts. A grant is never derived from request claims.
use concert_domain::beacon::{Audience, CreateBeacon, UpdateBeacon};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MeetingError {
    #[error("resource unavailable")]
    ResourceUnavailable,
    #[error("meeting revision changed")]
    RevisionConflict,
    #[error("map revision changed")]
    MapMismatch,
    #[error("invalid meeting command")]
    InvalidInput,
    #[error("event is read only")]
    EventReadOnly,
    #[error("meeting authority unavailable")]
    PolicyUnavailable,
    #[error("meeting storage unavailable")]
    StorageUnavailable,
    #[error("invalid stored meeting state")]
    InvalidState,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeetingActor {
    pub event: Uuid,
    pub actor: Uuid,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeetingOperation {
    Read,
    Create,
    Update,
    Revoke,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudienceQuery {
    pub actor: MeetingActor,
    pub owner: Uuid,
    pub audience: Audience,
    pub operation: MeetingOperation,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudienceGrant {
    /// Adapters must bind their returned decision to the exact request.
    pub query: AudienceQuery,
}
#[derive(Clone, Debug)]
pub enum BeaconMutation {
    Create(CreateBeacon),
    Update(UpdateBeacon),
    Revoke { expected_revision: i64 },
}
impl BeaconMutation {
    pub fn operation_id(&self) -> &'static str {
        match self {
            Self::Create(_) => "createConcertBeacon",
            Self::Update(_) => "updateConcertBeacon",
            Self::Revoke { .. } => "revokeConcertBeacon",
        }
    }
    pub fn valid(&self) -> bool {
        match self {
            Self::Create(value) => value.physical_presence_acknowledged && value.meeting.valid(),
            Self::Update(value) => {
                value.physical_presence_acknowledged
                    && value.meeting.valid()
                    && (1..concert_domain::MAX_REVISION).contains(&value.expected_revision)
            }
            Self::Revoke { expected_revision } => {
                (1..concert_domain::MAX_REVISION).contains(expected_revision)
            }
        }
    }
}
