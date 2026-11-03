//! Adapter persistence tests require a runner-owned real PostgreSQL cluster.
#[path = "support/event_lifecycle/mod.rs"]
mod support;
use chrono::Duration;
use concert_application::{commands::*, event_lifecycle::CloseCommand};
use concert_domain::event_lifecycle::{JobState, Step};
use concert_service::event_lifecycle::*;
use std::sync::atomic::Ordering;
use support::*;
use uuid::Uuid;
fn start() -> CloseCommand {
    CloseCommand::Start {
        expected_revision: 1,
        reason: "Owned fixture closure".into(),
    }
}
fn resume(id: Uuid, rev: i64) -> CloseCommand {
    CloseCommand::Resume {
        job_id: id,
        expected_revision: rev,
        reason: "Explicit fixture retry".into(),
    }
}
#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture"]
async fn event_lifecycle_idempotent_start_receipt_is_admission_only() {
    let pool = pool().await;
    let actor = actor(&pool).await;
    let key = Uuid::new_v4();
    let service = PgEventLifecycle::new(
        pool.clone(),
        Authority {
            delay: Duration::zero(),
        },
        Owners::healthy(),
    );
    let first = service.command(actor, key, start(), &keys()).await.unwrap();
    assert_eq!(first.state, ReceiptState::Succeeded);
    assert_eq!(first.result_status, Some(200));
    let jobid = first.reference.unwrap().resource;
    assert_eq!(jobid, start_job_id(actor, key));
    let replay = service.command(actor, key, start(), &keys()).await.unwrap();
    assert_eq!(first, replay);
    let job = service.job(actor, jobid).await.unwrap();
    assert_eq!(job.state, JobState::Pending);
    assert_eq!(job.next(), Some(Step::FreezeMural));
    let phase: String = sqlx::query_scalar("select phase from concert_events where event_id=$1")
        .bind(actor.event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(phase, "READ_ONLY");
    let outbox: i64 = sqlx::query_scalar("select count(*) from concert_outbox where receipt_id=$1")
        .bind(first.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(outbox, 1);
    let other = service
        .command(actor, Uuid::new_v4(), start(), &keys())
        .await
        .unwrap();
    assert_eq!(other.failure, Some(SafeFailure::InvalidTransition));
}
#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture"]
async fn event_lifecycle_partial_failure_rolls_back_and_explicit_resume_skips_success() {
    let pool = pool().await;
    let actor = actor(&pool).await;
    let owners = Owners::healthy();
    owners.fail_snapshot.store(true, Ordering::SeqCst);
    let service = PgEventLifecycle::new(
        pool.clone(),
        Authority {
            delay: Duration::zero(),
        },
        owners.clone(),
    );
    let jobid = service
        .command(actor, Uuid::new_v4(), start(), &keys())
        .await
        .unwrap()
        .reference
        .unwrap()
        .resource;
    service.run_next(actor, jobid).await.unwrap();
    let failed = service.run_next(actor, jobid).await.unwrap();
    assert_eq!(failed.state, JobState::PartialFailure);
    assert_eq!(count(&pool, actor.event, "FREEZE_MURAL").await, 1);
    assert_eq!(count(&pool, actor.event, "SNAPSHOT").await, 0);
    assert_eq!(
        service.run_next(actor, jobid).await.unwrap_err(),
        CommandError::InvalidState
    );
    let stale = service
        .command(
            actor,
            Uuid::new_v4(),
            resume(jobid, failed.revision - 1),
            &keys(),
        )
        .await
        .unwrap();
    assert_eq!(stale.failure, Some(SafeFailure::VersionConflict));
    owners.fail_snapshot.store(false, Ordering::SeqCst);
    let retrykey = Uuid::new_v4();
    let request = resume(jobid, failed.revision);
    let accepted = service
        .command(actor, retrykey, request.clone(), &keys())
        .await
        .unwrap();
    assert_eq!(accepted.reference.unwrap().revision, failed.revision + 1);
    assert_eq!(
        accepted,
        service
            .command(actor, retrykey, request, &keys())
            .await
            .unwrap()
    );
    while service.job(actor, jobid).await.unwrap().state != JobState::Completed {
        service.run_next(actor, jobid).await.unwrap();
    }
    assert_eq!(count(&pool, actor.event, "FREEZE_MURAL").await, 1);
    assert_eq!(count(&pool, actor.event, "SNAPSHOT").await, 1);
    let phase: String = sqlx::query_scalar("select phase from concert_events where event_id=$1")
        .bind(actor.event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(phase, "ARCHIVED");
    service.run_next(actor, jobid).await.unwrap();
    assert_eq!(count(&pool, actor.event, "CLEANUP").await, 1);
}
#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture"]
async fn event_lifecycle_retention_wait_and_current_authority_fail_closed() {
    let pool = pool().await;
    let actor = actor(&pool).await;
    let key = Uuid::new_v4();
    let service = PgEventLifecycle::new(
        pool.clone(),
        Authority {
            delay: Duration::hours(1),
        },
        Owners::healthy(),
    );
    let first = service.command(actor, key, start(), &keys()).await.unwrap();
    let job = first.reference.unwrap().resource;
    for _ in 0..5 {
        service.run_next(actor, job).await.unwrap();
    }
    let waiting = service.run_next(actor, job).await.unwrap();
    assert_eq!(waiting.next(), Some(Step::Cleanup));
    assert_ne!(waiting.state, JobState::Completed);
    assert_eq!(count(&pool, actor.event, "CLEANUP").await, 0);
    assert_eq!(waiting, service.run_next(actor, job).await.unwrap());
    sqlx::query(
        "update be41_fixture_authority set allowed=false,revision=revision+1 where event_id=$1",
    )
    .bind(actor.event)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        service
            .command(actor, key, start(), &keys())
            .await
            .unwrap_err(),
        CommandError::Denied
    );
    assert_eq!(
        service.job(actor, job).await.unwrap_err(),
        CommandError::Denied
    );
    assert_eq!(
        service.run_next(actor, job).await.unwrap_err(),
        CommandError::Denied
    );
}
#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture"]
async fn event_lifecycle_concurrent_workers_and_wrong_receipt() {
    let pool = pool().await;
    let actor = actor(&pool).await;
    let owners = Owners::healthy();
    let service = PgEventLifecycle::new(
        pool.clone(),
        Authority {
            delay: Duration::zero(),
        },
        owners.clone(),
    );
    let job = service
        .command(actor, Uuid::new_v4(), start(), &keys())
        .await
        .unwrap()
        .reference
        .unwrap()
        .resource;
    let (a, b) = tokio::join!(service.run_next(actor, job), service.run_next(actor, job));
    a.unwrap();
    b.unwrap();
    assert_eq!(count(&pool, actor.event, "FREEZE_MURAL").await, 1);
    assert_eq!(count(&pool, actor.event, "SNAPSHOT").await, 1);
    owners.wrong_receipt.store(true, Ordering::SeqCst);
    let failed = service.run_next(actor, job).await.unwrap();
    assert_eq!(failed.state, JobState::PartialFailure);
    assert_eq!(count(&pool, actor.event, "EXPIRE_GRANTS").await, 0);
    let mut wrong = actor;
    wrong.actor = Uuid::new_v4();
    assert_eq!(
        service.job(wrong, job).await.unwrap_err(),
        CommandError::Denied
    );
}
