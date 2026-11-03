//! Durable owner evidence for Profile mutations.
use crate::dto::profile_dto::ProfileDto;
use chrono::{DateTime, Utc};
use profile_domain::profile::AccountVisibility;
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
    /// Installed only by the dedicated lookup RPC.
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

#[derive(Clone)]
pub struct ProfileMutationOperationDto {
    pub context: MutationContextDto,
    pub(crate) scope: &'static str,
    /// Canonical business input; generated IDs and timestamps are excluded.
    pub canonical_command: String,
}

impl Debug for ProfileMutationOperationDto {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProfileMutationOperationDto")
            .field("context", &self.context)
            .field("scope", &self.scope)
            .field("canonical_command", &"[REDACTED]")
            .field("canonical_command_len", &self.canonical_command.len())
            .finish()
    }
}

impl ProfileMutationOperationDto {
    pub const SCOPE: &'static str = "profile.v1.UpdateProfilePrivacy";
    pub const PROFILE_EDIT_SCOPE: &'static str = "profile.v1.UpdateProfile";

    /// Read the validated operation scope without permitting adapters to change it.
    pub fn scope(&self) -> &'static str {
        self.scope
    }

    pub fn new(
        context: MutationContextDto,
        user_id: Uuid,
        visibility: AccountVisibility,
    ) -> anyhow::Result<Self> {
        Self::build(
            context,
            user_id,
            Self::SCOPE,
            serde_json::json!({
                "userId": user_id,
                "accountVisibility": visibility.as_str(),
            })
            .to_string(),
        )
    }

    pub fn profile_edit(
        context: MutationContextDto,
        user_id: Uuid,
        username: &str,
        nickname: &str,
        avatar_url: &str,
        bio: &str,
    ) -> anyhow::Result<Self> {
        Self::build(
            context,
            user_id,
            Self::PROFILE_EDIT_SCOPE,
            serde_json::json!({
                "userId": user_id,
                "username": username,
                "nickname": nickname,
                "avatarUrl": avatar_url,
                "bio": bio,
            })
            .to_string(),
        )
    }

    fn build(
        context: MutationContextDto,
        user_id: Uuid,
        scope: &'static str,
        canonical_command: String,
    ) -> anyhow::Result<Self> {
        if context.operation_id.is_nil()
            || user_id.is_nil()
            || context.actor_id != user_id
            || context.idempotency_key.as_ref().is_some_and(|key| {
                key.is_empty()
                    || key.len() > 128
                    || !key.bytes().all(|byte| byte.is_ascii_graphic())
            })
        {
            return Err(MutationError::InvalidContext.into());
        }
        Ok(Self {
            context,
            scope,
            canonical_command,
        })
    }
}

/// Backwards-compatible name for the original privacy mutation operation.
pub type PrivacyMutationOperationDto = ProfileMutationOperationDto;

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
    pub committed_at: DateTime<Utc>,
    pub replay_until: DateTime<Utc>,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MutationOutcomeDto {
    PrivacyUpdated(ProfileDto),
}

#[cfg(test)]
mod secret_debug_tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn profile_mutation_debug_redacts_identity_key_and_canonical_profile_data() {
        let actor_id = Uuid::new_v4();
        let operation_id = Uuid::new_v4();
        let context = MutationContextDto {
            operation_id,
            actor_id,
            idempotency_key: Some("private-idempotency-key-marker".into()),
            lookup_only: false,
        };
        let operation = ProfileMutationOperationDto::profile_edit(
            context,
            actor_id,
            "private-profile-username-marker",
            "private-profile-nickname-marker",
            "private-profile-avatar-marker",
            "private-profile-bio-marker",
        )
        .expect("valid profile mutation");

        let rendered = format!("{operation:?}");
        for private_value in [
            operation_id.to_string(),
            actor_id.to_string(),
            "private-idempotency-key-marker".to_owned(),
            "private-profile-username-marker".to_owned(),
            "private-profile-nickname-marker".to_owned(),
            "private-profile-avatar-marker".to_owned(),
            "private-profile-bio-marker".to_owned(),
        ] {
            assert!(!rendered.contains(&private_value), "{rendered}");
        }
        assert!(rendered.contains("profile.v1.UpdateProfile"), "{rendered}");
        assert!(rendered.contains("canonical_command_len"), "{rendered}");
        assert!(rendered.contains("[REDACTED]"), "{rendered}");
    }

    #[test]
    fn mutation_receipt_debug_redacts_operation_and_actor_identifiers() {
        let operation_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let committed_at = Utc::now();
        let status = MutationReceiptStatusDto {
            operation_id,
            actor_id,
            scope: ProfileMutationOperationDto::PROFILE_EDIT_SCOPE.into(),
            committed_at,
            replay_until: committed_at + Duration::hours(1),
            state: MutationReceiptStateDto::Replayable,
        };

        let rendered = format!("{status:?}");
        assert!(!rendered.contains(&operation_id.to_string()), "{rendered}");
        assert!(!rendered.contains(&actor_id.to_string()), "{rendered}");
        assert!(rendered.contains("Replayable"), "{rendered}");
        assert!(rendered.contains("[REDACTED]"), "{rendered}");
    }
}
