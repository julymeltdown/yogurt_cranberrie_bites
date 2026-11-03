//! SQLx fresh install, forward upgrade, immutable receipts and reapply qualification.
use sqlx::{migrate::Migrator, postgres::PgPoolOptions, PgPool};
use std::{borrow::Cow, path::Path};
use uuid::Uuid;

pub async fn qualify(server: &PgPool) -> anyhow::Result<()> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let full = Migrator::new(directory.clone()).await?;
    assert_eq!(full.iter().last().unwrap().version, 18);
    let baseline = Migrator {
        migrations: Cow::Owned(full.iter().filter(|m| m.version < 18).cloned().collect()),
        ..Migrator::DEFAULT
    };
    for upgrade in [false, true] {
        // Closed generated identifier on the marker-verified Unix-socket fixture server.
        let database = format!("mcr_be41_{}", Uuid::new_v4().simple());
        sqlx::query(&format!("CREATE DATABASE {database} TEMPLATE template0"))
            .execute(server)
            .await?;
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect_with(
                server
                    .connect_options()
                    .as_ref()
                    .clone()
                    .database(&database),
            )
            .await?;
        let mut before = None;
        if upgrade {
            baseline.run(&pool).await?;
            let event = Uuid::new_v4();
            sqlx::query("INSERT INTO concert_events(event_id,slug,title,phase) VALUES($1,$2,'Upgrade preserved event','LIVE')")
                .bind(event).bind(event.to_string()).execute(&pool).await?;
            before = Some(
                sqlx::query_scalar::<_, serde_json::Value>(
                    "SELECT to_jsonb(e) FROM concert_events e",
                )
                .fetch_one(&pool)
                .await?,
            );
        }
        migration_runtime::prepare_database(
            &pool,
            &directory,
            migration_runtime::MigrationMode::Startup,
            &[],
        )
        .await?;
        migration_runtime::verify_receipts(&pool, &full).await?;
        let ledger: serde_json::Value = sqlx::query_scalar(
            "SELECT jsonb_agg(to_jsonb(m) ORDER BY version) FROM _sqlx_migrations m",
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(ledger.as_array().unwrap().len(), full.iter().count());
        assert!(ledger
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["success"] == true));
        let exists: bool = sqlx::query_scalar("SELECT to_regclass('concert_closure_jobs') IS NOT NULL AND to_regclass('concert_closure_effects') IS NOT NULL AND to_regclass('concert_closure_archives') IS NOT NULL")
            .fetch_one(&pool).await?;
        assert!(exists);
        if let Some(before) = before {
            let after: serde_json::Value =
                sqlx::query_scalar("SELECT to_jsonb(e) FROM concert_events e")
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(before, after, "0018 must preserve existing event rows");
        }
        migration_runtime::prepare_database(
            &pool,
            &directory,
            migration_runtime::MigrationMode::Startup,
            &[],
        )
        .await?;
        migration_runtime::verify_receipts(&pool, &full).await?;
        let reapplied: serde_json::Value = sqlx::query_scalar(
            "SELECT jsonb_agg(to_jsonb(m) ORDER BY version) FROM _sqlx_migrations m",
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(
            ledger, reapplied,
            "reapply must preserve every exact receipt"
        );
        pool.close().await;
        sqlx::query(&format!("DROP DATABASE {database}"))
            .execute(server)
            .await?;
    }
    println!("BE41_MIGRATIONS=fresh,forward0018,preserved_event,exact_receipts,reapply");
    Ok(())
}
