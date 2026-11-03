//! Pure adapter checks plus explicitly runner-owned native PG16 qualification.
use async_trait::async_trait;
use auth_application::concert_deletion::*;
use auth_application::dto::deletion_dto::AuthUserDeletionTargetDto as Target;
use auth_application::errors::{AuthError, AuthResult};
use auth_application::ports::{UserDeletionDispatcherPort, UserDeletionSagaPort};
use auth_service::concert_deletion::{ConcertDeletionDispatcher, ConcertDeletionOwners};
use chrono::{Duration, Utc};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration as StdDuration;
use uuid::Uuid;

#[derive(Clone)]
struct Legacy;
#[async_trait]
impl UserDeletionDispatcherPort for Legacy {
    async fn apply(
        &self,
        target: Target,
        _: Uuid,
        _: Uuid,
        _: Uuid,
        _: chrono::DateTime<Utc>,
    ) -> AuthResult<()> {
        match target {
            Target::Profile | Target::Friend | Target::Post | Target::Notification => Ok(()),
            _ => panic!("Concert target escaped to legacy dispatcher"),
        }
    }
}

#[derive(Default)]
struct Owner {
    calls: Mutex<HashMap<ConcertDeletionKey, usize>>,
    effects: Mutex<HashMap<ConcertDeletionKey, ConcertDeletionCommand>>,
    fail_sharing: Mutex<bool>,
    invalid_ack: bool,
    never_reply: bool,
    lose_first_reply: AtomicBool,
}
impl Owner {
    async fn apply_command(
        &self,
        command: ConcertDeletionCommand,
    ) -> Result<ConcertDeletionAcknowledgement, ConcertDeletionFailure> {
        if self.never_reply {
            return std::future::pending().await;
        }
        *self.calls.lock().unwrap().entry(command.key).or_default() += 1;
        if command.key.target == ConcertDeletionTarget::Sharing
            && *self.fail_sharing.lock().unwrap()
        {
            return Err(ConcertDeletionFailure::Unavailable);
        }
        {
            let mut effects = self.effects.lock().unwrap();
            if let Some(previous) = effects.get(&command.key) {
                if previous != &command {
                    return Err(ConcertDeletionFailure::InvalidCommand);
                }
            }
            effects.insert(command.key, command.clone());
        }
        if self.lose_first_reply.swap(false, Ordering::SeqCst) {
            return std::future::pending().await;
        }
        Ok(ConcertDeletionAcknowledgement {
            command,
            fence_revision: if self.invalid_ack { 0 } else { 1 },
        })
    }
}

#[tokio::test]
async fn timeout_after_owner_commit_retries_the_identical_operation() {
    let owner = Arc::new(Owner {
        lose_first_reply: AtomicBool::new(true),
        ..Default::default()
    });
    let d =
        ConcertDeletionDispatcher::new(Legacy, owners(owner.clone()), StdDuration::from_millis(10))
            .unwrap();
    let c = command(ConcertDeletionTarget::Sessions);
    assert_eq!(
        d.dispatch(c.clone()).await,
        Err(AuthError::DependencyUnavailable)
    );
    assert_eq!(owner.effects.lock().unwrap().len(), 1);
    d.dispatch(c.clone()).await.unwrap();
    assert_eq!(owner.effects.lock().unwrap().len(), 1);
    assert_eq!(owner.calls.lock().unwrap()[&c.key], 2);
}

struct UnexpectedLegacyClient;
#[async_trait]
impl auth_service::adapters::out::grpc::user_deletion_dispatcher::UserDeletionTargetClientPort
    for UnexpectedLegacyClient
{
    async fn apply(
        &self,
        _: auth_service::adapters::out::grpc::user_deletion_dispatcher::UserDeletionDispatchRequest,
    ) -> anyhow::Result<()> {
        panic!("Concert cleanup must not route to a legacy service")
    }
}
macro_rules! owner_impl {
    ($($port:ident),+) => {$(
        #[async_trait]
        impl $port for Owner {
            async fn apply(&self, command: ConcertDeletionCommand) -> Result<ConcertDeletionAcknowledgement, ConcertDeletionFailure> {
                self.apply_command(command).await
            }
        }
    )+};
}
owner_impl!(
    ConcertTicketDeletionPort,
    ConcertSessionDeletionPort,
    ConcertSharingDeletionPort,
    ConcertPrivateDataDeletionPort
);

fn owners(owner: Arc<Owner>) -> ConcertDeletionOwners {
    ConcertDeletionOwners {
        tickets: Some(owner.clone()),
        sessions: Some(owner.clone()),
        sharing: Some(owner.clone()),
        private_data: Some(owner),
    }
}
fn command(target: ConcertDeletionTarget) -> ConcertDeletionCommand {
    ConcertDeletionCommand {
        key: ConcertDeletionKey {
            saga_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            target,
        },
        actor_id: Uuid::new_v4(),
        retention_until: Utc::now() + Duration::days(30),
    }
}

