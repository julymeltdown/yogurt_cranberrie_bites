use super::*;
use crate::adapters::out::relationship_authority::invalidation::{
    ReplayBudget, ReplayCancellation, MAX_OPERATION,
};
use std::time::{Duration, Instant};

fn id() -> String {
    Uuid::new_v4().to_string()
}
fn pos(epoch: &str, sequence: i64) -> proto::InvalidationCursor {
    proto::InvalidationCursor {
        owner_epoch: epoch.into(),
        sequence,
    }
}
fn fixture() -> (
    proto::ReadInvalidationsRequest,
    proto::ReadInvalidationsResponse,
) {
    let epoch = id();
    let query = proto::ReadInvalidationsRequest {
        consumer_id: id(),
        request_id: id(),
        after: Some(pos(&epoch, 0)),
        limit: 2,
    };
    let signal = proto::InvalidationSignal {
        cursor: Some(pos(&epoch, 1)),
        signal_id: id(),
        reason: proto::InvalidationReason::AuthSessionChanged as i32,
        scope: Some(proto::invalidation_signal::Scope::Family(
            proto::InvalidationFamilyScope {
                user_id: id(),
                session_family_id: id(),
            },
        )),
    };
    let response = proto::ReadInvalidationsResponse {
        query: Some(query.clone()),
        next: Some(pos(&epoch, 1)),
        head: Some(pos(&epoch, 1)),
        signals: vec![signal],
    };
    (query, response)
}
fn reply<T>(value: T) -> tonic::Response<T> {
    let mut r = tonic::Response::new(value);
    r.metadata_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    r
}

#[test]
fn head_requires_exact_echo_and_canonical_nonzero_position() {
    let q = proto::ReadInvalidationHeadRequest {
        consumer_id: id(),
        request_id: id(),
    };
    let valid = proto::ReadInvalidationHeadResponse {
        query: Some(q.clone()),
        head: Some(pos(&id(), 0)),
    };
    let h = head(InvalidationOwner::Auth, q.clone(), reply(valid.clone())).unwrap();
    assert_eq!(h.consumer_id().to_string(), q.consumer_id);
    assert_eq!(h.owner(), InvalidationOwner::Auth);
    assert_eq!(h.cursor().sequence(), 0);
    for kind in 0..7 {
        let mut v = valid.clone();
        match kind {
            0 => v.query = None,
            1 => v.query.as_mut().unwrap().consumer_id = id(),
            2 => v.query.as_mut().unwrap().request_id = id(),
            3 => v.head = None,
            4 => v.head.as_mut().unwrap().owner_epoch = Uuid::nil().to_string(),
            5 => v.head.as_mut().unwrap().sequence = -1,
            6 => {
                v.head.as_mut().unwrap().owner_epoch = "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA".into()
            }
            _ => unreachable!(),
        }
        assert!(
            head(InvalidationOwner::Auth, q.clone(), reply(v)).is_err(),
            "case {kind}"
        );
    }
    assert_eq!(
        head(InvalidationOwner::Auth, q, tonic::Response::new(valid)).unwrap_err(),
        ReadError::MalformedResponse
    );
}

#[test]
fn replay_rejects_gap_epoch_future_duplicate_and_scope_confusion() {
    let (q, v) = fixture();
    assert!(page(InvalidationOwner::Auth, q.clone(), reply(v.clone())).is_ok());
    for kind in 0..18 {
        let mut bad = v.clone();
        let expected = match kind {
            0 => {
                bad.query = None;
                ReadError::QueryMismatch
            }
            1 => {
                bad.query.as_mut().unwrap().limit = 1;
                ReadError::QueryMismatch
            }
            2 => {
                bad.head.as_mut().unwrap().owner_epoch = id();
                ReadError::EpochChanged
            }
            3 => {
                bad.next.as_mut().unwrap().owner_epoch = id();
                ReadError::EpochChanged
            }
            4 => {
                bad.head.as_mut().unwrap().sequence = 0;
                ReadError::FutureCursor
            }
            5 => {
                bad.next.as_mut().unwrap().sequence = 2;
                ReadError::FutureCursor
            }
            6 => {
                bad.signals[0].cursor.as_mut().unwrap().sequence = 2;
                ReadError::SequenceGap
            }
            7 => {
                bad.signals.clear();
                ReadError::SequenceGap
            }
            8 => {
                bad.signals[0].scope = None;
                ReadError::MalformedResponse
            }
            9 => {
                bad.signals[0].reason = 0;
                ReadError::MalformedResponse
            }
            10 => {
                bad.signals[0].reason = 12345;
                ReadError::MalformedResponse
            }
            11 => {
                bad.signals[0].reason = proto::InvalidationReason::FriendUserRemoved as i32;
                ReadError::OwnerMismatch
            }
            12 => {
                bad.signals[0].signal_id = Uuid::nil().to_string();
                ReadError::MalformedResponse
            }
            13 => {
                bad.signals[0].scope = Some(proto::invalidation_signal::Scope::User(
                    proto::InvalidationUserScope { user_id: id() },
                ));
                ReadError::MalformedResponse
            }
            14 => {
                bad.signals.push(bad.signals[0].clone());
                ReadError::DuplicateSignal
            }
            15 => {
                let mut duplicate = bad.signals[0].clone();
                duplicate.cursor.as_mut().unwrap().sequence = 2;
                bad.signals.push(duplicate);
                ReadError::ConflictingSignal
            }
            16 => {
                bad.signals[0].cursor.as_mut().unwrap().owner_epoch = id();
                ReadError::EpochChanged
            }
            17 => {
                bad.head.as_mut().unwrap().sequence = 3;
                ReadError::SequenceGap
            }
            _ => unreachable!(),
        };
        assert_eq!(
            page(InvalidationOwner::Auth, q.clone(), reply(bad)).unwrap_err(),
            expected,
            "case {kind}"
        );
    }
    let mut friend = v.clone();
    friend.signals[0].reason = proto::InvalidationReason::FriendRelationshipChanged as i32;
    let user = id();
    friend.signals[0].scope = Some(proto::invalidation_signal::Scope::Pair(
        proto::InvalidationPairScope {
            actor_id: user.clone(),
            subject_id: user,
        },
    ));
    assert_eq!(
        page(InvalidationOwner::Friend, q.clone(), reply(friend.clone())).unwrap_err(),
        ReadError::MalformedResponse
    );
    if let Some(proto::invalidation_signal::Scope::Pair(p)) = &mut friend.signals[0].scope {
        p.subject_id = id();
    }
    assert_eq!(
        page(InvalidationOwner::Friend, q.clone(), reply(friend.clone()))
            .unwrap()
            .signals()[0]
            .reason(),
        SignalReason::FriendRelationshipChanged
    );
    assert_eq!(
        page(InvalidationOwner::Auth, q, reply(friend)).unwrap_err(),
        ReadError::OwnerMismatch
    );
}

