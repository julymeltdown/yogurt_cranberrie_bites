//! Adapted fixture ownership checks from owner_authority_support/environment.rs
//! at 93ef0d895dcc4b247b727e2cccba0bb84576b262. No existing helper is mutated.
use async_trait::async_trait;
use chrono::{Duration, Utc};
use concert_application::{commands::*, mural::*};
use concert_service::{
    adapters::out::repository::mural::MuralAuthority,
    community::{giveaway::*, *},
};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgConnection, PgPool, Row,
};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    str::FromStr,
    time::Instant,
};
use uuid::Uuid;

pub async fn pool() -> anyhow::Result<PgPool> {
    let root = PathBuf::from(std::env::var("QA03_ROOT")?);
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        root.is_absolute()
            && root.canonicalize()? == root
            && info.is_dir()
            && !info.file_type().is_symlink()
            && info.permissions().mode() & 0o077 == 0,
        "private QA03 root required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino()) == std::env::var("QA03_IDENTITY")?,
        "replaced root"
    );
    let marker = root.join("mcr-owner.json");
    let m = std::fs::symlink_metadata(&marker)?;
    anyhow::ensure!(
        m.is_file() && !m.file_type().is_symlink() && m.permissions().mode() & 0o077 == 0,
        "private marker required"
    );
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(marker)?)?;
    anyhow::ensure!(
        value["token"].as_str() == Some(std::env::var("QA03_TOKEN")?.as_str())
            && value["uid"].as_u64() == Some(info.uid() as u64),
        "foreign marker"
    );
    let options = PgConnectOptions::from_str(&std::env::var("QA03_DATABASE_URL")?)?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_qa_races")
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(12)
        .connect_with(options.application_name("qa03-races"))
        .await?;
    let row = sqlx::query("select current_setting('data_directory') data,current_setting('listen_addresses') listeners,(select system_identifier::text from pg_control_system()) system").fetch_one(&pool).await?;
    anyhow::ensure!(
        row.get::<String, _>("data") == root.join("data").to_string_lossy()
            && row.get::<String, _>("listeners").is_empty()
            && row.get::<String, _>("system") == std::env::var("QA03_SYSTEM")?,
        "foreign PostgreSQL identity"
    );
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    sqlx::migrate::Migrator::new(path.as_path())
        .await?
        .run(&pool)
        .await?;
    sqlx::raw_sql("create table if not exists qa03_acl(event uuid,actor uuid,allowed boolean not null,primary key(event,actor)); create table if not exists qa03_places(event uuid,place uuid,primary key(event,place)); create table if not exists qa03_restart(kind text primary key,payload jsonb not null)").execute(&pool).await?;
    Ok(pool)
}

