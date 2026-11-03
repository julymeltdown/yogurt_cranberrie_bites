//! Real SQL component qualification, not production Auth or native-send acceptance.
#[path = "event_lifecycle_support/read_only.rs"]
mod fixture;
#[path = "support/event_lifecycle/mod.rs"]
mod support;
use chrono::{Duration, Utc};
use concert_application::{
    commands::*,
    event_lifecycle::{Actor, CloseCommand},
    world_ticket::*,
};
use concert_domain::event_lifecycle::{JobState, Step};
use concert_service::{
    event_lifecycle::{read_only::apply_read_only_in, *},
    world::PgWorldRepository,
};
use fixture::*;
use sqlx::{PgPool, Row};
use support::{actor, keys, pool, Authority};
use uuid::Uuid;
fn start() -> CloseCommand {
    CloseCommand::Start {
        expected_revision: 1,
        reason: "Real READ_ONLY component fixture".into(),
    }
}
fn service(pool: &PgPool, fail_after: bool) -> PgEventLifecycle<Authority, ActualReadOnly> {
    PgEventLifecycle::new(
        pool.clone(),
        Authority {
            delay: Duration::hours(1),
        },
        ActualReadOnly {
            fail_after,
            expire_after: false,
        },
    )
}
async fn barrier_count(pool: &PgPool, event: Uuid) -> i64 {
    sqlx::query_scalar("select (select count(*) from concert_closure_read_only_barriers where event_id=$1)+(select count(*) from concert_closure_community_barriers where event_id=$1)").bind(event).fetch_one(pool).await.unwrap()
}
#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture and actual World API"]
async fn event_lifecycle_read_only_actual_components_replay_and_foreign_isolation(
) -> anyhow::Result<()> {
    let pool = pool().await;
    let actor = actor(&pool).await;
    let seeded = seed(&pool, actor).await?;
    let foreign = support::actor(&pool).await;
    seed(&pool, foreign).await?;
    let before = snapshot(&pool, foreign.event).await?;
    let service = service(&pool, false);
    let key = Uuid::new_v4();
    let admission = service.command(actor, key, start(), &keys()).await?;
    assert_eq!(admission.state, ReceiptState::Succeeded);
    let job = admission.reference.unwrap().resource;
    assert_eq!(service.job(actor, job).await?.state, JobState::Pending);
    let party: serde_json::Value = sqlx::query_scalar(
        "select aggregate from concert_community_parties where event_id=$1 and party_id=$2",
    )
    .bind(actor.event)
    .bind(seeded.party)
    .fetch_one(&pool)
    .await?;
    assert_eq!(party["state"], "CANCELLED");
    assert_eq!(party["applications"][0]["state"], "CANCELLED");
    assert_eq!(party["audience_revision"], 2);
    let gift: serde_json::Value = sqlx::query_scalar(
        "select aggregate from concert_community_giveaways where event_id=$1 and giveaway_id=$2",
    )
    .bind(actor.event)
    .bind(seeded.gift)
    .fetch_one(&pool)
    .await?;
    assert_eq!(gift["state"], "CANCELLED");
    assert_eq!(gift["reserved_unclaimed"], 0);
    assert_eq!(gift["quantity_claimed"], 1);
    assert!(gift["meeting"].is_null());
    let claimed: String = sqlx::query_scalar(
        "select state from concert_community_reservations where event_id=$1 and reservation_id=$2",
    )
    .bind(actor.event)
    .bind(seeded.claimed)
    .fetch_one(&pool)
    .await?;
    assert_eq!(claimed, "CLAIMED");
    let held: String = sqlx::query_scalar(
        "select state from concert_community_reservations where event_id=$1 and reservation_id=$2",
    )
    .bind(actor.event)
    .bind(seeded.held)
    .fetch_one(&pool)
    .await?;
    assert_eq!(held, "CANCELLED");
    let secrets: i64 = sqlx::query_scalar(
        "select count(*) from concert_community_pickup_secrets where event_id=$1",
    )
    .bind(actor.event)
    .fetch_one(&pool)
    .await?;
    assert_eq!(secrets, 0);
    let active:i64=sqlx::query_scalar("select (select count(*) from concert_world_memberships where event_id=$1 and not disconnected and lease_until>clock_timestamp())+(select count(*) from concert_world_owner_leases where event_id=$1 and lease_until>clock_timestamp())").bind(actor.event).fetch_one(&pool).await?;
    assert_eq!(active, 0);
    assert!(
        !matches!(
            PgWorldRepository::new(pool.clone())
                .consume(ConsumeClaim {
                    digest: seeded.record.digest,
                    connection_id: ConnectionId::from_uuid(Uuid::new_v4())?,
                    requested: seeded.record.pin.clone(),
                    now: Utc::now(),
                })
                .await,
            Ok(ConsumeOutcome::Consumed(_))
        ),
        "unconsumed ticket must also lose authority"
    );
    assert_eq!(barrier_count(&pool, actor.event).await, 2);
    let exact = snapshot(&pool, actor.event).await?;
    assert_eq!(
        admission,
        service.command(actor, key, start(), &keys()).await?
    );
    assert_eq!(exact, snapshot(&pool, actor.event).await?);
    assert_eq!(before, snapshot(&pool, foreign.event).await?);
    let row=sqlx::query("select world_receipt_id,world_revision,community_receipt_id from concert_closure_read_only_barriers where job_id=$1").bind(job).fetch_one(&pool).await?;
    assert!(!row.get::<Uuid, _>("world_receipt_id").is_nil());
    assert!(row.get::<i64, _>("world_revision") > 0);
    assert!(!row.get::<Uuid, _>("community_receipt_id").is_nil());
    Ok(())
}
#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture and actual World API"]
async fn event_lifecycle_read_only_owner_failure_rolls_back_every_sql_effect() -> anyhow::Result<()>
{
    let pool = pool().await;
    let actor = actor(&pool).await;
    seed(&pool, actor).await?;
    let before = snapshot(&pool, actor.event).await?;
    let key = Uuid::new_v4();
    assert_eq!(
        service(&pool, true)
            .command(actor, key, start(), &keys())
            .await
            .unwrap_err(),
        CommandError::Unavailable
    );
    assert_eq!(before, snapshot(&pool, actor.event).await?);
    assert_eq!(barrier_count(&pool, actor.event).await, 0);
    let jobs: i64 =
        sqlx::query_scalar("select count(*) from concert_closure_jobs where event_id=$1")
            .bind(actor.event)
            .fetch_one(&pool)
            .await?;
    assert_eq!(jobs, 0);
    let receipt: i64 =
        sqlx::query_scalar("select count(*) from concert_command_receipts where event_id=$1")
            .bind(actor.event)
            .fetch_one(&pool)
            .await?;
    assert_eq!(receipt, 0);
    let outbox: i64 = sqlx::query_scalar("select count(*) from concert_outbox where event_id=$1")
        .bind(actor.event)
        .fetch_one(&pool)
        .await?;
    assert_eq!(outbox, 0);
    // A failed SQL attempt did not consume its key or leave component evidence.
    assert_eq!(
        service(&pool, false)
            .command(actor, key, start(), &keys())
            .await?
            .state,
        ReceiptState::Succeeded
    );
    Ok(())
}
#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture and actual World API"]
async fn event_lifecycle_read_only_preconsumed_claim_and_new_lease_are_denied() -> anyhow::Result<()>
{
    let pool = pool().await;
    let actor = actor(&pool).await;
    let seeded = seed(&pool, actor).await?;
    let repository = PgWorldRepository::new(pool.clone());
    let consumed = repository
        .consume(ConsumeClaim {
            digest: seeded.record.digest,
            connection_id: ConnectionId::from_uuid(Uuid::new_v4())?,
            requested: seeded.record.pin.clone(),
            now: Utc::now(),
        })
        .await?;
    let ConsumeOutcome::Consumed(claim) = consumed else {
        panic!("fixture must hold a real preconsumed ticket")
    };
    service(&pool, false)
        .command(actor, Uuid::new_v4(), start(), &keys())
        .await?;
    assert!(!matches!(
        repository.admit(&seeded.owner, &claim, Utc::now()).await,
        Ok(AdmissionOutcome::Admitted(_) | AdmissionOutcome::Replaced(_))
    ));
    assert!(repository
        .claim_owner(
            seeded.record.pin.event_id,
            RoomId::from_uuid(seeded.room),
            &seeded.owner,
            Utc::now(),
            Duration::seconds(5)
        )
        .await
        .is_err());
    assert!(repository
        .renew_owner(
            seeded.record.pin.event_id,
            RoomId::from_uuid(seeded.room),
            &seeded.owner,
            Utc::now(),
            Duration::seconds(5)
        )
        .await
        .is_err());
    let mut later = seeded.record.clone();
    later.digest = TicketDigest::from_bytes([0x73; 32])?;
    assert!(!matches!(
        repository.issue(later).await,
        Ok(IssueOutcome::Issued)
    ));
    Ok(())
}
#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture and actual World API"]
async fn event_lifecycle_read_only_concurrent_start_single_component_receipt() -> anyhow::Result<()>
{
    let pool = pool().await;
    let actor = actor(&pool).await;
    seed(&pool, actor).await?;
    let service = service(&pool, false);
    let key = Uuid::new_v4();
    let a = keys();
    let b = keys();
    let (one, two) = tokio::join!(
        service.command(actor, key, start(), &a),
        service.command(actor, key, start(), &b)
    );
    assert_eq!(one?, two?);
    assert_eq!(barrier_count(&pool, actor.event).await, 2);
    let count: i64 = sqlx::query_scalar(
        "select count(*) from concert_closure_effects where event_id=$1 and step='READ_ONLY'",
    )
    .bind(actor.event)
    .fetch_one(&pool)
    .await?;
    assert_eq!(count, 1);
    Ok(())
}
#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture and actual World API"]
async fn event_lifecycle_read_only_invalid_scope_and_receipt_immutability() -> anyhow::Result<()> {
    let pool = pool().await;
    let actor = actor(&pool).await;
    seed(&pool, actor).await?;
    let admission = service(&pool, false)
        .command(actor, Uuid::new_v4(), start(), &keys())
        .await?;
    let job = admission.reference.unwrap().resource;
    let key = EffectKey {
        event: actor.event,
        job,
        step: Step::ReadOnly,
    };
    let mut autocommit = pool.acquire().await?;
    assert_eq!(
        apply_read_only_in(&mut autocommit, key).await.unwrap_err(),
        CommandError::InvalidInput
    );
    assert_eq!(
        concert_service::community::closure::fence_event_read_only_in(&mut autocommit, key)
            .await
            .unwrap_err(),
        CommandError::InvalidInput
    );
    drop(autocommit);
    let mut tx = pool.begin().await?;
    sqlx::query("select set_config('mcr.lifecycle_authority_until',(clock_timestamp()+interval '5 seconds')::text,true)").execute(&mut *tx).await?;
    let first = apply_read_only_in(&mut tx, key).await?;
    let second = apply_read_only_in(&mut tx, key).await?;
    assert_eq!(first.receipt, second.receipt);
    assert!(apply_read_only_in(
        &mut tx,
        EffectKey {
            event: Uuid::new_v4(),
            ..key
        }
    )
    .await
    .is_err());
    assert!(apply_read_only_in(
        &mut tx,
        EffectKey {
            step: Step::Cleanup,
            ..key
        }
    )
    .await
    .is_err());
    tx.rollback().await?;
    assert!(
        sqlx::query("delete from concert_closure_read_only_barriers where job_id=$1")
            .bind(job)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query(
        "update concert_closure_community_barriers set parties_cancelled=0 where job_id=$1"
    )
    .bind(job)
    .execute(&pool)
    .await
    .is_err());
    Ok(())
}
#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture and actual World API"]
async fn event_lifecycle_read_only_publication_share_lock_blocks_start() -> anyhow::Result<()> {
    let pool = pool().await;
    let actor = actor(&pool).await;
    seed(&pool, actor).await?;
    let mut publication = pool.begin().await?;
    sqlx::query("select event_id from concert_events where event_id=$1 for share")
        .bind(actor.event)
        .fetch_one(&mut *publication)
        .await?;
    let service = service(&pool, false);
    let key = Uuid::new_v4();
    let keys = keys();
    let attempt = service.command(actor, key, start(), &keys);
    tokio::pin!(attempt);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut attempt)
            .await
            .is_err()
    );
    // SQL serialization only. The actual ProductionOwners native-send lease
    // disable-before-rollback test remains the World owner's qualification gate.
    publication.rollback().await?;
    assert_eq!(attempt.await?.state, ReceiptState::Succeeded);
    Ok(())
}
#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture and actual World API"]
async fn event_lifecycle_read_only_restart_seed() -> anyhow::Result<()> {
    let pool = pool().await;
    let actor = actor(&pool).await;
    seed(&pool, actor).await?;
    let key = Uuid::new_v4();
    let admission = service(&pool, false)
        .command(actor, key, start(), &keys())
        .await?;
    sqlx::query("create table be41_read_only_restart_fixture(actor_id uuid,event_id uuid,start_key uuid,job_id uuid,receipt_id uuid)").execute(&pool).await?;
    sqlx::query("insert into be41_read_only_restart_fixture values($1,$2,$3,$4,$5)")
        .bind(actor.actor)
        .bind(actor.event)
        .bind(key)
        .bind(admission.reference.unwrap().resource)
        .bind(admission.id)
        .execute(&pool)
        .await?;
    Ok(())
}
#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture and actual World API"]
async fn event_lifecycle_read_only_restart_resume() -> anyhow::Result<()> {
    let pool = pool().await;
    let row = sqlx::query("select * from be41_read_only_restart_fixture")
        .fetch_one(&pool)
        .await?;
    let actor = Actor {
        actor: row.get("actor_id"),
        event: row.get("event_id"),
    };
    let service = service(&pool, false);
    let replay = service
        .command(actor, row.get("start_key"), start(), &keys())
        .await?;
    assert_eq!(replay.id, row.get::<Uuid, _>("receipt_id"));
    assert_eq!(
        replay.reference.unwrap().resource,
        row.get::<Uuid, _>("job_id")
    );
    assert_eq!(barrier_count(&pool, actor.event).await, 2);
    assert_eq!(
        service.job(actor, row.get("job_id")).await?.state,
        JobState::Pending
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture and actual World API"]
async fn event_lifecycle_read_only_community_failure_rolls_back_world() -> anyhow::Result<()> {
    let pool = pool().await;
    let actor = actor(&pool).await;
    let seeded = seed(&pool, actor).await?;
    let before = snapshot(&pool, actor.event).await?;
    // Unique fixture-owned trigger fails after World SQL but before community save.
    let name = format!("be41_failure_{}", Uuid::new_v4().simple());
    sqlx::raw_sql(&format!("CREATE FUNCTION {name}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.party_id='{}'::uuid AND NEW.state='CANCELLED' THEN RAISE EXCEPTION 'owned community fault'; END IF; RETURN NEW; END $$; CREATE TRIGGER {name} BEFORE UPDATE ON concert_community_parties FOR EACH ROW EXECUTE FUNCTION {name}();",seeded.party)).execute(&pool).await?;
    assert!(service(&pool, false)
        .command(actor, Uuid::new_v4(), start(), &keys())
        .await
        .is_err());
    assert_eq!(before, snapshot(&pool, actor.event).await?);
    assert_eq!(barrier_count(&pool, actor.event).await, 0);
    sqlx::raw_sql(&format!(
        "DROP TRIGGER {name} ON concert_community_parties; DROP FUNCTION {name}();"
    ))
    .execute(&pool)
    .await?;
    Ok(())
}
#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture and actual World API"]
async fn event_lifecycle_read_only_commit_deadline_rolls_back_components() -> anyhow::Result<()> {
    let pool = pool().await;
    let actor = actor(&pool).await;
    seed(&pool, actor).await?;
    let before = snapshot(&pool, actor.event).await?;
    let service = PgEventLifecycle::new(
        pool.clone(),
        Authority {
            delay: Duration::hours(1),
        },
        ActualReadOnly {
            fail_after: false,
            expire_after: true,
        },
    );
    assert!(service
        .command(actor, Uuid::new_v4(), start(), &keys())
        .await
        .is_err());
    assert_eq!(before, snapshot(&pool, actor.event).await?);
    assert_eq!(barrier_count(&pool, actor.event).await, 0);
    Ok(())
}
