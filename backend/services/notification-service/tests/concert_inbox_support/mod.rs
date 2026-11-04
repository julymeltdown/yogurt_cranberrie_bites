//! BE33 test-only ownership guards adapted from BE37 at a25b855.
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::{path::PathBuf, str::FromStr};
pub async fn owned_pool(database: &str) -> anyhow::Result<PgPool> {
    anyhow::ensure!(
        matches!(
            database,
            "mcr_concert_notification" | "mcr_concert_notification_source"
        ),
        "unknown fixture refused"
    );
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
    // The cross-owner tests use two explicitly supplied databases in the same
    // private cluster. Each URL is checked against its requested database below;
    // never rewrite an arbitrary URL or fall back to an ambient database.
    let variable = match database {
        "mcr_concert_notification" => "CONCERT_TEST_NOTIFICATION_DATABASE_URL",
        _ => "CONCERT_TEST_DATABASE_URL",
    };
    let options = PgConnectOptions::from_str(&std::env::var(variable)?)
        .map_err(|_| anyhow::anyhow!("invalid owned fixture options"))?;
    anyhow::ensure!(
        options.get_database() == Some(database)
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
