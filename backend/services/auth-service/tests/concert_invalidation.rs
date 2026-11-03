use auth_service::concert_invalidation::*;
use uuid::Uuid;

#[test]
fn hints_are_exactly_user_and_family_bound_and_redacted() {
    let user = Uuid::new_v4();
    let family = Uuid::new_v4();
    let mut s = InvalidationSignal {
        cursor: ReplayCursor::new(Uuid::new_v4(), 1).unwrap(),
        signal_id: Uuid::new_v4(),
        user_id: user,
        family_id: Some(family),
        kind: InvalidationKind::SessionChanged,
    };
    assert!(s.affects(user, family));
    assert!(!s.affects(Uuid::new_v4(), family));
    assert!(!s.affects(user, Uuid::new_v4()));
    s.family_id = None;
    s.kind = InvalidationKind::CredentialsChanged;
    assert!(s.affects(user, Uuid::new_v4()));
    assert!(!s.affects(Uuid::new_v4(), family));
    let value = serde_json::to_value(&s).unwrap();
    let keys: std::collections::BTreeSet<_> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        std::collections::BTreeSet::from(["cursor", "signalId", "userId", "familyId", "kind"])
    );
    assert!(!format!("{s:?}").contains(&user.to_string()));
    assert_eq!(value["kind"], "CREDENTIALS_CHANGED");
}

#[tokio::test]
async fn malformed_cursors_limits_and_deadlines_fail_before_database_access() {
    let epoch = Uuid::new_v4();
    for (owner, after) in [(Uuid::nil(), 0), (epoch, -1), (epoch, MAX_CURSOR + 1)] {
        assert_eq!(
            ReplayCursor::new(owner, after),
            Err(ReplayError::InvalidCursor)
        );
    }
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgresql://invalid/unused")
        .unwrap();
    assert!(PgInvalidationJournal::new(pool.clone(), std::time::Duration::ZERO).is_err());
    assert!(PgInvalidationJournal::new(pool.clone(), std::time::Duration::from_secs(6)).is_err());
    let journal = PgInvalidationJournal::new(pool, std::time::Duration::from_millis(50)).unwrap();
    let cursor = ReplayCursor::new(epoch, 0).unwrap();
    for limit in [0, 257, u16::MAX] {
        assert_eq!(
            journal.replay(cursor, limit).await,
            Err(ReplayError::InvalidBatchSize)
        );
    }
    assert_eq!(
        journal
            .replay(
                ReplayCursor {
                    owner_epoch: Uuid::nil(),
                    after: 0
                },
                1
            )
            .await,
        Err(ReplayError::InvalidCursor)
    );
}

#[path = "concert_invalidation_support/native.rs"]
mod native;
