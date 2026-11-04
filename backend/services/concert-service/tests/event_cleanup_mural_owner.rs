//! Real mural effects and retained engine; authority is an explicit fixture.
//! Additive path shim until Root alone adds the eventual library export.
pub use concert_service::{event_cleanup_live, event_lifecycle};
#[path = "../src/event_cleanup_mural_owner/mod.rs"]
mod event_cleanup_mural_owner;
mod event_cleanup_mural_owner_support;
use async_trait::async_trait;
use concert_application::{commands::CommandError, event_cleanup::*, event_lifecycle::Actor};
use concert_domain::MAX_REVISION;
use concert_service::event_cleanup_live::*;
use event_cleanup_mural_owner::MuralFreezeOwner;
use event_cleanup_mural_owner_support::*;
use sqlx::{PgConnection, Row};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

#[test]
fn mural_owner_source_pin() {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    for bytes in [
        include_bytes!("../src/event_cleanup_mural_owner/mod.rs").as_slice(),
        include_bytes!("event_cleanup_mural_owner.rs").as_slice(),
        include_bytes!("event_cleanup_mural_owner_support/mod.rs").as_slice(),
    ] {
        h.update(bytes);
        h.update([0]);
    }
    assert_eq!(MuralFreezeOwner.owner(), Owner::Mural);
    println!("MURAL_OWNER_SOURCE_PIN={:x}", h.finalize());
}
#[tokio::test]
#[ignore = "owned frozen runner only"]
async fn mural_owner_prepare() {
    let pool = pool().await;
    let migrator = sqlx::migrate::Migrator::new(std::path::PathBuf::from(
        std::env::var("MCR_MURAL_OWNER_MIGRATIONS").expect("frozen migrations required"),
    ))
    .await
    .unwrap();
    migrator.run(&pool).await.unwrap();
    migration_runtime::verify_receipts(&pool, &migrator)
        .await
        .unwrap();
    let before: serde_json::Value = sqlx::query_scalar(
        "select jsonb_agg(to_jsonb(m) order by version) from _sqlx_migrations m",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    migrator.run(&pool).await.unwrap();
    let after: serde_json::Value = sqlx::query_scalar(
        "select jsonb_agg(to_jsonb(m) order by version) from _sqlx_migrations m",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before, after);
    sqlx::raw_sql("create table mural_owner_fixture_authority(event_id uuid,actor_id uuid,allowed boolean,primary key(event_id,actor_id));create table mural_owner_fixture_restart(actor_id uuid,event_id uuid,job_id uuid,receipt_id uuid,revision bigint);").execute(&pool).await.unwrap();
    println!("GENUINE_SQLX_MIGRATIONS={}", migrator.iter().count());
}
#[tokio::test]
#[ignore = "owned frozen runner only"]
async fn mural_owner_native_freezes_editable_paused_and_replays_exact_receipt() {
    let pool = pool().await;
    for state in ["EDITABLE", "PAUSED"] {
        let (actor, job) = seed(&pool, state, 7).await;
        let before = mural(&pool, actor.event).await;
        let private_before: serde_json::Value = sqlx::query_scalar(
            "select jsonb_agg(to_jsonb(l)) from concert_mural_limits l where event_id=$1",
        )
        .bind(actor.event)
        .fetch_one(&pool)
        .await
        .unwrap();
        let run = runtime(&pool, Arc::new(MuralFreezeOwner));
        let result = run.run_owner(actor, job, Owner::Mural).await.unwrap();
        assert_eq!(result.state, State::Complete);
        assert_eq!(result.processed, 1);
        let after = mural(&pool, actor.event).await;
        assert_eq!(after["state"], "FROZEN");
        assert_eq!(after["revision"], 8);
        assert_eq!(after["reset_revision"], 8);
        assert_ne!(before["snapshot_id"], after["snapshot_id"]);
        for field in [
            "pixels",
            "palette",
            "cooldown_ms",
            "retained_from_revision",
            "created_at",
        ] {
            assert_eq!(before[field], after[field]);
        }
        let private_after: serde_json::Value = sqlx::query_scalar(
            "select jsonb_agg(to_jsonb(l)) from concert_mural_limits l where event_id=$1",
        )
        .bind(actor.event)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(private_before, private_after);
        let r = receipt(&pool, job).await;
        let req = request(&pool, actor, job).await;
        assert_eq!(r.id.to_string(), after["snapshot_id"].as_str().unwrap());
        assert_eq!(r.revision, 8);
        assert_eq!(r.binding, req.binding());
        assert_eq!(r.operation, req.operation());
        assert!(run.ready(actor, job).await.unwrap());
        assert_eq!(
            run.run_owner(actor, job, Owner::Mural).await.unwrap().state,
            State::Complete
        );
        assert_eq!(receipt(&pool, job).await, r);
        assert_eq!(batches(&pool, job).await, 1);
        assert_eq!(after, mural(&pool, actor.event).await);
        let snapshot: serde_json::Value = sqlx::query_scalar(
            "select to_jsonb(s) from concert_mural_snapshots s where snapshot_id=$1",
        )
        .bind(r.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let keys: Vec<_> = snapshot
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            vec![
                "created_at",
                "event_id",
                "palette",
                "pixels",
                "revision",
                "snapshot_id",
                "state"
            ]
        );
        parent_pending(&pool, job).await;
    }
}
#[tokio::test]
#[ignore = "owned frozen runner only"]
async fn mural_owner_native_frozen_requires_current_matching_checkpoint_and_handles_exhaustion() {
    let pool = pool().await;
    for state in ["EDITABLE", "PAUSED", "FROZEN"] {
        let (actor, job) = seed(&pool, state, MAX_REVISION).await;
        let before = mural(&pool, actor.event).await;
        let result = runtime(&pool, Arc::new(MuralFreezeOwner))
            .run_owner(actor, job, Owner::Mural)
            .await
            .unwrap();
        assert_eq!(
            result.state,
            if state == "FROZEN" {
                State::Complete
            } else {
                State::PartialFailure
            }
        );
        assert_eq!(before, mural(&pool, actor.event).await);
    }
    for mismatch in [false, true] {
        let (actor, job) = seed(&pool, "FROZEN", 4).await;
        sqlx::query("delete from concert_mural_snapshots where event_id=$1")
            .bind(actor.event)
            .execute(&pool)
            .await
            .unwrap();
        if mismatch {
            // Same ID/revision, mismatched bytes: state flag and UUID are not proof.
            sqlx::query("insert into concert_mural_snapshots select snapshot_id,event_id,revision,state,set_byte(pixels,0,15),palette,snapshot_created_at from concert_mural_state where event_id=$1").bind(actor.event).execute(&pool).await.unwrap();
        }
        let before = mural(&pool, actor.event).await;
        assert_eq!(
            runtime(&pool, Arc::new(MuralFreezeOwner))
                .run_owner(actor, job, Owner::Mural)
                .await
                .unwrap()
                .state,
            State::PartialFailure
        );
        assert_eq!(before, mural(&pool, actor.event).await);
        assert_eq!(batches(&pool, job).await, 0);
    }
}

struct Interrupt(u8);
#[async_trait]
impl LocalOwner for Interrupt {
    fn owner(&self) -> Owner {
        Owner::Mural
    }
    async fn apply_in(
        &self,
        c: &mut PgConnection,
        r: &BatchRequest,
    ) -> Result<Outcome, CommandError> {
        let actual = MuralFreezeOwner.apply_in(c, r).await?;
        match self.0 {
            0 => Err(CommandError::Unavailable),
            1 => Ok(Outcome::Pending),
            _ => {
                tokio::time::sleep(Duration::from_millis(180)).await;
                Ok(actual)
            }
        }
    }
    async fn verify_in(
        &self,
        c: &mut PgConnection,
        r: &BatchRequest,
        receipt: Receipt,
    ) -> Result<(), CommandError> {
        MuralFreezeOwner.verify_in(c, r, receipt).await
    }
}
#[tokio::test]
#[ignore = "owned frozen runner only"]
async fn mural_owner_native_real_effects_roll_back_on_error_pending_and_expired_commit() {
    let pool = pool().await;
    for mode in 0..3 {
        let (actor, job) = seed(&pool, "EDITABLE", 3).await;
        let before = mural(&pool, actor.event).await;
        let run = CleanupRuntime::new(
            pool.clone(),
            Some(Arc::new(Authority(if mode == 2 { 120 } else { 10000 }))),
            Owners::new(vec![Arc::new(Interrupt(mode))]).unwrap(),
            2,
        )
        .unwrap();
        let result = run.run_owner(actor, job, Owner::Mural).await;
        if mode == 2 {
            assert!(result.is_err());
        } else {
            assert_eq!(
                result.unwrap().state,
                if mode == 0 {
                    State::PartialFailure
                } else {
                    State::Pending
                }
            );
        }
        assert_eq!(before, mural(&pool, actor.event).await);
        assert_eq!(batches(&pool, job).await, 0);
        let n: i64 =
            sqlx::query_scalar("select count(*) from concert_mural_snapshots where event_id=$1")
                .bind(actor.event)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(n, 1);
        assert_eq!(
            runtime(&pool, Arc::new(MuralFreezeOwner))
                .run_owner(actor, job, Owner::Mural)
                .await
                .unwrap()
                .state,
            State::Complete
        );
        parent_pending(&pool, job).await;
    }
}
#[tokio::test]
#[ignore = "owned frozen runner only"]
async fn mural_owner_native_receipt_scope_tamper_and_missing_authority_fail_closed() {
    let pool = pool().await;
    let (actor, job) = seed(&pool, "EDITABLE", 2).await;
    let req = request(&pool, actor, job).await;
    let mut c = pool.acquire().await.unwrap();
    assert!(matches!(
        MuralFreezeOwner.apply_in(&mut c, &req).await,
        Err(CommandError::InvalidInput)
    ));
    drop(c);
    let absent = CleanupRuntime::new(
        pool.clone(),
        None,
        Owners::new(vec![Arc::new(MuralFreezeOwner)]).unwrap(),
        2,
    )
    .unwrap();
    assert_eq!(
        absent
            .run_owner(actor, job, Owner::Mural)
            .await
            .unwrap_err(),
        CommandError::Unavailable
    );
    let run = runtime(&pool, Arc::new(MuralFreezeOwner));
    run.run_owner(actor, job, Owner::Mural).await.unwrap();
    let r = receipt(&pool, job).await;
    let mut tx = pool.begin().await.unwrap();
    fixture_deadline(&mut tx).await;
    MuralFreezeOwner.verify_in(&mut tx, &req, r).await.unwrap();
    use concert_service::event_lifecycle::{EffectKey, LifecycleOwners};
    let converged = ConvergedOwners::new(Owners::new(vec![Arc::new(MuralFreezeOwner)]).unwrap());
    converged
        .apply_in(
            &mut tx,
            EffectKey {
                event: actor.event,
                job,
                step: req.scope().step,
            },
            req.retention(),
        )
        .await
        .unwrap();
    for i in 0..5 {
        let mut bad = r;
        match i {
            0 => bad.id = Uuid::new_v4(),
            1 => bad.revision += 1,
            2 => bad.binding[0] ^= 1,
            3 => bad.operation = Uuid::new_v4(),
            _ => bad.processed = 0,
        };
        assert!(MuralFreezeOwner
            .verify_in(&mut tx, &req, bad)
            .await
            .is_err());
    }
    for i in 0..5 {
        let mut scope = req.scope();
        let mut retention = req.retention();
        let mut limit = req.limit();
        match i {
            0 => scope.actor = Uuid::new_v4(),
            1 => scope.event = Uuid::new_v4(),
            2 => scope.job = Uuid::new_v4(),
            3 => retention.policy_revision += 1,
            _ => limit += 1,
        }
        let bad = BatchRequest::new(scope, retention, 0, None, limit).unwrap();
        assert!(MuralFreezeOwner.verify_in(&mut tx, &bad, r).await.is_err());
    }
    tx.rollback().await.unwrap();
    sqlx::query("update mural_owner_fixture_authority set allowed=false where event_id=$1")
        .bind(actor.event)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        run.run_owner(actor, job, Owner::Mural).await.unwrap_err(),
        CommandError::Denied
    );
    assert_eq!(batches(&pool, job).await, 1);
}

