use profile_service::adapters::out::persistence::profile_repository::PgProfileRepository;
use profile_service::adapters::r#in::messaging::mutation_receipt_retention_worker::ProfileMutationReceiptRetentionWorker;
use profile_service::application::dto::mutation_dto::{
    MutationContextDto, MutationReceiptStateDto,
};
use profile_service::application::dto::profile_dto::{UpdateProfileDto, UpdateProfilePrivacyDto};
use profile_service::application::use_cases::purge_expired_mutation_outcomes::PurgeExpiredMutationOutcomesUseCase;
use std::sync::Arc;
use uuid::Uuid;

const USER_ID: &str = "a1000000-0000-4000-8000-000000000001";

#[tokio::test]
async fn receipt_retention_clears_expired_bodies_in_bounded_batches_and_keeps_tombstones() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");
    let user_id = Uuid::parse_str(USER_ID).expect("user id");
    app.state
        .update_profile
        .execute(UpdateProfileDto {
            user_id: USER_ID.into(),
            username: "receipt_owner".into(),
            nickname: "Receipt Retention Owner".into(),
            avatar_url: "/avatars/receipt-retention.svg".into(),
            bio: "Profile mutation receipt retention test".into(),
        })
        .await
        .expect("create profile");

    let operation_ids = [Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3)];
    for (operation_id, key, visibility) in [
        (operation_ids[0], "retention-key-1", "PUBLIC"),
        (operation_ids[1], "retention-key-2", "PRIVATE"),
        (operation_ids[2], "retention-key-3", "PRIVATE"),
    ] {
        let context = MutationContextDto {
            operation_id,
            actor_id: user_id,
            idempotency_key: Some(key.into()),
            lookup_only: false,
        };
        app.state
            .update_privacy
            .execute_with_operation(
                UpdateProfilePrivacyDto {
                    user_id: USER_ID.into(),
                    actor_id: USER_ID.into(),
                    account_visibility: visibility.into(),
                },
                Some(context),
            )
            .await
            .expect("commit privacy receipt");
    }

    sqlx::query(
        "update profile_mutation_receipts set replay_until = now() - interval '1 second' where operation_id = any($1)",
    )
    .bind(&operation_ids[..2])
    .execute(&app.pool)
    .await
    .expect("expire two receipts");

    let repository = Arc::new(PgProfileRepository::new(app.pool.clone()));
    let metrics = app.state.metrics.clone();
    let worker = ProfileMutationReceiptRetentionWorker::new(
        PurgeExpiredMutationOutcomesUseCase::new(repository),
        metrics.clone(),
        1,
    );

    assert_eq!(worker.tick().await.expect("first bounded tick"), 1);
    assert!(outcome_is_null(&app.pool, operation_ids[0]).await);
    assert!(!outcome_is_null(&app.pool, operation_ids[1]).await);
    assert!(!outcome_is_null(&app.pool, operation_ids[2]).await);
    assert_eq!(receipt_count(&app.pool, &operation_ids).await, 3);

    assert_eq!(worker.tick().await.expect("second bounded tick"), 1);
    assert_eq!(worker.tick().await.expect("empty tick"), 0);
    assert!(outcome_is_null(&app.pool, operation_ids[0]).await);
    assert!(outcome_is_null(&app.pool, operation_ids[1]).await);
    assert!(!outcome_is_null(&app.pool, operation_ids[2]).await);
    assert_eq!(receipt_count(&app.pool, &operation_ids).await, 3);

    let expired_status = app
        .state
        .update_privacy
        .mutation_receipt_status(operation_ids[0])
        .await
        .expect("expired receipt status")
        .expect("tombstone remains");
    assert_eq!(expired_status.state, MutationReceiptStateDto::Expired);
    let live_status = app
        .state
        .update_privacy
        .mutation_receipt_status(operation_ids[2])
        .await
        .expect("live receipt status")
        .expect("live receipt remains");
    assert_eq!(live_status.state, MutationReceiptStateDto::Replayable);

    let expired_retry = app
        .state
        .update_privacy
        .execute_with_operation(
            UpdateProfilePrivacyDto {
                user_id: USER_ID.into(),
                actor_id: USER_ID.into(),
                account_visibility: "PUBLIC".into(),
            },
            Some(MutationContextDto {
                operation_id: operation_ids[0],
                actor_id: user_id,
                idempotency_key: Some("retention-key-1".into()),
                lookup_only: false,
            }),
        )
        .await
        .expect_err("expired outcome must not be replayed or redispatched");
    assert!(expired_retry.to_string().contains("replay window expired"));

    let conflicting_key = app
        .state
        .update_privacy
        .execute_with_operation(
            UpdateProfilePrivacyDto {
                user_id: USER_ID.into(),
                actor_id: USER_ID.into(),
                account_visibility: "PUBLIC".into(),
            },
            Some(MutationContextDto {
                operation_id: Uuid::from_u128(4),
                actor_id: user_id,
                idempotency_key: Some("retention-key-1".into()),
                lookup_only: false,
            }),
        )
        .await
        .expect_err("pruned key must remain bound to its original operation");
    assert!(conflicting_key.to_string().contains("conflicts"));

    let visibility: String =
        sqlx::query_scalar("select account_visibility from profiles where user_id = $1")
            .bind(user_id)
            .fetch_one(&app.pool)
            .await
            .expect("visibility after refused retries");
    assert_eq!(
        visibility, "PRIVATE",
        "expired retry did not execute a write"
    );
    let audit_count: i64 = sqlx::query_scalar(
        "select count(*) from profile_audit_events where profile_user_id = $1 and action = 'UPDATE_PRIVACY'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .expect("privacy audit count");
    assert_eq!(
        audit_count, 3,
        "refused retries did not duplicate the audit"
    );

    let metrics = metrics.render_prometheus();
    assert!(
        metrics.contains("profile_mutation_receipt_retention_runs_total{outcome=\"success\"} 3")
    );
    assert!(metrics.contains("profile_mutation_receipt_outcome_bodies_purged_total{} 2"));
}

async fn outcome_is_null(pool: &sqlx::PgPool, operation_id: Uuid) -> bool {
    sqlx::query_scalar(
        "select outcome_json is null from profile_mutation_receipts where operation_id = $1",
    )
    .bind(operation_id)
    .fetch_one(pool)
    .await
    .expect("receipt body state")
}

async fn receipt_count(pool: &sqlx::PgPool, operation_ids: &[Uuid]) -> i64 {
    sqlx::query_scalar(
        "select count(*) from profile_mutation_receipts where operation_id = any($1)",
    )
    .bind(operation_ids)
    .fetch_one(pool)
    .await
    .expect("receipt count")
}
