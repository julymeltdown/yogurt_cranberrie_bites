//! C05 Concert local signal application. Remote service authority remains test-only.
use concert_service::world::invalidation::*;
use uuid::Uuid;
#[test]
fn unqualified_remote_hints_never_remove_or_acknowledge_authority() {
    for owner in [RemoteOwner::Auth, RemoteOwner::Friend] {
        let hint = RemoteInvalidationHint {
            owner,
            owner_epoch: Uuid::new_v4(),
            signal_id: Uuid::new_v4(),
            user: Some(Uuid::new_v4()),
            family: None,
            pair: None,
            event: None,
            revision: Some(9),
            cursor: Some(10),
        };
        assert_eq!(
            WorldInvalidationConsumer::reject_remote(&hint),
            Err(ApplyError::UnqualifiedSource)
        );
    }
}
#[path = "world_invalidation/native.rs"]
mod native;
