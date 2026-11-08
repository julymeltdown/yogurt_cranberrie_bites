//! Shared fixture for the WP-05 intro post tests.
//!
//! The database is the runner-owned private cluster from
//! `backend/scripts/mcr/wp05_intro_cluster.py`; the URL arrives through
//! `CONCERT_TEST_DATABASE_URL` and must name the `mcr_owner` role.
#![allow(dead_code)]

use sqlx::PgPool;
use uuid::Uuid;

pub async fn pool() -> anyhow::Result<PgPool> {
    let url = std::env::var("CONCERT_TEST_DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("CONCERT_TEST_DATABASE_URL is required"))?;
    let options: sqlx::postgres::PgConnectOptions = url.parse()?;
    anyhow::ensure!(
        options.get_username() == "mcr_owner",
        "intro fixture must use the owned cluster"
    );
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;
    migrate(&pool).await?;
    Ok(pool)
}

async fn migrate(pool: &PgPool) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('wp05-intro-migrations',0))")
        .execute(&mut *tx)
        .await?;
    let applied: bool =
        sqlx::query_scalar("select to_regclass('public.intro_like_notifications') is not null")
            .fetch_one(&mut *tx)
            .await?;
    if applied {
        tx.commit().await?;
        return Ok(());
    }
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut entries: Vec<_> = std::fs::read_dir(&directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|extension| extension == "sql")
                && !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(".down.sql"))
        })
        .collect();
    entries.sort();
    for path in entries {
        let sql = std::fs::read_to_string(&path)?;
        sqlx::raw_sql(&sql).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// The constraint name a failed statement reported, if any.
pub fn constraint(error: &sqlx::Error) -> Option<String> {
    match error {
        sqlx::Error::Database(database) => database.constraint().map(str::to_owned),
        _ => None,
    }
}

/// A profile fixture. `discovery_profiles` is the projection the intro
/// projections join for a nickname, so it is seeded too.
pub struct Profile {
    pub profile_id: Uuid,
    pub account_id: Uuid,
    pub nickname: String,
}

pub async fn profile(pool: &PgPool, tag: &str) -> anyhow::Result<Profile> {
    let profile_id = Uuid::new_v4();
    let account_id = Uuid::new_v4();
    let nickname = format!("wp05-{tag}-{}", &profile_id.simple().to_string()[..8]);
    sqlx::query(
        "insert into discovery_profiles(profile_id,account_id,nickname,bio,visibility) \
         values($1,$2,$3,'fixture bio','PUBLIC')",
    )
    .bind(profile_id)
    .bind(account_id)
    .bind(&nickname)
    .execute(pool)
    .await?;
    Ok(Profile {
        profile_id,
        account_id,
        nickname,
    })
}

/// A `READY` profile media row owned by `owner` with `purpose`.
pub async fn media(pool: &PgPool, owner: Uuid, purpose: &str) -> anyhow::Result<Uuid> {
    let media = Uuid::new_v4();
    sqlx::query(
        "insert into profile_media(media_id,owner_profile_id,purpose,content_type,state,\
         declared_bytes,sanitized,width,height,input_sha256) \
         values($1,$2,$3,'image/png','READY',4,decode('89504e47','hex'),2,2,\
         decode(repeat('ab',32),'hex'))",
    )
    .bind(media)
    .bind(owner)
    .bind(purpose)
    .execute(pool)
    .await?;
    Ok(media)
}

/// Count rows in a table, for the "nothing else changed" assertions.
pub async fn count(pool: &PgPool, table: &str) -> anyhow::Result<i64> {
    // `table` is a literal from the test, never user input.
    Ok(sqlx::query_scalar(&format!("select count(*) from {table}"))
        .fetch_one(pool)
        .await?)
}
