//! Durable, owner-side evidence for follow and unfollow mutations.
use super::friend_dto::FollowActionDto;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Formatter};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PurgeExpiredMutationOutcomesDto {
    pub batch_size: i32,
}

#[derive(Clone, PartialEq, Eq)]
pub struct MutationContextDto {
    pub operation_id: Uuid,
    pub actor_id: Uuid,
    pub idempotency_key: Option<String>,
    /// Set only by the private lookup RPC, never from incoming metadata.
    pub lookup_only: bool,
}

impl Debug for MutationContextDto {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MutationContextDto")
            .field("operation_id", &"[REDACTED]")
            .field("actor_id", &"[REDACTED]")
            .field("idempotency_key", &"[REDACTED]")
            .field("lookup_only", &self.lookup_only)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutationKind {
    Follow,
    Unfollow,
}

impl MutationKind {
    pub fn scope(self) -> &'static str {
        match self {
            Self::Follow => "friend.v1.FollowUser",
            Self::Unfollow => "friend.v1.UnfollowUser",
        }
    }
}

#[derive(Clone)]
pub struct MutationOperationDto {
    pub context: MutationContextDto,
    pub kind: MutationKind,
    /// Canonical command omits generated IDs and timestamps.
    pub canonical_command: String,
}

impl Debug for MutationOperationDto {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MutationOperationDto")
            .field("context", &self.context)
            .field("kind", &self.kind)
            .field("canonical_command", &"[REDACTED]")
            .finish()
    }
}

impl MutationOperationDto {
    pub fn new(
        context: MutationContextDto,
        actor_id: Uuid,
        followee_id: Uuid,
        kind: MutationKind,
    ) -> anyhow::Result<Self> {
        if context.operation_id.is_nil()
            || actor_id.is_nil()
            || followee_id.is_nil()
            || context.actor_id != actor_id
        {
            return Err(MutationError::InvalidContext.into());
        }
        if context.idempotency_key.as_ref().is_some_and(|key| {
            key.is_empty() || key.len() > 128 || !key.bytes().all(|byte| byte.is_ascii_graphic())
        }) {
            return Err(MutationError::InvalidContext.into());
        }
        Ok(Self {
            context,
            kind,
            canonical_command: serde_json::json!({ "followeeId": followee_id }).to_string(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "outcome", rename_all = "snake_case")]
pub enum MutationOutcomeDto {
    Follow(FollowActionDto),
    Unfollow(FollowActionDto),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutationReceiptStateDto {
    Replayable,
    Expired,
    Incomplete,
}

#[derive(Clone, PartialEq, Eq)]
pub struct MutationReceiptStatusDto {
    pub operation_id: Uuid,
    pub actor_id: Uuid,
    pub scope: String,
    pub committed_at: chrono::DateTime<chrono::Utc>,
    pub replay_until: chrono::DateTime<chrono::Utc>,
    pub state: MutationReceiptStateDto,
}

impl Debug for MutationReceiptStatusDto {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MutationReceiptStatusDto")
            .field("operation_id", &"[REDACTED]")
            .field("actor_id", &"[REDACTED]")
            .field("scope", &self.scope)
            .field("committed_at", &self.committed_at)
            .field("replay_until", &self.replay_until)
            .field("state", &self.state)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutationError {
    InvalidContext,
    Conflict,
    Expired,
    Unavailable,
    NotCommitted,
}

impl std::fmt::Display for MutationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidContext => "Invalid mutation context",
            Self::Conflict => "Operation identity or command conflicts with its durable receipt",
            Self::Expired => "Operation replay window expired; do not redispatch",
            Self::Unavailable => "Operation receipt unavailable; reconciliation required",
            Self::NotCommitted => "No committed owner receipt; operation outcome remains unknown",
        })
    }
}

impl std::error::Error for MutationError {}

#[cfg(test)]
mod secret_debug_tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn follow_mutation_debug_redacts_identity_key_and_target_identity() {
        let actor_id = Uuid::new_v4();
        let operation_id = Uuid::new_v4();
        let followee_id = Uuid::new_v4();
        let operation = MutationOperationDto::new(
            MutationContextDto {
                operation_id,
                actor_id,
                idempotency_key: Some("private-friend-idempotency-key".into()),
                lookup_only: false,
            },
            actor_id,
            followee_id,
            MutationKind::Follow,
        )
        .expect("valid follow mutation");

        let rendered = format!("{operation:?}");
        for private_value in [
            operation_id.to_string(),
            actor_id.to_string(),
            followee_id.to_string(),
            "private-friend-idempotency-key".to_owned(),
        ] {
            assert!(!rendered.contains(&private_value), "{rendered}");
        }
        assert!(rendered.contains("Follow"), "{rendered}");
        assert!(rendered.contains("[REDACTED]"), "{rendered}");

        let committed_at = Utc::now();
        let status = MutationReceiptStatusDto {
            operation_id,
            actor_id,
            scope: MutationKind::Follow.scope().into(),
            committed_at,
            replay_until: committed_at + Duration::hours(1),
            state: MutationReceiptStateDto::Replayable,
        };
        let rendered_status = format!("{status:?}");
        assert!(!rendered_status.contains(&operation_id.to_string()));
        assert!(!rendered_status.contains(&actor_id.to_string()));
        assert!(rendered_status.contains("Replayable"));
        assert!(rendered_status.contains("[REDACTED]"));
    }
}
