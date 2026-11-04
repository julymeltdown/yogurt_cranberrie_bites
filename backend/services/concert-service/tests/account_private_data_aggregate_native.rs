//! Owned PostgreSQL fixture for the aggregate coordinator. This is NOT a
//! production Auth transport and does not implement any real non-core owner.
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use concert_service::account_private_data::*;
use concert_service::world::account_deletion::*;
use sha2::{Digest, Sha256};
use sqlx::{
    migrate::Migrator,
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::{str::FromStr, sync::Arc, time::Duration as WallDuration};
use tokio::time::Instant;
use uuid::Uuid;

struct TestAuthority;
#[async_trait]
impl DeletionAuthority for TestAuthority {
    async fn verify(
        &self,
        c: &WorldDeletionCommand,
        d: Instant,
    ) -> Result<VerifiedDeletionSource, DeletionFailure> {
        let mut h = Sha256::new();
        h.update(b"BE42-PRIVATE-DATA-AGGREGATE-TEST-ONLY-v1");
        h.update(c.binding()?);
        VerifiedDeletionSource::from_verified_auth_source(c, d, h.finalize().into())
    }
}

/// Test-only cancellation owner. It opens a real transaction, writes a
/// sentinel, then waits indefinitely. Dropping the coordinator future must
/// roll back the transaction and cannot create a category receipt or ACK.
struct CancelOwner {
    pool: PgPool,
}

#[async_trait]
impl PrivateDataCategoryOwner for CancelOwner {
    async fn apply_category(
        &self,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        deadline: Instant,
    ) -> Result<PrivateDataCategoryReceipt, DeletionFailure> {
        source.check(command, deadline)?;
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| DeletionFailure::Unavailable)?;
        sqlx::query(
            "insert into public.be42_cancel_probe(user_id) values($1) on conflict do nothing",
        )
        .bind(command.user_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| DeletionFailure::Unavailable)?;
        std::future::pending::<()>().await;
        Err(DeletionFailure::Unavailable)
    }
}
async fn pool() -> Result<PgPool> {
    let options = PgConnectOptions::from_str(&std::env::var("CONCERT_TEST_DATABASE_URL")?)?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_private_data_aggregate"),
        "foreign database refused"
    );
    let p = PgPoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await?;
    let name: String = sqlx::query_scalar("select current_database()")
        .fetch_one(&p)
        .await?;
    anyhow::ensure!(
        name == "mcr_private_data_aggregate",
        "foreign database refused"
    );
    Migrator::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations"))
        .await?
        .run(&p)
        .await?;
    // Test-only witness for cancellation rollback. It is not a product schema
    // object and is created only in the marker-owned native database.
    sqlx::query("create table if not exists public.be42_cancel_probe(user_id uuid primary key)")
        .execute(&p)
        .await?;
    Ok(p)
}
fn cmd(user: Uuid) -> WorldDeletionCommand {
    WorldDeletionCommand {
        saga_id: Uuid::from_u128(42),
        user_id: user,
        target: WorldDeletionTarget::PrivateData,
        actor_id: user,
        // 0055's core uses PostgreSQL timestamp precision. The aggregate's
        // nanosecond representation is exercised by the pure regression and
        // must still bind the exact command used here.
        retention_until: "2070-01-01T00:00:00.123456000Z".parse().unwrap(),
    }
}

