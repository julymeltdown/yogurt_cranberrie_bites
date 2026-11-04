//! Real PgMuralRepository/BE15; fixture patterns from mural.rs at93ef0d8.
use super::support::*;
use concert_application::mural::*;
use concert_domain::mural::*;
use concert_service::adapters::out::repository::mural::*;
use sqlx::{PgPool, Row};
use uuid::Uuid;
fn pixel(revision: i64, x: i32) -> PlacePixel {
    PlacePixel {
        expected_revision: revision,
        x,
        y: 0,
        color_index: 3,
    }
}
async fn fixture(pool: &PgPool) -> anyhow::Result<MuralActor> {
    let (event, actor, _) = event(pool).await?;
    sqlx::query("insert into concert_mural_state(event_id,snapshot_id,state,pixels,palette,cooldown_ms) values($1,$2,'EDITABLE',$3,$4,3600000)").bind(event).bind(Uuid::new_v4()).bind(vec![0u8;PIXELS]).bind(vec!["#123ABC";16]).execute(pool).await?;
    Ok(MuralActor { actor, event })
}
async fn counts(pool: &PgPool, event: Uuid) -> anyhow::Result<(i64, i64, i64, i64)> {
    let row=sqlx::query("select (select count(*) from concert_mural_operations where event_id=$1) ops,(select count(*) from concert_mural_limits where event_id=$1) limits,(select count(*) from concert_command_receipts where event_id=$1) receipts,(select count(*) from concert_outbox where event_id=$1) outbox").bind(event).fetch_one(pool).await?;
    Ok((
        row.get("ops"),
        row.get("limits"),
        row.get("receipts"),
        row.get("outbox"),
    ))
}
#[tokio::test]
#[ignore = "requires owned QA03 PostgreSQL controller"]
async fn identical_idempotency_and_cooldown_wait_on_real_lock() -> anyhow::Result<()> {
    let pool = pool().await?;
    let actor = fixture(&pool).await?;
    let key = Uuid::new_v4();
    let mut held = pool.begin().await?;
    let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *held)
        .await?;
    sqlx::query("select event_id from concert_mural_state where event_id=$1 for update")
        .bind(actor.event)
        .fetch_one(&mut *held)
        .await?;
    let mut jobs = Vec::new();
    for _ in 0..2 {
        let p = pool.clone();
        jobs.push(tokio::spawn(async move {
            PgMuralRepository::new(p, IsolatedAuthority)
                .place(actor, key, pixel(1, 0), &keys())
                .await
        }));
    }
    waiting(&pool, pid, 2).await?;
    held.commit().await?;
    let first = jobs.remove(0).await??;
    let second = jobs.remove(0).await??;
    assert_eq!(first, second);
    assert_eq!(counts(&pool, actor.event).await?, (1, 1, 1, 1));
    let repo = PgMuralRepository::new(pool.clone(), IsolatedAuthority);
    assert_eq!(
        repo.place(actor, key, pixel(1, 1), &keys()).await,
        Err(MuralError::Command(
            concert_application::commands::CommandError::Conflict
        ))
    );
    let mut held = pool.begin().await?;
    let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *held)
        .await?;
    sqlx::query("select event_id from concert_mural_state where event_id=$1 for update")
        .bind(actor.event)
        .fetch_one(&mut *held)
        .await?;
    let mut jobs = Vec::new();
    for x in 1..=3 {
        let p = pool.clone();
        jobs.push(tokio::spawn(async move {
            PgMuralRepository::new(p, IsolatedAuthority)
                .place(actor, Uuid::new_v4(), pixel(2, x), &keys())
                .await
        }));
    }
    waiting(&pool, pid, 3).await?;
    held.commit().await?;
    for job in jobs {
        assert!(matches!(job.await?, Err(MuralError::Cooldown(Some(_)))))
    }
    assert_eq!(counts(&pool, actor.event).await?, (1, 1, 4, 1));
    let original = repo.recover(actor, key).await?;
    sqlx::query("insert into qa03_restart values('wall',$1)").bind(serde_json::json!({"actor":actor.actor,"event":actor.event,"key":key,"receipt":original.receipt_id,"operation":first.public.operation_id,"next_allowed_at":first.next_allowed_at})).execute(&pool).await?;
    terminal(&pool).await
}
#[tokio::test]
#[ignore = "requires owned QA03 PostgreSQL controller"]
async fn storage_failure_rolls_back_all_effects_then_same_key_recovers() -> anyhow::Result<()> {
    let pool = pool().await?;
    let actor = fixture(&pool).await?;
    let repo = PgMuralRepository::new(pool.clone(), IsolatedAuthority);
    let key = Uuid::new_v4();
    let before = repo.snapshot(actor).await?;
    let budget: i64 = sqlx::query_scalar("select receipts_used from concert_command_budget")
        .fetch_one(&pool)
        .await?;
    sqlx::query(&format!(
        "alter table concert_outbox add constraint qa03_fail_outbox check(event_id<>'{}'::uuid)",
        actor.event
    ))
    .execute(&pool)
    .await?;
    assert!(repo.place(actor, key, pixel(1, 0), &keys()).await.is_err());
    assert_eq!(counts(&pool, actor.event).await?, (0, 0, 0, 0));
    let after = repo.snapshot(actor).await?;
    assert_eq!(before.pixels, after.pixels);
    assert_eq!(before.revision, after.revision);
    assert_eq!(before.snapshot_id, after.snapshot_id);
    assert_eq!(after.next_allowed_at, None);
    let current: i64 = sqlx::query_scalar("select receipts_used from concert_command_budget")
        .fetch_one(&pool)
        .await?;
    assert_eq!(current, budget);
    sqlx::query("alter table concert_outbox drop constraint qa03_fail_outbox")
        .execute(&pool)
        .await?;
    assert_eq!(repo.recover(actor, key).await, Err(MuralError::Denied));
    repo.place(actor, key, pixel(1, 0), &keys()).await?;
    assert_eq!(counts(&pool, actor.event).await?, (1, 1, 1, 1));
    assert_eq!(
        repo.place(actor, Uuid::new_v4(), pixel(2, 128), &keys())
            .await,
        Err(MuralError::InvalidInput)
    );
    terminal(&pool).await
}
#[tokio::test]
#[ignore = "requires restart phase of owned QA03 controller"]
async fn restart_retains_operation_receipt_and_cooldown() -> anyhow::Result<()> {
    let pool = pool().await?;
    let v: serde_json::Value =
        sqlx::query_scalar("select payload from qa03_restart where kind='wall'")
            .fetch_one(&pool)
            .await?;
    let id = |k: &str| -> Uuid { serde_json::from_value(v[k].clone()).unwrap() };
    let actor = MuralActor {
        actor: id("actor"),
        event: id("event"),
    };
    let repo = PgMuralRepository::new(pool.clone(), IsolatedAuthority);
    let before = counts(&pool, actor.event).await?;
    let recovered = repo.recover(actor, id("key")).await?;
    assert_eq!(recovered.receipt_id, id("receipt"));
    assert_eq!(recovered.result.public.operation_id, id("operation"));
    assert_eq!(
        serde_json::to_value(recovered.result.next_allowed_at)?,
        v["next_allowed_at"]
    );
    assert_eq!(
        repo.place(actor, id("key"), pixel(1, 0), &keys()).await?,
        recovered.result
    );
    assert_eq!(counts(&pool, actor.event).await?, before);
    assert!(matches!(
        repo.place(actor, Uuid::new_v4(), pixel(2, 7), &keys())
            .await,
        Err(MuralError::Cooldown(Some(_)))
    ));
    terminal(&pool).await
}
