//! Internal durable-deletion transfers; serialization and policy remain at their existing boundaries.

use super::repository_dto::AuthAuditEventRecordDto;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthUserDeletionTargetDto {
    Profile,
    Friend,
    Post,
    Notification,
    ConcertTickets,
    ConcertSessions,
    ConcertSharing,
    ConcertPrivateData,
}
impl From<auth_domain::user_deletion::UserDeletionTarget> for AuthUserDeletionTargetDto {
    fn from(value: auth_domain::user_deletion::UserDeletionTarget) -> Self {
        match value {
            auth_domain::user_deletion::UserDeletionTarget::Profile => Self::Profile,
            auth_domain::user_deletion::UserDeletionTarget::Friend => Self::Friend,
            auth_domain::user_deletion::UserDeletionTarget::Post => Self::Post,
            auth_domain::user_deletion::UserDeletionTarget::Notification => Self::Notification,
            auth_domain::user_deletion::UserDeletionTarget::ConcertTickets => Self::ConcertTickets,
            auth_domain::user_deletion::UserDeletionTarget::ConcertSessions => {
                Self::ConcertSessions
            }
            auth_domain::user_deletion::UserDeletionTarget::ConcertSharing => Self::ConcertSharing,
            auth_domain::user_deletion::UserDeletionTarget::ConcertPrivateData => {
                Self::ConcertPrivateData
            }
        }
    }
}
impl From<AuthUserDeletionTargetDto> for auth_domain::user_deletion::UserDeletionTarget {
    fn from(value: AuthUserDeletionTargetDto) -> Self {
        match value {
            AuthUserDeletionTargetDto::Profile => Self::Profile,
            AuthUserDeletionTargetDto::Friend => Self::Friend,
            AuthUserDeletionTargetDto::Post => Self::Post,
            AuthUserDeletionTargetDto::Notification => Self::Notification,
            AuthUserDeletionTargetDto::ConcertTickets => Self::ConcertTickets,
            AuthUserDeletionTargetDto::ConcertSessions => Self::ConcertSessions,
            AuthUserDeletionTargetDto::ConcertSharing => Self::ConcertSharing,
            AuthUserDeletionTargetDto::ConcertPrivateData => Self::ConcertPrivateData,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthUserDeletionSagaStatusDto {
    InProgress,
    Completed,
    DeadLettered,
}
impl From<auth_domain::user_deletion::UserDeletionSagaStatus> for AuthUserDeletionSagaStatusDto {
    fn from(value: auth_domain::user_deletion::UserDeletionSagaStatus) -> Self {
        match value {
            auth_domain::user_deletion::UserDeletionSagaStatus::InProgress => Self::InProgress,
            auth_domain::user_deletion::UserDeletionSagaStatus::Completed => Self::Completed,
            auth_domain::user_deletion::UserDeletionSagaStatus::DeadLettered => Self::DeadLettered,
        }
    }
}
impl From<AuthUserDeletionSagaStatusDto> for auth_domain::user_deletion::UserDeletionSagaStatus {
    fn from(value: AuthUserDeletionSagaStatusDto) -> Self {
        match value {
            AuthUserDeletionSagaStatusDto::InProgress => Self::InProgress,
            AuthUserDeletionSagaStatusDto::Completed => Self::Completed,
            AuthUserDeletionSagaStatusDto::DeadLettered => Self::DeadLettered,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthUserDeletionStepStatusDto {
    Pending,
    InProgress,
    Retry,
    Succeeded,
    DeadLettered,
}
impl From<auth_domain::user_deletion::UserDeletionStepStatus> for AuthUserDeletionStepStatusDto {
    fn from(value: auth_domain::user_deletion::UserDeletionStepStatus) -> Self {
        match value {
            auth_domain::user_deletion::UserDeletionStepStatus::Pending => Self::Pending,
            auth_domain::user_deletion::UserDeletionStepStatus::InProgress => Self::InProgress,
            auth_domain::user_deletion::UserDeletionStepStatus::Retry => Self::Retry,
            auth_domain::user_deletion::UserDeletionStepStatus::Succeeded => Self::Succeeded,
            auth_domain::user_deletion::UserDeletionStepStatus::DeadLettered => Self::DeadLettered,
        }
    }
}
impl From<AuthUserDeletionStepStatusDto> for auth_domain::user_deletion::UserDeletionStepStatus {
    fn from(value: AuthUserDeletionStepStatusDto) -> Self {
        match value {
            AuthUserDeletionStepStatusDto::Pending => Self::Pending,
            AuthUserDeletionStepStatusDto::InProgress => Self::InProgress,
            AuthUserDeletionStepStatusDto::Retry => Self::Retry,
            AuthUserDeletionStepStatusDto::Succeeded => Self::Succeeded,
            AuthUserDeletionStepStatusDto::DeadLettered => Self::DeadLettered,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuthBeginUserDeletionDto {
    pub saga_id: Uuid,
    pub user_id: Uuid,
    pub actor_id: Uuid,
    pub reason: String,
    pub retention_until: DateTime<Utc>,
    pub targets: Vec<AuthUserDeletionTargetDto>,
    pub audit: AuthAuditEventRecordDto,
}
impl From<auth_domain::user_deletion::BeginUserDeletion> for AuthBeginUserDeletionDto {
    fn from(value: auth_domain::user_deletion::BeginUserDeletion) -> Self {
        Self {
            saga_id: value.saga_id,
            user_id: value.user_id,
            actor_id: value.actor_id,
            reason: value.reason,
            retention_until: value.retention_until,
            targets: value.targets.into_iter().map(Into::into).collect(),
            audit: value.audit.into(),
        }
    }
}
impl From<AuthBeginUserDeletionDto> for auth_domain::user_deletion::BeginUserDeletion {
    fn from(value: AuthBeginUserDeletionDto) -> Self {
        Self {
            saga_id: value.saga_id,
            user_id: value.user_id,
            actor_id: value.actor_id,
            reason: value.reason,
            retention_until: value.retention_until,
            targets: value.targets.into_iter().map(Into::into).collect(),
            audit: value.audit.into(),
        }
    }
}
impl From<&auth_domain::user_deletion::BeginUserDeletion> for AuthBeginUserDeletionDto {
    fn from(value: &auth_domain::user_deletion::BeginUserDeletion) -> Self {
        value.clone().into()
    }
}
impl std::fmt::Debug for AuthBeginUserDeletionDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthBeginUserDeletionDto")
            .field("fields", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuthRetryUserDeletionDto {
    pub saga_id: Uuid,
    pub resolution_id: Uuid,
    pub actor_id: Uuid,
    pub reason: String,
    pub evidence_reference: String,
    pub occurred_at: DateTime<Utc>,
}
impl From<auth_domain::user_deletion::RetryUserDeletion> for AuthRetryUserDeletionDto {
    fn from(value: auth_domain::user_deletion::RetryUserDeletion) -> Self {
        Self {
            saga_id: value.saga_id,
            resolution_id: value.resolution_id,
            actor_id: value.actor_id,
            reason: value.reason,
            evidence_reference: value.evidence_reference,
            occurred_at: value.occurred_at,
        }
    }
}
impl From<AuthRetryUserDeletionDto> for auth_domain::user_deletion::RetryUserDeletion {
    fn from(value: AuthRetryUserDeletionDto) -> Self {
        Self {
            saga_id: value.saga_id,
            resolution_id: value.resolution_id,
            actor_id: value.actor_id,
            reason: value.reason,
            evidence_reference: value.evidence_reference,
            occurred_at: value.occurred_at,
        }
    }
}
impl From<&auth_domain::user_deletion::RetryUserDeletion> for AuthRetryUserDeletionDto {
    fn from(value: &auth_domain::user_deletion::RetryUserDeletion) -> Self {
        value.clone().into()
    }
}
impl std::fmt::Debug for AuthRetryUserDeletionDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthRetryUserDeletionDto")
            .field("fields", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuthUserDeletionStepRecordDto {
    pub saga_id: Uuid,
    pub user_id: Uuid,
    pub actor_id: Uuid,
    pub target: AuthUserDeletionTargetDto,
    pub status: AuthUserDeletionStepStatusDto,
    pub attempt_count: i32,
    pub retry_generation: i32,
    pub next_attempt_at: DateTime<Utc>,
    pub locked_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub completed_at: Option<DateTime<Utc>>,
    pub retention_until: DateTime<Utc>,
    pub origin_traceparent: Option<String>,
}
impl From<auth_domain::user_deletion::UserDeletionStep> for AuthUserDeletionStepRecordDto {
    fn from(value: auth_domain::user_deletion::UserDeletionStep) -> Self {
        Self {
            saga_id: value.saga_id,
            user_id: value.user_id,
            actor_id: value.actor_id,
            target: value.target.into(),
            status: value.status.into(),
            attempt_count: value.attempt_count,
            retry_generation: value.retry_generation,
            next_attempt_at: value.next_attempt_at,
            locked_at: value.locked_at,
            last_error: value.last_error,
            completed_at: value.completed_at,
            retention_until: value.retention_until,
            origin_traceparent: value.origin_traceparent,
        }
    }
}
impl From<AuthUserDeletionStepRecordDto> for auth_domain::user_deletion::UserDeletionStep {
    fn from(value: AuthUserDeletionStepRecordDto) -> Self {
        Self {
            saga_id: value.saga_id,
            user_id: value.user_id,
            actor_id: value.actor_id,
            target: value.target.into(),
            status: value.status.into(),
            attempt_count: value.attempt_count,
            retry_generation: value.retry_generation,
            next_attempt_at: value.next_attempt_at,
            locked_at: value.locked_at,
            last_error: value.last_error,
            completed_at: value.completed_at,
            retention_until: value.retention_until,
            origin_traceparent: value.origin_traceparent,
        }
    }
}
impl From<&auth_domain::user_deletion::UserDeletionStep> for AuthUserDeletionStepRecordDto {
    fn from(value: &auth_domain::user_deletion::UserDeletionStep) -> Self {
        value.clone().into()
    }
}
impl std::fmt::Debug for AuthUserDeletionStepRecordDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthUserDeletionStepRecordDto")
            .field("fields", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuthUserDeletionSagaRecordDto {
    pub id: Uuid,
    pub user_id: Uuid,
    pub actor_id: Uuid,
    pub status: AuthUserDeletionSagaStatusDto,
    pub retention_until: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub steps: Vec<AuthUserDeletionStepRecordDto>,
}
impl From<auth_domain::user_deletion::UserDeletionSaga> for AuthUserDeletionSagaRecordDto {
    fn from(value: auth_domain::user_deletion::UserDeletionSaga) -> Self {
        Self {
            id: value.id,
            user_id: value.user_id,
            actor_id: value.actor_id,
            status: value.status.into(),
            retention_until: value.retention_until,
            created_at: value.created_at,
            updated_at: value.updated_at,
            completed_at: value.completed_at,
            steps: value.steps.into_iter().map(Into::into).collect(),
        }
    }
}
impl From<AuthUserDeletionSagaRecordDto> for auth_domain::user_deletion::UserDeletionSaga {
    fn from(value: AuthUserDeletionSagaRecordDto) -> Self {
        Self {
            id: value.id,
            user_id: value.user_id,
            actor_id: value.actor_id,
            status: value.status.into(),
            retention_until: value.retention_until,
            created_at: value.created_at,
            updated_at: value.updated_at,
            completed_at: value.completed_at,
            steps: value.steps.into_iter().map(Into::into).collect(),
        }
    }
}
impl From<&auth_domain::user_deletion::UserDeletionSaga> for AuthUserDeletionSagaRecordDto {
    fn from(value: &auth_domain::user_deletion::UserDeletionSaga) -> Self {
        value.clone().into()
    }
}
impl std::fmt::Debug for AuthUserDeletionSagaRecordDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthUserDeletionSagaRecordDto")
            .field("fields", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use auth_domain::session::AuthAuditEvent;
    use auth_domain::user_deletion::{
        BeginUserDeletion, RetryUserDeletion, UserDeletionSaga, UserDeletionSagaStatus,
        UserDeletionStep, UserDeletionStepStatus, UserDeletionTarget,
    };

    fn instant() -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000, 123_456_789).unwrap()
    }
    fn step() -> UserDeletionStep {
        UserDeletionStep {
            saga_id: Uuid::from_u128(17),
            user_id: Uuid::from_u128(23),
            actor_id: Uuid::from_u128(31),
            target: UserDeletionTarget::Profile,
            status: UserDeletionStepStatus::InProgress,
            attempt_count: 7,
            retry_generation: 11,
            next_attempt_at: instant() + chrono::Duration::seconds(1),
            locked_at: Some(instant() + chrono::Duration::milliseconds(2)),
            last_error: Some("private-dependency-error".into()),
            completed_at: None,
            retention_until: instant() + chrono::Duration::days(4),
            origin_traceparent: Some(
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".into(),
            ),
        }
    }
    fn audit() -> AuthAuditEvent {
        AuthAuditEvent {
            id: Uuid::from_u128(37),
            actor_id: Some(Uuid::from_u128(31)),
            subject_user_id: Some(Uuid::from_u128(23)),
            action: "private-action".into(),
            outcome: "private-outcome".into(),
            request_id: Some("private-request".into()),
            ip_address: Some("private-address".into()),
            user_agent: Some("private-agent".into()),
            metadata_json: " {\"opaque\":\"private-metadata\"} ".into(),
            occurred_at: instant(),
        }
    }
    #[test]
    fn step_transfer_preserves_every_target_state_claim_generation_and_optional_context() {
        for target in UserDeletionTarget::ALL {
            for status in [
                UserDeletionStepStatus::Pending,
                UserDeletionStepStatus::InProgress,
                UserDeletionStepStatus::Retry,
                UserDeletionStepStatus::Succeeded,
                UserDeletionStepStatus::DeadLettered,
            ] {
                for flags in 0..16 {
                    for retry_generation in [0, 1, i32::MAX] {
                        let record = UserDeletionStep {
                            target,
                            status,
                            retry_generation,
                            attempt_count: if retry_generation == i32::MAX {
                                i32::MAX
                            } else {
                                7
                            },
                            locked_at: (flags & 1 != 0)
                                .then_some(instant() + chrono::Duration::milliseconds(2)),
                            last_error: (flags & 2 != 0).then(|| " Exact Dependency Error ".into()),
                            completed_at: (flags & 4 != 0)
                                .then_some(instant() + chrono::Duration::seconds(3)),
                            origin_traceparent: (flags & 8 != 0)
                                .then(|| step().origin_traceparent.unwrap()),
                            ..step()
                        };
                        assert_eq!(
                            UserDeletionStep::from(AuthUserDeletionStepRecordDto::from(&record)),
                            record
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn saga_and_requests_preserve_nested_order_retention_operator_resolution_and_audit() {
        for status in [
            UserDeletionSagaStatus::InProgress,
            UserDeletionSagaStatus::Completed,
            UserDeletionSagaStatus::DeadLettered,
        ] {
            for completed_at in [None, Some(instant() + chrono::Duration::seconds(6))] {
                let saga = UserDeletionSaga {
                    id: step().saga_id,
                    user_id: step().user_id,
                    actor_id: step().actor_id,
                    status,
                    retention_until: instant() + chrono::Duration::days(4),
                    created_at: instant(),
                    updated_at: instant() + chrono::Duration::seconds(5),
                    completed_at,
                    steps: UserDeletionTarget::ALL
                        .into_iter()
                        .map(|target| UserDeletionStep { target, ..step() })
                        .collect(),
                };
                assert_eq!(
                    UserDeletionSaga::from(AuthUserDeletionSagaRecordDto::from(&saga)),
                    saga
                );
            }
        }
        let begin = BeginUserDeletion {
            saga_id: step().saga_id,
            user_id: step().user_id,
            actor_id: step().actor_id,
            reason: " Exact Reason ".into(),
            retention_until: instant() + chrono::Duration::days(4),
            targets: vec![
                UserDeletionTarget::Post,
                UserDeletionTarget::Profile,
                UserDeletionTarget::Post,
            ],
            audit: audit(),
        };
        assert_eq!(
            BeginUserDeletion::from(AuthBeginUserDeletionDto::from(&begin)),
            begin
        );
        let retry = RetryUserDeletion {
            saga_id: begin.saga_id,
            resolution_id: Uuid::from_u128(41),
            actor_id: begin.actor_id,
            reason: " Exact Operator Reason ".into(),
            evidence_reference: "INC-CaseSensitive/Record#7".into(),
            occurred_at: instant(),
        };
        assert_eq!(
            RetryUserDeletion::from(AuthRetryUserDeletionDto::from(&retry)),
            retry
        );
    }
    #[test]
    fn debug_redacts_nested_identity_trace_audit_rationale_and_dependency_data() {
        let record = step();
        let saga = UserDeletionSaga {
            id: record.saga_id,
            user_id: record.user_id,
            actor_id: record.actor_id,
            status: UserDeletionSagaStatus::InProgress,
            retention_until: instant() + chrono::Duration::days(4),
            created_at: instant(),
            updated_at: instant() + chrono::Duration::seconds(5),
            completed_at: None,
            steps: vec![record.clone()],
        };
        let begin = BeginUserDeletion {
            saga_id: record.saga_id,
            user_id: record.user_id,
            actor_id: record.actor_id,
            reason: "private-reason".into(),
            retention_until: instant() + chrono::Duration::days(4),
            targets: vec![record.target],
            audit: audit(),
        };
        let retry = RetryUserDeletion {
            saga_id: record.saga_id,
            resolution_id: Uuid::from_u128(41),
            actor_id: record.actor_id,
            reason: "private-resolution-reason".into(),
            evidence_reference: "private-evidence".into(),
            occurred_at: instant(),
        };
        for (rendered, name) in [
            (
                format!("{:?}", AuthUserDeletionStepRecordDto::from(&record)),
                "AuthUserDeletionStepRecordDto",
            ),
            (
                format!("{:?}", AuthUserDeletionSagaRecordDto::from(&saga)),
                "AuthUserDeletionSagaRecordDto",
            ),
            (
                format!("{:?}", AuthBeginUserDeletionDto::from(&begin)),
                "AuthBeginUserDeletionDto",
            ),
            (
                format!("{:?}", AuthRetryUserDeletionDto::from(&retry)),
                "AuthRetryUserDeletionDto",
            ),
        ] {
            assert_eq!(rendered, format!("{name} {{ fields: \"[REDACTED]\" }}"));
        }
    }
}