#[test]
fn page_boundary_redelivery_and_size_do_not_create_authority() {
    let (mut q, mut v) = fixture();
    q.limit = 1;
    v.query = Some(q.clone());
    v.head.as_mut().unwrap().sequence = 10;
    let p = page(InvalidationOwner::Auth, q.clone(), reply(v.clone())).unwrap();
    assert!(p.has_more());
    assert_eq!(p.next().sequence(), 1);
    assert_eq!(p.head().sequence(), 10);
    let same = page(InvalidationOwner::Auth, q.clone(), reply(v.clone())).unwrap();
    assert!(same.signals()[0].check_redelivery(&p.signals()[0]).is_ok());
    v.signals[0].reason = proto::InvalidationReason::AuthUserRemoved as i32;
    v.signals[0].scope = Some(proto::invalidation_signal::Scope::User(
        proto::InvalidationUserScope { user_id: id() },
    ));
    let conflict = page(InvalidationOwner::Auth, q.clone(), reply(v.clone())).unwrap();
    assert_eq!(
        conflict.signals()[0]
            .check_redelivery(&p.signals()[0])
            .unwrap_err(),
        ReadError::ConflictingSignal
    );
    v.query.as_mut().unwrap().consumer_id = "private-canary".repeat(20_000);
    assert_eq!(
        page(InvalidationOwner::Auth, q, reply(v)).unwrap_err(),
        ReadError::ResponseTooLarge
    );
    assert!(!format!("{p:?}").contains(&p.consumer_id().to_string()));
    assert!(OwnerCursor::new(InvalidationOwner::Auth, Uuid::nil(), 0).is_err());
    assert!(OwnerCursor::new(InvalidationOwner::Friend, Uuid::new_v4(), MAX_CURSOR + 1).is_err());
}

#[tokio::test]
async fn cancellation_and_shorter_parent_stop_survive_all_operation_work() {
    let cancel = ReplayCancellation::default();
    cancel.cancel();
    assert!(matches!(
        ReplayBudget::for_duration(MAX_OPERATION, cancel),
        Err(ReadError::Cancelled)
    ));
    assert!(matches!(
        ReplayBudget::for_duration(
            MAX_OPERATION + Duration::from_nanos(1),
            ReplayCancellation::default()
        ),
        Err(ReadError::InvalidInput)
    ));
    let budget = ReplayBudget::for_duration(MAX_OPERATION, ReplayCancellation::default()).unwrap();
    let parent = Instant::now() + Duration::from_millis(25);
    platform::call_deadline::scope(parent, async {
        let stop = budget.operation_stop().unwrap();
        assert!(stop <= parent);
        let result: Result<(), ReadError> = budget
            .run(stop, async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok(())
            })
            .await;
        assert_eq!(result.unwrap_err(), ReadError::DeadlineExceeded);
    })
    .await;
    let cancel = ReplayCancellation::default();
    let budget = ReplayBudget::for_duration(MAX_OPERATION, cancel.clone()).unwrap();
    let result = budget
        .run(budget.operation_stop().unwrap(), async {
            cancel.cancel();
            std::future::pending::<Result<(), ReadError>>().await
        })
        .await;
    assert_eq!(result.unwrap_err(), ReadError::Cancelled);
}
