use crate::session::AuthAuditEvent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UserDeletionTarget {
    Profile,
    Friend,
    Post,
    Notification,
    ConcertTickets,
    ConcertSessions,
    ConcertSharing,
    ConcertPrivateData,
}

impl UserDeletionTarget {
    pub const ALL: [Self; 8] = [Self::Profile, Self::Friend, Self::Post, Self::Notification,
        Self::ConcertTickets, Self::ConcertSessions, Self::ConcertSharing, Self::ConcertPrivateData];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Profile => "PROFILE",
            Self::Friend => "FRIEND",
            Self::Post => "POST",
            Self::Notification => "NOTIFICATION",
            Self::ConcertTickets => "CONCERT_TICKETS",
            Self::ConcertSessions => "CONCERT_SESSIONS",
            Self::ConcertSharing => "CONCERT_SHARING",
            Self::ConcertPrivateData => "CONCERT_PRIVATE_DATA",
        }
    }
}

impl fmt::Display for UserDeletionTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for UserDeletionTarget {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_uppercase().as_str() {
            "PROFILE" => Ok(Self::Profile),
            "FRIEND" => Ok(Self::Friend),
            "POST" => Ok(Self::Post),
            "NOTIFICATION" => Ok(Self::Notification),
            "CONCERT_TICKETS" => Ok(Self::ConcertTickets),
            "CONCERT_SESSIONS" => Ok(Self::ConcertSessions),
            "CONCERT_SHARING" => Ok(Self::ConcertSharing),
            "CONCERT_PRIVATE_DATA" => Ok(Self::ConcertPrivateData),
            _ => anyhow::bail!("Unknown user deletion target"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UserDeletionSagaStatus {
    InProgress,
    Completed,
    DeadLettered,
}

impl UserDeletionSagaStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InProgress => "IN_PROGRESS",
            Self::Completed => "COMPLETED",
            Self::DeadLettered => "DEAD_LETTERED",
        }
    }
}

impl FromStr for UserDeletionSagaStatus {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_uppercase().as_str() {
            "IN_PROGRESS" => Ok(Self::InProgress),
            "COMPLETED" => Ok(Self::Completed),
            "DEAD_LETTERED" => Ok(Self::DeadLettered),
            _ => anyhow::bail!("Unknown user deletion saga status"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UserDeletionStepStatus {
    Pending,
    InProgress,
    Retry,
    Succeeded,
    DeadLettered,
}

impl UserDeletionStepStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::InProgress => "IN_PROGRESS",
            Self::Retry => "RETRY",
            Self::Succeeded => "SUCCEEDED",
            Self::DeadLettered => "DEAD_LETTERED",
        }
    }
}

