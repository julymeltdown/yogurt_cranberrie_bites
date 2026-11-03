use chrono::{DateTime, Utc};
use std::fmt::{Debug, Formatter};
use uuid::Uuid;

use crate::follow_edge::FollowEdge;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AccountVisibility {
    Public,
    Private,
}

impl Debug for AccountVisibility {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("AccountVisibility")
            .field(&"[REDACTED]")
            .finish()
    }
}

impl AccountVisibility {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "PUBLIC" => Ok(Self::Public),
            "PRIVATE" => Ok(Self::Private),
            _ => anyhow::bail!("Account visibility must be PUBLIC or PRIVATE"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FollowRequestStatus {
    Pending,
    Accepted,
    Rejected,
    Cancelled,
}

impl Debug for FollowRequestStatus {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("FollowRequestStatus")
            .field(&"[REDACTED]")
            .finish()
    }
}

impl FollowRequestStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Accepted => "ACCEPTED",
            Self::Rejected => "REJECTED",
            Self::Cancelled => "CANCELLED",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "ACCEPTED" => Self::Accepted,
            "REJECTED" => Self::Rejected,
            "CANCELLED" => Self::Cancelled,
            _ => Self::Pending,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct FollowRequest {
    pub id: Uuid,
    pub follower_id: Uuid,
    pub followee_id: Uuid,
    pub status: FollowRequestStatus,
    pub created_at: DateTime<Utc>,
    pub decided_at: Option<DateTime<Utc>>,
}

impl Debug for FollowRequest {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FollowRequest")
            .field("id", &"[REDACTED]")
            .field("follower_id", &"[REDACTED]")
            .field("followee_id", &"[REDACTED]")
            .field("status", &"[REDACTED]")
            .field("created_at", &"[REDACTED]")
            .field("decided_at", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptFollowRequestOutcome {
    Accepted {
        request: FollowRequest,
        edge: FollowEdge,
    },
    NotFound,
    Blocked,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follow_request_debug_redacts_relationship_and_decision_state() {
        let request = FollowRequest {
            id: Uuid::new_v4(),
            follower_id: Uuid::new_v4(),
            followee_id: Uuid::new_v4(),
            status: FollowRequestStatus::Pending,
            created_at: Utc::now(),
            decided_at: None,
        };

        let rendered = format!("{request:?}");
        for private_value in [
            request.id.to_string(),
            request.follower_id.to_string(),
            request.followee_id.to_string(),
            "Pending".to_owned(),
        ] {
            assert!(!rendered.contains(&private_value), "{rendered}");
        }
        assert!(rendered.contains("[REDACTED]"), "{rendered}");
    }

    #[test]
    fn account_visibility_debug_does_not_reveal_private_setting() {
        let rendered = format!("{:?}", AccountVisibility::Private);
        assert!(!rendered.contains("Private"), "{rendered}");
        assert!(rendered.contains("[REDACTED]"), "{rendered}");
    }
}
