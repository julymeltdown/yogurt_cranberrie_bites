//! SQLx fresh install, forward upgrade, immutable receipts and reapply qualification.
use sqlx::{migrate::Migrator, postgres::PgPoolOptions, PgPool};
use std::{borrow::Cow, path::Path};
use uuid::Uuid;

pub async fn qualify(server: &PgPool) -> anyhow::Result<()> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let full = Migrator::new(directory.clone()).await?;
    // The ledger is every assigned migration file, exactly once, in version
    // order. Derive the expected set from the directory so newly assigned
    // migrations are covered instead of silently failing this qualification.
    let mut expected = Vec::new();
    for entry in std::fs::read_dir(&directory)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("sql") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let version: i64 = name
            .split('_')
            .next()
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| anyhow::anyhow!("migration {name} has no numeric version"))?;
        anyhow::ensure!(
            !expected.contains(&version),
            "duplicate migration version {version}"
        );
        expected.push(version);
    }
    expected.sort_unstable();
    assert_eq!(
        full.iter().map(|m| m.version).collect::<Vec<_>>(),
        expected,
        "every assigned migration must be present exactly once and ordered"
    );
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(std::fs::read(directory.join("0018_event_lifecycle.sql"))?)
        ),
        "8a93ed7e1b02345c3071cb0583384d61bb1e0b509e8939daf568f9ea352e2cb0",
        "published 0018 cannot change during forward qualification"
    );
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
