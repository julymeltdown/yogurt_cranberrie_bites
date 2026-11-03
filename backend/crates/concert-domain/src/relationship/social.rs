//! Private social values deliberately have no Serialize implementation.
use super::RelationshipError;
use crate::{AuthUserId, EventId, Revision};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Platform {
    Instagram,
    X,
}
impl Platform {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Instagram => "INSTAGRAM",
            Self::X => "X",
        }
    }
}
impl std::str::FromStr for Platform {
    type Err = RelationshipError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "INSTAGRAM" => Ok(Self::Instagram),
            "X" => Ok(Self::X),
            _ => Err(RelationshipError::Denied),
        }
    }
}
/// Only the explicit reveal adapter may read this value. Debug always redacts it.
pub struct PrivateHandle(String);
impl std::fmt::Debug for PrivateHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PrivateHandle([REDACTED])")
    }
}
impl PrivateHandle {
    pub fn normalize(platform: Platform, raw: &str) -> Result<Self, RelationshipError> {
        let valid = !raw.is_empty()
            && raw.bytes().all(|b| {
                b.is_ascii_alphanumeric()
                    || b == b'_'
                    || (platform == Platform::Instagram && b == b'.')
            });
        let bound = if platform == Platform::X { 15 } else { 30 };
        if !valid
            || raw.len() > bound
            || raw.starts_with('.')
            || raw.ends_with('.')
            || raw.contains("..")
        {
            return Err(RelationshipError::Denied);
        }
        Ok(Self(raw.to_ascii_lowercase()))
    }
    pub fn expose_for_reveal(&self) -> &str {
        &self.0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandleContext {
    pub event: EventId,
    pub owner: AuthUserId,
    pub platform: Platform,
    pub version: Revision,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConsentBinding {
    pub platform: Platform,
    pub requester_version: Revision,
    pub recipient_version: Revision,
    pub requester_consents: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SharingContext {
    pub event: EventId,
    pub requester: AuthUserId,
    pub recipient: AuthUserId,
    pub intro_id: Uuid,
    pub intro_revision: Revision,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreshSharing {
    pub context: SharingContext,
    pub checked_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub allowed: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandleState {
    pub event: EventId,
    pub owner: AuthUserId,
    pub platform: Platform,
    /// Zero only for a platform never configured; removals retain positive tombstones.
    pub version: i64,
    pub configured: bool,
}
/// Secret-bearing output never enters ordinary responses, events, notifications or caches.
#[derive(Debug)]
pub struct PrivateReveal {
    pub request_id: Uuid,
    pub context: HandleContext,
    pub handle: PrivateHandle,
    pub grant_revision: Revision,
    pub expires_at: DateTime<Utc>,
}
impl PrivateReveal {
    pub const CACHE_CONTROL: &'static str = "no-store";
}
