//! VENU-BE-M01 pure meeting values. Identifiers are lookup keys, never grants.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const MAX_LANDMARK_ID: usize = 128;
pub const MAX_MESSAGE_CHARS: usize = 2000;
pub const MAX_MESSAGE_BYTES: usize = 8000;
pub const PAGE_SIZE: i64 = 50;
pub const MAX_REQUEST_BYTES: usize = 12_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LandmarkSelection {
    pub landmark_id: String,
    pub catalog_revision: i64,
    pub landmark_revision: i64,
}

impl LandmarkSelection {
    pub fn new(
        landmark_id: impl Into<String>,
        catalog_revision: i64,
        landmark_revision: i64,
    ) -> Result<Self, MeetingError> {
        let landmark_id = landmark_id.into();
        if !(1..=MAX_LANDMARK_ID).contains(&landmark_id.len())
            || !landmark_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~'))
            || landmark_id == "."
            || landmark_id == ".."
        {
            return Err(MeetingError::InvalidInput);
        }
        if !(1..=crate::MAX_REVISION).contains(&catalog_revision)
            || !(1..=crate::MAX_REVISION).contains(&landmark_revision)
        {
            return Err(MeetingError::InvalidInput);
        }
        Ok(Self {
            landmark_id,
            catalog_revision,
            landmark_revision,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeetingSelection {
    pub selected_friend_id: Uuid,
    pub landmark: Option<LandmarkSelection>,
}

impl MeetingSelection {
    pub fn new(
        selected_friend_id: Uuid,
        landmark: Option<LandmarkSelection>,
    ) -> Result<Self, MeetingError> {
        if selected_friend_id.is_nil() {
            return Err(MeetingError::InvalidInput);
        }
        Ok(Self {
            selected_friend_id,
            landmark,
        })
    }
    pub fn pair(&self, actor: Uuid) -> (Uuid, Uuid) {
        if actor < self.selected_friend_id {
            (actor, self.selected_friend_id)
        } else {
            (self.selected_friend_id, actor)
        }
    }
    pub fn context_id(&self, actor: Uuid, event: Uuid) -> String {
        let (low, high) = self.pair(actor);
        let mut digest = Sha256::new();
        digest.update(b"venue-meeting-v1\0");
        digest.update(event.as_bytes());
        digest.update(low.as_bytes());
        digest.update(high.as_bytes());
        match &self.landmark {
            None => digest.update(b"\0none"),
            Some(landmark) => {
                digest.update(b"\0landmark\0");
                digest.update(landmark.landmark_id.as_bytes());
                digest.update(landmark.catalog_revision.to_be_bytes());
                digest.update(landmark.landmark_revision.to_be_bytes());
            }
        }
        format!("vm1_{}", hex(&digest.finalize()[..16]))
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(DIGITS[(byte >> 4) as usize] as char);
        value.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    value
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InvitationState {
    Pending,
    Accepted,
    Declined,
    Cancelled,
    Expired,
}

impl InvitationState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Accepted => "ACCEPTED",
            Self::Declined => "DECLINED",
            Self::Cancelled => "CANCELLED",
            Self::Expired => "EXPIRED",
        }
    }
    pub fn parse(value: &str) -> Result<Self, MeetingError> {
        match value {
            "PENDING" => Ok(Self::Pending),
            "ACCEPTED" => Ok(Self::Accepted),
            "DECLINED" => Ok(Self::Declined),
            "CANCELLED" => Ok(Self::Cancelled),
            "EXPIRED" => Ok(Self::Expired),
            _ => Err(MeetingError::StorageUnavailable),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Decision {
    Accept,
    Decline,
    Cancel,
}

impl Decision {
    pub fn parse(value: &str) -> Result<Self, MeetingError> {
        match value {
            "ACCEPT" => Ok(Self::Accept),
            "DECLINE" => Ok(Self::Decline),
            "CANCEL" => Ok(Self::Cancel),
            _ => Err(MeetingError::InvalidInput),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredInvitation {
    pub id: Uuid,
    pub ordinal: i64,
    pub event_id: Uuid,
    pub context_id: String,
    pub sender_id: Uuid,
    pub recipient_id: Uuid,
    pub selection: MeetingSelection,
    pub state: InvitationState,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub purge_at: DateTime<Utc>,
    pub policy_revision: i64,
}

impl StoredInvitation {
    pub fn allowed_actions(&self, actor: Uuid, now: DateTime<Utc>) -> Vec<Decision> {
        if now >= self.expires_at {
            return Vec::new();
        }
        if actor == self.recipient_id && self.state == InvitationState::Pending {
            vec![Decision::Accept, Decision::Decline]
        } else if actor == self.sender_id
            && matches!(
                self.state,
                InvitationState::Pending | InvitationState::Accepted
            )
        {
            vec![Decision::Cancel]
        } else {
            Vec::new()
        }
    }
    pub fn decision_state(
        &self,
        actor: Uuid,
        decision: Decision,
        now: DateTime<Utc>,
    ) -> Result<InvitationState, MeetingError> {
        if now >= self.expires_at {
            return Err(MeetingError::InvalidState);
        }
        match decision {
            Decision::Accept | Decision::Decline => {
                if actor != self.recipient_id || self.state != InvitationState::Pending {
                    return Err(MeetingError::ResourceUnavailable);
                }
                Ok(if decision == Decision::Accept {
                    InvitationState::Accepted
                } else {
                    InvitationState::Declined
                })
            }
            Decision::Cancel => {
                if actor != self.sender_id
                    || !matches!(
                        self.state,
                        InvitationState::Pending | InvitationState::Accepted
                    )
                {
                    return Err(MeetingError::ResourceUnavailable);
                }
                Ok(InvitationState::Cancelled)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredMessage {
    pub id: Uuid,
    pub event_id: Uuid,
    pub conversation_id: Uuid,
    pub sequence: i64,
    pub sender_id: Uuid,
    pub body: String,
    pub invitation_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub purge_at: DateTime<Utc>,
    pub policy_revision: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredContext {
    pub event_id: Uuid,
    pub context_id: String,
    pub member_low: Uuid,
    pub member_high: Uuid,
    pub landmark_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeetingPolicy {
    pub policy_revision: i64,
    pub valid_until: DateTime<Utc>,
    pub invitation_ms: i64,
    pub content_ms: i64,
    pub purge_ms: i64,
    pub event_timezone: String,
}

impl MeetingPolicy {
    pub fn valid(&self) -> bool {
        self.event_timezone == crate::EVENT_TIMEZONE
            && (1..=crate::MAX_REVISION).contains(&self.policy_revision)
            && self.invitation_ms > 0
            && self.content_ms > 0
            && self.purge_ms >= self.content_ms
    }
    pub fn invitation_deadline(&self, now: DateTime<Utc>) -> Result<(DateTime<Utc>, DateTime<Utc>), MeetingError> {
        deadline(now, self.invitation_ms, self.purge_ms, self.valid_until)
    }
    pub fn content_deadline(&self, now: DateTime<Utc>) -> Result<(DateTime<Utc>, DateTime<Utc>), MeetingError> {
        deadline(now, self.content_ms, self.purge_ms, self.valid_until)
    }
}

fn deadline(
    now: DateTime<Utc>,
    ttl_ms: i64,
    purge_ms: i64,
    valid_until: DateTime<Utc>,
) -> Result<(DateTime<Utc>, DateTime<Utc>), MeetingError> {
    let expires = (now + chrono::Duration::milliseconds(ttl_ms)).min(valid_until);
    if expires <= now || purge_ms <= 0 {
        return Err(MeetingError::PolicyUnavailable);
    }
    let purge = expires + chrono::Duration::milliseconds(purge_ms);
    if purge < expires {
        return Err(MeetingError::PolicyUnavailable);
    }
    Ok((expires, purge))
}

pub fn valid_message_body(body: &str) -> bool {
    body.len() <= MAX_MESSAGE_BYTES
        && (1..=MAX_MESSAGE_CHARS).contains(&body.chars().count())
        && body.chars().any(|c| !c.is_whitespace())
        && body
            .chars()
            .all(|c| !c.is_control() || matches!(c, '\n' | '\t'))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MeetingError {
    #[error("invalid meeting input")]
    InvalidInput,
    #[error("meeting resource unavailable")]
    ResourceUnavailable,
    #[error("meeting authority unavailable")]
    PolicyUnavailable,
    #[error("meeting revision changed")]
    RevisionConflict,
    #[error("meeting state conflict")]
    InvalidState,
    #[error("meeting storage unavailable")]
    StorageUnavailable,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn landmark_and_context_validation_is_strict() {
        assert!(LandmarkSelection::new("gate-a", 1, 2).is_ok());
        for id in ["", ".", "..", "a/b", "a b", "가"] {
            assert_eq!(
                LandmarkSelection::new(id, 1, 1),
                Err(MeetingError::InvalidInput)
            );
        }
        assert!(LandmarkSelection::new("gate-a", 0, 1).is_err());
        let actor = Uuid::from_u128(2);
        let event = Uuid::from_u128(9);
        let friend = Uuid::from_u128(1);
        let friend_only = MeetingSelection::new(friend, None).unwrap();
        let landmark = MeetingSelection::new(
            friend,
            Some(LandmarkSelection::new("gate-a", 1, 2).unwrap()),
        )
        .unwrap();
        assert_ne!(
            friend_only.context_id(actor, event),
            landmark.context_id(actor, event)
        );
        assert_eq!(
            friend_only.context_id(actor, event),
            friend_only.context_id(actor, event)
        );
        assert_ne!(
            friend_only.context_id(actor, event),
            friend_only.context_id(actor, Uuid::from_u128(10))
        );
    }

    #[test]
    fn decision_rights_and_expiry() {
        let now = Utc::now();
        let sender = Uuid::from_u128(1);
        let recipient = Uuid::from_u128(2);
        let invitation = StoredInvitation {
            id: Uuid::new_v4(),
            ordinal: 1,
            event_id: Uuid::new_v4(),
            context_id: "vm1_test".into(),
            sender_id: sender,
            recipient_id: recipient,
            selection: MeetingSelection::new(recipient, None).unwrap(),
            state: InvitationState::Pending,
            revision: 1,
            created_at: now,
            expires_at: now + chrono::Duration::minutes(5),
            purge_at: now + chrono::Duration::minutes(10),
            policy_revision: 1,
        };
        assert_eq!(
            invitation.decision_state(sender, Decision::Accept, now),
            Err(MeetingError::ResourceUnavailable)
        );
        assert_eq!(
            invitation.decision_state(recipient, Decision::Accept, now),
            Ok(InvitationState::Accepted)
        );
        assert_eq!(
            invitation.decision_state(sender, Decision::Cancel, now),
            Ok(InvitationState::Cancelled)
        );
        let expired = now + chrono::Duration::minutes(5);
        assert_eq!(
            invitation.decision_state(recipient, Decision::Accept, expired),
            Err(MeetingError::InvalidState)
        );
    }

    #[test]
    fn message_body_bounds() {
        assert!(valid_message_body("hello\nworld\t"));
        assert!(!valid_message_body("   "));
        assert!(!valid_message_body("bad\u{0007}"));
        assert!(!valid_message_body(&"a".repeat(2001)));
    }
}