#[tokio::test]
async fn every_missing_owner_and_noncompletion_fails_closed() {
    let d = ConcertDeletionDispatcher::new(
        Legacy,
        ConcertDeletionOwners::default(),
        StdDuration::from_millis(20),
    )
    .unwrap();
    for target in [
        ConcertDeletionTarget::Tickets,
        ConcertDeletionTarget::Sessions,
        ConcertDeletionTarget::Sharing,
        ConcertDeletionTarget::PrivateData,
    ] {
        assert_eq!(
            d.dispatch(command(target)).await,
            Err(AuthError::DependencyUnavailable)
        );
    }
    for owner in [
        Owner {
            invalid_ack: true,
            ..Default::default()
        },
        Owner {
            never_reply: true,
            ..Default::default()
        },
    ] {
        let d = ConcertDeletionDispatcher::new(
            Legacy,
            owners(Arc::new(owner)),
            StdDuration::from_millis(10),
        )
        .unwrap();
        assert_eq!(
            d.dispatch(command(ConcertDeletionTarget::Tickets)).await,
            Err(AuthError::DependencyUnavailable)
        );
    }
}

#[test]
fn acknowledgements_bind_the_entire_operation_and_diagnostics_redact() {
    let c = command(ConcertDeletionTarget::Tickets);
    let receipt = ConcertDeletionAcknowledgement {
        command: c.clone(),
        fence_revision: 7,
    };
    assert!(receipt.verifies(&c));
    let mut changes = Vec::new();
    let mut v = c.clone();
    v.key.user_id = Uuid::new_v4();
    changes.push(v);
    let mut v = c.clone();
    v.key.saga_id = Uuid::new_v4();
    changes.push(v);
    let mut v = c.clone();
    v.key.target = ConcertDeletionTarget::Sharing;
    changes.push(v);
    let mut v = c.clone();
    v.actor_id = Uuid::new_v4();
    changes.push(v);
    let mut v = c.clone();
    v.retention_until += Duration::seconds(1);
    changes.push(v);
    for v in changes {
        assert!(!receipt.verifies(&v));
    }
    assert!(!format!("{c:?}{receipt:?}").contains(&c.key.user_id.to_string()));
    assert!(ConcertDeletionDispatcher::new(
        Legacy,
        ConcertDeletionOwners::default(),
        StdDuration::ZERO
    )
    .is_err());
    assert!(ConcertDeletionDispatcher::new(
        Legacy,
        ConcertDeletionOwners::default(),
        StdDuration::from_secs(6)
    )
    .is_err());
}

#[tokio::test]
async fn invalid_identity_never_reaches_owner_and_retry_key_is_stable() {
    let owner = Arc::new(Owner::default());
    let d =
        ConcertDeletionDispatcher::new(Legacy, owners(owner.clone()), StdDuration::from_millis(50))
            .unwrap();
    let mut c = command(ConcertDeletionTarget::Sharing);
    for field in 0..3 {
        let mut invalid = c.clone();
        match field {
            0 => invalid.key.user_id = Uuid::nil(),
            1 => invalid.key.saga_id = Uuid::nil(),
            _ => invalid.actor_id = Uuid::nil(),
        }
        assert!(d.dispatch(invalid).await.is_err());
    }
    assert!(owner.calls.lock().unwrap().is_empty());
    d.dispatch(c.clone()).await.unwrap();
    d.dispatch(c.clone()).await.unwrap();
    assert_eq!(owner.effects.lock().unwrap().len(), 1);
    c.retention_until += Duration::seconds(1);
    assert_eq!(d.dispatch(c).await, Err(AuthError::DependencyUnavailable));
}

#[tokio::test]
async fn all_eight_canonical_targets_route_without_legacy_fallback() {
    let owner = Arc::new(Owner::default());
    let d =
        ConcertDeletionDispatcher::new(Legacy, owners(owner.clone()), StdDuration::from_millis(50))
            .unwrap();
    for target in auth_service::domain::user_deletion::UserDeletionTarget::ALL {
        let c = command(ConcertDeletionTarget::Tickets);
        d.apply(
            target.into(),
            c.key.saga_id,
            c.key.user_id,
            c.actor_id,
            c.retention_until,
        )
        .await
        .unwrap();
    }
    assert_eq!(owner.effects.lock().unwrap().len(), 4);
    let legacy = Arc::new(UnexpectedLegacyClient);
    let production_default = auth_service::adapters::out::grpc::user_deletion_dispatcher::GrpcUserDeletionDispatcher::from_clients(legacy.clone(), legacy.clone(), legacy.clone(), legacy);
    for target in [
        Target::ConcertTickets,
        Target::ConcertSessions,
        Target::ConcertSharing,
        Target::ConcertPrivateData,
    ] {
        let c = command(ConcertDeletionTarget::Tickets);
        assert_eq!(
            production_default
                .apply(
                    target,
                    c.key.saga_id,
                    c.key.user_id,
                    c.actor_id,
                    c.retention_until
                )
                .await,
            Err(AuthError::DependencyUnavailable)
        );
    }
}

#[path = "concert_deletion_support/native.rs"]
mod native;
