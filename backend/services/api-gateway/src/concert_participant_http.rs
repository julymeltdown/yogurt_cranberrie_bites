//! Captured Participant HTTP input shared by the route and signed dispatch adapter.

use uuid::Uuid;

pub const MAX_BODY: usize = 8192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Read,
    Avatar,
    Update,
}

impl Operation {
    pub fn method(self) -> &'static str {
        match self {
            Self::Read => "GET",
            Self::Avatar => "PUT",
            Self::Update => "PATCH",
        }
    }
    pub fn target(self, event: Uuid) -> String {
        format!(
            "/api/concert/events/{event}/me{}",
            if self == Self::Avatar { "/avatar" } else { "" }
        )
    }
}

pub struct ParticipantHttpRequest {
    pub event: Uuid,
    pub request_id: Uuid,
    pub operation: Operation,
    pub body: Vec<u8>,
}
