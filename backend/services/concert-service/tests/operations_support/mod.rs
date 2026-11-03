//! Ledger setup only; all paths, socket and database ownership are verified before mutation.
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    str::FromStr,
};
async fn owned_pool() -> anyhow::Result<PgPool> {
    let root = PathBuf::from(
        std::env::var("CONCERT_TEST_OWNER_ROOT")
            .map_err(|_| anyhow::anyhow!("owned fixture root required"))?,
    );
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        info.is_dir() && !info.file_type().is_symlink() && info.permissions().mode() & 0o077 == 0,
        "private owned root required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino()) == std::env::var("CONCERT_TEST_OWNER_IDENTITY")?,
        "fixture inode changed"
    );
    let marker_path = root.join("mcr-owner.json");
    let marker_info = std::fs::symlink_metadata(&marker_path)?;
    anyhow::ensure!(
        marker_info.is_file()
            && !marker_info.file_type().is_symlink()
            && marker_info.permissions().mode() & 0o077 == 0,
        "private marker required"
    );
    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(marker_path)?)?;
    anyhow::ensure!(
        marker["token"].as_str() == Some(std::env::var("CONCERT_TEST_OWNER_TOKEN")?.as_str())
            && marker["uid"].as_u64() == Some(info.uid() as u64),
        "fixture owner mismatch"
    );
    let options = PgConnectOptions::from_str(&std::env::var("CONCERT_TEST_DATABASE_URL")?)
        .map_err(|_| anyhow::anyhow!("invalid owned fixture options"))?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_concert_be_operations_forward")
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await
        .map_err(|_| anyhow::anyhow!("owned fixture connection failed"))?;
    let actual: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        actual == root.join("data").to_string_lossy(),
        "foreign data directory refused"
    );
    Ok(pool)
}

/// Execute explicitly with --ignored --exact from operations_smoke.py. This
/// creates a genuine SQLx ledger through published 0020, never fabricated rows.
#[tokio::test]
#[ignore = "explicit owned forward-ledger harness checkpoint"]
async fn published_0020_checkpoint() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let root = PathBuf::from(std::env::var("CONCERT_TEST_OWNER_ROOT")?);
    let packaged = root.join("published-migrations-through-0020");
    std::fs::create_dir(&packaged)?;
    std::fs::set_permissions(&packaged, std::fs::Permissions::from_mode(0o700))?;
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let expected = [1_i64, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 16, 18, 19, 20];
    let mut actual = Vec::new();
    for entry in std::fs::read_dir(source)? {
        let path = entry?.path();
        if path.extension().and_then(|v| v.to_str()) != Some("sql") {
            continue;
        }
        let name = path.file_name().unwrap().to_str().unwrap();
        let version: i64 = name.split('_').next().unwrap().parse()?;
        if version > 20 {
            continue;
        }
        anyhow::ensure!(
            expected.contains(&version),
            "unexpected published baseline migration"
        );
        std::fs::copy(&path, packaged.join(name))?;
        actual.push(version);
    }
    actual.sort();
    anyhow::ensure!(actual == expected, "published baseline set differs");
    let migrator = migration_runtime::load_migrations(&packaged).await?;
    migrator.run(&pool).await?;
    migration_runtime::verify_receipts(&pool, &migrator).await?;
    let ledger: i64 = sqlx::query_scalar("select count(*) from _sqlx_migrations where success")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(ledger == 16, "published ledger was not fully applied");
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values('00000000-0000-4000-8000-000000000021','operations-forward-fixture','Synthetic published checkpoint','OPEN')").execute(&pool).await?;
    sqlx::query("insert into concert_command_receipts(receipt_id,actor_id,event_id,operation_id,idempotency_key,request_hash,target_id,state,resource_id,resource_revision,result_status) values('00000000-0000-4000-8000-000000000022','00000000-0000-4000-8000-000000000023','00000000-0000-4000-8000-000000000021','setConcertSettings','00000000-0000-4000-8000-000000000024',$1,'00000000-0000-4000-8000-000000000021','SUCCEEDED','00000000-0000-4000-8000-000000000021',1,200)").bind(format!("hmac-sha256:v1:forward:{}","0".repeat(64))).execute(&pool).await?;
    // Only our forward fixture intentionally collides at late 0021 DDL. Earlier
    // CREATE TABLE/trigger work and the new ledger receipt must all roll back.
    sqlx::query("create table concert_operated_place_releases(owned_fault_fixture boolean)")
        .execute(&pool)
        .await?;
    pool.close().await;
    Ok(())
}