impl FromStr for UserDeletionStepStatus {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_uppercase().as_str() {
            "PENDING" => Ok(Self::Pending),
            "IN_PROGRESS" => Ok(Self::InProgress),
            "RETRY" => Ok(Self::Retry),
            "SUCCEEDED" => Ok(Self::Succeeded),
            "DEAD_LETTERED" => Ok(Self::DeadLettered),
            _ => anyhow::bail!("Unknown user deletion step status"),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct BeginUserDeletion {
    pub saga_id: Uuid,
    pub user_id: Uuid,
    pub actor_id: Uuid,
    pub reason: String,
    pub retention_until: DateTime<Utc>,
    pub targets: Vec<UserDeletionTarget>,
    pub audit: AuthAuditEvent,
}

/// A reviewed, administrator-authorized restart of a dead-lettered deletion.
/// The resolution identity and rationale are persisted with the state change.
#[derive(Clone, PartialEq, Eq)]
pub struct RetryUserDeletion {
    pub saga_id: Uuid,
    pub resolution_id: Uuid,
    pub actor_id: Uuid,
    pub reason: String,
    pub evidence_reference: String,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserDeletionStep {
    pub saga_id: Uuid,
    pub user_id: Uuid,
    pub actor_id: Uuid,
    pub target: UserDeletionTarget,
    pub status: UserDeletionStepStatus,
    pub attempt_count: i32,
    /// Changes on every operator-authorized retry cycle. A worker result from
    /// an older cycle is fenced out even if the same target has been reclaimed.
    pub retry_generation: i32,
    pub next_attempt_at: DateTime<Utc>,
    pub locked_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub completed_at: Option<DateTime<Utc>>,
    pub retention_until: DateTime<Utc>,
    /// Validated request context carried only across the durable worker boundary.
    /// It is deliberately omitted from serialized domain/API representations.
    #[serde(skip)]
    pub origin_traceparent: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserDeletionSaga {
    pub id: Uuid,
    pub user_id: Uuid,
    pub actor_id: Uuid,
    pub status: UserDeletionSagaStatus,
    pub retention_until: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub steps: Vec<UserDeletionStep>,
}

macro_rules! redact_debug {
    ($($type:ty),+ $(,)?) => {
        $(
            impl fmt::Debug for $type {
                fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    BeginUserDeletion,
    RetryUserDeletion,
    UserDeletionStep,
    UserDeletionSaga
);

#[cfg(test)]
mod debug_redaction_tests {
    use super::*;

    #[test]
    fn user_deletion_debug_redacts_ids_rationale_and_dependency_errors() {
        let now = Utc::now();
        let saga_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let step = UserDeletionStep {
            saga_id,
            user_id,
            actor_id,
            target: UserDeletionTarget::Profile,
            status: UserDeletionStepStatus::Retry,
            attempt_count: 2,
            retry_generation: 1,
            next_attempt_at: now,
            locked_at: None,
            last_error: Some("private-downstream-error-41f7".into()),
            completed_at: None,
            retention_until: now,
            origin_traceparent: Some(
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".into(),
            ),
        };
        let saga = UserDeletionSaga {
            id: saga_id,
            user_id,
            actor_id,
            status: UserDeletionSagaStatus::InProgress,
            retention_until: now,
            created_at: now,
            updated_at: now,
            completed_at: None,
            steps: vec![step],
        };
        let audit = AuthAuditEvent {
            id: Uuid::new_v4(),
            actor_id: Some(actor_id),
            subject_user_id: Some(user_id),
            action: "DELETE_USER".into(),
            outcome: "ACCEPTED".into(),
            request_id: Some("private-request-6d12".into()),
            ip_address: Some("198.51.100.17".into()),
            user_agent: Some("private-agent-196e".into()),
            metadata_json: r#"{"private":"private-audit-metadata"}"#.into(),
            occurred_at: now,
        };
        let begin = BeginUserDeletion {
            saga_id,
            user_id,
            actor_id,
            reason: "private-deletion-reason-09f4".into(),
            retention_until: now,
            targets: UserDeletionTarget::ALL.to_vec(),
            audit,
        };
        let retry = RetryUserDeletion {
            saga_id,
            resolution_id: Uuid::new_v4(),
            actor_id,
            reason: "private-operator-reason-255b".into(),
            evidence_reference: "private-evidence-reference-9db4".into(),
            occurred_at: now,
        };

        for rendered in [
            format!("{saga:?}"),
            format!("{begin:?}"),
            format!("{retry:?}"),
        ] {
            for secret in [
                &saga_id.to_string(),
                &user_id.to_string(),
                &actor_id.to_string(),
                "private-downstream-error-41f7",
                "private-request-6d12",
                "198.51.100.17",
                "private-agent-196e",
                "private-audit-metadata",
                "private-deletion-reason-09f4",
                "private-operator-reason-255b",
                "private-evidence-reference-9db4",
            ] {
                assert!(
                    !rendered.contains(secret),
                    "Debug leaked {secret}: {rendered}"
                );
            }
            assert!(rendered.contains("[REDACTED]"));
        }

        let serialized = serde_json::to_value(saga).unwrap();
        assert!(serialized.get("user_id").is_some());
        assert_eq!(
            serialized["steps"][0]["last_error"],
            "private-downstream-error-41f7"
        );
    }

    #[test]
    fn persisted_trace_context_is_not_serialized_or_debugged() {
        let now = Utc::now();
        let step = UserDeletionStep {
            saga_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            actor_id: Uuid::new_v4(),
            target: UserDeletionTarget::Profile,
            status: UserDeletionStepStatus::Pending,
            attempt_count: 0,
            retry_generation: 0,
            next_attempt_at: now,
            locked_at: None,
            last_error: None,
            completed_at: None,
            retention_until: now,
            origin_traceparent: Some(
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".into(),
            ),
        };

        let serialized = serde_json::to_value(&step).expect("serialize deletion step");
        assert!(serialized.get("origin_traceparent").is_none());
        assert!(!format!("{step:?}").contains("4bf92f3577b34da6a3ce929d0e0e4736"));
    }
}
