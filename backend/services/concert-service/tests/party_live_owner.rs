//! Default-deny integration and strict owner/profile projection boundaries.
use chrono::{Duration, Utc};
use concert_service::party_live::{
    PartyLiveBinding, PartyLiveError, PartyLiveOwnerFence, PartyLiveOwnerFuture,
    PartyLiveOwnerGuard, PartyLiveOwnerWork, UnavailablePartyLiveOwnerFence,
};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use std::sync::Arc;
use uuid::Uuid;

fn deadline_value() -> AuthorityRpcDeadline {
    AuthorityRpcDeadline::new(
        Utc::now().timestamp_micros() + Duration::seconds(2).num_microseconds().unwrap(),
        std::time::Duration::from_secs(3),
    )
    .unwrap()
}

fn work() -> PartyLiveOwnerWork {
    Box::new(|_: Arc<dyn PartyLiveOwnerGuard>| {
        Box::pin(async { Err::<_, PartyLiveError>(PartyLiveError::Internal) })
            as PartyLiveOwnerFuture
    })
}

#[test]
fn binding_requires_one_exact_protected_parent_digest() {
    let without_parent = PartyLiveBindingTest::make();
    assert_eq!(
        without_parent.parent_digest(),
        Err(PartyLiveError::DependencyUnavailable)
    );
    let with_parent = PartyLiveBinding::test_binding(
        without_parent.actor(),
        without_parent.event(),
        without_parent.session_family(),
        without_parent.request_id(),
        without_parent.deadline(),
        "listConcertParties",
        "GET",
        format!("/api/concert/events/{}/parties", without_parent.event()),
        &[],
        None,
        Some([7; 32]),
    );
    assert_eq!(with_parent.parent_digest(), Ok([7; 32]));
    assert_ne!(without_parent.digest(), with_parent.digest());
}

#[tokio::test]
async fn unavailable_owner_never_runs_work_or_becomes_empty_success() {
    let binding = Arc::new(PartyLiveBindingTest::make());
    assert_eq!(
        UnavailablePartyLiveOwnerFence.run(binding, work()).await,
        Err(PartyLiveError::DependencyUnavailable)
    );
}

#[test]
fn owner_binding_digest_covers_exact_request_context_and_redacts_values() {
    let base = PartyLiveBindingTest::make().test_parent([9; 32]);
    let digest = base.digest();
    assert_eq!(format!("{base:?}"), "PartyLiveBinding([REDACTED])");
    for changed in [
        PartyLiveBindingTest::alternate(false, false, false, false, false).test_parent([9; 32]),
        PartyLiveBindingTest::alternate(true, false, false, false, false).test_parent([9; 32]),
        PartyLiveBindingTest::alternate(false, true, false, false, false).test_parent([9; 32]),
        PartyLiveBindingTest::alternate(false, false, true, false, false).test_parent([9; 32]),
        PartyLiveBindingTest::alternate(false, false, false, true, false).test_parent([9; 32]),
        PartyLiveBindingTest::alternate(false, false, false, false, true).test_parent([9; 32]),
    ] {
        assert_ne!(digest, changed.digest());
    }
}

#[test]
fn unknown_or_cross_event_targets_are_not_accepted_by_the_binding() {
    let binding = PartyLiveBindingTest::make();
    binding.check().unwrap();
    let foreign = format!(
        "/api/concert/events/{}/parties",
        Uuid::new_v4().hyphenated()
    );
    assert!(PartyLiveBindingTest::with_target(foreign).check().is_err());
    assert!(
        PartyLiveBindingTest::with_target("/api/concert/events/not-a-uuid/parties".into())
            .check()
            .is_err()
    );
}

struct PartyLiveBindingTest;

impl PartyLiveBindingTest {
    fn make() -> PartyLiveBinding {
        Self::alternate(false, false, false, false, false)
    }

    fn alternate(
        actor: bool,
        event: bool,
        session: bool,
        request: bool,
        deadline: bool,
    ) -> PartyLiveBinding {
        let actor = if actor {
            Uuid::from_u128(11)
        } else {
            Uuid::from_u128(1)
        };
        let event = if event {
            Uuid::from_u128(12)
        } else {
            Uuid::from_u128(2)
        };
        let session = if session {
            Uuid::from_u128(13)
        } else {
            Uuid::from_u128(3)
        };
        let request = if request {
            Uuid::from_u128(14)
        } else {
            Uuid::from_u128(4)
        };
        let request_deadline = if deadline {
            AuthorityRpcDeadline::new(
                Utc::now().timestamp_micros() + Duration::seconds(1).num_microseconds().unwrap(),
                std::time::Duration::from_secs(2),
            )
            .unwrap()
        } else {
            deadline_value()
        };
        let target = format!("/api/concert/events/{event}/parties");
        party_live_test_binding(actor, event, session, request, request_deadline, target)
    }

    fn with_target(target: String) -> PartyLiveBinding {
        party_live_test_binding(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            Uuid::from_u128(3),
            Uuid::from_u128(4),
            deadline_value(),
            target,
        )
    }
}

fn party_live_test_binding(
    actor: Uuid,
    event: Uuid,
    session: Uuid,
    request: Uuid,
    deadline: AuthorityRpcDeadline,
    target: String,
) -> PartyLiveBinding {
    PartyLiveBinding::test_binding(
        actor,
        event,
        session,
        request,
        deadline,
        "listConcertParties",
        "GET",
        target,
        &[],
        None,
        None,
    )
}