/// Test-only ACL, backed by rows held through the adapter transaction. This
/// isolates database invariants and does not manufacture a canonical owner scope.
pub struct IsolatedAuthority;
#[async_trait]
impl CommunityAuthority for IsolatedAuthority {
    async fn actor_in(
        &self,
        c: &mut PgConnection,
        actor: Uuid,
        event: Uuid,
        access: CommunityAccess,
    ) -> Result<(), CommandError> {
        CommunityAuthority::authorize_in(self, c, actor, event, actor, access).await
    }
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        actor: Uuid,
        event: Uuid,
        subject: Uuid,
        _: CommunityAccess,
    ) -> Result<(), CommandError> {
        for user in [actor, subject] {
            let allowed: Option<bool> = sqlx::query_scalar(
                "select allowed from qa03_acl where event=$1 and actor=$2 for share",
            )
            .bind(event)
            .bind(user)
            .fetch_optional(&mut *c)
            .await
            .map_err(|_| CommandError::Unavailable)?;
            if allowed != Some(true) {
                return Err(CommandError::Denied);
            }
        }
        Ok(())
    }
    async fn place_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        place: Uuid,
    ) -> Result<(), CommandError> {
        let found: Option<Uuid> = sqlx::query_scalar(
            "select place from qa03_places where event=$1 and place=$2 for share",
        )
        .bind(event)
        .bind(place)
        .fetch_optional(c)
        .await
        .map_err(|_| CommandError::Unavailable)?;
        if found.is_some() {
            Ok(())
        } else {
            Err(CommandError::Denied)
        }
    }
}
#[async_trait]
impl GiftAuthority for IsolatedAuthority {
    async fn settings_in(
        &self,
        _: &mut PgConnection,
        _: Uuid,
    ) -> Result<GiftSettings, CommandError> {
        Ok(GiftSettings {
            pickup_grace_seconds: 60,
            event_cutoff: "2099-01-01T00:00:00Z".parse().unwrap(),
        })
    }
    async fn media_in(
        &self,
        _: &mut PgConnection,
        _: Uuid,
        _: Uuid,
        _: Uuid,
        media: Option<Uuid>,
    ) -> Result<(), CommandError> {
        if media.is_none() {
            Ok(())
        } else {
            Err(CommandError::Denied)
        }
    }
    async fn meeting_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        m: &concert_domain::community::giveaway::PickupMeeting,
    ) -> Result<(), CommandError> {
        self.place_in(c, event, m.place_id).await
    }
}
#[async_trait]
impl MuralAuthority for IsolatedAuthority {
    async fn authorize_in(&self, c: &mut PgConnection, a: MuralActor) -> Result<(), MuralError> {
        self.actor_in(c, a.actor, a.event, CommunityAccess::WriteParties)
            .await
            .map_err(|e| {
                if e == CommandError::Denied {
                    MuralError::Denied
                } else {
                    MuralError::PolicyUnavailable
                }
            })
    }
}
pub fn keys() -> FingerprintKeys {
    FingerprintKeys::new(SigningKey::new("qa03-isolated", &[82; 32]).unwrap(), vec![]).unwrap()
}
pub async fn allow(pool: &PgPool, event: Uuid, actor: Uuid) -> anyhow::Result<()> {
    sqlx::query("insert into qa03_acl values($1,$2,true) on conflict do nothing")
        .bind(event)
        .bind(actor)
        .execute(pool)
        .await?;
    Ok(())
}
pub async fn event(pool: &PgPool) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
    let (event, owner, place) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'QA03 isolated race','OPEN')").bind(event).bind(format!("qa03-{event}")).execute(pool).await?;
    allow(pool, event, owner).await?;
    sqlx::query("insert into qa03_places values($1,$2)")
        .bind(event)
        .bind(place)
        .execute(pool)
        .await?;
    Ok((event, owner, place))
}

/// The caller holds the root row in an open transaction. Require a lock-wait
/// graph rooted at that backend, not a timing sleep or merely spawned futures.
pub async fn waiting(pool: &PgPool, holder: i32, count: i64) -> anyhow::Result<()> {
    let deadline = Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let observed:i64=sqlx::query_scalar("with recursive waits(pid) as (select pid from pg_stat_activity where $1=any(pg_blocking_pids(pid)) union select a.pid from pg_stat_activity a join waits w on w.pid=any(pg_blocking_pids(a.pid))) select count(distinct pid) from waits where pid in(select pid from pg_stat_activity where application_name='qa03-races')").bind(holder).fetch_one(pool).await?;
        if observed >= count {
            println!("QA03_INTERLEAVING holder={holder} blocked={observed} required={count}");
            return Ok(());
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "forced interleaving not reached: {observed}/{count}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}
/// Record the exact rows a case observed. This is evidence output, never a
/// synchronization primitive.
pub fn state(tag: &str, value: serde_json::Value) {
    println!("QA03_STATE {tag} {value}");
}
/// The backend currently waiting behind the held root, once `waiting` has
/// proven the lock graph contains it.
pub async fn blocked_backend(pool: &PgPool, holder: i32) -> anyhow::Result<i32> {
    let pid: Option<i32> = sqlx::query_scalar(
        "select pid from pg_stat_activity where application_name='qa03-races' and pid<>$1 and $1=any(pg_blocking_pids(pid)) order by pid limit 1",
    )
    .bind(holder)
    .fetch_optional(pool)
    .await?;
    pid.ok_or_else(|| anyhow::anyhow!("no blocked backend behind holder {holder}"))
}
pub async fn terminal(pool: &PgPool) -> anyhow::Result<()> {
    let pending: i64 =
        sqlx::query_scalar("select count(*) from concert_command_receipts where state='PENDING'")
            .fetch_one(pool)
            .await?;
    let prepared: i64 = sqlx::query_scalar("select count(*) from pg_prepared_xacts")
        .fetch_one(pool)
        .await?;
    anyhow::ensure!(
        pending == 0 && prepared == 0,
        "orphan pending/prepared state"
    );
    Ok(())
}
pub fn future() -> chrono::DateTime<Utc> {
    // Aggregate JSON and PostgreSQL indexes use the same persisted precision.
    chrono::DateTime::from_timestamp_micros((Utc::now() + Duration::hours(1)).timestamp_micros())
        .unwrap()
}
