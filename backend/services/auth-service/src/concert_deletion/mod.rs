//! Bounded dispatch from the existing durable Auth saga to Concert owners.
//! Production owners are intentionally injected; absence never acknowledges cleanup.

use async_trait::async_trait;
use auth_application::concert_deletion::*;
use auth_application::dto::deletion_dto::AuthUserDeletionTargetDto;
use auth_application::errors::{AuthError, AuthResult};
use auth_application::ports::UserDeletionDispatcherPort;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// Independent owners can be connected as they become available. A missing
/// owner is retriable through the canonical worker, never a successful no-op.
#[derive(Clone, Default)]
pub struct ConcertDeletionOwners {
    pub tickets: Option<Arc<dyn ConcertTicketDeletionPort>>,
    pub sessions: Option<Arc<dyn ConcertSessionDeletionPort>>,
    pub sharing: Option<Arc<dyn ConcertSharingDeletionPort>>,
    pub private_data: Option<Arc<dyn ConcertPrivateDataDeletionPort>>,
}

#[derive(Clone)]
pub struct ConcertDeletionDispatcher<D> {
    inherited: D,
    owners: ConcertDeletionOwners,
    deadline: Duration,
}

impl<D> ConcertDeletionDispatcher<D> {
    pub fn new(
        inherited: D,
        owners: ConcertDeletionOwners,
        deadline: Duration,
    ) -> AuthResult<Self> {
        if deadline.is_zero() || deadline > Duration::from_secs(5) {
            return Err(AuthError::InvalidInput(
                "Concert deletion deadline must be within five seconds",
            ));
        }
        Ok(Self {
            inherited,
            owners,
            deadline,
        })
    }

    pub async fn dispatch(&self, command: ConcertDeletionCommand) -> AuthResult<()> {
        let now = std::time::Instant::now();
        let original = platform::call_deadline::current()
            .map_or(now + self.deadline, |parent| parent.min(now + self.deadline));
        if original <= now {
            return Err(AuthError::DependencyUnavailable);
        }
        command
            .validate()
            .map_err(|_| AuthError::InvalidInput("Invalid Concert deletion command"))?;
        let apply = async {
            match command.key.target {
                ConcertDeletionTarget::Tickets => {
                    self.owners
                        .tickets
                        .as_ref()
                        .ok_or(ConcertDeletionFailure::Unavailable)?
                        .apply(command.clone())
                        .await
                }
                ConcertDeletionTarget::Sessions => {
                    self.owners
                        .sessions
                        .as_ref()
                        .ok_or(ConcertDeletionFailure::Unavailable)?
                        .apply(command.clone())
                        .await
                }
                ConcertDeletionTarget::Sharing => {
                    self.owners
                        .sharing
                        .as_ref()
                        .ok_or(ConcertDeletionFailure::Unavailable)?
                        .apply(command.clone())
                        .await
                }
                ConcertDeletionTarget::PrivateData => {
                    self.owners
                        .private_data
                        .as_ref()
                        .ok_or(ConcertDeletionFailure::Unavailable)?
                        .apply(command.clone())
                        .await
                }
            }
        };
        let receipt = tokio::time::timeout_at(
            tokio::time::Instant::from_std(original),
            platform::call_deadline::scope(original, apply),
        )
        .await
        .map_err(|_| AuthError::DependencyUnavailable)?
        .map_err(|_| AuthError::DependencyUnavailable)?;
        if std::time::Instant::now() >= original || !receipt.verifies(&command) {
            return Err(AuthError::DependencyUnavailable);
        }
        Ok(())
    }
}

#[async_trait]
impl<D: UserDeletionDispatcherPort> UserDeletionDispatcherPort for ConcertDeletionDispatcher<D> {
    async fn apply(
        &self,
        target: AuthUserDeletionTargetDto,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> AuthResult<()> {
        let target = match target {
            AuthUserDeletionTargetDto::ConcertTickets => ConcertDeletionTarget::Tickets,
            AuthUserDeletionTargetDto::ConcertSessions => ConcertDeletionTarget::Sessions,
            AuthUserDeletionTargetDto::ConcertSharing => ConcertDeletionTarget::Sharing,
            AuthUserDeletionTargetDto::ConcertPrivateData => ConcertDeletionTarget::PrivateData,
            inherited => {
                return self
                    .inherited
                    .apply(inherited, saga_id, user_id, actor_id, retention_until)
                    .await
            }
        };
        self.dispatch(ConcertDeletionCommand {
            key: ConcertDeletionKey {
                saga_id,
                user_id,
                target,
            },
            actor_id,
            retention_until,
        })
        .await
    }
}
