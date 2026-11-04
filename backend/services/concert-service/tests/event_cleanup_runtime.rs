//! Actual PostgreSQL qualification of the additive durable engine. Explicit
//! synthetic privilege/owner seams do not qualify full original BE41 delivery.
mod event_cleanup_support;
use concert_application::{commands::CommandError, event_cleanup::*};
use concert_domain::event_lifecycle::{Retention, Step};
use concert_service::{
    event_cleanup_live::*,
    event_lifecycle::{EffectKey, LifecycleOwners},
};
use event_cleanup_support::*;
use sqlx::{migrate::Migrator, Row};
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use uuid::Uuid;
#[test]
fn event_cleanup_config_has_no_enabled_or_unbounded_default() {
    use concert_service::config::event_cleanup::EventCleanupConfig as C;
    assert_eq!(C::from_values(None, None).unwrap(), None);
    for value in [None, Some("0"), Some("101"), Some("bad")] {
        assert!(C::from_values(Some("true"), value).is_err());
    }
    assert_eq!(
        C::from_values(Some("true"), Some("100"))
            .unwrap()
            .unwrap()
            .batch_limit,
        100
    );
    println!("BE41_CLEANUP_SOURCE_PIN={}", owned_source_pin());
}
#[tokio::test]
#[ignore = "owned event_cleanup/run.py fixture only"]
async fn event_cleanup_fixture_prepare() {
    let pool = pool().await;
    let migrations = std::path::PathBuf::from(
        std::env::var("MCR_EVENT_CLEANUP_MIGRATIONS").expect("frozen migration directory required"),
    );
    let full = Migrator::new(migrations).await.unwrap();
    assert_eq!(full.iter().filter(|m| m.version == 46).count(), 1);
    let baseline = Migrator {
        migrations: std::borrow::Cow::Owned(
            full.iter().filter(|m| m.version < 46).cloned().collect(),
        ),
        ..Migrator::DEFAULT
    };
    baseline.run(&pool).await.unwrap();
    let event = Uuid::new_v4();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Upgrade preserved','LIVE')").bind(event).bind(event.to_string()).execute(&pool).await.unwrap();
    let before: serde_json::Value =
        sqlx::query_scalar("select to_jsonb(e) from concert_events e where event_id=$1")
            .bind(event)
            .fetch_one(&pool)
            .await
            .unwrap();
    full.run(&pool).await.unwrap();
    migration_runtime::verify_receipts(&pool, &full)
        .await
        .unwrap();
    full.run(&pool).await.unwrap();
    let after: serde_json::Value =
        sqlx::query_scalar("select to_jsonb(e) from concert_events e where event_id=$1")
            .bind(event)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before, after);
    let database = format!("mcr_event_cleanup_fresh_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("create database {database} template template0"))
        .execute(&pool)
        .await
        .unwrap();
    let fresh = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect_with(pool.connect_options().as_ref().clone().database(&database))
        .await
        .unwrap();
    full.run(&fresh).await.unwrap();
    migration_runtime::verify_receipts(&fresh, &full)
        .await
        .unwrap();
    fresh.close().await;
    sqlx::query(&format!("drop database {database}"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql("create table event_cleanup_fixture_authority(event_id uuid,actor_id uuid,allowed boolean,revision bigint,primary key(event_id,actor_id));create table event_cleanup_fixture_policy(singleton boolean primary key,revision bigint);insert into event_cleanup_fixture_policy values(true,1);create table event_cleanup_fixture_items(event_id uuid,id bigint,payload text,primary key(event_id,id));create table event_cleanup_fixture_effects(operation_id uuid primary key,event_id uuid,binding bytea,receipt_id uuid,processed smallint,complete boolean);create table event_cleanup_fixture_restart(actor_id uuid,event_id uuid,job_id uuid);").execute(&pool).await.unwrap();
    println!("BE41_CLEANUP_SOURCE_PIN={}", owned_source_pin());
}
#[tokio::test]
#[ignore = "owned fixture"]
async fn event_cleanup_runtime_absent_authority_and_owners_never_apply() {
    let pool = pool().await;
    let (actor, job) = seed(&pool, 3).await;
    let absent = CleanupRuntime::new(pool.clone(), None, Owners::default(), 2).unwrap();
    assert_eq!(
        absent
            .run_owner(actor, job, Owner::Mural)
            .await
            .unwrap_err(),
        CommandError::Unavailable
    );
    let runtime = CleanupRuntime::new(
        pool.clone(),
        Some(Arc::new(Authority {
            milliseconds: 10000,
        })),
        Owners::default(),
        2,
    )
    .unwrap();
    assert_eq!(
        runtime
            .run_owner(actor, job, Owner::Mural)
            .await
            .unwrap()
            .state,
        State::Pending
    );
    assert!(!runtime.ready(actor, job).await.unwrap());
    assert_eq!(remaining(&pool, actor.event).await, 3);
    parent_pending(&pool, job).await;
    let mut c = pool.acquire().await.unwrap();
    let retention = Retention {
        policy_revision: 1,
        closed_at: chrono::Utc::now(),
        cleanup_after: chrono::Utc::now(),
    };
    assert!(ConvergedOwners::new(Owners::default())
        .apply_in(
            &mut c,
            EffectKey {
                event: actor.event,
                job,
                step: Step::FreezeMural
            },
            retention
        )
        .await
        .is_err());
}
#[tokio::test]
#[ignore = "owned fixture"]
async fn event_cleanup_runtime_partial_failure_pending_and_verification_rollback_resume() {
    let pool = pool().await;
    let (actor, job) = seed(&pool, 5).await;
    let owner = Arc::new(OwnerFixture::default());
    let run = runtime(&pool, owner.clone(), 10000);
    for mode in [1, 2, 3] {
        owner.mode.store(mode, Ordering::SeqCst);
        let p = run.run_owner(actor, job, Owner::Mural).await.unwrap();
        assert_eq!(
            p.state,
            if mode == 2 {
                State::Pending
            } else {
                State::PartialFailure
            }
        );
        assert_eq!(p.processed, 0);
        assert_eq!(remaining(&pool, actor.event).await, 5);
        let effects: i64 = sqlx::query_scalar(
            "select count(*) from event_cleanup_fixture_effects where event_id=$1",
        )
        .bind(actor.event)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(effects, 0);
    }
    owner.mode.store(0, Ordering::SeqCst);
    assert_eq!(
        run.run_owner(actor, job, Owner::Mural)
            .await
            .unwrap()
            .processed,
        2
    );
    owner.mode.store(1, Ordering::SeqCst);
    assert_eq!(
        run.run_owner(actor, job, Owner::Mural)
            .await
            .unwrap()
            .processed,
        2
    );
    // New runtime instance must reload the committed cursor, not restart from zero.
    owner.mode.store(0, Ordering::SeqCst);
    let restarted = runtime(&pool, owner.clone(), 10000);
    assert_eq!(
        restarted
            .run_owner(actor, job, Owner::Mural)
            .await
            .unwrap()
            .processed,
        4
    );
    assert_eq!(
        restarted
            .run_owner(actor, job, Owner::Mural)
            .await
            .unwrap()
            .state,
        State::Complete
    );
    assert_eq!(
        restarted
            .run_owner(actor, job, Owner::Mural)
            .await
            .unwrap()
            .processed,
        5
    );
    assert!(restarted.ready(actor, job).await.unwrap());
    assert_eq!(remaining(&pool, actor.event).await, 0);
    parent_pending(&pool, job).await;
}
#[tokio::test]
#[ignore = "owned fixture"]
async fn event_cleanup_runtime_concurrent_workers_serialize_and_operator_revocation_fences() {
    let pool = pool().await;
    let (actor, job) = seed(&pool, 5).await;
    let owner = Arc::new(OwnerFixture::default());
    owner.mode.store(4, Ordering::SeqCst);
    let run = Arc::new(runtime(&pool, owner.clone(), 10000));
    let first = {
        let run = run.clone();
        tokio::spawn(async move { run.run_owner(actor, job, Owner::Mural).await })
    };
    tokio::time::timeout(Duration::from_secs(3), owner.entered.notified())
        .await
        .unwrap();
    let second = {
        let run = run.clone();
        tokio::spawn(async move { run.run_owner(actor, job, Owner::Mural).await })
    };
    let revoke = {
        let pool = pool.clone();
        tokio::spawn(async move {
            sqlx::query("update event_cleanup_fixture_authority set allowed=false,revision=revision+1 where event_id=$1").bind(actor.event).execute(&pool).await.unwrap()
        })
    };
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!second.is_finished());
    assert!(!revoke.is_finished());
    owner.mode.store(0, Ordering::SeqCst);
    owner.release.notify_one();
    assert_eq!(first.await.unwrap().unwrap().processed, 2);
    revoke.await.unwrap();
    // Whichever contender acquired its source lock first may commit one page;
    // after revocation returns, every subsequent attempt must reject.
    let _ = second.await.unwrap();
    let before = remaining(&pool, actor.event).await;
    assert_eq!(
        run.run_owner(actor, job, Owner::Mural).await.unwrap_err(),
        CommandError::Denied
    );
    assert_eq!(remaining(&pool, actor.event).await, before);
    parent_pending(&pool, job).await;
    let duplicates:i64=sqlx::query_scalar("select count(*)-count(distinct operation_id) from concert_event_cleanup_batches where job_id=$1").bind(job).fetch_one(&pool).await.unwrap();
    assert_eq!(duplicates, 0);
}
#[tokio::test]
#[ignore = "owned fixture"]
async fn event_cleanup_runtime_expired_fence_rolls_back_effect_cursor_and_commit() {
    let pool = pool().await;
    let (actor, job) = seed(&pool, 3).await;
    let owner = Arc::new(OwnerFixture::default());
    owner.mode.store(4, Ordering::SeqCst);
    let run = Arc::new(runtime(&pool, owner.clone(), 250));
    let task = {
        let run = run.clone();
        tokio::spawn(async move { run.run_owner(actor, job, Owner::Mural).await })
    };
    owner.entered.notified().await;
    tokio::time::sleep(Duration::from_millis(350)).await;
    owner.release.notify_one();
    assert!(task.await.unwrap().is_err());
    assert_eq!(remaining(&pool, actor.event).await, 3);
    let batches: i64 =
        sqlx::query_scalar("select count(*) from concert_event_cleanup_batches where job_id=$1")
            .bind(job)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(batches, 0);
    parent_pending(&pool, job).await;
}
#[tokio::test]
#[ignore = "owned fixture"]
async fn event_cleanup_runtime_sql_immutable_evidence_cannot_be_rewritten_or_truncated() {
    let pool = pool().await;
    let (actor, job) = seed(&pool, 3).await;
    let owner = Arc::new(OwnerFixture::default());
    runtime(&pool, owner, 10000)
        .run_owner(actor, job, Owner::Mural)
        .await
        .unwrap();
    for query in [
        "update concert_event_cleanup_batches set receipt_id=operation_id",
        "delete from concert_event_cleanup_batches",
        "truncate concert_event_cleanup_batches cascade",
        "truncate concert_event_cleanup_progress cascade",
        "update concert_event_cleanup_progress set processed=processed+1",
    ] {
        assert!(sqlx::query(query).execute(&pool).await.is_err(), "{query}");
    }
    assert_eq!(remaining(&pool, actor.event).await, 1);
}
#[tokio::test]
#[ignore = "owned fixture"]
async fn event_cleanup_runtime_policy_change_and_wrong_scope_do_not_advance() {
    let pool = pool().await;
    let (actor, job) = seed(&pool, 3).await;
    let run = runtime(&pool, Arc::new(OwnerFixture::default()), 10000);
    assert_eq!(
        run.run_owner(actor, Uuid::new_v4(), Owner::Mural)
            .await
            .unwrap_err(),
        CommandError::Denied
    );
    assert_eq!(
        run.run_owner(actor, job, Owner::MediaRetention)
            .await
            .unwrap_err(),
        CommandError::InvalidState
    );
    sqlx::query("update event_cleanup_fixture_policy set revision=2 where singleton=true")
        .execute(&pool)
        .await
        .unwrap();
    let result = run.run_owner(actor, job, Owner::Mural).await;
    sqlx::query("update event_cleanup_fixture_policy set revision=1 where singleton=true")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(result.unwrap_err(), CommandError::Unavailable);
    assert_eq!(remaining(&pool, actor.event).await, 3);
    parent_pending(&pool, job).await;
}

#[tokio::test]
#[ignore = "owned fixture; runner restarts actual PostgreSQL after this test"]
async fn event_cleanup_restart_prepare() {
    let pool = pool().await;
    let (actor, job) = seed(&pool, 5).await;
    let owner = Arc::new(OwnerFixture::default());
    let run = runtime(&pool, owner, 10000);
    assert_eq!(
        run.run_owner(actor, job, Owner::Mural)
            .await
            .unwrap()
            .processed,
        2
    );
    sqlx::query("insert into event_cleanup_fixture_restart values($1,$2,$3)")
        .bind(actor.actor)
        .bind(actor.event)
        .bind(job)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
}
#[tokio::test]
#[ignore = "owned fixture; must execute after actual PostgreSQL restart"]
async fn event_cleanup_restart_verify() {
    let pool = pool().await;
    let row = sqlx::query("select * from event_cleanup_fixture_restart")
        .fetch_one(&pool)
        .await
        .unwrap();
    let actor = concert_application::event_lifecycle::Actor {
        actor: row.get("actor_id"),
        event: row.get("event_id"),
    };
    let job = row.get("job_id");
    assert_eq!(remaining(&pool, actor.event).await, 3);
    let run = runtime(&pool, Arc::new(OwnerFixture::default()), 10000);
    assert_eq!(
        run.run_owner(actor, job, Owner::Mural)
            .await
            .unwrap()
            .processed,
        4
    );
    assert_eq!(
        run.run_owner(actor, job, Owner::Mural)
            .await
            .unwrap()
            .processed,
        5
    );
    let batches: i64 =
        sqlx::query_scalar("select count(*) from concert_event_cleanup_batches where job_id=$1")
            .bind(job)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(batches, 3);
    assert!(run.ready(actor, job).await.unwrap());
    parent_pending(&pool, job).await;
}
