//! Owned PostgreSQL qualification for the bounded PrivateData core.
//!
//! The authority below checks an explicit test-only source binding. It does not
//! certify the canonical Auth repository transport.
use anyhow::Result;
use async_trait::async_trait;
use concert_service::account_private_data::*;
use concert_service::world::account_deletion::{
    DeletionAuthority, DeletionFailure, VerifiedDeletionSource, WorldDeletionCommand,
    WorldDeletionTarget,
};
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
        h.update(b"BE42-PRIVATE-DATA-TEST-ONLY-v1");
        h.update(c.binding()?);
        VerifiedDeletionSource::from_verified_auth_source(c, d, h.finalize().into())
    }
}

async fn owned_pool() -> Result<PgPool> {
    let options = PgConnectOptions::from_str(&std::env::var("CONCERT_TEST_DATABASE_URL")?)?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_private_data"),
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await?;
    let name: String = sqlx::query_scalar("select current_database()")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(name == "mcr_private_data", "foreign database refused");
    // A final-only database has no prior schema. The named native round owns
    // this database, so apply the exact current migration set before any row.
    let migrations =
        Migrator::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations")).await?;
    migrations.run(&pool).await?;
    Ok(pool)
}

fn command(user: Uuid) -> WorldDeletionCommand {
    WorldDeletionCommand {
        saga_id: Uuid::from_u128(41),
        user_id: user,
        target: WorldDeletionTarget::PrivateData,
        actor_id: user,
        retention_until: "2070-01-01T00:00:00Z".parse().unwrap(),
    }
}

#[tokio::test]
#[ignore = "requires marker-owned PostgreSQL fixture"]
async fn private_data_core_native() -> Result<()> {
    let pool = owned_pool().await?;
    // A dedicated connection is held by the fixture so the test can prove the
    // canonical advisory lock is the first thing the erase transaction takes.
    let mut observer = pool.acquire().await?;
    let user = Uuid::from_u128(4200);
    let other = Uuid::from_u128(4201);
    let event_a = Uuid::from_u128(4300);
    let event_b = Uuid::from_u128(4301);
    let page_events: Vec<Uuid> = (0..129).map(|n| Uuid::from_u128(5000 + n)).collect();
    // A physical restart reruns this ignored case against durable rows. Seed
    // only before the first receipt; a committed fence correctly refuses any
    // attempt to recreate those participants during the replay.
    let already_fenced: bool = sqlx::query_scalar(
        "select exists(select 1 from public.concert_private_data_participant_fences where user_id=$1)",
    )
    .bind(user)
    .fetch_one(&pool)
    .await?;
    anyhow::ensure!(already_fenced || user != other);
    if !already_fenced {
        for event in page_events.iter().copied().chain([event_a, event_b]) {
            sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,$3,'OPEN') on conflict(event_id) do nothing")
                .bind(event).bind(format!("private-data-{event}")).bind(format!("Private data {event}")).execute(&pool).await?;
            sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'DANGER',true,true,true) on conflict do nothing")
                .bind(event).bind(user).execute(&pool).await?;
            sqlx::query("insert into concert_avatar_settings(event_id,user_id,contract_version,representation,preset_id,manifest_id,manifest_version) values($1,$2,'1.1.0','whole_character','photo01_front_01','mcr_selected_static_20261103',1) on conflict do nothing")
                .bind(event).bind(user).execute(&pool).await?;
        }
        sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,true) on conflict do nothing")
            .bind(event_a).bind(other).execute(&pool).await?;
    }
    let c = command(user);
    let deletion = PrivateDataCoreDeletion::new(pool.clone(), Arc::new(TestAuthority));
    // Hold the personal lock on an independent connection. This models a
    // normal writer/erasure contention window without forging authority.
    sqlx::query("select pg_advisory_lock(hashtextextended('mcr-world-account-deletion:' || $1::uuid::text,0))")
        .bind(user)
        .execute(&mut *observer)
        .await?;
    let contender = {
        let pool = pool.clone();
        let command = c.clone();
        tokio::spawn(async move {
            PrivateDataCoreDeletion::new(pool, Arc::new(TestAuthority))
                .apply(command, Instant::now() + WallDuration::from_secs(4))
                .await
        })
    };
    tokio::time::sleep(WallDuration::from_millis(80)).await;
    let lock_waiting: bool = sqlx::query_scalar(
        "select exists(select 1 from pg_locks where locktype='advisory' and not granted)",
    )
    .fetch_one(&mut *observer)
    .await?;
    // A COMPLETE receipt retries without restructuring rows; on that restart
    // path, contention against the retained user lock is not a new erase.
    anyhow::ensure!(
        lock_waiting || already_fenced,
        "erasure did not take user-first lock"
    );
    sqlx::query("select pg_advisory_unlock(hashtextextended('mcr-world-account-deletion:' || $1::uuid::text,0))")
        .bind(user)
        .execute(&mut *observer)
        .await?;
    let ack = contender
        .await
        .map_err(|error| anyhow::anyhow!(error))?
        .map_err(|error| anyhow::anyhow!("private-data apply failed: {error:?}"))?;
    anyhow::ensure!(
        ack.verifies(&c)
            && ack.scanned_events >= 131
            && ack.neutralized_participants >= 131
            && ack.removed_avatars >= 2
    );
    let rows: Vec<(String,bool,bool,bool)> = sqlx::query_as("select theme,low_power,reduced_motion,sound_enabled from concert_participants where user_id=$1")
        .bind(user).fetch_all(&pool).await?;
    anyhow::ensure!(
        rows.len() == 131
            && rows
                .iter()
                .all(|r| r == &("PARADE".into(), false, false, false))
    );
    let avatars: i64 =
        sqlx::query_scalar("select count(*) from concert_avatar_settings where user_id=$1")
            .bind(user)
            .fetch_one(&pool)
            .await?;
    anyhow::ensure!(avatars == 0);
    let other_theme: String = sqlx::query_scalar(
        "select theme from concert_participants where event_id=$1 and user_id=$2",
    )
    .bind(event_a)
    .bind(other)
    .fetch_one(&pool)
    .await?;
    anyhow::ensure!(other_theme == "REVENGE");
    let retry = deletion
        .apply(c.clone(), Instant::now() + WallDuration::from_secs(4))
        .await
        .map_err(|error| anyhow::anyhow!("private-data retry failed: {error:?}"))?;
    anyhow::ensure!(retry == ack);
    anyhow::ensure!(
        sqlx::query("update concert_participants set theme='DANGER' where user_id=$1")
            .bind(user)
            .execute(&pool)
            .await
            .is_err()
    );
    anyhow::ensure!(sqlx::query("insert into concert_avatar_settings(event_id,user_id,contract_version,representation,preset_id,manifest_id,manifest_version) values($1,$2,'1.1.0','whole_character','photo01_front_01','mcr_selected_static_20261103',1)")
        .bind(event_a).bind(user).execute(&pool).await.is_err());
    let core_state: String =
        sqlx::query_scalar("select state from concert_private_data_deletion_core where user_id=$1")
            .bind(user)
            .fetch_one(&pool)
            .await?;
    anyhow::ensure!(core_state == "COMPLETE");
    Ok(())
}
