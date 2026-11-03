//! Native adapter qualification. Always requires a marker-owned isolated MCR database.
use async_trait::async_trait;
use concert_application::{commands::*, mural::*};
use concert_domain::mural::*;
use concert_service::{adapters::out::repository::mural::*, world::mural_subscription::*};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgConnection, PgPool, Row,
};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    str::FromStr,
};
use uuid::Uuid;

async fn pool() -> anyhow::Result<PgPool> {
    let root = PathBuf::from(std::env::var("MURAL_TEST_OWNER_ROOT")?);
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        info.is_dir() && !info.file_type().is_symlink() && info.permissions().mode() & 0o077 == 0,
        "private fixture root"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino()) == std::env::var("MURAL_TEST_OWNER_IDENTITY")?,
        "fixture identity"
    );
    let marker: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("mcr-owner.json"))?)?;
    anyhow::ensure!(
        marker["token"].as_str() == Some(std::env::var("MURAL_TEST_OWNER_TOKEN")?.as_str()),
        "fixture token"
    );
    let options = PgConnectOptions::from_str(&std::env::var("MURAL_TEST_DATABASE_URL")?)?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_mural")
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "foreign database"
    );
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await?;
    let actual: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        actual == root.join("data").to_string_lossy(),
        "foreign data directory"
    );
    Ok(pool)
}
struct SyntheticAuthority;
#[async_trait]
impl MuralAuthority for SyntheticAuthority {
    async fn authorize_in(&self, c: &mut PgConnection, a: MuralActor) -> Result<(), MuralError> {
        let row = sqlx::query(
            "select members,unavailable from mural_fixture_authority where event_id=$1 for share",
        )
        .bind(a.event)
        .fetch_optional(c)
        .await
        .map_err(|_| MuralError::PolicyUnavailable)?
        .ok_or(MuralError::Denied)?;
        if row.get::<bool, _>("unavailable") {
            return Err(MuralError::PolicyUnavailable);
        }
        if !row.get::<Vec<Uuid>, _>("members").contains(&a.actor) {
            return Err(MuralError::Denied);
        }
        Ok(())
    }
}
async fn setup(pool: &PgPool, cooldown: i64) -> anyhow::Result<(MuralActor, MuralActor)> {
    let event = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let peer = Uuid::new_v4();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Synthetic mural','OPEN')").bind(event).bind(format!("mural-{}",event.simple())).execute(pool).await?;
    sqlx::query("insert into concert_mural_state(event_id,snapshot_id,state,pixels,palette,cooldown_ms) values($1,$2,'EDITABLE',$3,$4,$5)").bind(event).bind(Uuid::new_v4()).bind(vec![0u8;PIXELS]).bind(vec!["#123ABC";16]).bind(cooldown).execute(pool).await?;
    sqlx::query("create table if not exists mural_fixture_authority(event_id uuid primary key,members uuid[] not null,unavailable boolean not null default false)").execute(pool).await?;
    sqlx::query("insert into mural_fixture_authority values($1,$2,false)")
        .bind(event)
        .bind(vec![actor, peer])
        .execute(pool)
        .await?;
    Ok((
        MuralActor { event, actor },
        MuralActor { event, actor: peer },
    ))
}
fn keys() -> FingerprintKeys {
    FingerprintKeys::new(SigningKey::new("mural-native", &[41; 32]).unwrap(), vec![]).unwrap()
}
fn pixel(revision: i64, x: i32, color: i32) -> PlacePixel {
    PlacePixel {
        expected_revision: revision,
        x,
        y: 0,
        color_index: color,
    }
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
async fn parallel_cooldown_receipt_recovery_and_private_projection() -> anyhow::Result<()> {
    let pool = pool().await?;
    let (actor, peer) = setup(&pool, 3600000).await?;
    let repo = PgMuralRepository::new(pool.clone(), SyntheticAuthority);
    let keys = keys();
    let key = Uuid::new_v4();
    let (a, b) = tokio::join!(
        repo.place(actor, key, pixel(1, 0, 3), &keys),
        repo.place(actor, key, pixel(1, 0, 3), &keys)
    );
    let first = a?;
    assert_eq!(first, b?);
    assert_eq!(first.public.revision, 2);
    assert_eq!(repo.recover(actor, key).await?, Some(first.clone()));
    assert_eq!(repo.recover(peer, key).await?, None);
    assert_eq!(repo.recover(actor, Uuid::new_v4()).await?, None);
    assert_eq!(
        repo.place(actor, key, pixel(1, 0, 4), &keys).await,
        Err(MuralError::Command(CommandError::Conflict))
    );
    let (a, b) = tokio::join!(
        repo.place(actor, Uuid::new_v4(), pixel(2, 1, 4), &keys),
        repo.place(actor, Uuid::new_v4(), pixel(2, 2, 5), &keys)
    );
    assert!(matches!(a, Err(MuralError::Cooldown(Some(_)))));
    assert!(matches!(b, Err(MuralError::Cooldown(Some(_)))));
    assert_eq!(counts(&pool, actor.event).await?, (1, 1, 3, 1));
    let (fresh, _) = setup(&pool, 3600000).await?;
    let (left, right) = tokio::join!(
        repo.place(fresh, Uuid::new_v4(), pixel(1, 0, 3), &keys),
        repo.place(fresh, Uuid::new_v4(), pixel(1, 1, 4), &keys)
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    assert!(matches!(left, Ok(_) | Err(MuralError::Cooldown(Some(_)))));
    assert!(matches!(right, Ok(_) | Err(MuralError::Cooldown(Some(_)))));
    assert_eq!(counts(&pool, fresh.event).await?, (1, 1, 2, 1));

    let snapshot = repo.snapshot(peer).await?;
    assert_eq!(snapshot.pixels[0], 3);
    assert_eq!(snapshot.next_allowed_at, None);
    let own = repo.snapshot(actor).await?;
    let checkpoint = sqlx::query(
        "select revision,pixels from concert_mural_snapshots where snapshot_id=$1 and event_id=$2",
    )
    .bind(own.snapshot_id)
    .bind(actor.event)
    .fetch_one(&pool)
    .await?;
    assert_eq!(checkpoint.get::<i64, _>("revision"), own.revision);
    assert_eq!(checkpoint.get::<Vec<u8>, _>("pixels"), own.pixels);
    assert!(
        sqlx::query("update concert_mural_snapshots set pixels=$2 where snapshot_id=$1")
            .bind(own.snapshot_id)
            .bind(vec![0u8; PIXELS])
            .execute(&pool)
            .await
            .is_err()
    );

    assert_eq!(own.next_allowed_at, Some(first.next_allowed_at));
    let public = serde_json::to_value(repo.delta(peer, 1).await?)?;
    assert!(!public.to_string().contains("actor"));
    assert!(!public.to_string().contains("next_allowed_at"));
    let op = serde_json::to_value(&first)?;
    assert_eq!(op.as_object().unwrap().len(), 8);
    // Revocation blocks both queued reads and successful receipt recovery.
    sqlx::query("update mural_fixture_authority set members=$2 where event_id=$1")
        .bind(actor.event)
        .bind(vec![peer.actor])
        .execute(&pool)
        .await?;
    assert!(repo.recover(actor, key).await.is_err());
    assert_eq!(repo.snapshot(actor).await, Err(MuralError::Denied));
    sqlx::query("update mural_fixture_authority set unavailable=true where event_id=$1")
        .bind(actor.event)
        .execute(&pool)
        .await?;
    assert_eq!(
        repo.snapshot(peer).await,
        Err(MuralError::PolicyUnavailable)
    );
    Ok(())
}
#[tokio::test]
async fn concurrent_actors_retry_conflicts_preserving_strict_revision_order() -> anyhow::Result<()>
{
    let pool = pool().await?;
    let (a, b) = setup(&pool, 0).await?;
    let repo = PgMuralRepository::new(pool.clone(), SyntheticAuthority);
    let keys = keys();
    let (left, right) = tokio::join!(
        repo.place(a, Uuid::new_v4(), pixel(1, 1, 3), &keys),
        repo.place(b, Uuid::new_v4(), pixel(1, 2, 4), &keys)
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    let loser = if left.is_err() { a } else { b };
    assert!(matches!(left, Ok(_) | Err(MuralError::RevisionConflict)));
    assert!(matches!(right, Ok(_) | Err(MuralError::RevisionConflict)));
    repo.place(loser, Uuid::new_v4(), pixel(2, 3, 5), &keys)
        .await?;
    let delta = repo.delta(a, 1).await?;
    assert!(delta.contiguous());
    assert_eq!(
        delta
            .operations
            .iter()
            .map(|v| v.revision)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert_eq!(counts(&pool, a.event).await?, (2, 2, 3, 2));
    Ok(())
}
#[tokio::test]
async fn invalid_inputs_and_persisted_failures_never_mutate_or_emit_outbox() -> anyhow::Result<()> {
    let pool = pool().await?;
    let (a, _) = setup(&pool, 3600000).await?;
    let repo = PgMuralRepository::new(pool.clone(), SyntheticAuthority);
    let keys = keys();
    for bad in [
        PlacePixel {
            y: -1,
            ..pixel(1, 0, 1)
        },
        PlacePixel {
            y: 128,
            ..pixel(1, 0, 1)
        },
        pixel(1, -1, 1),
        pixel(1, 128, 1),
        pixel(1, 0, -1),
        pixel(1, 0, 16),
        pixel(0, 0, 1),
    ] {
        assert_eq!(
            repo.place(a, Uuid::new_v4(), bad, &keys).await,
            Err(MuralError::InvalidInput)
        );
    }
    assert_eq!(counts(&pool, a.event).await?, (0, 0, 0, 0));
    repo.place(a, Uuid::new_v4(), pixel(1, 0, 1), &keys).await?;
    let key = Uuid::new_v4();
    assert!(matches!(
        repo.place(a, key, pixel(2, 1, 2), &keys).await,
        Err(MuralError::Cooldown(Some(_)))
    ));
    assert_eq!(
        repo.place(a, key, pixel(2, 1, 2), &keys).await,
        Err(MuralError::Cooldown(None)),
        "replay never fabricates original retry timestamp"
    );
    assert_eq!(repo.recover(a, key).await?, None);
    for (state, reason, error) in [
        ("PAUSED", "MURAL_PAUSED", MuralError::Paused),
        ("FROZEN", "MURAL_FROZEN", MuralError::Frozen),
    ] {
        let (actor, _) = setup(&pool, 0).await?;
        sqlx::query(
            "update concert_mural_state set state=$2,revision=2,reset_revision=2,snapshot_id=gen_random_uuid(),snapshot_created_at=clock_timestamp() where event_id=$1",
        )
        .bind(actor.event)
        .bind(state)
        .execute(&pool)
        .await?;
        let key = Uuid::new_v4();
        for _ in 0..2 {
            assert_eq!(
                repo.place(actor, key, pixel(2, 0, 2), &keys).await,
                Err(error.clone())
            );
        }
        let actual: String = sqlx::query_scalar(
            "select failure_reason from concert_command_receipts where event_id=$1",
        )
        .bind(actor.event)
        .fetch_one(&pool)
        .await?;
        assert_eq!(actual, reason);
        assert_eq!(counts(&pool, actor.event).await?, (0, 0, 1, 0));
    }
    let (actor, _) = setup(&pool, 0).await?;
    sqlx::query("insert into concert_mural_locks values($1,$2,0,0,1,1)")
        .bind(actor.event)
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await?;
    let key = Uuid::new_v4();
    for _ in 0..2 {
        assert_eq!(
            repo.place(actor, key, pixel(1, 0, 2), &keys).await,
            Err(MuralError::RegionLocked)
        );
    }
    sqlx::query("delete from concert_mural_locks where event_id=$1")
        .bind(actor.event)
        .execute(&pool)
        .await?;
    assert_eq!(
        repo.place(actor, key, pixel(1, 0, 2), &keys).await,
        Err(MuralError::RegionLocked),
        "terminal failure remains terminal after unlock"
    );
    assert_eq!(counts(&pool, actor.event).await?, (0, 0, 1, 0));
    let (actor, _) = setup(&pool, 0).await?;
    sqlx::query(
        "update concert_events set phase='READ_ONLY',revision=revision+1 where event_id=$1",
    )
    .bind(actor.event)
    .execute(&pool)
    .await?;
    assert_eq!(
        repo.place(actor, Uuid::new_v4(), pixel(1, 0, 1), &keys)
            .await,
        Err(MuralError::EventReadOnly)
    );
    Ok(())
}
#[tokio::test]
async fn forced_outbox_failure_rolls_back_pixel_revision_cooldown_receipt_and_budget(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let (a, _) = setup(&pool, 0).await?;
    let repo = PgMuralRepository::new(pool.clone(), SyntheticAuthority);
    let keys = keys();
    // A fixture-specific constraint fails after all aggregate effects, at outbox insert.
    sqlx::query(&format!("alter table concert_outbox add constraint mural_fixture_failure check(event_id<>'{}'::uuid)",a.event)).execute(&pool).await?;
    let key = Uuid::new_v4();
    let before = repo.snapshot(a).await?;
    let budget: i64 = sqlx::query_scalar("select receipts_used from concert_command_budget")
        .fetch_one(&pool)
        .await?;
    assert!(repo.place(a, key, pixel(1, 0, 8), &keys).await.is_err());
    assert_eq!(counts(&pool, a.event).await?, (0, 0, 0, 0));
    let after = repo.snapshot(a).await?;
    assert_eq!(before.pixels, after.pixels);
    assert_eq!(before.revision, after.revision);
    assert_eq!(before.snapshot_id, after.snapshot_id);
    assert_eq!(after.next_allowed_at, None);
    let actual: i64 = sqlx::query_scalar("select receipts_used from concert_command_budget")
        .fetch_one(&pool)
        .await?;
    assert_eq!(budget, actual);
    sqlx::query("alter table concert_outbox drop constraint mural_fixture_failure")
        .execute(&pool)
        .await?;
    assert_eq!(repo.recover(a, key).await?, None);
    repo.place(a, key, pixel(1, 0, 8), &keys).await?;
    assert_eq!(counts(&pool, a.event).await?, (1, 1, 1, 1));
    Ok(())
}
#[tokio::test]
async fn new_and_reconnecting_subscribers_converge_with_retention_restore_and_backpressure(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let (a, b) = setup(&pool, 0).await?;
    let repo = PgMuralRepository::new(pool.clone(), SyntheticAuthority);
    let keys = keys();
    let epoch = Uuid::new_v4();
    let mut slow = MuralSubscription::new(a, epoch, 1)?;
    slow.poll(&repo, epoch).await?;
    repo.place(a, Uuid::new_v4(), pixel(1, 4, 6), &keys).await?;
    slow.poll(&repo, epoch).await?;
    assert_eq!(slow.queued(), 1);
    let snap = match slow.next(&repo, epoch).await?.unwrap() {
        MuralDelivery::Snapshot { value, reset } => {
            assert!(reset);
            value
        }
        _ => panic!("expected resnapshot"),
    };
    assert_eq!(snap.pixels, repo.snapshot(b).await?.pixels);
    let mut newcomer = MuralSubscription::new(b, epoch, 2)?;
    newcomer.poll(&repo, epoch).await?;
    let joined = match newcomer.next(&repo, epoch).await?.unwrap() {
        MuralDelivery::Snapshot { value, .. } => value,
        _ => panic!("new subscriber must snapshot"),
    };
    assert_eq!(snap.pixels, joined.pixels);
    assert_eq!(snap.revision, joined.revision);

    repo.place(b, Uuid::new_v4(), pixel(2, 5, 7), &keys).await?;
    slow.poll(&repo, epoch).await?;
    let delta = match slow.next(&repo, epoch).await?.unwrap() {
        MuralDelivery::Delta(value) => value,
        _ => panic!("expected delta"),
    };
    let mut pixels = snap.pixels;
    for op in delta.operations {
        pixels[(op.y * 128 + op.x) as usize] = op.color_index as u8;
    }
    assert_eq!(pixels, repo.snapshot(a).await?.pixels);
    assert!(repo.delta(a, 4).await?.reset_required);
    // BE40 simulation only: restore creates NEW revision, reset barrier, snapshot ID.
    sqlx::query("update concert_mural_state set revision=revision+1,reset_revision=revision+1,pixels=$2,snapshot_id=$3,snapshot_created_at=clock_timestamp() where event_id=$1").bind(a.event).bind(vec![2u8;PIXELS]).bind(Uuid::new_v4()).execute(&pool).await?;
    assert!(repo.delta(a, 3).await?.reset_required);
    slow.poll(&repo, epoch).await?;
    assert!(matches!(
        slow.next(&repo, epoch).await?,
        Some(MuralDelivery::Snapshot { reset: true, .. })
    ));
    repo.place(a, Uuid::new_v4(), pixel(4, 6, 8), &keys).await?;
    // Retention advances with an operator/control revision; never deletes original own-result evidence.
    sqlx::query("update concert_mural_state set revision=revision+1,reset_revision=revision+1,retained_from_revision=revision,snapshot_id=gen_random_uuid(),snapshot_created_at=clock_timestamp() where event_id=$1").bind(a.event).execute(&pool).await?;
    assert!(repo.delta(a, 4).await?.reset_required);
    let next_epoch = Uuid::new_v4();
    slow.poll(&repo, next_epoch).await?;
    assert!(matches!(
        slow.next(&repo, next_epoch).await?,
        Some(MuralDelivery::Snapshot { reset: true, .. })
    ));
    slow.poll(&repo, next_epoch).await?;
    sqlx::query("update mural_fixture_authority set members='{}' where event_id=$1")
        .bind(a.event)
        .execute(&pool)
        .await?;
    assert_eq!(
        slow.next(&repo, next_epoch).await.unwrap_err(),
        MuralError::Denied
    );
    assert_eq!(slow.queued(), 0);
    Ok(())
}
#[tokio::test]
async fn restart_reopens_durable_operation_and_read_only_receipt_result() -> anyhow::Result<()> {
    let pool = pool().await?;
    sqlx::query("create table if not exists mural_restart_fixture(id boolean primary key, event_id uuid,actor_id uuid,key uuid,result jsonb)").execute(&pool).await?;
    let repo = PgMuralRepository::new(pool.clone(), SyntheticAuthority);
    if std::env::var_os("MURAL_RESTART_VERIFY").is_none() {
        let (actor, _) = setup(&pool, 3600000).await?;
        let key = Uuid::new_v4();
        let result = repo.place(actor, key, pixel(1, 0, 15), &keys()).await?;
        sqlx::query("insert into mural_restart_fixture values(true,$1,$2,$3,$4)")
            .bind(actor.event)
            .bind(actor.actor)
            .bind(key)
            .bind(serde_json::to_value(&result)?)
            .execute(&pool)
            .await?;
    } else {
        let row = sqlx::query("select * from mural_restart_fixture where id")
            .fetch_one(&pool)
            .await?;
        let actor = MuralActor {
            event: row.get("event_id"),
            actor: row.get("actor_id"),
        };
        let result = repo
            .recover(actor, row.get("key"))
            .await?
            .expect("persisted original result");
        assert_eq!(
            serde_json::to_value(&result)?,
            row.get::<serde_json::Value, _>("result")
        );
        let snap = repo.snapshot(actor).await?;
        assert_eq!(snap.pixels[0], 15);
        assert_eq!(snap.revision, 2);
        assert!(matches!(
            repo.place(actor, Uuid::new_v4(), pixel(2, 1, 3), &keys())
                .await,
            Err(MuralError::Cooldown(Some(_)))
        ));
        let delta = repo.delta(actor, 1).await?;
        assert!(delta.contiguous());
        assert_eq!(delta.operations[0], result.public);
    }
    Ok(())
}

#[tokio::test]
async fn pending_expired_and_dependency_failed_receipt_reads_never_reexecute() -> anyhow::Result<()>
{
    let pool = pool().await?;
    let (actor, _) = setup(&pool, 0).await?;
    let repo = PgMuralRepository::new(pool.clone(), SyntheticAuthority);
    let pending = Uuid::new_v4();
    let expired = Uuid::new_v4();
    for (key, old) in [(pending, false), (expired, true)] {
        sqlx::query("insert into concert_command_receipts(receipt_id,actor_id,event_id,operation_id,idempotency_key,request_hash,target_id,state,created_at,expires_at) select $1,$2,$3,'placeConcertPixel',$4,$5,$3,'PENDING',t,t+interval '86400 seconds' from (select clock_timestamp()-case when $6 then interval '2 days' else interval '0 days' end t) stamp")
            .bind(Uuid::new_v4()).bind(actor.actor).bind(actor.event).bind(key).bind(format!("hmac-sha256:v1:mural-native:{}","00".repeat(32))).bind(old).execute(&pool).await?;
    }
    assert_eq!(repo.recover(actor, pending).await?, None);
    assert_eq!(
        repo.recover(actor, expired).await,
        Err(MuralError::Command(CommandError::Expired))
    );
    assert_eq!(counts(&pool, actor.event).await?, (0, 0, 2, 0));
    sqlx::query("update mural_fixture_authority set unavailable=true where event_id=$1")
        .bind(actor.event)
        .execute(&pool)
        .await?;
    assert!(repo.recover(actor, pending).await.is_err());
    assert_eq!(counts(&pool, actor.event).await?, (0, 0, 2, 0));
    Ok(())
}