struct PixelAuthority(Option<(Arc<tokio::sync::Notify>, Arc<tokio::sync::Notify>)>);
#[async_trait]
impl concert_service::adapters::out::repository::mural::MuralAuthority for PixelAuthority {
    async fn authorize_in(
        &self,
        _: &mut PgConnection,
        _: concert_application::mural::MuralActor,
    ) -> Result<(), concert_application::mural::MuralError> {
        if let Some((entered, release)) = &self.0 {
            entered.notify_one();
            release.notified().await;
        }
        Ok(())
    }
}
#[tokio::test]
#[ignore = "owned frozen runner only"]
async fn mural_owner_native_concurrent_production_pixel_writer_serializes_with_closure() {
    use concert_application::mural::MuralActor;
    use concert_domain::mural::PlacePixel;
    use concert_service::adapters::out::repository::mural::PgMuralRepository;
    let pool = pool().await;
    let actor = seed_open(&pool, "EDITABLE", 1).await;
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let repo = PgMuralRepository::new(
        pool.clone(),
        PixelAuthority(Some((entered.clone(), release.clone()))),
    );
    let pixel = tokio::spawn(async move {
        repo.place(
            MuralActor {
                actor: actor.actor,
                event: actor.event,
            },
            Uuid::new_v4(),
            PlacePixel {
                expected_revision: 1,
                x: 0,
                y: 0,
                color_index: 15,
            },
            &keys(),
        )
        .await
    });
    entered.notified().await;
    let close_pool = pool.clone();
    let closure = tokio::spawn(async move { close(&close_pool, actor).await });
    // The actual pixel adapter holds the event SHARE fence before calling the
    // fixture authority. Closure's UPDATE lock cannot pass it.
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert!(!closure.is_finished());
    release.notify_one();
    assert_eq!(pixel.await.unwrap().unwrap().public.revision, 2);
    let job = closure.await.unwrap();
    runtime(&pool, Arc::new(MuralFreezeOwner))
        .run_owner(actor, job, Owner::Mural)
        .await
        .unwrap();
    let revision:i64=sqlx::query_scalar("select revision from concert_mural_state where event_id=$1 and get_byte(pixels,0)=15 and reset_revision=3 and state='FROZEN'").bind(actor.event).fetch_one(&pool).await.unwrap();
    assert_eq!(revision, 3);
    let repo = PgMuralRepository::new(pool.clone(), PixelAuthority(None));
    assert!(repo
        .place(
            MuralActor {
                actor: actor.actor,
                event: actor.event
            },
            Uuid::new_v4(),
            PlacePixel {
                expected_revision: 3,
                x: 0,
                y: 0,
                color_index: 1
            },
            &keys()
        )
        .await
        .is_err());
    assert_eq!(receipt(&pool, job).await.revision, 3);
}
#[tokio::test]
#[ignore = "owned frozen runner only"]
async fn mural_owner_native_immutable_batch_and_missing_checkpoint_never_succeed() {
    let pool = pool().await;
    let (actor, job) = seed(&pool, "PAUSED", 3).await;
    let run = runtime(&pool, Arc::new(MuralFreezeOwner));
    run.run_owner(actor, job, Owner::Mural).await.unwrap();
    let r = receipt(&pool, job).await;
    for statement in [
        "update concert_event_cleanup_batches set owner_revision=owner_revision+1 where job_id=$1",
        "delete from concert_event_cleanup_batches where job_id=$1",
    ] {
        let error = sqlx::query(statement)
            .bind(job)
            .execute(&pool)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("append only"));
    }
    let error =
        sqlx::query("update concert_mural_snapshots set revision=revision+1 where snapshot_id=$1")
            .bind(r.id)
            .execute(&pool)
            .await
            .unwrap_err();
    assert!(error.to_string().contains("immutable mural"));
    // 0019 has UPDATE immutability but no DELETE guard. Detect lost native source
    // truth on replay; never certify the retained batch alone as an actual effect.
    sqlx::query("delete from concert_mural_snapshots where snapshot_id=$1")
        .bind(r.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(run.run_owner(actor, job, Owner::Mural).await.is_err());
    assert!(run.ready(actor, job).await.is_err());
    assert_eq!(receipt(&pool, job).await, r);
    parent_pending(&pool, job).await;
}
#[tokio::test]
#[ignore = "owned frozen runner only"]
async fn mural_owner_restart_prepare() {
    let pool = pool().await;
    let (actor, job) = seed(&pool, "EDITABLE", 9).await;
    runtime(&pool, Arc::new(MuralFreezeOwner))
        .run_owner(actor, job, Owner::Mural)
        .await
        .unwrap();
    let r = receipt(&pool, job).await;
    sqlx::query("insert into mural_owner_fixture_restart values($1,$2,$3,$4,$5)")
        .bind(actor.actor)
        .bind(actor.event)
        .bind(job)
        .bind(r.id)
        .bind(r.revision)
        .execute(&pool)
        .await
        .unwrap();
}
#[tokio::test]
#[ignore = "owned frozen runner only"]
async fn mural_owner_restart_verify() {
    let pool = pool().await;
    let row = sqlx::query("select * from mural_owner_fixture_restart")
        .fetch_one(&pool)
        .await
        .unwrap();
    let actor = Actor {
        actor: row.get("actor_id"),
        event: row.get("event_id"),
    };
    let job = row.get("job_id");
    let run = runtime(&pool, Arc::new(MuralFreezeOwner));
    assert!(run.ready(actor, job).await.unwrap());
    assert_eq!(
        run.run_owner(actor, job, Owner::Mural).await.unwrap().state,
        State::Complete
    );
    let r = receipt(&pool, job).await;
    assert_eq!(r.id, row.get::<Uuid, _>("receipt_id"));
    assert_eq!(r.revision, row.get::<i64, _>("revision"));
    assert_eq!(batches(&pool, job).await, 1);
    parent_pending(&pool, job).await;
}
