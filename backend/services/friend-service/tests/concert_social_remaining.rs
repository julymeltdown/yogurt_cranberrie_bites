//! Real owner-database contention tests; no Auth/Concert/browser acceptance.
#[path = "concert_social_remaining_support/owned.rs"]
mod owned;
use chrono::Utc;
use friend_service::{
    adapters::out::persistence::follow_repository::PgFollowRepository,
    application::{dto::mutation_dto::*, ports::*},
    domain::{follow_edge::FollowEdge, follow_request::AcceptFollowRequestOutcome},
};
use sqlx::{PgPool, Postgres, Transaction};
use std::time::Duration;
use uuid::Uuid;

async fn pending_block(p: &PgPool, a: Uuid, b: Uuid) -> Transaction<'_, Postgres> {
    let mut tx = p.begin().await.unwrap();
    // Actual owner table + all production deletion/revision/journal triggers.
    // Leave this uncommitted so a premature policy SELECT cannot see the block.
    sqlx::query(
        "insert into user_blocks(blocker_id,blocked_id,created_at) values($1,$2,clock_timestamp())",
    )
    .bind(a)
    .bind(b)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx
}
async fn wait_for_subject_lock(p: &PgPool) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let waiting: bool = sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and wait_event='advisory' and query like '%friend_lock_subject_ids%')")
                .fetch_one(p).await.unwrap();
            if waiting { break; }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.expect("repository must wait on subject locks before checking blocks");
}
fn edge(a: Uuid, b: Uuid) -> FollowEdge {
    FollowEdge {
        follower_id: a,
        followee_id: b,
        created_at: Utc::now(),
    }
}
async fn active_edges(p: &PgPool, a: Uuid, b: Uuid) -> i64 {
    sqlx::query_scalar("select count(*) from follows where deleted_at is null and ((follower_id=$1 and followee_id=$2) or (follower_id=$2 and followee_id=$1))")
        .bind(a).bind(b).fetch_one(p).await.unwrap()
}

#[tokio::test]
#[ignore = "private social_remaining_support/native.py fixture required"]
async fn committed_block_wins_over_waiting_plain_follow_in_both_directions() {
    let p = owned::pool().await;
    for reverse in [false, true] {
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        let tx = pending_block(&p, a, b).await;
        let repo = PgFollowRepository::new(p.clone());
        let next = if reverse { edge(b, a) } else { edge(a, b) };
        let task = tokio::spawn(async move { repo.follow(&next).await });
        wait_for_subject_lock(&p).await;
        tx.commit().await.unwrap();
        assert!(tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .unwrap()
            .unwrap()
            .is_err());
        assert_eq!(active_edges(&p, a, b).await, 0);
    }
    p.close().await;
}

#[tokio::test]
#[ignore = "private social_remaining_support/native.py fixture required"]
async fn committed_block_rejects_receipt_follow_and_request_creation() {
    let p = owned::pool().await;
    for request in [false, true] {
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        let tx = pending_block(&p, b, a).await;
        let repo = PgFollowRepository::new(p.clone());
        let task = tokio::spawn(async move {
            let operation = MutationOperationDto::new(
                MutationContextDto {
                    operation_id: Uuid::new_v4(),
                    actor_id: a,
                    idempotency_key: Some(Uuid::new_v4().to_string()),
                    lookup_only: false,
                },
                a,
                b,
                MutationKind::Follow,
            )
            .unwrap();
            repo.follow_with_receipt(&edge(a, b), request, &operation)
                .await
        });
        wait_for_subject_lock(&p).await;
        tx.commit().await.unwrap();
        assert!(task.await.unwrap().is_err());
        assert_eq!(active_edges(&p, a, b).await, 0);
        let requests:i64=sqlx::query_scalar("select count(*) from follow_requests where follower_id=$1 and followee_id=$2 and status='PENDING'").bind(a).bind(b).fetch_one(&p).await.unwrap();
        assert_eq!(requests, 0);
    }
    p.close().await;
}

#[tokio::test]
#[ignore = "private social_remaining_support/native.py fixture required"]
async fn acceptance_waits_before_request_lock_and_rechecks_committed_block() {
    let p = owned::pool().await;
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    let repo = PgFollowRepository::new(p.clone());
    let request = repo.create_request(a, b, Utc::now()).await.unwrap();
    let mut tx = pending_block(&p, b, a).await;
    let task = tokio::spawn(async move { repo.accept_request(request.id, b, Utc::now()).await });
    wait_for_subject_lock(&p).await;
    // Block must still be able to cancel the pending row. The old request-first
    // order deadlocked against this UPDATE while holding that same request row.
    tokio::time::timeout(Duration::from_secs(2), sqlx::query("update follow_requests set status='CANCELLED',decided_at=clock_timestamp() where id=$1")
        .bind(request.id).execute(&mut *tx)).await.unwrap().unwrap();
    tx.commit().await.unwrap();
    assert!(matches!(
        task.await.unwrap().unwrap(),
        AcceptFollowRequestOutcome::Blocked
    ));
    assert_eq!(active_edges(&p, a, b).await, 0);
    p.close().await;
}

#[tokio::test]
#[ignore = "private social_remaining_support/native.py fixture required"]
async fn rolled_back_block_does_not_poison_follow_or_create_false_signal() {
    let p = owned::pool().await;
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    let tx = pending_block(&p, a, b).await;
    let repo = PgFollowRepository::new(p.clone());
    let task = tokio::spawn(async move { repo.follow(&edge(a, b)).await });
    wait_for_subject_lock(&p).await;
    tx.rollback().await.unwrap();
    task.await.unwrap().unwrap();
    assert_eq!(active_edges(&p, a, b).await, 1);
    let signals:i64=sqlx::query_scalar("select count(*) from friend_world_invalidation_journal where user_id=least($1::uuid,$2::uuid) and peer_id=greatest($1::uuid,$2::uuid)")
        .bind(a).bind(b).fetch_one(&p).await.unwrap();
    assert_eq!(signals, 0);
    p.close().await;
}

#[tokio::test]
#[ignore = "private social_remaining_support/native.py fixture required"]
async fn actual_block_unblock_never_restores_prior_edge_or_request() {
    let p = owned::pool().await;
    let repo = PgFollowRepository::new(p.clone());
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    repo.follow(&edge(a, b)).await.unwrap();
    let request = repo.create_request(b, a, Utc::now()).await.unwrap();
    repo.block(a, b, Utc::now()).await.unwrap();
    repo.block(b, a, Utc::now()).await.unwrap();
    repo.unblock(a, b).await.unwrap();
    assert!(repo.policy(a, b).await.unwrap().blocked_viewer);
    repo.unblock(b, a).await.unwrap();
    assert_eq!(active_edges(&p, a, b).await, 0);
    assert!(matches!(
        repo.accept_request(request.id, a, Utc::now())
            .await
            .unwrap(),
        AcceptFollowRequestOutcome::NotFound
    ));
    assert!(
        repo.create_request(b, a, Utc::now()).await.is_ok(),
        "new consent is possible after both directions unblock"
    );
    p.close().await;
}
