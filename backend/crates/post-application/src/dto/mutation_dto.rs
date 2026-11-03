//! Owner-side receipts are business outcomes, not cached transport responses.
use chrono::{DateTime, Utc};
use post_domain::{
    comment::Comment,
    post::{Post, StoredEvent},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq)]
pub struct MutationContextDto {
    pub operation_id: Uuid,
    pub actor_id: Uuid,
    pub idempotency_key: Option<String>,
    pub lookup_only: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutationKind {
    CreatePost,
    CreateComment,
    DeletePost,
    LikePost,
    UnlikePost,
}

impl MutationKind {
    pub fn scope(self) -> &'static str {
        match self {
            Self::CreatePost => "post.v1.CreatePost",
            Self::CreateComment => "post.v1.CreateComment",
            Self::DeletePost => "post.v1.DeletePost",
            Self::LikePost => "post.v1.LikePost",
            Self::UnlikePost => "post.v1.UnlikePost",
        }
    }
}

#[derive(Clone)]
pub struct MutationOperationDto {
    pub context: MutationContextDto,
    pub kind: MutationKind,
    /// Canonical business input, excluding generated IDs, times, and audit headers.
    /// Only its SHA-256 digest is retained in the owner database.
    pub canonical_command: String,
}

impl MutationOperationDto {
    pub fn new(
        context: MutationContextDto,
        actor_id: Uuid,
        kind: MutationKind,
        command: serde_json::Value,
    ) -> anyhow::Result<Self> {
        if context.actor_id != actor_id || context.operation_id.is_nil() || actor_id.is_nil() {
            return Err(MutationError::InvalidContext.into());
        }
        if context.idempotency_key.as_ref().is_some_and(|key| {
            key.is_empty() || key.len() > 128 || !key.bytes().all(|b| b.is_ascii_graphic())
        }) {
            return Err(MutationError::InvalidContext.into());
        }
        Ok(Self {
            context,
            kind,
            canonical_command: command.to_string(),
        })
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "outcome", rename_all = "snake_case")]
pub enum MutationOutcomeDto {
    Post(Post, StoredEvent),
    Comment(Comment, StoredEvent),
    Deleted {
        post_id: Uuid,
    },
    Like {
        post_id: Uuid,
        changed: bool,
        like_count: i64,
        liked: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutationReceiptStateDto {
    Replayable,
    Expired,
    Incomplete,
}

/// A minimized, ID-only view of an owner receipt for authorized reconciliation.
/// Never contains the idempotency key, fingerprint, command, or outcome body.
#[derive(Clone, PartialEq, Eq)]
pub struct MutationReceiptStatusDto {
    pub operation_id: Uuid,
    pub actor_id: Uuid,
    pub scope: String,
    pub committed_at: DateTime<Utc>,
    pub replay_until: DateTime<Utc>,
    pub state: MutationReceiptStateDto,
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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidContext => "Invalid mutation context",
            Self::Conflict => "Operation identity or command conflicts with its durable receipt",
            Self::Expired => "Operation replay window expired; do not redispatch",
            Self::Unavailable => "Operation receipt unavailable; reconciliation required",
            Self::NotCommitted => "No committed owner receipt; operation outcome remains unknown",
        })
    }
}

impl std::error::Error for MutationError {}

macro_rules! redact_debug {
    ($($type:ty),+ $(,)?) => {
        $(
            impl std::fmt::Debug for $type {
                fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    formatter
                        .debug_struct(stringify!($type))
                        .field("details", &"[REDACTED]")
                        .finish()
                }
            }
        )+
    };
}

redact_debug!(
    MutationContextDto,
    MutationOperationDto,
    MutationReceiptStatusDto
);

impl std::fmt::Debug for MutationOutcomeDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MutationOutcomeDto")
            .field("details", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod debug_redaction_tests {
    use super::*;

    #[test]
    fn mutation_debug_hides_idempotency_command_and_outcome() {
        let context = MutationContextDto {
            operation_id: Uuid::new_v4(),
            actor_id: Uuid::new_v4(),
            idempotency_key: Some("private-idempotency-key".into()),
            lookup_only: false,
        };
        let operation = MutationOperationDto {
            context,
            kind: MutationKind::CreatePost,
            canonical_command: r#"{"content":"private-canonical-post-body"}"#.into(),
        };
        let post = Post {
            id: Uuid::new_v4(),
            author_id: Uuid::new_v4(),
            content: "private-outcome-post-body".into(),
            visibility: "PRIVATE".into(),
            created_at: Utc::now(),
            comment_count: 0,
            like_count: 0,
        };
        let outcome = MutationOutcomeDto::Post(
            post,
            StoredEvent {
                event_id: Uuid::new_v4(),
                event_type: "POST_CREATED".into(),
                aggregate_id: Uuid::new_v4(),
                aggregate_version: Some(1),
                origin_traceparent: None,
                actor_id: None,
                attributes: vec![],
                payload: "private-outcome-event".into(),
                occurred_at: Utc::now(),
                created_at: Utc::now(),
                published: false,
                published_at: None,
                replay_resolution_id: None,
            },
        );

        for rendered in [format!("{operation:?}"), format!("{outcome:?}")] {
            for secret in [
                "private-idempotency-key",
                "private-canonical-post-body",
                "private-outcome-post-body",
                "private-outcome-event",
            ] {
                assert!(!rendered.contains(secret));
            }
            assert!(rendered.contains("[REDACTED]"));
        }
    }
}
