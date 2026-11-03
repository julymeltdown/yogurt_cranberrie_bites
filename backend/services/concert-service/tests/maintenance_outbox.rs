//! Native test-only metadata maintenance; no actor authority is created for system work.
#[path = "support/community/owned.rs"]
mod owned;
use concert_application::commands::{maintenance::*, *};
use concert_service::adapters::out::repository::{
    outbox::{DeliveryFailure, PgOutbox},
    outbox_maintenance::enqueue_maintenance_in,
};
use uuid::Uuid;
#[tokio::test]
#[ignore = "requires marker-owned native cluster"]
async fn maintenance_origin_transactions_and_leases() -> anyhow::Result<()> {
    let pool = owned::owned_pool().await?;
    let event = Uuid::new_v4();
    let id = Uuid::new_v4();
    let value = MaintenanceInvalidation {
        id,
        event,
        reference: SafeReference {
            resource: Uuid::new_v4(),
            revision: 2,
        },
    };
    sqlx::query(
        "insert into concert_events(event_id,slug,title,phase) values($1,$2,'Maintenance','OPEN')",
    )
    .bind(event)
    .bind(format!("maint-{event}"))
    .execute(&pool)
    .await?;
    let mut tx = pool.begin().await?;
    enqueue_maintenance_in(&mut tx, value).await.unwrap();
    tx.rollback().await?;
    let count: i64 = sqlx::query_scalar("select count(*) from concert_outbox where outbox_id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 0);
    let mut tx = pool.begin().await?;
    enqueue_maintenance_in(&mut tx, value).await.unwrap();
    tx.commit().await?;
    let mut tx = pool.begin().await?;
    assert_eq!(enqueue_maintenance_in(&mut tx, value).await.unwrap(), id);
    tx.commit().await?;
    let mut tx = pool.begin().await?;
    assert_eq!(
        enqueue_maintenance_in(
            &mut tx,
            MaintenanceInvalidation {
                reference: SafeReference {
                    revision: 3,
                    ..value.reference
                },
                ..value
            }
        )
        .await,
        Err(CommandError::Conflict)
    );
    tx.rollback().await?;
    let box_ = PgOutbox::new(pool.clone());
    let items = box_.claim(100).await.unwrap();
    let first = items.iter().find(|x| x.id == id).unwrap();
    assert_eq!(first.receipt, None);
    assert_eq!(first.kind, InvalidationKind::MaintenanceInvalidated);
    // Crash/reclaim rotates lease and stale holder cannot acknowledge.
    sqlx::query("update concert_outbox set lease_until=clock_timestamp()-interval '1 second' where outbox_id=$1").bind(id).execute(&pool).await?;
    let second = box_
        .claim(100)
        .await
        .unwrap()
        .into_iter()
        .find(|x| x.id == id)
        .unwrap();
    assert_ne!(second.lease, first.lease);
    assert!(!box_.acknowledge(first).await.unwrap());
    assert!(box_
        .fail(&second, DeliveryFailure::Retryable)
        .await
        .unwrap());
    sqlx::query("update concert_outbox set available_at=clock_timestamp() where outbox_id=$1")
        .bind(id)
        .execute(&pool)
        .await?;
    let third = box_
        .claim(100)
        .await
        .unwrap()
        .into_iter()
        .find(|x| x.id == id)
        .unwrap();
    assert!(box_.acknowledge(&third).await.unwrap());
    sqlx::query("update concert_outbox set terminal_at=clock_timestamp()-interval '8 days' where outbox_id=$1").bind(id).execute(&pool).await?;
    assert!(box_.prune(100).await.unwrap() >= 1);
    let mut tx = pool.begin().await?;
    enqueue_maintenance_in(&mut tx, value).await.unwrap();
    tx.commit().await?;
    let count: i64 = sqlx::query_scalar("select count(*) from concert_outbox where outbox_id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 0);
    let receipts: i64 =
        sqlx::query_scalar("select count(*) from concert_command_receipts where event_id=$1")
            .bind(event)
            .fetch_one(&pool)
            .await?;
    assert_eq!(receipts, 0);
    assert!(InvalidationKind::parse("MAINTENANCE_INVALIDATED", Some(Uuid::new_v4())).is_err());
    assert!(InvalidationKind::parse("RESOURCE_INVALIDATED", None).is_err());
    Ok(())
}
#[tokio::test]
#[ignore = "requires marker-owned native cluster"]
async fn maintenance_restart_dedupe() -> anyhow::Result<()> {
    let pool = owned::owned_pool().await?;
    let event = Uuid::from_u128(19001);
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,'maintenance-restart','Maintenance restart','OPEN') on conflict(event_id) do nothing").bind(event).execute(&pool).await?;
    let mut tx = pool.begin().await?;
    enqueue_maintenance_in(
        &mut tx,
        MaintenanceInvalidation {
            id: Uuid::from_u128(19002),
            event,
            reference: SafeReference {
                resource: Uuid::from_u128(19003),
                revision: 2,
            },
        },
    )
    .await
    .unwrap();
    tx.commit().await?;
    let count: i64 = sqlx::query_scalar("select count(*) from concert_outbox where event_id=$1")
        .bind(event)
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 1);
    Ok(())
}
