use chrono::{Duration, TimeZone, Utc};
use concert_application::event_cleanup::*;
use concert_domain::event_lifecycle::{Retention, Step};
use uuid::Uuid;
fn request() -> BatchRequest {
    let closed_at = Utc.timestamp_opt(1800000000, 0).unwrap();
    BatchRequest::new(
        Scope {
            actor: Uuid::from_u128(1),
            event: Uuid::from_u128(2),
            job: Uuid::from_u128(3),
            step: Step::Cleanup,
            owner: Owner::PostRetention,
        },
        Retention {
            policy_revision: 7,
            closed_at,
            cleanup_after: closed_at + Duration::days(10),
        },
        0,
        None,
        2,
    )
    .unwrap()
}
fn receipt(r: &BatchRequest, n: u16) -> Receipt {
    Receipt {
        binding: r.binding(),
        operation: r.operation(),
        id: Uuid::from_u128(77 + r.ordinal() as u128),
        revision: 9,
        processed: n,
    }
}
#[test]
fn partial_failure_retries_same_operation_then_restores_exact_cursor() {
    let initial = request();
    let mut p = Checkpoint::new(initial.clone()).unwrap();
    p.failed(Failure::OwnerUnavailable).unwrap();
    assert_eq!(p.request().operation(), initial.operation());
    let r = receipt(p.request(), 2);
    p.apply(Outcome::Page {
        receipt: r,
        next: Cursor::new(vec![1, 2]).unwrap(),
    })
    .unwrap();
    assert_eq!(p.processed(), 2);
    assert_eq!(p.request().ordinal(), 1);
    let mut restored = Checkpoint::restore(
        p.request().clone(),
        p.state(),
        p.processed(),
        p.last_receipt(),
        p.failure(),
    )
    .unwrap();
    assert_ne!(restored.request().operation(), initial.operation());
    assert_eq!(restored.request().cursor().unwrap().bytes(), [1, 2]);
    let r = receipt(restored.request(), 1);
    restored.apply(Outcome::Converged { receipt: r }).unwrap();
    assert_eq!(restored.processed(), 3);
    assert_eq!(restored.state(), State::Complete);
    assert!(Checkpoint::restore(
        restored.request().clone(),
        State::Complete,
        3,
        Some(r),
        None
    )
    .is_ok());
    assert!(restored.failed(Failure::SourceChanged).is_err());
}
#[test]
fn stale_cross_owner_and_cross_policy_receipts_cannot_advance() {
    let r = request();
    let good = receipt(&r, 1);
    let mut p = Checkpoint::new(r.clone()).unwrap();
    let mut scope = r.scope();
    scope.owner = Owner::MediaRetention;
    let wrong = BatchRequest::new(scope, r.retention(), 0, None, r.limit()).unwrap();
    assert!(p
        .apply(Outcome::Converged {
            receipt: receipt(&wrong, 1)
        })
        .is_err());
    let mut fine_policy = r.retention();
    fine_policy.cleanup_after += Duration::nanoseconds(1);
    let fine = BatchRequest::new(r.scope(), fine_policy, 0, None, r.limit()).unwrap();
    assert_ne!(fine.binding(), r.binding());
    let mut policy = r.retention();
    policy.policy_revision += 1;
    let wrong = BatchRequest::new(r.scope(), policy, 0, None, r.limit()).unwrap();
    assert!(p
        .apply(Outcome::Converged {
            receipt: receipt(&wrong, 1)
        })
        .is_err());
    p.apply(Outcome::Page {
        receipt: good,
        next: Cursor::new(vec![2]).unwrap(),
    })
    .unwrap();
    assert!(p.apply(Outcome::Converged { receipt: good }).is_err());
    assert_eq!(p.processed(), 1);
}
#[test]
fn empty_or_repeated_pages_and_forged_restore_fail_without_mutation() {
    let r = request();
    let mut p = Checkpoint::new(r).unwrap();
    assert!(p
        .apply(Outcome::Page {
            receipt: receipt(p.request(), 0),
            next: Cursor::new(vec![1]).unwrap()
        })
        .is_err());
    assert_eq!(p.processed(), 0);
    p.apply(Outcome::Page {
        receipt: receipt(p.request(), 1),
        next: Cursor::new(vec![1]).unwrap(),
    })
    .unwrap();
    let op = p.request().operation();
    assert!(p
        .apply(Outcome::Page {
            receipt: receipt(p.request(), 1),
            next: Cursor::new(vec![1]).unwrap()
        })
        .is_err());
    assert_eq!(p.request().operation(), op);
    assert!(Checkpoint::new(p.request().clone()).is_err());
    assert!(Checkpoint::restore(p.request().clone(), State::Pending, 0, None, None).is_err());
    assert!(Checkpoint::restore(
        p.request().clone(),
        State::Complete,
        1,
        p.last_receipt(),
        None
    )
    .is_err());
}
#[test]
fn exact_policy_deadline_and_closed_owner_set() {
    let r = request();
    assert!(!r.due(r.retention().cleanup_after - Duration::microseconds(1)));
    assert!(r.due(r.retention().cleanup_after));
    let mut scope = r.scope();
    scope.step = Step::ReadOnly;
    assert!(scope.validate().is_err());
    scope.step = Step::ExpireGrants;
    assert!(scope.validate().is_err());
    assert!(Cursor::new(vec![]).is_err());
    assert!(Cursor::new(vec![0; MAX_CURSOR_BYTES + 1]).is_err());
    assert_eq!(
        required_owners(Step::ExpireBeacons),
        [Owner::Beacons, Owner::World, Owner::Community]
    );
}
