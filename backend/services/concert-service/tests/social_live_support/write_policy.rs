//! Synthetic actor authority qualifies only the local phase gate and projection.
//! No actual canonical scope, operator capability or HTTP runtime is fabricated.
use super::*;
use chrono::Duration;
use concert_domain::{relationship::FreshAuthority, Revision};
#[path = "../social_remaining_support/owned.rs"]
mod owned;

#[test]
fn only_frozen_normal_operations_receive_the_readonly_gate() {
    let operations: Vec<Value> = serde_json::from_slice(include_bytes!(
        "../../../../../contracts/domain/v1/operations.json"
    ))
    .unwrap();
    let subject = Uuid::new_v4();
    let registered = [
        SocialOperation::Handles,
        SocialOperation::SetHandle(Platform::Instagram),
        SocialOperation::RemoveHandle(Platform::Instagram),
        SocialOperation::Create,
        SocialOperation::Requests,
        SocialOperation::Request(subject),
        SocialOperation::Decide(subject),
        SocialOperation::Cancel(subject),
        SocialOperation::Revoke(subject),
        SocialOperation::Reveal(subject),
        SocialOperation::Blocks,
        SocialOperation::Block(subject),
        SocialOperation::Unblock(subject),
    ];
    for operation in registered {
        let frozen = operations
            .iter()
            .find(|r| r["operationId"] == operation.id())
            .unwrap();
        assert_eq!(
            normal_mutation(operation),
            frozen["write_policy"] == "NORMAL",
            "{}",
            operation.id()
        );
    }
}
#[test]
fn generic_denials_and_transition_errors_never_gain_a_readonly_reason() {
    let gate = ReadOnlyGate::default();
    assert_eq!(
        response_error(&command_rejection(CommandError::Denied, &gate)),
        (404, "RESOURCE_UNAVAILABLE")
    );
    gate.0.store(true, Ordering::Release);
    assert_eq!(
        response_error(&command_rejection(CommandError::Denied, &gate)),
        (409, "EVENT_READ_ONLY")
    );
    assert_eq!(
        response_error(&command_rejection(CommandError::Conflict, &gate)),
        (409, "IDEMPOTENCY_CONFLICT")
    );
    assert_eq!(
        response_error(&command_rejection(CommandError::Unavailable, &gate)),
        (503, "DEPENDENCY_UNAVAILABLE")
    );
    assert_eq!(
        response_error(&Status::failed_precondition("Event is read-only")),
        (409, "INVALID_TRANSITION")
    );
}
#[derive(Clone, Copy)]
enum FixtureAuthority {
    Allow,
    Deny,
    Unavailable,
}
#[async_trait::async_trait]
impl FencedRelationshipAuthority for FixtureAuthority {
    async fn check_in(
        &self,
        _: &mut PgConnection,
        query: PolicyQuery,
        now: DateTime<Utc>,
    ) -> Result<concert_domain::relationship::FreshAuthority, RelationshipError> {
        match self {
            Self::Deny => return Err(RelationshipError::Denied),
            Self::Unavailable => return Err(RelationshipError::Unavailable),
            Self::Allow => {}
        }
        assert_eq!(query.actor, query.subject);
        assert_eq!(query.field, Field::SafetySelf);
        Ok(FreshAuthority {
            query,
            checked_at: now,
            valid_until: now + Duration::seconds(5),
            revision: Revision::initial(),
            actor_allowed: true,
            subject_allowed: true,
            inherited_block: false,
        })
    }
}
async fn phase_change_is_excluded(pool: &PgPool, event: Uuid) {
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("set local lock_timeout='100ms'")
        .execute(&mut *tx)
        .await
        .unwrap();
    let error = sqlx::query(
        "update concert_events set phase='READ_ONLY',revision=revision+1 where event_id=$1",
    )
    .bind(event)
    .execute(&mut *tx)
    .await
    .unwrap_err();
    assert_eq!(
        error.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("55P03")
    );
    tx.rollback().await.unwrap();
}
#[tokio::test]
#[ignore = "owned social_live native runner required"]
async fn readonly_reason_requires_current_actor_and_same_transaction_phase_lock() {
    let pool = owned::pool().await;
    let actor = ParticipantKey {
        event_id: EventId::from_uuid(Uuid::new_v4()),
        user_id: AuthUserId::from_uuid(Uuid::new_v4()),
    };
    let event = actor.event_id.uuid();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Read-only gate','READ_ONLY')")
        .bind(event).bind(event.to_string()).execute(&pool).await.unwrap();
    sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)")
        .bind(event).bind(actor.user_id.uuid()).execute(&pool).await.unwrap();
    for authority in [
        FixtureAuthority::Allow,
        FixtureAuthority::Deny,
        FixtureAuthority::Unavailable,
    ] {
        let policy = PgRelationshipPolicy::new(pool.clone(), authority);
        let gate = ReadOnlyGate::default();
        let mut tx = pool.begin().await.unwrap();
        let result = gate
            .check_in(
                &mut tx,
                &policy,
                actor,
                SocialOperation::SetHandle(Platform::X),
            )
            .await;
        assert!(result.is_err());
        assert_eq!(
            gate.rejected(),
            matches!(authority, FixtureAuthority::Allow)
        );
        if gate.rejected() {
            assert_eq!(
                response_error(&command_rejection(CommandError::Denied, &gate)),
                (409, "EVENT_READ_ONLY")
            );
            phase_change_is_excluded(&pool, event).await;
        }
        tx.rollback().await.unwrap();
    }
    let policy = PgRelationshipPolicy::new(pool.clone(), FixtureAuthority::Allow);
    let gate = ReadOnlyGate::default();
    let outsider = ParticipantKey {
        user_id: AuthUserId::from_uuid(Uuid::new_v4()),
        ..actor
    };
    let mut tx = pool.begin().await.unwrap();
    assert!(gate
        .check_in(&mut tx, &policy, outsider, SocialOperation::Create)
        .await
        .is_err());
    assert!(!gate.rejected());
    tx.rollback().await.unwrap();
    // Existing reads and privacy-restricting actions bypass only this NORMAL gate;
    // their retained policy/target authorization is still mandatory downstream.
    let mut tx = pool.begin().await.unwrap();
    for operation in [
        SocialOperation::Reveal(Uuid::new_v4()),
        SocialOperation::RemoveHandle(Platform::X),
        SocialOperation::Revoke(Uuid::new_v4()),
        SocialOperation::Block(Uuid::new_v4()),
        SocialOperation::Unblock(Uuid::new_v4()),
    ] {
        gate.check_in(&mut tx, &policy, actor, operation)
            .await
            .unwrap();
    }
    tx.rollback().await.unwrap();
    sqlx::query("update concert_events set phase='LIVE',revision=revision+1 where event_id=$1")
        .bind(event)
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    gate.check_in(&mut tx, &policy, actor, SocialOperation::Create)
        .await
        .unwrap();
    assert!(!gate.rejected());
    phase_change_is_excluded(&pool, event).await;
    tx.rollback().await.unwrap();
    for table in [
        "concert_command_receipts",
        "concert_outbox",
        "concert_social_effect_witnesses",
    ] {
        let count: i64 =
            sqlx::query_scalar(&format!("select count(*) from {table} where event_id=$1"))
                .bind(event)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            count, 0,
            "phase rejection must not fabricate receipts/effects"
        );
    }
    pool.close().await;
}
