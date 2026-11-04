//! BE42 PrivateData source checks. Native PostgreSQL qualifiers live in the
//! marker-owned native test and are never substituted by these assertions.
use chrono::Duration;
use concert_service::account_private_data::*;
use concert_service::world::account_deletion::{
    DeletionFailure, VerifiedDeletionSource, WorldDeletionCommand, WorldDeletionTarget,
};
use std::time::Duration as WallDuration;
use tokio::time::Instant;
use uuid::Uuid;

fn command() -> WorldDeletionCommand {
    WorldDeletionCommand {
        saga_id: Uuid::from_u128(1),
        user_id: Uuid::from_u128(2),
        target: WorldDeletionTarget::PrivateData,
        actor_id: Uuid::from_u128(2),
        retention_until: "2070-01-01T00:00:00.123456789Z".parse().unwrap(),
    }
}
fn deadline() -> Instant {
    Instant::now() + WallDuration::from_secs(4)
}

#[test]
fn exact_command_binding_preserves_target_and_submicrosecond_retention() {
    let c = command();
    let mut other = c.clone();
    other.target = WorldDeletionTarget::Tickets;
    assert_eq!(
        c.identity_binding().unwrap(),
        other.identity_binding().unwrap()
    );
    assert_ne!(c.binding().unwrap(), other.binding().unwrap());
    other = c.clone();
    other.retention_until += Duration::nanoseconds(1);
    assert_ne!(c.binding().unwrap(), other.binding().unwrap());
    assert_eq!(format!("{c:?}"), "WorldDeletionCommand([REDACTED])");
}

#[test]
fn source_deadline_authority_and_binding_fail_closed() {
    let c = command();
    let d = deadline();
    assert!(VerifiedDeletionSource::from_verified_auth_source(&c, d, [0; 32]).is_err());
    assert!(
        VerifiedDeletionSource::from_verified_auth_source(&c, Instant::now(), [1; 32]).is_err()
    );
    assert!(VerifiedDeletionSource::from_verified_auth_source(
        &c,
        Instant::now() + WallDuration::from_secs(6),
        [1; 32]
    )
    .is_err());
    let proof = VerifiedDeletionSource::from_verified_auth_source(&c, d, [1; 32]).unwrap();
    let mut rebound = c.clone();
    rebound.actor_id = Uuid::from_u128(3);
    assert_eq!(
        proof.check(&rebound, d),
        Err(DeletionFailure::InvalidCommand)
    );
    assert_eq!(
        proof.check(&c, d + WallDuration::from_millis(1)),
        Err(DeletionFailure::InvalidCommand)
    );
}

#[test]
fn acknowledgement_is_exact_and_redacted() {
    let c = command();
    let proof = PrivateDataCoreAcknowledgement {
        command: c.clone(),
        fence_revision: 1,
        scanned_events: 3,
        neutralized_participants: 3,
        removed_avatars: 2,
    };
    assert!(proof.verifies(&c));
    let mut other = c;
    other.saga_id = Uuid::from_u128(9);
    assert!(!proof.verifies(&other));
    assert_eq!(
        format!("{proof:?}"),
        "PrivateDataCoreAcknowledgement([REDACTED])"
    );
}
