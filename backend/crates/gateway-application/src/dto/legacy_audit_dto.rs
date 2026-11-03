//! Private legacy audit transfers. These are not public response models.
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LegacyAuditOutcomeDto {
    Success,
    Failure,
}

impl LegacyAuditOutcomeDto {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "SUCCESS",
            Self::Failure => "FAILURE",
        }
    }
}

// Deliberately no Debug or serialization: actor and request context stay private.
pub struct LegacyAuditCommandDto {
    pub event_id: Uuid,
    pub actor_id: Option<String>,
    pub action: String,
    pub resource_path: String,
    pub outcome: LegacyAuditOutcomeDto,
    pub request_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}
