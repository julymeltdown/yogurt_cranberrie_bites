use chrono::{Duration, Utc};
use friend_service::adapters::r#in::messaging::mutation_receipt_retention_worker::MutationReceiptRetentionWorker;
use friend_service::application::dto::mutation_dto::MutationContextDto;
use friend_service::application::use_cases::purge_expired_mutation_outcomes::PurgeExpiredMutationOutcomesUseCase;
use uuid::Uuid;

const FOLLOWER_ID: &str = "11111111-1111-1111-1111-111111111111";

#[tokio::test]
async fn receipt_retention_is_bounded_and_keeps_identity_tombstones() -> anyhow::Result<()> {
    let app = crate::support::start_postgres_app(false).await?;
    let actor_id = Uuid::parse_str(FOLLOWER_ID)?;
    let now = Utc::now();
    let mut receipts = Vec::new();

    for index in 0..3 {
        let operation_id = Uuid::new_v4();
        let followee_id = Uuid::new_v4();
        let idempotency_key = format!("retention-{index}");
        let replay_until = if index < 2 {
            now - Duration::minutes(1)
        } else {
            now + Duration::days(1)
        };
        let canonical_command = serde_json::json!({"followeeId": followee_id}).to_string();
        let outcome = serde_json::json!({
            "kind": "follow",
            "outcome": {
                "edge": null,
                "following": true,
                "status": "ACTIVE",
                "requestId": null
            }
        });
        sqlx::query(
            "insert into friend_mutation_receipts
             (operation_id, actor_id, idempotency_key, scope, command_fingerprint,
              outcome_json, replay_until)
             values ($1, $2, $3, 'friend.v1.FollowUser',
                     encode(sha256(convert_to($4, 'UTF8')), 'hex'), $5, $6)",
        )
        .bind(operation_id)
        .bind(actor_id)
        .bind(&idempotency_key)
        .bind(canonical_command)
        .bind(outcome)
        .bind(replay_until)
        .execute(&app.pool)
        .await?;
        receipts.push((operation_id, followee_id, idempotency_key));
    }

    let worker = MutationReceiptRetentionWorker::new(
        PurgeExpiredMutationOutcomesUseCase::new(app.state.relationship_repository.clone()),
        app.state.mutation_receipt_retention_metrics.clone(),
        1,
    );
    assert_eq!(worker.tick().await?, 1);
    let after_first: (i64, i64, i64) = sqlx::query_as(
        "select count(*) filter (where replay_until <= now() and outcome_json is not null),
                count(*) filter (where replay_until > now() and outcome_json is not null),
                count(*)
         from friend_mutation_receipts",
    )
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(after_first, (1, 1, 3));

    let purged_operation_id: Uuid = sqlx::query_scalar(
        "select operation_id from friend_mutation_receipts
         where replay_until <= now() and outcome_json is null",
    )
    .fetch_one(&app.pool)
    .await?;
    let (operation_id, followee_id, idempotency_key) = receipts
        .iter()
        .find(|(operation_id, _, _)| *operation_id == purged_operation_id)
        .expect("purged tombstone belongs to the expired receipts");
    let expired = app
        .state
        .follow
        .execute_with_operation(
            actor_id,
            *followee_id,
            Some(MutationContextDto {
                operation_id: *operation_id,
                actor_id,
                idempotency_key: Some(idempotency_key.clone()),
                lookup_only: false,
            }),
        )
        .await
        .expect_err("expired tombstone must refuse replay");
    assert!(expired.to_string().contains("expired"));

    let reused_key = app
        .state
        .follow
        .execute_with_operation(
            actor_id,
            *followee_id,
            Some(MutationContextDto {
                operation_id: Uuid::new_v4(),
                actor_id,
                idempotency_key: Some(idempotency_key.clone()),
                lookup_only: false,
            }),
        )
        .await
        .expect_err("purged tombstone must continue reserving its key");
    assert!(reused_key.to_string().contains("conflicts"));
    let active_edges: i64 = sqlx::query_scalar(
        "select count(*) from follows where follower_id = $1 and deleted_at is null",
    )
    .bind(actor_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(active_edges, 0, "expired commands must not be reexecuted");

    assert_eq!(worker.tick().await?, 1);
    assert_eq!(worker.tick().await?, 0);
    let after_second: (i64, i64, i64) = sqlx::query_as(
        "select count(*) filter (where replay_until <= now() and outcome_json is not null),
                count(*) filter (where replay_until > now() and outcome_json is not null),
                count(*)
         from friend_mutation_receipts",
    )
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(after_second, (0, 1, 3));
    assert!(app
        .state
        .mutation_receipt_retention_metrics
        .render_prometheus()
        .contains("friend_mutation_receipt_outcome_bodies_purged_total 2"));

    app.shutdown().await?;
    Ok(())
}