#[tokio::test]
#[ignore = "requires marker-owned PostgreSQL fixture"]
async fn aggregate_native() -> Result<()> {
    let p = pool().await?;
    let user = Uuid::from_u128(4200);
    let event = Uuid::from_u128(4300);
    let c = cmd(user);
    let restart = std::env::var("BE42_AGGREGATE_NATIVE_PHASE").as_deref() == Ok("restart");
    let fenced: bool = sqlx::query_scalar(
        "select exists(select 1 from concert_private_data_owner_progress where user_id=$1)",
    )
    .bind(user)
    .fetch_one(&p)
    .await?;
    anyhow::ensure!(
        !restart || fenced,
        "restart requires an already-fenced database"
    );
    // Seed exactly once. Create only ordinary profile rows here. The common
    // fence and private core receipt are initialized by the aggregate's public
    // protocol path, not forged by the test fixture.
    if !fenced {
        sqlx::query(
            "insert into concert_events(event_id,slug,title,phase) values($1,$2,$3,'OPEN')",
        )
        .bind(event)
        .bind(format!("private-data-aggregate-{event}"))
        .bind("Aggregate")
        .execute(&p)
        .await?;
        sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'DANGER',true,true,true)").bind(event).bind(user).execute(&p).await?;
        sqlx::query("insert into concert_avatar_settings(event_id,user_id,contract_version,representation,preset_id,manifest_id,manifest_version) values($1,$2,'1.1.0','whole_character','photo01_front_01','mcr_selected_static_20261103',1)").bind(event).bind(user).execute(&p).await?;
    }
    let authority: Arc<dyn DeletionAuthority> = Arc::new(TestAuthority);
    let coordinator = if restart {
        PrivateDataAggregateCoordinator::new(
            p.clone(),
            authority,
            PrivateDataCategoryOwners::empty(),
        )
    } else {
        let core: Arc<dyn PrivateDataCategoryOwner> = Arc::new(CorePrivateDataOwner::new(
            PrivateDataCoreDeletion::new(p.clone(), authority.clone()),
        ));
        PrivateDataAggregateCoordinator::new(
            p.clone(),
            authority,
            PrivateDataCategoryOwners::core_only(core),
        )
    };
    // This fixture runs one phase per test process. The first process performs
    // the sole owner round. The restart process opens a fresh pool and only
    // projects/replays the durable receipt; it must not invoke the owner again.
    if !fenced {
        let result = coordinator
            .apply(c.clone(), Instant::now() + WallDuration::from_secs(4))
            .await;
        anyhow::ensure!(matches!(result, Err(DeletionFailure::Incomplete)));
    }
    let common_fence_count: i64 = sqlx::query_scalar(
        "select count(*) from concert_world_account_deletion_fences where user_id=$1 and saga_id=$2",
    )
    .bind(user)
    .bind(c.saga_id)
    .fetch_one(&p)
    .await?;
    anyhow::ensure!(common_fence_count == 1);
    let common_saga: Uuid = sqlx::query_scalar(
        "select saga_id from concert_world_account_deletion_fences where user_id=$1",
    )
    .bind(user)
    .fetch_one(&p)
    .await?;
    anyhow::ensure!(common_saga == c.saga_id);
    let core_initialized: bool = sqlx::query_scalar(
        "select exists(select 1 from concert_private_data_deletion_core where user_id=$1)",
    )
    .bind(user)
    .fetch_one(&p)
    .await?;
    anyhow::ensure!(core_initialized);
    let seed_event_state: String = sqlx::query_scalar(
        "select theme from concert_participants where event_id=$1 and user_id=$2",
    )
    .bind(event)
    .bind(user)
    .fetch_one(&p)
    .await?;
    anyhow::ensure!(seed_event_state == "PARADE");
    let rows:Vec<(String,String)>=sqlx::query_as("select category,state from concert_private_data_category_progress where user_id=$1 order by ordinal").bind(user).fetch_all(&p).await?;
    anyhow::ensure!(rows
        .iter()
        .any(|x| x.0 == "CORE_PREFERENCES_AND_AVATAR" && x.1 == "COMPLETE"));
    anyhow::ensure!(rows.len() == PrivateDataCategory::ALL.len());
    let initially_completed = rows
        .iter()
        .filter(|x| x.1 == "COMPLETE")
        .map(|x| x.0.as_str())
        .collect::<Vec<_>>();
    anyhow::ensure!(initially_completed == vec!["CORE_PREFERENCES_AND_AVATAR"]);
    anyhow::ensure!(rows.iter().filter(|x| x.1 == "INCOMPLETE").count() == 6);
    let completed: Option<chrono::DateTime<Utc>> = sqlx::query_scalar(
        "select completed_at from concert_private_data_owner_progress where user_id=$1",
    )
    .bind(user)
    .fetch_one(&p)
    .await?;
    anyhow::ensure!(completed.is_none());
    // This read runs in both phases through the coordinator built by the
    // current process. On restart it therefore proves durable progress through
    // a newly constructed empty-owner registry, not cached first-process state.
    let progress = coordinator
        .progress(user)
        .await
        .map_err(|e| anyhow::anyhow!("progress: {e:?}"))?
        .ok_or_else(|| anyhow::anyhow!("progress"))?;
    anyhow::ensure!(progress.cursor == PrivateDataCategory::ALL.len());
    anyhow::ensure!(!progress.completed.is_empty() && !progress.missing.is_empty());
    anyhow::ensure!(progress.completed == vec![PrivateDataCategory::CorePreferencesAndAvatar]);
    anyhow::ensure!(progress.missing.len() == 6);
    anyhow::ensure!(progress.command.retention_until == c.retention_until);
    anyhow::ensure!(progress.command.retention_until.timestamp_subsec_nanos() == 123_456_000);
    anyhow::ensure!(progress.fence_revision > 0);
    let persisted_fence: i64 = sqlx::query_scalar(
        "select fence_revision from concert_private_data_owner_progress where user_id=$1",
    )
    .bind(user)
    .fetch_one(&p)
    .await?;
    anyhow::ensure!(progress.fence_revision == persisted_fence as u64);
    // Replay in both processes through a coordinator constructed here. The
    // restart registry has no core owner, so a core effect here would fail.
    let second = coordinator
        .apply(c.clone(), Instant::now() + WallDuration::from_secs(4))
        .await;
    anyhow::ensure!(matches!(second, Err(DeletionFailure::Incomplete)));
    let replay_cursor: i32 = sqlx::query_scalar(
        "select next_category from concert_private_data_owner_progress where user_id=$1",
    )
    .bind(user)
    .fetch_one(&p)
    .await?;
    anyhow::ensure!(replay_cursor == PrivateDataCategory::ALL.len() as i32);
    let replay_whole_ack: Option<chrono::DateTime<Utc>> = sqlx::query_scalar(
        "select completed_at from concert_private_data_owner_progress where user_id=$1",
    )
    .bind(user)
    .fetch_one(&p)
    .await?;
    anyhow::ensure!(replay_whole_ack.is_none());
    let post_replay_complete: i64 = sqlx::query_scalar(
        "select count(*) from concert_private_data_category_progress where user_id=$1 and state='COMPLETE'",
    )
    .bind(user)
    .fetch_one(&p)
    .await?;
    anyhow::ensure!(post_replay_complete == 1);
    let core_state: String =
        sqlx::query_scalar("select state from concert_private_data_deletion_core where user_id=$1")
            .bind(user)
            .fetch_one(&p)
            .await?;
    anyhow::ensure!(core_state == "COMPLETE");
    let seed_was_fenced: bool = sqlx::query_scalar(
        "select exists(select 1 from concert_private_data_participant_fences where user_id=$1)",
    )
    .bind(user)
    .fetch_one(&p)
    .await?;
    anyhow::ensure!(seed_was_fenced);
    let core_fence_count: i64 = sqlx::query_scalar(
        "select count(*) from concert_private_data_participant_fences where user_id=$1",
    )
    .bind(user)
    .fetch_one(&p)
    .await?;
    anyhow::ensure!(core_fence_count == 1);
    let fence_before_restart: i64 = sqlx::query_scalar(
        "select fence_revision from concert_private_data_owner_progress where user_id=$1",
    )
    .bind(user)
    .fetch_one(&p)
    .await?;
    let receipt = PrivateDataCategoryReceipt {
        command: c.clone(),
        category: PrivateDataCategory::IntroText,
        version: CATEGORY_VERSION,
        source_binding: [9; 32],
        progress_revision: 2,
        complete: true,
    };
    anyhow::ensure!(!receipt.verifies(&c, &[8; 32]));
    anyhow::ensure!(sqlx::query("update concert_private_data_owner_progress set completed_at=clock_timestamp() where user_id=$1").bind(user).execute(&p).await.is_err());
    anyhow::ensure!(sqlx::query(
        "delete from concert_private_data_category_progress where user_id=$1"
    )
    .bind(user)
    .execute(&p)
    .await
    .is_err());
    let before_version: i32 = sqlx::query_scalar("select category_version from concert_private_data_category_progress where user_id=$1 and state='INCOMPLETE' order by ordinal limit 1").bind(user).fetch_one(&p).await?;
    anyhow::ensure!(before_version == CATEGORY_VERSION);
    anyhow::ensure!(sqlx::query("update concert_private_data_category_progress set category_version=2 where user_id=$1 and state='INCOMPLETE' and ordinal=(select min(ordinal) from concert_private_data_category_progress where user_id=$1 and state='INCOMPLETE')").bind(user).execute(&p).await.is_err());
    let complete_category: String = sqlx::query_scalar("select category from concert_private_data_category_progress where user_id=$1 and state='COMPLETE' order by ordinal").bind(user).fetch_one(&p).await?;
    anyhow::ensure!(complete_category == "CORE_PREFERENCES_AND_AVATAR");
    // A missing row has no owner revision. The SQL guard admits exactly one
    // positive transition; a later terminal row stays immutable.
    let missing_category: String = sqlx::query_scalar("select category from concert_private_data_category_progress where user_id=$1 and state='INCOMPLETE' order by ordinal limit 1").bind(user).fetch_one(&p).await?;
    anyhow::ensure!(missing_category != "CORE_PREFERENCES_AND_AVATAR");
    anyhow::ensure!(sqlx::query("update concert_private_data_category_progress set owner_revision=2 where user_id=$1 and category=$2").bind(user).bind(&missing_category).execute(&p).await.is_err());
    anyhow::ensure!(sqlx::query("update concert_private_data_category_progress set completed_at=clock_timestamp() where user_id=$1 and category=$2 and state='INCOMPLETE' and completed_at is null").bind(user).bind(&missing_category).execute(&p).await.is_err());
    anyhow::ensure!(sqlx::query("update concert_private_data_category_progress set source_binding=decode(repeat('aa',32),'hex') where user_id=$1 and category=$2").bind(user).bind(&missing_category).execute(&p).await.is_err());
    let not_whole_after_missing:i64 = sqlx::query_scalar("select count(*) from concert_private_data_category_progress where user_id=$1 and state='INCOMPLETE'").bind(user).fetch_one(&p).await?;
    anyhow::ensure!(not_whole_after_missing == 6);
    anyhow::ensure!(sqlx::query("update concert_private_data_category_progress set owner_revision=3 where user_id=$1 and category=$2").bind(user).bind(&missing_category).execute(&p).await.is_err());
    anyhow::ensure!(sqlx::query("update concert_private_data_category_progress set state='COMPLETE' where user_id=$1 and category=(select category from concert_private_data_category_progress where user_id=$1 and state='INCOMPLETE' order by ordinal limit 1)").bind(user).execute(&p).await.is_err());
    anyhow::ensure!(sqlx::query("update concert_private_data_category_progress set owner_revision=3,state='INCOMPLETE' where user_id=$1 and category=$2").bind(user).bind(&missing_category).execute(&p).await.is_err());
    anyhow::ensure!(sqlx::query(
        "delete from concert_private_data_owner_progress where user_id=$1"
    )
    .bind(user)
    .execute(&p)
    .await
    .is_err());
    let core_fence_after: i64 = sqlx::query_scalar(
        "select count(*) from concert_private_data_participant_fences where user_id=$1",
    )
    .bind(user)
    .fetch_one(&p)
    .await?;
    anyhow::ensure!(core_fence_after == 1);
    let no_whole_ack: Option<chrono::DateTime<Utc>> = sqlx::query_scalar(
        "select completed_at from concert_private_data_owner_progress where user_id=$1",
    )
    .bind(user)
    .fetch_one(&p)
    .await?;
    anyhow::ensure!(no_whole_ack.is_none());
    let fence_after_restart: i64 = sqlx::query_scalar(
        "select fence_revision from concert_private_data_owner_progress where user_id=$1",
    )
    .bind(user)
    .fetch_one(&p)
    .await?;
    anyhow::ensure!(fence_after_restart == fence_before_restart);
    // Only the process after the physical restart performs the single guarded
    // missing-to-complete transition. The first process leaves all six missing
    // categories durable so the restart observes the original owner-round state.
    if restart {
        anyhow::ensure!(sqlx::query("update concert_private_data_category_progress set owner_revision=1,state='COMPLETE',completed_at=clock_timestamp() where user_id=$1 and category=$2 and state='INCOMPLETE' and owner_revision is null").bind(user).bind(&missing_category).execute(&p).await?.rows_affected()==1);
        let still_incomplete:i64=sqlx::query_scalar("select count(*) from concert_private_data_category_progress where user_id=$1 and state='INCOMPLETE'").bind(user).fetch_one(&p).await?;
        anyhow::ensure!(still_incomplete == 5);
        let no_ack_after_guarded_transition: Option<chrono::DateTime<Utc>> = sqlx::query_scalar(
            "select completed_at from concert_private_data_owner_progress where user_id=$1",
        )
        .bind(user)
        .fetch_one(&p)
        .await?;
        anyhow::ensure!(no_ack_after_guarded_transition.is_none());
    }
    // Run the cancellation probe before the first-phase owner round is already
    // fenced. The probe's own coordinator establishes a separate saga and the
    // owner's open transaction is dropped by the original deadline.
    if !restart && !fenced {
        let cancel_user = Uuid::from_u128(4202);
        let cancel_event = Uuid::from_u128(4302);
        sqlx::query(
            "insert into concert_events(event_id,slug,title,phase) values($1,$2,$3,'OPEN') on conflict(event_id) do nothing",
        )
        .bind(cancel_event)
        .bind(format!("private-data-cancel-{cancel_event}"))
        .bind("Cancel aggregate")
        .execute(&p)
        .await?;
        sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'DANGER',true,true,true) on conflict do nothing")
            .bind(cancel_event).bind(cancel_user).execute(&p).await?;
        let mut cancel_command = cmd(cancel_user);
        cancel_command.saga_id = Uuid::from_u128(43);
        let cancel_authority: Arc<dyn DeletionAuthority> = Arc::new(TestAuthority);
        let core: Arc<dyn PrivateDataCategoryOwner> = Arc::new(CorePrivateDataOwner::new(
            PrivateDataCoreDeletion::new(p.clone(), cancel_authority.clone()),
        ));
        let cancel: Arc<dyn PrivateDataCategoryOwner> = Arc::new(CancelOwner { pool: p.clone() });
        let owners = PrivateDataCategoryOwners::new([
            (PrivateDataCategory::CorePreferencesAndAvatar, core),
            (PrivateDataCategory::IntroText, cancel),
        ]);
        let coordinator = PrivateDataAggregateCoordinator::new(p.clone(), cancel_authority, owners);
        let cancelled = coordinator
            .apply(
                cancel_command,
                Instant::now() + WallDuration::from_millis(120),
            )
            .await;
        anyhow::ensure!(matches!(cancelled, Err(DeletionFailure::Unavailable)));
        tokio::time::sleep(WallDuration::from_millis(100)).await;
        let cancel_whole: Option<chrono::DateTime<Utc>> = sqlx::query_scalar(
            "select completed_at from concert_private_data_owner_progress where user_id=$1",
        )
        .bind(cancel_user)
        .fetch_one(&p)
        .await?;
        anyhow::ensure!(cancel_whole.is_none());
        let cancel_complete: i64 = sqlx::query_scalar(
            "select count(*) from concert_private_data_category_progress where user_id=$1 and state='COMPLETE'",
        )
        .bind(cancel_user)
        .fetch_one(&p)
        .await?;
        anyhow::ensure!(cancel_complete == 1);
        let probe_left: bool = tokio::time::timeout(
            WallDuration::from_secs(2),
            sqlx::query_scalar(
                "select exists(select 1 from public.be42_cancel_probe where user_id=$1)",
            )
            .bind(cancel_user)
            .fetch_one(&p),
        )
        .await??;
        anyhow::ensure!(!probe_left, "cancelled owner transaction committed");
    }
    Ok(())
}
