//! Full current SQLx ledger qualification on the already marker-verified fixture server.
use sqlx::{migrate::Migrator, postgres::PgPoolOptions, PgPool};
use std::{borrow::Cow, path::Path};
use tokio::sync::OnceCell;
use uuid::Uuid;
static QUALIFIED: OnceCell<()> = OnceCell::const_new();

pub async fn prepare(main: &PgPool) -> anyhow::Result<()> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let full = Migrator::new(directory).await?;
    let versions: Vec<i64> = full.iter().map(|m| m.version).collect();
    anyhow::ensure!(
        versions == vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 16, 18, 19, 20],
        "unexpected current migration package"
    );
    full.run(main).await?;
    migration_runtime::verify_receipts(main, &full).await?;
    QUALIFIED.get_or_try_init(|| qualify(main, &full)).await?;
    Ok(())
}
async fn ledger(pool: &PgPool) -> anyhow::Result<serde_json::Value> {
    Ok(sqlx::query_scalar(
        "select jsonb_agg(to_jsonb(m) order by version) from public._sqlx_migrations m",
    )
    .fetch_one(pool)
    .await?)
}
async fn qualify(server: &PgPool, full: &Migrator) -> anyhow::Result<()> {
    let baseline = Migrator {
        migrations: Cow::Owned(full.iter().filter(|m| m.version <= 18).cloned().collect()),
        ..Migrator::DEFAULT
    };
    for (database, upgrade) in [("mcr_mural_fresh", false), ("mcr_mural_upgrade", true)] {
        let exists: bool =
            sqlx::query_scalar("select exists(select 1 from pg_database where datname=$1)")
                .bind(database)
                .fetch_one(server)
                .await?;
        if !exists {
            sqlx::query(&format!("create database {database} template template0"))
                .execute(server)
                .await?;
        }
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect_with(server.connect_options().as_ref().clone().database(database))
            .await?;
        let mut preserved = None;
        if upgrade && !exists {
            baseline.run(&pool).await?;
            migration_runtime::verify_receipts(&pool, &baseline).await?;
            let event = Uuid::new_v4();
            sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,'mural-upgrade-preserved','Synthetic upgrade preservation','OPEN')").bind(event).execute(&pool).await?;
            sqlx::query("insert into concert_command_receipts(receipt_id,actor_id,event_id,operation_id,idempotency_key,request_hash,target_id,state,failure_reason) values($1,$2,$3,'placeConcertPixel',$4,$5,$3,'FAILED','INVALID_INPUT')")
                .bind(Uuid::new_v4()).bind(Uuid::new_v4()).bind(event).bind(Uuid::new_v4()).bind(format!("hmac-sha256:v1:upgrade:{}","00".repeat(32))).execute(&pool).await?;
            preserved=Some(sqlx::query_scalar::<_,serde_json::Value>("select jsonb_build_object('event',(select to_jsonb(e) from concert_events e),'receipt',(select to_jsonb(r) from concert_command_receipts r))").fetch_one(&pool).await?);
        }
        full.run(&pool).await?;
        migration_runtime::verify_receipts(&pool, full).await?;
        if let Some(before) = preserved {
            let after:serde_json::Value=sqlx::query_scalar("select jsonb_build_object('event',(select to_jsonb(e) from concert_events e),'receipt',(select to_jsonb(r) from concert_command_receipts r))").fetch_one(&pool).await?;
            anyhow::ensure!(
                before == after,
                "forward upgrade changed published event or receipt"
            );
        }
        let before = ledger(&pool).await?;
        full.run(&pool).await?;
        migration_runtime::verify_receipts(&pool, full).await?;
        anyhow::ensure!(
            before == ledger(&pool).await?,
            "reapply changed migration receipts"
        );
        pool.close().await;
    }
    println!(
        "MURAL_LEDGER=fresh,forward0018_to0020,preserved_event_receipt,reapply,exact_checksums"
    );
    Ok(())
}
